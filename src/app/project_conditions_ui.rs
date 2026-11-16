use super::*;
use crate::{
    application_host::{CommandContext, HostCommand},
    command::ModelCommand,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ConditionsGroup {
    #[default]
    Sources,
    Site,
    Requirements,
}
impl ConditionsGroup {
    fn label(self) -> &'static str {
        match self {
            Self::Sources => "Documents and evidence",
            Self::Site => "Site and orientation",
            Self::Requirements => "Requirements and constraints",
        }
    }
}
pub(super) struct ConditionsPreview {
    pub base: CommandContext,
    pub request_id: String,
    pub prepared: std::rc::Rc<crate::model_service::PreparedChange>,
    review: Vec<ConditionRow>,
}
pub(super) struct ConditionsJob {
    pub base: CommandContext,
    pub request_id: String,
    pub commands: Vec<ModelCommand>,
    pub commit: bool,
    pub admission: crate::project_repository::conditions::ConditionsCommitGuard,
}
#[derive(Default)]
pub(super) struct ProjectConditionsUi {
    pub open: bool,
    pub group: ConditionsGroup,
    pub navigation: Option<ConditionsGroup>,
    pub session: Option<u64>,
    pub site: Option<crate::site::SiteContext>,
    pub original_site: Option<crate::site::SiteContext>,
    pub commands: Vec<ModelCommand>,
    pub staged_base: Option<CommandContext>,
    pub preview: Option<ConditionsPreview>,
    pub error: String,
    pub unresolved: Option<(crate::automation::Target, String)>,
    pub inspected: Option<serde_json::Value>,
    pub file_save_pending: bool,
}
impl AtelierApp {
    pub(super) fn open_project_conditions(&mut self, group: ConditionsGroup) {
        let session = self.host.document.editor.document_session();
        if self.project_conditions_ui.session != Some(session) {
            self.project_conditions_ui = ProjectConditionsUi {
                session: Some(session),
                site: Some(self.host.document.editor.project.site.clone()),
                original_site: Some(self.host.document.editor.project.site.clone()),
                ..Default::default()
            };
        }
        // Refresh only untouched fields. Reopening after undo must show actual
        // project values, while edited fields retain their original conflict base.
        if let (Some(draft), Some(original)) = (
            &mut self.project_conditions_ui.site,
            &mut self.project_conditions_ui.original_site,
        ) {
            let live = &self.host.document.editor.project.site;
            macro_rules! refresh {
                ($field:ident) => {
                    if draft.$field == original.$field {
                        draft.$field = live.$field.clone();
                        original.$field = live.$field.clone();
                    }
                };
            }
            refresh!(horizontal);
            refresh!(true_north);
            refresh!(vertical);
            refresh!(controls);
            refresh!(control_tolerance_m);
            refresh!(boundary);
            refresh!(control_region);
        }
        self.project_conditions_ui.open = true;
        self.project_conditions_ui.group = group;
        self.source_ui.open = true;
        self.program_ui.open = true;
        self.evidence_ui.open = true;
        self.rules_ui.open = true;
        self.terrain_ui.open = true;
    }
    pub(super) fn project_conditions_window(&mut self, ctx: &Context) {
        if let Some(group) = self.project_conditions_ui.navigation.take() {
            self.open_project_conditions(group);
        }
        if !self.project_conditions_ui.open {
            return;
        }
        let l = self.language;
        let mut open = true;
        let mut action = None;
        let mut preview = false;
        let mut save = false;
        let mut discard = false;
        let mut continue_design = false;
        let mut back = false;
        let mut inspect = false;
        let mut save_file = false;
        let busy = self.source_ui.pending_for_ui();
        let unresolved = self.project_conditions_ui.unresolved.is_some();
        egui::Window::new(l.translate("Project conditions")).id(Id::new("project_conditions")).open(&mut open).default_width(900.).default_height(780.).resizable(true).show(ctx,|ui|{
            ui.label(l.translate("Existing project information is loaded. Unknowns may remain blank; design can continue at any time."));
            ui.horizontal_wrapped(|ui|{for group in [ConditionsGroup::Sources,ConditionsGroup::Site,ConditionsGroup::Requirements]{ui.selectable_value(&mut self.project_conditions_ui.group,group,l.translate(group.label()));}});
            ui.separator();
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-270.).max(180.)).show(ui,|ui|{
                if let Some(p)=&self.project_conditions_ui.preview{show_conditions_review(ui,l,&p.review);return;}
                match self.project_conditions_ui.group{
                    ConditionsGroup::Sources=>{
                        ui.label(l.translate("Copied originals and quality reports are stored separately. Discarding condition changes keeps those records."));
                        ui.strong(l.translate("Project attachments"));
                        for page in &self.host.document.editor.project.drawings.pages{ui.label(&page.name);}
                        ui.collapsing(l.translate("Stored originals and source files"),|ui|self.project_sources_contents(ui,l,&mut action));
                        egui::CollapsingHeader::new(l.translate("Quotes, requirements and observations")).default_open(true).show(ui,|ui|self.evidence_conditions_contents(ui));
                        if self.source_ui.review.open { ui.separator();ui.heading(l.translate("Source quality reports"));self.source_review_contents(ui,&mut action); }
                    }
                    ConditionsGroup::Site=>{
                        let choices=self.source_ui.repository.as_ref().filter(|r|r.manifest.document_id==self.host.document.editor.project_identity()).map(|r|r.manifest.sources.iter().map(|s|(s.id.clone(),s.names.join(" / "))).collect::<Vec<_>>()).unwrap_or_default();
                        if let Some(site)=&mut self.project_conditions_ui.site{super::site_ui::site_conditions_fields(ui,site,l,&choices,ctx.content_rect().height()-240.);}
                        ui.collapsing(l.translate("Terrain and surroundings"),|ui|{let mut edit=None;self.terrain_content(ui,l,&mut action,&mut edit);if let Some((base,edit))=edit{self.apply_terrain_edit(base,edit);}});
                    }
                    ConditionsGroup::Requirements=>{
                        if self.rules_ui.register.open {
                            if ui.button(l.translate("Return to requirements")).clicked(){self.rules_ui.register.open=false;}
                            let book=self.host.document.editor.project.rules.clone();let base=self.host.context();let mut intent=None;
                            self.rule_register_embedded(ui,&book,base,&mut intent,&mut action);
                            if let Some(preview)=intent{self.rules_ui.preview=None;self.rules_ui.request=Some(preview.clone());action=Some(super::project_sources_ui::SourceAction::PreviewRule(preview));}
                            return;
                        }
                        egui::CollapsingHeader::new(l.translate("Functional program")).default_open(true).show(ui,|ui|{let mut edit=None;let bound=self.source_ui.repository.as_ref().is_some_and(|r|r.manifest.document_id==self.host.document.editor.project_identity());self.program_content(ui,l,bound,&mut action,&mut edit);if let Some((base,edit))=edit{self.apply_program_edit(base,edit);}});
                        egui::CollapsingHeader::new(l.translate("Use and applicable conditions")).default_open(true).show(ui,|ui|{let book=self.host.document.editor.project.rules.clone();let base=self.host.context();let mut intent=None;self.rules_contents(ui,&book,base,&mut action,&mut intent);if let Some(preview)=intent{self.rules_ui.preview=None;self.rules_ui.request=Some(preview.clone());action=Some(super::project_sources_ui::SourceAction::PreviewRule(preview));}});
                    }
                }
            });
            ui.separator();
            if !self.source_ui.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,l.translate(&self.source_ui.error));}
            if !self.project_conditions_ui.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,l.translate(&self.project_conditions_ui.error));}
            if self.project_conditions_ui.unresolved.is_some(){ui.label(l.translate("The original save outcome must be checked before another submission."));if ui.button(l.translate("Inspect original save outcome")).clicked(){inspect=true;}}
            if let Some(value)=&self.project_conditions_ui.inspected{ui.collapsing(l.translate("Original operation details"),|ui|{ui.monospace(serde_json::to_string_pretty(value).unwrap_or_default());});}
            ui.horizontal_wrapped(|ui|{
                if ui.add_enabled(!busy&&!unresolved&&self.project_conditions_ui.preview.is_none(),Button::new(l.translate("Preview all condition changes"))).clicked(){preview=true;}
                if ui.add_enabled(!busy&&!unresolved&&self.project_conditions_ui.preview.is_some(),Button::new(l.translate("Save project conditions"))).clicked(){save=true;}
                if ui.add_enabled(!busy&&!unresolved&&self.project_conditions_ui.preview.is_some(),Button::new(l.translate("Return to condition editing"))).clicked(){back=true;}
                if ui.add_enabled(!busy&&!unresolved,Button::new(l.translate("Discard condition changes"))).clicked(){discard=true;}
                if self.project_conditions_ui.file_save_pending&&ui.button(l.translate("Save project file")).clicked(){save_file=true;}
                if ui.button(l.translate("Continue designing")).clicked(){continue_design=true;}
            });
        });
        if continue_design {
            open = false;
        }
        self.project_conditions_ui.open = open;
        if !open {
            self.source_ui.review.open = false;
            self.rules_ui.register.open = false;
            self.project_conditions_ui.navigation = None;
            self.source_ui.open = false;
            self.program_ui.open = false;
            self.evidence_ui.open = false;
            self.rules_ui.open = false;
            self.terrain_ui.open = false;
        }
        if let Some(action) = action {
            self.start_source_action(action);
        }
        if discard {
            self.clear_condition_drafts();
            let group = self.project_conditions_ui.group;
            self.project_conditions_ui = Default::default();
            self.open_project_conditions(group);
        }
        if back {
            self.project_conditions_ui.preview = None;
        }
        if inspect {
            self.inspect_condition_save();
        }
        if save_file {
            self.save_applied_conditions_file();
        }
        if preview {
            self.preview_project_conditions();
        }
        if save {
            self.save_project_conditions();
        }
    }

    fn condition_commands(&self) -> Result<Vec<ModelCommand>, String> {
        if !self.project_conditions_ui.commands.is_empty()
            && self.project_conditions_ui.staged_base != Some(self.host.context())
        {
            return Err("Project conditions changed; reload before applying".into());
        }
        let mut commands = self.project_conditions_ui.commands.clone();
        let session = self.host.document.editor.document_session();
        let contexts = self
            .program_ui
            .draft
            .as_ref()
            .map(|d| d.base)
            .into_iter()
            .chain(self.program_ui.preview.as_ref().map(|d| d.base))
            .chain(self.evidence_ui.draft.as_ref().map(|d| d.base))
            .chain(self.terrain_ui.draft.as_ref().map(|d| d.base))
            .chain(self.terrain_ui.preview.as_ref().map(|d| d.base))
            .chain(self.rules_ui.context_draft.as_ref().map(|d| d.2))
            .chain(self.rules_ui.source_draft.as_ref().map(|d| d.1))
            .chain(self.rules_ui.requirement_draft.as_ref().map(|d| d.2))
            .chain(self.rules_ui.preview.as_ref().map(|d| d.base))
            .chain(self.source_ui.boundary_preview.as_ref().map(|d| d.base))
            .chain(self.rules_ui.register.draft.as_ref().map(|d| match d {
                super::rule_register_ui::RegisterDraft::Lifecycle { base, .. }
                | super::rule_register_ui::RegisterDraft::Condition { base, .. } => *base,
            }));
        if contexts.into_iter().any(|base| base.session != session) {
            return Err("Project conditions belong to another document".into());
        }

        if let (Some(site), Some(original)) = (
            &self.project_conditions_ui.site,
            &self.project_conditions_ui.original_site,
        ) {
            if site_reference_changed(site, original) {
                let mut merged = self.host.document.editor.project.site.clone();
                macro_rules! field {
                    ($key:ident) => {
                        if site.$key != original.$key {
                            if merged.$key != original.$key {
                                return Err(
                                    "Project conditions changed; reload before applying".into()
                                );
                            }
                            merged.$key = site.$key.clone();
                        }
                    };
                }
                field!(horizontal);
                field!(true_north);
                field!(vertical);
                field!(controls);
                field!(control_tolerance_m);
                field!(boundary);
                field!(control_region);
                let detach_site = merged.boundary.is_none()
                    && self.host.document.editor.project.site.boundary.is_some();
                let detach_control = merged.control_region.is_none()
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .site
                        .control_region
                        .is_some();
                commands.insert(0, ModelCommand::SetSiteContext { context: merged });
                if detach_site {
                    commands.insert(0, ModelCommand::DetachSiteBoundary);
                }
                if detach_control {
                    commands.insert(0, ModelCommand::DetachSiteControlRegion);
                }
            }
        }
        let mut include = |command: ModelCommand| -> Result<(), String> {
            let key = condition_key(&command)?;
            if let Some(i) = commands
                .iter()
                .position(|c| condition_key(c).ok().as_ref() == Some(&key))
            {
                commands[i] = command;
            } else {
                commands.push(command);
            }
            Ok(())
        };
        if let Some(d) = &self.program_ui.draft {
            if d.values != d.expected {
                include(ModelCommand::Program {
                    edit: crate::program::ProgramEdit::UpdateRow {
                        sheet_id: d.sheet.clone(),
                        key: d.key.clone(),
                        expected: d.expected.clone(),
                        values: d.values.clone(),
                    },
                })?;
            }
        }
        if let Some(p) = &self.program_ui.preview {
            include(ModelCommand::Program {
                edit: crate::program::ProgramEdit::Import {
                    proposal: p.proposal.clone(),
                },
            })?;
        }
        if let Some((context, expected, _)) = &self.rules_ui.context_draft {
            if context != expected {
                include(ModelCommand::Rules {
                    edit: crate::rules::RuleEdit::SetContext {
                        expected: expected.clone(),
                        context: context.clone(),
                    },
                })?;
            }
        }
        if let Some((source, _)) = &self.rules_ui.source_draft {
            include(ModelCommand::Rules {
                edit: crate::rules::RuleEdit::AddSource {
                    source: source.clone(),
                },
            })?;
        }
        if let Some((requirement, expected, _)) = &self.rules_ui.requirement_draft {
            if expected.as_ref() != Some(requirement) {
                include(ModelCommand::Rules {
                    edit: crate::rules::RuleEdit::PutRequirement {
                        requirement: requirement.clone(),
                        expected: expected.clone(),
                    },
                })?;
            }
        }
        if let Some(draft) = &self.rules_ui.register.draft {
            let edit = match draft {
                super::rule_register_ui::RegisterDraft::Lifecycle {
                    value, expected, ..
                } => crate::rules::RuleEdit::PutLifecycle {
                    record: value.clone(),
                    expected: expected.clone(),
                },
                super::rule_register_ui::RegisterDraft::Condition {
                    value, expected, ..
                } => crate::rules::RuleEdit::PutCondition {
                    condition: value.clone(),
                    expected: expected.clone(),
                },
            };
            include(ModelCommand::Rules { edit })?;
        }
        if let Some(p) = &self.rules_ui.preview {
            include(ModelCommand::Rules {
                edit: p.edit.clone(),
            })?;
        }
        if let Some(d) = &self.evidence_ui.draft {
            let changed = evidence_draft_changed(d);
            if changed {
                include(ModelCommand::Evidence {
                    edit: crate::evidence::EvidenceEdit::Put {
                        draft: d.draft.clone(),
                        expected_previous: d.expected.clone(),
                    },
                })?;
            }
        }
        if let Some(d) = &self.terrain_ui.draft {
            if d.position != d.expected {
                include(ModelCommand::Terrain {
                    edit: crate::site::terrain::Edit::UpdateVertex {
                        index: d.index,
                        expected: d.expected,
                        position: d.position,
                    },
                })?;
            }
        }
        if let Some(p) = &self.terrain_ui.preview {
            include(ModelCommand::Terrain {
                edit: crate::site::terrain::Edit::Import {
                    proposal: p.proposal.clone(),
                },
            })?;
        }
        if let Some(p) = &self.source_ui.boundary_preview {
            include(p.command())?;
        }
        Ok(commands)
    }
    pub(super) fn project_conditions_has_draft(&self) -> bool {
        if self.project_conditions_ui.session != Some(self.host.document.editor.document_session())
        {
            return false;
        }
        !self.project_conditions_ui.commands.is_empty()
            || self.project_conditions_ui.unresolved.is_some()
            || self
                .project_conditions_ui
                .site
                .as_ref()
                .zip(self.project_conditions_ui.original_site.as_ref())
                .is_some_and(|(a, b)| site_reference_changed(a, b))
            || self
                .program_ui
                .draft
                .as_ref()
                .is_some_and(|d| d.values != d.expected)
            || self.program_ui.preview.is_some()
            || self
                .program_ui
                .mapping_changed(&self.host.document.editor.project.program)
            || self
                .rules_ui
                .context_draft
                .as_ref()
                .is_some_and(|(a, b, _)| a != b)
            || self.rules_ui.preview.is_some()
            || self.rules_ui.source_draft.is_some()
            || self.rules_ui.requirement_draft.is_some()
            || self.rules_ui.register.draft.is_some()
            || self
                .evidence_ui
                .draft
                .as_ref()
                .is_some_and(|d| evidence_draft_changed(d))
            || self
                .terrain_ui
                .draft
                .as_ref()
                .is_some_and(|d| d.position != d.expected)
            || self.terrain_ui.preview.is_some()
            || self.source_ui.boundary_preview.is_some()
    }
    fn clear_condition_drafts(&mut self) {
        self.project_conditions_ui.commands.clear();
        self.project_conditions_ui.staged_base = None;
        self.project_conditions_ui.preview = None;
        self.project_conditions_ui.inspected = None;
        self.program_ui.draft = None;
        self.program_ui.preview = None;
        self.program_ui.workbook = None;
        self.evidence_ui.draft = None;
        self.evidence_ui.preview = None;
        self.rules_ui.request = None;
        self.rules_ui.preview = None;
        self.rules_ui.context_draft = None;
        self.rules_ui.source_draft = None;
        self.rules_ui.requirement_draft = None;
        self.rules_ui.register.draft = None;
        self.terrain_ui.draft = None;
        self.terrain_ui.preview = None;
        self.source_ui.boundary_preview = None;
    }
    fn prepare_condition_mapping(&mut self) -> Result<(), String> {
        if self.program_ui.preview.is_some()
            || self.program_ui.unit.is_none()
            || self.program_ui.basis.is_none()
        {
            return Ok(());
        }
        let Some((source, workbook)) = &self.program_ui.workbook else {
            return Ok(());
        };
        let (mapping, id, name, expected) = self
            .program_ui
            .mapping_request(&self.host.document.editor.project.program)?;
        if expected.as_ref().is_some_and(|sheet| {
            sheet.mapping == mapping && sheet.name == name && sheet.source_sha256 == *source
        }) {
            return Ok(());
        }
        let proposal =
            crate::program::workbook::map(workbook, source, id, name, mapping, expected)?;
        self.program_ui.preview = Some(super::program_ui::ProgramPreview {
            proposal,
            base: self.host.context(),
        });
        Ok(())
    }
    pub(super) fn preview_project_conditions(&mut self) {
        if self.project_conditions_ui.unresolved.is_some() {
            return;
        }
        if let Err(error) = self.prepare_condition_mapping() {
            self.project_conditions_ui.error = error;
            return;
        }
        let commands = match self.condition_commands() {
            Ok(c) => c,
            Err(e) => {
                self.project_conditions_ui.error = e;
                return;
            }
        };
        let base = self.host.context();
        let request_id = storage::new_document_identity();
        self.project_conditions_ui.preview = None;
        if crate::project_repository::conditions::needs_sources(&commands) {
            self.start_source_action(super::project_sources_ui::SourceAction::Conditions {
                commands,
                base,
                request_id,
                commit: false,
            });
        } else {
            match crate::project_repository::conditions::guard_conditions(
                None,
                self.host.document.editor.project_identity(),
                &self.host.document.editor.project,
                &commands,
            ) {
                Ok(admission) => self.accept_conditions_job(ConditionsJob {
                    commands,
                    base,
                    request_id,
                    commit: false,
                    admission,
                }),
                Err(e) => self.project_conditions_ui.error = e,
            }
        }
    }
    pub(super) fn save_project_conditions(&mut self) {
        if self.project_conditions_ui.unresolved.is_some() {
            return;
        }
        let Some(p) = &self.project_conditions_ui.preview else {
            return;
        };
        let commands = match self.condition_commands() {
            Ok(c) => c,
            Err(e) => {
                self.project_conditions_ui.error = e;
                return;
            }
        };
        let base = p.base;
        let request_id = p.request_id.clone();
        if crate::project_repository::conditions::needs_sources(&commands) {
            self.start_source_action(super::project_sources_ui::SourceAction::Conditions {
                commands,
                base,
                request_id,
                commit: true,
            });
        } else {
            match crate::project_repository::conditions::guard_conditions(
                None,
                self.host.document.editor.project_identity(),
                &self.host.document.editor.project,
                &commands,
            ) {
                Ok(admission) => self.accept_conditions_job(ConditionsJob {
                    commands,
                    base,
                    request_id,
                    commit: true,
                    admission,
                }),
                Err(e) => self.project_conditions_ui.error = e,
            }
        }
    }
    pub(super) fn accept_conditions_job(&mut self, job: ConditionsJob) {
        if !self.project_conditions_ui.open
            || job.base != self.host.context()
            || self.condition_commands().ok().as_ref() != Some(&job.commands)
        {
            self.project_conditions_ui.error =
                "Project conditions or reviewed changes have changed".into();
            return;
        }
        let imported_role = job.commands.iter().rev().find_map(|command| match command {
            ModelCommand::ApplySiteBoundary { .. } => Some(crate::site::boundary::Role::Site),
            ModelCommand::ApplySiteControlRegion { .. } => {
                Some(crate::site::boundary::Role::ControlRegion)
            }
            _ => None,
        });
        match self.host.document.editor.prepare_conditions_change(
            job.base.revision,
            job.commands,
            &job.admission,
        ) {
            Ok(prepared) => {
                if !job.commit {
                    self.project_conditions_ui.preview = Some(ConditionsPreview {
                        base: job.base,
                        request_id: job.request_id,
                        review: conditions_review_rows(
                            &self.host.document.editor.project,
                            prepared.project(),
                        ),
                        prepared: std::rc::Rc::new(prepared),
                    });
                    self.project_conditions_ui.error.clear();
                    return;
                }
                let Some(reviewed) = self.project_conditions_ui.preview.take() else {
                    self.project_conditions_ui.error =
                        "Preview project conditions before saving".into();
                    return;
                };
                if reviewed.request_id != job.request_id
                    || reviewed.base != job.base
                    || !prepared
                        .project()
                        .same_model_state(reviewed.prepared.project())
                {
                    self.project_conditions_ui.error =
                        "Project conditions or reviewed changes have changed".into();
                    self.project_conditions_ui.preview = Some(reviewed);
                    return;
                }
                let original_target = self.host.operation_target();
                let result = self.host.confirm_task_prepared(
                    reviewed.base,
                    &reviewed.request_id,
                    "Edit project conditions",
                    reviewed.prepared.clone(),
                );
                if result.get("result").is_some() {
                    let imported = imported_role.and_then(|role| {
                        role.get(&self.host.document.editor.project.site)
                            .map(|r| r.element_id)
                    });
                    self.clear_condition_drafts();
                    self.project_conditions_ui.site =
                        Some(self.host.document.editor.project.site.clone());
                    self.project_conditions_ui.original_site =
                        self.project_conditions_ui.site.clone();
                    self.project_conditions_ui.error.clear();
                    if let Some(id) = imported {
                        self.focus_imported_reference(id);
                    }
                    self.project_conditions_ui.file_save_pending = true;
                    self.save_applied_conditions_file();
                } else {
                    let code = result["error"]["code"]
                        .as_str()
                        .unwrap_or("OPERATION_OUTCOME_UNKNOWN");
                    if result["operation_submitted"] == true
                        || matches!(
                            code,
                            "RECOVERY_REQUIRED"
                                | "OPERATION_OUTCOME_UNKNOWN"
                                | "TASK_OPERATION_PENDING"
                        )
                    {
                        let target =
                            serde_json::from_value(result["error"]["original_target"].clone())
                                .unwrap_or(original_target);
                        self.project_conditions_ui.unresolved =
                            Some((target, reviewed.request_id.clone()));
                    }
                    self.project_conditions_ui.error = code.into();
                    self.project_conditions_ui.preview = Some(reviewed);
                }
            }
            Err(e) => self.project_conditions_ui.error = e.message,
        }
    }
    fn save_applied_conditions_file(&mut self) {
        if self.file_baseline.is_some() {
            match self.save_project() {
                Ok(()) => {
                    self.project_conditions_ui.file_save_pending = false;
                    self.project_conditions_ui.error.clear();
                }
                Err(error) => {
                    self.project_conditions_ui.error = format!(
                        "{}: {}",
                        self.language
                            .translate("Conditions applied; the project file was not saved"),
                        error
                    )
                }
            }
        } else {
            self.file_action = Some(FileAction::Save);
        }
    }
    fn inspect_condition_save(&mut self) {
        let Some((target, id)) = self.project_conditions_ui.unresolved.clone() else {
            return;
        };
        let result = self.host.inspect_operation(target, &id);
        if result["result"]["status"] == "committed"
            && result["result"]["current_effect"] == "AfterImage"
        {
            self.clear_condition_drafts();
            self.project_conditions_ui.unresolved = None;
            self.project_conditions_ui.site = Some(self.host.document.editor.project.site.clone());
            self.project_conditions_ui.original_site = self.project_conditions_ui.site.clone();
            self.project_conditions_ui.file_save_pending = true;
            self.project_conditions_ui.error =
                "Conditions applied; the project file was not saved".into();
        }
        self.project_conditions_ui.inspected = Some(result);
    }
    pub(super) fn stage_condition_commands(
        &mut self,
        base: CommandContext,
        commands: Vec<ModelCommand>,
    ) -> Result<(), String> {
        if self.project_conditions_ui.session != Some(base.session)
            || base.session != self.host.context().session
        {
            return Err("Project conditions belong to another document".into());
        }
        if base != self.host.context()
            || self
                .project_conditions_ui
                .staged_base
                .is_some_and(|previous| previous != base)
        {
            return Err("Project conditions changed; reload before applying".into());
        }
        self.project_conditions_ui.staged_base = Some(base);
        for command in commands {
            let key = condition_key(&command)?;
            if let Some(i) = self
                .project_conditions_ui
                .commands
                .iter()
                .position(|c| condition_key(c).ok().as_ref() == Some(&key))
            {
                self.project_conditions_ui.commands[i] = command;
            } else {
                self.project_conditions_ui.commands.push(command);
            }
        }
        self.project_conditions_ui.preview = None;
        self.project_conditions_ui.error.clear();
        Ok(())
    }
    pub(super) fn dispatch_project_preparation(
        &mut self,
        context: CommandContext,
        command: HostCommand,
    ) -> Result<Option<crate::model_service::ChangeReceipt>, String> {
        if self.project_conditions_ui.open {
            if let HostCommand::Apply { commands, .. } = command {
                self.stage_condition_commands(context, commands)?;
                return Ok(None);
            }
        }
        self.host.dispatch(context, command)
    }
}
fn condition_key(command: &ModelCommand) -> Result<String, String> {
    use crate::{
        evidence::EvidenceEdit as E, program::ProgramEdit as P, rules::RuleEdit as R,
        site::terrain::Edit as T,
    };
    Ok(match command {
        ModelCommand::SetSiteContext { .. } => "site.reference".into(),
        ModelCommand::ApplySiteControlRegion { .. } | ModelCommand::DetachSiteControlRegion => {
            "site.control_region".into()
        }
        ModelCommand::ApplySiteBoundary { .. } | ModelCommand::DetachSiteBoundary => {
            "site.boundary".into()
        }
        ModelCommand::Program { edit } => match edit {
            P::Import { proposal } => format!("program:{}", proposal.sheet.id),
            P::RemoveSheet { expected } => format!("program:{}", expected.id),
            P::UpdateRow { sheet_id, key, .. } => format!("program:{sheet_id}:{key}"),
        },
        ModelCommand::Evidence { edit } => format!(
            "evidence:{}",
            match edit {
                E::Put { draft, .. } => &draft.id,
                E::Remove { expected } => &expected.id,
                E::RecordTopic { id, .. }
                | E::RecordTopicReply { id, .. }
                | E::SubmitTopicTask { id, .. }
                | E::TopicTaskEvent { id, .. } => id,
            }
        ),
        ModelCommand::Terrain { edit } => match edit {
            T::UpdateVertex { index, .. } => format!("terrain:{index}"),
            _ => "terrain".into(),
        },
        ModelCommand::Rules { edit } => match edit {
            R::SetContext { .. } => "rules.context".into(),
            R::AddSource { source } => format!("rules.source:{}", source.id),
            R::RemoveSource { expected } => format!("rules.source:{}", expected.id),
            R::PutRequirement { requirement, .. } => {
                format!("rules.requirement:{}", requirement.id)
            }
            R::RemoveRequirement { expected } => format!("rules.requirement:{}", expected.id),
            R::PutLifecycle { record, .. } => format!("rules.lifecycle:{}", record.id),
            R::RemoveLifecycle { expected } => format!("rules.lifecycle:{}", expected.id),
            R::PutCondition { condition, .. } => format!("rules.condition:{}", condition.id),
            R::RemoveCondition { expected } => format!("rules.condition:{}", expected.id),
        },
        _ => return Err("Command is outside project condition editing".into()),
    })
}

