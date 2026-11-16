//! P0 guards at the model write boundary, including legacy callers.
use atelier_bim::{
    command::ModelCommand,
    editor::{Editor, Tool},
    layers::{ElementLayer, LayerStatus},
    model::{Material, ModelAdapter, OpeningKind, Point, Project, Shape},
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
fn primitive_creation_at_exhausted_identity_preserves_model_and_real_redo() {
    for kind in ["Column", "Beam", "Roof"] {
        let mut editor = editor_with_redo();
        editor.project.next_id = u64::MAX;
        editor.project.validate().unwrap();
        editor.mark_saved();
        let before = editor.project.clone();
        let revision = editor.revision;
        let selection = editor.selection_ids();
        let history = format!("{:?}/{:?}", editor.undo, editor.redo);
        let result = match kind {
            "Column" => editor.place_column(Point::new(2., 2.)),
            "Beam" => editor.place_beam(Point::new(0., 2.), Point::new(4., 2.)),
            "Roof" => editor.place_roof(atelier_bim::roof::RoofSpec {
                contour: None,
                pivot_start: Point::default(),
                pivot_end: Point::new(4., 0.),
                rise_left: true,
                min: Point::default(),
                max: Point::new(4., 3.),
                pitch: 20.,
                thickness: 0.2,
                pivot_offset: 3.,
            }),
            _ => unreachable!(),
        };
        assert!(result.is_err(), "{kind}");
        assert_eq!(editor.project, before, "{kind}");
        assert_eq!(editor.revision, revision, "{kind}");
        assert_eq!(editor.selection_ids(), selection, "{kind}");
        assert_eq!(format!("{:?}/{:?}", editor.undo, editor.redo), history);
        assert!(!editor.dirty());
        let mut moved = before.clone();
        moved.elements[0].translate(Point::new(1., 0.));
        editor.redo();
        assert_eq!(editor.project, moved, "{kind} redo");
        editor.undo();
        assert_eq!(editor.project, before, "{kind} undo");
    }
}

#[test]
fn legacy_creation_capacity_rejections_preserve_project_draft_and_real_redo() {
    use atelier_bim::wall::WallGeometryMethod;
    for tool in [
        Tool::Wall,
        Tool::Polyline,
        Tool::Door,
        Tool::Window,
        Tool::Stair,
    ] {
        let mut editor = editor_with_redo();
        editor.set_tool(tool);
        if tool == Tool::Wall {
            editor.set_wall_geometry_method(WallGeometryMethod::Rectangular);
            // Three IDs remain but this operation requires four walls.
            editor.project.next_id = u64::MAX - 3;
        } else {
            editor.project.next_id = u64::MAX;
        }
        editor.project.validate().unwrap();
        editor.mark_saved();
        if matches!(tool, Tool::Wall | Tool::Stair | Tool::Polyline) {
            editor.click(Point::new(0., 2.), 0.01);
        }
        if tool == Tool::Polyline {
            editor.click(Point::new(4., 2.), 0.01);
        }
        editor.set_selection(vec![1]);
        let before = editor.project.clone();
        let revision = editor.revision;
        let selection = editor.selection_ids();
        let anchor = editor.anchor;
        let nodes = editor.polyline_points.clone();
        let history = format!("{:?}/{:?}", editor.undo, editor.redo);
        match tool {
            Tool::Polyline => assert!(editor.finish_polyline(None).is_err()),
            Tool::Wall => editor.click(Point::new(4., 5.), 0.01),
            Tool::Stair => editor.click(Point::new(4.8, 2.), 0.01),
            Tool::Door | Tool::Window => editor.click(Point::new(2., 0.), 0.01),
            _ => unreachable!(),
        }
        assert_eq!(
            editor.status, "Invalid element identity or story",
            "{tool:?}"
        );
        assert_eq!(editor.project, before, "{tool:?}");
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.selection_ids(), selection);
        assert_eq!(editor.anchor, anchor);
        assert_eq!(editor.polyline_points, nodes);
        assert_eq!(format!("{:?}/{:?}", editor.undo, editor.redo), history);
        assert!(!editor.dirty());
        let mut moved = before.clone();
        moved.elements[0].translate(Point::new(1., 0.));
        editor.redo();
        assert_eq!(editor.project, moved, "{tool:?} redo");
        editor.undo();
        assert_eq!(editor.project, before, "{tool:?} undo");
    }
}

