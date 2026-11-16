//! Personal configuration layers. No Editor or Project dependency.
use super::{CommandRegistry, WorkMode, WorkbenchCommand, workspace::NativeWorkspace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
const LIMIT: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Panels {
    pub toolbox: bool,
    pub navigator: bool,
    pub info_box: bool,
    pub quick_options: bool,
    pub quick_palette: bool,
    #[serde(default = "visible", skip_serializing_if = "is_visible")]
    pub standard_toolbar: bool,
    #[serde(default = "visible", skip_serializing_if = "is_visible")]
    pub edit_toolbar: bool,
}
fn visible() -> bool {
    true
}
fn is_visible(value: &bool) -> bool {
    *value
}
impl Default for Panels {
    fn default() -> Self {
        Self {
            toolbox: true,
            navigator: true,
            info_box: true,
            quick_options: true,
            quick_palette: false,
            standard_toolbar: true,
            edit_toolbar: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileLayer {
    pub id: String,
    pub base_mode: WorkMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_menu: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<Panels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<NativeWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<Vec<super::shortcuts::Binding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_presets: Option<super::creation_presets::CreationPresets>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema_version: u32,
    pub revision: u64,
    pub active: String,
    pub profiles: Vec<ProfileLayer>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub base_mode: WorkMode,
    pub commands: Vec<String>,
    #[serde(skip_serializing_if = "is_default_project_menu")]
    pub project_menu: Vec<String>,
    pub panels: Panels,
    pub workspace: NativeWorkspace,
    pub shortcuts: Vec<super::shortcuts::Binding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_presets: Option<super::creation_presets::CreationPresets>,
    pub customized: bool,
    pub builtin_name: bool,
}
/// Differences include the personal layers, even when their resolved values
/// currently match defaults: an explicit override has different upgrade semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProfileDifference {
    pub id: String,
    pub before: Option<Profile>,
    pub after: Option<Profile>,
    pub before_layer: Option<ProfileLayer>,
    pub after_layer: Option<ProfileLayer>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfigurationDifference {
    pub active_before: String,
    pub active_after: String,
    pub profiles: Vec<ProfileDifference>,
}
fn is_default_project_menu(commands: &[String]) -> bool {
    commands
        .iter()
        .map(String::as_str)
        .eq(WorkbenchCommand::PROJECT_MENU.iter().map(|c| c.id()))
}
impl Profile {
    pub fn builtin(mode: WorkMode) -> Self {
        let p = mode.profile();
        Self {
            id: p.id,
            name: mode.label().into(),
            base_mode: mode,
            commands: p.commands.iter().map(|c| c.id().into()).collect(),
            project_menu: WorkbenchCommand::PROJECT_MENU
                .iter()
                .map(|c| c.id().into())
                .collect(),
            panels: Panels::default(),
            workspace: NativeWorkspace::default(),
            shortcuts: vec![],
            creation_presets: None,
            customized: false,
            builtin_name: true,
        }
    }
    pub fn layer(&self) -> ProfileLayer {
        let builtin = Self::builtin(self.base_mode);
        ProfileLayer {
            id: self.id.clone(),
            base_mode: self.base_mode,
            name: (!self.builtin_name || self.name != builtin.name).then(|| self.name.clone()),
            commands: (self.commands != builtin.commands).then(|| self.commands.clone()),
            project_menu: (self.project_menu != builtin.project_menu)
                .then(|| self.project_menu.clone()),
            panels: (self.panels != builtin.panels).then_some(self.panels),
            workspace: (self.workspace != builtin.workspace).then(|| self.workspace.clone()),
            shortcuts: (!self.shortcuts.is_empty()).then(|| self.shortcuts.clone()),
            creation_presets: self.creation_presets.clone(),
        }
    }
    pub fn command(&self, id: &str) -> Option<WorkbenchCommand> {
        CommandRegistry::resolve(id)
    }
}
pub(super) fn identifier(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}
pub fn defaults() -> Vec<Profile> {
    WorkMode::ALL
        .iter()
        .copied()
        .map(Profile::builtin)
        .collect()
}
impl Configuration {
    pub fn differences_from(&self, baseline: &Self) -> Result<ConfigurationDifference, String> {
        self.validate()?;
        baseline.validate()?;
        let ids = self
            .profiles()
            .into_iter()
            .chain(baseline.profiles())
            .map(|p| p.id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut profiles = Vec::new();
        for id in ids {
            let effective_layer = |config: &Self| {
                config
                    .profiles
                    .iter()
                    .find(|p| p.id == id)
                    .filter(|p| {
                        p.id.starts_with("user.")
                            || p.name.is_some()
                            || p.commands.is_some()
                            || p.project_menu.is_some()
                            || p.panels.is_some()
                            || p.workspace.is_some()
                            || p.shortcuts.is_some()
                            || p.creation_presets.is_some()
                    })
                    .cloned()
            };
            let before_layer = effective_layer(baseline);
            let after_layer = effective_layer(self);
            let mut before = baseline.resolve(&id).ok();
            let mut after = self.resolve(&id).ok();
            // An empty builtin layer overrides nothing. Ignore its storage-only
            // customized flag in the view, without changing either saved config.
            if let Some(p) = &mut before {
                p.customized = before_layer.is_some();
            }
            if let Some(p) = &mut after {
                p.customized = after_layer.is_some();
            }
            if before != after || before_layer != after_layer {
                profiles.push(ProfileDifference {
                    id,
                    before,
                    after,
                    before_layer,
                    after_layer,
                });
            }
        }
        Ok(ConfigurationDifference {
            active_before: baseline.active.clone(),
            active_after: self.active.clone(),
            profiles,
        })
    }
    pub fn new(mode: WorkMode) -> Self {
        Self {
            schema_version: 2,
            revision: 0,
            active: mode.profile().id,
            profiles: vec![],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 2 {
            return Err("Unsupported work environment version".into());
        }
        if self.profiles.len() > 32 {
            return Err("Work environment exceeds32 personal layers".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for p in &self.profiles {
            if let Some(menu) = &p.project_menu {
                let mut seen = std::collections::BTreeSet::new();
                if menu.len() > 128
                    || menu
                        .iter()
                        .any(|id| !identifier(id, 256) || !seen.insert(id))
                    || WorkbenchCommand::PROJECT_MENU
                        .iter()
                        .any(|c| !menu.iter().any(|id| id == c.id()))
                {
                    return Err(
                        "Project menu must retain unique valid commands and its required entries"
                            .into(),
                    );
                }
            }
            if let Some(presets) = &p.creation_presets {
                presets.validate()?;
            }
            if let Some(shortcuts) = &p.shortcuts {
                super::shortcuts::validate(shortcuts)?;
            }
            if let Some(workspace) = &p.workspace {
                workspace.validate()?;
            }
            if !identifier(&p.id, 128)
                || !ids.insert(&p.id)
                || (!p.id.starts_with("user.") && p.id != p.base_mode.profile().id)
            {
                return Err("Invalid or duplicate work environment profile ID".into());
            }
            if p.name.as_ref().is_some_and(|s| {
                s.trim().is_empty() || s.len() > 256 || s.chars().any(char::is_control)
            }) || (p.id.starts_with("user.") && p.name.is_none())
            {
                return Err("Personal work environment needs a valid name".into());
            }
            if let Some(commands) = &p.commands {
                let mut seen = std::collections::BTreeSet::new();
                if commands.len() > 128
                    || commands
                        .iter()
                        .any(|s| !identifier(s, 256) || !seen.insert(s))
                {
                    return Err("Invalid or duplicate work environment command IDs".into());
                }
            }
        }
        if !defaults().iter().any(|p| p.id == self.active)
            && !self.profiles.iter().any(|p| p.id == self.active)
        {
            return Err("Active work environment profile is missing".into());
        }
        Ok(())
    }
    pub fn resolve_with_defaults(&self, id: &str, defaults: &[Profile]) -> Result<Profile, String> {
        let layer = self.profiles.iter().find(|p| p.id == id);
        let mut p = if let Some(l) = layer {
            defaults
                .iter()
                .find(|p| p.base_mode == l.base_mode)
                .cloned()
                .ok_or("Work environment base mode is unavailable")?
        } else {
            defaults
                .iter()
                .find(|p| p.id == id)
                .cloned()
                .ok_or("Work environment profile is missing")?
        };
        if let Some(l) = layer {
            p.id = l.id.clone();
            p.customized = true;
            if let Some(name) = &l.name {
                p.name = name.clone();
                p.builtin_name = false;
            }
            if let Some(commands) = &l.commands {
                p.commands = commands.clone()
            }
            if let Some(menu) = &l.project_menu {
                p.project_menu = menu.clone();
            }
            if let Some(shortcuts) = &l.shortcuts {
                p.shortcuts = shortcuts.clone();
            }
            if let Some(presets) = &l.creation_presets {
                p.creation_presets = Some(presets.clone());
            }
            if let Some(workspace) = &l.workspace {
                p.workspace = workspace.clone();
            }
            if let Some(panels) = l.panels {
                p.panels = panels
            }
        }
        Ok(p)
    }
    pub fn resolve(&self, id: &str) -> Result<Profile, String> {
        self.resolve_with_defaults(id, &defaults())
    }
    pub fn profiles(&self) -> Vec<Profile> {
        let mut ids = defaults().into_iter().map(|p| p.id).collect::<Vec<_>>();
        ids.extend(
            self.profiles
                .iter()
                .filter(|p| p.id.starts_with("user."))
                .map(|p| p.id.clone()),
        );
        ids.into_iter()
            .filter_map(|id| self.resolve(&id).ok())
            .collect()
    }
    pub fn put(&mut self, layer: ProfileLayer) {
        if let Some(p) = self.profiles.iter_mut().find(|p| p.id == layer.id) {
            *p = layer
        } else {
            self.profiles.push(layer)
        }
    }
    /// Apply the existing profile editor's changes without normalizing untouched
    /// imported overrides into inheritance (including explicit default values).
    pub fn apply_profile_draft(&mut self, profile: &Profile) {
        self.active = profile.id.clone();
        let before = self.resolve(&profile.id).ok();
        if before.as_ref() == Some(profile) {
            return;
        }
        let previous = self.profiles.iter().find(|p| p.id == profile.id).cloned();
        let mut next = profile.layer();
        if let Some(before) = before.filter(|p| p.base_mode == profile.base_mode) {
            if before.name == profile.name && before.builtin_name == profile.builtin_name {
                next.name = previous.as_ref().and_then(|p| p.name.clone());
            }
            if before.commands == profile.commands {
                next.commands = previous.as_ref().and_then(|p| p.commands.clone());
            }
            if before.project_menu == profile.project_menu {
                next.project_menu = previous.as_ref().and_then(|p| p.project_menu.clone());
            }
            if before.panels == profile.panels {
                next.panels = previous.as_ref().and_then(|p| p.panels);
            }
            if before.shortcuts == profile.shortcuts {
                next.shortcuts = previous.as_ref().and_then(|p| p.shortcuts.clone());
            }
            if before.creation_presets == profile.creation_presets {
                next.creation_presets = previous.as_ref().and_then(|p| p.creation_presets.clone());
            }
            if before.workspace == profile.workspace {
                next.workspace = previous.as_ref().and_then(|p| p.workspace.clone());
            }
        }
        self.put(next);
    }
    /// Single on-disk encoding shared by saves, export and proposal recovery hashes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        let menu_toolbars = self.profiles.iter().any(|p| {
            p.project_menu.is_some()
                || p.panels
                    .is_some_and(|p| !p.standard_toolbar || !p.edit_toolbar)
        });
        let extended = menu_toolbars || self.profiles.iter().any(|p| p.creation_presets.is_some());
        if menu_toolbars {
            value["schema_version"] = serde_json::json!(5);
        } else if extended {
            value["schema_version"] = serde_json::json!(4);
        } else if self.profiles.iter().any(|p| p.shortcuts.is_some()) {
            value["schema_version"] = serde_json::json!(3);
        }
        // Keep legacy field order/bytes stable for existing operation hashes.
        let bytes = if extended || self.profiles.iter().any(|p| p.shortcuts.is_some()) {
            serde_json::to_vec_pretty(&value)
        } else {
            serde_json::to_vec_pretty(self)
        }
        .map_err(|e| e.to_string())?;
        if bytes.len() > LIMIT {
            return Err("Work environment file exceeds1 MiB".into());
        }
        Ok(bytes)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > LIMIT {
            return Err("Work environment file exceeds1 MiB".into());
        }
        let mut value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "Work environment file is invalid")?;
        if matches!(value["schema_version"].as_u64(), Some(1 | 2 | 3 | 4))
            && value["profiles"].as_array().is_some_and(|p| {
                p.iter().any(|p| {
                    p.get("project_menu").is_some()
                        || p.get("panels").is_some_and(|p| {
                            p.get("standard_toolbar").is_some() || p.get("edit_toolbar").is_some()
                        })
                })
            })
        {
            return Err("Menu and toolbar overrides require work environment version5".into());
        }
        if matches!(value["schema_version"].as_u64(), Some(1 | 2 | 3))
            && value["profiles"]
                .as_array()
                .is_some_and(|p| p.iter().any(|p| p.get("creation_presets").is_some()))
        {
            return Err("Creation presets require work environment version4".into());
        }
        if matches!(value["schema_version"].as_u64(), Some(1 | 2))
            && value["profiles"]
                .as_array()
                .is_some_and(|p| p.iter().any(|p| p.get("shortcuts").is_some()))
        {
            return Err("Shortcut overrides require work environment version3".into());
        }
        match value["schema_version"].as_u64() {
            Some(1) => {
                value["schema_version"] = serde_json::json!(2);
            }
            Some(2) => {
                if value["profiles"]
                    .as_array()
                    .is_some_and(|p| p.iter().any(|p| p.get("shortcuts").is_some()))
                {
                    return Err("Shortcut overrides require work environment version3".into());
                }
            }
            Some(3 | 4 | 5) => {
                value["schema_version"] = serde_json::json!(2);
            }
            _ => return Err("Unsupported work environment version".into()),
        }
        let config: Self =
            serde_json::from_value(value).map_err(|_| "Work environment file is invalid")?;
        config.validate()?;
        Ok(config)
    }
}
fn fingerprint(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn raw(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Work environment path must be a regular file".into());
    }
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    Read::by_ref(&mut file)
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err("Work environment file exceeds1 MiB".into());
    }
    Ok(Some(bytes))
}
fn suffix(path: &Path, name: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(name);
    PathBuf::from(value)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Work environment directory required")?;
    let temp = parent.join(format!(
        ".environment-{}.tmp",
        crate::storage::new_document_identity()
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
#[derive(Clone, Debug)]
pub struct SaveOutcome {
    pub revision: u64,
    pub warning: Option<String>,
    pub preserved_file: Option<PathBuf>,
    pub preserved_backup_file: Option<PathBuf>,
}
pub struct EnvironmentService {
    path: PathBuf,
    pub configuration: Configuration,
    baseline: Option<String>,
    pub recovery: Option<String>,
    /// Last observed backup read/validation failure; never makes a valid primary unusable.
    pub backup_warning: Option<String>,
    pub existed: bool,
}
impl EnvironmentService {
    pub fn open(path: &Path, legacy_mode: WorkMode) -> Result<Self, String> {
        let bytes = raw(path)?;
        let baseline = bytes.as_ref().map(|b| fingerprint(b));
        let backup = raw(&suffix(path, ".last-good.json"))
            .and_then(|bytes| bytes.map(|b| Configuration::parse(&b)).transpose());
        let backup_warning = backup.as_ref().err().cloned();
        let (configuration, recovery) = if let Some(b) = &bytes {
            match Configuration::parse(b) {
                Ok(c) => (c, None),
                Err(e) => {
                    let good = backup
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| Configuration::new(legacy_mode));
                    (good, Some(e))
                }
            }
        } else {
            (Configuration::new(legacy_mode), None)
        };
        Ok(Self {
            path: path.into(),
            configuration,
            baseline,
            recovery,
            backup_warning,
            existed: bytes.is_some(),
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn save(&mut self, draft: Configuration) -> Result<SaveOutcome, String> {
        if self.recovery.is_some() {
            return Err("Recover or reload the preserved work environment before saving".into());
        }
        self.persist(draft, false)
    }
    fn persist(&mut self, mut draft: Configuration, recover: bool) -> Result<SaveOutcome, String> {
        draft.validate()?;
        if draft.revision != self.configuration.revision {
            return Err("Work environment revision changed; reload before saving".into());
        }
        // Reject known-invalid output before touching previous settings or recovery archives.
        draft.revision = draft
            .revision
            .checked_add(1)
            .ok_or("Work environment revision overflow")?;
        let bytes = draft.to_bytes()?;
        if bytes.len() > LIMIT {
            return Err("Work environment file exceeds1 MiB".into());
        }
        let parent = self
            .path
            .parent()
            .ok_or("Work environment directory required")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let lock_path = suffix(&self.path, ".lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Work environment lock path is a symlink".into());
        }
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let owner = options.open(&lock_path).map_err(|e| e.to_string())?;
        owner
            .try_lock()
            .map_err(|_| "Work environment is being changed by another process")?;
        let current = raw(&self.path)?;
        if current.as_ref().map(|b| fingerprint(b)) != self.baseline {
            return Err("Work environment file changed; reload before saving".into());
        }
        let mut preserved = None;
        let mut preserved_backup = None;
        if recover && let Some(bytes) = &current {
            let archive = parent.join(format!(
                "work-environment-preserved-{}.json",
                crate::storage::new_document_identity()
            ));
            atomic(&archive, bytes)?;
            preserved = Some(archive);
        }
        if let Some(bytes) = &current
            && Configuration::parse(bytes).is_ok()
        {
            atomic(&suffix(&self.path, ".previous.json"), bytes)?;
        }
        let backup_path = suffix(&self.path, ".last-good.json");
        let backup = raw(&backup_path);
        if let Ok(Some(old)) = &backup
            && (recover || Configuration::parse(old).is_err())
        {
            let archive = parent.join(format!(
                "work-environment-backup-preserved-{}.json",
                crate::storage::new_document_identity()
            ));
            atomic(&archive, old)?;
            preserved_backup = Some(archive);
        }
        atomic(&self.path, &bytes)?;
        self.configuration = draft;
        self.baseline = Some(fingerprint(&bytes));
        self.recovery = None;
        self.existed = true;
        // An unreadable/oversized/non-regular backup is preserved in place. In
        // particular rename must not replace a symlink whose target was not read.
        self.backup_warning = match backup {
            Ok(_) => atomic(&backup_path, &bytes).err(),
            Err(error) => Some(error),
        };
        let warning = self
            .backup_warning
            .as_ref()
            .map(|_| "Work environment saved; valid backup could not be updated".into());
        Ok(SaveOutcome {
            revision: self.configuration.revision,
            warning,
            preserved_file: preserved,
            preserved_backup_file: preserved_backup,
        })
    }
    pub fn restore_valid(&mut self) -> Result<SaveOutcome, String> {
        let mut config = raw(&suffix(&self.path, ".last-good.json"))?
            .map(|b| Configuration::parse(&b))
            .transpose()?
            .ok_or("No valid work environment backup exists")?;
        config.revision = self.configuration.revision;
        self.persist(config, true)
    }
    /// Explicit recovery only; archives the original bytes before replacing them.
    pub fn restore_defaults(&mut self, mode: WorkMode) -> Result<SaveOutcome, String> {
        let mut config = Configuration::new(mode);
        config.revision = self.configuration.revision;
        self.persist(config, self.recovery.is_some())
    }
    pub fn restore_previous(&mut self) -> Result<SaveOutcome, String> {
        if self.recovery.is_some() {
            return Err(
                "Recover the preserved configuration before restoring previous settings".into(),
            );
        }
        let mut config = raw(&suffix(&self.path, ".previous.json"))?
            .map(|b| Configuration::parse(&b))
            .transpose()?
            .ok_or("No previous work environment exists")?;
        config.revision = self.configuration.revision;
        self.persist(config, false)
    }
    /// Reads a proposal into the existing editable draft. It grants no authority
    /// and never writes or refreshes this service's saved-file baseline.
    pub fn preview_import(&self, path: &Path) -> Result<Configuration, String> {
        let mut config =
            Configuration::parse(&raw(path)?.ok_or("Work environment import file is missing")?)?;
        config.revision = self.configuration.revision;
        Ok(config)
    }
    /// Explicit programmatic apply retained for existing callers. GUI import
    /// uses preview_import and the normal draft Save/Cancel controls instead.
    pub fn import(&mut self, path: &Path) -> Result<SaveOutcome, String> {
        self.save(self.preview_import(path)?)
    }
    pub fn export(&self, path: &Path) -> Result<(), String> {
        if fs::symlink_metadata(path).is_ok() {
            return Err("Work environment export already exists".into());
        }
        let bytes = self.configuration.to_bytes()?;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(e.to_string());
        }
        Ok(())
    }
}

#[path = "environment_proposal.rs"]
pub mod proposal;
