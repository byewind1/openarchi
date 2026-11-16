//! In-process application boundary over the existing Editor. No transport or second live model.
use crate::{
    command::{ModelCommand, ProjectDelta},
    editor::Editor,
    model::{Project, Shape},
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicU64, Ordering},
};
mod batch;
pub use batch::{
    BATCH_CHANGE_VERSION, BatchChange, ElementReference, ElementTemplate, NamedElement,
    ReferencedCommand,
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
const MAX_RECEIPTS: usize = 1024;

pub(crate) struct SessionState {
    id: u64,
    project_id: String,
    file_generation: Option<String>,
    baseline_path: Option<std::path::PathBuf>,
    receipts: HashMap<String, CommittedChange>,
}
struct CommittedChange {
    base_revision: u64,
    commands: Vec<ModelCommand>,
    batch_input: Option<[u8; 32]>,
    receipt: ChangeReceipt,
}
impl Default for SessionState {
    fn default() -> Self {
        Self {
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            project_id: crate::storage::new_document_identity(),
            file_generation: None,
            baseline_path: None,
            receipts: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ChangeError {
    pub code: &'static str,
    pub message: String,
    /// Proposed-element diagnostics; rejected IDs were never committed.
    pub diagnostics: Vec<crate::derived_geometry::GeometryDiagnostic>,
}
fn error(code: &'static str, message: impl Into<String>) -> ChangeError {
    ChangeError {
        code,
        message: message.into(),
        diagnostics: vec![],
    }
}

/// Immutable candidate, bound to this document incarnation and complete baseline.
/// Snapshot lifetime is controlled by the application caller; it is never authoritative.
pub struct PreparedChange {
    session: u64,
    base_revision: u64,
    before: Project,
    after: Project,
    commands: Vec<ModelCommand>,
    batch_input: Option<[u8; 32]>,
    resolved_references: BTreeMap<String, u64>,
    recovery_admission: bool,
    conditions_admission: bool,
    cited_admission: Option<std::sync::Weak<crate::project_repository::constraints::Admission>>,
}
impl PreparedChange {
    /// Re-validate the reviewed commands against the same model baseline, using
    /// a freshly read opaque source guard. Keep symbolic IDs and batch identity.
    pub(crate) fn recheck_cited(
        &self,
        editor: &Editor,
        guard: &crate::project_repository::constraints::ConstraintCommitGuard,
    ) -> Result<Self, ChangeError> {
        if !self.matches_baseline(editor) {
            return Err(error(
                "REVISION_CONFLICT",
                "Model changed after cited preview",
            ));
        }
        let mut fresh =
            editor.prepare_constraint_change(self.base_revision, self.commands.clone(), guard)?;
        if !fresh.after.same_model_state(&self.after) {
            return Err(error(
                "PREVIEW_CHANGED",
                "Rechecked result differs from the reviewed model",
            ));
        }
        fresh.batch_input = self.batch_input;
        fresh.resolved_references = self.resolved_references.clone();
        Ok(fresh)
    }
    pub(crate) fn commands(&self) -> &[ModelCommand] {
        &self.commands
    }
    pub(crate) fn needs_cited_recheck(&self) -> bool {
        self.cited_admission.is_some()
    }
    /// Also called before the durable coordinator writes its model intent.
    pub(crate) fn validate_cited_admission(&self, editor: &Editor) -> Result<(), ChangeError> {
        if let Some(lease) = &self.cited_admission {
            let guard = lease.upgrade().ok_or_else(|| {
                error(
                    "CITED_ORIGINAL_RECHECK_REQUIRED",
                    "Recheck cited originals before confirming this preview",
                )
            })?;
            guard
                .verify(editor, &self.commands)
                .map_err(|message| error("CITED_ORIGINAL_ADMISSION_MISMATCH", message))?;
        }
        Ok(())
    }
    pub(crate) fn only_constraint_commands(&self) -> bool {
        !self.commands.is_empty()
            && self
                .commands
                .iter()
                .all(|c| matches!(c, ModelCommand::Constraint { .. }))
    }
    /// Native standalone surface assignment admits only opaque surface changes
    /// on existing Zones. The full element and all project headers stay equal.
    pub(crate) fn only_zone_surface_commands(&self) -> bool {
        if self.commands.is_empty() || !self.commands.iter().all(|c| matches!(c, ModelCommand::UpdateElement { element } if matches!(element.shape, Shape::Zone { .. }))) { return false; }
        let delta = self.difference();
        !delta.elements.is_empty()
            && delta.before_header.is_none()
            && delta.after_header.is_none()
            && delta.before_order.is_none()
            && delta.after_order.is_none()
            && delta.elements.iter().all(|edit| {
                let (Some(before), Some(after)) = (&edit.before, &edit.after) else {
                    return false;
                };
                let mut expected = before.clone();
                let (Shape::Zone { spec: old }, Shape::Zone { spec: new }) =
                    (&mut expected.shape, &after.shape)
                else {
                    return false;
                };
                old.surface = new.surface.clone();
                &expected == after
            })
    }
    pub(crate) fn only_mass_space_command(&self) -> bool {
        self.commands.len() == 1 && matches!(self.commands[0], ModelCommand::MassSpaces { .. })
    }
    pub(crate) fn only_object_discussion_command(&self) -> bool {
        self.commands.len() == 1
            && matches!(
                &self.commands[0],
                ModelCommand::Evidence {
                    edit: crate::evidence::EvidenceEdit::RecordTopic { .. }
                        | crate::evidence::EvidenceEdit::RecordTopicReply { .. }
                        | crate::evidence::EvidenceEdit::SubmitTopicTask { .. }
                        | crate::evidence::EvidenceEdit::TopicTaskEvent { .. }
                }
            )
    }

    pub(crate) fn verified_conditions_command(&self) -> bool {
        self.conditions_admission
    }
    /// World-geometry impact includes elements whose relative parameters did
    /// not change but whose home story changed elevation. Deltas still retain
    /// only real parameter/header changes.
    pub fn affected_ids(&self) -> Vec<u64> {
        let mut ids = self.difference().affected_ids();
        for change in self.story_changes() {
            ids.extend(change.affected_ids);
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }
    pub fn story_changes(&self) -> Vec<StoryElevationChange> {
        self.before
            .stories
            .iter()
            .filter_map(|before| {
                let after = self.after.stories.iter().find(|s| s.id == before.id)?;
                if before.elevation == after.elevation {
                    return None;
                }
                let mut affected_ids: Vec<_> = self
                    .before
                    .elements
                    .iter()
                    .chain(&self.after.elements)
                    .filter(|e| e.story == before.id)
                    .map(|e| e.id)
                    .collect();
                affected_ids.sort_unstable();
                affected_ids.dedup();
                Some(StoryElevationChange {
                    story: before.id,
                    before_elevation_m: before.elevation,
                    after_elevation_m: after.elevation,
                    affected_ids,
                })
            })
            .collect()
    }
    pub(crate) fn belongs_to_session(&self, editor: &Editor) -> bool {
        self.session == editor.document_session()
    }
    pub(crate) fn matches_baseline(&self, editor: &Editor) -> bool {
        self.session == editor.document_session()
            && self.base_revision == editor.revision
            && self.before.same_model_state(&editor.project)
    }
    pub fn resolved_references(&self) -> &BTreeMap<String, u64> {
        &self.resolved_references
    }
    pub fn base_revision(&self) -> u64 {
        self.base_revision
    }
    pub fn project(&self) -> &Project {
        &self.after
    }
    /// Read-only declaration checks on this exact proposed geometry. Caller
    /// retains the same baseline/preview identity; nothing is run or saved.
    pub fn declared_check_previews(&self) -> Vec<crate::constraints::Preview> {
        crate::constraints::preview_affected(&self.before, &self.after, &self.affected_ids())
    }
    pub fn difference(&self) -> ProjectDelta {
        ProjectDelta::between(&self.before, &self.after)
    }
}

#[derive(Clone, Debug, serde::Serialize, schemars::JsonSchema)]
pub struct StoryElevationChange {
    pub story: u32,
    pub before_elevation_m: f64,
    pub after_elevation_m: f64,
    pub affected_ids: Vec<u64>,
}

fn site_flag_false(value: &bool) -> bool {
    !*value
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeReceipt {
    pub request_id: String,
    pub document_session: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub changed_ids: Vec<u64>,
    #[serde(default, skip_serializing_if = "site_flag_false")]
    pub design_process_changed: bool,
    #[serde(default, skip_serializing_if = "site_flag_false")]
    pub edit_state_changed: bool,
    #[serde(default, skip_serializing_if = "site_flag_false")]
    pub site_context_changed: bool,
    #[serde(default, skip_serializing_if = "site_flag_false")]
    pub program_changed: bool,
    #[serde(default, skip_serializing_if = "site_flag_false")]
    pub evidence_changed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rules_changed: bool,
    #[serde(default, skip_serializing_if = "site_flag_false")]
    pub drawings_changed: bool,
    pub resolved_references: BTreeMap<String, u64>,
}

impl Editor {
    /// Diagnostic/reference execution control; not a project preference.
    pub fn set_source_cache_enabled(&mut self, enabled: bool) {
        self.derivation_cache.get_mut().set_enabled(enabled);
    }
    /// A finite read scope over this Editor's single authoritative Project.
    pub fn with_geometry_read<T>(
        &self,
        read: impl FnOnce(&mut crate::derivation_cache::GeometryRead<'_>) -> T,
    ) -> Result<T, crate::derived_geometry::GeometryDiagnostic> {
        let mut cache = self.derivation_cache.try_borrow_mut().map_err(|_| {
            crate::derived_geometry::GeometryDiagnostic::error(
                0,
                "GEOMETRY_READ_BUSY",
                "cache",
                "Use the current geometry read scope for nested queries",
            )
        })?;
        let mut scope = cache.read(&self.project, self.revision, self.document_session())?;
        Ok(read(&mut scope))
    }
    pub fn geometry_source(
        &self,
        id: u64,
    ) -> Result<crate::derivation_cache::GeometryHandle, crate::derived_geometry::GeometryDiagnostic>
    {
        self.with_geometry_read(|scope| scope.source(id))?
    }

    pub fn geometry_batch(
        &self,
        ids: &[u64],
        plan_story: Option<u32>,
    ) -> Result<crate::derivation_cache::GeometryBatch, crate::derived_geometry::GeometryDiagnostic>
    {
        self.with_geometry_read(|read| read.batch(ids, plan_story))
    }
    pub fn pick_model(
        &self,
        camera: &crate::geometry::Camera,
        ndc: [f64; 2],
        aspect: f64,
        filter: crate::geometry::ModelViewFilter<'_>,
    ) -> Result<Option<u64>, crate::derived_geometry::GeometryDiagnostic> {
        self.with_geometry_read(|read| camera.pick_in_view_cached(read, ndc, aspect, filter))?
    }
    pub(crate) fn validate_geometry_snapshot(
        &self,
        project: &Project,
    ) -> crate::geometry_validation::GeometryReport {
        match self.derivation_cache.try_borrow_mut() {
            Ok(mut cache) => crate::geometry_validation::validate_cached(
                project,
                &mut cache,
                self.revision,
                self.document_session(),
            ),
            Err(_) => crate::geometry_validation::GeometryReport {
                input_revision: Some(self.revision),
                document_session: Some(self.document_session()),
                diagnostics: vec![crate::derived_geometry::GeometryDiagnostic::error(
                    0,
                    "GEOMETRY_READ_BUSY",
                    "cache",
                    "Close the active geometry read before validating a proposal",
                )],
                ..Default::default()
            },
        }
    }
    /// Read-only derived-geometry diagnostics, bound to this editor context.
    pub fn geometry_report(&self) -> crate::geometry_validation::GeometryReport {
        let mut report = self.project.validation_report();
        report.input_revision = Some(self.revision);
        report.document_session = Some(self.document_session());
        report
    }
    /// Plan-only curves with analytic length/bounds and sampled display path.
    pub fn polyline_geometry(
        &self,
        id: u64,
    ) -> Result<
        crate::polyline_geometry::DerivedPolyline,
        crate::derived_geometry::GeometryDiagnostic,
    > {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Polyline { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Polyline(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    "Polyline does not exist",
                )
            })?;
        let mut g = crate::polyline_geometry::derive(&self.project, e)?;
        g.identity.input_revision = Some(self.revision);
        g.identity.document_session = Some(self.document_session());
        Ok(g)
    }
    /// Read-only straight-flight body plus separate display handrails.
    pub fn stair_geometry(
        &self,
        id: u64,
    ) -> Result<crate::stair_geometry::DerivedStair, crate::derived_geometry::GeometryDiagnostic>
    {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Stair { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Stair(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    "Stair does not exist",
                )
            })?;
        let mut g = crate::stair_geometry::derive(&self.project, e)?;
        g.identity.input_revision = Some(self.revision);
        g.identity.document_session = Some(self.document_session());
        Ok(g)
    }
    /// A read-only geometry snapshot of this editor's actual document/revision.
    pub fn slab_geometry(
        &self,
        id: u64,
    ) -> Result<crate::slab_geometry::DerivedSlab, crate::slab_geometry::GeometryDiagnostic> {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Slab { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Slab(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| crate::slab_geometry::GeometryDiagnostic {
                code: "ELEMENT_NOT_FOUND".into(),
                severity: crate::slab_geometry::DiagnosticSeverity::Error,
                location_m: None,
                element_id: id,
                feature: "element".into(),
                message: format!("Element {id} does not exist"),
            })?;
        let mut geometry = crate::slab_geometry::derive(e, self.project.elevation(e.story))?;
        geometry.identity.input_revision = Some(self.revision);
        geometry.identity.document_session = Some(self.document_session());
        Ok(geometry)
    }

    /// Read-only roof result, bound to the same document session as slab queries.
    pub fn roof_geometry(
        &self,
        id: u64,
    ) -> Result<crate::roof_geometry::DerivedRoof, crate::derived_geometry::GeometryDiagnostic>
    {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Roof { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Roof(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    format!("Element {id} does not exist"),
                )
            })?;
        let mut geometry = crate::roof_geometry::derive(e, self.project.elevation(e.story))?;
        geometry.identity.input_revision = Some(self.revision);
        geometry.identity.document_session = Some(self.document_session());
        Ok(geometry)
    }

    pub fn beam_geometry(
        &self,
        id: u64,
    ) -> Result<crate::beam_geometry::DerivedBeam, crate::derived_geometry::GeometryDiagnostic>
    {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Beam { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Beam(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    format!("Element {id} does not exist"),
                )
            })?;
        let mut g = crate::beam_geometry::derive(&self.project, e)?;
        g.identity.input_revision = Some(self.revision);
        g.identity.document_session = Some(self.document_session());
        Ok(g)
    }

    pub fn column_geometry(
        &self,
        id: u64,
    ) -> Result<crate::column_geometry::DerivedColumn, crate::derived_geometry::GeometryDiagnostic>
    {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Column { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Column(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    format!("Element {id} does not exist"),
                )
            })?;
        let mut g = crate::column_geometry::derive(&self.project, e)?;
        g.identity.input_revision = Some(self.revision);
        g.identity.document_session = Some(self.document_session());
        Ok(g)
    }

    pub fn column_plan_geometry(
        &self,
        id: u64,
        story: u32,
    ) -> Result<
        crate::column_geometry::DerivedColumnPlan,
        crate::derived_geometry::GeometryDiagnostic,
    > {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Column { .. }))
        {
            return self.with_geometry_read(|read| read.column_plan(id, story))?;
        }
        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    format!("Element {id} does not exist"),
                )
            })?;
        let mut g = crate::column_geometry::derive_plan(&self.project, e, story)?;
        for identity in [&mut g.identity, &mut g.source_identity] {
            identity.input_revision = Some(self.revision);
            identity.document_session = Some(self.document_session());
        }
        Ok(g)
    }

    pub fn wall_geometry(
        &self,
        id: u64,
    ) -> Result<crate::wall_solid::DerivedWall, crate::derived_geometry::GeometryDiagnostic> {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Wall { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Wall(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;

                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    format!("Element {id} does not exist"),
                )
            })?;
        let mut g = crate::wall_solid::derive(&self.project, e)?;
        g.identity.input_revision = Some(self.revision);
        g.identity.document_session = Some(self.document_session());
        Ok(g)
    }

    pub fn opening_geometry(
        &self,
        id: u64,
    ) -> Result<crate::opening_geometry::DerivedOpening, crate::derived_geometry::GeometryDiagnostic>
    {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, crate::model::Shape::Opening { .. }))
        {
            let handle = self.geometry_source(id)?;
            if let crate::derivation_cache::GeometrySource::Opening(g) = handle.source.as_ref() {
                let mut result = g.clone();
                result.identity = handle.identity;
                result.host_identity.input_revision = Some(self.revision);
                result.host_identity.document_session = Some(self.document_session());
                return Ok(result);
            }
        }

        let e = self
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    id,
                    "ELEMENT_NOT_FOUND",
                    "element",
                    format!("Element {id} does not exist"),
                )
            })?;
        let mut g = crate::opening_geometry::derive(&self.project, e)?;
        for identity in [&mut g.identity, &mut g.host_identity] {
            identity.input_revision = Some(self.revision);
            identity.document_session = Some(self.document_session());
        }
        Ok(g)
    }

    pub fn from_stored_document(path: &std::path::Path) -> Result<Self, String> {
        let (project, version, id) = crate::storage::load_document(path)?;
        let mut editor = Self::new(project);
        if let Some(id) = id {
            editor.adopt_document_identity(id)?;
        }
        editor.bind_file_generation(version.token());
        editor.bind_file_path(Some(std::path::absolute(path).map_err(|e| e.to_string())?));
        Ok(editor)
    }
    pub(crate) fn bind_file_path(&mut self, path: Option<std::path::PathBuf>) {
        self.model_service.baseline_path = path;
    }
    pub(crate) fn file_generation_matches_disk(&self) -> bool {
        match (&self.model_service.baseline_path, self.file_generation()) {
            (Some(path), Some(generation)) => {
                crate::storage::FileVersion::read(path)
                    .ok()
                    .and_then(|v| v.token())
                    .as_deref()
                    == Some(generation)
            }
            _ => false,
        }
    }
    pub fn file_generation(&self) -> Option<&str> {
        self.model_service.file_generation.as_deref()
    }
    pub(crate) fn bind_file_generation(&mut self, generation: Option<String>) {
        self.model_service.file_generation = generation;
    }
    /// Trusted application recovery only; never accepts a remote arbitrary snapshot.
    pub(crate) fn prepare_recovery_snapshot(
        &self,
        expected_revision: u64,
        mut after: Project,
    ) -> Result<PreparedChange, ChangeError> {
        if expected_revision != self.revision {
            return Err(error("REVISION_CONFLICT", "Model changed before recovery"));
        }
        after
            .job_archive
            .check_project(self.project_identity())
            .map_err(|e| error("JOB_HISTORY_CONFLICT", e))?;
        after.job_archive = self
            .project
            .job_archive
            .merge(&after.job_archive)
            .map_err(|e| error("JOB_HISTORY_CONFLICT", e))?;
        after
            .validate()
            .map_err(|e| error("VALIDATION_FAILED", e))?;
        if after.same_model_state(&self.project) {
            return Err(error("NO_CHANGE", "Recovery image already present"));
        }
        Ok(PreparedChange {
            session: self.document_session(),
            base_revision: self.revision,
            before: self.project.clone(),
            after,
            commands: vec![ModelCommand::ChangeSet],
            batch_input: None,
            resolved_references: BTreeMap::new(),
            recovery_admission: true,
            conditions_admission: false,
            cited_admission: None,
        })
    }
    pub fn project_identity(&self) -> &str {
        &self.model_service.project_id
    }
    pub fn adopt_document_identity(&mut self, id: String) -> Result<(), String> {
        self.project.job_archive.check_project(&id)?;
        if !crate::storage::valid_document_identity(&id)
            || !self.model_service.receipts.is_empty()
            || !self.undo.is_empty()
            || !self.redo.is_empty()
        {
            return Err("Document identity cannot be rebound after editing".into());
        }
        self.model_service.project_id = id;
        Ok(())
    }
    /// Rotates when Editor is replaced on document open/recovery. Not a persistent project UUID.
    pub fn document_session(&self) -> u64 {
        self.model_service.id
    }

    /// Run the existing validation and typed operations on a disposable snapshot.
    /// The live editor's IDs, history, selection, status and dirty state remain untouched.
    pub fn prepare_change(
        &self,
        expected_revision: u64,
        commands: Vec<ModelCommand>,
    ) -> Result<PreparedChange, ChangeError> {
        self.prepare_change_admitted(expected_revision, commands, false, None)
    }
    pub(crate) fn prepare_rule_change(
        &self,
        expected_revision: u64,
        edit: crate::rules::RuleEdit,
        admission: &crate::project_repository::rules::RuleCommitGuard,
    ) -> Result<PreparedChange, ChangeError> {
        admission
            .verify(self, &edit)
            .map_err(|message| error("SOURCE_ADMISSION_MISMATCH", message))?;
        self.prepare_change_admitted(
            expected_revision,
            vec![ModelCommand::Rules { edit }],
            true,
            None,
        )
    }
    pub(crate) fn prepare_conditions_change(
        &self,
        expected_revision: u64,
        commands: Vec<ModelCommand>,
        admission: &crate::project_repository::conditions::ConditionsCommitGuard,
    ) -> Result<PreparedChange, ChangeError> {
        admission
            .verify(self, &commands)
            .map_err(|message| error("SOURCE_ADMISSION_MISMATCH", message))?;
        let mut prepared = self.prepare_change_admitted(expected_revision, commands, true, None)?;
        prepared.conditions_admission = true;
        Ok(prepared)
    }
    /// Source IO has already completed off-thread. The caller retains the opaque
    /// guard only until this immediate transaction returns; previews re-admit.
    pub fn prepare_constraint_change(
        &self,
        expected_revision: u64,
        commands: Vec<ModelCommand>,
        admission: &crate::project_repository::constraints::ConstraintCommitGuard,
    ) -> Result<PreparedChange, ChangeError> {
        admission
            .verify(self, &commands)
            .map_err(|message| error("CITED_ORIGINAL_ADMISSION_MISMATCH", message))?;
        self.prepare_change_admitted(expected_revision, commands, false, Some(admission))
    }
    fn prepare_change_admitted(
        &self,
        expected_revision: u64,
        commands: Vec<ModelCommand>,
        rule_sources_verified: bool,
        cited_admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Result<PreparedChange, ChangeError> {
        if !rule_sources_verified
            && commands.iter().any(|c| {
                matches!(
                    c,
                    ModelCommand::Rules {
                        edit: crate::rules::RuleEdit::AddSource { .. }
                            | crate::rules::RuleEdit::PutRequirement { .. }
                            | crate::rules::RuleEdit::PutLifecycle { .. }
                            | crate::rules::RuleEdit::PutCondition { .. }
                    }
                )
            })
        {
            return Err(error(
                "RULE_SOURCE_ADMISSION_REQUIRED",
                "Cited rule edits require repository source admission",
            ));
        }
        if expected_revision != self.revision {
            return Err(error(
                "REVISION_CONFLICT",
                "Model changed before preparation",
            ));
        }
        if commands.is_empty() {
            return Err(error("NO_CHANGE", "Empty command batch"));
        }
        let cited = crate::project_repository::constraints::cited_run_citations(
            &self.project.rules,
            &commands,
        )
        .map_err(|message| error("VALIDATION_FAILED", message))?;
        if !cited.is_empty() && cited_admission.is_none() {
            return Err(error(
                "CITED_ORIGINAL_ADMISSION_REQUIRED",
                "Verify current originals before running a cited check; its definition may be saved without running",
            ));
        }
        let mut candidate = Editor::new(self.project.clone());
        if let Ok(cache) = self.derivation_cache.try_borrow() {
            let session = candidate.document_session();
            *candidate.derivation_cache.get_mut() = cache.fork_for_session(session);
        }
        for command in &commands {
            if !matches!(
                command,
                ModelCommand::AddElement { .. }
                    | ModelCommand::SetOptions { .. }
                    | ModelCommand::Constraint { .. }
                    | ModelCommand::MassSpaces { .. }
                    | ModelCommand::DesignProcess { .. }
                    | ModelCommand::SetStoryElevation { .. }
                    | ModelCommand::SetSiteContext { .. }
                    | ModelCommand::Terrain { .. }
                    | ModelCommand::Program { .. }
                    | ModelCommand::Evidence { .. }
                    | ModelCommand::Rules { .. }
                    | ModelCommand::Diagram { .. }
                    | ModelCommand::SetDrawings { .. }
                    | ModelCommand::ApplySiteBoundary { .. }
                    | ModelCommand::DetachSiteBoundary
                    | ModelCommand::ApplySiteControlRegion { .. }
                    | ModelCommand::DetachSiteControlRegion
                    | ModelCommand::AddElements { .. }
                    | ModelCommand::UpdateElement { .. }
                    | ModelCommand::MoveElements { .. }
                    | ModelCommand::RemoveElements { .. }
            ) {
                return Err(error(
                    "UNSUPPORTED_COMMAND",
                    "Command is outside the model service allowlist",
                ));
            }
            candidate.apply(command.clone()).map_err(|e| {
                let mut failure = error("VALIDATION_FAILED", e);
                failure.diagnostics = candidate
                    .last_validation_diagnostic
                    .iter()
                    .cloned()
                    .collect();
                failure
            })?;
        }
        crate::design_process::update::validate_transition(&self.project, &candidate.project)
            .map_err(|message| error("VALIDATION_FAILED", message))?;
        if candidate.project == self.project {
            return Err(error("NO_CHANGE", "Batch has no model difference"));
        }
        Ok(PreparedChange {
            session: self.document_session(),
            base_revision: self.revision,
            before: self.project.clone(),
            after: candidate.project,
            commands,
            batch_input: None,
            resolved_references: BTreeMap::new(),
            recovery_admission: false,
            conditions_admission: false,
            cited_admission: cited_admission.map(|g| g.lease()),
        })
    }

    /// Recover the original committed result after a lost response, including after Undo.
    pub fn change_receipt(&self, request_id: &str) -> Option<&ChangeReceipt> {
        self.model_service
            .receipts
            .get(request_id)
            .map(|record| &record.receipt)
    }

    /// One live commit and one history entry. A duplicate never reapplies an undone operation.
    pub fn commit_change(
        &mut self,
        request_id: &str,
        prepared: &PreparedChange,
    ) -> Result<ChangeReceipt, ChangeError> {
        if prepared.session != self.document_session() {
            return Err(error(
                "DOCUMENT_CONFLICT",
                "Candidate belongs to another document session",
            ));
        }
        if request_id.is_empty() || request_id.len() > 256 {
            return Err(error(
                "INVALID_REQUEST_ID",
                "Request identity must contain 1–256 bytes",
            ));
        }
        if let Some(record) = self.model_service.receipts.get(request_id) {
            return if record.base_revision == prepared.base_revision
                && record.commands == prepared.commands
                && record.batch_input == prepared.batch_input
            {
                Ok(record.receipt.clone())
            } else {
                Err(error(
                    "REQUEST_ID_CONFLICT",
                    "Request identity was already committed with different input",
                ))
            };
        }
        if self.model_service.receipts.len() >= MAX_RECEIPTS {
            return Err(error(
                "SESSION_CAPACITY",
                "Receipt capacity reached; start a new document session explicitly",
            ));
        }
        if self.revision != prepared.base_revision
            || !self.project.same_model_state(&prepared.before)
        {
            return Err(error(
                "REVISION_CONFLICT",
                "Model changed after preparation",
            ));
        }
        prepared.validate_cited_admission(self)?;
        let task_changed = prepared.before.design_process.current_task
            != prepared.after.design_process.current_task;
        if task_changed && self.model_input_active() {
            return Err(error(
                "USER_INTERACTION_ACTIVE",
                "Finish or cancel the current model interaction before changing design tasks",
            ));
        }
        if self.revision == u64::MAX {
            return Err(error("REVISION_EXHAUSTED", "Revision cannot advance"));
        }
        let changed_ids = prepared.affected_ids();
        let old_status = self.status.clone();
        let before = self.project.clone();
        let mut after = prepared.after.clone();
        after
            .job_archive
            .check_project(self.project_identity())
            .map_err(|e| error("JOB_HISTORY_CONFLICT", e))?;
        after.job_archive = before
            .job_archive
            .merge(&after.job_archive)
            .map_err(|e| error("JOB_HISTORY_CONFLICT", e))?;
        self.project = after;
        let committed = if prepared.recovery_admission {
            self.commit_recovery_checked(before)
        } else if prepared
            .commands
            .iter()
            .any(|command| matches!(command, ModelCommand::MassSpaces { .. }))
        {
            self.commit_mass_space_checked(before)
        } else {
            self.commit_checked(before)
        };
        if !committed {
            let message = std::mem::replace(&mut self.status, old_status);
            return Err(error("VALIDATION_FAILED", message));
        }
        if task_changed {
            if let Some(task) = self.project.design_process.task() {
                let story = task.story;
                let active = self.project.plan_options.active_design_option;
                let selected = self
                    .project
                    .elements
                    .iter()
                    .find(|e| {
                        e.design_option == active
                            && e.story == story
                            && e.worksheet.is_none()
                            && task.accepts_shape(&e.shape)
                    })
                    .map(|e| e.id);
                self.switch_story(story);
                self.set_selection(selected.into_iter().collect());
            }
        }
        let receipt = ChangeReceipt {
            request_id: request_id.to_owned(),
            document_session: self.document_session(),
            revision_before: prepared.base_revision,
            revision_after: self.revision,
            changed_ids,
            design_process_changed: prepared.before.design_process != prepared.after.design_process,
            edit_state_changed: prepared.before.edit_state != prepared.after.edit_state,
            site_context_changed: prepared.before.site != prepared.after.site,
            program_changed: prepared.before.program != prepared.after.program,
            evidence_changed: prepared.before.evidence != prepared.after.evidence,
            rules_changed: prepared.before.rules != prepared.after.rules,
            drawings_changed: prepared.before.drawings != prepared.after.drawings,
            resolved_references: prepared.resolved_references.clone(),
        };
        self.model_service.receipts.insert(
            request_id.to_owned(),
            CommittedChange {
                base_revision: prepared.base_revision,
                commands: prepared.commands.clone(),
                batch_input: prepared.batch_input,
                receipt: receipt.clone(),
            },
        );
        Ok(receipt)
    }
}
