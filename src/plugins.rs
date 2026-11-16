//! Explicit local declarations, not an executable loader or a second model.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub mod archive;
pub mod jobs;
pub mod provider;
pub mod query;
pub mod solar;
pub mod worker;
pub const API_VERSION: u32 = 1;
pub const MAX_BYTES: u64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadAuthoredGeometry,
    ReadProgram,
    ExecuteLocalProgram,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    AuthoredQuantities,
    ZoneRequirements,
    SolarStudy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub operation: Operation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    pub version: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub api_version: u32,
    pub id: String,
    pub version: u32,
    pub label: String,
    pub panel_title: String,
    pub permissions: Vec<Permission>,
    pub dependencies: Vec<Dependency>,
    pub commands: Vec<Command>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<provider::Declaration>,
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
}
fn label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 65536 {
            return Err("Plugin declaration exceeds 64 KiB".into());
        }
        let m: Self = serde_json::from_slice(bytes)
            .map_err(|e| format!("Invalid plugin declaration: {e}"))?;
        m.validate()?;
        Ok(m)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.api_version != API_VERSION || self.version == 0 {
            return Err("Unsupported plugin API or declaration version".into());
        }
        if !identifier(&self.id)
            || !label(&self.label)
            || !label(&self.panel_title)
            || self.commands.is_empty()
            || self.commands.len() > 16
            || self.dependencies.len() > 16
            || self.permissions != self.required_permissions()
            || self
                .provider
                .as_ref()
                .is_some_and(|p| p.protocol_version != 1 || !(100..=30_000).contains(&p.timeout_ms))
        {
            return Err("Invalid plugin identity, labels or read-only permission".into());
        }
        if self
            .commands
            .iter()
            .any(|c| c.operation == Operation::SolarStudy)
            && (self.id != solar::ID || self.provider.is_some() || self.commands.len() != 1)
        {
            return Err("Solar jobs currently require the bundled read-only provider".into());
        }
        let mut seen = BTreeSet::new();
        for c in &self.commands {
            if !identifier(&c.id)
                || !c.id.starts_with(&(self.id.clone() + "."))
                || c.id.len() <= self.id.len() + 1
                || !label(&c.label)
                || !seen.insert(&c.id)
            {
                return Err("Plugin command identity conflicts or has no namespace".into());
            }
        }
        let mut seen = BTreeSet::new();
        for d in &self.dependencies {
            if !identifier(&d.id) || d.id == self.id || d.version == 0 || !seen.insert(&d.id) {
                return Err("Invalid plugin dependency".into());
            }
        }
        Ok(())
    }
    pub fn required_permissions(&self) -> Vec<Permission> {
        let mut p = vec![Permission::ReadAuthoredGeometry];
        if self
            .commands
            .iter()
            .any(|c| c.operation == Operation::ZoneRequirements)
        {
            p.push(Permission::ReadProgram);
        }
        if self.provider.is_some() {
            p.push(Permission::ExecuteLocalProgram);
        }
        p
    }
}
pub fn builtin() -> Manifest {
    Manifest::parse(include_bytes!(
        "../assets/plugins/openarchi-quantities.json"
    ))
    .expect("valid bundled plugin")
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub manifest: Manifest,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<provider::ExecutableGrant>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u32,
    entries: Vec<Registration>,
}
pub struct Registry {
    pub(crate) entries: Vec<Registration>,
    path: Option<PathBuf>,
    baseline: Option<String>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: [builtin(), solar::builtin()]
                .into_iter()
                .map(|manifest| Registration {
                    manifest,
                    enabled: true,
                    executable: None,
                })
                .collect(),
            path: None,
            baseline: None,
        }
    }
}
impl Registry {
    pub fn entries(&self) -> &[Registration] {
        &self.entries
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        if path.is_symlink() {
            return Err("Plugin configuration symlink is unsupported".into());
        }
        if !path.exists() {
            return Ok(Self {
                path: Some(path.to_owned()),
                ..Self::default()
            });
        }
        let bytes = read(path)?;
        let mut c: Configuration = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Plugin configuration is invalid; original preserved: {e}"))?;
        if c.schema_version != 1 {
            return Err("Unsupported plugin configuration version".into());
        }
        if !c.entries.iter().any(|e| e.manifest.id == solar::ID) {
            c.entries.push(Registration {
                manifest: solar::builtin(),
                enabled: true,
                executable: None,
            });
        }
        Self::validate_entries(&c.entries)?;
        Ok(Self {
            entries: c.entries,
            path: Some(path.to_owned()),
            baseline: Some(crate::drawings::digest(&bytes)),
        })
    }
    fn validate_entries(entries: &[Registration]) -> Result<(), String> {
        if entries.len() > 65
            || entries
                .iter()
                .filter(|e| e.manifest.id != solar::ID)
                .count()
                > 64
        {
            return Err("Plugin registration limit reached".into());
        }
        let mut ids = BTreeSet::new();
        let mut commands = BTreeSet::new();
        for e in entries {
            e.manifest.validate()?;
            if e.manifest.provider.is_some() != e.executable.is_some() {
                return Err("External provider requires explicit executable authorization".into());
            }
            if let Some(grant) = &e.executable {
                grant.validate()?;
            }
            if !ids.insert(&e.manifest.id) {
                return Err("Plugin is already registered".into());
            }
            if e.manifest.id == builtin().id && e.manifest != builtin() {
                return Err("Bundled plugin declaration cannot be replaced".into());
            }
            if e.manifest.id == solar::ID && e.manifest != solar::builtin() {
                return Err("Bundled solar declaration cannot be replaced".into());
            }
            for c in &e.manifest.commands {
                if crate::workbench::CommandRegistry::resolve(&c.id).is_some()
                    || !commands.insert(&c.id)
                {
                    return Err("Duplicate plugin command".into());
                }
            }
            if e.enabled {
                for d in &e.manifest.dependencies {
                    if !entries.iter().any(|p| {
                        p.enabled && p.manifest.id == d.id && p.manifest.version == d.version
                    }) {
                        return Err(format!(
                            "Plugin dependency unavailable: {} v{}",
                            d.id, d.version
                        ));
                    }
                }
            }
        }
        fn visit<'a>(
            id: &'a str,
            entries: &'a [Registration],
            active: &mut BTreeSet<&'a str>,
            done: &mut BTreeSet<&'a str>,
        ) -> Result<(), String> {
            if done.contains(id) {
                return Ok(());
            }
            if !active.insert(id) {
                return Err("Cyclic plugin dependencies".into());
            }
            if let Some(e) = entries.iter().find(|e| e.enabled && e.manifest.id == id) {
                for d in &e.manifest.dependencies {
                    visit(&d.id, entries, active, done)?;
                }
            }
            active.remove(id);
            done.insert(id);
            Ok(())
        }
        let mut done = BTreeSet::new();
        for e in entries.iter().filter(|e| e.enabled) {
            visit(&e.manifest.id, entries, &mut BTreeSet::new(), &mut done)?;
        }
        Ok(())
    }
    fn save(&mut self, entries: Vec<Registration>) -> Result<(), String> {
        Self::validate_entries(&entries)?;
        let p = self
            .path
            .as_ref()
            .ok_or("Plugin configuration storage unavailable")?;
        let root = p.parent().ok_or("Invalid plugin configuration path")?;
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let _lock = Lock::acquire(root)?;
        if p.is_symlink() {
            return Err("Plugin configuration symlink is unsupported".into());
        }
        let current = if p.exists() {
            Some(crate::drawings::digest(&read(p)?))
        } else {
            None
        };
        if current != self.baseline {
            return Err("Plugin configuration changed; reopen before editing".into());
        }
        let bytes = serde_json::to_vec_pretty(&Configuration {
            schema_version: 1,
            entries: entries.clone(),
        })
        .map_err(|e| e.to_string())?;
        publish(p, &bytes, true)?;
        self.baseline = Some(crate::drawings::digest(&bytes));
        self.entries = entries;
        Ok(())
    }
    pub fn enable(&mut self, m: Manifest) -> Result<(), String> {
        if m.provider.is_some() {
            return Err("Review and explicitly authorize the local executable first".into());
        }
        self.enable_with_grant(m, None)
    }
    pub fn enable_reviewed_executable(
        &mut self,
        m: Manifest,
        grant: provider::ExecutableGrant,
    ) -> Result<(), String> {
        if m.provider.is_none() {
            return Err("This declaration has no external provider".into());
        }
        m.validate()?;
        grant.validate()?;
        if let Some(index) = self.entries.iter().position(|p| p.manifest.id == m.id) {
            if self.entries[index].manifest != m {
                return Err(
                    "Provider declaration changed; explicit version migration is not supported"
                        .into(),
                );
            }
            // A new explicit native authorization replaces only the local grant.
            // The ordinary enable toggle never renews it; old job intents remain immutable.
            let mut entries = self.entries.clone();
            entries[index].executable = Some(grant);
            entries[index].enabled = true;
            return self.save(entries);
        }
        self.enable_with_grant(m, Some(grant))
    }
    fn enable_with_grant(
        &mut self,
        m: Manifest,
        executable: Option<provider::ExecutableGrant>,
    ) -> Result<(), String> {
        m.validate()?;
        if self.entries.iter().any(|p| p.manifest.id == m.id) {
            return Err("Plugin is already registered; change its enabled state".into());
        }
        let mut e = self.entries.clone();
        e.push(Registration {
            manifest: m,
            enabled: true,
            executable,
        });
        self.save(e)
    }
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let mut entries = self.entries.clone();
        let e = entries
            .iter_mut()
            .find(|p| p.manifest.id == id)
            .ok_or("Plugin is not registered")?;
        e.enabled = enabled;
        self.save(entries)
    }
    pub fn resolve(&self, id: &str) -> Result<(&Manifest, &Command), String> {
        self.entries
            .iter()
            .filter(|p| p.enabled)
            .find_map(|p| {
                p.manifest
                    .commands
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| (&p.manifest, c))
            })
            .ok_or_else(|| "Plugin command is unavailable".into())
    }
    pub fn executable(&self, plugin: &str) -> Option<&provider::ExecutableGrant> {
        self.entries
            .iter()
            .find(|p| p.enabled && p.manifest.id == plugin)
            .and_then(|p| p.executable.as_ref())
    }
}
pub(crate) fn read(p: &Path) -> Result<Vec<u8>, String> {
    let f = fs::File::open(p).map_err(|e| e.to_string())?;
    if f.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES {
        return Err("Plugin artifact exceeds 4 MiB".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    f.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Plugin artifact exceeds 4 MiB".into());
    }
    Ok(bytes)
}
/// Same-directory complete-file publication. Hard-link for new immutable evidence
/// refuses replacement. This is not a cross-file or power-loss transaction.
pub(crate) fn publish(p: &Path, bytes: &[u8], replace: bool) -> Result<(), String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Plugin artifact exceeds 4 MiB".into());
    }
    let root = p.parent().ok_or("Invalid plugin path")?;
    let tmp = root.join(format!(
        ".plugin-{}.tmp",
        crate::storage::new_document_identity()
    ));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        if replace {
            fs::rename(&tmp, p)
        } else {
            fs::hard_link(&tmp, p)
        }
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(tmp);
    result
}
pub(crate) struct Lock {
    path: PathBuf,
    token: String,
}
impl Lock {
    pub fn acquire(root: &Path) -> Result<Self, String> {
        let path = root.join(".plugin-write.lock");
        let token = crate::storage::new_document_identity();
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("Plugin storage busy; preserve and inspect its owner: {e}"))?;
        f.write_all(token.as_bytes()).map_err(|e| e.to_string())?;
        Ok(Self { path, token })
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        if fs::read(&self.path).ok().as_deref() == Some(self.token.as_bytes()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
