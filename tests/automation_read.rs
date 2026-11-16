use atelier_bim::{
    automation::{Query, ReadService},
    editor::Editor,
    model::Project,
};

#[test]
fn authored_slab_loops_are_readable_without_changing_model_or_history() {
    use atelier_bim::automation_registry::NativeBatch;
    use serde_json::json;
    let mut editor = Editor::new(Project::default());
    let request: NativeBatch = serde_json::from_value(json!({"version":1,"creates":[{
        "name":"contour-slab","story":0,"layer":0,"material":"concrete",
        "geometry":{"type":"slab","outline":[{"x":0.,"y":0.},{"x":8.,"y":0.},{"x":8.,"y":6.},{"x":2.,"y":6.},{"x":0.,"y":4.}],
        "holes":[[{"x":3.,"y":1.},{"x":4.,"y":1.},{"x":4.,"y":2.},{"x":3.,"y":2.}]],"thickness":0.2,"elevation":0.4}
    }]})).unwrap();
    let prepared = editor
        .prepare_batch_change(0, &request.lower(&editor).unwrap())
        .unwrap();
    editor
        .commit_change("slab-contour-fixture", &prepared)
        .unwrap();
    let before = editor.project.clone();
    let state = (
        editor.revision,
        editor.undo.len(),
        editor.redo.len(),
        editor.dirty(),
        editor.selected_ids.clone(),
    );
    let service = ReadService::new("contour-query".into());
    let result = service
        .query(
            &editor,
            &Query {
                design_scope: Default::default(),
                target: service.target(&editor),
                expected_revision: editor.revision,
                ids: vec![1],
                kind: None,
                story: None,
                region: None,
                fields: vec![
                    "id".into(),
                    "shape".into(),
                    "slab_contour".into(),
                    "wall_contour".into(),
                ],
                limit: 1,
                cursor: None,
            },
        )
        .unwrap();
    let row = &result["elements"][0];
    assert_eq!(
        row["shape"],
        serde_json::to_value(&before.elements[0].shape).unwrap()
    );
    assert_eq!(
        row["slab_contour"],
        serde_json::to_value(&before.elements[0].slab_contour).unwrap()
    );
    assert_eq!(row["slab_contour"]["points"].as_array().unwrap().len(), 5);
    assert_eq!(
        row["slab_contour"]["holes"][0]["points"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(row["slab_contour"]["elevation"], 0.4);
    assert!(row["wall_contour"].is_null());
    assert_eq!(editor.project, before);
    assert_eq!(
        (
            editor.revision,
            editor.undo.len(),
            editor.redo.len(),
            editor.dirty(),
            editor.selected_ids.clone()
        ),
        state
    );
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&before).unwrap()).unwrap();
    assert_eq!(restored, before);
}

#[test]
fn paginated_read_is_bound_and_preserves_model_and_history() {
    let mut editor = Editor::new(Project::demo());
    let service = ReadService::new("test-instance".into());
    let before = serde_json::to_value(&editor.project).unwrap();
    let selection = editor.selected_ids.clone();
    let history = (
        editor.undo.len(),
        editor.redo.len(),
        editor.revision,
        serde_json::to_value(&editor.saved).unwrap(),
    );
    let mut q = Query {
        design_scope: Default::default(),
        region: None,
        target: service.target(&editor),
        expected_revision: editor.revision,
        ids: vec![],
        kind: None,
        story: None,
        fields: vec!["id".into(), "kind".into()],
        limit: 1,
        cursor: None,
    };
    let mut ids = vec![];
    loop {
        let page = service.query(&editor, &q).unwrap();
        ids.extend(
            page["elements"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["id"].as_u64().unwrap()),
        );
        q.cursor = page["next_cursor"].as_str().map(str::to_owned);
        if q.cursor.is_none() {
            break;
        }
    }
    let mut expected: Vec<_> = editor.project.elements.iter().map(|e| e.id).collect();
    expected.sort();
    assert_eq!(ids, expected);
    assert_eq!(serde_json::to_value(&editor.project).unwrap(), before);
    assert_eq!(editor.selected_ids, selection);
    assert_eq!(
        (
            editor.undo.len(),
            editor.redo.len(),
            editor.revision,
            serde_json::to_value(&editor.saved).unwrap()
        ),
        history
    );
    q.target.instance_id = "foreign".into();
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "TARGET_MISMATCH"
    );
    q.target = service.target(&editor);
    q.expected_revision += 1;
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "REVISION_CONFLICT"
    );
    editor = Editor::new(editor.project.clone());
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "TARGET_MISMATCH"
    );
}

