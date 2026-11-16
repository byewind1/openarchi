//! Crash-recovery file lifecycle for unsaved projects.
//!
//! The recovery file is a normal validated OpenArchi JSON document in the
//! platform configuration directory. It is written only while the project is
//! dirty, removed after a successful save or explicit discard, and offered at
//! startup when a previous session ended unexpectedly.

use crate::model::Project;
use std::path::{Path, PathBuf};

pub fn path() -> Option<PathBuf> {
    crate::platform::config_dir().map(|directory| directory.join("recovery.openarchi.json"))
}

pub fn read(path: &Path) -> Result<Option<Project>, String> {
    if !path.exists() {
        return Ok(None);
    }
    crate::storage::load(path).map(Some)
}

pub fn write(path: &Path, project: &Project) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::storage::save(project, path)
}

pub fn clear(path: Option<&Path>) {
    if let Some(path) = path {
        let _ = std::fs::remove_file(path);
    }
}

/// An explicit, exclusively owned recovery slot. Dropping the owner releases
/// its OS lock but preserves the snapshot for crash recovery. Discovery and
/// presentation of recoverable slots belong to the application layer.
#[derive(Debug)]
pub struct RecoverySlot {
    directory: PathBuf,
    _owner: std::fs::File,
    snapshot: PathBuf,
}

impl RecoverySlot {
    pub fn create(root: &Path) -> Result<Self, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
        loop {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let directory = root.join(format!(
                "session-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
            let owner = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("owner.lock"))
                .map_err(|error| error.to_string())?;
            // A concurrent discoverer may see the directory before this lock
            // is acquired. Never wait for it or reuse its newly claimed slot.
            match owner.try_lock() {
                Ok(()) => {}
                Err(std::fs::TryLockError::WouldBlock) => continue,
                Err(std::fs::TryLockError::Error(error)) => return Err(error.to_string()),
            }
            return Ok(Self {
                snapshot: directory.join("project.openarchi.json"),
                directory,
                _owner: owner,
            });
        }
    }

    /// Acquire a known existing slot. None means another live owner holds it;
    /// missing or inaccessible slots are errors, not recoverable empty slots.
    pub fn claim(directory: &Path) -> Result<Option<Self>, String> {
        let owner = std::fs::OpenOptions::new()
            .write(true)
            .open(directory.join("owner.lock"))
            .map_err(|error| error.to_string())?;
        match owner.try_lock() {
            Ok(()) => Ok(Some(Self {
                snapshot: directory.join("project.openarchi.json"),
                directory: directory.to_owned(),
                _owner: owner,
            })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(error)) => Err(error.to_string()),
        }
    }

    /// Compatibility with the old single-file snapshot. No file is moved,
    /// so external drawing references keep their original base directory.
    pub fn claim_legacy(path: &Path) -> Result<Option<Self>, String> {
        let owner = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("recovery-lock"))
            .map_err(|e| e.to_string())?;
        match owner.try_lock() {
            Ok(()) => Ok(Some(Self {
                directory: path.parent().ok_or("Invalid recovery path")?.to_owned(),
                snapshot: path.to_owned(),
                _owner: owner,
            })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e.to_string()),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn snapshot(&self) -> &Path {
        &self.snapshot
    }

    /// Keep this session's ownership lock alive for one owned background write.
    pub(crate) fn writer_guard(&self) -> Result<std::fs::File, String> {
        self._owner.try_clone().map_err(|error| error.to_string())
    }

    pub fn read(&self) -> Result<Option<Project>, String> {
        read(&self.snapshot())
    }

    pub fn write(&self, project: &Project) -> Result<(), String> {
        write(&self.snapshot(), project)
    }

    pub fn write_document(
        &self,
        project: &Project,
        identity: &str,
        generation: Option<&str>,
    ) -> Result<(), String> {
        crate::storage::save_recovery_document(project, &self.snapshot, identity, generation)
    }
    pub fn read_document(
        &self,
    ) -> Result<Option<(Project, Option<String>, Option<String>)>, String> {
        if !self.snapshot.exists() {
            return Ok(None);
        }
        crate::storage::load_document_state(&self.snapshot)
            .map(|(project, _, id, base)| Some((project, id, base)))
    }
    /// Clear only this slot's snapshot, retaining its lock inode and ownership.
    pub fn clear(&self) -> Result<(), String> {
        match std::fs::remove_file(self.snapshot()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RecoveryCandidate {
    pub path: PathBuf,
    pub legacy: bool,
    pub name: String,
}

impl RecoveryCandidate {
    pub fn claim(&self) -> Result<Option<RecoverySlot>, String> {
        if self.legacy {
            RecoverySlot::claim_legacy(&self.path)
        } else {
            RecoverySlot::claim(&self.path)
        }
    }
}

/// Enumerate without reserving candidates for a particular window. Every UI
/// action must claim again and reload, because discovery is only a snapshot.
pub fn discover(config: &Path) -> (Vec<RecoveryCandidate>, Vec<String>) {
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    let mut paths = Vec::new();
    let legacy = config.join("recovery.openarchi.json");
    if legacy.exists() {
        paths.push((legacy, true));
    }
    match std::fs::read_dir(config.join("recoveries")) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        if entry.path().join("project.openarchi.json").exists() {
                            paths.push((entry.path(), false));
                        }
                    }
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => errors.push(e.to_string()),
    }
    for (path, legacy) in paths {
        let mut candidate = RecoveryCandidate {
            path,
            legacy,
            name: String::new(),
        };
        match candidate
            .claim()
            .and_then(|slot| slot.map(|slot| slot.read()).transpose())
        {
            Ok(Some(Some(project))) => {
                candidate.name = project.name;
                candidates.push(candidate);
            }
            Ok(_) => {}
            Err(e) => errors.push(format!("{}: {e}", candidate.path.display())),
        }
    }
    candidates.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    (candidates, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Material, Point, Shape};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openarchi-recovery-{name}-{}-{}.openarchi.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn recovery_roundtrip_and_clear() {
        let path = temp_path("roundtrip");
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        assert_eq!(read(&path).unwrap(), None);
        write(&path, &project).unwrap();
        assert_eq!(read(&path).unwrap(), Some(project));
        clear(Some(&path));
        assert_eq!(read(&path).unwrap(), None);
    }

    #[test]
    fn invalid_recovery_file_is_reported_not_silently_loaded() {
        let path = temp_path("invalid");
        std::fs::write(&path, b"not json").unwrap();
        assert!(read(&path).is_err());
        clear(Some(&path));
    }
}
