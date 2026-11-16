mod common;
use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    automation::ReadService,
    automation_change::{ChangeService, Commit},
    automation_registry::{DiscoveredBatch, NativeBatch},
    command::ModelCommand,
    editor::Editor,
    site::*,
    storage,
};
use serde_json::json;
fn evidence() -> SiteEvidence {
    SiteEvidence {
        source_sha256: "a".repeat(64),
        locator: "synthetic fixture; not a real survey".into(),
        note: "analytic reference".into(),
    }
}
fn context() -> SiteContext {
    SiteContext {
        horizontal: Some(HorizontalReference {
            crs: "synthetic EN metres".into(),
            origin_east_north_m: [500000., 4600000.],
            model_to_grid_ccw_deg: 90.,
            evidence: evidence(),
        }),
        true_north: Some(TrueNorth {
            clockwise_from_model_y_deg: 30.,
            evidence: evidence(),
        }),
        vertical: Some(VerticalReference {
            datum: "fixture datum".into(),
            model_zero_elevation_m: 68.3,
            evidence: evidence(),
        }),
        controls: vec![
            CoordinateControl {
                model_xy_m: [0., 0.],
                reference_east_north_m: [500000., 4600000.],
                evidence: evidence(),
            },
            CoordinateControl {
                model_xy_m: [20., 0.],
                reference_east_north_m: [500000., 4600020.],
                evidence: evidence(),
            },
        ],
        control_tolerance_m: Some(0.005),
        ..Default::default()
    }
}
#[test]
fn unknown_and_legacy_references_never_become_zero_north_or_flat_terrain() {
    let s = SiteContext::default();
    assert!(s.is_unknown());
    assert!(s.to_reference_xy([0., 0.]).is_err());
    assert!(s.true_north_vector().is_err());
    assert!(s.reference_elevation(0.).is_err());
    assert!(matches!(
        s.control_check().unwrap(),
        ControlCheck::Insufficient { .. }
    ));
    let p = common::synthetic_building_98();
    assert!(p.site.is_unknown());
    let old = serde_json::to_value(&p).unwrap();
    assert!(old.get("site").is_none());
    let loaded: atelier_bim::model::Project = serde_json::from_value(old).unwrap();
    assert_eq!(loaded, p);
}
#[test]
fn analytic_rotation_inverse_and_true_north_are_independent() {
    let s = context();
    assert_eq!(s.to_reference_xy([3., 4.]).unwrap(), [499996., 4600003.]);
    let inverse = s.from_reference_xy([499996., 4600003.]).unwrap();
    assert!((inverse[0] - 3.).abs() < 1e-9);
    assert!((inverse[1] - 4.).abs() < 1e-9);
    let north = s.true_north_vector().unwrap();
    assert!((north[0] - 0.5).abs() < 1e-12);
    assert!((north[1] - 3f64.sqrt() / 2.).abs() < 1e-12);
    assert!((s.reference_elevation(4.2).unwrap() - 72.5).abs() < 1e-12);
    let mut edge = s.clone();
    edge.horizontal.as_mut().unwrap().origin_east_north_m = [1e9, 0.];
    edge.horizontal.as_mut().unwrap().model_to_grid_ccw_deg = 0.;
    assert!(edge.to_reference_xy([1., 0.]).is_err());
    let mut invalid_context = Editor::new(atelier_bim::model::Project::default());
    invalid_context
        .apply(ModelCommand::SetSiteContext { context: edge })
        .unwrap();
    let read = ReadService::new("invalid-site".into());
    let invalid = read
        .context(&invalid_context, &read.target(&invalid_context))
        .unwrap();
    assert_eq!(invalid["site_control_check"]["status"], "invalid");

    let mut large = s.clone();
    large.horizontal.as_mut().unwrap().origin_east_north_m = [123456789., 456789123.];
    let p = [12.345678, -98.765432];
    let round = large
        .from_reference_xy(large.to_reference_xy(p).unwrap())
        .unwrap();
    assert!((round[0] - p[0]).abs() < 1e-7 && (round[1] - p[1]).abs() < 1e-7);
}
#[test]
fn controls_report_consistency_mismatch_or_missing_inputs_without_solving() {
    let mut s = context();
    assert!(matches!(
        s.control_check().unwrap(),
        ControlCheck::Measured {
            consistent: true,
            ..
        }
    ));
    s.controls[1].reference_east_north_m[0] += 0.01;
    let before = s.clone();
    assert!(
        matches!(s.control_check().unwrap(),ControlCheck::Measured{consistent:false,max_residual_m,..} if (max_residual_m-0.01).abs()<1e-9)
    );
    assert_eq!(s, before);
    s.control_tolerance_m = None;
    assert!(matches!(
        s.control_check().unwrap(),
        ControlCheck::Insufficient { .. }
    ));
    s.controls[1].model_xy_m = s.controls[0].model_xy_m;
    assert!(s.validate().is_err());
}
#[test]
fn shared_host_header_delta_undo_and_portable_save_do_not_move_geometry() {
    let original = common::synthetic_building_98();
    let mut host = ApplicationHost::new(original.clone());
    let receipt = host
        .dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "site-reference".into(),
                commands: vec![ModelCommand::SetSiteContext { context: context() }],
            },
        )
        .unwrap()
        .unwrap();
    assert!(receipt.site_context_changed);
    assert!(receipt.changed_ids.is_empty());
    assert_eq!(host.document.editor.project.elements, original.elements);
    assert_eq!(host.document.editor.undo.len(), 1);
    let edited = host.document.editor.project.clone();
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, original);
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project, edited);
    let path =
        std::env::temp_dir().join(format!("oa-site-{}.json", storage::new_document_identity()));
    storage::save_document(&edited, &path, host.document.editor.project_identity()).unwrap();
    let reopened = Editor::from_stored_document(&path).unwrap();
    assert_eq!(reopened.project, edited);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn invalid_site_command_is_atomic_and_both_wire_adapters_match_host() {
    let original = common::synthetic_building_98();
    let mut editor = Editor::new(original.clone());
    let mut invalid = context();
    invalid.horizontal.as_mut().unwrap().origin_east_north_m[0] = f64::NAN;
    assert!(
        editor
            .apply(ModelCommand::SetSiteContext { context: invalid })
            .is_err()
    );
    assert_eq!(editor.project, original);
    assert!(editor.undo.is_empty());
    assert_eq!(editor.revision, 0);
    let raw = json!({"version":1,"edits":[{"operation":"set_site_context","context":context()}]});
    let native: NativeBatch = serde_json::from_value(raw.clone()).unwrap();
    let discovered: DiscoveredBatch = serde_json::from_value(raw).unwrap();
    let a = native.lower(&editor).unwrap();
    let b = discovered.resolve().unwrap().lower(&editor).unwrap();
    assert_eq!(a, b);
    let prepared = editor.prepare_batch_change(0, &a).unwrap();
    editor.commit_change("native-site", &prepared).unwrap();
    assert_eq!(editor.project.site, context());
    assert_eq!(editor.project.elements, original.elements);
    let descriptor = atelier_bim::automation_registry::describe("site_context").unwrap();
    assert_eq!(
        descriptor["parameters_schema"]["properties"]["operation"]["const"],
        "set_site_context"
    );
    assert!(descriptor["parameters_schema"]["$defs"]["SiteContext"].is_object());
}
#[test]
fn automation_preview_receipt_and_context_expose_site_change() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let read = ReadService::new("site-test".into());
    let target = read.target(&editor);
    let mut service = ChangeService::new("site-test".into(), true);
    let payload = json!({"target":target,"expected_revision":0,"change":{"version":1,"edits":[{"operation":"set_site_context","context":context()}]}});
    let preview = service.prepare_native(&editor, serde_json::from_value(payload).unwrap());
    assert!(
        preview["result"]["site_context_change"]["after"]["horizontal"].is_object(),
        "{preview}"
    );
    let result = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "site-wire".into(),
        },
    );
    assert_eq!(
        result["result"]["receipt"]["site_context_changed"], true,
        "{result}"
    );
    let queried = read.context(&editor, &target).unwrap();
    assert_eq!(queried["site"], serde_json::to_value(context()).unwrap());
    assert_eq!(queried["site_control_check"]["status"], "measured");
}

