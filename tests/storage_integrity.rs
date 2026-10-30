//! P0 save integrity: never share or truncate another writer's temporary file.
use atelier_bim::{model::Project, storage};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "openarchi-storage-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn save_preserves_existing_temporary_file_and_rejects_invalid_project() {
    let dir = TestDirectory::new();
    let path = dir.0.join("project.json");
    let old_tmp = path.with_extension("json.tmp");
    std::fs::write(&old_tmp, b"another writer's candidate").unwrap();
    let project = Project::default();
    storage::save(&project, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), project);
    assert_eq!(
        std::fs::read(&old_tmp).unwrap(),
        b"another writer's candidate"
    );
    let original = std::fs::read(&path).unwrap();
    let mut invalid = project;
    invalid.stories.clear();
    assert!(storage::save(&invalid, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 2);
}

#[test]
fn failed_rename_cleans_only_owned_temporary_file() {
    let dir = TestDirectory::new();
    let target = dir.0.join("existing-directory.json");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep"), b"original").unwrap();
    assert!(storage::save(&Project::default(), &target).is_err());
    assert_eq!(std::fs::read(target.join("keep")).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn concurrent_saves_publish_complete_candidates() {
    let dir = TestDirectory::new();
    let path = dir.0.join("project.json");
    let projects: Vec<_> = (0..16)
        .map(|index| {
            let mut project = Project::default();
            project.stories[0].name = format!("writer-{index}");
            project
        })
        .collect();
    let barrier = std::sync::Barrier::new(projects.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = projects
            .iter()
            .map(|project| {
                let path = &path;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    storage::save(project, path)
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
    });
    assert!(projects.contains(&storage::load(&path).unwrap()));
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn save_does_not_follow_old_temporary_symlink() {
    let dir = TestDirectory::new();
    let victim = dir.0.join("keep.json");
    std::fs::write(&victim, b"keep original bytes").unwrap();
    let path = dir.0.join("project.json");
    let old_tmp = path.with_extension("json.tmp");
    std::os::unix::fs::symlink(&victim, &old_tmp).unwrap();
    storage::save(&Project::default(), &path).unwrap();
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep original bytes");
    assert!(old_tmp.is_symlink());
    assert_eq!(storage::load(&path).unwrap(), Project::default());
}
