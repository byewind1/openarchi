use atelier_bim::{
    automation_registry::{COMMANDS, NativeBatch, describe, schema},
    editor::Editor,
    model::Project,
};
use serde_json::json;
fn batch() -> serde_json::Value {
    let p = |x: f64, y: f64| json!({"x":x,"y":y});
    let shapes = vec![
        json!({"type":"wall","start":p(0.,0.),"end":p(8.,0.),"thickness":0.2,"height":3.}),
        json!({"type":"slab","outline":[p(0.,0.),p(8.,0.),p(8.,6.),p(0.,6.)],"thickness":0.2}),
        json!({"type":"column","center":p(20.,0.),"section":{"section":"circular","diameter":0.4},"height":3.}),
        json!({"type":"beam","start":p(20.,0.),"end":p(25.,0.),"width":0.3,"height":0.5,"top_offset":3.}),
        json!({"type":"roof","outline":[p(0.,0.),p(8.,0.),p(8.,6.),p(0.,6.)],"pivot_start":p(0.,0.),"pivot_end":p(8.,0.),"rise_left":true,"pitch_degrees":20.,"thickness":0.2,"pivot_offset":3.}),
        json!({"type":"stair","start":p(12.,0.),"end":p(16.8,0.),"width":1.1,"total_rise":3.2,"risers":18,"slab_thickness":0.1,"handrail_height":1.}),
        json!({"type":"opening","host":{"kind":"temporary","value":"wall"},"opening":"door","width":0.9,"height":2.1,"sill":0.,"offset":2.}),
        json!({"type":"polyline","points":[p(0.,10.),p(4.,10.),p(4.,12.)]}),
    ];
    json!({"version":1,"creates":shapes.into_iter().zip(COMMANDS).map(|(geometry,name)|json!({"name":name,"story":0,"layer":1,"material":"concrete","geometry":geometry})).collect::<Vec<_>>()})
}

#[test]
fn native_joinery_create_read_and_legacy_update_preserve_fittings() {
    use atelier_bim::opening_joinery::DoorStyle;
    let mut editor = Editor::new(Project::default());
    let mut b = batch();
    b["creates"][6]["geometry"]["joinery"] =
        json!({"door_style":"double","opens_positive":false,"frame_width":0.06});
    let create: NativeBatch = serde_json::from_value(b).unwrap();
    let p = editor
        .prepare_batch_change(0, &create.lower(&editor).unwrap())
        .unwrap();
    let receipt = editor.commit_change("joinery-native-create", &p).unwrap();
    let id = receipt.resolved_references["opening"];
    let item = editor.project.elements.iter().find(|e| e.id == id).unwrap();
    let settings = item.opening_joinery.unwrap();
    assert_eq!(settings.door_style, DoorStyle::Double);
    assert!(!settings.opens_positive);
    let mut update = batch()["creates"][6]["geometry"].clone();
    update["host"] = json!({"kind":"existing","value":receipt.resolved_references["wall"]});
    update["width"] = json!(1.5);
    let raw:NativeBatch=serde_json::from_value(json!({"version":1,"edits":[{"operation":"update","target":{"kind":"existing","value":id},"geometry":update}]})).unwrap();
    let p = editor
        .prepare_batch_change(editor.revision, &raw.lower(&editor).unwrap())
        .unwrap();
    editor
        .commit_change("joinery-old-client-update", &p)
        .unwrap();
    assert_eq!(
        editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .opening_joinery,
        Some(settings)
    );
    let service = atelier_bim::automation::ReadService::new("joinery-read".into());
    let target = service.target(&editor);
    let q = atelier_bim::automation::Query {
        design_scope: Default::default(),
        target,
        expected_revision: editor.revision,
        ids: vec![id],
        kind: None,
        story: None,
        region: None,
        fields: vec!["opening_joinery".into(), "shape".into()],
        limit: 100,
        cursor: None,
    };
    let result = service.query(&editor, &q).unwrap();
    assert_eq!(
        result["elements"][0]["opening_joinery"]["door_style"],
        "double"
    );
    assert_eq!(result["elements"][0]["shape"]["width"], 1.5);
}
#[test]
fn eight_native_types_one_shared_transaction_and_file_roundtrip() {
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    let request: NativeBatch = serde_json::from_value(batch()).unwrap();
    let raw = request.lower(&editor).unwrap();
    let prepared = editor.prepare_batch_change(0, &raw).unwrap();
    assert_eq!(editor.project, before);
    let receipt = editor.commit_change("eight-native", &prepared).unwrap();
    assert_eq!(receipt.changed_ids.len(), 8);
    assert_eq!(editor.undo.len(), 1);
    editor.project.validate().unwrap();
    let restored: Project =
        serde_json::from_str(&serde_json::to_string(&editor.project).unwrap()).unwrap();
    assert_eq!(restored, editor.project);
    for name in COMMANDS.into_iter().take(8) {
        assert!(editor.project.elements.iter().any(|e| e.name == name));
    }
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, restored);
}
#[test]
fn registry_is_generated_from_wire_and_rejects_unknown_fields() {
    let root = schema();
    assert!(root["$defs"]["Geometry"].is_object());
    for name in COMMANDS {
        let d = describe(name).expect(name);
        assert_eq!(d["name"], name);
        assert_eq!(d["units"]["angle"], "degrees");
    }
    assert!(describe("shell").is_none());
    let mut b = batch();
    b["creates"][0]["geometry"]["unsupported"] = json!(1);
    assert!(serde_json::from_value::<NativeBatch>(b).is_err());
}
#[test]
fn invalid_mixed_batch_does_not_pollute_project_ids_or_history() {
    let editor = Editor::new(Project::default());
    let before = editor.project.clone();
    let mut b = batch();
    b["creates"][4]["geometry"]["thickness"] = json!(0.);
    let raw = serde_json::from_value::<NativeBatch>(b)
        .unwrap()
        .lower(&editor)
        .unwrap();
    assert!(editor.prepare_batch_change(0, &raw).is_err());
    assert_eq!(editor.project, before);
    assert!(editor.undo.is_empty());
}

