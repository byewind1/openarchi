use atelier_bim::{
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    editor::Editor,
    mass_spaces::{self, Edit},
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};
fn fixture() -> Editor {
    let mut p = Project::default();
    p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(0., 0.),
                    Point::new(8., 0.),
                    Point::new(8., 6.),
                    Point::new(0., 6.),
                ],
                holes: vec![vec![
                    Point::new(2., 2.),
                    Point::new(2., 4.),
                    Point::new(4., 4.),
                    Point::new(4., 2.),
                ]],
                height: 6.,
                bottom_offset: 1.2,
            },
        },
    );
    Editor::new(p)
}
fn create(e: &Editor) -> Edit {
    Edit::Create {
        mass: 1,
        expected_source_sha256: mass_spaces::element_stamp(&e.project, &e.project.elements[0])
            .unwrap(),
        name: "概念空间A".into(),
        purpose: "用途未知（概念）".into(),
        height: 3.,
        assumption: "Single concept level, assumed clear height; no wall deduction or approved use"
            .into(),
    }
}
fn apply(e: &mut Editor, edit: Edit) {
    e.apply(ModelCommand::MassSpaces { edit }).unwrap();
}
#[test]
fn registered_shared_preview_real_zone_holes_quantities_native17_and_history() {
    let mut e = fixture();
    let before = e.project.clone();
    let descriptor = atelier_bim::automation_registry::describe("mass_space").unwrap();
    assert!(descriptor.to_string().contains("link_existing"));
    let raw =
        serde_json::json!({"version":1,"edits":[{"operation":"mass_space","edit":create(&e)}]});
    let discovered: atelier_bim::automation_registry::DiscoveredBatch =
        serde_json::from_value(raw.clone()).unwrap();
    let resolved = discovered.resolve().unwrap();
    let batch: atelier_bim::automation_registry::NativeBatch = serde_json::from_value(raw).unwrap();
    assert_eq!(
        serde_json::to_value(&resolved).unwrap(),
        serde_json::to_value(&batch).unwrap()
    );
    let prepared = e
        .prepare_batch_change(e.revision, &batch.lower(&e).unwrap())
        .unwrap();
    assert_eq!(e.project, before);
    e.commit_change("create-real-linked-space", &prepared)
        .unwrap();
    assert_eq!(e.project.elements[0], before.elements[0]);
    assert_eq!(e.project.elements.len(), 2);
    let Shape::Zone { spec } = &e.project.elements[1].shape else {
        panic!("real Zone required")
    };
    assert_eq!(spec.holes.len(), 1);
    assert_eq!(spec.bottom_offset, 1.2);
    assert_eq!(spec.height, 3.);
    assert_eq!(spec.area(), 44.);
    assert_eq!(spec.area() * spec.height, 132.);
    assert!(geo::Contains::contains(
        &spec.polygon(),
        &geo::Point::new(spec.label_point().x, spec.label_point().y)
    ));
    let l = &e.project.design_process.mass_spaces[0];
    assert_eq!(
        mass_spaces::status(&e.project, l),
        "Mass-space link is current"
    );
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 17);
    let reopened = JsonAdapter.import(&bytes).unwrap();
    assert_eq!(reopened, e.project);
    assert_eq!(
        mass_spaces::status(&reopened, l),
        "Mass-space link is current"
    );
    for version in [2, 9, 15, 16] {
        v["schema_version"] = version.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&v).unwrap())
                .is_err()
        );
    }
    let after = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&JsonAdapter.export(&before).unwrap()).unwrap()
            ["schema_version"],
        9
    );
}
#[test]
fn source_updates_are_explicit_and_never_overwrite_user_space_even_with_fresh_guards() {
    let mut e = fixture();
    let edit = create(&e);
    apply(&mut e, edit);
    let initial = e.project.clone();
    let mut m = e.project.elements[0].clone();
    if let Shape::ConceptMass { spec } = &mut m.shape {
        spec.transform(|p| Point::new(p.x + 10., p.y));
    }
    e.apply(ModelCommand::UpdateElement { element: m }).unwrap();
    let l = e.project.design_process.mass_spaces[0].clone();
    assert_eq!(
        mass_spaces::status(&e.project, &l),
        "Mass-space source changed"
    );
    apply(
        &mut e,
        Edit::Refresh {
            zone: l.zone,
            expected_sha256: mass_spaces::expected(&l),
            height: None,
        },
    );
    assert_eq!(
        mass_spaces::status(&e.project, &e.project.design_process.mass_spaces[0]),
        "Mass-space link is current"
    );
    let Shape::Zone { spec } = &e.project.elements[1].shape else {
        unreachable!()
    };
    assert_eq!(spec.outline[0].x, 10.);
    e.undo();
    assert_eq!(
        mass_spaces::status(&e.project, &l),
        "Mass-space source changed"
    );
    e.undo();
    assert_eq!(e.project, initial);
    let mut z = e.project.elements[1].clone();
    z.name = "User's later space".into();
    e.apply(ModelCommand::UpdateElement { element: z }).unwrap();
    let protected = e.project.clone();
    let h = e.undo.len();
    let fresh = e.project.design_process.mass_spaces[0].clone();
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::MassSpaces {
                edit: Edit::Refresh {
                    zone: fresh.zone,
                    expected_sha256: mass_spaces::expected(&fresh),
                    height: Some(2.)
                }
            }]
        )
        .is_err()
    );
    assert_eq!(e.project, protected);
    assert_eq!(e.undo.len(), h);
    apply(
        &mut e,
        Edit::Detach {
            zone: fresh.zone,
            expected_sha256: mass_spaces::expected(&fresh),
        },
    );
    assert_eq!(e.project.elements, protected.elements);
    e.undo();
    assert_eq!(e.project, protected);
}
#[test]
fn existing_binding_checks_real_holes_scope_and_height_and_never_regenerates() {
    let mut e = fixture();
    let mut z = e.project.elements[0].clone();
    z.id = 2;
    z.name = "Existing room".into();
    z.shape = Shape::Zone {
        spec: atelier_bim::zones::ZoneSpec {
            surface: Default::default(),
            purpose: "explicit concept use".into(),
            outline: vec![
                Point::new(0.2, 0.2),
                Point::new(1.8, 0.2),
                Point::new(1.8, 1.8),
                Point::new(0.2, 1.8),
            ],
            holes: vec![],
            height: 2.,
            bottom_offset: 1.5,
            label_position: None,
        },
    };
    e.apply(ModelCommand::AddElement { element: z }).unwrap();
    let link = |e: &Editor| Edit::LinkExisting {
        mass: 1,
        zone: 2,
        expected_source_sha256: mass_spaces::element_stamp(&e.project, &e.project.elements[0])
            .unwrap(),
        expected_zone_sha256: mass_spaces::element_stamp(&e.project, &e.project.elements[1])
            .unwrap(),
        assumption: "Existing authored space, not regenerated".into(),
    };
    let before = e.project.clone();
    let edit = link(&e);
    apply(&mut e, edit);
    assert_eq!(e.project.elements, before.elements);
    let l = e.project.design_process.mass_spaces[0].clone();
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::MassSpaces {
                edit: Edit::Refresh {
                    zone: 2,
                    expected_sha256: mass_spaces::expected(&l),
                    height: None
                }
            }]
        )
        .is_err()
    );
    e.undo();
    for mode in ["courtyard", "height", "story"] {
        let mut bad = e.project.elements[1].clone();
        if let Shape::Zone { spec } = &mut bad.shape {
            match mode {
                "courtyard" => spec.transform(|p| Point::new(p.x + 2., p.y + 2.)),
                "height" => spec.height = 10.,
                _ => bad.story = 1,
            }
        }
        e.apply(ModelCommand::UpdateElement { element: bad })
            .unwrap();
        let snapshot = e.project.clone();
        assert!(
            e.prepare_change(
                e.revision,
                vec![ModelCommand::MassSpaces { edit: link(&e) }]
            )
            .is_err(),
            "{mode}"
        );
        assert_eq!(e.project, snapshot);
        e.undo();
    }
}
#[test]
fn source_frame_lock_missing_and_bad_batch_preserve_shared_model() {
    let mut e = fixture();
    e.project.edit_state.locked.insert(1);
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::MassSpaces { edit: create(&e) }]
        )
        .is_err()
    );
    e.project.edit_state.locked.clear();
    let edit = create(&e);
    apply(&mut e, edit);
    let l = e.project.design_process.mass_spaces[0].clone();
    let old_zone = e.project.elements[1].clone();
    e.apply(ModelCommand::SetStoryElevation {
        story: 0,
        elevation: 4.,
    })
    .unwrap();
    assert_eq!(
        mass_spaces::status(&e.project, &l),
        "Mass-space source changed"
    );
    apply(
        &mut e,
        Edit::Refresh {
            zone: 2,
            expected_sha256: mass_spaces::expected(&l),
            height: None,
        },
    );
    assert_eq!(e.project.elements[1], old_zone);
    assert_eq!(
        mass_spaces::status(&e.project, &e.project.design_process.mass_spaces[0]),
        "Mass-space link is current"
    );
    e.undo();
    e.undo();
    e.apply(ModelCommand::RemoveElements { ids: vec![1] })
        .unwrap();
    assert_eq!(
        mass_spaces::status(&e.project, &l),
        "Mass-space source is unavailable"
    );
    let bytes = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    let before = e.project.clone();
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::MassSpaces {
                edit: Edit::Refresh {
                    zone: 2,
                    expected_sha256: mass_spaces::expected(&l),
                    height: None
                }
            }]
        )
        .is_err()
    );
    assert_eq!(e.project, before);
    e.undo();
    let before = e.project.clone();
    let mut wrong = create(&e);
    if let Edit::Create {
        expected_source_sha256,
        ..
    } = &mut wrong
    {
        *expected_source_sha256 = "0".repeat(64)
    }
    assert!(
        e.prepare_change(
            e.revision,
            vec![
                ModelCommand::MassSpaces { edit: create(&e) },
                ModelCommand::MassSpaces { edit: wrong }
            ]
        )
        .is_err()
    );
    assert_eq!(e.project, before);
}

