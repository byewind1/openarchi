use atelier_bim::{
    editor::Editor,
    project_repository::{
        Repository, SourceQuality,
        quality::{Counts, Draft, ReviewOperation, Verdict},
    },
    storage,
};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("oa-quality-{}", storage::new_document_identity()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn setup() -> (Fixture, Editor, Repository, Draft) {
    let f = Fixture::new();
    let editor = Editor::new(storage::startup_example());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &editor).unwrap();
    fs::write(
        f.0.join("original.dwg"),
        b"Synthetic original bytes, not a DWG decoder fixture",
    )
    .unwrap();
    fs::write(f.0.join("output.dxf"),b"0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nAA\n8\nLayer\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDSEC\n0\nEOF\n").unwrap();
    fs::write(f.0.join("comparison.json"), b"{\"fixture\":true}").unwrap();
    let reference = repo.add_source(&f.0.join("original.dwg")).unwrap();
    let subject = repo.add_source(&f.0.join("output.dxf")).unwrap();
    let evidence = repo.add_source(&f.0.join("comparison.json")).unwrap();
    let draft = Draft {
        subject_id: subject,
        reference_source_id: Some(reference),
        method: "Synthetic declared handle comparison".into(),
        scope: "One supported record; another expected reference".into(),
        summary: "Missing one handle; do not use this conversion".into(),
        limits: "Declared fixture, not independent geometry or approval".into(),
        verdict: Verdict::Rejected,
        comparison: Some(Counts {
            expected_unique_handles: 2,
            observed_unique_handles: 1,
            missing_handles: 1,
            ..Default::default()
        }),
        evidence_source_ids: vec![evidence],
    };
    (f, editor, repo, draft)
}
#[test]
fn quality_report_atomic_metadata_is_portable_and_preserves_model_selection_history() {
    let (f, mut editor, mut repo, draft) = setup();
    editor.set_selection(vec![13]);
    let model = editor.project.clone();
    let selection = editor.selected_ids.clone();
    let before = repo.manifest.revision;
    let operation = storage::new_document_identity();
    let review = repo
        .record_source_review(&operation, draft.clone())
        .unwrap();
    assert_eq!(repo.manifest.schema_version, 2);
    assert_eq!(repo.manifest.revision, before + 1);
    assert!(matches!(
        repo.manifest
            .sources
            .iter()
            .find(|s| s.id == draft.subject_id)
            .unwrap()
            .quality,
        SourceQuality::Rejected { .. }
    ));
    assert_eq!(editor.project, model);
    assert_eq!(editor.selected_ids, selection);
    assert!(editor.undo.is_empty());
    assert!(!editor.dirty());
    let moved = f.0.join("moved");
    fs::rename(repo.root(), &moved).unwrap();
    let reopened = Repository::open(&moved).unwrap();
    assert_eq!(
        reopened.source_reviews(Some(&draft.subject_id)).unwrap(),
        vec![review.clone()]
    );
    assert_eq!(reopened.load_model().unwrap().project, model);
    assert_eq!(
        reopened.review_operation(&operation).unwrap(),
        ReviewOperation::Published { review }
    );
}
#[test]
fn counts_mismatch_arithmetic_overflow_unknown_sources_and_approval_flags_refuse() {
    let (_f, _e, mut repo, mut draft) = setup();
    let original = fs::read(repo.root().join("project.json")).unwrap();
    draft.verdict = Verdict::Unverified;
    assert!(
        repo.record_source_review(&storage::new_document_identity(), draft.clone())
            .is_err()
    );
    draft.verdict = Verdict::Rejected;
    draft.comparison.as_mut().unwrap().expected_unique_handles = 3;
    assert!(
        repo.record_source_review(&storage::new_document_identity(), draft.clone())
            .is_err()
    );
    draft.comparison = Some(Counts {
        expected_unique_handles: u64::MAX,
        observed_unique_handles: u64::MAX,
        missing_handles: 1,
        extra_handles: 1,
        duplicate_handles: 0,
    });
    assert!(
        repo.record_source_review(&storage::new_document_identity(), draft.clone())
            .is_err()
    );
    draft.comparison = None;
    draft.evidence_source_ids.push("f".repeat(64));
    assert!(
        repo.record_source_review(&storage::new_document_identity(), draft.clone())
            .is_err()
    );
    assert_eq!(
        fs::read(repo.root().join("project.json")).unwrap(),
        original
    );
    let mut wire = serde_json::to_value(draft).unwrap();
    wire["verdict"] = serde_json::json!("approved");
    assert!(serde_json::from_value::<Draft>(wire).is_err());
}
#[test]
fn operation_identity_lookup_never_replays_unpublished_artifact_and_prior_rejection_survives() {
    let (_f, _e, mut repo, draft) = setup();
    let operation = storage::new_document_identity();
    let review = repo
        .record_source_review(&operation, draft.clone())
        .unwrap();
    let revision = repo.manifest.revision;
    assert_eq!(
        repo.record_source_review(&operation, draft.clone())
            .unwrap(),
        review
    );
    assert_eq!(repo.manifest.revision, revision);
    let mut changed = draft.clone();
    changed.summary = "Different input".into();
    assert!(repo.record_source_review(&operation, changed).is_err());
    let mut uncertain = draft.clone();
    uncertain.verdict = Verdict::Unverified;
    uncertain.comparison = None;
    uncertain.summary = "Later uncertainty does not undo prior rejection".into();
    repo.record_source_review(&storage::new_document_identity(), uncertain)
        .unwrap();
    assert!(
        matches!(repo.manifest.sources.iter().find(|s|s.id==draft.subject_id).unwrap().quality,SourceQuality::Rejected{ref reason} if reason==&draft.summary)
    );
    let orphan = storage::new_document_identity();
    let mut unpublished = review.clone();
    unpublished.operation_id = orphan.clone();
    fs::write(
        repo.root().join(format!("reviews/{orphan}.json")),
        serde_json::to_vec(&unpublished).unwrap(),
    )
    .unwrap();
    assert_eq!(
        repo.review_operation(&orphan).unwrap(),
        ReviewOperation::Unpublished {
            review: unpublished
        }
    );
    assert!(repo.record_source_review(&orphan, draft).is_err());
    assert_eq!(
        repo.review_operation(&storage::new_document_identity())
            .unwrap(),
        ReviewOperation::NotFound
    );
    fs::remove_file(repo.root().join(format!("reviews/{operation}.json"))).unwrap();
    assert!(repo.review_operation(&operation).is_err());
}
#[test]
fn stale_tampered_missing_and_legacy_marker_validation_are_explicit() {
    let (f, _e, mut repo, draft) = setup();
    let bytes = fs::read(repo.root().join("project.json")).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert!(value.get("reviews").is_none());
    let mut stale = Repository::open(repo.root()).unwrap();
    let op = storage::new_document_identity();
    repo.record_source_review(&op, draft.clone()).unwrap();
    assert!(
        stale
            .record_source_review(&storage::new_document_identity(), draft.clone())
            .is_err()
    );
    let manifest = repo.root().join("project.json");
    let published = fs::read(&manifest).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&published).unwrap();
    value["schema_version"] = serde_json::json!(1);
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(Repository::open(repo.root()).is_err());
    fs::write(&manifest, &published).unwrap();
    let report = repo.root().join(format!("reviews/{op}.json"));
    let saved = fs::read(&report).unwrap();
    fs::write(&report, b"{}").unwrap();
    assert!(Repository::open(repo.root()).is_err());
    fs::write(&report, saved).unwrap();
    let attachment = repo
        .manifest
        .sources
        .iter()
        .find(|s| draft.evidence_source_ids.contains(&s.id))
        .unwrap();
    fs::remove_file(repo.root().join(&attachment.blob.path)).unwrap();
    assert!(Repository::open(repo.root()).is_err());
    assert!(repo.source_reviews(None).is_err());
    assert!(f.0.join("original.dwg").is_file());
}

