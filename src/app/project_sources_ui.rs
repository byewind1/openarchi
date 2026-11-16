use super::*;
use crate::project_repository::{LengthUnit, Repository};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Default)]
pub(super) struct ProjectSourcesUi {
    pub open: bool,
    pub review: super::source_review_ui::ReviewUi,
    pub root: String,
    pub source_path: String,
    pub copy_root: String,
    pub converter_path: String,
    pub rejection_reason: String,
    pub boundary_handle: String,
    pub boundary_role: crate::site::boundary::Role,
    pub boundary_filter: String,
    pub(super) boundary_entities: Option<(String, Vec<crate::dxf::inventory::EntityRecord>)>,
    pub boundary_name: String,
    pub boundary_layer: u32,
    pub boundary_placement: Option<crate::project_repository::boundary::BoundaryPlacement>,
    pub boundary_centerline: bool,
    pub(super) boundary_preview: Option<BoundaryPreview>,
    pub repository: Option<Repository>,
    pub(super) connections_directory: Option<PathBuf>,
    restore_session: Option<u64>,
    known_connection: Option<crate::project_repository::connection::Connection>,
    owned_model_connection: Option<crate::project_repository::connection::Connection>,
    pub error: String,
    receipt: Option<crate::project_repository::conversion::ConversionReceipt>,
    pending: Option<SourceJob>,
    unit_choices: std::collections::BTreeMap<String, Option<LengthUnit>>,
}
#[derive(Clone)]
pub(super) struct BoundaryPreview {
    pub proposal: crate::site::boundary::BoundaryProposal,
    pub role: crate::site::boundary::Role,
    pub base: crate::application_host::CommandContext,
}
impl BoundaryPreview {
    pub(super) fn command(&self) -> crate::command::ModelCommand {
        match self.role {
            crate::site::boundary::Role::Site => crate::command::ModelCommand::ApplySiteBoundary {
                proposal: self.proposal.clone(),
            },
            crate::site::boundary::Role::ControlRegion => {
                crate::command::ModelCommand::ApplySiteControlRegion {
                    proposal: self.proposal.clone(),
                }
            }
        }
    }
}
struct SourceJob {
    document_id: String,
    session: u64,
    restoring: bool,
    generation: Option<String>,
    result: Receiver<Result<SourceResult, String>>,
}
struct SourceResult {
    repository: Repository,
    reviews: Option<(String, Vec<crate::project_repository::quality::Review>)>,
    review_operation: Option<(String, crate::project_repository::quality::ReviewOperation)>,
    model_path: Option<PathBuf>,
    receipt: Option<crate::project_repository::conversion::ConversionReceipt>,
    boundary: Option<BoundaryPreview>,
    ready: Option<crate::project_repository::boundary::BoundaryCommitGuard>,
    workbook: Option<(String, crate::program::workbook::Workbook)>,
    evidence: Option<super::evidence_ui::EvidencePreview>,
    evidence_ready: Option<crate::project_repository::evidence::EvidenceCommitGuard>,
    rule_original: Option<crate::project_repository::rules::RuleOriginal>,
    rule_preview: Option<super::rules_ui::RulePreview>,
    rule_commit: bool,
    rule_ready: Option<crate::project_repository::rules::RuleCommitGuard>,
    terrain: Option<super::terrain_ui::TerrainPreview>,
    terrain_ready: Option<crate::project_repository::terrain::TerrainCommitGuard>,
    program: Option<super::program_ui::ProgramPreview>,
    program_ready: Option<crate::project_repository::program::ProgramCommitGuard>,
    entities: Option<(String, Vec<crate::dxf::inventory::EntityRecord>)>,
    conditions: Option<super::project_conditions_ui::ConditionsJob>,
    checks: Option<super::checks_ui::ChecksJob>,
    checks_read: Option<super::checks_ui::ChecksReadResult>,
}
pub(super) enum SourceAction {
    ReadChecks(super::checks_ui::ChecksReadJob),
    CheckOriginals {
        commands: Vec<crate::command::ModelCommand>,
        base: crate::application_host::CommandContext,
        request_id: String,
        commit: bool,
        book: crate::rules::RuleBook,
    },
    Conditions {
        commands: Vec<crate::command::ModelCommand>,
        base: crate::application_host::CommandContext,
        request_id: String,
        commit: bool,
    },
    InspectRuleOriginal(String),
    PreviewRule(super::rules_ui::RulePreview),
    CommitRule(super::rules_ui::RulePreview),
    Create,
    Checkpoint,
    Open,
    Restore(crate::project_repository::connection::Connection),
    CopyStored(String),
    Add,
    InspectReviews(String),
    RecordReview(super::source_review_ui::ReviewRequest),
    ReviewOperation(String),
    Convert(String),
    Reject(String, String),
    Confirm(String, Option<LengthUnit>),
    OpenModel,
    PreviewBoundary(String),
    ListBoundaryEntities(String),
    InspectProgram(String),
    PreviewEvidence(super::evidence_ui::DraftState),
    CommitEvidence(super::evidence_ui::EvidencePreview),
    PreviewTerrain(String),
    CommitTerrain(super::terrain_ui::TerrainPreview),
    PreviewProgram(String),
    CommitProgram(super::program_ui::ProgramPreview),
    CommitBoundary(BoundaryPreview),
}
impl ProjectSourcesUi {
    pub(super) fn pending_for_ui(&self) -> bool {
        self.pending.is_some() || self.restore_session.is_some()
    }
    #[cfg(test)]
    pub(super) fn pending_for_test(&self) -> bool {
        self.pending_for_ui()
    }
}
impl AtelierApp {
    pub(super) fn focus_imported_reference(&mut self, id: u64) {
        let p = &self.host.document.editor.project;
        let Some(element) = p.elements.iter().find(|e| e.id == id) else {
            return;
        };
        let story = element.story;
        let worksheet = element.worksheet;
        let points = geometry::element_footprint(p, element).to_vec();
        self.host.document.editor.switch_worksheet(worksheet);
        self.host.document.editor.story = story;
        self.host.document.editor.set_tool(Tool::Select);
        self.host.document.editor.set_selection(vec![id]);
        self.view = View::Plan;
        self.fit_plan_points_in_viewport(points, vec2(600., 420.));
    }
    pub(super) fn initialize_source_connections(&mut self, directory: &std::path::Path) {
        // Same application configuration root; tests explicitly provide an isolated root.
        match std::fs::create_dir_all(directory).and_then(|_| directory.canonicalize()) {
            Ok(root) => {
                self.source_ui.connections_directory = Some(root.join("source-connections"))
            }
            Err(error) => self.source_ui.error = error.to_string(),
        }
    }
    pub(super) fn source_document_opened(&mut self) {
        self.restore_environment_creation_presets();
        let owned = self.source_ui.owned_model_connection.take();
        self.source_ui.repository = None;
        self.source_ui.known_connection = None;
        self.source_ui.root.clear();
        self.source_ui.error.clear();
        self.source_ui.restore_session = Some(self.host.document.editor.document_session());
        if let (Some(connection), Some((path, version))) = (owned, &self.file_baseline)
            && version.token().as_deref().is_some_and(|v| {
                connection.matches(path, self.host.document.editor.project_identity(), v)
            })
        {
            self.source_ui.restore_session = None;
            self.source_ui.root = connection.repository_path().display().to_string();
            self.source_ui.known_connection = Some(connection.clone());
            // Explicit Open stored model produced this exact copy. Preserve its
            // source connection even when no personal configuration is available.
            self.start_source_action(SourceAction::Restore(connection));
            return;
        }
        self.try_restore_source_connection();
    }
    fn try_restore_source_connection(&mut self) {
        if self.source_ui.pending.is_some() {
            return;
        }
        let Some(session) = self.source_ui.restore_session.take() else {
            return;
        };
        if session != self.host.document.editor.document_session() {
            return;
        }
        let Some(directory) = &self.source_ui.connections_directory else {
            return;
        };
        let Some((path, version)) = &self.file_baseline else {
            return;
        };
        let Some(version) = version.token() else {
            return;
        };
        match crate::project_repository::connection::find(
            directory,
            path,
            self.host.document.editor.project_identity(),
            &version,
        ) {
            Ok(Some(connection)) => {
                self.source_ui.root = connection.repository_path().display().to_string();
                self.source_ui.known_connection = Some(connection.clone());
                self.start_source_action(SourceAction::Restore(connection));
            }
            Ok(None) => {}
            Err(error) => self.source_ui.error = error,
        }
    }
    pub(super) fn remember_source_connection(&mut self, after_save: bool) {
        let Some(directory) = &self.source_ui.connections_directory else {
            return;
        };
        let id = self.host.document.editor.project_identity();
        let Some((path, version)) = &self.file_baseline else {
            return;
        };
        let Some(version) = version.token() else {
            return;
        };
        let result = if let Some(repository) = self
            .source_ui
            .repository
            .as_ref()
            .filter(|r| r.manifest.document_id == id)
        {
            crate::project_repository::connection::remember(
                directory, path, id, &version, repository,
            )
        } else if let Some(connection) = &self.source_ui.known_connection {
            connection.rebind(directory, path, id, &version)
        } else {
            return;
        };
        if let Ok(connection) = result {
            self.source_ui.known_connection = Some(connection);
            if after_save
                && self
                    .source_ui
                    .pending
                    .as_ref()
                    .is_some_and(|job| job.restoring)
            {
                self.source_ui.restore_session = Some(self.host.document.editor.document_session());
            }
        } else if let Err(error) = result {
            let message = format!(
                "{}: {}",
                self.language.translate(if after_save {
                    "Project saved; its source folder connection was not remembered"
                } else {
                    "Source folder connection was not remembered"
                }),
                error
            );
            self.source_ui.error = message.clone();
            self.host.document.editor.status = message;
        }
    }
    pub(super) fn start_source_action(&mut self, action: SourceAction) {
        if self.source_ui.pending.is_some() {
            return;
        }
        if !matches!(action, SourceAction::Restore(_)) {
            self.source_ui.restore_session = None;
        }
        if self.project_conditions_ui.open {
            if let SourceAction::PreviewRule(preview) | SourceAction::CommitRule(preview) = &action
            {
                let command = crate::command::ModelCommand::Rules {
                    edit: preview.edit.clone(),
                };
                if !crate::project_repository::conditions::needs_sources(std::slice::from_ref(
                    &command,
                )) {
                    match crate::rules::apply(
                        &self.host.document.editor.project.rules,
                        &preview.edit,
                    )
                    .and_then(|book| {
                        book.validate_edit_against_project(
                            &self.host.document.editor.project,
                            &preview.edit,
                        )
                    }) {
                        Ok(()) => {
                            self.rules_ui.preview = Some(preview.clone());
                            self.source_ui.error.clear();
                        }
                        Err(e) => self.source_ui.error = e,
                    }
                    return;
                }
            }
        }
        if let SourceAction::PreviewEvidence(state) = &action {
            self.evidence_ui.preview = None;
            if state.base != self.host.context() {
                self.source_ui.error = "Model changed; reload the source note".into();
                return;
            }
        }
        if matches!(action, SourceAction::PreviewTerrain(..)) {
            self.terrain_ui.preview = None;
        }
        if matches!(action, SourceAction::PreviewBoundary(..)) {
            self.source_ui.boundary_preview = None;
        }
        let program_request = if matches!(action, SourceAction::PreviewProgram(..)) {
            self.program_ui.preview = None;
            match self
                .program_ui
                .mapping_request(&self.host.document.editor.project.program)
            {
                Ok(request) => Some(request),
                Err(error) => {
                    self.source_ui.error = error;
                    return;
                }
            }
        } else {
            None
        };
        let document_id = self.host.document.editor.project_identity().to_owned();
        let session = self.host.document.editor.document_session();
        let generation = self
            .host
            .document
            .editor
            .file_generation()
            .map(str::to_owned);
        let restoring = matches!(action, SourceAction::Restore(_));
        let root = PathBuf::from(self.source_ui.root.trim());
        if root.as_os_str().is_empty() {
            self.source_ui.error = "Choose a project folder".into();
            return;
        }
        let rule_commit = matches!(action, SourceAction::CommitRule(..));
        let repository = self.source_ui.repository.clone();
        if matches!(
            action,
            SourceAction::ReadChecks(..)
                | SourceAction::CheckOriginals { .. }
                | SourceAction::Conditions { .. }
                | SourceAction::Add
                | SourceAction::PreviewTerrain(..)
                | SourceAction::CommitTerrain(..)
                | SourceAction::Confirm(..)
                | SourceAction::Checkpoint
                | SourceAction::Convert(..)
                | SourceAction::Reject(..)
                | SourceAction::PreviewBoundary(..)
                | SourceAction::InspectProgram(..)
                | SourceAction::RecordReview(..)
                | SourceAction::PreviewEvidence(..)
                | SourceAction::InspectRuleOriginal(..)
                | SourceAction::PreviewRule(..)
                | SourceAction::CommitRule(..)
                | SourceAction::CommitEvidence(..)
                | SourceAction::PreviewProgram(..)
                | SourceAction::CommitProgram(..)
                | SourceAction::ListBoundaryEntities(..)
                | SourceAction::CommitBoundary(..)
        ) && repository
            .as_ref()
            .is_none_or(|r| r.manifest.document_id != document_id)
        {
            self.source_ui.error = "Connect sources for the current project to use source-backed conditions. You can continue designing without them.".into();
            return;
        }
        if let SourceAction::RecordReview(request) = &action {
            if repository.as_ref().is_none_or(|r| r.root() != request.root) {
                self.source_ui.error =
                    "Reopen the original project folder to inspect this operation".into();
                return;
            }
        }
        if let SourceAction::ReviewOperation(id) = &action {
            if self.source_ui.review.pending.as_ref().is_none_or(|p| {
                p.operation_id != *id || repository.as_ref().is_none_or(|r| r.root() != p.root)
            }) {
                self.source_ui.error =
                    "Reopen the original project folder to inspect this operation".into();
                return;
            }
        }
        let save_snapshot = matches!(action, SourceAction::Create | SourceAction::Checkpoint)
            .then(|| self.host.project_snapshot_for_save());
        let snapshot = matches!(
            action,
            SourceAction::Conditions { .. }
                | SourceAction::Create
                | SourceAction::Checkpoint
                | SourceAction::PreviewBoundary(..)
                | SourceAction::PreviewTerrain(..)
                | SourceAction::PreviewEvidence(..)
                | SourceAction::InspectRuleOriginal(..)
                | SourceAction::PreviewRule(..)
                | SourceAction::CommitRule(..)
        )
        .then(|| {
            let mut project = self.host.document.editor.project.clone();
            if self.project_conditions_ui.open
                && matches!(
                    action,
                    SourceAction::PreviewBoundary(..) | SourceAction::PreviewTerrain(..)
                )
            {
                if let Some(site) = &self.project_conditions_ui.site {
                    let terrain = project.site.terrain.take();
                    let boundary = project.site.boundary.take();
                    project.site = site.clone();
                    project.site.terrain = terrain;
                    project.site.boundary = boundary;
                }
            }
            project
        });
        let revision = self.host.document.editor.revision;
        let path = PathBuf::from(self.source_ui.source_path.trim());
        let identity = document_id.clone();
        let converter = PathBuf::from(self.source_ui.converter_path.trim());
        let base = self.host.context();
        let terrain_name = self.terrain_ui.name.clone();
        let terrain_placement = self.terrain_ui.placement;
        let handle = self.source_ui.boundary_handle.trim().to_owned();
        let role = self.source_ui.boundary_role;
        if role == crate::site::boundary::Role::ControlRegion
            && self.source_ui.boundary_placement.is_none()
        {
            if let (SourceAction::PreviewBoundary(id), Some(project), Some(repository)) = (
                &action,
                snapshot.as_ref(),
                self.source_ui.repository.as_ref(),
            ) {
                let unit = repository
                    .manifest
                    .sources
                    .iter()
                    .find(|s| s.id == *id)
                    .and_then(|s| s.confirmed_unit);
                let origin = [
                    project.site.control_region.as_ref(),
                    project.site.boundary.as_ref(),
                ]
                .into_iter()
                .flatten()
                .find(|b| b.origin.source_sha256 == *id && unit == Some(b.origin.unit))
                .map(|b| &b.origin);
                self.source_ui.boundary_placement=origin.and_then(|o|match &o.placement {
                    crate::site::boundary::Placement::ModelAxes=>Some(crate::project_repository::boundary::BoundaryPlacement::ModelAxes),
                    crate::site::boundary::Placement::Projected{reference} if project.site.horizontal.as_ref()==Some(reference)=>Some(crate::project_repository::boundary::BoundaryPlacement::ProjectedReference),
                    _=>None,
                });
            }
        }
        let placement = self.source_ui.boundary_placement;
        let width_policy = if self.source_ui.boundary_centerline {
            crate::project_repository::boundary::BoundaryWidthPolicy::ConstantWidthCenterline
        } else {
            crate::project_repository::boundary::BoundaryWidthPolicy::RejectNonzero
        };
        let name = self.source_ui.boundary_name.clone();
        let story = self.host.document.editor.story;
        let layer = self.source_ui.boundary_layer;
        let (story, layer) = role
            .get(&self.host.document.editor.project.site)
            .and_then(|b| {
                self.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == b.element_id)
            })
            .map_or((story, layer), |e| (e.story, e.layer.0));
        let (sender, result) = mpsc::channel();
        match std::thread::Builder::new()
            .name("project-source-admission".into())
            .spawn(move || {
                let mut boundary = None;
                let mut ready = None;
                let mut workbook = None;
                let mut reviews = None;
                let mut review_operation = None;
                let mut rule_original = None;
                let mut rule_preview = None;
                let mut rule_ready = None;
                let mut evidence = None;
                let mut evidence_ready = None;
                let mut terrain = None;
                let mut terrain_ready = None;
                let mut program = None;
                let mut program_ready = None;
                let mut entities = None;
                let mut conditions = None;
                let mut checks = None;
                let mut checks_read = None;
                let output = (|| match action {
                    SourceAction::ReadChecks(read) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let observation = read.plan.execute();
                        let mut entries = read.entries;
                        observation.apply_to_entries(&mut entries);
                        checks_read = Some(super::checks_ui::ChecksReadResult {
                            key: read.key,
                            entries,
                            checked_at: std::time::Instant::now(),
                        });
                        Ok((repository, None))
                    }
                    SourceAction::CheckOriginals {
                        commands,
                        base,
                        request_id,
                        commit,
                        book,
                    } => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let admission = crate::project_repository::constraints::guard_cited_runs(
                            &repository,
                            &identity,
                            &book,
                            &commands,
                        );
                        checks = Some(super::checks_ui::ChecksJob {
                            commands,
                            base,
                            request_id,
                            commit,
                            admission,
                        });
                        Ok((repository, None))
                    }
                    SourceAction::Conditions {
                        commands,
                        base,
                        request_id,
                        commit,
                    } => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let project = snapshot.ok_or("Missing model snapshot")?;
                        let admission = crate::project_repository::conditions::guard_conditions(
                            Some(&repository),
                            &identity,
                            &project,
                            &commands,
                        )?;
                        conditions = Some(super::project_conditions_ui::ConditionsJob {
                            commands,
                            base,
                            request_id,
                            commit,
                            admission,
                        });
                        Ok((repository, None))
                    }
                    SourceAction::InspectRuleOriginal(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        rule_original = Some(repository.inspect_rule_original(&id)?);
                        Ok((repository, None))
                    }
                    SourceAction::PreviewRule(preview) | SourceAction::CommitRule(preview) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let project = snapshot.ok_or("Missing model snapshot")?;
                        let candidate = crate::rules::apply(&project.rules, &preview.edit)?;
                        candidate.validate_edit_against_project(&project, &preview.edit)?;
                        rule_ready =
                            Some(repository.guard_rule_edit(&project.rules, &preview.edit)?);
                        rule_preview = Some(preview);
                        Ok((repository, None))
                    }
                    SourceAction::Create => Repository::create(
                        &root,
                        &save_snapshot.ok_or("Missing model snapshot")?.collect()?,
                        &identity,
                        revision,
                    )
                    .map(|r| (r, None)),
                    SourceAction::Open => Repository::open(&root).map(|r| (r, None)),
                    SourceAction::Restore(connection) => connection.open().map(|r| (r, None)),
                    SourceAction::CopyStored(destination) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        Ok((
                            repository.copy_to(PathBuf::from(destination.trim()).as_path())?,
                            None,
                        ))
                    }
                    SourceAction::Checkpoint => {
                        let mut repository = repository.ok_or("Open a project repository first")?;
                        repository.checkpoint(
                            &save_snapshot.ok_or("Missing model snapshot")?.collect()?,
                            &identity,
                            revision,
                        )?;
                        Ok((repository, None))
                    }
                    SourceAction::Add => {
                        let mut repository = repository.ok_or("Open a project repository first")?;
                        repository.add_source(&path)?;
                        Ok((repository, None))
                    }
                    SourceAction::Convert(id) => {
                        let mut repository = repository.ok_or("Open a project repository first")?;
                        repository.convert_dwg(
                            &id,
                            &converter,
                            std::time::Duration::from_secs(300),
                        )?;
                        Ok((repository, None))
                    }
                    SourceAction::Reject(id, reason) => {
                        let mut repository = repository.ok_or("Open a project repository first")?;
                        repository.reject_source(&id, &reason)?;
                        Ok((repository, None))
                    }
                    SourceAction::Confirm(id, unit) => {
                        let mut repository = repository.ok_or("Open a project repository first")?;
                        repository.confirm_unit(&id, unit)?;
                        Ok((repository, None))
                    }
                    SourceAction::PreviewTerrain(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let project = snapshot.ok_or("Missing model snapshot")?;
                        let proposal = repository.preview_terrain(
                            &project.site,
                            &id,
                            terrain_placement.ok_or("Choose terrain placement explicitly")?,
                            terrain_name,
                        )?;
                        let metrics = proposal.terrain.geometry.metrics()?;
                        terrain = Some(super::terrain_ui::TerrainPreview {
                            proposal,
                            base,
                            metrics,
                        });
                        Ok((repository, None))
                    }
                    SourceAction::CommitTerrain(preview) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        terrain_ready = Some(repository.guard_terrain_commit(&preview.proposal)?);
                        terrain = Some(preview);
                        Ok((repository, None))
                    }
                    SourceAction::PreviewEvidence(state) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let project = snapshot.ok_or("Missing model snapshot")?;
                        let guard = repository.guard_evidence_commit(&state.draft.quote)?;
                        let next = crate::evidence::apply(
                            &project,
                            &crate::evidence::EvidenceEdit::Put {
                                draft: state.draft.clone(),
                                expected_previous: state.expected.clone(),
                            },
                        )?;
                        let prepared = next
                            .notes
                            .iter()
                            .find(|n| n.id == state.draft.id)
                            .cloned()
                            .ok_or("Missing prepared source note")?;
                        drop(guard);
                        evidence = Some(super::evidence_ui::EvidencePreview { state, prepared });
                        Ok((repository, None))
                    }
                    SourceAction::CommitEvidence(preview) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        evidence_ready =
                            Some(repository.guard_evidence_commit(&preview.state.draft.quote)?);
                        evidence = Some(preview);
                        Ok((repository, None))
                    }
                    SourceAction::InspectReviews(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        reviews = Some((id.clone(), repository.source_reviews(Some(&id))?));
                        Ok((repository, None))
                    }
                    SourceAction::RecordReview(request) => {
                        let mut repository = repository.ok_or("Open a project repository first")?;
                        let review = repository
                            .record_source_review(&request.operation_id, request.draft)?;
                        reviews = Some((
                            review.draft.subject_id.clone(),
                            repository.source_reviews(Some(&review.draft.subject_id))?,
                        ));
                        Ok((repository, None))
                    }
                    SourceAction::ReviewOperation(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        review_operation = Some((id.clone(), repository.review_operation(&id)?));
                        Ok((repository, None))
                    }
                    SourceAction::InspectProgram(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        workbook = Some((id.clone(), repository.inspect_program(&id)?));
                        Ok((repository, None))
                    }
                    SourceAction::PreviewProgram(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let (mapping, binding, name, expected) =
                            program_request.ok_or("Missing program mapping")?;
                        let proposal =
                            repository.preview_program(&id, binding, name, mapping, expected)?;
                        program = Some(super::program_ui::ProgramPreview { proposal, base });
                        Ok((repository, None))
                    }
                    SourceAction::CommitProgram(preview) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        program_ready = Some(repository.guard_program_commit(&preview.proposal)?);
                        program = Some(preview);
                        Ok((repository, None))
                    }
                    SourceAction::ListBoundaryEntities(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        entities = Some((id.clone(), repository.boundary_entities(&id)?));
                        Ok((repository, None))
                    }
                    SourceAction::PreviewBoundary(id) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let proposal = repository.preview_reference_region(
                            &snapshot.ok_or("Missing model snapshot")?,
                            &id,
                            &handle,
                            placement.ok_or("Choose boundary placement explicitly")?,
                            name,
                            story,
                            layer,
                            width_policy,
                            role,
                        )?;
                        boundary = Some(BoundaryPreview {
                            proposal,
                            base,
                            role,
                        });
                        Ok((repository, None))
                    }
                    SourceAction::CommitBoundary(preview) => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        ready = Some(repository.guard_boundary_commit(&preview.proposal)?);
                        boundary = Some(preview);
                        Ok((repository, None))
                    }
                    SourceAction::OpenModel => {
                        let repository = repository.ok_or("Open a project repository first")?;
                        let path = repository.working_model_copy()?;
                        Ok((repository, Some(path)))
                    }
                })();
                let output = output.and_then(|(repository, path)| {
                    let receipt = repository.latest_conversion()?;
                    Ok(SourceResult {
                        repository,
                        reviews,
                        review_operation,
                        model_path: path,
                        receipt,
                        boundary,
                        ready,
                        terrain,
                        terrain_ready,
                        workbook,
                        evidence,
                        evidence_ready,
                        rule_original,
                        rule_preview,
                        rule_ready,
                        rule_commit,
                        program,
                        program_ready,
                        entities,
                        conditions,
                        checks,
                        checks_read,
                    })
                });
                let _ = sender.send(output);
            }) {
            Ok(_) => {
                self.source_ui.error.clear();
                self.source_ui.pending = Some(SourceJob {
                    document_id,
                    session,
                    restoring,
                    generation,
                    result,
                });
            }
            Err(error) => self.source_ui.error = error.to_string(),
        }
    }
    pub(super) fn poll_source_job(&mut self) {
        self.try_restore_source_connection();
        let Some(job) = &self.source_ui.pending else {
            return;
        };
        let output = match job.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(
                "Source job stopped without a result; inspect the project folder before retrying"
                    .into(),
            ),
        };
        let job = self.source_ui.pending.take().expect("pending job");
        if job.restoring
            && job.session == self.host.document.editor.document_session()
            && job.generation.as_deref() != self.host.document.editor.file_generation()
        {
            // A successful save queued a new read with its new fingerprint. This
            // is a discarded old read, never a replay of a model/source write.
            self.try_restore_source_connection();
            return;
        }
        if job.document_id != self.host.document.editor.project_identity()
            || job.session != self.host.document.editor.document_session()
        {
            self.source_ui.error="Document changed during source job; reopen the original project folder to inspect its result".into();
            return;
        }
        if job.restoring && !self.host.document.editor.file_generation_matches_disk() {
            self.source_ui.error = "Saved project changed; reconnect its source folder".into();
            return;
        }
        match output {
            Ok(SourceResult {
                repository,
                reviews,
                review_operation,
                model_path,
                receipt,
                boundary,
                ready,
                terrain,
                terrain_ready,
                workbook,
                evidence,
                evidence_ready,
                rule_original,
                rule_preview,
                mut rule_ready,
                rule_commit,
                program,
                program_ready,
                entities,
                conditions,
                checks,
                checks_read,
            }) => {
                if let Some(view) = checks_read {
                    self.accept_checks_read(view);
                    return;
                }

                if let Some(job) = checks {
                    self.accept_checks_job(job);
                }
                if let Some(job) = conditions {
                    self.accept_conditions_job(job);
                }
                if let Some(original) = rule_original {
                    if self.rules_ui.open
                        && self
                            .host
                            .document
                            .editor
                            .project
                            .rules
                            .source(&self.rules_ui.source_id)
                            .is_some_and(|s| s.original_sha256 == original.source_sha256)
                    {
                        if let Err(error) = self.rules_ui.accept_original(original) {
                            self.source_ui.error = error;
                        }
                    }
                }
                if let Some(preview) = rule_preview {
                    if preview.base != self.host.context() {
                        self.rules_ui.preview = None;
                        self.rules_ui.request = None;
                        self.source_ui.error = "Model changed; preview the rule edit again".into();
                    } else if !(self.rules_ui.open
                        || (super::rule_register_ui::register_edit(&preview.edit)
                            && self.rules_ui.register.open))
                        || self.rules_ui.request.as_ref() != Some(&preview)
                    {
                        self.source_ui.error =
                            "Rule edit cancelled or draft changed; preview again".into();
                    } else if rule_commit {
                        if self.rules_ui.preview.as_ref() != Some(&preview) {
                            self.source_ui.error =
                                "Rule edit cancelled or draft changed; preview again".into();
                        } else if let Some(admission) = rule_ready.take() {
                            self.apply_rule_edit(preview, admission);
                        }
                    } else {
                        self.rules_ui.preview = Some(preview);
                    }
                }
                drop(rule_ready);
                if let Some((subject, records)) = reviews {
                    self.source_ui.review.accept(subject, records);
                }
                if let Some((id, result)) = review_operation {
                    self.source_ui.review.accept_operation(id, result);
                }
                if let Some(preview) = evidence {
                    if preview.state.base != self.host.context() {
                        self.source_ui.error =
                            "Model changed; preview the source note again".into();
                    } else if evidence_ready.is_some() {
                        if !self.evidence_ui.open
                            || self
                                .evidence_ui
                                .preview
                                .as_ref()
                                .is_none_or(|p| p.state != preview.state)
                        {
                            self.source_ui.error =
                                "Source note commit cancelled; preview retained".into();
                        } else {
                            self.apply_evidence_edit(
                                preview.state.base,
                                crate::evidence::EvidenceEdit::Put {
                                    draft: preview.state.draft.clone(),
                                    expected_previous: preview.state.expected.clone(),
                                },
                            );
                        }
                    } else if self.evidence_ui.draft.as_ref() != Some(&preview.state) {
                        self.source_ui.error = "Source note draft changed; preview again".into();
                    } else {
                        self.evidence_ui.preview = Some(preview);
                    }
                }
                drop(evidence_ready);
                if let Some(preview) = terrain {
                    if preview.base != self.host.context() {
                        self.source_ui.error = "Model changed; preview the terrain again".into();
                    } else if terrain_ready.is_some() {
                        if !self.terrain_ui.open {
                            self.source_ui.error =
                                "Terrain import cancelled; preview retained".into();
                        } else if self.menu_gated() || self.workbench_pointer_active() {
                            self.source_ui.error =
                                "Finish the active interaction before importing terrain".into();
                        } else {
                            match self.dispatch_project_preparation(
                                preview.base,
                                crate::application_host::HostCommand::Apply {
                                    request_id: crate::storage::new_document_identity(),
                                    commands: vec![crate::command::ModelCommand::Terrain {
                                        edit: crate::site::terrain::Edit::Import {
                                            proposal: preview.proposal.clone(),
                                        },
                                    }],
                                },
                            ) {
                                Ok(_) => {
                                    self.terrain_ui.preview = None;
                                    self.terrain_ui.metrics_cache =
                                        Some((self.host.context(), preview.metrics.clone()));
                                    self.fit();
                                }
                                Err(e) => self.source_ui.error = e,
                            }
                        }
                    } else {
                        self.terrain_ui.preview = Some(preview);
                    }
                }
                drop(terrain_ready);
                if let Some((source, book)) = workbook {
                    self.program_ui.accept_workbook(source, book);
                }
                if let Some(preview) = program {
                    if preview.base != self.host.context() {
                        self.source_ui.error = "Model changed; preview the program again".into();
                    } else if program_ready.is_some() {
                        if !self.program_ui.open {
                            self.source_ui.error =
                                "Program import cancelled; preview retained".into();
                        } else if self.menu_gated() || self.workbench_pointer_active() {
                            self.source_ui.error =
                                "Finish the active interaction before importing requirements"
                                    .into();
                        } else {
                            match self.dispatch_project_preparation(
                                preview.base,
                                crate::application_host::HostCommand::Apply {
                                    request_id: crate::storage::new_document_identity(),
                                    commands: vec![crate::command::ModelCommand::Program {
                                        edit: crate::program::ProgramEdit::Import {
                                            proposal: preview.proposal.clone(),
                                        },
                                    }],
                                },
                            ) {
                                Ok(_) => {
                                    self.program_ui.target =
                                        Some(preview.proposal.sheet.id.clone());
                                    self.program_ui.preview = None;
                                }
                                Err(error) => self.source_ui.error = error,
                            }
                        }
                    } else {
                        self.program_ui.preview = Some(preview);
                    }
                }
                drop(program_ready);
                if let Some(entities) = entities {
                    self.source_ui.boundary_entities = Some(entities);
                }

                if let Some(preview) = boundary {
                    if preview.role != self.source_ui.boundary_role {
                        self.source_ui.error = "Outline role changed; preview again".into();
                    } else if preview.base != self.host.context() {
                        self.source_ui.error = "Model changed; preview the boundary again".into();
                    } else if ready.is_some() {
                        if !self.source_ui.open {
                            self.source_ui.error =
                                "Boundary import cancelled; preview retained".into();
                        } else if self.menu_gated() || self.workbench_pointer_active() {
                            self.source_ui.error =
                                "Finish the active interaction before importing a boundary".into();
                        } else {
                            let result = self.dispatch_project_preparation(
                                preview.base,
                                crate::application_host::HostCommand::Apply {
                                    request_id: crate::storage::new_document_identity(),
                                    commands: vec![preview.command()],
                                },
                            );
                            match result {
                                Ok(_) => {
                                    self.source_ui.boundary_preview = None;
                                    if let Some(boundary) =
                                        preview.role.get(&self.host.document.editor.project.site)
                                    {
                                        let id = boundary.element_id;
                                        self.focus_imported_reference(id);
                                    }
                                }
                                Err(error) => self.source_ui.error = error,
                            }
                        }
                    } else {
                        self.source_ui.boundary_preview = Some(preview);
                    }
                }
                drop(ready);

                self.source_ui.receipt = receipt;
                if let Some(path) = model_path {
                    match storage::FileVersion::read(&path)
                        .and_then(|v| v.token().ok_or("Missing saved model fingerprint".into()))
                        .and_then(|version| {
                            crate::project_repository::connection::Connection::for_saved(
                                &path,
                                &repository.manifest.document_id,
                                &version,
                                &repository,
                            )
                            .map(|connection| (connection, version))
                        }) {
                        Ok((connection, version)) => {
                            if let Some(directory) = &self.source_ui.connections_directory
                                && let Err(error) = connection.rebind(
                                    directory,
                                    &path,
                                    &repository.manifest.document_id,
                                    &version,
                                )
                            {
                                self.source_ui.error = format!(
                                    "{}: {}",
                                    self.language
                                        .translate("Source folder connection was not remembered"),
                                    error
                                );
                            }
                            self.source_ui.owned_model_connection = Some(connection);
                        }
                        Err(error) => self.source_ui.error = error,
                    }
                    if self.menu_gated() {
                        self.source_ui.error =
                            "Model copy created; close the active dialog before opening it".into();
                    } else {
                        self.file_dialog_path = Some(path.display().to_string());
                        self.file_action = Some(FileAction::Open);
                        self.source_ui.open = false;
                    }
                }
                self.source_ui.root = repository.root().display().to_string();
                self.source_ui.unit_choices.clear();
                self.source_ui.repository = Some(repository);
                if !job.restoring {
                    self.remember_source_connection(false);
                }
            }
            Err(error) => {
                if self.checks_ui.read_pending.take().is_some() {
                    self.checks_ui.read_error = error.clone();
                }
                if let Some((id, _)) = self.checks_ui.pending.take() {
                    self.invalidate_checks_original_read();
                    if self.process_ui.preview.as_ref().is_some_and(|p| p.id == id) {
                        self.process_ui.error = error.clone();
                    }
                    self.checks_ui.error = error.clone();
                }
                self.source_ui.error = if job.restoring {
                    format!("{}: {}", self.language.translate("Source folder unavailable; continue designing or reconnect it in Project conditions"), error)
                } else {
                    error
                };
                if job.restoring {
                    self.host.document.editor.status = self.source_ui.error.clone();
                }
            }
        }
    }
    pub(super) fn project_sources_ui(&mut self, ctx: &Context) {
        self.poll_source_job();
        if self.source_ui.pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !self.source_ui.open || self.project_conditions_ui.open {
            return;
        }
        let language = self.language;
        let mut open = true;
        let mut action = None;
        egui::Window::new(language.translate("Project sources"))
            .id(Id::new("project_sources"))
            .open(&mut open)
            .default_width(650.)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                self.project_sources_contents(ui, language, &mut action);
            });
        self.source_ui.open = open;
        if let Some(action) = action {
            self.start_source_action(action);
        }
    }
    pub(super) fn project_sources_contents(
        &mut self,
        ui: &mut Ui,
        language: UiLanguage,
        action: &mut Option<SourceAction>,
    ) {
        ui.label(language.translate("Sources are copied without changing the model. Save a checkpoint to update the stored model."));
        ui.add_enabled_ui(self.source_ui.pending.is_none(),|ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label(language.translate("Project folder"));
                        ui.text_edit_singleline(&mut self.source_ui.root).labelled_by(label.id);
                        if ui.button(language.translate("Browse folder")).clicked() && let Some(path)=rfd::FileDialog::new().pick_folder() { self.source_ui.root=path.display().to_string(); }
                    });
                    ui.horizontal(|ui| {
                        if ui.button(language.translate("Create from current model")).clicked() { *action=Some(SourceAction::Create); }
                        if ui.button(language.translate("Open project folder")).clicked() { *action=Some(SourceAction::Open); }
                    });
                    if let Some(repository)=&self.source_ui.repository {
                        ui.separator();
                        ui.monospace(repository.root().display().to_string());
                        ui.label(language.translate("Copy the stored snapshot only; checkpoint first to include unsaved model changes"));
                        ui.horizontal(|ui|{ui.label(language.translate("New copy folder"));ui.text_edit_singleline(&mut self.source_ui.copy_root);if ui.button(language.translate("Copy stored project")).clicked(){*action=Some(SourceAction::CopyStored(self.source_ui.copy_root.clone()));}});
                        let bound=repository.manifest.document_id==self.host.document.editor.project_identity();
                        if !bound { ui.colored_label(Color32::YELLOW,language.translate("Connect sources for the current project to use source-backed conditions. You can continue designing without them.")); }
                        ui.label(format!("{}: {}",language.translate("Stored model revision"),repository.manifest.model_revision));
                        if ui.add_enabled(bound,egui::Button::new(language.translate("Save model checkpoint"))).clicked() { *action=Some(SourceAction::Checkpoint); }
                        if ui.button(language.translate("Open stored model…")).clicked() { *action=Some(SourceAction::OpenModel); }
                        ui.add_enabled_ui(bound,|ui| {
                            ui.horizontal(|ui| {
                                let label = ui.label(language.translate("Source file"));
                                ui.text_edit_singleline(&mut self.source_ui.source_path).labelled_by(label.id);
                                if ui.button(language.translate("Browse source")).clicked() && let Some(path)=rfd::FileDialog::new().pick_file() { self.source_ui.source_path=path.display().to_string(); }
                                if ui.button(language.translate("Copy source")).clicked() { *action=Some(SourceAction::Add); }
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.label(language.translate("Local dwg2dxf executable"));
                            ui.text_edit_singleline(&mut self.source_ui.converter_path);
                            if ui.button(language.translate("Choose converter")).clicked() && let Some(path)=rfd::FileDialog::new().pick_file() {self.source_ui.converter_path=path.display().to_string();}
                        });
                        ui.label(language.translate("Conversion coverage is unverified. Review the receipt before using derived geometry."));
                        if let Some(receipt)=&self.source_ui.receipt {
                            ui.label(language.translate(if receipt.status=="converted_unverified" {"Converted; coverage unverified"} else {"Conversion failed"}));
                            if let Some(error)=&receipt.error {ui.colored_label(Color32::LIGHT_RED,error);}
                            if let Some(capture)=&receipt.capture {ui.collapsing(language.translate("Conversion diagnostics"),|ui| {
                                ui.label(language.translate("Preview only. Each stored stream is limited to 64 KiB; byte counts and truncation are recorded in the receipt."));
                                ui.label(format!("stdout: {} / {} bytes; stderr: {} / {} bytes",capture.stdout.bytes.len(),capture.stdout.total_bytes,capture.stderr.bytes.len(),capture.stderr.total_bytes));
                                ui.monospace(String::from_utf8_lossy(&capture.stdout.bytes).chars().take(750).collect::<String>());
                                ui.monospace(String::from_utf8_lossy(&capture.stderr.bytes).chars().take(1500).collect::<String>());
                            });}
                        }
                        if let Some(item)=repository.manifest.conversions.last() {
                            ui.label(format!("{}: {}",language.translate("Latest conversion receipt"),item.path));
                        }
                        ui.horizontal(|ui| {ui.label(language.translate("Reason for excluding a source"));ui.text_edit_singleline(&mut self.source_ui.rejection_reason);});
                        if ui.button(language.translate("Functional program")).clicked(){self.project_conditions_ui.navigation=Some(ConditionsGroup::Requirements);}
                        let old_role=self.source_ui.boundary_role;
                        ui.horizontal_wrapped(|ui|{ui.label(language.translate("Use source outline as"));ui.selectable_value(&mut self.source_ui.boundary_role,crate::site::boundary::Role::Site,language.translate("Site boundary"));ui.selectable_value(&mut self.source_ui.boundary_role,crate::site::boundary::Role::ControlRegion,language.translate("Declared control region"));});
                        if old_role!=self.source_ui.boundary_role{self.source_ui.boundary_preview=None;if self.source_ui.boundary_name==language.translate("Site boundary") || self.source_ui.boundary_name==language.translate("Declared control region"){self.source_ui.boundary_name.clear();}}
                        if self.source_ui.boundary_name.is_empty(){self.source_ui.boundary_name=language.translate(if self.source_ui.boundary_role==crate::site::boundary::Role::Site{"Site boundary"}else{"Declared control region"}).into();}
                        egui::CollapsingHeader::new(language.translate("Boundary import settings")).default_open(true).show(ui,|ui|{
                            ui.horizontal(|ui|{ui.label(language.translate("Filter boundary layer or handle"));let response=ui.text_edit_singleline(&mut self.source_ui.boundary_filter);#[cfg(test)]ui.ctx().data_mut(|d|d.insert_temp(Id::new("boundary_filter_field"),response.rect));let _=response;});
                            if let Some((source,entities))=&self.source_ui.boundary_entities {
                                let filter=self.source_ui.boundary_filter.to_lowercase();let matching=entities.iter().filter(|r|r.handle.as_deref().unwrap_or("").to_lowercase().contains(&filter)||r.layer.as_deref().unwrap_or("").to_lowercase().contains(&filter)).collect::<Vec<_>>();
                                ui.label(format!("{}: {}",language.translate("Candidate records"),matching.len()));
                                for record in matching.iter().take(100){let handle=record.handle.as_deref().unwrap_or("");if ui.selectable_label(self.source_ui.boundary_handle==handle,format!("{} · {}",record.layer.as_deref().unwrap_or("—"),handle)).clicked(){self.source_ui.boundary_handle=handle.into();*action=Some(SourceAction::PreviewBoundary(source.clone()));}}
                                if matching.len()>100{ui.label(language.translate("Only the first 100 matching records are shown; refine the filter."));}
                                ui.label(language.translate("Candidates are not validated outlines until preview succeeds."));
                            }
                            ui.horizontal(|ui|{ui.label(language.translate("DXF entity handle"));ui.text_edit_singleline(&mut self.source_ui.boundary_handle);});
                            ui.horizontal(|ui|{ui.label(language.translate("Boundary name"));ui.text_edit_singleline(&mut self.source_ui.boundary_name);});
                            if self.source_ui.boundary_layer==0{self.source_ui.boundary_layer=self.host.document.editor.project.layers.layers.iter().find(|l|!self.host.document.editor.project.layers.is_locked(l.id)).map_or(0,|l|l.id);}
                            ui.add_enabled_ui(self.source_ui.boundary_role.get(&self.host.document.editor.project.site).is_none(),|ui|{egui::ComboBox::from_id_salt("boundary_layer").selected_text(format!("{} {}",language.translate("Target layer"),self.source_ui.boundary_layer)).show_ui(ui,|ui|{for layer in &self.host.document.editor.project.layers.layers{if !self.host.document.editor.project.layers.is_locked(layer.id){ui.selectable_value(&mut self.source_ui.boundary_layer,layer.id,&layer.name);}}});});
                            use crate::project_repository::boundary::BoundaryPlacement;
                            egui::ComboBox::from_id_salt("boundary_placement").selected_text(language.translate(match self.source_ui.boundary_placement{None=>"Choose boundary placement explicitly",Some(BoundaryPlacement::ModelAxes)=>"Drawing axes to model axes",Some(BoundaryPlacement::ProjectedReference)=>"Projected EN through site reference"})).show_ui(ui,|ui|{ui.selectable_value(&mut self.source_ui.boundary_placement,Some(BoundaryPlacement::ModelAxes),language.translate("Drawing axes to model axes"));ui.selectable_value(&mut self.source_ui.boundary_placement,Some(BoundaryPlacement::ProjectedReference),language.translate("Projected EN through site reference"));});
                            ui.checkbox(&mut self.source_ui.boundary_centerline,language.translate("Use vertex centerline; omit constant width explicitly"));
                            ui.label(language.translate("Use Preview boundary on a DXF below. Units must be explicitly confirmed. This does not approve the legal site boundary."));
                        });
                        if let Some(preview)=&self.source_ui.boundary_preview {
                            ui.separator();ui.strong(format!("{} · {}",language.translate("Boundary preview"),preview.proposal.origin.entity_handle));
                            if let Some(width)=preview.proposal.origin.centerline_width_native {ui.colored_label(Color32::YELLOW,format!("{}: {} {}",language.translate("Centerline only; omitted constant width"),width,preview.proposal.origin.unit.symbol()));}
                            let points=preview.proposal.outline_m.iter().map(|p|Point::new(p[0],p[1])).collect::<Vec<_>>();
                            ui.label(format!("{}: {:.4} m² · {} {}",language.translate("Area"),crate::slab_geometry::area(&points),language.translate("Story"),preview.proposal.story));
                            let (rect,_)=ui.allocate_exact_size(vec2(340.,180.),Sense::hover());
                            let min=[points.iter().map(|p|p.x).fold(f64::INFINITY,f64::min),points.iter().map(|p|p.y).fold(f64::INFINITY,f64::min)];let max=[points.iter().map(|p|p.x).fold(f64::NEG_INFINITY,f64::max),points.iter().map(|p|p.y).fold(f64::NEG_INFINITY,f64::max)];let scale=((rect.width()-16.) as f64/(max[0]-min[0]).max(0.01)).min((rect.height()-16.) as f64/(max[1]-min[1]).max(0.01));
                            let path=points.iter().map(|p|pos2(rect.left() as f64+8.+(p.x-min[0])*scale,rect.bottom() as f64-8.-(p.y-min[1])*scale)).collect();ui.painter().add(egui::Shape::closed_line(path,Stroke::new(2.,ACCENT)));
                            if ui.button(language.translate(if preview.role==crate::site::boundary::Role::Site{"Use preview as site boundary"}else{"Use preview as control region"})).clicked(){*action=Some(SourceAction::CommitBoundary(preview.clone()));}
                            if ui.button(language.translate("Discard boundary preview")).clicked(){self.source_ui.boundary_preview=None;}
                        }
                        ui.label(language.translate("Header units are declarations. Coordinate reference, true north, elevation and survey approval remain unknown."));
                        egui::ScrollArea::vertical().max_height(380.).show(ui,|ui| {
                            for source in &repository.manifest.sources {
                                ui.separator();
                                ui.strong(source.names.join(" / "));
                                ui.label(format!("{} · {} bytes",source.format,source.blob.bytes));
                                ui.monospace(&source.id);
                                if ui.button(language.translate("Quality reports")).clicked(){self.source_ui.review.choose(source.id.clone());*action=Some(SourceAction::InspectReviews(source.id.clone()));}
                                match &source.quality {
                                    crate::project_repository::SourceQuality::Unreviewed=>{ui.label(language.translate("Source quality unreviewed"));},
                                    crate::project_repository::SourceQuality::Rejected{reason}=>{ui.colored_label(Color32::LIGHT_RED,language.translate("Source rejected; do not use for geometry"));ui.label(reason);},
                                }
                                if ui.add_enabled(bound && !self.source_ui.rejection_reason.trim().is_empty(),egui::Button::new(language.translate("Mark source unusable"))).clicked() {*action=Some(SourceAction::Reject(source.id.clone(),self.source_ui.rejection_reason.clone()));}

                                if let Some(summary)=&source.summary {
                                    ui.label(format!("{}: {}",language.translate("Entity records"),summary.records));
                                    ui.label(format!("$INSUNITS: {:?}",summary.declared_units));
                                    for (kind,count) in &summary.by_type { ui.label(format!("{kind}: {count}")); }
                                    ui.label(format!("{}: {} / {} / {}",language.translate("Missing handles / duplicate handles / non-UTF8 labels"),summary.missing_handles,summary.duplicate_handles,summary.non_utf8_values));
                                }
                                if matches!(source.format.as_str(),"csv"|"xlsx"|"docx") && ui.button(language.translate("Inspect functional worksheet")).clicked(){self.project_conditions_ui.navigation=Some(ConditionsGroup::Requirements);*action=Some(SourceAction::InspectProgram(source.id.clone()));}
                                if source.format=="obj" && ui.button(language.translate("Terrain surface")).clicked(){self.project_conditions_ui.navigation=Some(ConditionsGroup::Site);}
                                if source.format=="dxf" && ui.button(language.translate("Find boundary records")).clicked(){*action=Some(SourceAction::ListBoundaryEntities(source.id.clone()));}
                                if source.format=="dxf" && ui.add_enabled(bound && !matches!(source.quality,crate::project_repository::SourceQuality::Rejected{..}),egui::Button::new(language.translate("Preview boundary"))).clicked(){*action=Some(SourceAction::PreviewBoundary(source.id.clone()));}
                                if source.format=="dwg" && ui.add_enabled(bound,egui::Button::new(language.translate("Convert stored DWG"))).clicked() { *action=Some(SourceAction::Convert(source.id.clone())); }
                                if let Some(error)=&source.inventory_error { ui.colored_label(Color32::YELLOW,language.translate(error)); }
                                let unit=self.source_ui.unit_choices.entry(source.id.clone()).or_insert(source.confirmed_unit);
                                ui.add_enabled_ui(bound,|ui| {
                                    ui.horizontal(|ui| {
                                        egui::ComboBox::from_id_salt(("source_unit",&source.id)).selected_text(unit.map_or(language.translate("Units unconfirmed"),LengthUnit::symbol)).show_ui(ui,|ui| {
                                            ui.selectable_value(unit,None,language.translate("Units unconfirmed"));
                                            for choice in LengthUnit::ALL { ui.selectable_value(unit,Some(choice),choice.symbol()); }
                                        });
                                        if ui.button(language.translate("Confirm units")).clicked() { *action=Some(SourceAction::Confirm(source.id.clone(),*unit)); }
                                        ui.label(format!("{}: {}",language.translate("Confirmed units"),source.confirmed_unit.map_or(language.translate("Units unconfirmed"),LengthUnit::symbol)));
                                    });
                                });
                            }
                        });
                    }
                });
        if self.source_ui.pending.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(language.translate("Inspecting and copying sources…"));
            });
        }
        if !self.source_ui.error.is_empty() {
            ui.colored_label(
                Color32::LIGHT_RED,
                language.translate(&self.source_ui.error),
            );
        }
    }
}

