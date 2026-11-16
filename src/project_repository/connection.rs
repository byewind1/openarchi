//! Local, previously authorized source-folder connections. Never loads a model
//! or obtains write authority from a native document's contents.
use super::Repository;
use crate::storage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_BYTES: usize = 16 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    schema_version: u32,
    document_id: String,
    project: PathBuf,
    version: String,
    repository: PathBuf,
}
fn ordinary(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            Err("Source connection must be a regular file".into())
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
fn canonical(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    let resolved = absolute.canonicalize().map_err(|e| e.to_string())?;
    if fs::symlink_metadata(&absolute).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Source connection path contains a symlink".into());
    }
    Ok(resolved)
}
fn location(directory: &Path, project: &Path, id: &str) -> Result<PathBuf, String> {
    if !storage::valid_document_identity(id) {
        return Err("Invalid source connection document identity".into());
    }
    let key = serde_json::to_vec(&(project, id)).map_err(|e| e.to_string())?;
    Ok(directory.join(format!("{:x}.json", Sha256::digest(key))))
}
fn read(path: &Path) -> Result<Option<Connection>, String> {
    ordinary(path)?;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Source connection is too large".into());
    }
    let value: Connection = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value.schema_version != 1
        || !storage::valid_document_identity(&value.document_id)
        || !storage::valid_document_identity(&value.version)
        || !value.project.is_absolute()
        || !value.repository.is_absolute()
    {
        return Err("Invalid source connection record".into());
    }
    Ok(Some(value))
}

pub fn find(
    directory: &Path,
    project: &Path,
    id: &str,
    version: &str,
) -> Result<Option<Connection>, String> {
    let project = canonical(project)?;
    let path = location(directory, &project, id)?;
    // Do not follow a substituted configuration directory.
    if directory.exists()
        && canonical(directory)? != std::path::absolute(directory).map_err(|e| e.to_string())?
    {
        return Err("Source connection directory changed".into());
    }
    let Some(record) = read(&path)? else {
        return Ok(None);
    };
    if record.project != project || record.document_id != id || record.version != version {
        return Err("Saved project changed; reconnect its source folder".into());
    }
    Ok(Some(record))
}
impl Connection {
    pub fn for_saved(
        project: &Path,
        id: &str,
        version: &str,
        repository: &Repository,
    ) -> Result<Self, String> {
        if repository.manifest.document_id != id || !storage::valid_document_identity(version) {
            return Err("Source connection does not match the saved document".into());
        }
        Ok(Self {
            schema_version: 1,
            document_id: id.into(),
            project: canonical(project)?,
            version: version.into(),
            repository: canonical(repository.root())?,
        })
    }
    pub fn matches(&self, project: &Path, id: &str, version: &str) -> bool {
        self.document_id == id
            && self.version == version
            && canonical(project).is_ok_and(|p| p == self.project)
    }
    /// Carry an existing local read grant across a successful save, including
    /// while its read-only repository-open worker is still in flight.
    pub fn rebind(
        &self,
        directory: &Path,
        project: &Path,
        id: &str,
        version: &str,
    ) -> Result<Self, String> {
        if self.document_id != id || !storage::valid_document_identity(version) {
            return Err("Source connection does not match the saved document".into());
        }
        let mut next = self.clone();
        next.project = canonical(project)?;
        next.version = version.into();
        publish(directory, &next)?;
        Ok(next)
    }
    pub fn repository_path(&self) -> &Path {
        &self.repository
    }
    pub fn open(&self) -> Result<Repository, String> {
        if canonical(&self.project)? != self.project
            || storage::FileVersion::read(&self.project)?
                .token()
                .as_deref()
                != Some(&self.version)
        {
            return Err("Saved project changed; reconnect its source folder".into());
        }
        if canonical(&self.repository)? != self.repository {
            return Err("Source folder path changed; reconnect it explicitly".into());
        }
        let repository = Repository::open(&self.repository)?;
        if repository.manifest.document_id != self.document_id {
            return Err("Source folder belongs to another document".into());
        }
        if storage::FileVersion::read(&self.project)?
            .token()
            .as_deref()
            != Some(&self.version)
        {
            return Err("Saved project changed; reconnect its source folder".into());
        }
        Ok(repository)
    }
}
pub fn remember(
    directory: &Path,
    project: &Path,
    id: &str,
    version: &str,
    repository: &Repository,
) -> Result<Connection, String> {
    let record = Connection::for_saved(project, id, version, repository)?;
    publish(directory, &record)?;
    Ok(record)
}
fn publish(directory: &Path, record: &Connection) -> Result<(), String> {
    let project = &record.project;
    let id = &record.document_id;
    let version = record.version.as_str();
    let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Source connection is too large".into());
    }
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    if canonical(directory)? != std::path::absolute(directory).map_err(|e| e.to_string())? {
        return Err("Source connection directory changed".into());
    }
    let path = location(directory, &project, id)?;
    let lock_path = path.with_extension("lock");
    ordinary(&lock_path)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    if storage::FileVersion::read(&project)?.token().as_deref() != Some(version) {
        return Err("Saved project changed; reconnect its source folder".into());
    }
    if read(&path)?.as_ref() == Some(&record) {
        return Ok(());
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = directory.join(format!(
        ".source-connection-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