#[test]
fn native_no_task_admission_is_typed_durable_and_never_grants_mixed_edits() {
    use atelier_bim::{application_host::ApplicationHost, storage};
    use std::rc::Rc;
    let root =
        std::env::temp_dir().join(format!("oa-mass-host-{}", storage::new_document_identity()));
    let mut host = ApplicationHost::new(fixture().project);
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let before = host.document.editor.project.clone();
    let command = ModelCommand::MassSpaces {
        edit: create(&host.document.editor),
    };
    let prepared = host
        .document
        .editor
        .prepare_change(host.document.editor.revision, vec![command.clone()])
        .unwrap();
    let target = host.operation_target();
    let result = host.confirm_task_prepared(
        host.context(),
        "original-space-operation",
        "Edit mass-space links",
        Rc::new(prepared),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    let after = host.document.editor.project.clone();
    assert_eq!(after.elements[0], before.elements[0]);
    let path = root.join("linked.json");
    storage::save_document(&after, &path, host.document.editor.project_identity()).unwrap();
    drop(host);
    let mut host = ApplicationHost::new(Project::default());
    host.document.editor = Editor::from_stored_document(&path).unwrap();
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let recovered = host.inspect_operation(target.clone(), "original-space-operation");
    assert_eq!(recovered["result"]["status"], "committed", "{recovered}");
    assert_eq!(
        result["result"]["operation_identity"],
        recovered["result"]["operation_identity"]
    );
    assert_eq!(host.document.editor.project, after);
    let command = ModelCommand::MassSpaces {
        edit: create(&host.document.editor),
    };
    for extra in [
        ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(0.1, 0.),
        },
        ModelCommand::SetDrawings {
            document: {
                let mut d = after.drawings.clone();
                d.worksheets.push(atelier_bim::drawings::Worksheet {
                    id: 1,
                    name: "Unrelated".into(),
                });
                d
            },
        },
    ] {
        let change = host
            .document
            .editor
            .prepare_change(host.document.editor.revision, vec![command.clone(), extra])
            .unwrap();
        let reply = host.confirm_task_prepared(
            host.context(),
            "mixed-must-refuse",
            "Edit mass-space links",
            Rc::new(change),
        );
        assert_eq!(reply["error"]["code"], "DESIGN_TASK_MISMATCH", "{reply}");
        assert_eq!(host.document.editor.project, after);
    }
    let prepared = host
        .document
        .editor
        .prepare_change(host.document.editor.revision, vec![command.clone()])
        .unwrap();
    let duplicate = host.confirm_task_prepared(
        host.context(),
        "original-space-operation",
        "Edit mass-space links",
        Rc::new(prepared),
    );
    assert_eq!(
        duplicate["error"]["code"], "RECOVERY_REQUIRED",
        "{duplicate}"
    );
    let prepared = host
        .document
        .editor
        .prepare_change(host.document.editor.revision, vec![command])
        .unwrap();
    let context = host.context();
    host.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(0.1, 0.),
        })
        .unwrap();
    let user = host.document.editor.project.clone();
    let stale = host.confirm_task_prepared(
        context,
        "stale-space",
        "Edit mass-space links",
        Rc::new(prepared),
    );
    assert!(stale.get("error").is_some(), "{stale}");
    assert_eq!(host.document.editor.project, user);
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
