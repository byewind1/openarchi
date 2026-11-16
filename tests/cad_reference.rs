use atelier_bim::{
    cad_reference as cad,
    command::ModelCommand,
    drawings::DrawingDocument,
    editor::Editor,
    geometry::{self, SnapKind},
    model::{ModelAdapter, Point, Project},
    storage::{self, JsonAdapter},
};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("oa-cad-{}", storage::new_document_identity()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn source(&self) -> cad::Source {
        let path = self.0.join("drawing.dxf");
        fs::write(&path, dxf()).unwrap();
        cad::prepare(&path, None, &self.0.join("cache")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn dxf() -> String {
    "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB\n10\n100\n20\n200\n0\nLINE\n5\nA\n8\n0\n10\n100\n20\n200\n11\n1100\n21\n200\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n5\nB\n8\nSurvey\n2\nB\n10\n50000\n20\n70000\n41\n2\n42\n2\n50\n90\n0\nARC\n5\nC\n8\nCurve\n10\n0\n20\n0\n40\n1000\n50\n0\n51\n90\n0\nLWPOLYLINE\n5\nD\n8\nCurve\n90\n2\n70\n0\n10\n0\n20\n0\n42\n1\n10\n2000\n20\n0\n0\nTEXT\n5\nE\n8\nLabels\n10\n50000\n20\n70000\n40\n200\n1\nTest \\U+7CAE\n0\nACAD_PROXY_ENTITY\n5\nF\n8\nProxy\n0\nENDSEC\n0\nEOF\n".into()
}
#[test]
fn block_base_rotation_curves_identity_layers_and_loss_are_explicit() {
    let data = atelier_bim::dxf::reference::parse(dxf().as_bytes()).unwrap();
    let line = &data.primitives[0];
    assert_eq!(line.layer, "Survey");
    assert!(line.identity.contains('A'));
    assert_eq!(line.points[0], [50000., 70000.]);
    assert!((line.points[1][0] - 50000.).abs() < 1e-8);
    assert!((line.points[1][1] - 72000.).abs() < 1e-8);
    assert!(data.primitives[1].points.len() > 20);
    assert!(data.primitives[2].points.iter().any(|p| p[1] < -999.));
    assert!(
        data.primitives[3]
            .text
            .as_ref()
            .unwrap()
            .value
            .contains('粮')
    );
    assert_eq!(data.report.omitted["ACAD_PROXY_ENTITY"], 1);
    assert_eq!(data.report.units, Some(4));
}
#[test]
fn same_transform_and_locked_snap_are_undoable_portable_and_excluded_from_elements() {
    let f = Fixture::new();
    let source = f.source();
    let t = cad::Transform {
        anchor: [50000., 70000.],
        metres_per_unit: 0.001,
        xy: Point::new(8., 9.),
        angle: 90.,
        elevation: 3.5,
    };
    let (mut doc, worksheet, _) =
        cad::attach(&DrawingDocument::default(), source, None, t.clone(), true).unwrap();
    doc.cad_underlays[0].show_3d = true;
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    editor
        .apply(ModelCommand::SetDrawings { document: doc })
        .unwrap();
    assert_eq!(editor.project.elements, before.elements);
    let a = t.world([50000., 70000.]);
    let b = t.world([50000., 72000.]);
    assert_eq!(a, Point::new(8., 9.));
    assert!((b.x - 6.).abs() < 1e-10);
    assert!((a.distance(b) - 2.).abs() < 1e-10);
    assert_eq!(t.elevation, 3.5);
    assert_eq!(t.native(b), [50000., 72000.]);
    editor.switch_worksheet(Some(worksheet));
    let (point, kind) = geometry::snap(&editor.project, 0, Point::new(6.01, 9.), 0.1, true);
    assert_eq!(kind, SnapKind::Endpoint);
    assert!((point.x - 6.).abs() < 1e-9);
    assert!(editor.project.drawings.cad_underlays[0].locked);
    editor.undo();
    assert!(editor.project.drawings.cad_sources.is_empty());
    editor.redo();
    assert_eq!(editor.project.drawings.cad_sources.len(), 1);
    let path = f.0.join("project.json");
    storage::save(&editor.project, &path).unwrap();
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 35);
    assert!(
        raw["drawings"]["cad_sources"][0]["original"]["path"]
            .as_str()
            .unwrap()
            .starts_with("project.json.assets/")
    );
    let moved = f.0.join("moved");
    fs::create_dir(&moved).unwrap();
    fs::rename(
        f.0.join("project.json.assets"),
        moved.join("project.json.assets"),
    )
    .unwrap();
    fs::rename(path, moved.join("project.json")).unwrap();
    let loaded = storage::load(&moved.join("project.json")).unwrap();
    assert_eq!(loaded.elements, before.elements);
    assert_eq!(
        cad::snap(&loaded.drawings, Some(worksheet), Point::new(6.01, 9.), 0.1).unwrap(),
        point
    );
    let mut old = raw.clone();
    old["schema_version"] = serde_json::json!(34);
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&old).unwrap())
            .is_err()
    );
}
#[test]
fn unconfirmed_empty_wrong_units_and_unsupported_ocs_do_not_silently_commit() {
    let f = Fixture::new();
    let source = f.source();
    assert!(
        cad::attach(
            &DrawingDocument::default(),
            source.clone(),
            None,
            cad::Transform::default(),
            false
        )
        .is_err()
    );
    let mut t = cad::Transform::default();
    t.metres_per_unit = 0.;
    assert!(cad::attach(&DrawingDocument::default(), source, None, t, true).is_err());
    assert_eq!(cad::unit_scale(Some(0)), None);
    let d = dxf()
        .replace("70\n4\n", "70\n0\n")
        .replace("40\n1000\n50", "210\n1\n40\n1000\n50");
    let data = atelier_bim::dxf::reference::parse(d.as_bytes()).unwrap();
    assert_eq!(data.report.units, Some(0));
    assert_eq!(data.report.omitted["ARC: non-XY OCS"], 1);
}

