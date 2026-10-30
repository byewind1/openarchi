//! P0 guards at the model write boundary, including legacy callers.
use atelier_bim::{
    command::ModelCommand,
    editor::Editor,
    layers::{ElementLayer, LayerStatus},
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};

fn project() -> Project {
    let mut p = Project::default();
    p.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    p
}

fn editor_with_redo() -> Editor {
    let mut editor = Editor::new(project());
    editor.set_selection(vec![1]);
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(1., 0.),
        })
        .unwrap();
    editor.undo();
    assert!(!editor.dirty());
    assert_eq!(editor.redo.len(), 1);
    editor
}

#[test]
fn legacy_invalid_candidates_restore_model_and_preserve_history() {
    for invalid_case in 0..3 {
        let mut editor = editor_with_redo();
        let before = editor.project.clone();
        let revision = editor.revision;
        let selection = editor.selection_ids();
        match invalid_case {
            0 => {
                if let Shape::Wall { thickness, .. } = &mut editor.project.elements[0].shape {
                    *thickness = -1.;
                }
            }
            1 => editor
                .project
                .elements
                .push(editor.project.elements[0].clone()),
            _ => editor.project.elements[0].story = 999,
        }
        assert!(editor.project.validate().is_err());
        editor.commit(before.clone());
        assert_eq!(editor.project, before, "invalid candidate {invalid_case}");
        assert_eq!(editor.revision, revision);
        assert!(editor.undo.is_empty());
        assert_eq!(editor.redo.len(), 1);
        assert_eq!(editor.selection_ids(), selection);
        assert!(!editor.dirty());
        editor.redo();
        assert_ne!(editor.project, before);
        editor.undo();
        assert_eq!(editor.project, before);
    }
}

#[test]
fn maximum_element_id_is_rejected_without_panicking_or_mutating_history() {
    let mut editor = editor_with_redo();
    let before = editor.project.clone();
    let revision = editor.revision;
    let mut element = before.elements[0].clone();
    element.id = u64::MAX;
    assert!(editor.apply(ModelCommand::AddElement { element }).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    assert!(editor.undo.is_empty());
    assert_eq!(editor.redo.len(), 1);
    assert!(!editor.dirty());
}

#[test]
fn restore_mixed_locked_layer_batch_is_rejected_atomically() {
    let mut p = project();
    let locked = p
        .layers
        .layers
        .iter()
        .find(|layer| !layer.protected)
        .unwrap()
        .id;
    p.layers.set_status(
        locked,
        LayerStatus {
            locked: true,
            ..LayerStatus::default()
        },
    );
    // IDs below next_id are available to a restore request after earlier deletion.
    p.next_id = 4;
    let mut editor = Editor::new(p);
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(1., 0.),
        })
        .unwrap();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    let mut allowed = before.elements[0].clone();
    allowed.id = 2;
    let mut forbidden = allowed.clone();
    forbidden.id = 3;
    forbidden.layer = ElementLayer(locked);
    assert!(
        editor
            .apply(ModelCommand::RestoreElements {
                elements: vec![(1, allowed), (2, forbidden)]
            })
            .is_err()
    );
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    assert!(editor.undo.is_empty());
    assert_eq!(editor.redo.len(), 1);
    assert!(!editor.dirty());
}

#[test]
fn valid_legacy_and_typed_edits_share_exact_history_and_persistence() {
    let mut editor = Editor::new(project());
    let original = editor.project.clone();
    editor.project.elements[0].name = "Legacy wall".into();
    editor.commit(original.clone());
    let legacy = editor.project.clone();
    let mut updated = legacy.elements[0].clone();
    if let Shape::Wall { height, .. } = &mut updated.shape {
        *height = 4.;
    }
    editor
        .apply(ModelCommand::UpdateElement { element: updated })
        .unwrap();
    let final_project = editor.project.clone();
    assert_eq!(editor.undo.len(), 2);
    assert_eq!(editor.revision, 2);
    let bytes = JsonAdapter.export(&final_project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), final_project);
    editor.undo();
    assert_eq!(editor.project, legacy);
    editor.undo();
    assert_eq!(editor.project, original);
    assert!(!editor.dirty());
    editor.redo();
    editor.redo();
    assert_eq!(editor.project, final_project);
}

#[test]
fn allowed_restore_batch_is_one_transaction_and_restores_element_order() {
    let mut p = project();
    p.next_id = 4;
    let mut editor = Editor::new(p);
    let before = editor.project.clone();
    let mut second = before.elements[0].clone();
    second.id = 2;
    second.name = "Restored second".into();
    let mut third = second.clone();
    third.id = 3;
    third.name = "Restored third".into();
    assert_eq!(
        editor
            .apply(ModelCommand::RestoreElements {
                elements: vec![(2, third), (1, second)]
            })
            .unwrap(),
        vec![2, 3]
    );
    let after = editor.project.clone();
    assert_eq!(
        after.elements.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(editor.undo.len(), 1);
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, after);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&after).unwrap())
            .unwrap(),
        after
    );
}
