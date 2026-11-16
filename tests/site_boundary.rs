use atelier_bim::{
    automation_registry::{DiscoveredBatch, NativeBatch},
    command::ModelCommand,
    editor::Editor,
    model::Point,
    project_repository::boundary::BoundaryPlacement,
    project_repository::{LengthUnit, Repository},
    site::boundary::{self, BoundaryProposal},
    storage,
};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("oa-boundary-{}", storage::new_document_identity()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn drawing(offset: f64) -> Vec<u8> {
    format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n0\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nTEXT\n5\nB\n8\nNotes\n1\nIgnored only by explicit selection\n0\nLWPOLYLINE\n5\nA1\n8\nSITE\n90\n4\n70\n1\n10\n{offset}\n20\n20000\n10\n{}\n20\n20000\n10\n{}\n20\n26000\n10\n{offset}\n20\n26000\n0\nENDSEC\n0\nEOF\n",offset+8000.,offset+8000.).into_bytes()
}
fn setup() -> (Fixture, Editor, Repository, String) {
    let f = Fixture::new();
    let editor = Editor::new(storage::startup_example());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &editor).unwrap();
    let src = f.0.join("plot.dxf");
    fs::write(&src, drawing(20000.)).unwrap();
    let id = repo.add_source(&src).unwrap();
    (f, editor, repo, id)
}
fn preview(repo: &Repository, editor: &Editor, id: &str) -> BoundaryProposal {
    repo.preview_boundary(
        &editor.project,
        id,
        "a1",
        BoundaryPlacement::ModelAxes,
        "Synthetic site".into(),
        0,
        1,
    )
    .unwrap()
}
#[test]
fn single_closed_record_has_exact_identity_and_strict_geometry_limits() {
    use atelier_bim::dxf::primitives::closed_outline;
    let bytes = drawing(20000.);
    let a = closed_outline(&bytes, "A1").unwrap();
    assert_eq!(a.points.len(), 4);
    assert_eq!(a.handle, "A1");
    assert_eq!(a.layer.as_deref(), Some("SITE"));
    assert_eq!(a.points[1], [28000., 20000.]);
    assert_eq!(a.record_sha256.len(), 64);
    assert!(closed_outline(&bytes, "B").is_err());
    assert!(closed_outline(&bytes, "0").is_err());
    for modified in [
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("70\n1", "70\n0"),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("90\n4", "90\n4.5"),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("70\n1", "70\n1\n42\n1"),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("70\n1", "70\n1\n210\n1"),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("70\n1", "70\n1\n67\n1"),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("70\n1", "70\n1\n90\n4"),
    ] {
        assert!(closed_outline(modified.as_bytes(), "A1").is_err());
    }
}
#[test]
fn units_and_quality_are_explicit_and_source_guard_lives_until_commit() {
    let (_f, editor, mut repo, id) = setup();
    assert!(
        repo.preview_boundary(
            &editor.project,
            &id,
            "A1",
            BoundaryPlacement::ModelAxes,
            "Plot".into(),
            0,
            1
        )
        .is_err()
    );
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    let p = preview(&repo, &editor, &id);
    assert_eq!(p.outline_m[0], [20., 20.]);
    let guard = repo.guard_boundary_commit(&p).unwrap();
    assert!(repo.confirm_unit(&id, Some(LengthUnit::Metre)).is_err());
    drop(guard);
    let mut forged = p.clone();
    forged.outline_m[0][0] += 0.2;
    assert!(repo.guard_boundary_commit(&forged).is_err());
    repo.reject_source(&id, "fixture rejected").unwrap();
    assert!(repo.guard_boundary_commit(&p).is_err());
    assert!(
        repo.preview_boundary(
            &editor.project,
            &id,
            "A1",
            BoundaryPlacement::ModelAxes,
            "Plot".into(),
            0,
            1
        )
        .is_err()
    );
}
#[test]
fn boundary_is_one_undoable_editable_polyline_and_survives_native_save() {
    let (f, mut editor, mut repo, id) = setup();
    let original = editor.project.clone();
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    let p = preview(&repo, &editor, &id);
    let prepared = editor
        .prepare_change(0, vec![ModelCommand::ApplySiteBoundary { proposal: p }])
        .unwrap();
    let receipt = editor.commit_change("boundary", &prepared).unwrap();
    assert!(receipt.site_context_changed);
    assert_eq!(receipt.changed_ids.len(), 1);
    assert_eq!(editor.project.elements.len(), original.elements.len() + 1);
    assert!((boundary::area(&editor.project).unwrap().unwrap() - 48.).abs() < 1e-9);
    let id = editor.project.site.boundary.as_ref().unwrap().element_id;
    assert!(
        editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .worksheet
            .is_none()
    );
    let after = editor.project.clone();
    editor.undo();
    assert_eq!(editor.project, original);
    editor.redo();
    assert_eq!(editor.project, after);
    let path = f.0.join("with-boundary.json");
    storage::save_document(&editor.project, &path, editor.project_identity()).unwrap();
    assert_eq!(Editor::from_stored_document(&path).unwrap().project, after);
    let mut altered = editor.project.site.clone();
    altered.boundary = None;
    assert!(
        editor
            .apply(ModelCommand::SetSiteContext { context: altered })
            .is_err()
    );
    assert!(
        editor
            .apply(ModelCommand::RemoveElements { ids: vec![id] })
            .is_err()
    );
    editor.apply(ModelCommand::DetachSiteBoundary).unwrap();
    assert!(editor.project.site.boundary.is_none());
    assert!(editor.project.elements.iter().any(|e| e.id == id));
    editor.undo();
    assert_eq!(editor.project, after);
}
#[test]
fn fresh_reimport_keeps_id_and_refuses_manual_move_and_locks() {
    let (f, mut editor, mut repo, source) = setup();
    repo.confirm_unit(&source, Some(LengthUnit::Millimetre))
        .unwrap();
    let p = preview(&repo, &editor, &source);
    editor
        .apply(ModelCommand::ApplySiteBoundary {
            proposal: p.clone(),
        })
        .unwrap();
    let id = editor.project.site.boundary.as_ref().unwrap().element_id;
    let changed = f.0.join("revised.dxf");
    fs::write(&changed, drawing(21000.)).unwrap();
    let next = repo.add_source(&changed).unwrap();
    repo.confirm_unit(&next, Some(LengthUnit::Millimetre))
        .unwrap();
    let proposal = preview(&repo, &editor, &next);
    editor
        .apply(ModelCommand::ApplySiteBoundary {
            proposal: proposal.clone(),
        })
        .unwrap();
    assert_eq!(
        editor.project.site.boundary.as_ref().unwrap().element_id,
        id
    );
    assert_eq!(
        editor
            .project
            .site
            .boundary
            .as_ref()
            .unwrap()
            .imported_outline_m[0],
        [21., 20.]
    );
    editor.undo();
    let before = editor.project.clone();
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(1., 0.),
        })
        .unwrap();
    let moved = editor.project.clone();
    assert!(boundary::user_modified(&moved).unwrap());
    assert!(
        editor
            .apply(ModelCommand::ApplySiteBoundary {
                proposal: proposal.clone()
            })
            .is_err()
    );
    assert_eq!(editor.project, moved);
    editor.undo();
    assert_eq!(editor.project, before);
    editor.project.edit_state.locked.insert(id);
    let locked = editor.project.clone();
    assert!(
        editor
            .apply(ModelCommand::ApplySiteBoundary { proposal })
            .is_err()
    );
    assert_eq!(editor.project, locked);
}
#[test]
fn both_ai_adapters_use_the_same_boundary_command() {
    let (_f, mut editor, mut repo, id) = setup();
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    let p = preview(&repo, &editor, &id);
    let wire =
        serde_json::json!({"version":1,"edits":[{"operation":"apply_site_boundary","proposal":p}]});
    let native: NativeBatch = serde_json::from_value(wire.clone()).unwrap();
    let discovered: DiscoveredBatch = serde_json::from_value(wire).unwrap();
    let a = native.lower(&editor).unwrap();
    assert_eq!(a, discovered.resolve().unwrap().lower(&editor).unwrap());
    let prepared = editor.prepare_batch_change(0, &a).unwrap();
    editor.commit_change("ai-boundary", &prepared).unwrap();
    assert!(editor.project.site.boundary.is_some());
    assert!(atelier_bim::automation_registry::describe("apply_site_boundary").is_some());
}
#[test]
fn projected_boundary_requires_numeric_control_evidence() {
    let (_f, editor, mut repo, id) = setup();
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    assert!(
        repo.preview_boundary(
            &editor.project,
            &id,
            "A1",
            BoundaryPlacement::ProjectedReference,
            "Plot".into(),
            0,
            1
        )
        .is_err()
    );
}