#[test]
fn supported_variants_update_move_remove_preserve_native_types_and_shared_quantities() {
    use atelier_bim::model::Shape;
    let mut editor = Editor::new(Project::default());
    let create: NativeBatch = serde_json::from_value(batch()).unwrap();
    let prepared = editor
        .prepare_batch_change(0, &create.lower(&editor).unwrap())
        .unwrap();
    let receipt = editor.commit_change("variants-create", &prepared).unwrap();
    let ids = &receipt.resolved_references;
    let mut modified = batch();
    modified["creates"][0]["geometry"]["height"] = json!(3.6);
    for i in [1, 4] {
        modified["creates"][i]["geometry"]["holes"] =
            json!([[{"x":2.,"y":2.},{"x":3.,"y":2.},{"x":3.,"y":3.},{"x":2.,"y":3.}]]);
    }
    modified["creates"][1]["geometry"]["thickness"] = json!(0.3);
    modified["creates"][1]["geometry"]["elevation"] = json!(0.4);
    modified["creates"][2]["geometry"]["section"] =
        json!({"section":"rectangular","width":0.4,"depth":0.5});
    modified["creates"][2]["geometry"]["height"] = json!(3.5);
    modified["creates"][2]["geometry"]["inclination"] =
        json!({"slant_degrees":75.,"direction_degrees":-90.});
    modified["creates"][2]["geometry"]["axis_rotation_degrees"] = json!(30.);
    modified["creates"][3]["geometry"]["slant_angle_degrees"] = json!(10.);
    modified["creates"][3]["geometry"]["width"] = json!(0.4);
    modified["creates"][4]["geometry"]["pitch_degrees"] = json!(30.);
    modified["creates"][5]["geometry"]["width"] = json!(1.2);
    modified["creates"][6]["geometry"] = json!({"type":"opening","host":{"kind":"existing","value":ids["wall"]},"opening":"window","width":1.2,"height":1.2,"sill":1.,"offset":2.5});
    modified["creates"][7]["geometry"]["points"] =
        json!([{"x":0.,"y":10.},{"x":6.,"y":10.},{"x":6.,"y":13.}]);
    let edits:Vec<_>=COMMANDS.iter().take(8).enumerate().map(|(i,name)|json!({"operation":"update","target":{"kind":"existing","value":ids[*name]},"geometry":modified["creates"][i]["geometry"]})).collect();
    let updates: NativeBatch = serde_json::from_value(json!({"version":1,"edits":edits})).unwrap();
    let old = editor.project.clone();
    let prepared = editor
        .prepare_batch_change(editor.revision, &updates.lower(&editor).unwrap())
        .unwrap();
    assert_eq!(editor.project, old);
    editor.commit_change("variants-update", &prepared).unwrap();
    assert_eq!(editor.undo.len(), 2);
    editor.project.validate().unwrap();
    for element in &editor.project.elements {
        let original = old.elements.iter().find(|e| e.id == element.id).unwrap();
        assert_eq!(element.name, original.name);
        assert_eq!(element.material, original.material);
        assert_eq!(element.story, original.story);
        assert_eq!(element.layer, original.layer);
    }
    let slab = editor.geometry_source(ids["slab"]).unwrap().source;
    match slab.nominal_measure() {
        atelier_bim::derivation_cache::NominalMeasure::AreaM2(area) => {
            assert!((area - 47.).abs() < 1e-9)
        }
        _ => panic!("area expected"),
    }
    assert!((slab.material_volume_m3().unwrap() - 14.1).abs() < 1e-9);
    let column = editor.geometry_source(ids["column"]).unwrap().source;
    assert!((column.material_volume_m3().unwrap() - 0.7 / 75_f64.to_radians().sin()).abs() < 1e-9);
    let opening = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == ids["opening"])
        .unwrap();
    assert!(
        matches!(opening.shape,Shape::Opening{host_wall,kind:atelier_bim::model::OpeningKind::Window,..} if host_wall==ids["wall"])
    );
    assert_eq!(
        editor
            .geometry_source(ids["polyline"])
            .unwrap()
            .source
            .material_volume_m3(),
        None
    );
    let snapshot = editor.project.clone();
    let bounds: Vec<_> = editor
        .project
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
    let targets: Vec<_> = ids
        .values()
        .map(|id| json!({"kind":"existing","value":id}))
        .collect();
    let request:NativeBatch=serde_json::from_value(json!({"version":1,"edits":[{"operation":"move","targets":targets,"offset":{"x":10.,"y":-5.}}]})).unwrap();
    let prepared = editor
        .prepare_batch_change(editor.revision, &request.lower(&editor).unwrap())
        .unwrap();
    editor.commit_change("variants-move", &prepared).unwrap();
    for (id, old_bounds) in bounds {
        let new = editor
            .geometry_source(id)
            .unwrap()
            .source
            .display_bounds()
            .unwrap();
        for side in 0..2 {
            assert!(
                (new[side] - old_bounds[side] - glam::DVec3::new(10., -5., 0.)).length() < 1e-8,
                "id={id}"
            );
        }
    }
    editor.undo();
    assert_eq!(editor.project, snapshot);
    editor.redo();
    let moved = editor.project.clone();
    let request: NativeBatch = serde_json::from_value(
        json!({"version":1,"edits":[{"operation":"remove","targets":targets}]}),
    )
    .unwrap();
    let prepared = editor
        .prepare_batch_change(editor.revision, &request.lower(&editor).unwrap())
        .unwrap();
    editor.commit_change("variants-remove", &prepared).unwrap();
    assert!(editor.project.elements.is_empty());
    editor.undo();
    assert_eq!(editor.project, moved);
    editor.redo();
    assert!(editor.project.elements.is_empty());
}

