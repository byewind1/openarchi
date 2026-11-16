use atelier_bim::{
    automation_registry::{NativeBatch, describe},
    concept_mass::{self, ConceptMassSpec},
    editor::{Editor, Tool},
    geometry,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::{JsonAdapter, MAX_PROJECT_BYTES},
};
use serde_json::json;
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h)]
}
fn spec() -> ConceptMassSpec {
    ConceptMassSpec {
        arcs: vec![],
        hole_arcs: vec![],
        outline: rect(0., 0., 8., 6.),
        holes: vec![rect(2., 2., 2., 2.)],
        height: 6.,
        bottom_offset: 0.5,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}")
}
#[test]
fn true_mass_with_courtyard_uses_shared_mesh_picking_metrics_and_never_zone_or_material_totals() {
    let mut project = Project::default();
    let id = project.add(0, Material::Plaster, Shape::ConceptMass { spec: spec() });
    project.validate().unwrap();
    let e = &project.elements[0];
    assert_eq!(e.kind(), "Concept mass");
    close(e.area(), 44.);
    let g = concept_mass::derive(e, 3.2).unwrap();
    close(g.volume_m3, 264.);
    close(g.bounds[0].z, 3.7);
    close(g.bounds[1].z, 9.7);
    let surface =
        atelier_bim::derived_geometry::inspect_closed_surface(&g.triangles().collect::<Vec<_>>())
            .unwrap();
    close(surface.signed_volume_m3, 264.);
    let ed = Editor::new(project);
    let view = || atelier_bim::render::SceneView {
        selected: &[],
        hovered: None,
        story: None,
        marquee: None,
        outside_marquee: false,
        selection_only: false,
    };
    let uncached = atelier_bim::render::prepare_scene(&ed.project, view(), None);
    let scene = ed
        .with_geometry_read(|read| {
            atelier_bim::render::prepare_scene(&ed.project, view(), Some(read))
        })
        .unwrap();
    assert!(scene.diagnostics.is_empty());
    assert_eq!(scene, uncached);
    assert!(scene.triangle_count() > 0 && scene.line_count() > 0);
    let reader = atelier_bim::automation::ReadService::new("mass-quantities".into());
    let query = atelier_bim::automation::Query {
        target: reader.target(&ed),
        expected_revision: ed.revision,
        ids: vec![],
        kind: Some("Concept mass".into()),
        story: None,
        region: None,
        fields: vec!["id".into(), "shape".into(), "quantities".into()],
        limit: 10,
        cursor: None,
        design_scope: Default::default(),
    };
    let result = reader.query(&ed, &query).unwrap();
    assert_eq!(result["summary"]["count"], 1);
    close(
        result["elements"][0]["quantities"]["concept_volume_m3"]
            .as_f64()
            .unwrap(),
        264.,
    );
    assert!(result["elements"][0]["quantities"]["material_volume_m3"].is_null());
    assert!(result["elements"][0]["quantities"]["space_volume_m3"].is_null());
    let cached = ed.geometry_source(id).unwrap();
    close(cached.source.concept_volume_m3().unwrap(), 264.);
    assert_eq!(cached.source.material_volume_m3(), None);
    assert!(
        atelier_bim::zones::analysis::analyze(&ed.project, &Default::default())
            .unwrap()
            .spaces
            .is_empty()
    );
    assert_eq!(geometry::hit_plan(&ed.project, 0, p(3., 3.), 0.01), None);
    assert_eq!(
        geometry::hit_plan(&ed.project, 0, p(1., 1.), 0.01),
        Some(id)
    );
    assert!(!geometry::element_intersects_polygon(
        &ed.project,
        &ed.project.elements[0],
        &rect(2.4, 2.4, 0.2, 0.2)
    ));
    assert!(
        cached
            .source
            .ray_hit(
                glam::DVec3::new(3., 3., 20.),
                glam::DVec3::NEG_Z,
                None,
                None
            )
            .unwrap()
            .is_none()
    );
    assert!(
        cached
            .source
            .ray_hit(
                glam::DVec3::new(1., 1., 20.),
                glam::DVec3::NEG_Z,
                None,
                None
            )
            .unwrap()
            .is_some()
    );
}
#[test]
fn mass_wire_preview_cancel_update_transform_lock_undo_and_format9_roundtrip() {
    let mut ed = Editor::new(Project::default());
    let baseline = ed.project.clone();
    let geom = json!({"type":"concept_mass","outline":rect(0.,0.,8.,6.),"holes":[rect(2.,2.,2.,2.)],"height":6.,"bottom_offset":0.5});
    let b:NativeBatch=serde_json::from_value(json!({"version":1,"creates":[{"name":"intent_envelope","story":0,"layer":0,"material":"plaster","geometry":geom}]})).unwrap();
    let prepared = ed
        .prepare_batch_change(ed.revision, &b.lower(&ed).unwrap())
        .unwrap();
    assert_eq!(ed.project, baseline);
    drop(prepared);
    assert!(ed.undo.is_empty());
    let prepared = ed
        .prepare_batch_change(ed.revision, &b.lower(&ed).unwrap())
        .unwrap();
    let r = ed.commit_change("mass-original", &prepared).unwrap();
    let id = r.changed_ids[0];
    let original = ed.project.clone();
    let mut edit = geom.clone();
    edit["height"] = json!(7.);
    let b:NativeBatch=serde_json::from_value(json!({"version":1,"edits":[{"operation":"update","target":{"kind":"existing","value":id},"geometry":edit},{"operation":"move","targets":[{"kind":"existing","value":id}],"offset":{"x":20.,"y":0.}}]})).unwrap();
    let prepared = ed
        .prepare_batch_change(ed.revision, &b.lower(&ed).unwrap())
        .unwrap();
    ed.commit_change("mass-feedback", &prepared).unwrap();
    close(
        ed.geometry_source(id)
            .unwrap()
            .source
            .concept_volume_m3()
            .unwrap(),
        308.,
    );
    assert_eq!(geometry::hit_plan(&ed.project, 0, p(23., 3.), 0.01), None);
    assert_eq!(
        geometry::hit_plan(&ed.project, 0, p(21., 1.), 0.01),
        Some(id)
    );
    let bytes = JsonAdapter.export(&ed.project).unwrap();
    assert!((bytes.len() as u64) < MAX_PROJECT_BYTES);
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 9);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), ed.project);
    for old in 2..=8 {
        v["schema_version"] = json!(old);
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&v).unwrap())
                .is_err()
        );
    }
    ed.undo();
    assert_eq!(ed.project, original);
    ed.redo();
    assert_eq!(ed.project, JsonAdapter.import(&bytes).unwrap());
    ed.project.edit_state.locked.insert(id);
    let before = ed.project.clone();
    assert!(
        ed.prepare_batch_change(ed.revision, &b.lower(&ed).unwrap())
            .is_err()
    );
    assert_eq!(ed.project, before);
    assert!(describe("concept_mass").unwrap()["concept_mass_semantics"].is_object());
}
#[test]
fn native_mass_polygon_rejects_invalid_input_and_plan_transforms_preserve_the_declared_height() {
    let mut ed = Editor::new(Project::default());
    let before = ed.project.clone();
    let mut bad = spec();
    bad.holes = vec![rect(7., 5., 2., 2.)];
    assert!(ed.place_concept_mass(bad).is_err());
    assert_eq!(ed.project, before);
    ed.set_tool(Tool::ConceptMass);
    ed.zone_polygon = true;
    ed.concept_mass_height = 4.;
    for q in [
        p(0., 0.),
        p(4., 0.),
        p(4., 1.),
        p(1., 1.),
        p(1., 4.),
        p(0., 4.),
        p(0., 0.),
    ] {
        ed.click(q, 0.01);
    }
    assert_eq!(ed.project.elements.len(), 1);
    close(ed.project.elements[0].area(), 7.);
    let id = ed.project.elements[0].id;
    ed.rotate_selection_quarter_turn(true);
    close(ed.project.elements[0].area(), 7.);
    close(
        ed.geometry_source(id)
            .unwrap()
            .source
            .concept_volume_m3()
            .unwrap(),
        28.,
    );
    let mut bad = ed.project.elements[0].clone();
    if let Shape::ConceptMass { spec } = &mut bad.shape {
        spec.height = 0.;
    }
    assert!(
        ed.prepare_change(
            ed.revision,
            vec![atelier_bim::command::ModelCommand::UpdateElement { element: bad }]
        )
        .is_err()
    );
}

