use super::*;

impl AtelierApp {
    pub(super) fn browse_project_path(&mut self, action: FileAction) {
        let current = PathBuf::from(self.path.trim());
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
        if let Some(path) = selected {
            self.path = path.display().to_string();
            self.file_error.clear();
        }
    }

    pub(super) fn handle_close_request(&mut self, ctx: &Context) {
        if self.allow_close || !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.recovery_prompt.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            return;
        }
        if self.editor.dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.close_prompt = true;
            self.close_error.clear();
            self.file_action = None;
        } else {
            self.allow_close = true;
            self.discard_recovery_file();
        }
    }

    pub(super) fn save_project(&mut self) -> Result<(), String> {
        let path = self.path.trim();
        if path.is_empty() {
            return Err(self.language.translate("Choose a project file path").into());
        }
        storage::save(&self.editor.project, &PathBuf::from(path))?;
        self.editor.mark_saved();
        self.editor.status = self.language.translate("Project saved").into();
        self.discard_recovery_file();
        Ok(())
    }

    pub(super) fn discard_recovery_file(&mut self) {
        crate::recovery::clear(self.recovery_path.as_deref());
        self.last_recovery_save = None;
        self.last_recovery_revision = None;
    }

    pub(super) fn autosave_recovery(&mut self, ctx: &Context) {
        if self.allow_close || self.recovery_prompt.is_some() {
            return;
        }
        let Some(path) = self.recovery_path.clone() else {
            return;
        };
        if !self.editor.dirty() {
            if self.last_recovery_save.is_some() {
                self.discard_recovery_file();
            }
            return;
        }
        let now = Instant::now();
        let revision = self.editor.revision;
        let throttled = self
            .last_recovery_save
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(60));
        if throttled && self.last_recovery_revision == Some(revision) {
            return;
        }
        self.last_recovery_save = Some(now);
        self.last_recovery_revision = Some(revision);
        if let Err(error) = crate::recovery::write(&path, &self.editor.project) {
            self.recovery_error = error;
        } else {
            self.recovery_error.clear();
        }
        ctx.request_repaint_after(Duration::from_secs(60));
    }

    pub(super) fn recovery_dialog(&mut self, ctx: &Context) {
        let Some(project) = self.recovery_prompt.clone() else {
            return;
        };
        let mut recover = false;
        let mut discard = false;
        Modal::new(Id::new("recovery_dialog")).show(ctx, |ui| {
            ui.set_min_width(500.);
            ui.heading(self.language.translate("Recover unsaved project"));
            ui.label(
                self.language
                    .translate("OpenArchi found unsaved changes from a previous session."),
            );
            if !self.recovery_error.is_empty() {
                ui.colored_label(Color32::from_rgb(177, 60, 40), &self.recovery_error);
            }
            ui.horizontal(|ui| {
                if ui.button(self.language.translate("Recover")).clicked() {
                    recover = true;
                }
                if ui.button(self.language.translate("Discard")).clicked() {
                    discard = true;
                }
            });
        });
        if recover {
            self.editor.restore_unsaved(project);
            self.guide_ui = GuideUi::default();
            self.draft_id = None;
            self.draft = None;
            self.recovery_prompt = None;
            self.recovery_error.clear();
            self.last_recovery_save = Some(Instant::now());
            self.last_recovery_revision = Some(self.editor.revision);
            self.fit();
        } else if discard {
            self.recovery_prompt = None;
            self.recovery_error.clear();
            self.discard_recovery_file();
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
                ui.colored_label(Color32::from_rgb(177, 60, 40), &self.close_error);
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
        if let Some(action) = self.file_action {
            let title = match action {
                FileAction::Save => self.language.translate("Save project"),
                FileAction::Open => self.language.translate("Open project"),
                FileAction::New => self.language.translate("New project"),
            };
            let mut close = false;
            Modal::new(Id::new("project_file_dialog")).show(ctx, |ui| {
                ui.set_min_width(500.);
                ui.heading(title);
                if action != FileAction::Save && self.editor.dirty() {
                    ui.colored_label(
                        Color32::from_rgb(166, 101, 45),
                        self.language.translate(
                            "Unsaved changes will be replaced. Save first if you need them.",
                        ),
                    );
                }
                if action != FileAction::New {
                    ui.label(self.language.translate("Project file · OpenArchi JSON v1"));
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.path).desired_width(420.));
                        if ui.button(self.language.translate("Browse…")).clicked() {
                            self.browse_project_path(action);
                        }
                    });
                } else {
                    ui.label(
                        self.language
                            .translate("Start an empty project with three stories."),
                    );
                }
                if !self.file_error.is_empty() {
                    ui.colored_label(Color32::from_rgb(177, 60, 40), &self.file_error);
                }
                ui.horizontal(|ui| {
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                    if ui
                        .button(match action {
                            FileAction::Save => self.language.translate("Save"),
                            FileAction::Open => self.language.translate("Open / replace"),
                            FileAction::New => self.language.translate("Create / replace"),
                        })
                        .clicked()
                    {
                        let result = match action {
                            FileAction::Save => self.save_project(),
                            FileAction::Open => storage::load(&PathBuf::from(self.path.trim()))
                                .map(|p| {
                                    self.editor = Editor::new(p);
                                    self.guide_ui = GuideUi::default();
                                    self.draft_id = None;
                                    self.draft = None;
                                    self.discard_recovery_file();
                                    self.fit();
                                }),
                            FileAction::New => {
                                self.editor = Editor::new(Project {
                                    name: "Untitled project".into(),
                                    ..Project::default()
                                });
                                self.guide_ui = GuideUi::default();
                                self.draft_id = None;
                                self.draft = None;
                                self.discard_recovery_file();
                                self.fit();
                                Ok(())
                            }
                        };
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
            }
        }
    }
}
impl AtelierApp {}