#[test]
fn rejects_cross_query_cursor_and_unbounded_output() {
    let editor = Editor::new(Project::demo());
    let service = ReadService::new("a".into());
    let mut q = Query {
        design_scope: Default::default(),
        region: None,
        target: service.target(&editor),
        expected_revision: editor.revision,
        ids: vec![],
        kind: None,
        story: None,
        fields: vec!["id".into()],
        limit: 1,
        cursor: None,
    };
    let page = service.query(&editor, &q).unwrap();
    q.cursor = Some(page["next_cursor"].as_str().unwrap().into());
    q.fields.push("shape".into());
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "INVALID_CURSOR"
    );
    q.cursor = None;
    q.limit = 101;
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "INVALID_LIMIT"
    );
    q.limit = 1;
    q.fields = vec!["arbitrary".into()];
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "INVALID_PROJECTION"
    );
}

#[test]
fn byte_budget_paginates_without_silently_omitting_rows() {
    let mut editor = Editor::new(Project::demo());
    for element in &mut editor.project.elements {
        element.name = "n".repeat(20000);
    }
    let service = ReadService::new("budget".into());
    let mut query = Query {
        design_scope: Default::default(),
        region: None,
        target: service.target(&editor),
        expected_revision: editor.revision,
        ids: vec![],
        kind: None,
        story: None,
        fields: vec!["id".into(), "name".into()],
        limit: 100,
        cursor: None,
    };
    let mut ids = vec![];
    loop {
        let page = service.query(&editor, &query).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() < 65536);
        ids.extend(
            page["elements"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["id"].as_u64().unwrap()),
        );
        query.cursor = page["next_cursor"].as_str().map(str::to_owned);
        if query.cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), editor.project.elements.len());
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), editor.project.elements.len());
    editor.project.elements[0].name = "n".repeat(60000);
    query.cursor = None;
    assert_eq!(
        service.query(&editor, &query).unwrap_err().code,
        "RESOURCE_REQUIRED"
    );
}

#[test]
fn derived_projections_match_shared_sources_and_keep_filter_totals_across_pages() {
    let editor = Editor::new(Project::demo());
    let service = ReadService::new("quantities".into());
    let before = serde_json::to_value(&editor.project).unwrap();
    let mut query = Query {
        design_scope: Default::default(),
        region: None,
        target: service.target(&editor),
        expected_revision: 0,
        ids: vec![],
        kind: None,
        story: None,
        fields: vec![
            "id".into(),
            "kind".into(),
            "bounds".into(),
            "quantities".into(),
            "validation".into(),
        ],
        limit: 2,
        cursor: None,
    };
    let mut read_ids = vec![];
    loop {
        let page = service.query(&editor, &query).unwrap();
        assert_eq!(page["summary"]["count"], editor.project.elements.len());
        assert_eq!(page["page_quantities"]["complete"], true);
        let mut volume = 0.;
        let mut void_volume = 0.;
        for row in page["elements"].as_array().unwrap() {
            let id = row["id"].as_u64().unwrap();
            read_ids.push(id);
            let source = editor.geometry_source(id).unwrap().source;
            assert_eq!(
                row["quantities"]["nominal"],
                serde_json::to_value(source.nominal_measure()).unwrap()
            );
            assert_eq!(
                row["quantities"]["material_volume_m3"],
                serde_json::json!(source.material_volume_m3())
            );
            assert_eq!(
                row["quantities"]["void_volume_m3"],
                serde_json::json!(source.void_volume_m3())
            );
            assert_eq!(row["validation"]["scope"], "derived_source");
            assert_eq!(row["validation"]["status"], "valid");
            if let Some([lo, hi]) = source.display_bounds() {
                assert_eq!(row["bounds"]["min"], serde_json::json!(lo.to_array()));
                assert_eq!(row["bounds"]["max"], serde_json::json!(hi.to_array()));
            }
            volume += source.material_volume_m3().unwrap_or(0.);
            void_volume += source.void_volume_m3().unwrap_or(0.);
        }
        assert!(
            (page["page_quantities"]["material_volume_m3"]
                .as_f64()
                .unwrap()
                - volume)
                .abs()
                < 1e-12
        );
        assert!(
            (page["page_quantities"]["void_volume_m3"].as_f64().unwrap() - void_volume).abs()
                < 1e-12
        );
        query.cursor = page["next_cursor"].as_str().map(str::to_owned);
        if query.cursor.is_none() {
            break;
        }
    }
    assert_eq!(read_ids.len(), editor.project.elements.len());
    assert_eq!(serde_json::to_value(&editor.project).unwrap(), before);
    assert_eq!(editor.revision, 0);
    assert!(editor.undo.is_empty() && editor.redo.is_empty());
    assert!(!editor.dirty());
}