#[test]
fn broken_or_misdirected_caches_rebuild_from_the_same_source_without_manual_cleanup() {
    let f = Fixture::new();
    let source = f.source();
    let expected = source.data.as_ref().unwrap().primitives.clone();
    let original = fs::read(&source.original.path).unwrap();
    let index = f.0.join("cache").join(format!(
        "{}-v{}.cache",
        source.dxf.sha256,
        cad::PARSER_VERSION
    ));
    fs::write(&index, b"interrupted cache index").unwrap();
    let rebuilt = cad::prepare(&f.0.join("drawing.dxf"), None, &f.0.join("cache")).unwrap();
    assert_eq!(rebuilt.data.as_ref().unwrap().primitives, expected);
    let other = f.0.join("other.dxf");
    fs::write(&other, dxf().replace("70000", "80000")).unwrap();
    let unrelated = cad::prepare(&other, None, &f.0.join("cache")).unwrap();
    fs::write(&index, serde_json::to_vec(&unrelated.parsed).unwrap()).unwrap();
    let rechecked = cad::prepare(&f.0.join("drawing.dxf"), None, &f.0.join("cache")).unwrap();
    assert_eq!(rechecked.data.as_ref().unwrap().primitives, expected);
    assert_eq!(
        rechecked.data.as_ref().unwrap().source_sha256,
        source.dxf.sha256
    );
    assert_eq!(fs::read(&source.original.path).unwrap(), original);
    let thumbnail = cad::thumbnail(rechecked.data.as_ref().unwrap(), 300, 120);
    assert!(thumbnail.pixels().any(|p| p.0[0] < 100));
    assert_eq!(
        rechecked.data.as_ref().unwrap().primitives,
        expected,
        "overview must not reduce authoritative vectors"
    );
}

#[test]
fn cache_recovery_roundtrips_and_missing_original_clears_previous_geometry() {
    let f = Fixture::new();
    let mut source = f.source();
    let expected = source.data.as_ref().unwrap().primitives.clone();
    fs::write(&source.parsed.path, b"damaged parsed asset").unwrap();
    source.load().unwrap();
    assert_eq!(source.data.as_ref().unwrap().primitives, expected);
    let (doc, _, _) = cad::attach(
        &DrawingDocument::default(),
        source.clone(),
        None,
        cad::Transform::default(),
        true,
    )
    .unwrap();
    let project = Project {
        drawings: doc,
        ..Default::default()
    };
    let path = f.0.join("recovered.json");
    storage::save(&project, &path).unwrap();
    let reopened = storage::load(&path).unwrap();
    assert_eq!(
        reopened.drawings.cad_sources[0]
            .data
            .as_ref()
            .unwrap()
            .primitives,
        expected
    );
    fs::remove_file(&source.original.path).unwrap();
    assert!(source.load().is_err());
    assert!(
        source.data.is_none(),
        "failed reload must not leave stale visible data"
    );
    assert!(source.load_error.is_some());
}