#[test]
fn native_front_door_rejects_scope_type_and_dependency_errors_atomically() {
    use atelier_bim::{
        automation::ReadService,
        automation_change::{ChangeService, PrepareNative},
    };
    let mut project = Project::default();
    let locked = project.layers.new_layer("locked fixture", None);
    project.layers.layer_mut(locked).unwrap().status.locked = true;
    let editor = Editor::new(project);
    let service = ReadService::new("guard".into());
    let target = service.target(&editor);
    let mut changes = ChangeService::new("guard".into(), true);
    let baseline = serde_json::to_value(&editor.project).unwrap();
    let mut scenarios = vec![];
    let mut wrong_story = batch();
    wrong_story["creates"][1]["story"] = json!(999);
    scenarios.push(wrong_story);
    let mut wrong_layer = batch();
    wrong_layer["creates"][2]["layer"] = json!(locked);
    scenarios.push(wrong_layer);
    let mut missing_host = batch();
    missing_host["creates"][6]["geometry"]["host"] =
        json!({"kind":"temporary","value":"missing-wall"});
    scenarios.push(missing_host);
    let mut invalid_parameter = batch();
    invalid_parameter["creates"][3]["geometry"]["width"] = json!(0.);
    scenarios.push(invalid_parameter);
    for scenario in scenarios {
        let change = serde_json::from_value(scenario).unwrap();
        let result = changes.prepare_native(
            &editor,
            PrepareNative {
                expected_context_revision: None,
                target: target.clone(),
                expected_revision: 0,
                change,
            },
        );
        assert!(result.get("error").is_some(), "{result}");
        assert_eq!(serde_json::to_value(&editor.project).unwrap(), baseline);
        assert_eq!(editor.revision, 0);
        assert!(editor.undo.is_empty() && editor.redo.is_empty());
    }
}

