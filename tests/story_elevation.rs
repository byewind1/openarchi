//! Product P2 task semantics, not an Archicad equivalence claim.
use atelier_bim::{
    automation::ReadService,
    automation_change::{Cancel, ChangeService, Commit, PrepareNative},
    automation_registry::{DiscoveredBatch, NativeBatch, describe},
    command::ModelCommand,
    editor::Editor,
    model::Project,
    model_service::{BatchChange, ReferencedCommand},
};
use serde_json::json;

fn setup() -> Editor {
    let mut project = Project::default();
    project.stories.truncate(2);
    project.stories[1].elevation = 3.;
    let mut editor = Editor::new(project);
    let batch: NativeBatch = serde_json::from_value(json!({"version":1,"creates":[
        {"name":"ground-wall","story":0,"layer":0,"material":"concrete","geometry":{"type":"wall","start":{"x":0.,"y":0.},"end":{"x":8.,"y":0.},"thickness":0.2,"height":3.}},
        {"name":"upper-wall","story":1,"layer":0,"material":"concrete","geometry":{"type":"wall","start":{"x":0.,"y":0.},"end":{"x":8.,"y":0.},"thickness":0.2,"height":3.}},
        {"name":"upper-slab","story":1,"layer":0,"material":"concrete","geometry":{"type":"slab","outline":[{"x":0.,"y":0.},{"x":8.,"y":0.},{"x":8.,"y":6.},{"x":0.,"y":6.}],"thickness":0.2}},
        {"name":"upper-door","story":1,"layer":0,"material":"concrete","geometry":{"type":"opening","host":{"kind":"temporary","value":"upper-wall"},"opening":"door","offset":4.,"width":0.9,"height":2.1,"sill":0.}}
    ]})).unwrap();
    let prepared = editor
        .prepare_batch_change(0, &batch.lower(&editor).unwrap())
        .unwrap();
    editor.commit_change("fixture", &prepared).unwrap();
    editor
}

fn batch(elevation: f64) -> BatchChange {
    BatchChange {
        edits: vec![ReferencedCommand::SetStoryElevation {
            story: 1,
            elevation,
        }],
        ..Default::default()
    }
}

#[test]
fn shared_story_edit_moves_world_geometry_not_relative_parameters_and_roundtrips() {
    let mut editor = setup();
    let before = editor.project.clone();
    let bounds: Vec<_> = before
        .elements
        .iter()
        .map(|e| {
            (
                e.id,
                editor
                    .geometry_source(e.id)
                    .unwrap()
                    .source
                    .display_bounds()
                    .unwrap(),
            )
        })
        .collect();
    let preview = editor
        .prepare_batch_change(editor.revision, &batch(3.3))
        .unwrap();
    assert_eq!(editor.project, before);
    assert_eq!(preview.affected_ids(), vec![2, 3, 4]);
    assert!(
        preview.difference().elements.is_empty(),
        "no redundant element copies in header-only delta"
    );
    let receipt = editor.commit_change("raise", &preview).unwrap();
    assert_eq!(receipt.changed_ids, vec![2, 3, 4]);
    assert_eq!(editor.project.elements, before.elements);
    assert_eq!(editor.undo.len(), 2);
    for (id, old) in bounds {
        let new = editor
            .geometry_source(id)
            .unwrap()
            .source
            .display_bounds()
            .unwrap();
        let delta = if id == 1 { 0. } else { 0.3 };
        for side in 0..2 {
            assert!((new[side] - old[side] - glam::DVec3::new(0., 0., delta)).length() < 1e-9);
        }
    }
    let after = editor.project.clone();
    let bytes = serde_json::to_vec(&after).unwrap();
    let restored: Project = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored, after);
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, after);
    assert_eq!(editor.commit_change("raise", &preview).unwrap(), receipt);
    assert_eq!(editor.project, after);
}

#[test]
fn p2_combined_wall_height_and_story_elevation_have_one_atomic_history_entry() {
    let mut editor = setup();
    let before = editor.project.clone();
    let request: DiscoveredBatch = serde_json::from_value(json!({"version":1,"edits":[
        {"operation":"set_story_elevation","story":1,"elevation":3.3},
        {"operation":"update","target":{"kind":"existing","value":1},"geometry":{"command":"wall","parameters":{"start":{"x":0.,"y":0.},"end":{"x":8.,"y":0.},"thickness":0.2,"height":3.3}}}
    ]})).unwrap();
    let change = request.resolve().unwrap().lower(&editor).unwrap();
    let prepared = editor
        .prepare_batch_change(editor.revision, &change)
        .unwrap();
    let receipt = editor.commit_change("p2-height", &prepared).unwrap();
    assert_eq!(receipt.changed_ids, vec![1, 2, 3, 4]);
    assert_eq!(editor.undo.len(), 2);
    assert_eq!(editor.project.stories[1].elevation, 3.3);
    assert_eq!(&editor.project.elements[1..], &before.elements[1..]);
    let after = editor.project.clone();
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, after);
}