#[test]
fn last_legal_wall_batch_advances_to_max_and_keeps_exact_undo_redo() {
    use atelier_bim::wall::WallGeometryMethod;
    let mut editor = Editor::new(Project::default());
    editor.project.next_id = u64::MAX - 4;
    editor.set_tool(Tool::Wall);
    editor.set_wall_geometry_method(WallGeometryMethod::Rectangular);
    let before = editor.project.clone();
    editor.click(Point::default(), 0.01);
    editor.click(Point::new(4., 3.), 0.01);
    assert_eq!(editor.project.elements.len(), 4);
    assert_eq!(editor.project.next_id, u64::MAX);
    assert_eq!(editor.undo.len(), 1);
    editor.project.validate().unwrap();
    let created = editor.project.clone();
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, created);
    editor.click(Point::new(10., 0.), 0.01);
    editor.click(Point::new(14., 3.), 0.01);
    assert_eq!(editor.status, "Invalid element identity or story");
    assert_eq!(editor.project, created);
    assert_eq!(editor.undo.len(), 1);
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

#[test]
fn legacy_mixed_placement_on_locked_layer_preserves_baseline_and_redo() {
    let mut editor = editor_with_redo();
    let locked = editor
        .project
        .layers
        .layers
        .iter()
        .find(|l| !l.protected)
        .unwrap()
        .id;
    editor.project.layers.set_status(
        locked,
        LayerStatus {
            locked: true,
            ..LayerStatus::default()
        },
    );
    editor.mark_saved();
    let before = editor.project.clone();
    let revision = editor.revision;
    let selection = editor.selection_ids();
    let mut allowed = before.elements[0].clone();
    allowed.id = 2;
    let mut forbidden = allowed.clone();
    forbidden.id = 3;
    forbidden.layer = ElementLayer(locked);
    editor.project.elements.extend([allowed, forbidden]);
    editor.project.next_id = 4;
    assert!(editor.project.validate().is_ok());
    editor.commit(before.clone());
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    assert_eq!(editor.selection_ids(), selection);
    assert!(editor.undo.is_empty());
    assert_eq!(editor.redo.len(), 1);
    assert!(!editor.dirty());
    editor.redo();
    editor.undo();
    assert_eq!(editor.project, before);
}

#[test]
fn opening_placement_checks_locked_host_for_add_restore_and_legacy() {
    for layer_lock in [false, true] {
        for entry in 0..3 {
            let mut editor = editor_with_redo();
            if layer_lock {
                let layer = editor
                    .project
                    .layers
                    .layers
                    .iter()
                    .find(|l| !l.protected)
                    .unwrap()
                    .id;
                editor.project.elements[0].layer = ElementLayer(layer);
                editor.project.layers.set_status(
                    layer,
                    LayerStatus {
                        locked: true,
                        ..LayerStatus::default()
                    },
                );
            } else {
                editor.project.edit_state.locked.insert(1);
            }
            editor.project.next_id = 4;
            editor.mark_saved();
            let before = editor.project.clone();
            let revision = editor.revision;
            let selection = editor.selection_ids();
            let mut opening = project().elements[0].clone();
            opening.id = 2;
            opening.shape = Shape::Opening {
                host_wall: 1,
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.,
                offset: 2.,
            };
            let mut candidate = before.clone();
            candidate.elements.push(opening.clone());
            assert!(candidate.validate().is_ok());
            match entry {
                0 => assert!(
                    editor
                        .apply(ModelCommand::AddElement { element: opening })
                        .is_err()
                ),
                1 => {
                    let mut allowed = project().elements[0].clone();
                    allowed.id = 3;
                    assert!(
                        editor
                            .apply(ModelCommand::RestoreElements {
                                elements: vec![(1, allowed), (2, opening)]
                            })
                            .is_err()
                    );
                }
                _ => {
                    editor.project = candidate;
                    editor.commit(before.clone());
                }
            }
            assert_eq!(
                editor.project, before,
                "layer_lock={layer_lock}, entry={entry}"
            );
            assert_eq!(editor.revision, revision);
            assert_eq!(editor.selection_ids(), selection);
            assert!(editor.undo.is_empty());
            assert_eq!(editor.redo.len(), 1);
            assert!(!editor.dirty());
        }
    }
}

