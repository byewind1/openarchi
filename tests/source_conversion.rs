#![cfg(unix)]
use atelier_bim::{editor::Editor, model::Project, project_repository::Repository, storage};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("oa-convert-{}", storage::new_document_identity()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn script(&self, body: &str) -> PathBuf {
        let path = self.0.join(format!(
            "converter {} ; literal",
            storage::new_document_identity()
        ));
        fs::write(&path,format!("#!/bin/sh\nif [ \"$1\" = '--version' ]; then printf 'fixture-tool 1.0\\n'; exit 0; fi\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    fn repo(&self) -> (Repository, String) {
        let mut repo = Repository::create_for_editor(
            &self.0.join("project"),
            &Editor::new(Project::default()),
        )
        .unwrap();
        let path = self.0.join("untrusted name ; input.dwg");
        fs::write(&path, b"original bytes").unwrap();
        let id = repo.add_source(&path).unwrap();
        (repo, id)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const VALID: &str = "printf '0\\nSECTION\\n2\\nENTITIES\\n0\\nLINE\\n5\\nA\\n8\\nLayer\\n0\\nENDSEC\\n0\\nEOF\\n' > output.dxf";
#[test]
fn explicit_args_and_success_receipt_keep_coverage_and_units_unknown() {
    let f = Fixture::new();
    let (mut repo, id) = f.repo();
    let before = repo.load_model().unwrap();
    let tool=f.script(&format!("[ \"$1\" = '-v1' ] && [ \"$2\" = '-o' ] && [ \"$3\" = 'output.dxf' ] && [ \"$4\" = 'input.dwg' ] || exit 19\n{VALID}"));
    let r = repo
        .convert_dwg(&id, &tool, Duration::from_secs(2))
        .unwrap();
    assert_eq!(r.status, "converted_unverified");
    assert!(r.coverage.starts_with("unknown"));
    let derived = repo
        .manifest
        .sources
        .iter()
        .find(|s| Some(&s.id) == r.derived_source_id.as_ref())
        .unwrap();
    assert!(derived.confirmed_unit.is_none());
    assert_eq!(derived.summary.as_ref().unwrap().records, 1);
    assert_eq!(repo.manifest.sources.len(), 2);
    assert_eq!(repo.manifest.conversions.len(), 1);
    let reopened = Repository::open(repo.root()).unwrap();
    assert_eq!(
        reopened.latest_conversion().unwrap().unwrap().operation_id,
        r.operation_id
    );
    assert_eq!(reopened.load_model().unwrap().project, before.project);
}
#[test]
fn failure_malformed_output_and_timeout_never_admit_derived_source() {
    for body in [
        "printf 'diagnostic' >&2; exit 7",
        "printf 'invalid' > output.dxf",
        "exec sleep 5",
    ] {
        let f = Fixture::new();
        let (mut repo, id) = f.repo();
        let tool = f.script(body);
        let r = repo
            .convert_dwg(&id, &tool, Duration::from_millis(60))
            .unwrap();
        assert_eq!(r.status, "failed");
        assert!(r.error.is_some());
        assert!(r.derived_source_id.is_none());
        assert_eq!(repo.manifest.sources.len(), 1);
        assert_eq!(repo.manifest.conversions.len(), 1);
        assert!(Repository::open(repo.root()).is_ok());
    }
}
#[test]
fn diagnostics_are_drained_and_retained_with_explicit_truncation() {
    let f = Fixture::new();
    let (mut repo, id) = f.repo();
    let tool = f.script(&format!("/usr/bin/head -c 150000 /dev/zero >&2\n{VALID}"));
    let r = repo
        .convert_dwg(&id, &tool, Duration::from_secs(3))
        .unwrap();
    let capture = r.capture.unwrap();
    assert_eq!(r.status, "converted_unverified");
    assert_eq!(capture.stderr.total_bytes, 150000);
    assert_eq!(capture.stderr.bytes.len(), 65536);
    assert!(capture.stderr.truncated());
}
#[test]
fn modified_input_and_stale_repository_preserve_existing_model_and_sources() {
    let f = Fixture::new();
    let (mut repo, id) = f.repo();
    let mut stale = Repository::open(repo.root()).unwrap();
    let tool = f.script(&format!("printf altered > input.dwg\n{VALID}"));
    let r = repo
        .convert_dwg(&id, &tool, Duration::from_secs(2))
        .unwrap();
    assert_eq!(r.status, "failed");
    assert!(r.error.unwrap().contains("input copy"));
    assert_eq!(repo.manifest.sources.len(), 1);
    assert!(
        stale
            .convert_dwg(&id, &tool, Duration::from_secs(2))
            .is_err()
    );
    assert!(
        repo.convert_dwg(&id, Path::new("/does/not/exist"), Duration::from_secs(2))
            .is_err()
    );
    assert!(Repository::open(repo.root()).is_ok());
}

#[test]
fn executable_change_and_output_symlink_are_retained_as_failed_receipts() {
    for body in [
        format!("{VALID}\nprintf '# changed\\n' >> \"$0\""),
        "ln -s input.dwg output.dxf".into(),
    ] {
        let f = Fixture::new();
        let (mut repo, id) = f.repo();
        let tool = f.script(&body);
        let receipt = repo
            .convert_dwg(&id, &tool, Duration::from_secs(2))
            .unwrap();
        assert_eq!(receipt.status, "failed");
        assert!(receipt.error.is_some());
        assert!(receipt.derived_source_id.is_none());
        assert_eq!(repo.manifest.sources.len(), 1);
        Repository::open(repo.root()).unwrap();
    }
}
#[test]
fn manifest_change_during_process_retains_execution_without_replaying_or_publishing() {
    let f = Fixture::new();
    let (mut repo, id) = f.repo();
    let tool = f.script(&format!("{VALID}\nprintf ' ' >> ../../project.json"));
    let error = repo
        .convert_dwg(&id, &tool, Duration::from_secs(2))
        .unwrap_err();
    assert!(error.contains("Repository changed"));
    let reopened = Repository::open(repo.root()).unwrap();
    assert_eq!(reopened.manifest.sources.len(), 1);
    assert!(reopened.manifest.conversions.is_empty());
    let jobs = fs::read_dir(repo.root().join("jobs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].join("execution.json").is_file());
    assert!(jobs[0].join("receipt.json").is_file());
}

#[test]
fn failed_quality_review_survives_reopen_without_modifying_source_bytes_or_receipts() {
    use atelier_bim::project_repository::SourceQuality;
    let f = Fixture::new();
    let (mut repo, id) = f.repo();
    let tool = f.script(VALID);
    let receipt = repo
        .convert_dwg(&id, &tool, Duration::from_secs(2))
        .unwrap();
    let derived = receipt.derived_source_id.clone().unwrap();
    let source = repo
        .manifest
        .sources
        .iter()
        .find(|s| s.id == derived)
        .unwrap();
    let path = repo.root().join(&source.blob.path);
    let bytes = fs::read(&path).unwrap();
    let receipt_path = repo.root().join(&repo.manifest.conversions[0].path);
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    assert!(repo.reject_source(&derived, "").is_err());
    repo.reject_source(
        &derived,
        "Handle coverage mismatch; geometry not independently measured",
    )
    .unwrap();
    let reopened = Repository::open(repo.root()).unwrap();
    assert!(
        matches!(&reopened.manifest.sources.iter().find(|s|s.id==derived).unwrap().quality,SourceQuality::Rejected{reason} if reason.contains("coverage"))
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read(receipt_path).unwrap(), receipt_bytes);
    assert!(matches!(
        reopened.manifest.sources[0].quality,
        SourceQuality::Unreviewed
    ));
    let repeated = repo
        .convert_dwg(&id, &tool, Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        repeated.derived_source_id.as_deref(),
        Some(derived.as_str())
    );
    assert!(matches!(
        repo.manifest
            .sources
            .iter()
            .find(|s| s.id == derived)
            .unwrap()
            .quality,
        SourceQuality::Rejected { .. }
    ));
}