#[test]
fn constant_width_requires_explicit_choice_and_loss_is_verified_and_persistent() {
    use atelier_bim::dxf::primitives::{closed_outline, closed_outline_with_width};
    use atelier_bim::project_repository::boundary::BoundaryWidthPolicy;
    let (f, mut editor, mut repo, _) = setup();
    let text = String::from_utf8(drawing(20000.)).unwrap();
    let bytes = text.replace("70\n1", "70\n1\n43\n150").into_bytes();
    assert!(closed_outline(&bytes, "A1").is_err());
    let extracted = closed_outline_with_width(&bytes, "A1", true).unwrap();
    assert_eq!(extracted.constant_width_native, 150.);
    for invalid in ["43\n-1", "43\nNaN", "43\n150\n43\n150", "43\n150\n40\n2"] {
        let bad = text.replace("70\n1", &format!("70\n1\n{invalid}"));
        assert!(closed_outline_with_width(bad.as_bytes(), "A1", true).is_err());
    }
    let path = f.0.join("width.dxf");
    fs::write(&path, bytes).unwrap();
    let id = repo.add_source(&path).unwrap();
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    assert!(
        repo.preview_boundary(
            &editor.project,
            &id,
            "A1",
            BoundaryPlacement::ModelAxes,
            "Plot".into(),
            0,
            1
        )
        .is_err()
    );
    let p = repo
        .preview_boundary_with_width(
            &editor.project,
            &id,
            "A1",
            BoundaryPlacement::ModelAxes,
            "Plot".into(),
            0,
            1,
            BoundaryWidthPolicy::ConstantWidthCenterline,
        )
        .unwrap();
    assert_eq!(p.origin.centerline_width_native, Some(150.));
    let guard = repo.guard_boundary_commit(&p).unwrap();
    editor
        .apply(ModelCommand::ApplySiteBoundary {
            proposal: p.clone(),
        })
        .unwrap();
    drop(guard);
    assert_eq!(boundary::area(&editor.project).unwrap(), Some(48.));
    let output = f.0.join("width.json");
    storage::save_document(&editor.project, &output, editor.project_identity()).unwrap();
    assert_eq!(
        Editor::from_stored_document(&output).unwrap().project,
        editor.project
    );
    for width in [None, Some(151.)] {
        let mut forged = p.clone();
        forged.origin.centerline_width_native = width;
        assert!(repo.guard_boundary_commit(&forged).is_err());
    }
    let wire =
        serde_json::json!({"version":1,"edits":[{"operation":"apply_site_boundary","proposal":p}]});
    let native: NativeBatch = serde_json::from_value(wire.clone()).unwrap();
    let discovered: DiscoveredBatch = serde_json::from_value(wire).unwrap();
    assert_eq!(
        native.lower(&editor).unwrap(),
        discovered.resolve().unwrap().lower(&editor).unwrap()
    );
}