#[test]
fn opening_tool_rejects_protected_host_without_success_or_history_changes() {
    use atelier_bim::{drawings::DrawingPlacement, external::ReferenceInstance};
    for tool in [Tool::Door, Tool::Window] {
        for protection in 0..3 {
            let mut editor = editor_with_redo();
            if protection == 0 {
                editor.project.edit_state.locked.insert(1);
            } else if protection == 1 {
                let layer = editor
                    .project
                    .layers
                    .layers
                    .iter()
                    .find(|l| !l.protected)
                    .unwrap()
                    .id;
                editor.project.elements[0].layer = ElementLayer(layer);
                editor.project.layers.set_status(
                    layer,
                    LayerStatus {
                        locked: true,
                        ..LayerStatus::default()
                    },
                );
            } else {
                editor
                    .project
                    .drawings
                    .reference_instances
                    .push(ReferenceInstance {
                        id: 88,
                        source: 87,
                        elements: vec![1],
                        loaded: true,
                        placement: DrawingPlacement {
                            worksheet: None,
                            story: 0,
                            origin: Point::new(0., 0.),
                            angle: 0.,
                            visible: true,
                            opacity: 255,
                            frame: None,
                        },
                    });
            }
            editor.mark_saved();
            editor.set_tool(tool);
            let before = editor.project.clone();
            let selection = editor.selection_ids();
            let revision = editor.revision;
            assert!(
                editor
                    .opening_placement(
                        if tool == Tool::Door {
                            OpeningKind::Door
                        } else {
                            OpeningKind::Window
                        },
                        Point::new(2., 0.),
                        0.1
                    )
                    .is_err()
            );
            editor.click(Point::new(2., 0.), 0.1);
            assert_eq!(
                editor.status, "Selection contains locked elements",
                "{tool:?}, protection={protection}"
            );
            assert_eq!(editor.project, before);
            assert_eq!(editor.selection_ids(), selection);
            assert_eq!(editor.revision, revision);
            assert!(!editor.dirty());
            assert!(editor.undo.is_empty());
            assert_eq!(editor.redo.len(), 1);
            editor.redo();
            assert_ne!(editor.project, before);
        }
    }
}