#[test]
fn source_update_retains_every_placement_and_requires_alignment_acknowledgement() {
    let f = Fixture::new();
    let source = f.source();
    let (mut doc, worksheet, _) = cad::attach(
        &DrawingDocument::default(),
        source.clone(),
        None,
        cad::Transform::default(),
        true,
    )
    .unwrap();
    let mut copy = doc.cad_underlays[0].clone();
    copy.id = doc.next_id().unwrap();
    copy.transform.xy = Point::new(12., 34.);
    copy.transform.angle = 32.;
    copy.transform.elevation = 4.2;
    copy.hidden_layers.insert("Labels".into());
    copy.show_3d = true;
    copy.locked = false;
    doc.cad_underlays.push(copy);
    let placements = doc.cad_underlays.clone();
    let id = source.id.max(doc.cad_sources[0].id);
    let path = f.0.join("updated.dxf");
    fs::write(
        &path,
        dxf()
            .replace("70\n4\n", "70\n6\n")
            .replace("70000", "75000"),
    )
    .unwrap();
    let replacement = cad::prepare(&path, None, &f.0.join("cache")).unwrap();
    assert!(cad::replace_source(&doc, id, replacement.clone(), false).is_err());
    let updated = cad::replace_source(&doc, id, replacement, true).unwrap();
    assert_eq!(updated.cad_underlays, placements);
    assert_eq!(updated.worksheets[0].id, worksheet);
    assert_eq!(updated.cad_sources[0].id, id);
    assert_ne!(
        updated.cad_sources[0].original.sha256,
        source.original.sha256
    );
    let mut editor = Editor::new(Project {
        drawings: doc.clone(),
        ..Default::default()
    });
    editor
        .apply(ModelCommand::SetDrawings {
            document: updated.clone(),
        })
        .unwrap();
    editor.undo();
    assert_eq!(editor.project.drawings, doc);
    editor.redo();
    assert_eq!(editor.project.drawings, updated);
}

#[test]
fn plan_visibility_controls_snapping_independently_of_3d_and_survives_native_version_guard() {
    let f = Fixture::new();
    let (mut doc, worksheet, _) = cad::attach(
        &DrawingDocument::default(),
        f.source(),
        None,
        cad::Transform::default(),
        true,
    )
    .unwrap();
    let u = &mut doc.cad_underlays[0];
    u.show_3d = true;
    u.fade = 220;
    assert_eq!(u.grey(), 255);
    let point = Point::new(50000.01, 70000.);
    assert!(cad::snap(&doc, None, point, 0.1).is_none());
    assert!(cad::snap(&doc, Some(worksheet), point, 0.1).is_some());
    doc.cad_underlays[0].show_plan = true;
    assert!(doc.cad_underlays[0].visible_in_plan(None));
    assert_eq!(
        cad::snap(&doc, None, point, 0.1),
        Some(Point::new(50000., 70000.))
    );
    doc.cad_underlays[0].hidden_layers.insert("Survey".into());
    assert!(cad::snap(&doc, None, point, 0.1).is_none());
    doc.cad_underlays[0].hidden_layers.clear();
    doc.cad_underlays[0].visible = false;
    assert!(cad::snap(&doc, None, point, 0.1).is_none());
    doc.cad_underlays[0].visible = true;
    let project = Project {
        drawings: doc,
        ..Default::default()
    };
    let bytes = JsonAdapter.export(&project).unwrap();
    let mut raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(raw["schema_version"], 36);
    assert!(JsonAdapter.import(&bytes).unwrap().drawings.cad_underlays[0].show_plan);
    raw["schema_version"] = serde_json::json!(35);
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&raw).unwrap())
            .is_err()
    );
}