fn site_reference_changed(a: &crate::site::SiteContext, b: &crate::site::SiteContext) -> bool {
    a.horizontal != b.horizontal
        || a.true_north != b.true_north
        || a.vertical != b.vertical
        || a.controls != b.controls
        || a.control_tolerance_m != b.control_tolerance_m
        || a.boundary != b.boundary
        || a.control_region != b.control_region
}
fn evidence_draft_changed(d: &super::evidence_ui::DraftState) -> bool {
    d.expected.as_ref().is_none_or(|n| {
        n.title != d.draft.title
            || n.kind != d.draft.kind
            || n.quote != d.draft.quote
            || n.annotation != d.draft.annotation
            || n.links.iter().map(|l| &l.target).collect::<Vec<_>>()
                != d.draft.targets.iter().collect::<Vec<_>>()
            || d.draft.refresh_links
    })
}

struct ConditionRow {
    label: String,
    before: String,
    after: String,
}
fn conditions_review_rows(before: &Project, after: &Project) -> Vec<ConditionRow> {
    let mut rows = Vec::new();
    let mut row = |label: &str, a: String, b: String| {
        if a != b {
            rows.push(ConditionRow {
                label: label.into(),
                before: a,
                after: b,
            });
        }
    };
    let north = |n: &Option<crate::site::TrueNorth>| {
        n.as_ref().map_or("Unknown".into(), |v| {
            format!("{:.3}°", v.clockwise_from_model_y_deg)
        })
    };
    let horizontal = |n: &Option<crate::site::HorizontalReference>| {
        n.as_ref().map_or("Unknown".into(), |v| {
            format!(
                "{} · {:.3} / {:.3} m · {:.3}°",
                v.crs, v.origin_east_north_m[0], v.origin_east_north_m[1], v.model_to_grid_ccw_deg
            )
        })
    };
    let vertical = |n: &Option<crate::site::VerticalReference>| {
        n.as_ref().map_or("Unknown".into(), |v| {
            format!("{} · {:.3} m", v.datum, v.model_zero_elevation_m)
        })
    };
    let tolerance = |n: Option<f64>| n.map_or("Unknown".into(), |v| format!("{v:.4} m"));
    row(
        "Projected reference",
        horizontal(&before.site.horizontal),
        horizontal(&after.site.horizontal),
    );
    row(
        "True north",
        north(&before.site.true_north),
        north(&after.site.true_north),
    );
    row(
        "Elevation datum",
        vertical(&before.site.vertical),
        vertical(&after.site.vertical),
    );
    row(
        "Control tolerance (m)",
        tolerance(before.site.control_tolerance_m),
        tolerance(after.site.control_tolerance_m),
    );
    for (key, a, b) in [
        (
            "Jurisdiction",
            &before.rules.context.jurisdiction,
            &after.rules.context.jurisdiction,
        ),
        (
            "Project use",
            &before.rules.context.uses,
            &after.rules.context.uses,
        ),
        (
            "Assessment date",
            &before.rules.context.assessment_date,
            &after.rules.context.assessment_date,
        ),
        (
            "Project stage",
            &before.rules.context.stage,
            &after.rules.context.stage,
        ),
    ] {
        row(
            key,
            if a.is_empty() {
                "Unknown".into()
            } else {
                a.clone()
            },
            if b.is_empty() {
                "Unknown".into()
            } else {
                b.clone()
            },
        );
    }
    // Other source-bound records have many exact fields. Keep their complete
    // before/after data inspectable rather than substituting an item count.
    for (key, a, b) in [
        (
            "Coordinate controls",
            serde_json::to_value(&before.site.controls),
            serde_json::to_value(&after.site.controls),
        ),
        (
            "Site source evidence",
            serde_json::to_value((
                &before.site.horizontal,
                &before.site.true_north,
                &before.site.vertical,
            )),
            serde_json::to_value((
                &after.site.horizontal,
                &after.site.true_north,
                &after.site.vertical,
            )),
        ),
        (
            "Site boundary",
            serde_json::to_value(&before.site.boundary),
            serde_json::to_value(&after.site.boundary),
        ),
        (
            "Functional program",
            serde_json::to_value(&before.program),
            serde_json::to_value(&after.program),
        ),
        (
            "Quotes, requirements and observations",
            serde_json::to_value(&before.evidence),
            serde_json::to_value(&after.evidence),
        ),
        (
            "Cited sources and conditions",
            serde_json::to_value((
                &before.rules.sources,
                &before.rules.requirements,
                &before.rules.lifecycle,
                &before.rules.conditions,
            )),
            serde_json::to_value((
                &after.rules.sources,
                &after.rules.requirements,
                &after.rules.lifecycle,
                &after.rules.conditions,
            )),
        ),
    ] {
        if let (Ok(a), Ok(b)) = (a, b) {
            if a != b {
                row(
                    key,
                    serde_json::to_string_pretty(&a).unwrap_or_default(),
                    serde_json::to_string_pretty(&b).unwrap_or_default(),
                );
            }
        }
    }
    if before.site.terrain != after.site.terrain {
        row(
            "Terrain and surroundings",
            serde_json::to_string_pretty(&crate::site::terrain::summary(&before.site))
                .unwrap_or_default(),
            serde_json::to_string_pretty(&crate::site::terrain::summary(&after.site))
                .unwrap_or_default(),
        );
    }
    rows
}
fn show_conditions_review(ui: &mut Ui, l: UiLanguage, rows: &[ConditionRow]) {
    ui.heading(l.translate("Review project condition changes"));
    ui.label(l.translate("All reviewed changes are saved together. Unknowns remain unknown."));
    for row in rows {
        if row.before.len() + row.after.len() < 500
            && !row.before.contains('\n')
            && !row.after.contains('\n')
        {
            ui.label(format!(
                "{}: {} → {}",
                l.translate(&row.label),
                l.translate(&row.before),
                l.translate(&row.after)
            ));
        } else {
            ui.collapsing(l.translate(&row.label), |ui| {
                ui.strong(l.translate("Before"));
                ui.label(&row.before);
                ui.strong(l.translate("After"));
                ui.label(&row.after);
            });
        }
    }
}
