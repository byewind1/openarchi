use atelier_bim::{
    model::Project,
    project_repository::{Repository, connection},
    storage,
};
use std::{fs, path::PathBuf};
struct Fixture {
    root: PathBuf,
    model: PathBuf,
    directory: PathBuf,
    id: String,
    repo: Repository,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oa-source-connect-{}",
            storage::new_document_identity()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let model = root.join("current.json");
        let id = storage::new_document_identity();
        storage::save_document(&Project::default(), &model, &id).unwrap();
        let mut repo =
            Repository::create(&root.join("repository"), &Project::default(), &id, 0).unwrap();
        let original = root.join("brief.md");
        fs::write(
            &original,
            b"A declared design assumption, not a measurement.",
        )
        .unwrap();
        repo.add_source(&original).unwrap();
        Self {
            directory: root.join("connections"),
            root,
            model,
            id,
            repo,
        }
    }
    fn version(&self) -> String {
        storage::FileVersion::read(&self.model)
            .unwrap()
            .token()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn only_previously_connected_exact_saved_files_restore_sources_without_loading_models() {
    let f = Fixture::new();
    let before = fs::read(&f.model).unwrap();
    assert!(
        connection::find(&f.directory, &f.model, &f.id, &f.version())
            .unwrap()
            .is_none()
    );
    connection::remember(&f.directory, &f.model, &f.id, &f.version(), &f.repo).unwrap();
    let original_id = f.repo.manifest.sources[0].id.clone();
    let known = connection::find(&f.directory, &f.model, &f.id, &f.version())
        .unwrap()
        .unwrap();
    let reopened = known.open().unwrap();
    assert_eq!(reopened.manifest.sources[0].id, original_id);
    assert_eq!(fs::read(&f.model).unwrap(), before);
    let unknown = f.root.join("another-location.json");
    fs::copy(&f.model, &unknown).unwrap();
    assert!(
        connection::find(&f.directory, &unknown, &f.id, &f.version())
            .unwrap()
            .is_none()
    );
    assert!(
        connection::find(
            &f.directory,
            &f.model,
            &storage::new_document_identity(),
            &f.version()
        )
        .unwrap()
        .is_none()
    );
    let mut project = storage::load(&f.model).unwrap();
    project.name = "Externally changed".into();
    storage::save_document(&project, &f.model, &f.id).unwrap();
    assert!(connection::find(&f.directory, &f.model, &f.id, &f.version()).is_err());
    assert!(known.open().is_err());
}
#[test]
fn successful_save_carries_existing_read_grant_while_old_open_is_pending() {
    let f = Fixture::new();
    let original = fs::read(f.repo.root().join(&f.repo.manifest.sources[0].blob.path)).unwrap();
    let pending =
        connection::remember(&f.directory, &f.model, &f.id, &f.version(), &f.repo).unwrap();
    let mut next = storage::load(&f.model).unwrap();
    next.name = "New saved design".into();
    storage::save_document(&next, &f.model, &f.id).unwrap();
    let carried = pending
        .rebind(&f.directory, &f.model, &f.id, &f.version())
        .unwrap();
    assert!(pending.open().is_err());
    assert!(carried.open().is_ok());
    assert!(
        connection::find(&f.directory, &f.model, &f.id, &f.version())
            .unwrap()
            .unwrap()
            .open()
            .is_ok()
    );
    let save_as = f.root.join("new-version.json");
    storage::save_document(&next, &save_as, &f.id).unwrap();
    let token = storage::FileVersion::read(&save_as)
        .unwrap()
        .token()
        .unwrap();
    carried
        .rebind(&f.directory, &save_as, &f.id, &token)
        .unwrap();
    assert!(
        connection::find(&f.directory, &save_as, &f.id, &token)
            .unwrap()
            .unwrap()
            .open()
            .is_ok()
    );
    assert!(
        carried
            .rebind(
                &f.directory,
                &save_as,
                &storage::new_document_identity(),
                &token
            )
            .is_err()
    );
    assert_eq!(
        fs::read(f.repo.root().join(&f.repo.manifest.sources[0].blob.path)).unwrap(),
        original
    );
}
#[test]
fn missing_or_wrong_folder_and_damaged_connection_never_replace_saved_project() {
    let f = Fixture::new();
    let before = fs::read(&f.model).unwrap();
    let known = connection::remember(&f.directory, &f.model, &f.id, &f.version(), &f.repo).unwrap();
    let moved = f.root.join("old-folder");
    fs::rename(f.repo.root(), &moved).unwrap();
    assert!(known.open().is_err());
    Repository::create(
        f.repo.root(),
        &Project::default(),
        &storage::new_document_identity(),
        0,
    )
    .unwrap();
    assert!(known.open().is_err());
    let record = fs::read_dir(&f.directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|s| s == "json"))
        .unwrap();
    let damaged = vec![b'!'; 16 * 1024 + 1];
    fs::write(&record, &damaged).unwrap();
    assert!(connection::find(&f.directory, &f.model, &f.id, &f.version()).is_err());
    assert!(connection::remember(&f.directory, &f.model, &f.id, &f.version(), &f.repo).is_err());
    assert_eq!(fs::read(record).unwrap(), damaged);
    assert_eq!(fs::read(&f.model).unwrap(), before);
}
#[cfg(unix)]
#[test]
fn substituted_folder_or_connection_symlink_is_not_a_read_grant() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let known = connection::remember(&f.directory, &f.model, &f.id, &f.version(), &f.repo).unwrap();
    let moved = f.root.join("moved");
    fs::rename(f.repo.root(), &moved).unwrap();
    symlink(&moved, f.repo.root()).unwrap();
    assert!(known.open().is_err());
    let record = fs::read_dir(&f.directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|s| s == "json"))
        .unwrap();
    let protected = f.root.join("do-not-overwrite");
    fs::write(&protected, b"preserved").unwrap();
    fs::remove_file(&record).unwrap();
    symlink(&protected, &record).unwrap();
    assert!(connection::find(&f.directory, &f.model, &f.id, &f.version()).is_err());
    assert_eq!(fs::read(protected).unwrap(), b"preserved");
}