#[test]
fn parameter_injection_reports_protected_batch_rejection_and_preserves_redo() {
    use atelier_bim::{
        drawings::DrawingPlacement,
        external::{ReferenceInstance, ReferenceKind, ReferenceSource},
        transfer::TransferSettings,
    };
    for protection in 0..3 {
        for ids in [vec![1, 3], vec![3, 1]] {
            let mut p = project();
            for (y, material) in [(2., Material::Concrete), (4., Material::Plaster)] {
                p.add(
                    0,
                    material,
                    Shape::Wall {
                        start: Point::new(0., y),
                        end: Point::new(4., y),
                        thickness: 0.2,
                        height: 3.,
                    },
                );
            }
            let mut editor = Editor::new(p);
            editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![3],
                    offset: Point::new(1., 0.),
                })
                .unwrap();
            editor.undo();
            if protection == 0 {
                editor.project.edit_state.locked.insert(1);
            } else if protection == 1 {
                let layer = editor
                    .project
                    .layers
                    .layers
                    .iter()
                    .find(|l| !l.protected)
                    .unwrap()
                    .id;
                editor.project.elements[0].layer = ElementLayer(layer);
                editor.project.layers.set_status(
                    layer,
                    LayerStatus {
                        locked: true,
                        ..LayerStatus::default()
                    },
                );
            } else {
                editor
                    .project
                    .drawings
                    .reference_sources
                    .push(ReferenceSource {
                        id: 87,
                        name: "Fixture".into(),
                        path: "/fixture/module.json".into(),
                        sha256: "0".repeat(64),
                        kind: ReferenceKind::Hotlink,
                        elements: vec![editor.project.elements[0].clone()],
                        stories: editor.project.stories.clone(),
                        layers: editor.project.layers.clone(),
                    });
                editor
                    .project
                    .drawings
                    .reference_instances
                    .push(ReferenceInstance {
                        id: 88,
                        source: 87,
                        elements: vec![1],
                        loaded: true,
                        placement: DrawingPlacement {
                            worksheet: None,
                            story: 0,
                            origin: Point::new(0., 0.),
                            angle: 0.,
                            visible: true,
                            opacity: 255,
                            frame: None,
                        },
                    });
            }
            editor.mark_saved();
            let settings = TransferSettings::default();
            editor.pick_up_from(2, &settings).unwrap();
            editor.set_selection(ids.clone());
            let before = editor.project.clone();
            let selection = editor.selection_ids();
            let revision = editor.revision;
            assert_eq!(
                editor.inject_into(&ids, &settings).unwrap_err(),
                "Selection contains locked elements"
            );
            assert_eq!(editor.project, before);
            assert_eq!(editor.selection_ids(), selection);
            assert_eq!(editor.revision, revision);
            assert!(!editor.dirty());
            assert!(editor.undo.is_empty());
            assert_eq!(editor.redo.len(), 1);
            editor.redo();
            assert_ne!(editor.project, before);
        }
    }
}

#[test]
fn marquee_stretch_reports_protected_batch_rejection_and_preserves_redo() {
    use atelier_bim::{
        drawings::DrawingPlacement,
        editor::{MarqueeArea, MarqueeScope},
        external::{ReferenceInstance, ReferenceKind, ReferenceSource},
    };
    for protection in 0..3 {
        for reverse in [false, true] {
            let mut p = project();
            p.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(0., 2.),
                    end: Point::new(4., 2.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
            let mut editor = Editor::new(p);
            editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![2],
                    offset: Point::new(1., 0.),
                })
                .unwrap();
            editor.undo();
            if protection == 0 {
                editor.project.edit_state.locked.insert(1);
            } else if protection == 1 {
                let layer = editor
                    .project
                    .layers
                    .layers
                    .iter()
                    .find(|l| !l.protected)
                    .unwrap()
                    .id;
                editor.project.elements[0].layer = ElementLayer(layer);
                editor.project.layers.set_status(
                    layer,
                    LayerStatus {
                        locked: true,
                        ..LayerStatus::default()
                    },
                );
            } else {
                editor
                    .project
                    .drawings
                    .reference_sources
                    .push(ReferenceSource {
                        id: 87,
                        name: "Fixture".into(),
                        path: "/fixture/module.json".into(),
                        sha256: "0".repeat(64),
                        kind: ReferenceKind::Hotlink,
                        elements: vec![editor.project.elements[0].clone()],
                        stories: editor.project.stories.clone(),
                        layers: editor.project.layers.clone(),
                    });
                editor
                    .project
                    .drawings
                    .reference_instances
                    .push(ReferenceInstance {
                        id: 88,
                        source: 87,
                        elements: vec![1],
                        loaded: true,
                        placement: DrawingPlacement {
                            worksheet: None,
                            story: 0,
                            origin: Point::new(0., 0.),
                            angle: 0.,
                            visible: true,
                            opacity: 255,
                            frame: None,
                        },
                    });
            }
            if reverse {
                editor.project.elements.reverse();
            }
            editor.mark_saved();
            editor.marquee = MarqueeArea::new(
                vec![
                    Point::new(-1., -1.),
                    Point::new(1., -1.),
                    Point::new(1., 3.),
                    Point::new(-1., 3.),
                ],
                MarqueeScope::CurrentStory,
                0,
            );
            let before = editor.project.clone();
            let selection = editor.selection_ids();
            let revision = editor.revision;
            assert_eq!(
                editor
                    .stretch_nodes_in_marquee(Point::new(0.5, 0.))
                    .unwrap_err(),
                "Selection contains locked elements"
            );
            assert_eq!(editor.project, before);
            assert_eq!(editor.selection_ids(), selection);
            assert_eq!(editor.revision, revision);
            assert!(!editor.dirty());
            assert!(editor.undo.is_empty());
            assert_eq!(editor.redo.len(), 1);
            editor.redo();
            assert_ne!(editor.project, before);
        }
    }
}