#[test]
fn portable_copy_keeps_only_reachable_history_rejects_existing_destination_and_preserves_original()
{
    let (f, editor, mut repo, draft) = setup();
    repo.record_source_review(&storage::new_document_identity(), draft)
        .unwrap();
    let original = fs::read(repo.root().join("project.json")).unwrap();
    fs::write(
        repo.root().join("unreferenced-secret.txt"),
        b"Do not copy arbitrary leftovers",
    )
    .unwrap();
    let copied = repo.copy_to(&f.0.join("copy")).unwrap();
    assert_eq!(copied.load_model().unwrap().project, editor.project);
    assert_eq!(
        copied.source_reviews(None).unwrap(),
        repo.source_reviews(None).unwrap()
    );
    assert_eq!(
        fs::read(repo.root().join("project.json")).unwrap(),
        original
    );
    assert!(!copied.root().join("unreferenced-secret.txt").exists());
    assert!(repo.copy_to(copied.root()).is_err());
    assert_eq!(
        fs::read(copied.root().join("project.json")).unwrap(),
        original
    );
}

#[test]
fn portable_copy_includes_verified_drawing_sidecars_without_external_cache_dependency() {
    let f = Fixture::new();
    let png = f.0.join("paper.png");
    image::RgbImage::new(3, 2).save(&png).unwrap();
    let mut project = storage::startup_example();
    project.drawings.pages.push(
        atelier_bim::drawings::import(
            &png,
            &f.0.join("cache"),
            project.drawings.next_id().unwrap(),
        )
        .unwrap(),
    );
    let editor = Editor::new(project);
    let repo = Repository::create_for_editor(&f.0.join("paper-repo"), &editor).unwrap();
    let copy = repo.copy_to(&f.0.join("paper-copy")).unwrap();
    fs::remove_dir_all(repo.root()).unwrap();
    fs::remove_dir_all(f.0.join("cache")).unwrap();
    let loaded = copy.load_model().unwrap();
    assert_eq!(loaded.project.elements, editor.project.elements);
    assert_eq!(
        loaded.project.drawings.pages[0].sha256,
        editor.project.drawings.pages[0].sha256
    );
    assert_eq!(
        atelier_bim::drawings::verified_bytes(&loaded.project.drawings.pages[0]).unwrap(),
        fs::read(png).unwrap()
    );
}
