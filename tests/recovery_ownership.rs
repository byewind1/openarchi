use atelier_bim::{model::Project, recovery::RecoverySlot};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "openarchi-recovery-owner-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Failed to create recovery test directory: {error}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn recovery_slots_keep_independent_snapshots_and_exclusive_ownership() {
    let root = Directory::new();
    let a = RecoverySlot::create(&root.0).unwrap();
    let b = RecoverySlot::create(&root.0).unwrap();
    let pa = Project {
        name: "A unsaved".into(),
        ..Project::default()
    };
    let pb = Project {
        name: "B unsaved".into(),
        ..Project::default()
    };
    a.write(&pa).unwrap();
    b.write(&pb).unwrap();
    let b_bytes = std::fs::read(b.directory().join("project.openarchi.json")).unwrap();
    assert!(RecoverySlot::claim(a.directory()).unwrap().is_none());
    a.clear().unwrap();
    a.clear().unwrap();
    assert_eq!(a.read().unwrap(), None);
    assert_eq!(b.read().unwrap(), Some(pb.clone()));
    assert_eq!(
        std::fs::read(b.directory().join("project.openarchi.json")).unwrap(),
        b_bytes
    );
    let path = b.directory().to_owned();
    drop(b);
    let owner = RecoverySlot::claim(&path).unwrap().unwrap();
    assert_eq!(owner.read().unwrap(), Some(pb));
    let mut invalid = pa;
    invalid.stories.clear();
    assert!(owner.write(&invalid).is_err());
    assert_eq!(
        std::fs::read(path.join("project.openarchi.json")).unwrap(),
        b_bytes
    );
    std::fs::write(path.join("project.openarchi.json"), "broken").unwrap();
    assert!(owner.read().is_err());
    assert!(RecoverySlot::claim(&root.0.join("missing")).is_err());
}

#[test]
fn recovery_owner_process_worker() {
    let Some(root) = std::env::var_os("OPENARCHI_RECOVERY_OWNER_TEST") else {
        return;
    };
    let root = PathBuf::from(root);
    let owner = RecoverySlot::create(&root).unwrap();
    owner
        .write(&Project {
            name: "crashed child".into(),
            ..Project::default()
        })
        .unwrap();
    let pending = root.join("ready.pending");
    std::fs::write(&pending, owner.directory().to_str().unwrap()).unwrap();
    std::fs::rename(pending, root.join("ready")).unwrap();
    // The parent kills this process, exercising OS release rather than Drop.
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn recovery_owner_crash_releases_lock_without_losing_snapshot() {
    let root = Directory::new();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "recovery_owner_process_worker", "--nocapture"])
        .env("OPENARCHI_RECOVERY_OWNER_TEST", &root.0)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !root.0.join("ready").exists() {
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Worker did not publish ready");
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "Worker exited before ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let slot = PathBuf::from(std::fs::read_to_string(root.0.join("ready")).unwrap());
    let busy = RecoverySlot::claim(&slot);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(busy.unwrap().is_none());
    let recovered = RecoverySlot::claim(&slot).unwrap().unwrap();
    assert_eq!(recovered.read().unwrap().unwrap().name, "crashed child");
    assert!(RecoverySlot::claim(&slot).unwrap().is_none());
    recovered.clear().unwrap();
    assert_eq!(recovered.read().unwrap(), None);
}

#[test]
fn recovery_document_keeps_project_identity_and_original_file_generation() {
    let root = std::env::temp_dir().join(format!(
        "oa-recovery-identity-{}",
        atelier_bim::storage::new_document_identity()
    ));
    let slot = atelier_bim::recovery::RecoverySlot::create(&root).unwrap();
    let project = atelier_bim::model::Project::default();
    let id = atelier_bim::storage::new_document_identity();
    let generation = "a".repeat(64);
    slot.write_document(&project, &id, Some(&generation))
        .unwrap();
    let (restored, identity, base) = slot.read_document().unwrap().unwrap();
    assert_eq!(restored, project);
    assert_eq!(identity, Some(id));
    assert_eq!(base, Some(generation));
    drop(slot);
    std::fs::remove_dir_all(root).unwrap();
}
