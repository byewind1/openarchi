use atelier_bim::{
    command::ModelCommand,
    editor::Editor,
    model::{Material, Point, Project, Shape},
};

fn grouped_editor() -> Editor {
    let mut project = Project::default();
    for x in [0., 6., 12.] {
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(x, 0.),
                end: Point::new(x + 4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
    }
    project.edit_state.groups = vec![vec![1, 2], vec![2, 3]];
    project.validate().unwrap();
    Editor::new(project)
}

#[test]
fn scoped_selection_keeps_group_expansion_inside_the_displayed_set_on_every_read() {
    let mut editor = grouped_editor();
    // Object 2 represents a hidden/off-story/forbidden member. Object 3 is
    // allowed and transitively grouped, so original grouping still applies.
    editor.set_scoped_selection(vec![1, 999], &[1, 3, 999]);
    assert_eq!(editor.selected_ids, vec![1, 3]);
    for _ in 0..3 {
        assert_eq!(editor.selection_ids(), vec![1, 3]);
        assert_eq!(editor.expanded_selection(vec![1]), vec![1, 3]);
    }

    // A hidden seed alone must not select an allowed grouped counterpart.
    editor.set_scoped_selection(vec![2, 999], &[1, 3]);
    assert!(editor.selection_ids().is_empty());

    editor.set_scoped_selection(vec![1], &[1]);
    editor.project.elements.retain(|element| element.id != 1);
    assert!(
        editor.selection_ids().is_empty(),
        "removed IDs stay unselected"
    );
}

#[test]
fn ordinary_selection_legacy_replacement_and_document_restore_do_not_inherit_a_scope() {
    let mut editor = grouped_editor();
    editor.set_scoped_selection(vec![1], &[1]);
    assert_eq!(editor.selection_ids(), vec![1]);
    editor.set_selection(vec![1]);
    assert_eq!(editor.selection_ids(), vec![1, 2, 3]);

    editor.set_scoped_selection(vec![1], &[1]);
    editor.selected = Some(2);
    editor.selected_ids = vec![2];
    assert_eq!(editor.selection_ids(), vec![1, 2, 3]);

    editor.set_scoped_selection(vec![1], &[1]);
    editor.set_scoped_selection(vec![], &[1]);
    assert!(editor.selection_ids().is_empty());
    editor.set_selection(vec![2]);
    assert_eq!(editor.selection_ids(), vec![1, 2, 3]);

    let project = editor.project.clone();
    let previous_session = editor.document_session();
    editor.set_scoped_selection(vec![1], &[1]);
    editor.restore_unsaved(project);
    assert_ne!(editor.document_session(), previous_session);
    assert!(editor.selection_ids().is_empty());
    editor.set_selection(vec![1]);
    assert_eq!(editor.selection_ids(), vec![1, 2, 3]);
}

#[test]
fn scoped_selection_preserves_model_dirty_state_and_real_nonempty_undo_redo() {
    for clean in [false, true] {
        let mut editor = grouped_editor();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset: Point::new(0., 1.),
            })
            .unwrap();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![3],
                offset: Point::new(0., 2.),
            })
            .unwrap();
        let after_both_commands = editor.project.clone();
        editor.undo();
        assert_eq!((editor.undo.len(), editor.redo.len()), (1, 1));
        if clean {
            editor.mark_saved();
        }
        let before = editor.project.clone();
        let saved = editor.saved.clone();
        let revision = editor.revision;
        let dirty = editor.dirty();
        let status = editor.status.clone();
        let history = format!("{:?}/{:?}", editor.undo, editor.redo);
        let serialized = serde_json::to_string(&editor.project).unwrap();

        editor.set_scoped_selection(vec![1], &[1, 3]);
        assert_eq!(editor.selection_ids(), vec![1, 3]);
        editor.set_scoped_selection(vec![3], &[3]);
        assert_eq!(editor.selection_ids(), vec![3]);
        editor.toggle_scoped_selection(vec![1], &[1, 3]);
        assert_eq!(editor.selection_ids(), vec![1, 3]);
        editor.toggle_scoped_selection(vec![3], &[1, 3]);
        assert!(editor.selection_ids().is_empty());
        assert_eq!(editor.project, before);
        assert_eq!(editor.saved, saved);
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.dirty(), dirty);
        assert_eq!(editor.status, status);
        assert_eq!(format!("{:?}/{:?}", editor.undo, editor.redo), history);
        assert_eq!(serde_json::to_string(&editor.project).unwrap(), serialized);

        editor.redo();
        assert_eq!(editor.project, after_both_commands);
        editor.undo();
        assert_eq!(editor.project, before);
    }
}

#[test]
fn comparison_scope_can_select_the_other_displayed_design_option() {
    let mut editor = grouped_editor();
    let a = editor.project.design_options.create("A".into());
    let b = editor.project.design_options.create("B".into());
    editor.project.elements[0].design_option = Some(a);
    editor.project.elements[1].design_option = Some(b);
    editor.project.plan_options.active_design_option = Some(a);
    editor.project.validate().unwrap();
    let before = editor.project.clone();

    editor.set_scoped_selection(vec![2], &[2]);
    assert_eq!(editor.selection_ids(), vec![2]);
    assert_eq!(editor.project, before);
}

#[test]
fn scoped_shift_removes_the_entire_selected_group_from_any_member() {
    for hit in [1, 2] {
        let mut editor = grouped_editor();
        editor.project.edit_state.groups = vec![vec![1, 2]];
        editor.set_scoped_selection(vec![1], &[1, 2, 3]);
        assert_eq!(editor.selection_ids(), vec![1, 2]);
        let before = editor.project.clone();
        editor.toggle_scoped_selection(vec![hit], &[1, 2, 3]);
        assert!(editor.selection_ids().is_empty());
        assert_eq!(editor.project, before);
    }
}

#[test]
fn scoped_shift_fills_a_partial_selectable_group_without_reintroducing_hidden_members() {
    let mut editor = grouped_editor();
    editor.set_scoped_selection(vec![1], &[1]);
    assert_eq!(editor.selection_ids(), vec![1]);
    // A wider allowed set exposes object 3; object 2 remains hidden. The
    // clicked partial group must be added as a whole within that wider set.
    editor.toggle_scoped_selection(vec![3], &[1, 3]);
    assert_eq!(editor.selection_ids(), vec![1, 3]);
    editor.toggle_scoped_selection(vec![1, 3], &[1, 3]);
    assert!(editor.selection_ids().is_empty(), "one toggle per group");
}

#[test]
fn scoped_shift_can_add_a_new_group_outside_the_previous_scope() {
    let mut editor = grouped_editor();
    let fourth = editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(18., 0.),
            end: Point::new(22., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    editor.project.edit_state.groups = vec![vec![1, 2], vec![3, fourth]];
    editor.set_scoped_selection(vec![1], &[1, 2]);
    editor.toggle_scoped_selection(vec![3, fourth], &[1, 2, 3, fourth]);
    assert_eq!(editor.selection_ids(), vec![1, 2, 3, fourth]);
    // Independent groups toggle independently in a mixed hit set.
    editor.toggle_scoped_selection(vec![1, 3], &[1, 2, 3, fourth]);
    assert!(editor.selection_ids().is_empty());

    editor.set_scoped_selection(vec![1], &[1, 2, 3, fourth]);
    editor.toggle_scoped_selection(vec![1, 3], &[1, 2, 3, fourth]);
    assert_eq!(editor.selection_ids(), vec![3, fourth]);
}