#[test]
fn invalid_or_locked_story_edits_and_late_failure_preserve_project_redo_and_ids() {
    let mut editor = setup();
    let preview = editor
        .prepare_batch_change(editor.revision, &batch(3.4))
        .unwrap();
    editor.commit_change("redo-marker", &preview).unwrap();
    editor.undo();
    let before = editor.project.clone();
    let history = (editor.revision, editor.undo.len(), editor.redo.len());
    for elevation in [f64::NAN, f64::INFINITY, 10000.1] {
        assert!(
            editor
                .apply(ModelCommand::SetStoryElevation {
                    story: 1,
                    elevation
                })
                .is_err()
        );
        assert_eq!(editor.project, before);
    }
    assert!(
        editor
            .apply(ModelCommand::SetStoryElevation {
                story: 99,
                elevation: 3.3
            })
            .is_err()
    );
    let mut late = batch(3.3);
    late.edits.push(ReferencedCommand::SetStoryElevation {
        story: 999,
        elevation: 0.,
    });
    assert!(editor.prepare_batch_change(editor.revision, &late).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(
        (editor.revision, editor.undo.len(), editor.redo.len()),
        history
    );
    editor.project.edit_state.locked.insert(4);
    let locked = editor.project.clone();
    assert!(
        editor
            .prepare_batch_change(editor.revision, &batch(3.3))
            .is_err()
    );
    assert_eq!(editor.project, locked);
    // No-op never moves locked geometry or consumes a history entry.
    assert!(
        editor
            .apply(ModelCommand::SetStoryElevation {
                story: 1,
                elevation: 3.
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        (editor.revision, editor.undo.len(), editor.redo.len()),
        history
    );
}

#[test]
fn story_preview_has_explicit_header_and_world_impact_cancel_and_idempotent_receipt() {
    let mut editor = setup();
    let mut service = ChangeService::new("story-preview".into(), true);
    let target = ReadService::new("story-preview".into()).target(&editor);
    let request: NativeBatch = serde_json::from_value(json!({"version":1,"edits":[{"operation":"set_story_elevation","story":1,"elevation":3.3}]})).unwrap();
    let before = editor.project.clone();
    let first = service.prepare_native(
        &editor,
        PrepareNative {
            target: target.clone(),
            expected_revision: editor.revision,
            expected_context_revision: None,
            change: request.clone(),
        },
    );
    let p = &first["result"];
    assert_eq!(p["changed_ids"], json!([2, 3, 4]));
    assert_eq!(
        p["story_changes"],
        json!([{"story":1,"before_elevation_m":3.,"after_elevation_m":3.3,"affected_ids":[2,3,4]}])
    );
    assert_eq!(p["changes"], json!([]));
    let id = p["preview_id"].as_str().unwrap().to_string();
    service.cancel(
        &editor,
        Cancel {
            target: target.clone(),
            preview_id: id,
        },
    );
    assert_eq!(editor.project, before);
    let second = service.prepare_native(
        &editor,
        PrepareNative {
            target: target.clone(),
            expected_revision: editor.revision,
            expected_context_revision: None,
            change: request,
        },
    );
    let id = second["result"]["preview_id"].as_str().unwrap().to_string();
    let commit = || Commit {
        target: target.clone(),
        preview_id: id.clone(),
        request_id: "story-once".into(),
    };
    let receipt = service.commit(&mut editor, commit());
    assert_eq!(receipt["result"]["status"], "committed");
    assert_eq!(
        receipt["result"]["receipt"]["changed_ids"],
        json!([2, 3, 4])
    );
    assert_eq!(
        service.commit(&mut editor, commit())["result"]["receipt"],
        receipt["result"]["receipt"]
    );
    let contract = describe("story_elevation").unwrap();
    assert_eq!(contract["native_type"], false);
    assert_eq!(
        contract["input_schema"]["properties"]["operation"]["const"],
        "set_story_elevation"
    );
}
