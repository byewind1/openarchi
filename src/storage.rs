use crate::model::{ModelAdapter, Project};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
pub struct JsonAdapter;
pub const MAX_PROJECT_BYTES: u64 = 32 * 1024 * 1024;
fn bounded_project_bytes(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        Err("Project exceeds 32 MB demo limit".into())
    } else {
        Ok(bytes)
    }
}
/// Bundled editable example; never binds saves to the preserved source asset.
pub fn startup_example() -> Project {
    crate::samples::concept_study()
}
#[cfg(test)]
pub(crate) fn synthetic_building_fixture() -> Project {
    crate::samples::building()
}
impl ModelAdapter for JsonAdapter {
    fn format_name(&self) -> &str {
        "OpenArchi JSON"
    }
    fn import(&self, bytes: &[u8]) -> Result<Project, String> {
        if bytes.len() as u64 > MAX_PROJECT_BYTES {
            return Err("Project exceeds 32 MB demo limit".into());
        }
        let mut p: Project = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if !p.trace.is_empty() || p.schema_version == 38 {
            if p.schema_version != 38 || p.trace.is_empty() {
                return Err("Trace settings require native document version 38".into());
            }
            let mut underlying = p.clone();
            underlying.trace = Default::default();
            underlying.schema_version = crate::precision::MODEL_SCHEMA_VERSION;
            let bytes = self.export(&underlying)?;
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            p.schema_version = value["schema_version"]
                .as_u64()
                .ok_or("Invalid native document version")? as u32;
        }
        if needs_mass_arc_format(&p) || p.schema_version == 37 {
            if p.schema_version != 37 || !needs_mass_arc_format(&p) {
                return Err("Curved concept masses require native document version 37".into());
            }
            p.schema_version = if crate::cad_reference::needs_extended_format(&p.drawings) {
                36
            } else if !p.drawings.cad_sources.is_empty() {
                35
            } else if needs_surface_format(&p) {
                34
            } else {
                previous_native_version(&p)
            };
        }
        if crate::cad_reference::needs_extended_format(&p.drawings) || p.schema_version == 36 {
            if p.schema_version != 36 || !crate::cad_reference::needs_extended_format(&p.drawings) {
                return Err("Model-plan CAD references require native document version 36".into());
            }
            p.schema_version = 35;
        }
        if !p.drawings.cad_sources.is_empty() || p.schema_version == 35 {
            if p.schema_version != 35 || p.drawings.cad_sources.is_empty() {
                return Err("CAD reference assets require native document version 35".into());
            }
            p.schema_version = if needs_surface_format(&p) {
                34
            } else {
                previous_native_version(&p)
            };
        }
        if needs_surface_format(&p) || p.schema_version == 34 {
            if p.schema_version != 34 || !needs_surface_format(&p) {
                return Err(
                    "Zone surfaces and 3D presentation require native document version 34".into(),
                );
            }
            p.schema_version = previous_native_version(&p);
        }
        if needs_linked_space_task_format(&p) || p.schema_version == 33 {
            if p.schema_version != 33 || !needs_linked_space_task_format(&p) {
                return Err("Linked space task scopes require native document version 33".into());
            }
            p.schema_version = if needs_reviewed_parametric_inputs_format(&p) {
                32
            } else if needs_object_creation_task_format(&p) {
                31
            } else {
                30
            };
        }
        if needs_reviewed_parametric_inputs_format(&p) || p.schema_version == 32 {
            if p.schema_version != 32 || !needs_reviewed_parametric_inputs_format(&p) {
                return Err("Reviewed parameter inputs require native document version 32".into());
            }
            p.schema_version = if needs_object_creation_task_format(&p) {
                31
            } else if needs_object_task_format(&p) {
                30
            } else if needs_agent_reply_format(&p) {
                29
            } else {
                28
            };
        }
        if needs_object_creation_task_format(&p) || p.schema_version == 31 {
            if p.schema_version != 31 || !needs_object_creation_task_format(&p) {
                return Err(
                    "Object task creation scopes require native document version 31".into(),
                );
            }
            p.schema_version = 30;
        }
        if needs_object_task_format(&p) || p.schema_version == 30 {
            if p.schema_version != 30 || !needs_object_task_format(&p) {
                return Err("Object task handoffs require native document version 30".into());
            }
            p.schema_version = if needs_agent_reply_format(&p) {
                29
            } else if needs_parametric_format(&p) {
                28
            } else {
                26
            };
        }
        if needs_agent_reply_format(&p) || p.schema_version == 29 {
            if p.schema_version != 29 || !needs_agent_reply_format(&p) {
                return Err(
                    "Agent discussion contributions require native document version 29".into(),
                );
            }
            p.schema_version = if needs_parametric_format(&p) { 28 } else { 26 };
        }
        let legacy_parameters = p.schema_version == 27
            && needs_parametric_format(&p)
            && !needs_parameter_state_format(&p);
        if needs_parameter_state_format(&p) || p.schema_version == 28 {
            if p.schema_version != 28 || !needs_parametric_format(&p) {
                return Err("Current parameter states require native document version 28".into());
            }
            p.schema_version = 27;
        }
        if needs_parametric_format(&p) {
            if p.schema_version != 27 {
                return Err(
                    "Parametric design definitions require native document version 27".into(),
                );
            }
            p.schema_version = if !p.evidence.topics.is_empty() {
                26
            } else {
                native_version_without_topics(&p)
            };
        }
        if !p.evidence.topics.is_empty() {
            if p.schema_version != 26 {
                return Err("Object discussions require native document version 26".into());
            }
            p.schema_version = native_version_without_topics(&p);
        }
        if needs_manual_mass_task_format(&p) {
            if p.schema_version != 25 {
                return Err("Manual mass candidates require native document version 25".into());
            }
            p.schema_version = if needs_control_region_format(&p) {
                24
            } else if needs_edge_check_format(&p) {
                23
            } else if needs_deferred_choice_format(&p) {
                22
            } else if needs_solar_job_format(&p) {
                21
            } else if needs_constraint_figure_format(&p) {
                20
            } else if !p.rules.checks.is_empty() {
                19
            } else {
                native_version_without_checks(&p)
            };
        }
        if needs_control_region_format(&p) {
            if p.schema_version != 24 {
                return Err("Control-region references require native document version 24".into());
            }
            p.schema_version = if needs_edge_check_format(&p) {
                23
            } else if needs_deferred_choice_format(&p) {
                22
            } else if needs_solar_job_format(&p) {
                21
            } else if needs_constraint_figure_format(&p) {
                20
            } else if !p.rules.checks.is_empty() {
                19
            } else {
                native_version_without_checks(&p)
            };
        }
        if needs_edge_check_format(&p) {
            if p.schema_version != 23 {
                return Err("Reference-edge checks require native document version 23".into());
            }
            p.schema_version = if needs_deferred_choice_format(&p) {
                22
            } else if needs_solar_job_format(&p) {
                21
            } else if needs_constraint_figure_format(&p) {
                20
            } else if !p.rules.checks.is_empty() {
                19
            } else {
                native_version_without_checks(&p)
            };
        }
        // A deliberately deferred rationale must remain distinct from corrupt
        // old records, and older readers must reject it rather than lose it.
        if needs_deferred_choice_format(&p) {
            if p.schema_version != 22 {
                return Err("Deferred choice reasons require native document version 22".into());
            }
            p.schema_version = if needs_solar_job_format(&p) {
                21
            } else if needs_constraint_figure_format(&p) {
                20
            } else if !p.rules.checks.is_empty() {
                19
            } else {
                native_version_without_checks(&p)
            };
        }
        if needs_solar_job_format(&p) {
            if p.schema_version != 21 {
                return Err("Solar jobs require native document version 21".into());
            }
            p.schema_version = if needs_constraint_figure_format(&p) {
                20
            } else if !p.rules.checks.is_empty() {
                19
            } else {
                native_version_without_checks(&p)
            };
        }
        if needs_constraint_figure_format(&p) {
            if p.schema_version != 20 {
                return Err("Constraint figures require native document version 20".into());
            }
            p.schema_version = if !p.rules.checks.is_empty() {
                19
            } else {
                native_version_without_checks(&p)
            };
        }
        if !p.rules.checks.is_empty() {
            if p.schema_version != 19 {
                return Err("Quantified checks require native document version 19".into());
            }
            p.schema_version = native_version_without_checks(&p);
        }
        let has_linked_figures = crate::diagrams::needs_mass_space_figure_format(&p);
        let has_links = crate::diagrams::needs_mass_space_format(&p);
        if has_linked_figures && p.schema_version != 18 {
            return Err("Mass-space source figures require native document version 18".into());
        }
        if has_links && !has_linked_figures && p.schema_version != 17 {
            return Err("Mass-space links require native document version 17".into());
        }
        let has_reports = !p.drawings.reports.is_empty();
        if has_reports && !has_links && p.schema_version != 16 {
            return Err("Report templates require native document version 16".into());
        }
        let has_jobs = !p.job_archive.is_empty();
        if has_jobs {
            #[derive(serde::Deserialize)]
            struct IdentityHeader {
                #[serde(rename = "_openarchi_document_id")]
                id: Option<String>,
            }
            let header: IdentityHeader =
                serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            if p.schema_version
                != (if has_linked_figures {
                    18
                } else if has_links {
                    17
                } else if has_reports {
                    16
                } else {
                    15
                })
                || header.id.as_deref() != p.job_archive.project_id()
            {
                return Err(format!(
                    "Project job archive requires native document version {} and its original identity",
                    if has_linked_figures {
                        18
                    } else if has_links {
                        17
                    } else if has_reports {
                        16
                    } else {
                        15
                    }
                ));
            }
        }
        // Native file v3 protects nonempty site metadata from older v2 readers.
        // Internal geometry precision remains schema 2. Empty-site legacy bytes stay v2.
        let has_masses = p.design_process.needs_mass_format()
            || p.elements
                .iter()
                .any(|e| matches!(e.shape, crate::model::Shape::ConceptMass { .. }));
        let has_figures = !p.drawings.diagrams.is_empty();
        let functional = p
            .drawings
            .diagrams
            .iter()
            .any(|d| !d.definition.presentation.is_default());
        let axon = p
            .drawings
            .diagrams
            .iter()
            .any(|d| d.definition.presentation.is_axon());
        let site_figure = p
            .drawings
            .diagrams
            .iter()
            .any(|d| d.definition.presentation.is_site());
        let figure_version = if site_figure {
            14
        } else if axon {
            13
        } else if functional {
            12
        } else {
            11
        };
        if has_figures
            && !has_jobs
            && !has_reports
            && !has_links
            && p.schema_version != figure_version
        {
            return Err(format!(
                "Model figures require native document version {figure_version}"
            ));
        }
        let has_updates = !p.design_process.updates.is_empty();
        if has_updates
            && !has_figures
            && !has_jobs
            && !has_reports
            && !has_links
            && p.schema_version != 10
        {
            return Err("Task updates require native document version 10".into());
        }
        if has_masses
            && !has_updates
            && !has_figures
            && !has_jobs
            && !has_reports
            && !has_links
            && p.schema_version != 9
        {
            return Err("Concept masses require native document version 9".into());
        }
        match p.schema_version {
            18 if has_linked_figures => p.schema_version = crate::precision::MODEL_SCHEMA_VERSION,
            17 if has_links => p.schema_version = crate::precision::MODEL_SCHEMA_VERSION,
            16 if has_reports => p.schema_version = crate::precision::MODEL_SCHEMA_VERSION,
            15 if has_jobs => p.schema_version = crate::precision::MODEL_SCHEMA_VERSION,
            11 | 12 | 13 | 14 if has_figures => {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            10 if has_updates => p.schema_version = crate::precision::MODEL_SCHEMA_VERSION,
            9 if has_masses => p.schema_version = crate::precision::MODEL_SCHEMA_VERSION,
            8 if p.design_process.needs_intent_format() => {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            7 if !p.design_process.is_empty() && !p.design_process.needs_intent_format() => {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            1 | 2
                if p.site.is_unknown()
                    && p.program.is_empty()
                    && p.evidence.is_empty()
                    && p.rules.is_empty()
                    && p.design_process.is_empty() =>
            {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            3 if !p.site.is_unknown()
                && p.program.is_empty()
                && p.evidence.is_empty()
                && p.rules.is_empty()
                && p.design_process.is_empty() =>
            {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            4 if !p.program.is_empty()
                && p.evidence.is_empty()
                && p.rules.is_empty()
                && p.design_process.is_empty() =>
            {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            5 if !p.evidence.is_empty() && p.rules.is_empty() && p.design_process.is_empty() => {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            6 if !p.rules.is_empty() && p.design_process.is_empty() => {
                p.schema_version = crate::precision::MODEL_SCHEMA_VERSION
            }
            _ => return Err("Unsupported native document version or site reference marker".into()),
        }
        if legacy_parameters {
            crate::design_process::parametric::migrate_legacy_states(&mut p);
        }
        p.validate()?;
        Ok(p)
    }
    fn export(&self, p: &Project) -> Result<Vec<u8>, String> {
        p.validate()?;
        let has_masses = p.design_process.needs_mass_format()
            || p.elements
                .iter()
                .any(|e| matches!(e.shape, crate::model::Shape::ConceptMass { .. }));
        if p.drawings.diagrams.is_empty()
            && p.drawings.reports.is_empty()
            && p.job_archive.is_empty()
            && p.drawings.cad_sources.is_empty()
            && !needs_surface_format(p)
            && !has_masses
            && p.site.is_unknown()
            && p.program.is_empty()
            && p.evidence.is_empty()
            && p.rules.is_empty()
            && p.design_process.is_empty()
            && p.trace.is_empty()
        {
            return bounded_project_bytes(serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?);
        }
        let mut value = serde_json::to_value(p).map_err(|e| e.to_string())?;
        if let Some(id) = p.job_archive.project_id() {
            value["_openarchi_document_id"] = serde_json::json!(id);
        }
        value["schema_version"] = serde_json::json!(if !p.trace.is_empty() {
            38
        } else if needs_mass_arc_format(p) {
            37
        } else if crate::cad_reference::needs_extended_format(&p.drawings) {
            36
        } else if !p.drawings.cad_sources.is_empty() {
            35
        } else if needs_surface_format(p) {
            34
        } else {
            previous_native_version(p)
        });
        bounded_project_bytes(serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?)
    }
}
fn previous_native_version(p: &Project) -> u32 {
    if needs_linked_space_task_format(p) {
        33
    } else if needs_reviewed_parametric_inputs_format(p) {
        32
    } else if needs_object_creation_task_format(p) {
        31
    } else if needs_object_task_format(p) {
        30
    } else if needs_agent_reply_format(p) {
        29
    } else if needs_parametric_format(p) {
        28
    } else if !p.evidence.topics.is_empty() {
        26
    } else {
        if p.site.is_unknown()
            && p.program.is_empty()
            && p.evidence.is_empty()
            && p.rules.is_empty()
            && p.design_process.is_empty()
            && p.drawings.diagrams.is_empty()
            && p.drawings.reports.is_empty()
            && p.job_archive.is_empty()
            && !p
                .elements
                .iter()
                .any(|e| matches!(e.shape, crate::model::Shape::ConceptMass { .. }))
        {
            crate::precision::MODEL_SCHEMA_VERSION
        } else {
            native_version_without_topics(p)
        }
    }
}

pub fn new_document_identity() -> String {
    use sha2::{Digest, Sha256};
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    let source = format!(
        "{}-{:?}-{}",
        std::process::id(),
        std::time::SystemTime::now(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
pub fn valid_document_identity(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn save(p: &Project, path: &Path) -> Result<(), String> {
    save_impl(p, path, None, None)
}
pub fn save_document(p: &Project, path: &Path, id: &str) -> Result<(), String> {
    if !valid_document_identity(id) {
        return Err("Invalid document identity".into());
    }
    save_impl(p, path, Some(id), None)
}
pub fn save_recovery_document(
    p: &Project,
    path: &Path,
    id: &str,
    base: Option<&str>,
) -> Result<(), String> {
    if !valid_document_identity(id) {
        return Err("Invalid document identity".into());
    }
    save_impl(p, path, Some(id), base)
}
fn save_impl(
    p: &Project,
    path: &Path,
    document_id: Option<&str>,
    recovery_base: Option<&str>,
) -> Result<(), String> {
    if let Some(id) = document_id {
        p.job_archive.check_project(id)?;
    }
    p.validate()?;
    let mut portable = p.clone();
    if !p.drawings.pages.is_empty() || !p.drawings.cad_sources.is_empty() {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let assets_name = format!(
            "{}.assets",
            path.file_name()
                .ok_or("Invalid project path")?
                .to_string_lossy()
        );
        crate::cad_reference::portable(
            &mut portable.drawings,
            &parent.join(&assets_name),
            Path::new(&assets_name),
        )?;
        for page in &mut portable.drawings.pages {
            let bytes = crate::drawings::verified_bytes(page)?;
            let stored = crate::drawings::store(&bytes, &parent.join(&assets_name))?;
            page.path =
                Path::new(&assets_name).join(stored.file_name().ok_or("Invalid asset path")?);
        }
    }
    let mut bytes = JsonAdapter.export(&portable)?;
    if let Some(id) = document_id {
        let mut document: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        document["_openarchi_document_id"] = serde_json::json!(id);
        document["_openarchi_save_id"] = serde_json::json!(new_document_identity());
        if let Some(base) = recovery_base {
            document["_openarchi_recovery_base"] = serde_json::json!(base);
        }
        bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    }
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project exceeds 32 MB demo limit".into());
    }
    // Each writer owns an exclusively created file in the destination directory.
    // Never truncate a leftover file or follow a pre-existing temporary symlink.
    static NEXT_SAVE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let (tmp, mut file) = loop {
        let sequence = NEXT_SAVE.fetch_add(1, Ordering::Relaxed);
        let tmp = parent.join(format!(
            ".openarchi-save-{}-{sequence}.tmp",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => break (tmp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|error| error.to_string())
}
/// Exact on-disk byte baseline for cooperative conditional saves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileVersion(Option<[u8; 32]>);

impl FileVersion {
    pub fn token(&self) -> Option<String> {
        self.0
            .map(|hash| hash.iter().map(|byte| format!("{byte:02x}")).collect())
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self(None)),
            Err(error) => return Err(error.to_string()),
        };
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        Ok(Self(Some(hash.finalize().into())))
    }
}

/// Serialize participating writers using the same target path, then reject a
/// stale byte baseline. Non-participating writers and path aliases are outside
/// this cooperative protocol. Keep the lock inode: unlinking it would let a new
/// writer acquire a different lock while another process still holds this one.
/// The existing unconditional save API deliberately remains unchanged.
pub fn save_if_unchanged(
    project: &Project,
    path: &Path,
    expected: &FileVersion,
) -> Result<FileVersion, String> {
    save_if_unchanged_impl(project, path, expected, None)
}
pub fn save_document_if_unchanged(
    project: &Project,
    path: &Path,
    expected: &FileVersion,
    id: &str,
) -> Result<FileVersion, String> {
    if !valid_document_identity(id) {
        return Err("Invalid document identity".into());
    }
    save_if_unchanged_impl(project, path, expected, Some(id))
}
fn save_if_unchanged_impl(
    project: &Project,
    path: &Path,
    expected: &FileVersion,
    document_id: Option<&str>,
) -> Result<FileVersion, String> {
    let name = path.file_name().ok_or("Invalid project path")?;
    let mut lock_name = std::ffi::OsString::from(".");
    lock_name.push(name);
    lock_name.push(".openarchi-lock");
    let lock_path = path.with_file_name(lock_name);
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
        .map_err(|error| error.to_string())?;
    lock.lock().map_err(|error| error.to_string())?;
    if &FileVersion::read(path)? != expected {
        return Err("Project file changed since its baseline was read".into());
    }
    save_impl(project, path, document_id, None)?;
    FileVersion::read(path)
}

pub fn load(path: &Path) -> Result<Project, String> {
    load_with_version(path).map(|(project, _)| project)
}

/// Bind the model and baseline to exactly the same bytes, not two disk reads.
pub fn load_with_version(path: &Path) -> Result<(Project, FileVersion), String> {
    load_document(path).map(|(project, version, _)| (project, version))
}
pub fn load_document(path: &Path) -> Result<(Project, FileVersion, Option<String>), String> {
    load_document_state(path).map(|(p, v, id, _)| (p, v, id))
}
pub fn load_document_state(
    path: &Path,
) -> Result<(Project, FileVersion, Option<String>, Option<String>), String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if meta.len() > MAX_PROJECT_BYTES {
        return Err("Project exceeds 32 MB demo limit".into());
    }
    // Metadata can understate a growing file or stream. Read one extra byte to
    // detect overflow without consuming an unbounded source, from this same fd.
    let mut bytes = Vec::new();
    file.take(MAX_PROJECT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project exceeds 32 MB demo limit".into());
    }
    let version = FileVersion(Some(Sha256::digest(&bytes).into()));
    let mut project = JsonAdapter.import(&bytes)?;
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    for page in &mut project.drawings.pages {
        if page.path.is_relative() {
            page.path = absolute
                .parent()
                .ok_or("Invalid project path")?
                .join(&page.path);
        }
    }
    crate::cad_reference::restore(
        &mut project.drawings,
        absolute.parent().ok_or("Invalid project path")?,
    );
    #[derive(serde::Deserialize)]
    struct Header {
        #[serde(rename = "_openarchi_document_id")]
        id: Option<String>,
        #[serde(rename = "_openarchi_recovery_base")]
        recovery_base: Option<String>,
    }
    let header = serde_json::from_slice::<Header>(&bytes).map_err(|e| e.to_string())?;
    let identity = header.id.filter(|id| valid_document_identity(id));
    Ok((project, version, identity, header.recovery_base))
}

fn needs_solar_job_format(p: &Project) -> bool {
    p.job_archive
        .records()
        .iter()
        .any(|r| r.bundle.intent.input.solar.is_some())
}
fn needs_constraint_figure_format(p: &Project) -> bool {
    p.drawings.diagrams.iter().any(|f| {
        matches!(
            f.definition.presentation,
            crate::diagrams::Presentation::ConstraintOverlay
        )
    })
}
fn needs_deferred_choice_format(p: &Project) -> bool {
    p.design_process
        .decisions
        .iter()
        .any(|d| d.reason.trim().is_empty())
}
fn needs_manual_mass_task_format(p: &Project) -> bool {
    p.design_process
        .tasks
        .iter()
        .any(|t| t.is_mass() && t.intent.is_none())
}
fn needs_control_region_format(p: &Project) -> bool {
    let has = |c: &crate::constraints::Check| {
        c.definition.metric == crate::constraints::Metric::ReferenceControlRegionClearance
            || c.report.as_ref().is_some_and(|r| {
                r.input_definition.metric
                    == crate::constraints::Metric::ReferenceControlRegionClearance
            })
    };
    p.site.control_region.is_some()
        || p.rules.checks.iter().any(has)
        || p.drawings.diagrams.iter().any(|f| {
            f.snapshot
                .site
                .as_ref()
                .is_some_and(|s| s.context.control_region.is_some() || s.control_element.is_some())
                || (matches!(
                    f.definition.presentation,
                    crate::diagrams::Presentation::ConstraintOverlay
                ) && crate::diagrams::checks::Source::read(&f.snapshot)
                    .is_ok_and(|s| s.entries.iter().any(|e| has(&e.check))))
        })
}
fn needs_edge_check_format(p: &Project) -> bool {
    let has = |c: &crate::constraints::Check| {
        matches!(
            c.definition.metric,
            crate::constraints::Metric::ReferenceBoundaryEdgeClearance { .. }
        ) || c.report.as_ref().is_some_and(|r| {
            matches!(
                r.input_definition.metric,
                crate::constraints::Metric::ReferenceBoundaryEdgeClearance { .. }
            )
        })
    };
    p.rules.checks.iter().any(has)
        || p.drawings.diagrams.iter().any(|f| {
            matches!(
                f.definition.presentation,
                crate::diagrams::Presentation::ConstraintOverlay
            ) && crate::diagrams::checks::Source::read(&f.snapshot)
                .is_ok_and(|s| s.entries.iter().any(|e| has(&e.check)))
        })
}
fn native_version_without_checks(p: &Project) -> u32 {
    let has_masses = p.design_process.needs_mass_format()
        || p.elements
            .iter()
            .any(|e| matches!(e.shape, crate::model::Shape::ConceptMass { .. }));
    if crate::diagrams::needs_mass_space_figure_format(p) {
        18
    } else if crate::diagrams::needs_mass_space_format(p) {
        17
    } else if !p.drawings.reports.is_empty() {
        16
    } else if !p.job_archive.is_empty() {
        15
    } else if p
        .drawings
        .diagrams
        .iter()
        .any(|d| d.definition.presentation.is_site())
    {
        14
    } else if p
        .drawings
        .diagrams
        .iter()
        .any(|d| d.definition.presentation.is_axon())
    {
        13
    } else if p
        .drawings
        .diagrams
        .iter()
        .any(|d| !d.definition.presentation.is_default())
    {
        12
    } else if !p.drawings.diagrams.is_empty() {
        11
    } else if !p.design_process.updates.is_empty() {
        10
    } else if has_masses {
        9
    } else if !p.design_process.is_empty() {
        if p.design_process.needs_intent_format() {
            8
        } else {
            7
        }
    } else if !p.rules.is_empty() {
        6
    } else if !p.evidence.is_empty() {
        5
    } else if p.program.is_empty() {
        3
    } else {
        4
    }
}

fn native_version_without_topics(p: &Project) -> u32 {
    if needs_manual_mass_task_format(p) {
        25
    } else if needs_control_region_format(p) {
        24
    } else if needs_edge_check_format(p) {
        23
    } else if needs_deferred_choice_format(p) {
        22
    } else if needs_solar_job_format(p) {
        21
    } else if needs_constraint_figure_format(p) {
        20
    } else if !p.rules.checks.is_empty() {
        19
    } else {
        native_version_without_checks(p)
    }
}

fn needs_parametric_format(p: &Project) -> bool {
    p.design_process
        .tasks
        .iter()
        .any(|t| t.intent.as_ref().is_some_and(|v| v.parametric.is_some()))
        || p.design_process
            .updates
            .iter()
            .any(|r| r.parametric_parameters.is_some())
}

fn needs_parameter_state_format(p: &Project) -> bool {
    p.evidence.topics.iter().any(|t| {
        t.exchanges
            .iter()
            .any(|e| e.scope.parameter_input_sha256.is_some())
    }) || !p.design_process.parametric_states.is_empty()
        || p.design_process
            .updates
            .iter()
            .any(|r| r.parametric_parameters.is_some() && r.before_sha256 == r.after_sha256)
        || p.design_process.decisions.iter().any(|d| {
            d.comparison
                .candidates
                .iter()
                .any(|v| v.parametric_parameters.is_some())
        })
}
fn needs_reviewed_parametric_inputs_format(p: &Project) -> bool {
    p.design_process.updates.iter().any(|r| {
        r.sources.iter().any(|s| {
            matches!(
                s,
                crate::design_process::update::Source::ReviewedParametricDefinition { .. }
            )
        })
    }) || p.design_process.decisions.iter().any(|d| {
        d.comparison
            .candidates
            .iter()
            .any(|v| v.parametric_inputs.is_some())
    })
}

fn needs_agent_reply_format(p: &Project) -> bool {
    p.evidence
        .topics
        .iter()
        .any(|t| t.exchanges.iter().any(|e| e.reply.is_some()))
}

fn needs_object_task_format(p: &Project) -> bool {
    p.evidence
        .topics
        .iter()
        .any(|t| t.exchanges.iter().any(|e| e.assignment.is_some()))
}

fn needs_object_creation_task_format(p: &Project) -> bool {
    p.evidence
        .topics
        .iter()
        .flat_map(|t| &t.exchanges)
        .any(|e| {
            e.assignment.as_ref().is_some_and(|a| {
                matches!(
                    a.mode,
                    crate::evidence::topic_tasks::Mode::CreateTwoWing { .. }
                )
            })
        })
}

fn needs_linked_space_task_format(p: &Project) -> bool {
    p.evidence
        .topics
        .iter()
        .flat_map(|t| &t.exchanges)
        .any(|e| {
            e.assignment.as_ref().is_some_and(|a| {
                matches!(
                    a.mode,
                    crate::evidence::topic_tasks::Mode::CreateLinkedSpace { .. }
                )
            })
        })
}

fn needs_surface_format(p: &Project) -> bool {
    fn contains(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                (m.contains_key("outline") && m.contains_key("purpose"))
                    && m.get("surface")
                        .is_some_and(|v| v.get("type").and_then(|v| v.as_str()) == Some("solid"))
                    || m.contains_key("drawing_scale")
                        && m.get("model_3d_style")
                            .is_some_and(|v| v.as_str().is_some_and(|v| v != "shaded_with_edges"))
                    || m.values().any(contains)
            }
            serde_json::Value::Array(a) => a.iter().any(contains),
            _ => false,
        }
    }
    serde_json::to_value(p).is_ok_and(|v| contains(&v))
}

fn needs_mass_arc_format(p: &Project) -> bool {
    p.elements.iter().any(|e| {
        matches!(&e.shape, crate::model::Shape::ConceptMass { spec }
        if !spec.arcs.is_empty() || !spec.hole_arcs.is_empty())
    })
}
