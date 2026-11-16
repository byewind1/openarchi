//! Application-owned document and the shared model command boundary.
//! The public editor is the temporary LegacyAdapter for existing UI/IPC paths.
use crate::{command::ModelCommand, editor::Editor, model::Project, model_service::ChangeReceipt};

pub struct DocumentSession {
    pub editor: Editor,
}

pub struct ApplicationHost {
    pub document: DocumentSession,
    pub plugins: crate::plugins::Registry,
    pub jobs: Option<crate::plugins::jobs::Jobs>,
    pub(crate) environment: Option<crate::workbench::environment::EnvironmentService>,
    pub(crate) operations: std::rc::Rc<std::cell::RefCell<crate::automation_change::ChangeService>>,
}

/// An owned snapshot for the existing save/checkpoint worker. No live Editor,
/// task authority, registry or provider handle crosses this boundary.
pub struct ProjectSaveSnapshot {
    project: Project,
    project_id: String,
    job_root: Option<std::path::PathBuf>,
}
impl ProjectSaveSnapshot {
    pub fn collect(mut self) -> Result<Project, String> {
        self.project.job_archive.check_project(&self.project_id)?;
        if let Some(root) = self.job_root {
            self.project.job_archive = crate::plugins::jobs::Jobs::new(root)
                .capture_archive(&self.project_id, &self.project.job_archive)?;
        }
        Ok(self.project)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandContext {
    pub session: u64,
    pub revision: u64,
}

pub enum HostCommand {
    Undo,
    Redo,
    Apply {
        request_id: String,
        commands: Vec<ModelCommand>,
    },
}

impl ApplicationHost {
    /// Document presentation settings share the host boundary, but never enter
    /// geometry Undo/Redo. The document revision guards outstanding operations.
    pub fn set_trace_window(
        &mut self,
        context: crate::trace_document::TraceContext,
        mut settings: crate::trace_document::TraceSettings,
        remember: bool,
    ) -> Result<(), String> {
        let mut document = self.document.editor.project.trace.clone();
        let reference = settings.reference;
        if settings.source_identity.is_none() {
            settings.source_identity = crate::trace_document::TraceSourceIdentity::for_reference(
                &self.document.editor.project,
                reference,
            );
        }
        document.set(
            context.view,
            settings,
            context.story,
            context.previous_story,
        )?;
        document.plan_visible = context.plan_visible;
        document.viewport = Some(context.viewport);
        if remember {
            document.remember(reference);
        }
        document.validate()?;
        if self.document.editor.project.trace != document {
            self.document.editor.project.trace = document;
            self.document.editor.revision = self.document.editor.revision.wrapping_add(1);
        }
        Ok(())
    }

    pub fn turn_off_all_trace(&mut self) {
        let changed = self
            .document
            .editor
            .project
            .trace
            .windows
            .iter()
            .any(|w| w.settings.enabled);
        for window in &mut self.document.editor.project.trace.windows {
            window.settings.enabled = false;
        }
        if changed {
            self.document.editor.revision = self.document.editor.revision.wrapping_add(1);
        }
    }

    pub fn apply_trace_visibility_to_all(&mut self, visibility: crate::trace::TraceVisibility) {
        let changed = self
            .document
            .editor
            .project
            .trace
            .windows
            .iter()
            .any(|w| w.settings.visibility != visibility);
        for window in &mut self.document.editor.project.trace.windows {
            window.settings.visibility = visibility;
        }
        if changed {
            self.document.editor.revision = self.document.editor.revision.wrapping_add(1);
        }
    }

    /// Reload only the accepted immutable source blobs. Updating a linked DWG
    /// remains the existing reviewed source-update transaction, never a silent
    /// replacement of project geometry or alignment.
    pub fn rebuild_trace_sources(
        &mut self,
        reference: crate::trace::TraceReference,
    ) -> Result<usize, String> {
        let editor = &mut self.document.editor;
        let selection = reference
            .resolve(&editor.project, editor.story, editor.previous_story)
            .ok_or("Saved Trace Reference is unavailable")?;
        let ids = editor
            .project
            .drawings
            .cad_underlays
            .iter()
            .filter(|u| match selection {
                crate::trace::TraceSelection::Worksheet(id) => u.worksheet == id,
                _ => u.visible_in_plan(None),
            })
            .map(|u| u.source)
            .collect::<std::collections::BTreeSet<_>>();
        let mut failures = Vec::new();
        for source in &mut editor.project.drawings.cad_sources {
            if ids.contains(&source.id)
                && let Err(error) = source.load()
            {
                failures.push(error);
            }
        }
        if failures.is_empty() {
            Ok(ids.len())
        } else {
            Err(format!(
                "Trace source unavailable · {}",
                failures.join("; ")
            ))
        }
    }

    /// Shared coordinate-reference edit: the existing typed option transaction
    /// persists the reference without translating elements or changing the grid.
    pub fn set_user_origin(
        &mut self,
        context: CommandContext,
        origin: Option<crate::model::Point>,
    ) -> Result<Option<ChangeReceipt>, String> {
        let mut plan = self.document.editor.project.plan_options.clone();
        plan.user_origin = origin;
        plan.validate()?;
        self.dispatch(
            context,
            HostCommand::Apply {
                request_id: format!("input-origin-{}-{}", context.session, context.revision),
                commands: vec![ModelCommand::SetOptions {
                    plan: Some(plan),
                    design: None,
                    guide: None,
                    grid: None,
                }],
            },
        )
    }
    /// Presentation mode never grants write authority or changes the document.
    pub fn pause_remote_writes(&self, paused: bool) {
        self.operations.borrow_mut().set_remote_paused(paused);
    }

    /// Creation preferences do not change the model, task authority or Undo.
    /// Future manual creation still uses the existing typed model commands.
    pub fn creation_defaults(&self) -> (f64, f64, String) {
        let e = &self.document.editor;
        (e.concept_mass_height, e.zone_height, e.zone_purpose.clone())
    }
    pub fn apply_creation_presets(
        &mut self,
        presets: &crate::workbench::creation_presets::CreationPresets,
    ) -> Result<(), String> {
        presets.validate()?;
        if self.document.editor.model_input_active() {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        let e = &mut self.document.editor;
        e.concept_mass_height = f64::from(presets.mass_height_mm) / 1000.;
        e.zone_height = f64::from(presets.zone_height_mm) / 1000.;
        e.zone_purpose = presets.zone_purpose.clone();
        Ok(())
    }
    /// Trusted native/library consent after original task confirmation. Never an RPC grant.
    pub fn authorize_object_task(
        &self,
        context: CommandContext,
        request_id: &str,
    ) -> serde_json::Value {
        if context != self.context() {
            return serde_json::json!({"error":{"code":"CONTEXT_CONFLICT"}});
        }
        self.operations
            .borrow_mut()
            .authorize_object_task(&self.document.editor, request_id)
    }
    pub fn object_task_authorization(&self) -> serde_json::Value {
        self.operations
            .borrow()
            .object_task_authorization(&self.document.editor)
    }
    pub fn configure_environment(
        &mut self,
        directory: &std::path::Path,
        mode: crate::workbench::WorkMode,
    ) -> Result<(), String> {
        if self.environment.is_some() {
            return Err("Work environment host is already configured".into());
        }
        self.environment = Some(crate::workbench::environment::EnvironmentService::open(
            &directory.join("work-environment.json"),
            mode,
        )?);
        Ok(())
    }
    pub fn environment(&self) -> Option<&crate::workbench::environment::EnvironmentService> {
        self.environment.as_ref()
    }
    pub fn project_snapshot_for_save(&self) -> ProjectSaveSnapshot {
        ProjectSaveSnapshot {
            project: self.document.editor.project.clone(),
            project_id: self.document.editor.project_identity().to_owned(),
            job_root: self.jobs.as_ref().map(|jobs| jobs.read_root()),
        }
    }
    pub fn configure_plugins(&mut self, directory: &std::path::Path) -> Result<(), String> {
        // Do not replace workers or storage while jobs may still be running.
        if self.jobs.is_some() {
            return Err("Plugin host is already configured".into());
        }
        self.jobs = Some(crate::plugins::jobs::Jobs::new(
            directory.join("plugin-jobs"),
        ));
        self.plugins = crate::plugins::Registry::open(&directory.join("plugins.json"))?;
        Ok(())
    }
    pub fn start_plugin_job(
        &mut self,
        command: &str,
        request_id: &str,
    ) -> Result<crate::plugins::jobs::Identity, String> {
        self.jobs
            .as_mut()
            .ok_or("Plugin job storage unavailable")?
            .start(&self.plugins, &self.document.editor, command, request_id)
    }
    pub fn start_solar_job(
        &mut self,
        parameters: &crate::plugins::solar::Parameters,
        request_id: &str,
    ) -> Result<crate::plugins::jobs::Identity, String> {
        self.jobs
            .as_mut()
            .ok_or("Plugin job storage unavailable")?
            .start_with_solar(
                &self.plugins,
                &self.document.editor,
                crate::plugins::solar::COMMAND,
                request_id,
                Some(parameters),
            )
    }
    /// Only immutable paths/identity cross to the evidence worker, never Editor.
    pub fn import_plugin_job(
        &self,
        source: std::path::PathBuf,
    ) -> Result<std::thread::JoinHandle<Result<crate::plugins::jobs::Identity, String>>, String>
    {
        let root = self
            .jobs
            .as_ref()
            .ok_or("Plugin job storage unavailable")?
            .read_root();
        let project = self.document.editor.project_identity().to_owned();
        std::thread::Builder::new()
            .name("openarchi-job-evidence-import".into())
            .spawn(move || crate::plugins::jobs::Jobs::new(root).import_record(&project, &source))
            .map_err(|e| e.to_string())
    }
    pub fn set_plugin_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        self.plugins.set_enabled(id, enabled)?;
        if !enabled {
            if let Some(jobs) = &mut self.jobs {
                jobs.cancel_plugin(id)?;
            }
        }
        Ok(())
    }
    pub(crate) fn set_remote_interaction_blocked(&self, blocked: bool) {
        self.operations
            .borrow_mut()
            .set_remote_interaction_blocked(blocked);
    }
    pub fn new(project: Project) -> Self {
        Self {
            plugins: Default::default(),
            jobs: None,
            environment: None,
            operations: std::rc::Rc::new(std::cell::RefCell::new(
                crate::automation_change::ChangeService::new(
                    crate::storage::new_document_identity(),
                    false,
                ),
            )),
            document: DocumentSession {
                editor: Editor::new(project),
            },
        }
    }
    /// Configure before opening a journal; an active operation store cannot
    /// be silently replaced when a transport starts or preferences change.
    pub fn configure_operation_journal(
        &mut self,
        directory: Option<std::path::PathBuf>,
    ) -> Result<(), String> {
        self.operations
            .borrow_mut()
            .configure_journal_directory(directory)
    }
    pub fn operation_target(&self) -> crate::automation::Target {
        self.operations
            .borrow()
            .current_target(&self.document.editor)
    }
    /// Explicit native task, figure-only or single typed mass-space confirmation; never a remote write grant.
    pub fn confirm_task_prepared(
        &mut self,
        context: CommandContext,
        request_id: &str,
        label: &str,
        prepared: std::rc::Rc<crate::model_service::PreparedChange>,
    ) -> serde_json::Value {
        if context.session != self.context().session || context.revision != prepared.base_revision()
        {
            return serde_json::json!({"error":{"code":"DOCUMENT_CONFLICT"}});
        }
        self.operations.borrow_mut().confirm_native_task(
            &mut self.document.editor,
            request_id,
            label,
            prepared,
        )
    }
    pub fn task_operations(&self) -> serde_json::Value {
        self.operations
            .borrow_mut()
            .task_checkpoint(&self.document.editor)
    }
    pub fn inspect_operation(
        &self,
        original_target: crate::automation::Target,
        request_id: &str,
    ) -> serde_json::Value {
        self.operations.borrow_mut().inspect_current(
            &self.document.editor,
            request_id.into(),
            original_target,
        )
    }
    pub fn context(&self) -> CommandContext {
        CommandContext {
            session: self.document.editor.document_session(),
            revision: self.document.editor.revision,
        }
    }
    /// Shared native/agent entry for original-bound edits; the opaque admission
    /// retains the repository lock until the typed transaction has committed.
    pub fn apply_rule_edit(
        &mut self,
        context: CommandContext,
        request_id: &str,
        edit: crate::rules::RuleEdit,
        admission: crate::project_repository::rules::RuleCommitGuard,
    ) -> Result<ChangeReceipt, String> {
        if context != self.context() {
            return Err("Model revision or document session changed".into());
        }
        let prepared = self
            .document
            .editor
            .prepare_rule_change(context.revision, edit, &admission)
            .map_err(|e| e.message)?;
        self.document
            .editor
            .commit_change(request_id, &prepared)
            .map_err(|e| e.message)
    }
    /// Commit exactly the immutable proposal reviewed by a native/agent caller.
    pub fn commit_prepared(
        &mut self,
        context: CommandContext,
        request_id: &str,
        prepared: &crate::model_service::PreparedChange,
    ) -> Result<ChangeReceipt, String> {
        if context.session != self.context().session || context.revision != prepared.base_revision()
        {
            return Err("Document session or proposal context changed".into());
        }
        self.document
            .editor
            .commit_change(request_id, prepared)
            .map_err(|e| e.message)
    }
    pub fn dispatch(
        &mut self,
        context: CommandContext,
        command: HostCommand,
    ) -> Result<Option<ChangeReceipt>, String> {
        if context.session != self.context().session {
            return Err("Document session changed".into());
        }
        if context.revision != self.context().revision {
            return Err("Model revision changed".into());
        }
        match command {
            HostCommand::Undo => {
                if self.document.editor.undo.is_empty() {
                    return Err("Nothing to undo".into());
                }
                self.document.editor.undo();
                Ok(None)
            }
            HostCommand::Redo => {
                if self.document.editor.redo.is_empty() {
                    return Err("Nothing to redo".into());
                }
                self.document.editor.redo();
                Ok(None)
            }
            HostCommand::Apply {
                request_id,
                commands,
            } => {
                let candidate = self
                    .document
                    .editor
                    .prepare_change(context.revision, commands)
                    .map_err(|e| e.message)?;
                self.document
                    .editor
                    .commit_change(&request_id, &candidate)
                    .map(Some)
                    .map_err(|e| e.message)
            }
        }
    }
}
