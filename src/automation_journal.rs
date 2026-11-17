//! Durable operation evidence, not a second model or an automatic replay engine.
use crate::model::Project;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

// The temporary file has already been flushed and closed. Keep publication
// on the same volume and preserve errors as an unknown operation outcome.
#[cfg(windows)]
fn publish_journal(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let wide = |path: &Path| -> std::io::Result<Vec<u16>> {
        let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        value.push(0);
        Ok(value)
    };
    let from = wide(temporary)?;
    let to = wide(destination)?;
    // MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH; no copy fallback.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(windows))]
fn publish_journal(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(temporary, destination)?;
    File::open(
        destination
            .parent()
            .ok_or(std::io::ErrorKind::InvalidInput)?,
    )?
    .sync_all()
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationIdentity {
    pub caller_id: String,
    pub project_id: String,
    pub document_session: String,
    pub request_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Intent,
    Committed,
    UndoIntent,
    Undone,
    Cancelled,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub identity: OperationIdentity,
    pub payload_hash: String,
    pub before_hash: String,
    pub after_hash: String,
    pub after_image: Project,
    pub phase: Phase,
    pub receipt: Option<serde_json::Value>,
    #[serde(default)]
    pub planned_receipt: Option<serde_json::Value>,
    #[serde(default)]
    pub before_image: Option<Project>,
    #[serde(default)]
    pub batch: Option<crate::model_service::BatchChange>,
    #[serde(default)]
    pub origin_generation: Option<String>,
    #[serde(default)]
    pub undo_generation: Option<String>,
    #[serde(default)]
    pub intent_payload: Option<serde_json::Value>,
    #[serde(default)]
    pub payload_hash_algorithm: Option<String>,
    #[serde(default)]
    pub execution_hash: Option<String>,
    /// Explicit marker makes older deny-unknown-field readers reject rather
    /// than recover an Undo image that would erase immutable design history.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub undo_preserves_design_history: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub native_confirmation: bool,
    /// Older strict readers must not restore a model whose linked figure
    /// dependency semantics they cannot maintain.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mass_space_figure_sources: bool,
}
impl Record {
    pub(crate) fn undo_image(&self) -> Option<Project> {
        let mut before = self.before_image.clone()?;
        if self.undo_preserves_design_history {
            before.design_process = crate::design_process::Journal::restore_with_history(
                &before.design_process,
                &self.after_image.design_process,
            );
        }
        Some(before)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    records: BTreeMap<String, Record>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryMatch {
    BeforeImage,
    AfterImage,
    Conflict,
}
pub fn canonical_hash(value: &impl Serialize) -> Result<String, String> {
    let mut value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    value.sort_all_objects();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value).map_err(|e| e.to_string())?)
    ))
}
pub fn project_hash(project: &Project) -> Result<String, String> {
    canonical_hash(project)
}