#[test]
fn preview_lists_native_changes_and_host_cascade_without_consuming_ids() {
    use atelier_bim::{
        automation::ReadService,
        automation_change::{ChangeService, Commit, PrepareNative},
    };
    let mut editor = Editor::new(Project::default());
    let mut service = ChangeService::new("impact".into(), true);
    let target = ReadService::new("impact".into()).target(&editor);
    let baseline = editor.project.clone();
    let prepared = service.prepare_native(
        &editor,
        PrepareNative {
            expected_context_revision: None,
            target: target.clone(),
            expected_revision: 0,
            change: serde_json::from_value(batch()).unwrap(),
        },
    );
    assert_eq!(editor.project, baseline);
    let r = &prepared["result"];
    assert_eq!(r["changes"].as_array().unwrap().len(), 8);
    assert!(
        r["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["action"] == "create")
    );
    let wall = r["resolved_references"]["wall"].as_u64().unwrap();
    let opening = r["resolved_references"]["opening"].as_u64().unwrap();
    assert_eq!(
        r["dependencies"],
        json!([{"element_id":opening,"host_wall":wall}])
    );
    assert_eq!(
        service.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: r["preview_id"].as_str().unwrap().into(),
                request_id: "impact-create".into()
            }
        )["result"]["status"],
        "committed"
    );
    let complete = editor.project.clone();
    let change=serde_json::from_value(json!({"version":1,"edits":[{"operation":"remove","targets":[{"kind":"existing","value":wall}]}]})).unwrap();
    let removal = service.prepare_native(
        &editor,
        PrepareNative {
            expected_context_revision: None,
            target: target.clone(),
            expected_revision: editor.revision,
            change,
        },
    );
    let r = &removal["result"];
    assert_eq!(editor.project, complete);
    assert_eq!(r["changes"].as_array().unwrap().len(), 2);
    assert!(
        r["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["action"] == "remove")
    );
    assert_eq!(
        r["dependencies"],
        json!([{"element_id":opening,"host_wall":wall}])
    );
    assert_eq!(
        service.commit(
            &mut editor,
            Commit {
                target,
                preview_id: r["preview_id"].as_str().unwrap().into(),
                request_id: "impact-remove".into()
            }
        )["result"]["status"],
        "committed"
    );
    assert_eq!(editor.project.elements.len(), 6);
    editor.undo();
    assert_eq!(editor.project, complete);
}