#[test]
fn restore_host_and_opening_together_roundtrips_history_and_disk() {
    let mut p = Project::default();
    p.next_id = 3;
    let mut editor = Editor::new(p);
    let before = editor.project.clone();
    let wall = project().elements[0].clone();
    let mut opening = wall.clone();
    opening.id = 2;
    opening.shape = Shape::Opening {
        host_wall: 1,
        kind: OpeningKind::Window,
        width: 1.2,
        height: 1.4,
        sill: 0.9,
        offset: 2.,
    };
    editor
        .apply(ModelCommand::RestoreElements {
            elements: vec![(1, opening), (0, wall)],
        })
        .unwrap();
    let after = editor.project.clone();
    assert_eq!(editor.undo.len(), 1);
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, after);
    let dir = std::env::temp_dir().join(format!(
        "oa-placement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("project.json");
    atelier_bim::storage::save(&after, &path).unwrap();
    assert_eq!(atelier_bim::storage::load(&path).unwrap(), after);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn protected_write_errors_identify_real_layer_lock_and_preserve_history() {
    use atelier_bim::{drawings::DrawingPlacement, external::ReferenceInstance};
    for case in 0..4 {
        for operation in 0..3 {
            let mut editor = editor_with_redo();
            let layer = editor
                .project
                .layers
                .layers
                .iter()
                .find(|l| !l.protected)
                .unwrap()
                .id;
            editor.project.elements[0].layer = ElementLayer(layer);
            if case == 0 || case == 3 {
                editor.project.layers.set_status(
                    layer,
                    LayerStatus {
                        visible: true,
                        locked: true,
                        solid_3d: true,
                    },
                );
            }
            if case == 1 || case == 3 {
                editor.project.edit_state.locked.insert(1);
            }
            if case == 2 {
                editor
                    .project
                    .drawings
                    .reference_instances
                    .push(ReferenceInstance {
                        id: 88,
                        source: 87,
                        elements: vec![1],
                        loaded: true,
                        placement: DrawingPlacement {
                            worksheet: None,
                            story: 0,
                            origin: Point::new(0., 0.),
                            angle: 0.,
                            visible: true,
                            opacity: 255,
                            frame: None,
                        },
                    });
            }
            let before = editor.project.clone();
            let revision = editor.revision;
            let undo = editor.undo.len();
            let redo = editor.redo.len();
            let selection = editor.selection_ids();
            let dirty = editor.dirty();
            let command = match operation {
                0 => ModelCommand::RemoveElements { ids: vec![1] },
                1 => ModelCommand::UpdateElement {
                    element: editor.project.elements[0].clone(),
                },
                _ => ModelCommand::MoveElements {
                    ids: vec![1],
                    offset: Point::new(1., 0.),
                },
            };
            let error = editor.apply(command).unwrap_err();
            assert_eq!(
                error,
                if case == 0 || case == 3 {
                    "Element 1 is on a locked layer"
                } else {
                    "Element 1 is locked or read-only"
                }
            );
            assert_eq!(editor.project, before);
            assert_eq!(editor.revision, revision);
            assert_eq!((editor.undo.len(), editor.redo.len()), (undo, redo));
            assert_eq!(editor.selection_ids(), selection);
            assert_eq!(editor.dirty(), dirty);
        }
    }
}

#[test]
fn adjacent_story_move_rejects_protected_batch_without_erasing_redo() {
    use atelier_bim::{
        drawings::DrawingPlacement,
        external::{ReferenceInstance, ReferenceKind, ReferenceSource},
    };
    for protection in 0..3 {
        let mut p = project();
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 2.),
                end: Point::new(4., 2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let mut editor = Editor::new(p);
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![2],
                offset: Point::new(1., 0.),
            })
            .unwrap();
        editor.undo();
        match protection {
            0 => {
                editor.project.edit_state.locked.insert(1);
            }
            1 => {
                let layer = editor
                    .project
                    .layers
                    .layers
                    .iter()
                    .find(|l| !l.protected)
                    .unwrap()
                    .id;
                editor.project.elements[0].layer = ElementLayer(layer);
                editor.project.layers.set_status(
                    layer,
                    LayerStatus {
                        locked: true,
                        ..LayerStatus::default()
                    },
                );
            }
            _ => {
                editor
                    .project
                    .drawings
                    .reference_sources
                    .push(ReferenceSource {
                        id: 87,
                        name: "Fixture".into(),
                        path: "/fixture/module.json".into(),
                        sha256: "0".repeat(64),
                        kind: ReferenceKind::Hotlink,
                        elements: vec![editor.project.elements[0].clone()],
                        stories: editor.project.stories.clone(),
                        layers: editor.project.layers.clone(),
                    });
                editor
                    .project
                    .drawings
                    .reference_instances
                    .push(ReferenceInstance {
                        id: 88,
                        source: 87,
                        elements: vec![1],
                        loaded: true,
                        placement: DrawingPlacement {
                            worksheet: None,
                            story: 0,
                            origin: Point::new(0., 0.),
                            angle: 0.,
                            visible: true,
                            opacity: 255,
                            frame: None,
                        },
                    });
            }
        }
        editor.set_selection(vec![1, 2]);
        editor.mark_saved();
        let before = editor.project.clone();
        let selection = editor.selection_ids();
        let revision = editor.revision;
        editor.move_selection_to_story(1);
        assert_eq!(
            editor.status, "Selection contains locked elements",
            "protection {protection}"
        );
        assert_eq!(editor.project, before);
        assert_eq!(editor.selection_ids(), selection);
        assert_eq!(editor.revision, revision);
        assert!(!editor.dirty());
        assert!(editor.undo.is_empty());
        assert_eq!(editor.redo.len(), 1);
        editor.redo();
        assert_ne!(editor.project, before);
    }
}