fn key(identity: &OperationIdentity) -> Result<String, String> {
    if [
        &identity.caller_id,
        &identity.project_id,
        &identity.document_session,
        &identity.request_id,
    ]
    .iter()
    .any(|s| s.is_empty() || s.len() > 256)
    {
        return Err("INVALID_OPERATION_IDENTITY".into());
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(identity).map_err(|e| e.to_string())?)
    ))
}
pub struct Journal {
    path: PathBuf,
    _lock: File,
    state: State,
    poisoned: bool,
}
impl Journal {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        for p in [&path, path.with_extension("lock").as_path()] {
            if fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err("JOURNAL_SYMLINK".into());
            }
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|_| "JOURNAL_BUSY".to_string())?;
        let state = match fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 64 * 1024 * 1024 {
                    return Err("JOURNAL_SIZE_LIMIT".into());
                }
                let state: State =
                    serde_json::from_slice(&bytes).map_err(|_| "JOURNAL_CORRUPT".to_string())?;
                if state.version != 1 {
                    return Err("JOURNAL_VERSION".into());
                }
                state
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State {
                version: 1,
                ..Default::default()
            },
            Err(e) => return Err(e.to_string()),
        };
        if state.records.len() > 1024 {
            return Err("JOURNAL_CAPACITY".into());
        }
        for (stored_key, record) in &state.records {
            if record.native_confirmation
                != record
                    .intent_payload
                    .as_ref()
                    .is_some_and(|intent| intent["native_confirmation"] == true)
            {
                return Err("JOURNAL_CORRUPT".into());
            }
            if record.undo_preserves_design_history
                && record
                    .before_image
                    .as_ref()
                    .is_none_or(|before| before.design_process == record.after_image.design_process)
            {
                return Err("JOURNAL_CORRUPT".into());
            }
            if let Some(algorithm) = &record.payload_hash_algorithm {
                if algorithm != "sha256-canonical-json" {
                    return Err("JOURNAL_HASH_VERSION".into());
                }
                if record.intent_payload.as_ref().is_none_or(|value| {
                    canonical_hash(value).ok().as_deref() != Some(record.payload_hash.as_str())
                }) {
                    return Err("JOURNAL_CORRUPT".into());
                }
            }
            if let Some(hash) = &record.execution_hash {
                if record.batch.as_ref().is_none_or(|batch| {
                    canonical_hash(batch).ok().as_deref() != Some(hash.as_str())
                }) {
                    return Err("JOURNAL_CORRUPT".into());
                }
            }

            let needs_space_sources =
                crate::diagrams::needs_mass_space_figure_format(&record.after_image)
                    || record
                        .before_image
                        .as_ref()
                        .is_some_and(crate::diagrams::needs_mass_space_figure_format);
            if record.mass_space_figure_sources != needs_space_sources {
                return Err("JOURNAL_UNSUPPORTED_MODEL_FEATURE".into());
            }
            if key(&record.identity)? != *stored_key
                || project_hash(&record.after_image)? != record.after_hash
                || record
                    .after_image
                    .job_archive
                    .check_project(&record.identity.project_id)
                    .is_err()
                || record.before_image.as_ref().is_some_and(|before| {
                    project_hash(before).ok().as_deref() != Some(record.before_hash.as_str())
                        || before
                            .job_archive
                            .check_project(&record.identity.project_id)
                            .is_err()
                })
            {
                return Err("JOURNAL_CORRUPT".into());
            }
        }
        Ok(Self {
            path,
            _lock: lock,
            state,
            poisoned: false,
        })
    }
    fn persist(&mut self, next: State) -> Result<(), String> {
        if self.poisoned {
            return Err("JOURNAL_OUTCOME_UNKNOWN".into());
        }
        let bytes = serde_json::to_vec(&next).map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("JOURNAL_SIZE_LIMIT".into());
        }
        let parent = self.path.parent().ok_or("JOURNAL_PATH")?;
        let temporary = parent.join(format!(
            ".oa-journal-{}-{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let write = (|| -> std::io::Result<()> {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            publish_journal(&temporary, &self.path)?;
            Ok(())
        })();
        if let Err(error) = write {
            self.poisoned = true;
            return Err(format!("JOURNAL_OUTCOME_UNKNOWN: {error}"));
        }
        self.state = next;
        Ok(())
    }
    pub fn begin(
        &mut self,
        identity: OperationIdentity,
        payload_hash: String,
        before: &Project,
        after: &Project,
    ) -> Result<Record, String> {
        self.begin_model(
            identity,
            payload_hash,
            before,
            after,
            None,
            None,
            None,
            None,
        )
    }
    pub fn begin_model(
        &mut self,
        identity: OperationIdentity,
        payload_hash: String,
        before: &Project,
        after: &Project,
        batch: Option<crate::model_service::BatchChange>,
        generation: Option<String>,
        planned_receipt: Option<serde_json::Value>,
        intent_payload: Option<serde_json::Value>,
    ) -> Result<Record, String> {
        if self.poisoned {
            return Err("JOURNAL_OUTCOME_UNKNOWN".into());
        }
        let k = key(&identity)?;
        before.job_archive.check_project(&identity.project_id)?;
        after.job_archive.check_project(&identity.project_id)?;
        let before_hash = project_hash(before)?;
        let after_hash = project_hash(after)?;
        if before_hash == after_hash {
            return Err("NO_CHANGE".into());
        }
        if let Some(record) = self.state.records.get(&k) {
            if record.payload_hash != payload_hash
                || record.before_hash != before_hash
                || record.after_hash != after_hash
            {
                return Err("REQUEST_ID_CONFLICT".into());
            }
            return Ok(record.clone());
        }
        if self.state.records.len() >= 1024 {
            return Err("JOURNAL_CAPACITY".into());
        }
        after
            .validate()
            .map_err(|e| format!("INVALID_AFTER_IMAGE: {e}"))?;
        if intent_payload.as_ref().is_some_and(|value| {
            canonical_hash(value).ok().as_deref() != Some(payload_hash.as_str())
        }) {
            return Err("INVALID_INPUT_FINGERPRINT".into());
        }
        let execution_hash = batch.as_ref().map(canonical_hash).transpose()?;
        let payload_hash_algorithm = intent_payload
            .as_ref()
            .map(|_| "sha256-canonical-json".to_owned());
        let native_confirmation = intent_payload
            .as_ref()
            .is_some_and(|intent| intent["native_confirmation"] == true);
        let record = Record {
            mass_space_figure_sources: crate::diagrams::needs_mass_space_figure_format(before)
                || crate::diagrams::needs_mass_space_figure_format(after),
            identity,
            payload_hash,
            before_hash,
            after_hash,
            after_image: after.clone(),
            phase: Phase::Intent,
            receipt: None,
            planned_receipt,
            before_image: Some(before.clone()),
            batch,
            origin_generation: generation,
            undo_generation: None,
            intent_payload,
            payload_hash_algorithm,
            execution_hash,
            undo_preserves_design_history: before.design_process != after.design_process,
            native_confirmation,
        };
        let mut next = self.state.clone();
        next.records.insert(k, record.clone());
        self.persist(next)?;
        Ok(record)
    }
    pub fn transition(
        &mut self,
        identity: &OperationIdentity,
        phase: Phase,
        receipt: Option<serde_json::Value>,
    ) -> Result<Record, String> {
        if self.poisoned {
            return Err("JOURNAL_OUTCOME_UNKNOWN".into());
        }
        let k = key(identity)?;
        let original = self.state.records.get(&k).ok_or("OPERATION_NOT_FOUND")?;
        if original.phase == phase && original.receipt == receipt {
            return Ok(original.clone());
        }
        let valid = matches!(
            (original.phase, phase),
            (
                Phase::Intent,
                Phase::Committed | Phase::Cancelled | Phase::Unknown
            ) | (
                Phase::Committed,
                Phase::UndoIntent | Phase::Undone | Phase::Unknown
            ) | (Phase::UndoIntent, Phase::Undone | Phase::Unknown)
        );
        if !valid {
            return Err("INVALID_OPERATION_TRANSITION".into());
        }
        if phase == Phase::Committed && receipt.is_none() {
            return Err("COMMIT_RECEIPT_REQUIRED".into());
        }
        let mut record = original.clone();
        record.phase = phase;
        record.receipt = receipt;
        let mut next = self.state.clone();
        next.records.insert(k, record.clone());
        self.persist(next)?;
        Ok(record)
    }
    pub fn begin_undo(
        &mut self,
        identity: &OperationIdentity,
        generation: Option<String>,
    ) -> Result<Record, String> {
        if self.poisoned {
            return Err("JOURNAL_OUTCOME_UNKNOWN".into());
        }
        let k = key(identity)?;
        let mut record = self
            .state
            .records
            .get(&k)
            .ok_or("OPERATION_NOT_FOUND")?
            .clone();
        if record.phase != Phase::Committed {
            return Err("INVALID_OPERATION_TRANSITION".into());
        }
        record.phase = Phase::UndoIntent;
        record.undo_generation = generation;
        let mut next = self.state.clone();
        next.records.insert(k, record.clone());
        self.persist(next)?;
        Ok(record)
    }
    pub fn records(&self) -> Result<Vec<&Record>, String> {
        if self.poisoned {
            return Err("JOURNAL_OUTCOME_UNKNOWN".into());
        }
        Ok(self.state.records.values().collect())
    }
    pub fn get(&self, identity: &OperationIdentity) -> Result<Option<&Record>, String> {
        if self.poisoned {
            return Err("JOURNAL_OUTCOME_UNKNOWN".into());
        }
        Ok(self.state.records.get(&key(identity)?))
    }
    pub fn reconcile(
        &self,
        identity: &OperationIdentity,
        bound_project_id: &str,
        current: &Project,
    ) -> Result<RecoveryMatch, String> {
        if identity.project_id != bound_project_id {
            return Err("PROJECT_MISMATCH".into());
        }
        let record = self.get(identity)?.ok_or("OPERATION_NOT_FOUND")?;
        // get()/load validation still verifies the complete persisted hashes.
        // A saved read-only receipt does not change this operation's model effect.
        Ok(if current.same_model_state(&record.after_image) {
            RecoveryMatch::AfterImage
        } else if (if matches!(record.phase, Phase::UndoIntent | Phase::Undone) {
            record.undo_image()
        } else {
            record.before_image.clone()
        })
        .is_some_and(|before| current.same_model_state(&before))
            || (record.before_image.is_none()
                && !record.undo_preserves_design_history
                && project_hash(current)? == record.before_hash)
        {
            RecoveryMatch::BeforeImage
        } else {
            RecoveryMatch::Conflict
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