#[test]
fn native_site_file_marker_and_legacy_hash_receipt_shapes_are_explicit() {
    use atelier_bim::{model::ModelAdapter, storage::JsonAdapter};
    let mut project = common::synthetic_building_98();
    let legacy = JsonAdapter.export(&project).unwrap();
    assert_eq!(legacy, serde_json::to_vec_pretty(&project).unwrap());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&legacy).unwrap()["schema_version"],
        2
    );
    project.site = context();
    let bytes = JsonAdapter.export(&project).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 3);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), project);
    let mut unsafe_old = value.clone();
    unsafe_old["schema_version"] = json!(2);
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&unsafe_old).unwrap())
            .is_err()
    );
    let mut incomplete = value;
    incomplete.as_object_mut().unwrap().remove("site");
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&incomplete).unwrap())
            .is_err()
    );
    let mut editor = Editor::new(common::synthetic_building_98());
    let candidate = editor
        .prepare_change(
            0,
            vec![ModelCommand::MoveElements {
                ids: vec![editor.project.elements[0].id],
                offset: atelier_bim::model::Point::new(0.1, 0.),
            }],
        )
        .unwrap();
    let receipt = editor.commit_change("legacy-shaped", &candidate).unwrap();
    let wire = serde_json::to_value(&receipt).unwrap();
    assert!(wire.get("site_context_changed").is_none());
    let restored: atelier_bim::model_service::ChangeReceipt = serde_json::from_value(wire).unwrap();
    assert_eq!(restored, receipt);
}