#[test]
fn reshape_rejects_read_only_reference_before_preview_and_preserves_history() {
    use atelier_bim::{
        drawings::DrawingPlacement,
        external::{ReferenceInstance, ReferenceKind, ReferenceSource},
        reshape::{ReshapeCommand, candidate},
    };
    let mut editor = editor_with_redo();
    let command = ReshapeCommand::InsertWallNode {
        id: 1,
        point: Point::new(2., 0.),
        split: 0.5,
    };
    assert!(candidate(&editor.project, &command).is_ok());
    editor
        .project
        .drawings
        .reference_sources
        .push(ReferenceSource {
            id: 87,
            name: "Fixture".into(),
            path: "/fixture/module.json".into(),
            sha256: "0".repeat(64),
            kind: ReferenceKind::Hotlink,
            elements: vec![editor.project.elements[0].clone()],
            stories: editor.project.stories.clone(),
            layers: editor.project.layers.clone(),
        });
    editor
        .project
        .drawings
        .reference_instances
        .push(ReferenceInstance {
            id: 88,
            source: 87,
            elements: vec![1],
            loaded: true,
            placement: DrawingPlacement {
                worksheet: None,
                story: 0,
                origin: Point::new(0., 0.),
                angle: 0.,
                visible: true,
                opacity: 255,
                frame: None,
            },
        });
    editor.mark_saved();
    let before = editor.project.clone();
    let selection = editor.selection_ids();
    let revision = editor.revision;
    assert_eq!(
        candidate(&editor.project, &command).unwrap_err(),
        "Selection contains locked elements"
    );
    assert_eq!(
        editor.reshape(command).unwrap_err(),
        "Selection contains locked elements"
    );
    assert_eq!(editor.project, before);
    assert_eq!(editor.selection_ids(), selection);
    assert_eq!(editor.revision, revision);
    assert!(!editor.dirty());
    assert!(editor.undo.is_empty());
    assert_eq!(editor.redo.len(), 1);
    editor.redo();
    assert_ne!(editor.project, before);
}