#[test]
fn curved_mass_exact_kernel_transforms_history_and_versioned_reopen() {
    use atelier_bim::{
        editing::EditCommand,
        reshape::{self, ReshapeCommand},
        slab_geometry::SlabChange as C,
    };
    let mut e = Editor::new(Project::default());
    let id = e.place_concept_mass(spec()).unwrap();
    let before = e.project.clone();
    let op = |index, change, anchor, point| ReshapeCommand::MassFootprint {
        id,
        index,
        change,
        anchor,
        point,
    };
    let curve = op(0, C::Curve, p(4., 0.), p(4., -2.));
    let (candidate, _) = reshape::candidate(&e.project, &curve).unwrap();
    assert_eq!(e.project, before, "preview is pure");
    e.reshape(curve).unwrap();
    assert_eq!(e.project, candidate);
    let curved = e.project.clone();
    let Shape::ConceptMass { spec: s } = &curved.elements[0].shape else {
        panic!()
    };
    let arc = s.arcs[0].unwrap();
    close(arc.radius, 5.);
    let (_, sweep) = geometry::arc_center_sweep(s.outline[0], s.outline[1], arc).unwrap();
    let expected = 44. + 25. * (sweep - sweep.sin()) * 0.5;
    close(s.area(), expected);
    let mesh = concept_mass::derive(&curved.elements[0], 0.).unwrap();
    close(mesh.plan_area_m2, expected);
    close(mesh.volume_m3, expected * 6.);
    assert!(
        geometry::element_plan_outline(&curved, &curved.elements[0])
            .iter()
            .any(|p| p.y < -1.9)
    );
    assert_eq!(geometry::hit_plan(&curved, 0, p(4., -1.), 0.01), Some(id));
    let json = JsonAdapter.export(&curved).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(value["schema_version"], 37);
    assert_eq!(JsonAdapter.import(&json).unwrap(), curved);
    value["schema_version"] = json!(9);
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, curved);
    let inverse = C::InsertArcNode;
    e.reshape(op(0, inverse, p(4., -2.), p(4., -2.))).unwrap();
    let Shape::ConceptMass { spec: s } = &e.project.elements[0].shape else {
        panic!()
    };
    assert_eq!(s.outline.len(), 5);
    assert!(s.arcs[0].is_some() && s.arcs[1].is_some());
    close(s.area(), expected);
    e.undo();
    assert_eq!(e.project, curved);
    // Reflection must reverse exact arc orientation; scale must preserve radius.
    e.set_selection(vec![id]);
    e.edit(EditCommand::Transform {
        offset: p(20., 1.),
        origin: p(0., 0.),
        angle: 30.,
        scale: 2.,
        mirror: true,
        copies: 0,
    })
    .unwrap();
    let Shape::ConceptMass { spec: s } = &e.project.elements[0].shape else {
        panic!()
    };
    close(s.arcs[0].unwrap().radius, 10.);
    assert_ne!(s.arcs[0].unwrap().clockwise, arc.clockwise);
    close(s.area(), expected * 4.);
    concept_mass::derive(&e.project.elements[0], 0.).unwrap();
    e.undo();
    assert_eq!(e.project, curved);
    e.project.edit_state.locked.insert(id);
    let locked = e.project.clone();
    let history = e.undo.len();
    assert!(e.reshape(op(0, C::Curve, p(4., 0.), p(4., -1.))).is_err());
    assert_eq!(e.project, locked);
    assert_eq!(e.undo.len(), history);
}

