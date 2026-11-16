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

#[test]
fn conditional_save_rejects_changed_created_and_deleted_files() {
    let dir = TestDirectory::new();
    let path = dir.0.join("conditional.json");
    let absent = storage::FileVersion::read(&path).unwrap();
    let project = Project::default();
    let saved = storage::save_if_unchanged(&project, &path, &absent).unwrap();
    assert!(storage::save_if_unchanged(&project, &path, &absent).is_err());
    let bytes = std::fs::read(&path).unwrap();
    let mut changed = bytes.clone();
    let i = changed.iter().position(|byte| *byte == b'\n').unwrap();
    changed[i] = b' ';
    std::fs::write(&path, &changed).unwrap();
    assert!(storage::save_if_unchanged(&project, &path, &saved).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    std::fs::remove_file(&path).unwrap();
    assert!(storage::save_if_unchanged(&project, &path, &saved).is_err());
    assert!(!path.exists());
}

#[test]
fn conditional_save_failure_preserves_baseline_and_can_retry() {
    let dir = TestDirectory::new();
    let path = dir.0.join("retry.json");
    let project = Project::default();
    storage::save(&project, &path).unwrap();
    let version = storage::FileVersion::read(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut invalid = project.clone();
    invalid.stories.clear();
    assert!(storage::save_if_unchanged(&invalid, &path, &version).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(storage::FileVersion::read(&path).unwrap(), version);
    assert_eq!(
        storage::save_if_unchanged(&project, &path, &version).unwrap(),
        version
    );
}

#[test]
fn conditional_save_process_worker() {
    let Some(directory) = std::env::var_os("OPENARCHI_SAVE_TEST_DIRECTORY") else {
        return;
    };
    let directory = PathBuf::from(directory);
    let identity = std::env::var("OPENARCHI_SAVE_TEST_ID").unwrap();
    let path = directory.join("shared.json");
    let baseline = storage::FileVersion::read(&path).unwrap();
    std::fs::write(directory.join(format!("ready-{identity}")), b"ready").unwrap();
    let start = std::time::Instant::now();
    while !directory.join("go").exists() {
        assert!(start.elapsed().as_secs() < 10);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let project = Project {
        name: identity.clone(),
        ..Project::default()
    };
    let result = storage::save_if_unchanged(&project, &path, &baseline);
    std::fs::write(
        directory.join(format!("result-{identity}")),
        if result.is_ok() { "saved" } else { "conflict" },
    )
    .unwrap();
}

#[test]
fn conditional_save_serializes_two_processes_from_same_baseline() {
    let dir = TestDirectory::new();
    let path = dir.0.join("shared.json");
    storage::save(&Project::default(), &path).unwrap();
    let mut children = ["one", "two"].map(|id| {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "conditional_save_process_worker"])
            .env("OPENARCHI_SAVE_TEST_DIRECTORY", &dir.0)
            .env("OPENARCHI_SAVE_TEST_ID", id)
            .spawn()
            .unwrap()
    });
    let start = std::time::Instant::now();
    while !(dir.0.join("ready-one").exists() && dir.0.join("ready-two").exists()) {
        assert!(start.elapsed().as_secs() < 10);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::fs::write(dir.0.join("go"), b"go").unwrap();
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let outcomes = ["one", "two"]
        .map(|id| std::fs::read_to_string(dir.0.join(format!("result-{id}"))).unwrap());
    assert_eq!(outcomes.iter().filter(|s| s.as_str() == "saved").count(), 1);
    assert_eq!(
        outcomes.iter().filter(|s| s.as_str() == "conflict").count(),
        1
    );
    let winner = if outcomes[0] == "saved" { "one" } else { "two" };
    assert_eq!(storage::load(&path).unwrap().name, winner);
}

#[test]
fn project_load_accepts_exact_limit_and_rejects_one_extra_byte() {
    let dir = TestDirectory::new();
    let path = dir.0.join("boundary.json");
    let project = Project::default();
    let mut bytes = serde_json::to_vec(&project).unwrap();
    bytes.resize(32 * 1024 * 1024, b' ');
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(storage::load(&path).unwrap(), project);
    bytes.push(b' ');
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        storage::load(&path).unwrap_err(),
        "Project exceeds 32 MB demo limit"
    );
}

#[cfg(unix)]
#[test]
fn project_load_bounds_stream_even_when_metadata_reports_zero_length() {
    use std::io::Write;
    let dir = TestDirectory::new();
    let path = dir.0.join("stream.json");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
    let project = Project::default();
    let mut bytes = serde_json::to_vec(&project).unwrap();
    bytes.resize(32 * 1024 * 1024 + 1, b' ');
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            let mut stream = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            if let Err(error) = stream.write_all(&bytes) {
                assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
            }
        });
        let result = storage::load(&path);
        writer.join().unwrap();
        assert_eq!(result.unwrap_err(), "Project exceeds 32 MB demo limit");
    });
}

#[test]
fn oversized_save_is_rejected_before_primary_replacement_with_reader_bound_unchanged() {
    use atelier_bim::model::ModelAdapter;
    let dir = TestDirectory::new();
    let path = dir.0.join("protected.json");
    let project = Project::default();
    storage::save_document(&project, &path, &storage::new_document_identity()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut oversized = project.clone();
    oversized.name = "x".repeat(storage::MAX_PROJECT_BYTES as usize);
    assert!(
        storage::JsonAdapter
            .export(&oversized)
            .unwrap_err()
            .contains("32 MB")
    );
    assert!(
        storage::save_document(&oversized, &path, &storage::new_document_identity())
            .unwrap_err()
            .contains("32 MB")
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(storage::load(&path).unwrap(), project);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}
