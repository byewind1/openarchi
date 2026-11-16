//! Original configuration proposals. No code execution, model state or automatic apply.
use super::*;
const RECORD_LIMIT: usize = 4 * 1024 * 1024;
const PROPOSAL_LIMIT: usize = 32 * 1024;
const CAPACITY: usize = 32;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub request_id: String,
    pub project_id: String,
    pub expected_snapshot: String,
    pub configuration: Configuration,
}
#[derive(Clone, Debug, Serialize)]
pub struct Inspection {
    pub environment_id: String,
    pub snapshot: String,
    pub configuration: Configuration,
    pub maximum_proposal_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_warning: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
enum State {
    Pending,
    Applying {
        applied: Configuration,
        primary_sha256: String,
    },
    Applied {
        applied: Configuration,
        primary_sha256: String,
    },
    Reverting {
        applied: Configuration,
        applied_sha256: String,
        primary_sha256: String,
    },
    Reverted {
        applied: Configuration,
        applied_sha256: String,
        primary_sha256: String,
    },
    Cancelled,
    Rejected {
        message: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u32,
    environment_id: String,
    sequence: u64,
    request: Request,
    before: Configuration,
    before_primary_sha256: Option<String>,
    state: State,
}
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub request: Request,
    pub sequence: u64,
    pub status: String,
    pub configuration_changed_since_proposal: bool,
    pub difference: ConfigurationDifference,
    pub automatic_apply: bool,
    pub applied_configuration: Option<Configuration>,
    pub current_matches_applied: bool,
    pub message: Option<String>,
    pub intended_configuration: Option<Configuration>,
    pub expected_configuration_observed: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    pub request_id: String,
    pub sequence: u64,
    pub status: String,
    pub mode_name: String,
}
fn hash_config(config: &Configuration) -> Result<String, String> {
    Ok(fingerprint(&config.to_bytes()?))
}
fn valid_id(id: &str) -> bool {
    identifier(id, 128)
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
fn valid_hash(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn request_valid(request: &Request) -> Result<(), String> {
    if request.schema_version != 1
        || !valid_id(&request.request_id)
        || !valid_hash(&request.project_id)
        || !valid_hash(&request.expected_snapshot)
    {
        return Err("Invalid work environment proposal identity".into());
    }
    request.configuration.validate()?;
    if serde_json::to_vec(request)
        .map_err(|e| e.to_string())?
        .len()
        > PROPOSAL_LIMIT
    {
        return Err("Work environment proposal exceeds32 KiB".into());
    }
    Ok(())
}
fn record_read(path: &Path) -> Result<Record, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Work environment proposal must be a regular file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(RECORD_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > RECORD_LIMIT {
        return Err("Work environment proposal record exceeds4 MiB".into());
    }
    let record: Record = serde_json::from_slice(&bytes)
        .map_err(|_| "Work environment proposal record is invalid")?;
    request_valid(&record.request)?;
    record.before.validate()?;
    if path.file_stem().and_then(|x| x.to_str()) != Some(record.request.request_id.as_str())
        || record.before.revision != record.request.configuration.revision
        || fingerprint(
            &serde_json::to_vec(&(&record.before, &record.before_primary_sha256))
                .map_err(|e| e.to_string())?,
        ) != record.request.expected_snapshot
    {
        return Err("Work environment proposal baseline is invalid".into());
    }
    if record.schema_version != 1 || !valid_hash(&record.environment_id) {
        return Err("Work environment proposal record is invalid".into());
    }
    match &record.state {
        State::Applying {
            applied,
            primary_sha256,
        }
        | State::Applied {
            applied,
            primary_sha256,
        } => {
            applied.validate()?;
            if hash_config(applied)? != *primary_sha256
                || applied.revision
                    != record
                        .before
                        .revision
                        .checked_add(1)
                        .ok_or("Work environment revision overflow")?
            {
                return Err("Work environment proposal receipt is invalid".into());
            }
        }
        State::Reverting {
            applied,
            applied_sha256,
            primary_sha256,
        }
        | State::Reverted {
            applied,
            applied_sha256,
            primary_sha256,
        } => {
            applied.validate()?;
            let mut restored = record.before.clone();
            restored.revision = applied
                .revision
                .checked_add(1)
                .ok_or("Work environment revision overflow")?;
            if hash_config(applied)? != *applied_sha256
                || hash_config(&restored)? != *primary_sha256
            {
                return Err("Work environment proposal receipt is invalid".into());
            }
        }
        _ => {}
    }
    Ok(record)
}
fn record_write(path: &Path, record: &Record) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    if bytes.len() > RECORD_LIMIT {
        return Err("Work environment proposal record exceeds4 MiB".into());
    }
    atomic(path, &bytes)
}
impl EnvironmentService {
    pub fn project_proposals(&self, project: &str) -> Result<Vec<Summary>, String> {
        let entries = match fs::read_dir(self.proposal_root()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut result = vec![];
        let mut count = 0;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                count += 1;
                if count > CAPACITY {
                    return Err("Work environment proposal capacity reached".into());
                }
                let record = record_read(&path)?;
                if record.environment_id != self.environment_id()? {
                    return Err("Work environment proposal target mismatch".into());
                }
                if record.request.project_id == project {
                    let name = record
                        .request
                        .configuration
                        .resolve(&record.request.configuration.active)?
                        .name;
                    let view = self.proposal_view(record)?;
                    result.push(Summary {
                        request_id: view.request.request_id,
                        sequence: view.sequence,
                        status: view.status,
                        mode_name: name,
                    });
                }
            }
        }
        result.sort_by_key(|s| s.sequence);
        Ok(result)
    }
    fn environment_id(&self) -> Result<String, String> {
        let path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(&self.path)
        };
        Ok(fingerprint(path.as_os_str().as_encoded_bytes()))
    }
    fn proposal_root(&self) -> PathBuf {
        suffix(&self.path, ".proposals")
    }
    fn proposal_path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Invalid work environment proposal identity".into());
        }
        Ok(self.proposal_root().join(format!("{id}.json")))
    }
    fn proposal_lock(&self) -> Result<fs::File, String> {
        let parent = self
            .path
            .parent()
            .ok_or("Work environment directory required")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let path = suffix(&self.path, ".proposals.lock");
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
            return Err("Work environment proposal lock must be a regular file".into());
        }
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "Work environment proposals are being changed by another process")?;
        let root = self.proposal_root();
        match fs::symlink_metadata(&root) {
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
                return Err("Work environment proposal directory is invalid".into());
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(root).map_err(|e| e.to_string())?;
            }
            Err(e) => return Err(e.to_string()),
        }
        Ok(file)
    }
    pub fn inspect(&self) -> Result<Inspection, String> {
        self.configuration.validate()?;
        if self.recovery.is_some() {
            return Err("Recover or reload the preserved work environment before saving".into());
        }
        if raw(&self.path)?.as_ref().map(|b| fingerprint(b)) != self.baseline {
            return Err("Work environment file changed; reload before saving".into());
        }
        Ok(Inspection {
            environment_id: self.environment_id()?,
            snapshot: fingerprint(
                &serde_json::to_vec(&(&self.configuration, &self.baseline))
                    .map_err(|e| e.to_string())?,
            ),
            configuration: self.configuration.clone(),
            maximum_proposal_bytes: PROPOSAL_LIMIT,
            backup_warning: self.backup_warning.clone(),
        })
    }
    fn original_proposal(&self, id: &str, project: &str) -> Result<Record, String> {
        let record = record_read(&self.proposal_path(id)?)?;
        if record.request.request_id != id
            || record.request.project_id != project
            || record.environment_id != self.environment_id()?
        {
            return Err("Work environment proposal target mismatch".into());
        }
        Ok(record)
    }
    fn proposal_view(&self, record: Record) -> Result<View, String> {
        let primary = raw(&self.path)?.as_ref().map(|b| fingerprint(b));
        let status = match &record.state {
            State::Pending => "pending",
            State::Applied { .. } => "applied",
            State::Reverted { .. } => "reverted",
            State::Cancelled => "cancelled",
            State::Rejected { .. } => "rejected",
            _ => "unknown",
        }
        .to_owned();
        let changed = self
            .inspect()
            .map(|i| i.snapshot != record.request.expected_snapshot)
            .unwrap_or(true);
        let (applied_configuration, current_matches_applied) = match &record.state {
            State::Applied {
                applied,
                primary_sha256,
            } => (
                Some(applied.clone()),
                primary.as_ref() == Some(primary_sha256),
            ),
            State::Reverting {
                applied,
                applied_sha256,
                ..
            }
            | State::Reverted {
                applied,
                applied_sha256,
                ..
            } => (
                Some(applied.clone()),
                primary.as_ref() == Some(applied_sha256),
            ),
            _ => (None, false),
        };
        // Equal bytes prove a state observation, never which writer produced it.
        // A missing completion receipt remains unknown and cannot authorize Undo.
        let (intended_configuration, expected_configuration_observed) = match &record.state {
            State::Applying {
                applied,
                primary_sha256,
            }
            | State::Applied {
                applied,
                primary_sha256,
            } => (
                Some(applied.clone()),
                primary.as_ref() == Some(primary_sha256),
            ),
            State::Reverting { primary_sha256, .. } | State::Reverted { primary_sha256, .. } => {
                (None, primary.as_ref() == Some(primary_sha256))
            }
            _ => (None, false),
        };
        let message = match &record.state {
            State::Rejected { message } => Some(message.clone()),
            _ => None,
        };
        Ok(View {
            difference: record
                .request
                .configuration
                .differences_from(&record.before)?,
            request: record.request,
            sequence: record.sequence,
            status,
            configuration_changed_since_proposal: changed,
            automatic_apply: false,
            intended_configuration,
            expected_configuration_observed,
            applied_configuration,
            current_matches_applied,
            message,
        })
    }
    pub fn read_proposal(&self, id: &str, project: &str) -> Result<View, String> {
        self.proposal_view(self.original_proposal(id, project)?)
    }
    pub fn propose(&self, request: Request) -> Result<View, String> {
        request_valid(&request)?;
        let _lock = self.proposal_lock()?;
        let path = self.proposal_path(&request.request_id)?;
        if fs::symlink_metadata(&path).is_ok() {
            let old = self.original_proposal(&request.request_id, &request.project_id)?;
            if old.request != request {
                return Err("Work environment proposal request ID conflict".into());
            }
            return self.proposal_view(old);
        }
        let inspection = self.inspect()?;
        if inspection.snapshot != request.expected_snapshot
            || inspection.configuration.revision != request.configuration.revision
        {
            return Err("Work environment proposal baseline changed".into());
        }
        let mut maximum = 0;
        let mut count = 0;
        for entry in fs::read_dir(self.proposal_root()).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().is_some_and(|x| x == "json") {
                count += 1;
                if count >= CAPACITY {
                    return Err("Work environment proposal capacity reached".into());
                }
                maximum = maximum.max(record_read(&entry.path())?.sequence);
            }
        }
        let record = Record {
            schema_version: 1,
            environment_id: inspection.environment_id,
            sequence: maximum
                .checked_add(1)
                .ok_or("Work environment proposal sequence overflow")?,
            request,
            before: inspection.configuration,
            before_primary_sha256: self.baseline.clone(),
            state: State::Pending,
        };
        record_write(&path, &record)?;
        self.proposal_view(record)
    }
    pub fn latest_pending_proposal(&self, project: &str) -> Result<Option<View>, String> {
        self.latest_proposal(project, true)
    }
    pub fn latest_project_proposal(&self, project: &str) -> Result<Option<View>, String> {
        self.latest_proposal(project, false)
    }
    fn latest_proposal(&self, project: &str, pending_only: bool) -> Result<Option<View>, String> {
        let root = self.proposal_root();
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut latest: Option<Record> = None;
        let mut count = 0;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|x| x == "json") {
                count += 1;
                if count > CAPACITY {
                    return Err("Work environment proposal capacity reached".into());
                }
                let record = record_read(&path)?;
                if record.environment_id != self.environment_id()? {
                    return Err("Work environment proposal target mismatch".into());
                }
                if record.request.project_id == project
                    && (!pending_only || matches!(record.state, State::Pending))
                    && latest
                        .as_ref()
                        .is_none_or(|old| old.sequence < record.sequence)
                {
                    latest = Some(record);
                }
            }
        }
        latest.map(|record| self.proposal_view(record)).transpose()
    }
    pub fn proposal_draft(&self, id: &str, project: &str) -> Result<Configuration, String> {
        let record = self.original_proposal(id, project)?;
        if !matches!(record.state, State::Pending) {
            return Err(
                "Inspect the original work environment proposal result; do not replay".into(),
            );
        }
        if self.inspect()?.snapshot != record.request.expected_snapshot {
            return Err("Work environment proposal baseline changed".into());
        }
        Ok(record.request.configuration)
    }
    pub fn cancel_proposal(&self, id: &str, project: &str) -> Result<View, String> {
        let _lock = self.proposal_lock()?;
        let mut record = self.original_proposal(id, project)?;
        if matches!(record.state, State::Pending) {
            record.state = State::Cancelled;
            record_write(&self.proposal_path(id)?, &record)?;
        }
        self.proposal_view(record)
    }
    pub fn confirm_proposal(
        &mut self,
        id: &str,
        project: &str,
        draft: Configuration,
    ) -> Result<SaveOutcome, String> {
        let _lock = self.proposal_lock()?;
        let mut record = self.original_proposal(id, project)?;
        if !matches!(record.state, State::Pending) {
            return Err(
                "Inspect the original work environment proposal result; do not replay".into(),
            );
        }
        if self.inspect()?.snapshot != record.request.expected_snapshot
            || draft.revision != record.before.revision
        {
            return Err("Work environment proposal baseline changed".into());
        }
        draft.validate()?;
        let mut applied = draft.clone();
        applied.revision = applied
            .revision
            .checked_add(1)
            .ok_or("Work environment revision overflow")?;
        let primary_sha256 = hash_config(&applied)?;
        record.state = State::Applying {
            applied: applied.clone(),
            primary_sha256: primary_sha256.clone(),
        };
        let path = self.proposal_path(id)?;
        record_write(&path, &record)?;
        match self.save(draft) {
            Ok(mut outcome) => {
                record.state = State::Applied {
                    applied,
                    primary_sha256,
                };
                if record_write(&path, &record).is_err() {
                    outcome.warning=Some("Work environment saved; inspect the original proposal receipt before continuing".into());
                }
                Ok(outcome)
            }
            Err(error) => {
                record.state = State::Rejected {
                    message: error.clone(),
                };
                let _ = record_write(&path, &record);
                Err(error)
            }
        }
    }
    pub fn restore_proposal(&mut self, id: &str, project: &str) -> Result<SaveOutcome, String> {
        let _lock = self.proposal_lock()?;
        let mut record = self.original_proposal(id, project)?;
        let (applied, primary_sha256) = match &record.state {
            State::Applied {
                applied,
                primary_sha256,
            } => (applied, primary_sha256),
            _ => {
                return Err(
                    "Inspect the original work environment proposal result; do not replay".into(),
                );
            }
        };
        if self.configuration != *applied
            || raw(&self.path)?.as_ref().map(|b| fingerprint(b)).as_ref() != Some(primary_sha256)
        {
            return Err("Work environment changed after the proposal; restore refused".into());
        }
        self.inspect()?;
        let original_applied = applied.clone();
        let original_applied_hash = primary_sha256.clone();
        let mut draft = record.before.clone();
        draft.revision = self.configuration.revision;
        let mut restored = draft.clone();
        restored.revision = restored
            .revision
            .checked_add(1)
            .ok_or("Work environment revision overflow")?;
        let primary_sha256 = hash_config(&restored)?;
        record.state = State::Reverting {
            applied: original_applied.clone(),
            applied_sha256: original_applied_hash.clone(),
            primary_sha256: primary_sha256.clone(),
        };
        let path = self.proposal_path(id)?;
        record_write(&path, &record)?;
        let mut outcome = self.save(draft)?;
        record.state = State::Reverted {
            applied: original_applied,
            applied_sha256: original_applied_hash,
            primary_sha256,
        };
        if record_write(&path, &record).is_err() {
            outcome.warning = Some(
                "Work environment saved; inspect the original proposal receipt before continuing"
                    .into(),
            );
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> (PathBuf, EnvironmentService) {
        let root = std::env::temp_dir().join(format!(
            "oa-env-proposal-{}",
            crate::storage::new_document_identity()
        ));
        fs::create_dir(&root).unwrap();
        let service =
            EnvironmentService::open(&root.join("environment.json"), WorkMode::Architecture)
                .unwrap();
        (root, service)
    }
    fn request(service: &EnvironmentService, id: &str) -> Request {
        let inspection = service.inspect().unwrap();
        let mut configuration = inspection.configuration;
        let mut profile = Profile::builtin(WorkMode::SiteConcept);
        profile.id = "user.review".into();
        profile.name = "场地与方案".into();
        profile.builtin_name = false;
        profile.commands = vec![
            "project.site_reference".into(),
            "project.space_options".into(),
            "view.model".into(),
        ];
        configuration.put(profile.layer());
        configuration.active = profile.id;
        Request {
            schema_version: 1,
            request_id: id.into(),
            project_id: "a".repeat(64),
            expected_snapshot: inspection.snapshot,
            configuration,
        }
    }
    #[test]
    fn original_proposal_confirm_reopen_read_and_owned_restore() {
        let (root, mut service) = store();
        let original = service.configuration.clone();
        let request = request(&service, "original");
        let project = request.project_id.clone();
        assert_eq!(service.propose(request.clone()).unwrap().status, "pending");
        assert!(!service.path.exists());
        assert_eq!(service.configuration, original);
        assert_eq!(service.propose(request.clone()).unwrap().sequence, 1);
        let mut conflicting = request.clone();
        conflicting.configuration.active = "openarchi.architecture".into();
        assert!(
            service
                .propose(conflicting)
                .unwrap_err()
                .contains("ID conflict")
        );
        let mut draft = service.proposal_draft("original", &project).unwrap();
        draft.profiles[0].name = Some("用户补充理由后确认".into());
        service
            .confirm_proposal("original", &project, draft)
            .unwrap();
        drop(service);
        let mut reopened =
            EnvironmentService::open(&root.join("environment.json"), WorkMode::Architecture)
                .unwrap();
        let view = reopened.read_proposal("original", &project).unwrap();
        assert_eq!(view.status, "applied");
        assert!(view.current_matches_applied);
        assert_ne!(
            view.applied_configuration.as_ref().unwrap().profiles[0].name,
            view.request.configuration.profiles[0].name
        );
        let committed = fs::read(reopened.path()).unwrap();
        assert!(
            reopened
                .confirm_proposal("original", &project, request.configuration.clone())
                .is_err()
        );
        assert_eq!(fs::read(reopened.path()).unwrap(), committed);
        reopened.restore_proposal("original", &project).unwrap();
        let view = reopened.read_proposal("original", &project).unwrap();
        assert_eq!(view.status, "reverted");
        assert!(view.applied_configuration.is_some());
        let mut restored = reopened.configuration.clone();
        restored.revision = original.revision;
        assert_eq!(restored, original);
        assert!(reopened.restore_proposal("original", &project).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn user_config_edits_never_renew_pending_or_applied_authority() {
        let (root, mut service) = store();
        let original = request(&service, "pending");
        let project = original.project_id.clone();
        service.propose(original.clone()).unwrap();
        let mut user = service.configuration.clone();
        user.active = "openarchi.site-concept".into();
        service.save(user).unwrap();
        let bytes = fs::read(service.path()).unwrap();
        assert!(service.proposal_draft("pending", &project).is_err());
        assert!(
            service
                .confirm_proposal("pending", &project, original.configuration.clone())
                .is_err()
        );
        let fresh = service.inspect().unwrap();
        assert_ne!(fresh.snapshot, original.expected_snapshot);
        assert!(
            service
                .propose(original.clone())
                .unwrap()
                .configuration_changed_since_proposal
        );
        assert_eq!(fs::read(service.path()).unwrap(), bytes);
        let next = request(&service, "next");
        service.propose(next.clone()).unwrap();
        service
            .confirm_proposal("next", &project, next.configuration)
            .unwrap();
        let mut user = service.configuration.clone();
        user.profiles[0].name = Some("用户后来改名".into());
        service.save(user).unwrap();
        let bytes = fs::read(service.path()).unwrap();
        assert!(
            !service
                .read_proposal("next", &project)
                .unwrap()
                .current_matches_applied
        );
        assert!(
            service
                .restore_proposal("next", &project)
                .unwrap_err()
                .contains("restore refused")
        );
        assert_eq!(fs::read(service.path()).unwrap(), bytes);
        assert!(service.read_proposal("next", &"b".repeat(64)).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_confirmation_is_observed_without_replay() {
        let (root, mut service) = store();
        let original = request(&service, "interrupted");
        let project = original.project_id.clone();
        service.propose(original.clone()).unwrap();
        let mut record = service.original_proposal("interrupted", &project).unwrap();
        let mut applied = original.configuration.clone();
        applied.revision += 1;
        record.state = State::Applying {
            primary_sha256: hash_config(&applied).unwrap(),
            applied,
        };
        record_write(&service.proposal_path("interrupted").unwrap(), &record).unwrap();
        assert_eq!(
            service
                .read_proposal("interrupted", &project)
                .unwrap()
                .status,
            "unknown"
        );
        assert_eq!(
            service
                .cancel_proposal("interrupted", &project)
                .unwrap()
                .status,
            "unknown"
        );
        assert!(
            service
                .confirm_proposal("interrupted", &project, original.configuration.clone())
                .is_err()
        );
        assert!(!service.path.exists());
        // Protected fixture representing the existing write having happened but
        // its reply/result record lost. Recovery never repeats this write.
        service.save(original.configuration).unwrap();
        drop(service);
        let mut reopened =
            EnvironmentService::open(&root.join("environment.json"), WorkMode::Architecture)
                .unwrap();
        let before = fs::read(reopened.path()).unwrap();
        assert_eq!(
            reopened
                .read_proposal("interrupted", &project)
                .unwrap()
                .status,
            "unknown"
        );
        assert_eq!(fs::read(reopened.path()).unwrap(), before);
        let view = reopened.read_proposal("interrupted", &project).unwrap();
        assert!(view.expected_configuration_observed);
        assert!(view.applied_configuration.is_none());
        assert!(reopened.restore_proposal("interrupted", &project).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_proposals_cancel_and_corruption_preserve_originals() {
        let (root, service) = store();
        for n in 0..CAPACITY {
            service
                .propose(request(&service, &format!("request-{n}")))
                .unwrap();
        }
        assert_eq!(
            service
                .latest_pending_proposal(&"a".repeat(64))
                .unwrap()
                .unwrap()
                .sequence,
            CAPACITY as u64
        );
        assert!(
            service
                .propose(request(&service, "overflow"))
                .unwrap_err()
                .contains("capacity")
        );
        assert_eq!(
            service
                .propose(request(&service, "request-0"))
                .unwrap()
                .sequence,
            1
        );
        assert_eq!(
            service
                .cancel_proposal("request-0", &"a".repeat(64))
                .unwrap()
                .status,
            "cancelled"
        );
        let path = service.proposal_path("request-1").unwrap();
        let mut record = record_read(&path).unwrap();
        record.before.active = "openarchi.site-concept".into();
        record_write(&path, &record).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(service.read_proposal("request-1", &"a".repeat(64)).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!service.path.exists());
        fs::remove_dir_all(root).unwrap();
        let (root, service) = store();
        let mut oversized = request(&service, "too-large");
        oversized.configuration.profiles[0].commands = Some(
            (0..128)
                .map(|i| format!("{i:03}.{}", "x".repeat(248)))
                .collect(),
        );
        assert!(service.propose(oversized).unwrap_err().contains("32 KiB"));
        assert!(!service.path.exists());
        assert!(!service.proposal_root().exists());
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod ownership_counterexample {
    use super::*;
    #[test]
    fn equal_target_bytes_from_another_writer_do_not_prove_original_apply_ownership() {
        let root = std::env::temp_dir().join(format!(
            "oa-env-other-writer-{}",
            crate::storage::new_document_identity()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("environment.json");
        let service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
        let inspection = service.inspect().unwrap();
        let mut config = inspection.configuration;
        config.active = "openarchi.site-concept".into();
        let request = Request {
            schema_version: 1,
            request_id: "unknown-original".into(),
            project_id: "a".repeat(64),
            expected_snapshot: inspection.snapshot,
            configuration: config.clone(),
        };
        service.propose(request).unwrap();
        let mut record = service
            .original_proposal("unknown-original", &"a".repeat(64))
            .unwrap();
        let mut expected = config.clone();
        expected.revision += 1;
        record.state = State::Applying {
            applied: expected.clone(),
            primary_sha256: hash_config(&expected).unwrap(),
        };
        record_write(&service.proposal_path("unknown-original").unwrap(), &record).unwrap();
        let mut unrelated = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
        unrelated.save(config).unwrap();
        drop(unrelated);
        let mut reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
        let before = fs::read(&path).unwrap();
        assert_eq!(
            reopened
                .read_proposal("unknown-original", &"a".repeat(64))
                .unwrap()
                .status,
            "unknown",
            "matching bytes do not prove which operation wrote them"
        );
        assert!(
            reopened
                .restore_proposal("unknown-original", &"a".repeat(64))
                .is_err(),
            "unknown ownership must not undo the other writer"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }
}