#[test]
fn mass_pickup_arms_creation_defaults_scope_and_injection_preserve_footprint() {
    use atelier_bim::transfer::{GROUP_GEOMETRY, TransferSettings};
    let mut e = Editor::new(Project::default());
    let id = e.place_concept_mass(spec()).unwrap();
    let original = e.project.clone();
    let h = e.undo.len();
    let mut settings = TransferSettings::default();
    e.set_tool(Tool::Wall);
    e.pick_up_from(id, &settings).unwrap();
    assert_eq!(e.tool, Tool::ConceptMass);
    close(e.concept_mass_height, 6.);
    close(e.concept_mass_bottom_offset, 0.5);
    assert_eq!(e.project, original);
    assert_eq!(e.undo.len(), h);
    e.click(p(20., 0.), 0.01);
    e.click(p(23., 4.), 0.01);
    let new = e.project.elements.last().unwrap().clone();
    let Shape::ConceptMass { spec: s } = &new.shape else {
        panic!()
    };
    close(s.height, 6.);
    close(s.bottom_offset, 0.5);
    close(s.area(), 12.);
    settings.sets[settings.active].groups.remove(GROUP_GEOMETRY);
    e.concept_mass_height = 8.;
    e.pick_up_from(id, &settings).unwrap();
    close(e.concept_mass_height, 8.);
    settings.sets[settings.active]
        .groups
        .insert(GROUP_GEOMETRY.into());
    let mut changed = new.clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        spec.height = 2.;
    }
    e.replace_element(changed).unwrap();
    e.inject_into(&[new.id], &settings).unwrap();
    assert_eq!(e.project.elements.last().unwrap(), &new);
}

