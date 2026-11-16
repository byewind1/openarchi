mod common;
use atelier_bim::{
    application_host::ApplicationHost,
    command::ModelCommand,
    design_process::{self as dp, Entry, intent, service},
    editor::Editor,
    model::{ModelAdapter, Point, Shape},
    storage,
    zones::ZoneSpec,
};
use std::{path::PathBuf, rc::Rc};
fn id() -> String {
    storage::new_document_identity()
}
fn setup() -> (ApplicationHost, PathBuf) {
    let root = std::env::temp_dir().join(format!("oa-intent-{}", &id()[..12]));
    std::fs::create_dir(&root).unwrap();
    let image = root.join("unscaled.png");
    image::RgbaImage::from_pixel(32, 24, image::Rgba([230, 220, 190, 255]))
        .save(&image)
        .unwrap();
    let mut project = common::synthetic_building_98();
    project
        .drawings
        .pages
        .push(atelier_bim::drawings::import(&image, &root.join("assets"), 1).unwrap());
    let mut host = ApplicationHost::new(project);
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let draft = service::Draft {
        name: "Previous authored task".into(),
        question: "Existing work must remain exact".into(),
        statements: vec![],
        candidate_names: ["Previous A".into(), "Previous B".into()],
        source_ids: host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
            .map(|e| e.id)
            .collect(),
        entry: Entry::ManualInterface,
    };
    let previous = id();
    let prepared = service::prepare_create(&host.document.editor, &previous, draft).unwrap();
    host.document
        .editor
        .commit_change(&id(), &prepared)
        .unwrap();
    let task = host
        .document
        .editor
        .project
        .design_process
        .task()
        .unwrap()
        .clone();
    let space = task.candidates[0].initial_zone_ids[0];
    let edit =
        service::prepare_edit(&host.document.editor, space, Point::new(0., 0.), Some(3.2)).unwrap();
    host.document.editor.commit_change(&id(), &edit).unwrap();
    let comparison = task.compare(&host.document.editor.project).unwrap();
    let choice = service::prepare_choose(
        &host.document.editor,
        &id(),
        &task.id,
        task.candidates[0].option,
        "Prior user decision".into(),
        comparison.fingerprint,
        Entry::ManualInterface,
        "controlled manual fixture".into(),
    )
    .unwrap();
    host.document.editor.commit_change(&id(), &choice).unwrap();
    (host, root)
}
fn seed(name: &str, x: f64, width: f64) -> intent::SpaceSeed {
    intent::SpaceSeed {
        name: name.into(),
        spec: ZoneSpec {
            surface: Default::default(),
            purpose: "declared concept use; not approved".into(),
            outline: vec![
                Point::new(x, 30.),
                Point::new(x + width, 30.),
                Point::new(x + width, 34.),
                Point::new(x, 34.),
            ],
            holes: vec![],
            height: 3.,
            bottom_offset: 0.,
            label_position: None,
        },
    }
}
fn draft(editor: &Editor) -> intent::Draft {
    intent::Draft {
        name: "Unscaled spatial intent".into(),
        question: "Make editable space candidates; protect existing design".into(),
        expected_result: "Two editable concepts with assumed dimensions and visible unknowns"
            .into(),
        images: vec![intent::ImageReference {
            page: 1,
            sha256: editor.project.drawings.pages[0].sha256.clone(),
        }],
        claims: vec![
            intent::Claim {
                kind: intent::ClaimKind::Observation,
                text: "The test original contains a colored region; no measured dimensions".into(),
            },
            intent::Claim {
                kind: intent::ClaimKind::Inference,
                text: "Interpret a region as a space; declaration not independently judged".into(),
            },
            intent::Claim {
                kind: intent::ClaimKind::DesignCompletion,
                text: "Choose room widths and clear height as design assumptions".into(),
            },
        ],
        geometry_assumptions: String::new(),
        statements: vec![dp::Statement {
            kind: dp::StatementKind::Unknown,
            text: "survey, approval and code compliance unknown".into(),
        }],
        candidate_names: ["Intent A".into(), "Intent B".into()],
        candidates: [
            vec![seed("Space1", 30., 6.), seed("Space2", 36., 3.)],
            vec![seed("Space1", 30., 7.), seed("Space2", 37., 2.)],
        ],
        story: 0,
        layer: 0,
    }
}
#[test]
fn intent_native_append_preserves_previous_work_choices_and_can_resume_exact_context() {
    let (mut host, root) = setup();
    let before = host.document.editor.project.clone();
    let previous = before.design_process.task().unwrap().clone();
    let previous_comparison = previous.compare(&before).unwrap();
    let task_id = id();
    let prepared = intent::prepare(
        &host.document.editor,
        &task_id,
        draft(&host.document.editor),
    )
    .unwrap();
    assert_eq!(host.document.editor.project, before);
    let original_target = host.operation_target();
    let request = id();
    let result = host.confirm_task_prepared(
        host.context(),
        &request,
        "Native spatial intent",
        Rc::new(prepared),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    let after = host.document.editor.project.clone();
    assert_eq!(
        &after.elements[..before.elements.len()],
        before.elements.as_slice()
    );
    assert_eq!(after.elements.len(), before.elements.len() + 6);
    assert_eq!(
        after.design_process.decisions,
        before.design_process.decisions
    );
    assert_eq!(
        after.design_process.tasks[0],
        before.design_process.tasks[0]
    );
    assert_eq!(
        after.design_process.current_task.as_deref(),
        Some(task_id.as_str())
    );
    assert_eq!(
        after.design_process.suspended_contexts[&previous.id].choice,
        before.design_process.current_choice
    );
    let task = after.design_process.task().unwrap();
    assert_eq!(task.entry, Entry::AgentProposal);
    assert!(
        task.intent
            .as_ref()
            .unwrap()
            .all_numeric_geometry_is_design_assumption
    );
    let comparison = task.compare(&after).unwrap();
    assert!(comparison.basis_current && comparison.different);
    assert_eq!(comparison.candidates[0].metrics.area_m2, 36.);
    assert_eq!(comparison.candidates[1].metrics.area_m2, 36.);
    assert_eq!(comparison.candidates[0].metrics.volume_m3, 108.);
    host.document.editor.undo();
    assert_eq!(host.document.editor.project.elements, before.elements);
    assert_eq!(
        host.document.editor.project.design_process.current_task,
        before.design_process.current_task
    );
    assert_eq!(
        host.document.editor.project.design_process.current_choice,
        before.design_process.current_choice
    );
    assert_eq!(host.document.editor.project.design_process.tasks.len(), 2);
    assert!(
        intent::prepare_activate(&host.document.editor, &task_id).is_err(),
        "an undone task is retained history, never rebound to reused model identities"
    );
    let future =
        intent::prepare(&host.document.editor, &id(), draft(&host.document.editor)).unwrap();
    let future_task = future.project().design_process.task().unwrap();
    assert!(future_task.baseline_option > task.candidates[1].option);
    assert!(future_task.source_zone_ids[0] > *task.candidates[1].initial_zone_ids.last().unwrap());
    drop(future);
    host.document.editor.redo();
    assert_eq!(host.document.editor.project, after);
    let activation = intent::prepare_activate(&host.document.editor, &previous.id).unwrap();
    let result = host.confirm_task_prepared(
        host.context(),
        &id(),
        "Resume previous task",
        Rc::new(activation),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    assert_eq!(host.document.editor.project.elements, after.elements);
    assert_eq!(
        host.document.editor.project.design_process.current_choice,
        before.design_process.current_choice
    );
    assert_eq!(
        previous.compare(&host.document.editor.project).unwrap(),
        previous_comparison
    );
    let activation = intent::prepare_activate(&host.document.editor, &task_id).unwrap();
    host.document
        .editor
        .commit_change(&id(), &activation)
        .unwrap();
    assert!(
        host.document
            .editor
            .project
            .design_process
            .current_choice
            .is_none()
    );
    let path = root.join("saved.json");
    storage::save_document(
        &host.document.editor.project,
        &path,
        host.document.editor.project_identity(),
    )
    .unwrap();
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 8);
    let reopened = Editor::from_stored_document(&path).unwrap();
    assert_eq!(
        reopened.project.design_process,
        host.document.editor.project.design_process
    );
    assert_eq!(reopened.project.elements, after.elements);
    let mut wrong = raw.clone();
    wrong["schema_version"] = serde_json::json!(7);
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&wrong).unwrap())
            .is_err()
    );
    // Storage deliberately rebinds asset paths to this saved file's sidecars. Historical outcome
    // remains queryable under the original identity; exact journal effect is conservatively Conflict.
    // This is not a permission to replay or portable operation recovery acceptance.
    drop(host);
    let mut host = ApplicationHost::new(atelier_bim::model::Project::default());
    host.document.editor = reopened;
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let recovered = host.inspect_operation(original_target, &request);
    assert_eq!(recovered["result"]["status"], "committed");
    assert_eq!(recovered["result"]["current_effect"], "Conflict");
    let mut images = host.document.editor.project.drawings.clone();
    images.pages[0].sha256 = "a".repeat(64);
    host.document
        .editor
        .apply(ModelCommand::SetDrawings { document: images })
        .unwrap();
    assert!(
        !task
            .compare(&host.document.editor.project)
            .unwrap()
            .basis_current
    );
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn intent_rejects_stale_sources_user_edits_and_legacy_snapshot_overwrite() {
    let (mut host, root) = setup();
    let before = host.document.editor.project.clone();
    let mut invalid = draft(&host.document.editor);
    invalid.images[0].sha256 = "b".repeat(64);
    assert!(intent::prepare(&host.document.editor, &id(), invalid).is_err());
    let proposal =
        intent::prepare(&host.document.editor, &id(), draft(&host.document.editor)).unwrap();
    let after = proposal.project().clone();
    for header in [false, true] {
        host.document.editor.project = after.clone();
        if header {
            host.document.editor.project.name = "overwrite user name".into();
        } else {
            host.document.editor.project.elements[0].name = "overwrite user element".into();
        }
        assert!(!host.document.editor.commit_checked(before.clone()));
        assert_eq!(host.document.editor.project, before);
    }
    let old_id = before.design_process.task().unwrap().candidates[0].initial_zone_ids[0];
    host.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![old_id],
            offset: Point::new(0.02, 0.),
        })
        .unwrap();
    let user = host.document.editor.project.clone();
    assert!(
        host.document
            .editor
            .commit_change(&id(), &proposal)
            .is_err()
    );
    assert_eq!(host.document.editor.project, user);
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn recorded_task_options_reuse_activation_and_inactive_membership_stays_protected() {
    let (mut host, root) = setup();
    let original = host.document.editor.project.clone();
    let previous = original.design_process.task().unwrap().clone();
    let prepared =
        intent::prepare(&host.document.editor, &id(), draft(&host.document.editor)).unwrap();
    host.document
        .editor
        .commit_change(&id(), &prepared)
        .unwrap();
    let task = host
        .document
        .editor
        .project
        .design_process
        .task()
        .unwrap()
        .clone();
    let main = service::prepare_activate(&host.document.editor, None).unwrap();
    host.document.editor.commit_change(&id(), &main).unwrap();
    let before = host.document.editor.project.clone();
    for target in [
        previous.source_zone_ids[0],
        previous.candidates[0].initial_zone_ids[0],
        previous.candidates[1].initial_zone_ids[0],
    ] {
        assert!(
            host.document
                .editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![target],
                    offset: Point::new(0.1, 0.)
                })
                .is_err()
        );
        assert_eq!(host.document.editor.project, before);
    }
    let switch =
        service::prepare_activate(&host.document.editor, Some(previous.candidates[1].option))
            .unwrap();
    let result = host.confirm_task_prepared(
        host.context(),
        &id(),
        "Existing menu task option",
        Rc::new(switch),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    assert_eq!(
        host.document
            .editor
            .project
            .design_process
            .current_task
            .as_deref(),
        Some(previous.id.as_str())
    );
    assert_eq!(
        host.document
            .editor
            .project
            .plan_options
            .active_design_option,
        Some(previous.candidates[1].option)
    );
    assert_eq!(
        host.document.editor.project.design_process.current_choice,
        original.design_process.current_choice
    );
    let back =
        service::prepare_activate(&host.document.editor, Some(task.candidates[0].option)).unwrap();
    host.document.editor.commit_change(&id(), &back).unwrap();
    assert_eq!(
        host.document
            .editor
            .project
            .design_process
            .current_task
            .as_deref(),
        Some(task.id.as_str())
    );
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
