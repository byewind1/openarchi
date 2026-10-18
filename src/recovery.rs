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