#[test]
fn invalid_derived_source_returns_diagnostic_instead_of_zero_quantity() {
    let mut editor = Editor::new(Project::demo());
    let wall = editor
        .project
        .elements
        .iter_mut()
        .find(|e| e.kind() == "Wall")
        .unwrap();
    let id = wall.id;
    if let atelier_bim::model::Shape::Wall { height, .. } = &mut wall.shape {
        *height = 0.;
    }
    let service = ReadService::new("invalid".into());
    let result = service
        .query(
            &editor,
            &Query {
                design_scope: Default::default(),
                region: None,
                target: service.target(&editor),
                expected_revision: 0,
                ids: vec![id],
                kind: None,
                story: None,
                fields: vec!["id".into(), "quantities".into(), "validation".into()],
                limit: 1,
                cursor: None,
            },
        )
        .unwrap();
    assert_eq!(result["elements"][0]["quantities"]["status"], "invalid");
    assert_eq!(
        result["elements"][0]["validation"]["diagnostics"][0]["element_id"],
        id
    );
    assert_eq!(result["page_quantities"]["complete"], false);
    assert_eq!(
        result["page_quantities"]["failed_ids"],
        serde_json::json!([id])
    );
}

#[test]
fn region_filter_uses_inclusive_shared_bounds_and_is_bound_to_cursor() {
    use atelier_bim::automation::Region;
    let editor = Editor::new(Project::demo());
    let service = ReadService::new("region".into());
    let region = Region {
        min: [-100., -100., -100.],
        max: [100., 100., 100.],
    };
    let mut query = Query {
        design_scope: Default::default(),
        target: service.target(&editor),
        expected_revision: 0,
        ids: vec![],
        kind: None,
        story: None,
        region: Some(region),
        fields: vec!["id".into()],
        limit: 1,
        cursor: None,
    };
    let first = service.query(&editor, &query).unwrap();
    assert_eq!(first["summary"]["count"], editor.project.elements.len());
    query.cursor = first["next_cursor"].as_str().map(str::to_owned);
    assert!(query.cursor.is_some());
    query.region.as_mut().unwrap().max[0] = 90.;
    assert_eq!(
        service.query(&editor, &query).unwrap_err().code,
        "INVALID_CURSOR"
    );
    query.cursor = None;
    let id = first["elements"][0]["id"].as_u64().unwrap();
    query.ids = vec![id];
    let [lo, hi] = editor
        .geometry_source(id)
        .unwrap()
        .source
        .display_bounds()
        .unwrap();
    query.region = Some(Region {
        min: hi.to_array(),
        max: hi.to_array(),
    });
    assert_eq!(
        service.query(&editor, &query).unwrap()["summary"]["count"],
        1,
        "touching boundary counts"
    );
    query.region = Some(Region {
        min: [hi.x + 1., lo.y, lo.z],
        max: [hi.x + 2., hi.y, hi.z],
    });
    assert_eq!(
        service.query(&editor, &query).unwrap()["summary"]["count"],
        0
    );
    query.region.as_mut().unwrap().min[0] = f64::NAN;
    assert_eq!(
        service.query(&editor, &query).unwrap_err().code,
        "INVALID_REGION"
    );
    query.region = Some(Region {
        min: [1., 0., 0.],
        max: [0., 1., 1.],
    });
    assert_eq!(
        service.query(&editor, &query).unwrap_err().code,
        "INVALID_REGION"
    );
}

