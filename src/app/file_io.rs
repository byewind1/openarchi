use super::*;
use crate::model::ModelAdapter;

/// One immutable save snapshot in flight; subsequent edits stay in the host and coalesce.
pub(super) struct RecoveryWrite {
    session: u64,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl RecoveryWrite {
    fn is_finished(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(|worker| worker.is_finished())
    }

    fn finish(mut self) -> Result<(), String> {
        self.worker
            .take()
            .expect("owned recovery worker")
            .join()
            .unwrap_or_else(|_| {
                Err("Recovery could not be completed. Recovery files have been kept.".into())
            })
    }
}

impl Drop for RecoveryWrite {
    fn drop(&mut self) {
        // Normal window teardown must retain a complete checkpoint and release its lock.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// File-dialog state only; the host remains the sole live project owner.
#[derive(Default)]
pub(super) struct ProjectOpenUi {
    pub(super) directory: Option<PathBuf>,
    pub(super) preference: Option<PathBuf>,
    selected: Option<PathBuf>,
    confirm_unsaved: bool,
    #[cfg(test)]
    pub(super) choices: std::collections::VecDeque<Option<PathBuf>>,
}

impl ProjectOpenUi {
    pub(super) fn initialize(&mut self, config: &std::path::Path) {
        let preference = config.join("last-open-directory.json");
        self.directory = std::fs::read(&preference)
            .ok()
            .filter(|bytes| bytes.len() <= 32768)
            .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok())
            .filter(|path| path.is_absolute() && path.is_dir());
        self.preference = Some(preference);
    }

    fn remember(&mut self, path: &std::path::Path) {
        let Some(parent) = path.parent().filter(|p| p.is_dir()) else {
            return;
        };
        self.directory = Some(parent.to_owned());
        if let Some(preference) = &self.preference {
            // A convenience preference must never turn a successful open into failure.
            if let (Some(parent), Ok(bytes)) = (preference.parent(), serde_json::to_vec(parent)) {
                let _ = std::fs::create_dir_all(parent)
                    .and_then(|()| std::fs::write(preference, bytes));
            }
        }
    }
}

impl AtelierApp {
    pub(super) fn browse_project_path(&mut self, action: FileAction, path: &mut String) {
        let current = PathBuf::from(path.trim());
        let mut dialog = rfd::FileDialog::new()
            .add_filter(self.language.translate("OpenArchi project"), &["json"]);
        if let Some(parent) = current.parent().filter(|parent| parent.is_dir()) {
            dialog = dialog.set_directory(parent);
        }
        if let Some(name) = current.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        let selected = match action {
            FileAction::Save => dialog.save_file(),
            FileAction::Open => dialog.pick_file(),
            FileAction::New => None,
        };
        if let Some(selected_path) = selected {
            *path = selected_path.display().to_string();
            self.file_error.clear();
        }
    }

    pub(super) fn handle_close_request(&mut self, ctx: &Context) {
        if self.allow_close || !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.recovery_open {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            return;
        }
        if self.block_report_draft_close(ctx) {
            return;
        }
        if self.host.document.editor.dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.close_prompt = true;
            self.close_error.clear();
            self.file_action = None;
        } else {
            self.allow_close = true;
            self.discard_recovery_file();
        }
    }