#[test]
fn mass_full_contour_adapter_curves_holes_corner_offset_tangent_and_elevation() {
    use atelier_bim::{reshape::ReshapeCommand, slab_geometry::SlabChange as C};
    let mut base = Editor::new(Project::default());
    let id = base.place_concept_mass(spec()).unwrap();
    for (index, change, anchor, point) in [
        (0, C::Tangent, p(0., 0.), p(1., -1.)),
        (0, C::OffsetEdge, p(4., 0.), p(4., -1.)),
        (0, C::OffsetAll, p(4., 0.), p(4., -0.2)),
        (0, C::Fillet(0.5), p(0., 0.), p(0., 0.)),
        (0, C::Chamfer(0.5), p(0., 0.), p(0., 0.)),
        (0, C::Elevate(2.), p(0., 0.), p(0., 0.)),
        (4, C::Curve, p(3., 2.), p(3., 1.8)),
    ] {
        let mut e = Editor::new(base.project.clone());
        let before = e.project.clone();
        e.reshape(ReshapeCommand::MassFootprint {
            id,
            index,
            change: change.clone(),
            anchor,
            point,
        })
        .unwrap_or_else(|error| panic!("{change:?}: {error}"));
        let after = e.project.clone();
        assert_ne!(after, before);
        let Shape::ConceptMass { spec: s } = &after.elements[0].shape else {
            panic!()
        };
        close(s.height, 6.);
        close(
            s.bottom_offset,
            if matches!(change, C::Elevate(_)) {
                2.5
            } else {
                0.5
            },
        );
        let g = concept_mass::derive(&after.elements[0], 0.).unwrap();
        close(g.plan_area_m2, s.area());
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, after);
        assert_eq!(
            JsonAdapter
                .import(&JsonAdapter.export(&after).unwrap())
                .unwrap(),
            after
        );
    }
}