#[test]
fn declared_gbk_labels_decode_and_explicit_update_preserves_hidden_layer_identity() {
    let f = Fixture::new();
    let text = "0\nSECTION\n2\nHEADER\n9\n$DWGCODEPAGE\n3\nANSI_936\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nA\n8\n粮干底图\n10\n0\n20\n0\n11\n1000\n21\n0\n0\nTEXT\n5\nB\n8\n文字层\n10\n0\n20\n0\n40\n20\n1\n教学用房\n0\nENDSEC\n0\nEOF\n";
    let (bytes, _, errors) = encoding_rs::GBK.encode(text);
    assert!(!errors);
    let path = f.0.join("legacy.dxf");
    fs::write(&path, &bytes).unwrap();
    let source = cad::prepare(&path, None, &f.0.join("cache")).unwrap();
    let data = source.data.as_ref().unwrap();
    assert_eq!(source.report.text_encoding.as_deref(), Some("GBK"));
    assert_eq!(data.primitives[0].layer, "粮干底图");
    assert_eq!(data.primitives[1].text.as_ref().unwrap().value, "教学用房");
    let mut previous = source.clone();
    let mut old = (**data).clone();
    old.version = 2;
    old.report.text_encoding = None;
    old.primitives[0].layer = "????".into();
    previous.report.text_encoding = None;
    previous.parsed = cad::store(
        &serde_json::to_vec(&old).unwrap(),
        &f.0.join("cache"),
        "cad.json",
    )
    .unwrap();
    previous.data = Some(std::sync::Arc::new(old));
    let (mut doc, worksheet, _) = cad::attach(
        &DrawingDocument::default(),
        previous.clone(),
        None,
        cad::Transform::default(),
        true,
    )
    .unwrap();
    let id = doc.cad_sources[0].id;
    doc.cad_underlays[0].hidden_layers.insert("????".into());
    previous.load().unwrap();
    assert_eq!(
        previous.data.as_ref().unwrap().primitives[0].layer,
        "????",
        "an existing loaded interpretation must not silently change"
    );
    fs::remove_file(&previous.parsed.path).unwrap();
    assert!(previous.load().is_err());
    assert!(previous.data.is_none());
    let updated = cad::replace_source(&doc, id, source, true).unwrap();
    assert_eq!(updated.cad_underlays[0].id, doc.cad_underlays[0].id);
    assert!(updated.cad_underlays[0].hidden_layers.contains("粮干底图"));
    assert!(cad::snap(&updated, Some(worksheet), Point::default(), 0.1).is_none());
    let project = Project {
        drawings: updated,
        ..Default::default()
    };
    let raw = JsonAdapter.export(&project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap()["schema_version"],
        36
    );
}

#[test]
fn display_levels_bound_curve_error_and_keep_full_precision_snap_vertices() {
    let f = Fixture::new();
    let source = f.source();
    let arc = &source.data.as_ref().unwrap().primitives[1];
    let raw = arc.points.clone();
    let display: Vec<_> = arc.display_points(20.).collect();
    assert!(display.len() < raw.len());
    assert_eq!(display.first(), raw.first());
    assert_eq!(display.last(), raw.last());
    for p in &raw {
        let distance = display
            .windows(2)
            .map(|line| {
                let dx = line[1][0] - line[0][0];
                let dy = line[1][1] - line[0][1];
                let t = (((p[0] - line[0][0]) * dx + (p[1] - line[0][1]) * dy)
                    / (dx * dx + dy * dy))
                    .clamp(0., 1.);
                (p[0] - line[0][0] - t * dx).hypot(p[1] - line[0][1] - t * dy)
            })
            .fold(f64::INFINITY, f64::min);
        assert!(distance <= 20. + 1e-8);
    }
    assert_eq!(arc.points, raw);
    let mut text =
        "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n5\nA\n8\nLine\n90\n11\n70\n0\n".to_string();
    for x in 0..=10 {
        text.push_str(&format!("10\n{x}\n20\n0\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    let path = f.0.join("many-nodes.dxf");
    fs::write(&path, text).unwrap();
    let source = cad::prepare(&path, None, &f.0.join("cache")).unwrap();
    assert_eq!(
        source.data.as_ref().unwrap().primitives[0]
            .display_points(1.)
            .count(),
        2
    );
    let (doc, worksheet, _) = cad::attach(
        &DrawingDocument::default(),
        source,
        None,
        cad::Transform::default(),
        true,
    )
    .unwrap();
    assert_eq!(
        cad::snap(&doc, Some(worksheet), Point::new(5., 0.), 1e-6),
        Some(Point::new(5., 0.))
    );
}