#[test]
fn reshape_corner_rejects_duplicate_ids_without_panicking_or_changing_history() {
    use atelier_bim::reshape::{ReshapeCommand, candidate};
    for polyline in [false, true] {
        for chamfer in [false, true] {
            let mut editor = editor_with_redo();
            if polyline {
                editor.project.elements[0].shape = Shape::Polyline {
                    points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                    arcs: vec![],
                };
            }
            editor.mark_saved();
            let before = editor.project.clone();
            let selected = editor.selection_ids();
            let revision = editor.revision;
            let command = ReshapeCommand::Corner {
                ids: vec![1, 1],
                radius: 0.25,
                chamfer,
            };
            assert!(candidate(&editor.project, &command).is_err());
            assert!(editor.reshape(command).is_err());
            assert_eq!(editor.project, before);
            assert_eq!(editor.selection_ids(), selected);
            assert_eq!(editor.revision, revision);
            assert!(!editor.dirty());
            assert!(editor.undo.is_empty());
            assert_eq!(editor.redo.len(), 1);
            editor.redo();
            assert_ne!(editor.project, before);
        }
    }
}

#[test]
fn copy_identity_exhaustion_rejects_without_panicking_or_consuming_redo() {
    use atelier_bim::{editing::EditCommand, reshape::ReshapeCommand};
    for route in ["edit", "reshape", "paste"] {
        let mut editor = editor_with_redo();
        editor.project.next_id = u64::MAX;
        editor.project.validate().unwrap();
        editor.copy_selection();
        editor.mark_saved();
        let before = editor.project.clone();
        let selection = editor.selection_ids();
        let clipboard = editor.clipboard.clone();
        let revision = editor.revision;
        let history = format!("{:?}/{:?}", editor.undo, editor.redo);
        let command = EditCommand::Transform {
            offset: Point::new(10., 0.),
            origin: Point::default(),
            angle: 0.,
            scale: 1.,
            mirror: false,
            copies: 1,
        };
        match route {
            "edit" => assert!(editor.edit(command).is_err()),
            "reshape" => assert!(
                editor
                    .reshape(ReshapeCommand::SelectionEdit {
                        ids: selection.clone(),
                        command
                    })
                    .is_err()
            ),
            _ => assert_eq!(editor.paste(Point::new(10., 0.)), 0),
        }
        assert_eq!(editor.project, before, "{route}");
        assert_eq!(editor.selection_ids(), selection);
        assert_eq!(editor.clipboard, clipboard);
        assert_eq!(editor.revision, revision);
        assert_eq!(format!("{:?}/{:?}", editor.undo, editor.redo), history);
        assert!(!editor.dirty());
        let mut expected = before.clone();
        expected.elements[0].translate(Point::new(1., 0.));
        editor.redo();
        assert_eq!(editor.project, expected, "{route} real redo");
    }
}