fn registered_payload(mut value: serde_json::Value) -> serde_json::Value {
    for create in value
        .get_mut("creates")
        .and_then(serde_json::Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        let mut geometry = create["geometry"].as_object().unwrap().clone();
        let command = geometry.remove("type").unwrap();
        create["geometry"] = json!({"command":command,"parameters":geometry});
    }
    for edit in value
        .get_mut("edits")
        .and_then(serde_json::Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if edit["operation"] == "update" {
            let mut geometry = edit["geometry"].as_object().unwrap().clone();
            let command = geometry.remove("type").unwrap();
            edit["geometry"] = json!({"command":command,"parameters":geometry});
        }
    }
    value
}
#[test]
fn discovered_payload_uses_identical_native_transaction_and_preserves_original_intent() {
    use atelier_bim::{
        automation::ReadService,
        automation_change::{ChangeService, Commit, PrepareDiscovered, PrepareNative},
        automation_registry::DiscoveredBatch,
    };
    let mut full = Editor::new(Project::default());
    let mut discovered = Editor::new(Project::default());
    let mut full_service = ChangeService::new("full".into(), true);
    let root = std::env::temp_dir().join(format!(
        "oa-discovered-{}",
        atelier_bim::storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut service =
        ChangeService::new("discovered".into(), true).with_journal_directory(root.clone());
    let target = ReadService::new("discovered".into()).target(&discovered);
    let full_target = ReadService::new("full".into()).target(&full);
    let original = registered_payload(batch());
    let wire: DiscoveredBatch = serde_json::from_value(original.clone()).unwrap();
    let normalized_intent = serde_json::to_value(&wire).unwrap();
    let full_preview = full_service.prepare_native(
        &full,
        PrepareNative {
            expected_context_revision: None,
            target: full_target.clone(),
            expected_revision: 0,
            change: serde_json::from_value(batch()).unwrap(),
        },
    );
    let preview = service.prepare_discovered(
        &discovered,
        PrepareDiscovered {
            expected_context_revision: None,
            target: target.clone(),
            expected_revision: 0,
            change: wire,
        },
    );
    assert_ne!(
        full_preview["result"]["payload_hash"],
        preview["result"]["payload_hash"]
    );
    assert_eq!(
        preview["result"]["payload_hash"],
        atelier_bim::automation_journal::canonical_hash(&normalized_intent).unwrap()
    );
    assert_eq!(
        full_service.commit(
            &mut full,
            Commit {
                target: full_target,
                preview_id: full_preview["result"]["preview_id"]
                    .as_str()
                    .unwrap()
                    .into(),
                request_id: "full".into()
            }
        )["result"]["status"],
        "committed"
    );
    assert_eq!(
        service.commit(
            &mut discovered,
            Commit {
                target: target.clone(),
                preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
                request_id: "discovered".into()
            }
        )["result"]["status"],
        "committed"
    );
    assert_eq!(full.project, discovered.project);
    assert_eq!(full.undo.len(), discovered.undo.len());
    drop(service);
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("{}.json", discovered.project_identity()))).unwrap(),
    )
    .unwrap();
    let record = record["records"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert_eq!(record["intent_payload"], normalized_intent);
    assert_ne!(record["execution_hash"], record["payload_hash"]);
    full.undo();
    discovered.undo();
    assert_eq!(full.project, discovered.project);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn discovered_parameters_are_data_with_the_same_strict_native_validation() {
    use atelier_bim::automation_registry::DiscoveredBatch;
    for (field, value) in [("unsupported", json!(1)), ("type", json!("wall"))] {
        let mut payload = registered_payload(batch());
        payload["creates"][0]["geometry"]["parameters"][field] = value;
        assert!(
            serde_json::from_value::<DiscoveredBatch>(payload)
                .unwrap()
                .resolve()
                .is_err()
        );
    }
    let mut payload = registered_payload(batch());
    payload["creates"][0]["geometry"]["command"] = json!("shell");
    assert!(
        serde_json::from_value::<DiscoveredBatch>(payload)
            .unwrap()
            .resolve()
            .is_err()
    );
}

#[test]
fn context_guard_binds_selection_and_story_without_changing_explicit_id_semantics() {
    use atelier_bim::{
        automation::ReadService,
        automation_change::{ChangeService, Commit, PrepareNative},
    };
    let directory = std::env::temp_dir().join(format!(
        "oa-context-{}",
        atelier_bim::storage::new_document_identity()
    ));
    std::fs::create_dir(&directory).unwrap();
    let mut editor = Editor::new(Project::default());
    let read = ReadService::new("context-instance".into());
    let target = read.target(&editor);
    let mut changes =
        ChangeService::new("context-instance".into(), true).with_journal_directory(directory);
    let baseline = editor.project.clone();
    let token = read.context_revision(&editor);
    let prepare = |guard: Option<String>| PrepareNative {
        target: target.clone(),
        expected_revision: 0,
        expected_context_revision: guard,
        change: serde_json::from_value(batch()).unwrap(),
    };
    let preview = changes.prepare_native(&editor, prepare(Some(token.clone())));
    assert_eq!(preview["result"]["context_revision"], token);
    let commit = || Commit {
        target: target.clone(),
        preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
        request_id: "context-commit".into(),
    };
    editor.story = 1;
    assert_eq!(editor.revision, 0);
    assert_eq!(
        changes.commit(&mut editor, commit())["error"]["code"],
        "CONTEXT_CONFLICT"
    );
    assert_eq!(
        changes.prepare_native(&editor, prepare(Some(token.clone())))["error"]["code"],
        "CONTEXT_CONFLICT"
    );
    editor.story = 0;
    editor.selected_ids = vec![42];
    assert_eq!(
        changes.commit(&mut editor, commit())["error"]["code"],
        "CONTEXT_CONFLICT"
    );
    assert_eq!(editor.project, baseline);
    assert!(editor.undo.is_empty() && editor.redo.is_empty());
    let explicit = changes.prepare_native(&editor, prepare(None));
    assert!(explicit.get("result").is_some(), "{explicit}");
    editor.selected_ids.clear();
    let receipt = changes.commit(&mut editor, commit());
    assert_eq!(receipt["result"]["status"], "committed");
    editor.story = 1;
    editor.selected_ids = vec![42];
    let retry = changes.commit(&mut editor, commit());
    assert_eq!(retry["result"]["receipt"], receipt["result"]["receipt"]);
    assert_eq!(retry["result"]["status"], "committed");
    assert_eq!(editor.revision, 1);
    assert_eq!(editor.project.elements.len(), baseline.elements.len() + 8);
}