#[test]
fn projected_mapping_and_stale_source_metadata_are_checked() {
    use atelier_bim::site::{CoordinateControl, HorizontalReference, SiteContext, SiteEvidence};
    let (f, mut editor, mut repo, id) = setup();
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    let evidence = SiteEvidence {
        source_sha256: "a".repeat(64),
        locator: "synthetic reference".into(),
        note: String::new(),
    };
    let site = SiteContext {
        horizontal: Some(HorizontalReference {
            crs: "SYNTHETIC EN metres".into(),
            origin_east_north_m: [10., 10.],
            model_to_grid_ccw_deg: 0.,
            evidence: evidence.clone(),
        }),
        controls: vec![
            CoordinateControl {
                model_xy_m: [0., 0.],
                reference_east_north_m: [10., 10.],
                evidence: evidence.clone(),
            },
            CoordinateControl {
                model_xy_m: [20., 0.],
                reference_east_north_m: [30., 10.],
                evidence,
            },
        ],
        control_tolerance_m: Some(0.001),
        ..Default::default()
    };
    editor
        .apply(ModelCommand::SetSiteContext { context: site })
        .unwrap();
    let p = repo
        .preview_boundary(
            &editor.project,
            &id,
            "A1",
            BoundaryPlacement::ProjectedReference,
            "Projected plot".into(),
            0,
            1,
        )
        .unwrap();
    assert_eq!(p.outline_m[0], [10., 10.]);
    let baseline = editor.project.clone();
    for controls in [Vec::new(), {
        let mut controls = baseline.site.controls.clone();
        controls[1].reference_east_north_m[0] += 1.;
        controls
    }] {
        editor.project = baseline.clone();
        editor.project.site.controls = controls;
        let unchanged = editor.project.clone();
        assert!(
            editor
                .apply(ModelCommand::ApplySiteBoundary {
                    proposal: p.clone()
                })
                .is_err()
        );
        assert_eq!(editor.project, unchanged);
    }
    editor.project = baseline;
    editor
        .apply(ModelCommand::ApplySiteBoundary {
            proposal: p.clone(),
        })
        .unwrap();
    assert_eq!(boundary::area(&editor.project).unwrap(), Some(48.));
    editor.undo();
    let _guard = repo.guard_boundary_commit(&p).unwrap();
    drop(_guard);
    let mut stale = Repository::open(repo.root()).unwrap();
    repo.confirm_unit(&id, Some(LengthUnit::Metre)).unwrap();
    assert!(stale.guard_boundary_commit(&p).is_err());
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    assert!(
        stale
            .preview_boundary(
                &editor.project,
                &id,
                "A1",
                BoundaryPlacement::ModelAxes,
                "Plot".into(),
                0,
                1
            )
            .is_err()
    );
    stale = Repository::open(repo.root()).unwrap();
    let item = stale.manifest.sources.iter().find(|s| s.id == id).unwrap();
    fs::write(f.0.join("repo").join(&item.blob.path), b"tampered").unwrap();
    assert!(stale.guard_boundary_commit(&p).is_err());
}