    pub(super) fn block_report_draft_close(&mut self, ctx: &Context) -> bool {
        if self.pending_parametric_parameters() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.tab = Tab::Assistant;
            self.host.document.editor.status =
                "Confirm or cancel the parameter draft before saving or replacing the project"
                    .into();
            return true;
        }
        if self.project_conditions_has_draft() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.project_conditions_ui.open = true;
            self.host.document.editor.status =
                "Save or discard project condition changes before replacing the project".into();
            return true;
        }
        if self.checks_ui.draft.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.checks_ui.open = true;
            self.host.document.editor.status =
                "Confirm or cancel the check draft before saving or replacing the project".into();
            return true;
        }
        if self.mass_parameter_draft_active() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.host.document.editor.status =
                "Confirm or cancel concept mass parameters before saving or replacing the project"
                    .into();
            return true;
        }
        if self.mass_spaces_ui.dirty {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.tab = Tab::Properties;
            self.host.document.editor.status =
                "Confirm or cancel concept space parameters before saving or replacing the project"
                    .into();
            return true;
        }
        if self.drawings_ui.report_draft.is_none() {
            return false;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        self.drawings_ui.manager = true;
        self.drawings_ui.report_open = true;
        self.drawings_ui.report_error =
            "Confirm or cancel the report template draft before saving or replacing the project"
                .into();
        true
    }

    /// Ordinary Save writes the bound file; only first save or failure needs a panel.
    pub(super) fn request_project_save(&mut self) {
        self.file_error.clear();
        self.file_dialog_path = None;
        if self.path_confirmed {
            match self.save_project() {
                Ok(()) => self.file_action = None,
                Err(error) => {
                    self.file_error = error;
                    self.file_action = Some(FileAction::Save);
                }
            }
        } else {
            self.file_action = Some(FileAction::Save);
        }
    }

    pub(super) fn save_project(&mut self) -> Result<(), String> {
        let path = self.path.clone();
        self.save_project_to(&path)
    }

    pub(super) fn save_project_to(&mut self, path: &str) -> Result<(), String> {
        if self.mass_parameter_draft_active() {
            return Err(
                "Confirm or cancel concept mass parameters before saving or replacing the project"
                    .into(),
            );
        }
        if self.pending_parametric_parameters() {
            return Err(
                "Confirm or cancel the parameter draft before saving or replacing the project"
                    .into(),
            );
        }
        if self.project_conditions_has_draft() {
            return Err(
                "Save or discard project condition changes before replacing the project".into(),
            );
        }
        if self.checks_ui.draft.is_some() {
            return Err(
                "Confirm or cancel the check draft before saving or replacing the project".into(),
            );
        }
        if self.mass_spaces_ui.dirty {
            return Err(
                "Confirm or cancel concept space parameters before saving or replacing the project"
                    .into(),
            );
        }
        if self.drawings_ui.report_draft.is_some() {
            return Err("Confirm or cancel the report template draft before saving or replacing the project".into());
        }
        let path = path.trim();
        if path.is_empty() {
            return Err(self.language.translate("Choose a project file path").into());
        }
        let target = std::path::absolute(path).map_err(|error| error.to_string())?;
        // Same host capture for GUI and the default automation file handler.
        // This path is synchronous today, as was the existing native save; the
        // repository worker uses the identical owned plan off the UI thread.
        self.sync_trace_context();
        crate::trace_document::TraceSettings::capture(&self.trace).validate()?;
        crate::trace_document::TraceViewport {
            center: self.plan_center,
            scale: self.plan_scale,
        }
        .validate()?;
        if !self.persist_trace_state(false) {
            return Err(self.host.document.editor.status.clone());
        }
        let snapshot = self.host.project_snapshot_for_save().collect()?;
        let version = if let Some((bound_path, baseline)) = &self.file_baseline
            && bound_path == &target
        {
            storage::save_document_if_unchanged(
                &snapshot,
                &target,
                baseline,
                self.host.document.editor.project_identity(),
            )
            .map_err(|error| self.language.translate(&error).to_owned())?
        } else {
            storage::save_document(
                &snapshot,
                &target,
                self.host.document.editor.project_identity(),
            )?;
            storage::FileVersion::read(&target)?
        };
        // The successful save carries these receipts. Keep them in the live
        // document too, so a later recovery image cannot revert to an empty archive.
        // No model Undo entry or execution authority is created.
        self.host.document.editor.project.job_archive = snapshot.job_archive;
        self.host
            .document
            .editor
            .bind_file_generation(version.token());
        self.host
            .document
            .editor
            .bind_file_path(Some(target.clone()));
        self.file_baseline = Some((target, version));
        self.path = path.to_owned();
        self.host.document.editor.mark_saved();
        self.path_confirmed = true;
        self.host.document.editor.status = self.language.translate("Project saved").into();
        self.discard_recovery_file();
        self.remember_source_connection(true);
        Ok(())
    }

    pub(super) fn discard_recovery_file(&mut self) {
        // Explicit save/discard/replacement must not let an older worker resurrect a file.
        self.finish_recovery_write(true);
        if let Some(slot) = &self.recovery_slot {
            if let Err(error) = slot.clear() {
                self.recovery_error = error;
                return;
            }
        }
        self.last_recovery_save = None;
        self.last_recovery_revision = None;
    }

    pub(super) fn finish_recovery_write(&mut self, wait: bool) {
        if self
            .recovery_write
            .as_ref()
            .is_none_or(|job| !wait && !job.is_finished())
        {
            return;
        }
        let job = self.recovery_write.take().expect("checked recovery writer");
        let session = job.session;
        let result = job.finish();
        if session != self.host.context().session {
            return;
        }
        match result {
            Ok(()) => self.recovery_error.clear(),
            Err(error) => self.recovery_error = self.language.translate(&error).to_owned(),
        }
    }

    pub(super) fn autosave_recovery(&mut self, ctx: &Context) {
        self.finish_recovery_write(false);
        if self.allow_close || self.recovery_open {
            return;
        }
        if self.recovery_slot.is_none() {
            return;
        }
        if self.recovery_write.is_some() {
            ctx.request_repaint_after(Duration::from_millis(25));
            return;
        }
        if !self.host.document.editor.dirty() {
            if self.last_recovery_save.is_some() {
                self.discard_recovery_file();
            }
            return;
        }
        if ctx.input(|input| {
            input.pointer.any_down()
                || input.is_scrolling()
                || [Key::Plus, Key::Equals, Key::Minus]
                    .iter()
                    .any(|key| input.key_down(*key))
        }) {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let now = Instant::now();
        if !self.recovery_error.is_empty()
            && let Some(last) = self.last_recovery_save
            && now.duration_since(last) < Duration::from_secs(1)
        {
            ctx.request_repaint_after(Duration::from_secs(1) - now.duration_since(last));
            return;
        }
        if !self.host.document.editor.project.trace.is_empty() {
            self.persist_trace_state(false);
        }
        let slot = self
            .recovery_slot
            .as_ref()
            .expect("recovery slot checked above");
        let revision = self.host.document.editor.revision;
        let throttled = self
            .last_recovery_save
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(60));
        if throttled
            && self.last_recovery_revision == Some(revision)
            && self.recovery_error.is_empty()
        {
            return;
        }
        self.last_recovery_save = Some(now);
        self.last_recovery_revision = Some(revision);
        let guard = match slot.writer_guard() {
            Ok(guard) => guard,
            Err(error) => {
                self.recovery_error = error;
                ctx.request_repaint_after(Duration::from_secs(1));
                return;
            }
        };
        let path = slot.snapshot().to_owned();
        let identity = self.host.document.editor.project_identity().to_owned();
        let generation = self
            .host
            .document
            .editor
            .file_generation()
            .map(str::to_owned);
        let snapshot = self.host.project_snapshot_for_save();
        let wake = ctx.clone();
        self.native_metrics.recovery_start();
        let worker = std::thread::Builder::new()
            .name("openarchi-recovery".into())
            .spawn(move || {
                let _owner_guard = guard;
                let result = snapshot.collect().and_then(|snapshot| {
                    storage::save_recovery_document(
                        &snapshot,
                        &path,
                        &identity,
                        generation.as_deref(),
                    )
                });
                wake.request_repaint();
                result
            });
        match worker {
            Ok(worker) => {
                self.recovery_write = Some(RecoveryWrite {
                    session: self.host.context().session,
                    worker: Some(worker),
                })
            }
            Err(error) => self.recovery_error = error.to_string(),
        }
        ctx.request_repaint_after(Duration::from_millis(25));
    }

    pub(super) fn initialize_recovery(&mut self, config: &std::path::Path) {
        self.finish_recovery_write(true);
        let (candidates, mut errors) = crate::recovery::discover(config);
        self.recovery_candidates = candidates;
        self.recovery_choice = (self.recovery_candidates.len() == 1).then_some(0);
        match crate::recovery::RecoverySlot::create(&config.join("recoveries")) {
            Ok(slot) => self.recovery_slot = Some(slot),
            Err(error) => errors.push(error),
        }
        self.recovery_error = errors.join("\n");
        self.recovery_open =
            !self.recovery_candidates.is_empty() || !self.recovery_error.is_empty();
    }

    pub(super) fn resolve_recovery(&mut self, recover: bool) -> Result<(), String> {
        self.finish_recovery_write(true);
        let candidate = self
            .recovery_choice
            .and_then(|i| self.recovery_candidates.get(i))
            .ok_or_else(|| {
                self.language
                    .translate("Select a recovery project")
                    .to_owned()
            })?
            .clone();
        let slot = candidate.claim()?.ok_or_else(|| {
            self.language
                .translate("Recovery project is in use or no longer available")
                .to_owned()
        })?;
        let (project, identity, generation) = slot.read_document()?.ok_or_else(|| {
            self.language
                .translate("Recovery project is in use or no longer available")
                .to_owned()
        })?;
        if recover {
            self.host.document.editor.restore_unsaved(project);
            if let Some(id) = identity {
                self.host.document.editor.adopt_document_identity(id)?;
            }
            self.host.document.editor.bind_file_generation(generation);
            self.file_baseline = None;
            self.path_confirmed = false;
            self.source_document_opened();
            self.reset_worksheet_session();
            self.guide_ui = GuideUi::default();
            self.draft_id = None;
            self.draft = None;
            self.recovery_slot = Some(slot);
            self.recovery_open = false;
            self.recovery_candidates.clear();
            self.recovery_choice = None;
            self.recovery_error.clear();
            self.last_recovery_save = Some(Instant::now());
            self.last_recovery_revision = Some(self.host.document.editor.revision);
            self.fit_opened_project();
        } else {
            slot.clear()?;
            self.recovery_candidates
                .remove(self.recovery_choice.unwrap());
            self.recovery_choice = (self.recovery_candidates.len() == 1).then_some(0);
            self.recovery_open = !self.recovery_candidates.is_empty();
            self.recovery_error.clear();
        }
        Ok(())
    }

    pub(super) fn recovery_dialog(&mut self, ctx: &Context) {
        if !self.recovery_open {
            return;
        }
        let mut action = None;
        let mut close_errors = false;
        Modal::new(Id::new("recovery_dialog")).show(ctx, |ui| {
            ui.set_min_width(500.);
            ui.set_max_width(640.);
            ui.heading(self.language.translate("Recover unsaved project"));
            ui.label(
                self.language
                    .translate("OpenArchi found unsaved changes from a previous session."),
            );
            ScrollArea::vertical().max_height(240.).show(ui, |ui| {
                for (index, candidate) in self.recovery_candidates.iter().enumerate() {
                    ui.selectable_value(&mut self.recovery_choice, Some(index), &candidate.name)
                        .on_hover_text(candidate.path.display().to_string());
                }
            });
            if !self.recovery_error.is_empty() {
                let expected = [
                    self.language.translate("Select a recovery project"),
                    self.language
                        .translate("Recovery project is in use or no longer available"),
                ];
                if expected.contains(&self.recovery_error.as_str()) {
                    ui.colored_label(Color32::from_rgb(177, 60, 40), &self.recovery_error);
                } else {
                    ui.colored_label(
                        Color32::from_rgb(177, 60, 40),
                        self.language.translate(
                            "Recovery could not be completed. Recovery files have been kept.",
                        ),
                    );
                    ui.collapsing(self.language.translate("Technical details"), |ui| {
                        ui.label(&self.recovery_error);
                    });
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.recovery_choice.is_some(),
                        Button::new(self.language.translate("Recover")),
                    )
                    .clicked()
                {
                    action = Some(true);
                }
                if ui
                    .add_enabled(
                        self.recovery_choice.is_some(),
                        Button::new(self.language.translate("Discard")),
                    )
                    .clicked()
                {
                    action = Some(false);
                }
                if ui.button(self.language.translate("Close")).clicked() {
                    close_errors = true;
                }
            });
        });
        if let Some(recover) = action {
            if let Err(error) = self.resolve_recovery(recover) {
                self.recovery_error = error;
            }
        }
        if close_errors {
            self.recovery_open = false;
        }
    }

    pub(super) fn open_project_directory(&self) -> PathBuf {
        self.project_open
            .directory
            .clone()
            .filter(|p| p.is_dir())
            .or_else(|| {
                self.path_confirmed
                    .then(|| PathBuf::from(&self.path))
                    .and_then(|p| p.parent().map(std::path::Path::to_owned))
                    .filter(|p| p.is_dir())
            })
            .or_else(crate::platform::documents_dir)
            .unwrap_or_else(std::env::temp_dir)
    }

    /// The test seam substitutes only the OS picker result, never a different UI flow.
    fn project_file_picker(&mut self, save: bool) -> Option<Option<PathBuf>> {
        #[cfg(test)]
        {
            let _ = save;
            self.project_open.choices.pop_front()
        }
        #[cfg(not(test))]
        {
            let mut dialog = rfd::FileDialog::new()
                .set_title(self.language.translate(if save {
                    "Save project"
                } else {
                    "Open project"
                }))
                .add_filter(self.language.translate("OpenArchi project"), &["json"])
                .set_directory(self.open_project_directory());
            if save {
                let name = std::path::Path::new(&self.path)
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("Untitled.openarchi.json"));
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
            Some(if save {
                dialog.save_file()
            } else {
                dialog.pick_file()
            })
        }
    }

    fn project_replacement_guard(&self) -> Result<(), String> {
        let reason = if self.mass_parameter_draft_active() {
            Some("Confirm or cancel concept mass parameters before saving or replacing the project")
        } else if self.pending_parametric_parameters() {
            Some("Confirm or cancel the parameter draft before saving or replacing the project")
        } else if self.project_conditions_has_draft() {
            Some("Save or discard project condition changes before replacing the project")
        } else if self.checks_ui.draft.is_some() {
            Some("Confirm or cancel the check draft before saving or replacing the project")
        } else if self.mass_spaces_ui.dirty {
            Some(
                "Confirm or cancel concept space parameters before saving or replacing the project",
            )
        } else if self.drawings_ui.report_draft.is_some() {
            Some(
                "Confirm or cancel the report template draft before saving or replacing the project",
            )
        } else {
            None
        };
        reason.map_or(Ok(()), |reason| Err(reason.to_owned()))
    }

    fn read_project_for_open(
        path: &std::path::Path,
    ) -> Result<(Editor, storage::FileVersion, PathBuf), String> {
        let target = std::path::absolute(path).map_err(|e| e.to_string())?;
        let (project, version, identity) = storage::load_document(&target)?;
        let mut editor = Editor::new(project);
        if let Some(identity) = identity {
            editor.adopt_document_identity(identity)?;
        }
        editor.bind_file_generation(version.token());
        editor.bind_file_path(Some(target.clone()));
        Ok((editor, version, target))
    }

    fn finish_project_open(&mut self) -> Result<(), String> {
        self.project_replacement_guard()?;
        let path = self
            .project_open
            .selected
            .as_ref()
            .ok_or("Choose a project file path")?;
        // Validate everything before changing the current editor, path, or baseline.
        let (editor, version, target) = Self::read_project_for_open(path)?;
        self.host.document.editor = editor;
        self.path = target.display().to_string();
        self.file_baseline = Some((target.clone(), version));
        self.navigation.reset_for_document();
        self.cancel_mass_push_pull();
        self.reset_worksheet_session();
        self.guide_ui = GuideUi::default();
        self.draft_id = None;
        self.draft = None;
        self.discard_recovery_file();
        self.path_confirmed = true;
        self.source_document_opened();
        self.fit_opened_project();
        self.project_open.remember(&target);
        self.cancel_project_open();
        Ok(())
    }

    pub(super) fn cancel_open_with_escape(&mut self, ctx: &Context) -> bool {
        if self.file_action == Some(FileAction::Open)
            && ctx
                .input_mut(|input| input.focused && input.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.cancel_project_open();
            return true;
        }
        false
    }

    fn cancel_project_open(&mut self) {
        self.file_action = None;
        self.file_dialog_path = None;
        self.project_open.selected = None;
        self.project_open.confirm_unsaved = false;
        self.file_error.clear();
    }

    fn accept_project_open_selection(&mut self, selection: Option<PathBuf>) {
        let Some(path) = selection else {
            self.cancel_project_open();
            return;
        };
        self.file_error.clear();
        self.project_open.selected = Some(path.clone());
        self.project_open.confirm_unsaved = false;
        let result = self.project_replacement_guard().and_then(|()| {
            if self.host.document.editor.dirty() {
                // Reject invalid targets without touching the current document or asking to save.
                Self::read_project_for_open(&path)?;
                self.project_open.confirm_unsaved = true;
                Ok(())
            } else {
                self.finish_project_open()
            }
        });
        if let Err(error) = result {
            self.file_error = error;
        }
    }

    fn open_project_window(&mut self, ctx: &Context) {
        if self.project_open.selected.is_none() && self.file_error.is_empty() {
            // An explicit project-source action already chose its target; all ordinary
            // Open entries leave this empty and go straight to the native picker.
            let selection = self
                .file_dialog_path
                .take()
                .map(|p| Some(PathBuf::from(p)))
                .or_else(|| self.project_file_picker(false));
            if let Some(selection) = selection {
                self.accept_project_open_selection(selection);
            }
            if self.file_action != Some(FileAction::Open)
                || (self.project_open.selected.is_none() && self.file_error.is_empty())
            {
                return;
            }
        }
        let mut choice = None;
        let unsaved = self.project_open.confirm_unsaved;
        Modal::new(Id::new("project_open_confirmation")).show(ctx, |ui| {
            ui.set_min_width(440.);
            ui.heading(self.language.translate(if unsaved {
                "Unsaved changes"
            } else {
                "Could not open project"
            }));
            if unsaved {
                ui.label(
                    self.language.translate(
                        "Save changes to the current project before opening another file?",
                    ),
                );
            }
            if let Some(path) = &self.project_open.selected {
                ui.label(path.display().to_string());
            }
            if !self.file_error.is_empty() {
                ui.colored_label(
                    Color32::from_rgb(177, 60, 40),
                    localized_error(self.language, &self.file_error),
                );
            }
            ui.horizontal(|ui| {
                if unsaved {
                    if ui.button(self.language.translate("Save")).clicked() {
                        choice = Some(0);
                    }
                    if ui.button(self.language.translate("Don't Save")).clicked() {
                        choice = Some(1);
                    }
                } else if ui
                    .button(self.language.translate("Choose another file…"))
                    .clicked()
                {
                    choice = Some(3);
                }
                if ui.button(self.language.translate("Cancel")).clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                let result = if self.path_confirmed {
                    self.save_project().map(|()| true)
                } else if let Some(selection) = self.project_file_picker(true) {
                    selection
                        .map(|path| self.save_project_to(&path.to_string_lossy()).map(|()| true))
                        .unwrap_or(Ok(false))
                } else {
                    Ok(false)
                };
                match result {
                    Ok(true) => {
                        if let Err(error) = self.finish_project_open() {
                            self.file_error = error;
                        }
                    }
                    Ok(false) => {}
                    Err(error) => self.file_error = error,
                }
            }
            Some(1) => {
                if let Err(error) = self.finish_project_open() {
                    self.file_error = error;
                }
            }
            Some(2) => self.cancel_project_open(),
            Some(3) => {
                self.project_open.selected = None;
                self.file_error.clear();
                ctx.request_repaint();
            }
            _ => {}
        }
    }

    pub(super) fn close_window(&mut self, ctx: &Context) {
        if !self.close_prompt {
            return;
        }
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        Modal::new(Id::new("unsaved_changes_dialog")).show(ctx, |ui| {
            ui.set_min_width(480.);
            ui.heading(self.language.translate("Unsaved changes"));
            ui.label(
                self.language
                    .translate("Save your changes before closing OpenArchi?"),
            );
            if !self.close_error.is_empty() {
                ui.colored_label(
                    Color32::from_rgb(177, 60, 40),
                    localized_error(self.language, &self.close_error),
                );
            }
            ui.horizontal(|ui| {
                if ui
                    .button(self.language.translate("Save and close"))
                    .clicked()
                {
                    save = true;
                }
                if ui
                    .button(self.language.translate("Discard and close"))
                    .clicked()
                {
                    discard = true;
                }
                if ui.button(self.language.translate("Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if save {
            match self.save_project() {
                Ok(()) => {
                    self.close_prompt = false;
                    self.close_error.clear();
                    self.allow_close = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                Err(error) => self.close_error = error,
            }
        } else if discard {
            self.discard_recovery_file();
            self.close_prompt = false;
            self.close_error.clear();
            self.allow_close = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if cancel {
            self.close_prompt = false;
            self.close_error.clear();
        }
    }

    pub(super) fn file_window(&mut self, ctx: &Context) {
        if self.file_action == Some(FileAction::Open) {
            self.open_project_window(ctx);
            return;
        }
        self.project_open.selected = None;
        self.project_open.confirm_unsaved = false;
        if let Some(action) = self.file_action {
            let mut path = self
                .file_dialog_path
                .take()
                .unwrap_or_else(|| self.path.clone());
            let title = match action {
                FileAction::Save => self.language.translate("Save project"),
                FileAction::Open => self.language.translate("Open project"),
                FileAction::New => self.language.translate("New project"),
            };
            let mut close = false;
            Modal::new(Id::new("project_file_dialog")).show(ctx, |ui| {
                ui.set_min_width(500.);
                ui.heading(title);
                if action != FileAction::Save && self.host.document.editor.dirty() {
                    ui.colored_label(
                        Color32::from_rgb(166, 101, 45),
                        self.language.translate(
                            "Unsaved changes will be replaced. Save first if you need them.",
                        ),
                    );
                }
                if action != FileAction::New {
                    ui.label(
                        self.language
                            .translate("Project file · {format}")
                            .replace("{format}", crate::storage::JsonAdapter.format_name()),
                    );
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut path).desired_width(420.));
                        if ui.button(self.language.translate("Browse…")).clicked() {
                            self.browse_project_path(action, &mut path);
                        }
                    });
                } else {
                    ui.label(
                        self.language
                            .translate("Start an empty project with three stories."),
                    );
                }
                if !self.file_error.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(177, 60, 40),
                        localized_error(self.language, &self.file_error),
                    );
                }
                ui.horizontal(|ui| {
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                    if ui
                        .button(match action {
                            FileAction::Save => self.language.translate("Save"),
                            FileAction::Open => unreachable!("Open uses the native picker"),
                            FileAction::New => self.language.translate("Create / replace"),
                        })
                        .clicked()
                    {
                        let result = if self.pending_parametric_parameters() {
                            Err("Confirm or cancel the parameter draft before saving or replacing the project".into())
                        } else if self.project_conditions_has_draft(){
                            Err("Save or discard project condition changes before replacing the project".into())
                        } else if self.checks_ui.draft.is_some() {
                            Err("Confirm or cancel the check draft before saving or replacing the project".into())
                        } else if self.mass_spaces_ui.dirty {
                            Err("Confirm or cancel concept space parameters before saving or replacing the project".into())
                        } else if self.drawings_ui.report_draft.is_some() {
                            Err("Confirm or cancel the report template draft before saving or replacing the project".into())
                        } else { match action {
                            FileAction::Save => self.save_project_to(&path),
                            FileAction::Open => unreachable!("Open uses the native picker"),
                            FileAction::New => {
                                self.navigation = Default::default();
                                self.host.document.editor = Editor::new(Project {
                                    name: "Untitled project".into(),
                                    ..Project::default()
                                });
                                self.reset_worksheet_session();
                                self.guide_ui = GuideUi::default();
                                self.draft_id = None;
                                self.draft = None;
                                self.discard_recovery_file();
                                self.path_confirmed = false;
                                self.file_baseline = None;
                                self.source_document_opened();
                                self.fit_opened_project();
                                Ok(())
                            }
                        }};
                        match result {
                            Ok(()) => {
                                close = true;
                                self.file_error.clear()
                            }
                            Err(e) => self.file_error = e,
                        }
                    }
                });
            });
            if close {
                self.file_action = None;
                self.file_error.clear();
            } else {
                self.file_dialog_path = Some(path);
            }
        } else {
            self.file_dialog_path = None;
        }
    }
}
impl AtelierApp {}

#[cfg(unix)]
impl AtelierApp {
    pub(super) fn finish_automation_document_recovery(&mut self) {
        self.finish_recovery_write(true);
        self.file_baseline = None;
        self.source_document_opened();
        self.path.clear();
        self.path_confirmed = false;
        self.file_dialog_path = None;
        self.recovery_open = false;
        self.recovery_candidates.clear();
        self.recovery_choice = None;
        self.recovery_error.clear();
        self.last_recovery_save = None;
        self.last_recovery_revision = None;
        self.reset_worksheet_session();
        self.guide_ui = GuideUi::default();
        self.draft_id = None;
        self.draft = None;
        self.fit_opened_project();
    }
    pub(crate) fn automation_file(
        &mut self,
        p: crate::automation_ipc::FileParams,
        reopen: bool,
    ) -> serde_json::Value {
        use serde_json::json;
        let error = |code: &str| json!({"error":{"code":code}});
        if let Some(scope) = self.mcp_file_scope() {
            return match scope {
                Ok(root) => self.automation_file_in_scope(p, reopen, &root),
                Err(code) => error(code),
            };
        }
        if std::env::var("OPENARCHI_AUTOMATION_WRITE").as_deref() != Ok("1") {
            return error("WRITE_NOT_AUTHORIZED");
        }
        let Some(root) = std::env::var_os("OPENARCHI_AUTOMATION_PROJECT_DIR")
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
        else {
            return error("FILE_SCOPE_NOT_CONFIGURED");
        };
        self.automation_file_in_scope(p, reopen, &root)
    }
    pub(super) fn automation_file_in_scope(
        &mut self,
        p: crate::automation_ipc::FileParams,
        reopen: bool,
        root: &std::path::Path,
    ) -> serde_json::Value {
        use serde_json::json;
        let error = |code: &str| json!({"error":{"code":code}});
        // The endpoint checks random instance identity; this handler also binds the live document.
        if p.target.document_session_id != self.host.document.editor.document_session()
            || p.target.project_id != self.host.document.editor.project_identity()
        {
            return error("TARGET_MISMATCH");
        }
        if p.expected_revision != self.host.document.editor.revision {
            return error("REVISION_CONFLICT");
        }
        if p.filename.is_empty()
            || p.filename.len() > 128
            || p.filename.starts_with('.')
            || !p
                .filename
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return error("INVALID_FILENAME");
        }
        let path = root.join(&p.filename);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return error("FILE_SCOPE_VIOLATION");
        }
        let bound = if self.path_confirmed {
            std::path::absolute(&self.path).ok().filter(|path| {
                self.file_baseline
                    .as_ref()
                    .is_some_and(|(bound, _)| bound == path)
            })
        } else {
            None
        };
        if reopen {
            if self.host.document.editor.dirty() {
                return error("UNSAVED_CHANGES");
            }
            if bound.as_ref() != Some(&path) {
                return error("FILE_SCOPE_VIOLATION");
            }
            match storage::load_document(&path) {
                Err(message) => json!({"error":{"code":"FILE_INVALID","message":message}}),
                Ok((project, version, identity)) => {
                    self.navigation = Default::default();
                    self.host.document.editor = Editor::new(project);
                    if let Some(id) = identity {
                        if let Err(message) = self.host.document.editor.adopt_document_identity(id)
                        {
                            return json!({"error":{"code":"DOCUMENT_IDENTITY_INVALID","message":message}});
                        }
                    }
                    self.host
                        .document
                        .editor
                        .bind_file_generation(version.token());
                    self.host.document.editor.bind_file_path(Some(path.clone()));
                    self.file_baseline = Some((path, version));
                    self.source_document_opened();
                    self.reset_worksheet_session();
                    self.guide_ui = GuideUi::default();
                    self.draft_id = None;
                    self.draft = None;
                    self.discard_recovery_file();
                    self.path_confirmed = true;
                    self.fit_opened_project();
                    json!({"result":{"status":"reopened","document_session_id":self.host.document.editor.document_session(),"revision":self.host.document.editor.revision}})
                }
            }
        } else {
            if path.exists() && bound.as_ref() != Some(&path) {
                return error("FILE_EXISTS");
            }
            match self.save_project_to(&path.to_string_lossy()) {
                Ok(()) => {
                    json!({"result":{"status":"saved","revision":self.host.document.editor.revision}})
                }
                Err(message) => json!({"error":{"code":"FILE_SAVE_FAILED","message":message}}),
            }
        }
    }
}