#[test]
fn whole_project_validation_has_global_counts_and_bound_diagnostic_pages() {
    use atelier_bim::automation::Validate;
    let mut editor = Editor::new(Project::demo());
    editor.project.elements.clear();
    for i in 0..3 {
        let mut bad = editor.polyline_creation_candidate(
            vec![
                atelier_bim::model::Point::default(),
                atelier_bim::model::Point::new(1., 0.),
            ],
            vec![Some(atelier_bim::model::ArcSegment {
                radius: 1e308,
                clockwise: false,
                large_arc: false,
            })],
        );
        bad.id = i + 1;
        editor.project.elements.push(bad);
    }
    editor.project.next_id = 4;
    let service = ReadService::new("validate".into());
    let before = serde_json::to_value(&editor.project).unwrap();
    let mut query = Validate {
        target: service.target(&editor),
        expected_revision: 0,
        limit: 1,
        cursor: None,
    };
    let mut diagnostics = vec![];
    let mut expected = None;
    loop {
        let page = service.validate(&editor, &query).unwrap();
        assert_eq!(page["valid"], false);
        assert!(page["error_count"].as_u64().unwrap() > 1);
        assert_eq!(page["scope"], "whole_project");
        if let Some(count) = expected {
            assert_eq!(page["diagnostic_count"], count);
        } else {
            expected = Some(page["diagnostic_count"].as_u64().unwrap());
        }
        diagnostics.extend(page["diagnostics"].as_array().unwrap().clone());
        query.cursor = page["next_cursor"].as_str().map(str::to_owned);
        if query.cursor.is_none() {
            break;
        }
    }
    assert_eq!(diagnostics.len() as u64, expected.unwrap());
    assert_eq!(serde_json::to_value(&editor.project).unwrap(), before);
    assert_eq!(editor.revision, 0);
    assert!(editor.undo.is_empty());
    query.cursor = service.validate(&editor, &query).unwrap()["next_cursor"]
        .as_str()
        .map(str::to_owned);
    query.limit = 2;
    assert_eq!(
        service.validate(&editor, &query).unwrap_err().code,
        "INVALID_CURSOR"
    );
    query.cursor = None;
    query.expected_revision = 1;
    assert_eq!(
        service.validate(&editor, &query).unwrap_err().code,
        "REVISION_CONFLICT"
    );
    query.expected_revision = 0;
    query.target.instance_id = "foreign".into();
    assert_eq!(
        service.validate(&editor, &query).unwrap_err().code,
        "TARGET_MISMATCH"
    );
}

#[test]
fn invalid_project_input_is_not_misreported_as_checked_or_valid() {
    use atelier_bim::automation::Validate;
    let mut editor = Editor::new(Project::demo());
    editor.project.schema_version = 999;
    let service = ReadService::new("header".into());
    let report = service
        .validate(
            &editor,
            &Validate {
                target: service.target(&editor),
                expected_revision: 0,
                limit: 10,
                cursor: None,
            },
        )
        .unwrap();
    assert_eq!(report["input_checked"], false);
    assert_eq!(report["valid"], false);
    assert_eq!(report["inspected_elements"], 0);
    assert_eq!(report["diagnostics"][0]["code"], "INVALID_PROJECT_INPUT");
}

#[test]
fn scoped_queries_share_membership_and_bind_pagination_without_changing_legacy_all() {
    use atelier_bim::{
        model::{Material, Point, Shape},
        view_settings::DesignOptionScope,
        zones::ZoneSpec,
    };
    let mut p = atelier_bim::model::Project::default();
    let a = p.design_options.create("A".into());
    let b = p.design_options.create("B".into());
    for option in [None, Some(a), Some(b)] {
        let id = p.add(
            0,
            Material::Plaster,
            Shape::Zone {
                spec: ZoneSpec {
                    surface: Default::default(),
                    purpose: "fixture".into(),
                    outline: vec![
                        Point::new(0., 0.),
                        Point::new(2., 0.),
                        Point::new(2., 2.),
                        Point::new(0., 2.),
                    ],
                    holes: vec![],
                    height: 3.,
                    bottom_offset: 0.,
                    label_position: None,
                },
            },
        );
        p.elements
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .design_option = option;
    }
    let editor = Editor::new(p);
    let service = ReadService::new("scope-read".into());
    let mut q = Query {
        design_scope: DesignOptionScope::Option { id: a },
        target: service.target(&editor),
        expected_revision: editor.revision,
        ids: vec![],
        kind: Some("Zone".into()),
        story: Some(0),
        region: None,
        fields: vec!["id".into(), "design_option".into(), "quantities".into()],
        limit: 1,
        cursor: None,
    };
    let first = service.query(&editor, &q).unwrap();
    assert_eq!(first["summary"]["count"], 2);
    assert_eq!(
        first["elements"][0]["design_option"],
        serde_json::Value::Null
    );
    q.cursor = Some(first["next_cursor"].as_str().unwrap().into());
    let second = service.query(&editor, &q).unwrap();
    assert_eq!(second["elements"][0]["design_option"], a);
    assert_eq!(second["elements"][0]["quantities"]["space_volume_m3"], 12.);
    q.design_scope = DesignOptionScope::Option { id: b };
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "INVALID_CURSOR"
    );
    q.cursor = None;
    q.limit = 100;
    q.design_scope = DesignOptionScope::Main {};
    assert_eq!(service.query(&editor, &q).unwrap()["summary"]["count"], 1);
    q.design_scope = DesignOptionScope::All {};
    let all = service.query(&editor, &q).unwrap();
    assert_eq!(all["summary"]["count"], 3);
    assert!(all["scope_warning"].is_string());
    q.design_scope = DesignOptionScope::Option { id: 999 };
    assert_eq!(
        service.query(&editor, &q).unwrap_err().code,
        "INVALID_DESIGN_OPTION_SCOPE"
    );
}