#[cfg(test)]
mod connection_boundaries {
    use super::*;
    fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oa-source-state-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&path).unwrap();
        path.canonicalize().unwrap()
    }
    fn finish(app: &mut AtelierApp) {
        let start = std::time::Instant::now();
        while app.source_ui.pending_for_ui() {
            app.poll_source_job();
            assert!(start.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn explicit_stored_model_copy_carries_its_sources_without_a_personal_profile() {
        let root = root();
        let mut app = AtelierApp::from_renderer(None, "stored source copy test".into());
        let repository =
            Repository::create_for_editor(&root.join("repository"), &app.host.document.editor)
                .unwrap();
        app.source_ui.root = repository.root().display().to_string();
        app.source_ui.repository = Some(repository.clone());
        app.start_source_action(SourceAction::OpenModel);
        finish(&mut app);
        let path = PathBuf::from(app.file_dialog_path.clone().unwrap());
        let model = storage::load(&path).unwrap();
        app.host.document.editor = Editor::from_stored_document(&path).unwrap();
        app.file_baseline = Some((path.clone(), storage::FileVersion::read(&path).unwrap()));
        app.path = path.display().to_string();
        app.path_confirmed = true;
        app.source_document_opened();
        finish(&mut app);
        assert!(app.source_ui.connections_directory.is_none());
        assert_eq!(
            app.source_ui.repository.as_ref().unwrap().root(),
            repository.root()
        );
        assert!(app.host.document.editor.project == model);
        assert_eq!(
            std::fs::read_dir(repository.root().join("working"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|p| p.path().extension().is_some_and(|x| x == "json"))
                .count(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn source_result_cannot_attach_to_new_session_with_the_same_document_identity() {
        let root = root();
        let mut app = AtelierApp::from_renderer(None, "source session test".into());
        let id = app.host.document.editor.project_identity().to_owned();
        app.source_ui.root = root.join("old-source-job").display().to_string();
        app.start_source_action(SourceAction::Create);
        assert!(app.source_ui.pending.is_some());
        app.host.document.editor = Editor::new(Project::default());
        app.host
            .document
            .editor
            .adopt_document_identity(id)
            .unwrap();
        let model = app.host.document.editor.project.clone();
        finish(&mut app);
        assert!(app.source_ui.repository.is_none());
        assert!(app.source_ui.error.contains("Document changed"));
        assert!(app.host.document.editor.project == model);
        assert!(
            root.join("old-source-job/project.json").is_file(),
            "original job result remains inspectable, never replayed"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_during_source_restore_retains_grant_and_discards_only_old_read_result() {
        let root = root();
        let path = root.join("current.json");
        let mut app = AtelierApp::from_renderer(None, "source save race test".into());
        let id = app.host.document.editor.project_identity().to_owned();
        storage::save_document(&app.host.document.editor.project, &path, &id).unwrap();
        app.host.document.editor = Editor::from_stored_document(&path).unwrap();
        app.path = path.display().to_string();
        app.path_confirmed = true;
        app.file_baseline = Some((path.clone(), storage::FileVersion::read(&path).unwrap()));
        app.initialize_source_connections(&root.join("profile"));
        let repo =
            Repository::create_for_editor(&root.join("repository"), &app.host.document.editor)
                .unwrap();
        crate::project_repository::connection::remember(
            app.source_ui.connections_directory.as_ref().unwrap(),
            &path,
            &id,
            app.host.document.editor.file_generation().unwrap(),
            &repo,
        )
        .unwrap();
        app.source_document_opened();
        assert!(app.source_ui.pending.as_ref().unwrap().restoring);
        let old_generation = app
            .host
            .document
            .editor
            .file_generation()
            .unwrap()
            .to_owned();
        let model = app.host.document.editor.project.clone();
        app.save_project().unwrap();
        assert_ne!(
            app.host.document.editor.file_generation().unwrap(),
            old_generation
        );
        finish(&mut app);
        assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
        assert_eq!(
            app.source_ui.repository.as_ref().unwrap().root(),
            repo.root()
        );
        assert!(app.host.document.editor.project == model);
        let stored = crate::project_repository::connection::find(
            app.source_ui.connections_directory.as_ref().unwrap(),
            &path,
            &id,
            app.host.document.editor.file_generation().unwrap(),
        )
        .unwrap()
        .unwrap();
        assert!(stored.open().is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn connection_write_failure_reports_partial_save_and_preserves_successful_model() {
        let root = root();
        let path = root.join("saved.json");
        let mut app = AtelierApp::from_renderer(None, "source persistence failure test".into());
        app.path = path.display().to_string();
        let repo =
            Repository::create_for_editor(&root.join("repository"), &app.host.document.editor)
                .unwrap();
        app.source_ui.repository = Some(repo);
        let protected = root.join("not-a-directory");
        std::fs::write(&protected, b"preserve existing evidence").unwrap();
        app.source_ui.connections_directory = Some(protected.clone());
        let original = app.host.document.editor.project.clone();
        app.save_project().unwrap();
        assert!(storage::load(&path).unwrap() == original);
        assert!(
            app.host
                .document
                .editor
                .status
                .contains("Project saved; its source folder connection was not remembered")
        );
        assert_eq!(
            std::fs::read(protected).unwrap(),
            b"preserve existing evidence"
        );
        assert!(app.file_baseline.is_some());
        assert!(!app.host.document.editor.dirty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
