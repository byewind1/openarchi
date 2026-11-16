//! Durable read-only jobs. Immutable input/result artifacts; never replays an intent.
use super::{Lock, MAX_BYTES, Manifest, Operation, Registry, publish, read};
use crate::{
    editor::Editor,
    model::{Element, Shape},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub project_id: String,
    pub story: u32,
    pub option: Option<u32>,
    pub option_name: String,
    pub task: Option<serde_json::Value>,
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<crate::program::Program>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solar: Option<super::solar::Payload>,
}
impl Input {
    pub fn capture(e: &Editor) -> Result<Self, String> {
        let p = &e.project;
        let option = p.plan_options.active_design_option;
        if !p.stories.iter().any(|s| s.id == e.story) {
            return Err("Job source story is unavailable".into());
        }
        let option_name = match option {
            None => "Main model",
            Some(id) => p
                .design_options
                .name(id)
                .ok_or("Job source candidate is unavailable")?,
        }
        .to_owned();
        let mut elements: Vec<_> = p
            .elements
            .iter()
            .filter(|v| {
                v.story == e.story
                    && v.design_option == option
                    && v.worksheet.is_none()
                    && matches!(v.shape, Shape::Zone { .. } | Shape::ConceptMass { .. })
            })
            .cloned()
            .collect();
        elements.sort_by_key(|v| v.id);
        let task = p
            .design_process
            .task()
            .filter(|t| {
                t.story == e.story
                    && (Some(t.baseline_option) == option
                        || t.candidates.iter().any(|c| Some(c.option) == option))
            })
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| e.to_string())?;
        let input = Self {
            project_id: e.project_identity().into(),
            story: e.story,
            option,
            option_name,
            task,
            elements,
            program: None,
            solar: None,
        };
        input.validate()?;
        Ok(input)
    }
    pub fn fingerprint(&self) -> Result<String, String> {
        crate::automation_journal::canonical_hash(self)
    }
    pub fn capture_with_program(e: &Editor, required: bool) -> Result<Self, String> {
        let mut input = Self::capture(e)?;
        if required {
            input.program = Some(e.project.program.clone());
            input.validate()?;
        }
        Ok(input)
    }
    pub fn capture_solar(
        e: &Editor,
        parameters: &super::solar::Parameters,
    ) -> Result<Self, String> {
        super::solar::Context::capture(e).apply(Self::capture(e)?, parameters)
    }
    pub fn work_units(&self) -> usize {
        self.solar
            .as_ref()
            .and_then(|s| s.request.work_units().ok())
            .unwrap_or(self.elements.len())
    }
    pub(super) fn current(e: &Editor, original: &Self) -> Result<Self, String> {
        let mut input = Self::capture_with_program(e, original.program.is_some())?;
        if let Some(s) = &original.solar {
            input = super::solar::Context::capture(e).apply(input, &s.parameters())?;
        }
        Ok(input)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        if !digest(&self.project_id)
            || self.elements.len() > 4096
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() as u64 > MAX_BYTES / 2
        {
            return Err("Job snapshot exceeds its identity or size limit".into());
        }
        if let Some(s) = &self.solar {
            if self.program.is_some() {
                return Err("Solar and Program job inputs cannot be mixed".into());
            }
            s.validate(&self.elements)?;
            let input_bytes = serde_json::to_vec_pretty(self)
                .map_err(|e| e.to_string())?
                .len();
            let budget = s
                .request
                .work_units()?
                .saturating_mul(512)
                .saturating_add(input_bytes)
                .saturating_add(16384);
            if budget as u64 > MAX_BYTES {
                return Err("Solar input and result exceed the 4 MiB job envelope".into());
            }
        }
        let mut last = None;
        if let Some(program) = &self.program {
            program.validate()?;
        }
        for e in &self.elements {
            if e.story != self.story
                || e.design_option != self.option
                || e.worksheet.is_some()
                || last.is_some_and(|id| e.id <= id)
            {
                return Err("Job snapshot scope is inconsistent".into());
            }
            last = Some(e.id);
            match &e.shape {
                Shape::Zone { spec } => spec.validate()?,
                Shape::ConceptMass { spec } => spec.validate()?,
                _ => return Err("Job input is not authored Zone or mass geometry".into()),
            }
        }
        Ok(())
    }
}
pub(super) fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub project_id: String,
    pub request_id: String,
    pub plugin_id: String,
    pub plugin_version: u32,
    pub command_id: String,
    pub input_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub schema_version: u32,
    pub identity: Identity,
    pub source_session: u64,
    pub source_revision: u64,
    pub declaration: Manifest,
    pub input: Input,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<super::provider::ExecutableGrant>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    pub element_ids: Vec<u64>,
    pub area_m2: f64,
    pub volume_m3: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub definition: String,
    pub zones: Totals,
    pub concept_masses: Totals,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_review: Option<ProgramReview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solar: Option<crate::solar::ResultSet>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramReview {
    pub method: String,
    pub rows: Vec<RequirementReview>,
    pub unassigned_zone_ids: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementReview {
    pub sheet_id: String,
    pub source_sha256: String,
    pub key: String,
    pub label: String,
    pub scope: String,
    pub declared_quantity: u32,
    pub required_area: crate::program::Area,
    pub candidate_zone_ids: Vec<u64>,
    pub candidate_area_m2: f64,
    pub delta_m2: Option<f64>,
    pub mapping_status: String,
}
// Floating-point interchange only: not design/measurement tolerance. Both the
// existing typed SI budget and eight adjacent f64 representations must pass.
fn same_derived_number(actual: f64, expected: f64, budget: crate::precision::ErrorBudget) -> bool {
    actual.is_finite()
        && expected.is_finite()
        && (actual == expected
            || (actual != 0.
                && expected != 0.
                && actual.is_sign_positive() == expected.is_sign_positive()
                && actual.to_bits().abs_diff(expected.to_bits()) <= 8
                && budget.accepts(actual, expected)))
}
impl Totals {
    fn matches_source(
        &self,
        expected: &Self,
        tolerance: crate::precision::TolerancePolicy,
    ) -> bool {
        let Self {
            element_ids,
            area_m2,
            volume_m3,
        } = self;
        element_ids == &expected.element_ids
            && same_derived_number(*area_m2, expected.area_m2, tolerance.area_m2)
            && same_derived_number(*volume_m3, expected.volume_m3, tolerance.volume_m3)
    }
}
impl Report {
    fn matches_source(&self, expected: &Self) -> bool {
        let Self {
            definition,
            zones,
            concept_masses,
            program_review,
            solar,
        } = self;
        let tolerance = crate::precision::TolerancePolicy::default();
        if solar != &expected.solar {
            return false;
        }
        if definition != &expected.definition
            || !zones.matches_source(&expected.zones, tolerance)
            || !concept_masses.matches_source(&expected.concept_masses, tolerance)
        {
            return false;
        }
        match (program_review, &expected.program_review) {
            (None, None) => true,
            (Some(actual), Some(expected)) => {
                actual.method == expected.method
                    && actual.unassigned_zone_ids == expected.unassigned_zone_ids
                    && actual.rows.len() == expected.rows.len()
                    && actual
                        .rows
                        .iter()
                        .zip(&expected.rows)
                        .all(|(actual, expected)| {
                            if !same_derived_number(
                                actual.candidate_area_m2,
                                expected.candidate_area_m2,
                                tolerance.area_m2,
                            ) {
                                return false;
                            }
                            match (actual.delta_m2, expected.delta_m2) {
                                (Some(a), Some(e))
                                    if same_derived_number(a, e, tolerance.area_m2) => {}
                                (None, None) => {}
                                _ => return false,
                            }
                            // Comparison copy only: keep original report/receipt bytes.
                            // All source facts, declarations, unknowns and new fields
                            // not explicitly listed as derived quantities stay exact.
                            let mut comparable = actual.clone();
                            comparable.candidate_area_m2 = expected.candidate_area_m2;
                            comparable.delta_m2 = expected.delta_m2;
                            &comparable == expected
                        })
            }
            _ => false,
        }
    }
}
fn calculate(input: &Input, cancel: &AtomicBool, progress: &AtomicUsize) -> Result<Report, String> {
    let mut report=Report{definition:"Authored boundary minus holes; overlaps counted separately within each domain. Zone area and mass footprint are separate; not net/legal floor area or compliance.".into(),zones:Totals::default(),concept_masses:Totals::default(),program_review:None,solar:None};
    if let Some(s) = &input.solar {
        if cancel.load(Ordering::Acquire) {
            return Err("Job cancelled".into());
        }
        report.definition = crate::solar::SEMANTICS.into();
        report.solar = Some(crate::solar::run_with_control(
            &s.snapshot(&input.elements)?,
            &s.request,
            cancel,
            progress,
        )?);
        return Ok(report);
    }
    for (i, e) in input.elements.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err("Job cancelled".into());
        }
        let (a, v) = crate::diagrams::quantities(e);
        let t = if matches!(e.shape, Shape::Zone { .. }) {
            &mut report.zones
        } else {
            &mut report.concept_masses
        };
        t.element_ids.push(e.id);
        t.area_m2 += a;
        t.volume_m3 += v;
        progress.store(i + 1, Ordering::Release)
    }
    if let Some(program) = &input.program {
        let mut review=ProgramReview{method:"Exact purpose-label candidates only; no approved requirement-to-Zone mapping. Areas are declared row totals, not multiplied by quantity. Empty requirements remain unknown; no satisfaction/compliance verdict.".into(),rows:vec![],unassigned_zone_ids:report.zones.element_ids.clone()};
        for sheet in &program.sheets {
            for row in &sheet.rows {
                if cancel.load(Ordering::Acquire) {
                    return Err("Job cancelled".into());
                }
                let candidates: Vec<_> = input
                    .elements
                    .iter()
                    .filter(
                        |e| matches!(&e.shape,Shape::Zone{spec} if spec.purpose==row.values.label),
                    )
                    .collect();
                let area = candidates
                    .iter()
                    .map(|e| crate::diagrams::quantities(e).0)
                    .sum();
                let duplicates = program
                    .sheets
                    .iter()
                    .flat_map(|s| &s.rows)
                    .filter(|r| r.values.label == row.values.label)
                    .count()
                    > 1;
                review.rows.push(RequirementReview {
                    sheet_id: sheet.id.clone(),
                    source_sha256: sheet.source_sha256.clone(),
                    key: row.key.clone(),
                    label: row.values.label.clone(),
                    scope: row.values.scope.clone(),
                    declared_quantity: row.values.quantity,
                    required_area: row.values.area.clone(),
                    candidate_zone_ids: candidates.iter().map(|e| e.id).collect(),
                    candidate_area_m2: area,
                    delta_m2: match row.values.area {
                        crate::program::Area::Known { m2 } => Some(area - m2),
                        crate::program::Area::Unknown { .. } => None,
                    },
                    mapping_status: if candidates.is_empty() {
                        "no_label_match"
                    } else if duplicates {
                        "ambiguous_label"
                    } else {
                        "unverified_label_match"
                    }
                    .into(),
                });
            }
        }
        report.program_review = Some(review);
    }
    Ok(report)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum Terminal {
    Completed { report: Report },
    Failed { message: String },
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultArtifact {
    pub schema_version: u32,
    pub identity: Identity,
    pub terminal: Terminal,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub intent: Intent,
    pub result: Option<ResultArtifact>,
    pub cancel_requested: bool,
}
impl Bundle {
    pub fn phase(&self) -> &'static str {
        match &self.result {
            Some(ResultArtifact {
                terminal: Terminal::Completed { .. },
                ..
            }) if !self.cancel_requested => "Completed",
            Some(ResultArtifact {
                terminal: Terminal::Completed { .. },
                ..
            }) => "Completed; cancellation requested",
            Some(ResultArtifact {
                terminal: Terminal::Failed { .. },
                ..
            }) => "Failed",
            Some(_) => "Cancelled",
            None if self.cancel_requested => "Cancel requested",
            None => "Unknown; inspect original job",
        }
    }
    pub fn current(&self, e: &Editor) -> bool {
        Input::current(e, &self.intent.input)
            .and_then(|i| i.fingerprint())
            .is_ok_and(|h| h == self.intent.identity.input_sha256)
    }
    /// Resolve a validated quantity report member in the current model, without
    /// activating a task, changing visibility, or granting any write authority.
    pub fn reviewed_member<'a>(&self, editor: &'a Editor, id: u64) -> Result<&'a Element, String> {
        self.validate()?;
        let Some(ResultArtifact {
            terminal: Terminal::Completed { report },
            ..
        }) = &self.result
        else {
            return Err("Only completed geometry reports can locate reviewed objects".into());
        };
        if report.solar.is_some() {
            return Err("Only completed geometry reports can locate reviewed objects".into());
        }
        if !self.current(editor) {
            return Err("Job result is stale for the current candidate".into());
        }
        let p = &editor.project;
        let member = p
            .elements
            .iter()
            .find(|element| {
                element.id == id
                    && element.story == self.intent.input.story
                    && element.design_option == self.intent.input.option
                    && element.worksheet.is_none()
                    && p.plan_options.element_visible(element, &p.layers)
                    && match element.shape {
                        Shape::Zone { .. } => report.zones.element_ids.contains(&id),
                        Shape::ConceptMass { .. } => {
                            report.concept_masses.element_ids.contains(&id)
                        }
                        _ => false,
                    }
            })
            .filter(|_| p.plan_options.active_worksheet.is_none())
            .ok_or("Reviewed object is unavailable or hidden in the current model view")?;
        if editor.expanded_selection(vec![id]) != [id] {
            return Err(
                "Suspend groups to inspect this reviewed object without selecting other objects"
                    .into(),
            );
        }
        Ok(member)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        let i = &self.intent;
        i.declaration.validate()?;
        i.input.validate()?;
        if i.schema_version != 1
            || i.identity.project_id != i.input.project_id
            || !digest(&i.identity.request_id)
            || i.identity.input_sha256 != i.input.fingerprint()?
            || i.identity.plugin_id != i.declaration.id
            || i.identity.plugin_version != i.declaration.version
            || !i
                .declaration
                .commands
                .iter()
                .any(|c| c.id == i.identity.command_id)
            || i.declaration.provider.is_some() != i.execution.is_some()
            || i.input.program.is_some()
                != i.declaration
                    .commands
                    .iter()
                    .find(|c| c.id == i.identity.command_id)
                    .is_some_and(|c| c.operation == Operation::ZoneRequirements)
        {
            return Err("Job identity or input fingerprint is inconsistent".into());
        }
        let solar_command = i
            .declaration
            .commands
            .iter()
            .find(|c| c.id == i.identity.command_id)
            .is_some_and(|c| c.operation == Operation::SolarStudy);
        if solar_command != i.input.solar.is_some() {
            return Err("Solar job operation and immutable parameters differ".into());
        }
        if let Some(grant) = &i.execution {
            grant.validate()?;
        }
        if let Some(r) = &self.result {
            if r.schema_version != 1 || r.identity != i.identity {
                return Err("Job result belongs to another operation".into());
            }
            if let Terminal::Completed { report } = &r.terminal {
                let expected = calculate(&i.input, &AtomicBool::new(false), &AtomicUsize::new(0))?;
                if !report.matches_source(&expected) {
                    return Err("Job result differs from its authored input".into());
                }
            }
            if matches!(&r.terminal,Terminal::Failed{message} if message.len()>8192) {
                return Err("Provider failure message exceeds 8 KiB".into());
            }
        }
        Ok(())
    }
    pub fn export(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        publish(
            path,
            &serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
            false,
        )
    }
    pub fn read_export(path: &Path) -> Result<Self, String> {
        // Imported paths are untrusted local input. In particular, opening a FIFO
        // must not wait for another process, and a link must not change the target.
        if !fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
        {
            return Err("Job record must be a regular file, not a link or device".into());
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let f = options.open(path).map_err(|e| e.to_string())?;
        if !f.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Job record must be a regular file, not a link or device".into());
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        f.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Plugin artifact exceeds 4 MiB".into());
        }
        let b: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        b.validate()?;
        Ok(b)
    }
}
struct Running {
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    handle: JoinHandle<Result<(), String>>,
    external_worker: bool,
}
pub struct Jobs {
    root: PathBuf,
    running: BTreeMap<(String, String), Running>,
    pub diagnostics: Vec<String>,
    worker_executable: Option<PathBuf>,
    worker_required: bool,
}
impl Jobs {
    /// Initial view hint only. Avoid parsing/recalculating every historical
    /// report on the UI thread. The selected record still gets full validation;
    /// a missing terminal file does not prove a running process or authority.
    pub(crate) fn preferred_initial_job(
        &self,
        project: &str,
        archive: &super::archive::Archive,
    ) -> Result<Option<String>, String> {
        let ids = self.ids_with_archive(project, archive)?;
        for id in &ids {
            if archive
                .get(project, id)?
                .is_some_and(|record| record.bundle.result.is_some())
            {
                continue;
            }
            let result = self.operation(project, id)?.join("result.json");
            match fs::symlink_metadata(result) {
                Ok(meta) if meta.is_file() => {}
                Ok(_) => return Ok(Some(id.clone())), // Select for inspection, not execution.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Some(id.clone()));
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(ids.last().cloned())
    }
    pub fn ids_with_archive(
        &self,
        project: &str,
        archive: &super::archive::Archive,
    ) -> Result<Vec<String>, String> {
        archive.check_project(project)?;
        let mut ids = self.ids(project)?;
        ids.extend(
            archive
                .records()
                .iter()
                .map(|r| r.bundle.intent.identity.request_id.clone()),
        );
        ids.sort();
        ids.dedup();
        if ids.len() > 64 {
            return Err("Project job archive exceeds 64 original receipts".into());
        }
        Ok(ids)
    }
    /// Prefer more complete evidence for exactly the same original intent.
    /// Corrupt local records cannot be hidden by a saved snapshot.
    pub fn bundle_with_archive(
        &self,
        project: &str,
        id: &str,
        archive: &super::archive::Archive,
    ) -> Result<Bundle, String> {
        let archived = archive.get(project, id)?;
        let path = self.operation(project, id)?;
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return archived
                    .map(|r| r.bundle.clone())
                    .ok_or_else(|| "Original job evidence is unavailable".into());
            }
            Err(e) => return Err(e.to_string()),
            Ok(_) => {}
        }
        let mut local = self.query(project, id)?;
        if let Some(old) = archived {
            if old.bundle.intent != local.intent
                || matches!((&old.bundle.result,&local.result),(Some(a),Some(b)) if a!=b)
            {
                return Err("Local and saved job evidence conflict; originals preserved".into());
            }
            if local.result.is_none() {
                local.result = old.bundle.result.clone();
            }
            local.cancel_requested |= old.bundle.cancel_requested;
        }
        Ok(local)
    }
    pub fn process_with_archive(
        &self,
        identity: &Identity,
        archive: &super::archive::Archive,
    ) -> Result<Option<super::provider::Capture>, String> {
        let old = archive
            .get(&identity.project_id, &identity.request_id)?
            .and_then(|r| r.process.as_ref());
        if let Some(receipt) = old {
            receipt.validate(identity)?;
        }
        let local = self.process_receipt(identity)?;
        if matches!((old,&local),(Some(a),Some(b)) if a!=b) {
            return Err("Local and saved process evidence conflict; originals preserved".into());
        }
        Ok(local.or_else(|| old.cloned()))
    }
    /// Strict combined capture for saving, while reads can diagnose process
    /// corruption separately and still show a valid original input/result.
    pub fn record_with_archive(
        &self,
        project: &str,
        id: &str,
        archive: &super::archive::Archive,
    ) -> Result<super::archive::Record, String> {
        let bundle = self.bundle_with_archive(project, id, archive)?;
        let process = self.process_with_archive(&bundle.intent.identity, archive)?;
        Ok(super::archive::Record { bundle, process })
    }
    /// Intended for a save worker. It reads original artifacts only, never dispatches.
    pub fn capture_archive(
        &self,
        project: &str,
        archive: &super::archive::Archive,
    ) -> Result<super::archive::Archive, String> {
        let mut records = vec![];
        let mut bytes = 0usize;
        for id in self.ids_with_archive(project, archive)? {
            let record = self.record_with_archive(project, &id, archive)?;
            bytes = bytes
                .checked_add(
                    serde_json::to_vec_pretty(&record)
                        .map_err(|e| e.to_string())?
                        .len(),
                )
                .ok_or("Project exceeds 32 MB demo limit")?;
            if bytes as u64 > crate::storage::MAX_PROJECT_BYTES {
                return Err("Project exceeds 32 MB demo limit".into());
            }
            records.push(record);
        }
        super::archive::Archive::new(records)
    }
    pub(crate) fn read_root(&self) -> PathBuf {
        self.root.clone()
    }
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            running: BTreeMap::new(),
            diagnostics: vec![],
            worker_executable: None,
            worker_required: false,
        }
    }
    /// Trusted application bootstrap, not a model command or remote permission.
    /// The product uses its own binary; library-only legacy callers remain explicit.
    pub fn use_process_worker(&mut self, executable: &Path) -> Result<(), String> {
        self.require_process_worker();
        let path = fs::canonicalize(executable).map_err(|e| e.to_string())?;
        if !path.is_file() {
            return Err("Original job worker must be a regular executable".into());
        }
        self.worker_executable = Some(path);
        Ok(())
    }
    pub fn require_process_worker(&mut self) {
        self.worker_required = true;
        self.worker_executable = None;
    }
    pub fn execution_observation(
        &self,
        identity: &Identity,
    ) -> Result<super::worker::Observation, String> {
        super::worker::observe(self, identity)
    }
    /// Restore evidence only. A declaration or an executable grant in a saved
    /// intent is provenance, never registration, execution authority or ownership.
    pub fn import_record(&self, project: &str, source: &Path) -> Result<Identity, String> {
        let b = Bundle::read_export(source)?;
        let identity = b.intent.identity.clone();
        if identity.project_id != project {
            return Err("Job record belongs to another project".into());
        }
        let doc = self.document(project)?;
        let path = self.operation(project, &identity.request_id)?;
        fs::create_dir_all(&doc).map_err(|e| e.to_string())?;
        let _lock = Lock::acquire(&doc)?;
        if fs::symlink_metadata(&path).is_ok() {
            return if self.query(project, &identity.request_id)? == b {
                Ok(identity)
            } else {
                Err("Original job already exists with different evidence; preserved".into())
            };
        }
        if self.ids(project)?.len() >= 64 {
            return Err("Job receipt capacity reached; export and preserve history".into());
        }
        // Same-directory publication under the same project lock as dispatch.
        // A crash before rename leaves only an undiscoverable staging directory;
        // after rename the ordinary reader sees the complete original bundle.
        // This does not promise power-loss durability or external-writer isolation.
        let stage = doc.join(format!(
            ".import-{}",
            crate::storage::new_document_identity()
        ));
        fs::create_dir(&stage).map_err(|e| e.to_string())?;
        let result = (|| {
            publish(
                &stage.join("intent.json"),
                &serde_json::to_vec_pretty(&b.intent).map_err(|e| e.to_string())?,
                false,
            )?;
            if let Some(result) = &b.result {
                publish(
                    &stage.join("result.json"),
                    &serde_json::to_vec_pretty(result).map_err(|e| e.to_string())?,
                    false,
                )?;
            }
            if b.cancel_requested {
                publish(
                    &stage.join("cancel.json"),
                    &serde_json::to_vec_pretty(&identity).map_err(|e| e.to_string())?,
                    false,
                )?;
            }
            // Preserve even an empty/unknown existing operation, not just valid ones.
            if fs::symlink_metadata(&path).is_ok() {
                return Err("Original job appeared during import; preserved".to_string());
            }
            fs::rename(&stage, &path).map_err(|e| e.to_string())
        })();
        result.map_err(|e| format!("{e} · Import staging retained: {}", stage.display()))?;
        Ok(identity)
    }
    fn document(&self, id: &str) -> Result<PathBuf, String> {
        if !digest(id) {
            return Err("Invalid job project identity".into());
        }
        let path = self.root.join(id);
        if self.root.is_symlink() || path.is_symlink() {
            return Err("Job storage symlinks are unsupported".into());
        }
        Ok(path)
    }
    pub(super) fn operation(&self, project: &str, id: &str) -> Result<PathBuf, String> {
        if !digest(id) {
            return Err("Invalid original job request identity".into());
        }
        let path = self.document(project)?.join(id);
        if path.is_symlink() {
            return Err("Job storage symlinks are unsupported".into());
        }
        Ok(path)
    }
    pub fn ids(&self, project: &str) -> Result<Vec<String>, String> {
        let dir = self.document(project)?;
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut ids = vec![];
        for p in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let p = p.map_err(|e| e.to_string())?;
            let id = p.file_name().to_string_lossy().into_owned();
            if digest(&id) && p.file_type().map_err(|e| e.to_string())?.is_dir() {
                ids.push(id)
            }
        }
        ids.sort();
        Ok(ids)
    }
    pub fn query(&self, project: &str, id: &str) -> Result<Bundle, String> {
        let path = self.operation(project, id)?;
        for name in ["intent.json", "result.json", "cancel.json"] {
            if path.join(name).is_symlink() {
                return Err("Job artifact symlinks are unsupported".into());
            }
        }
        let intent: Intent = serde_json::from_slice(&read(&path.join("intent.json"))?)
            .map_err(|e| format!("Job intent unknown; preserve original operation: {e}"))?;
        let result = if path.join("result.json").exists() {
            Some(
                serde_json::from_slice(&read(&path.join("result.json"))?)
                    .map_err(|e| format!("Job result unknown; do not replay: {e}"))?,
            )
        } else {
            None
        };
        let cancel_requested = if path.join("cancel.json").exists() {
            let i: Identity = serde_json::from_slice(&read(&path.join("cancel.json"))?)
                .map_err(|e| e.to_string())?;
            if i != intent.identity {
                return Err("Job cancellation identity differs".into());
            }
            true
        } else {
            false
        };
        let b = Bundle {
            intent,
            result,
            cancel_requested,
        };
        b.validate()?;
        if b.intent.identity.project_id != project || b.intent.identity.request_id != id {
            return Err("Job path and original identity differ".into());
        }
        Ok(b)
    }
    pub fn process_receipt(
        &self,
        identity: &Identity,
    ) -> Result<Option<super::provider::Capture>, String> {
        let path = self
            .operation(&identity.project_id, &identity.request_id)?
            .join("process-result.json");
        if path.is_symlink() {
            return Err("Job artifact symlinks are unsupported".into());
        }
        if !path.exists() {
            return Ok(None);
        }
        let capture: super::provider::Capture =
            serde_json::from_slice(&read(&path)?).map_err(|e| e.to_string())?;
        capture.validate(identity)?;
        Ok(Some(capture))
    }
    pub fn start(
        &mut self,
        registry: &Registry,
        e: &Editor,
        command: &str,
        id: &str,
    ) -> Result<Identity, String> {
        self.start_with_solar(registry, e, command, id, None)
    }
    pub fn start_with_solar(
        &mut self,
        registry: &Registry,
        e: &Editor,
        command: &str,
        id: &str,
        solar: Option<&super::solar::Parameters>,
    ) -> Result<Identity, String> {
        let (manifest, c) = registry.resolve(command)?;
        let input = match (&c.operation, solar) {
            (Operation::SolarStudy, Some(parameters)) => Input::capture_solar(e, parameters)?,
            (Operation::SolarStudy, None) => {
                return Err(
                    "Solar parameters are required; no location or date is inferred".into(),
                );
            }
            (_, Some(_)) => return Err("Solar parameters do not belong to this command".into()),
            (_, None) => {
                Input::capture_with_program(e, c.operation == Operation::ZoneRequirements)?
            }
        };
        let execution = registry.executable(&manifest.id).cloned();
        if manifest.provider.is_some() != execution.is_some() {
            return Err("External provider execution is not authorized".into());
        }
        if execution.is_some() && self.worker_required && self.worker_executable.is_none() {
            return Err("Original process worker unavailable; no program started".into());
        }
        let identity = Identity {
            project_id: input.project_id.clone(),
            request_id: id.into(),
            plugin_id: manifest.id.clone(),
            plugin_version: manifest.version,
            command_id: command.into(),
            input_sha256: input.fingerprint()?,
        };
        let path = self.operation(&identity.project_id, id)?;
        let doc = self.document(&identity.project_id)?;
        fs::create_dir_all(&doc).map_err(|e| e.to_string())?;
        let _lock = Lock::acquire(&doc)?;
        let ids = self.ids_with_archive(&identity.project_id, &e.project.job_archive)?;
        if ids.iter().any(|known| known == id) || path.exists() {
            let old = self
                .record_with_archive(&identity.project_id, id, &e.project.job_archive)?
                .bundle;
            if old.intent.identity == identity && old.intent.declaration == *manifest {
                return Ok(identity);
            }
            return Err("Job request identity was already used for different input".into());
        }
        if ids.len() >= 64 {
            return Err("Job receipt capacity reached; export and preserve history".into());
        }
        for prior in ids {
            let b = self
                .record_with_archive(&identity.project_id, &prior, &e.project.job_archive)?
                .bundle;
            if b.result.is_none() {
                return Err(format!(
                    "Unresolved job; inspect original request {}",
                    b.intent.identity.request_id
                ));
            }
        }
        let intent = Intent {
            schema_version: 1,
            identity: identity.clone(),
            source_session: e.document_session(),
            source_revision: e.revision,
            declaration: manifest.clone(),
            input,
            execution,
        };
        let bytes = serde_json::to_vec_pretty(&intent).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Job intent exceeds 4 MiB".into());
        }
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        publish(&path.join("intent.json"), &bytes, false)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let cc = cancel.clone();
        let pp = progress.clone();
        let key = (identity.project_id.clone(), identity.request_id.clone());
        let worker_executable = self
            .worker_executable
            .clone()
            .filter(|_| intent.execution.is_some());
        let external_worker = worker_executable.is_some();
        let root = self.root.clone();
        // The intent is durable before dispatch. No Editor/Project or mutation handle
        // crosses this boundary, and a process restart never respawns this worker.
        let handle = std::thread::Builder::new()
            .name("openarchi-read-only-job".into())
            .spawn(move || {
                if let Some(executable) = worker_executable {
                    let mut child = super::worker::launch(&executable, &root, &intent, &path)?;
                    let observer = Jobs::new(root);
                    loop {
                        if let Some(n) = observer.execution_observation(&intent.identity)?.completed {
                            pp.store(n, Ordering::Release);
                        }
                        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                            let original = observer.query(&intent.identity.project_id, &intent.identity.request_id)?;
                            return if original.result.is_some() {
                                Ok(())
                            } else {
                                Err(format!("Original worker outcome unknown ({status}); inspect original request, never replay"))
                            };
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if intent.execution.is_some() {
                        super::provider::execute(&intent, &path, &cc, &pp)
                    } else {
                        calculate(&intent.input, &cc, &pp)
                            .map(|report| Terminal::Completed { report })
                    }
                }));
                if let Ok(Err(message)) = &outcome {
                    if message.starts_with(super::provider::OWNERSHIP_UNKNOWN) { return Err(message.clone()); }
                }
                let terminal = if cc.load(Ordering::Acquire) {
                    Terminal::Cancelled
                } else {
                    match outcome {
                        Ok(Ok(terminal)) => terminal,
                        Ok(Err(message)) => Terminal::Failed { message },
                        Err(_) => Terminal::Failed {
                            message: "Plugin provider panicked".into(),
                        },
                    }
                };
                publish(
                    &path.join("result.json"),
                    &serde_json::to_vec_pretty(&ResultArtifact {
                        schema_version: 1,
                        identity: intent.identity,
                        terminal,
                    })
                    .map_err(|e| e.to_string())?,
                    false,
                )
            })
            .map_err(|e| format!("Job dispatch unknown; inspect original request: {e}"))?;
        self.running.insert(
            key,
            Running {
                cancel,
                progress,
                handle,
                external_worker,
            },
        );
        Ok(identity)
    }
    pub fn progress(&self, project: &str, id: &str) -> Option<usize> {
        self.running
            .get(&(project.into(), id.into()))
            .filter(|r| !r.handle.is_finished())
            .map(|r| r.progress.load(Ordering::Acquire))
            .or_else(|| {
                let original = self.query(project, id).ok()?;
                if original.result.is_some() {
                    return None;
                }
                let observed = self.execution_observation(&original.intent.identity).ok()?;
                (observed.status == "Running").then_some(observed.completed.unwrap_or(0))
            })
    }
    pub fn poll(&mut self) {
        let finished: Vec<_> = self
            .running
            .iter()
            .filter(|(_, r)| r.handle.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in finished {
            let r = self.running.remove(&id).unwrap();
            match r.handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => self.diagnostics.push(format!("{id:?}: {e}")),
                Err(_) => self.diagnostics.push(format!(
                    "{id:?}: worker outcome unknown; inspect original operation"
                )),
            }
        }
    }
    pub fn cancel_plugin(&mut self, plugin: &str) -> Result<(), String> {
        let keys: Vec<_> = self.running.keys().cloned().collect();
        for (project, id) in keys {
            if self.query(&project, &id)?.intent.identity.plugin_id == plugin {
                self.cancel(&project, &id)?;
            }
        }
        Ok(())
    }
    pub fn cancel(&mut self, project: &str, id: &str) -> Result<Bundle, String> {
        let b = self.query(project, id)?;
        if b.result.is_some() || b.cancel_requested {
            return Ok(b);
        }
        let doc = self.document(project)?;
        let _lock = Lock::acquire(&doc)?;
        publish(
            &self.operation(project, id)?.join("cancel.json"),
            &serde_json::to_vec_pretty(&b.intent.identity).map_err(|e| e.to_string())?,
            false,
        )?;
        if let Some(r) = self.running.get(&(project.into(), id.into())) {
            r.cancel.store(true, Ordering::Release)
        }
        self.query(project, id)
    }
}
impl Drop for Jobs {
    fn drop(&mut self) {
        // Preserve the existing normal-close cancellation intent. SIGKILL never
        // runs Drop: the original worker then finishes its bounded invocation.
        let keys: Vec<_> = self
            .running
            .iter()
            .filter(|(_, r)| r.external_worker)
            .map(|(key, _)| key.clone())
            .collect();
        for (project, id) in keys {
            let _ = self.cancel(&project, &id);
        }
        for r in self.running.values() {
            r.cancel.store(true, Ordering::Release)
        }
    }
}
