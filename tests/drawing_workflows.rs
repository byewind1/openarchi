use atelier_bim::model::ModelAdapter;
use atelier_bim::{
    command::ModelCommand,
    drawings::{self, Calibration, DrawingDocument},
    editor::Editor,
    model::Project,
    storage::{self, JsonAdapter},
};
use std::{io::Cursor, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "oa-drawing-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn document(&self) -> DrawingDocument {
        let image = image::RgbaImage::from_pixel(400, 200, image::Rgba([240, 230, 210, 255]));
        image.save(self.0.join("source.png")).unwrap();
        let page = drawings::import(&self.0.join("source.png"), &self.0.join("cache"), 1).unwrap();
        let mut doc = DrawingDocument {
            pages: vec![page],
            regions: vec![],
            ..Default::default()
        };
        doc.add_region(1, 0, [0., 0., 400., 200.]).unwrap();
        doc
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn drawing_regions_calibrate_independently_with_check_and_shared_history() {
    let fixture = Fixture::new();
    let mut doc = fixture.document();
    doc.add_region(1, 0, [100., 0., 400., 200.]).unwrap();
    let calibration = Calibration {
        points: [[100., 20.], [200., 20.]],
        distance_mm: 5000.,
    };
    doc.regions[0].calibrate(calibration.clone()).unwrap();
    doc.regions[1]
        .calibrate(Calibration {
            distance_mm: 10000.,
            ..calibration
        })
        .unwrap();
    assert_eq!(doc.regions[0].world([200., 100.]), Some([10., -5.]));
    assert_eq!(doc.regions[1].world([200., 100.]), Some([10., -10.]));
    assert_eq!(
        doc.regions[0]
            .check_distance([[20., 20.], [120., 20.]], 4900.)
            .unwrap(),
        (5000., 100.)
    );
    let project = Project::default();
    let mut editor = Editor::new(project.clone());
    editor
        .apply(ModelCommand::SetDrawings {
            document: doc.clone(),
        })
        .unwrap();
    assert_eq!(editor.project.elements, project.elements);
    assert!(editor.dirty());
    editor.undo();
    assert_eq!(editor.project, project);
    editor.redo();
    assert_eq!(editor.project.drawings, doc);
    let snapshot = editor.project.clone();
    let history = editor.undo.len();
    doc.regions[0].bounds[2] = 500.;
    assert!(
        editor
            .apply(ModelCommand::SetDrawings { document: doc })
            .is_err()
    );
    assert_eq!(editor.project, snapshot);
    assert_eq!(editor.undo.len(), history);
}

#[test]
fn recalibration_marks_review_without_editing_bim_or_other_regions() {
    let fixture = Fixture::new();
    let mut doc = fixture.document();
    let calibration = Calibration {
        points: [[10., 10.], [110., 10.]],
        distance_mm: 2000.,
    };
    doc.regions[0].calibrate(calibration.clone()).unwrap();
    let original = doc.regions[0].clone();
    doc.regions[0].calibrate(calibration.clone()).unwrap();
    assert_eq!(doc.regions[0], original);
    doc.regions[0]
        .calibrate(Calibration {
            distance_mm: 4000.,
            ..calibration
        })
        .unwrap();
    assert!(doc.regions[0].needs_review);
    assert_eq!(doc.regions[0].revision, 2);
    let mut editor = Editor::new(Project::demo());
    let original_elements = editor.project.elements.clone();
    editor
        .apply(ModelCommand::SetDrawings { document: doc })
        .unwrap();
    assert_eq!(editor.project.elements, original_elements);
}

#[test]
fn calibration_rejects_degenerate_nonfinite_and_overflow_without_mutation() {
    let fixture = Fixture::new();
    let doc = fixture.document();
    for calibration in [
        Calibration {
            points: [[0., 0.], [0., 0.]],
            distance_mm: 100.,
        },
        Calibration {
            points: [[0., 0.], [20., 0.]],
            distance_mm: f64::NAN,
        },
        Calibration {
            points: [[0., 0.], [500., 0.]],
            distance_mm: 100.,
        },
        Calibration {
            points: [[0., 0.], [20., 0.]],
            distance_mm: 0.,
        },
    ] {
        let mut region = doc.regions[0].clone();
        let before = region.clone();
        assert!(region.calibrate(calibration).is_err());
        assert_eq!(region, before);
    }
    let mut region = doc.regions[0].clone();
    region.revision = u64::MAX;
    let before = region.clone();
    assert!(
        region
            .calibrate(Calibration {
                points: [[0., 0.], [20., 0.]],
                distance_mm: 100.
            })
            .is_err()
    );
    assert_eq!(region, before);
}

#[test]
fn portable_save_move_reopen_and_missing_asset_remains_recoverable() {
    let fixture = Fixture::new();
    let doc = fixture.document();
    let project = Project {
        drawings: doc,
        ..Project::default()
    };
    let path = fixture.0.join("drawing.json");
    storage::save(&project, &path).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        PathBuf::from(json["drawings"]["pages"][0]["path"].as_str().unwrap()).is_relative(),
        true
    );
    std::fs::remove_dir_all(fixture.0.join("cache")).unwrap();
    std::fs::remove_file(fixture.0.join("source.png")).unwrap();
    let moved = fixture.0.join("moved");
    std::fs::create_dir(&moved).unwrap();
    std::fs::rename(&path, moved.join("drawing.json")).unwrap();
    std::fs::rename(
        fixture.0.join("drawing.json.assets"),
        moved.join("drawing.json.assets"),
    )
    .unwrap();
    let loaded = storage::load(&moved.join("drawing.json")).unwrap();
    assert!(drawings::verified_bytes(&loaded.drawings.pages[0]).is_ok());
    storage::save(&loaded, &moved.join("copy.json")).unwrap();
    std::fs::remove_file(&loaded.drawings.pages[0].path).unwrap();
    assert!(storage::load(&moved.join("drawing.json")).is_ok());
    let before = std::fs::read(moved.join("drawing.json")).unwrap();
    assert!(storage::save(&loaded, &moved.join("drawing.json")).is_err());
    assert_eq!(std::fs::read(moved.join("drawing.json")).unwrap(), before);
}

#[test]
fn import_copies_source_and_detects_tampered_content_addressed_object() {
    let fixture = Fixture::new();
    let doc = fixture.document();
    let page = &doc.pages[0];
    let original = std::fs::read(fixture.0.join("source.png")).unwrap();
    std::fs::write(&page.path, b"tampered").unwrap();
    assert!(drawings::verified_bytes(page).is_err());
    assert!(drawings::store(&original, &fixture.0.join("cache")).is_err());
    assert_eq!(
        std::fs::read(fixture.0.join("source.png")).unwrap(),
        original
    );
}

#[test]
fn old_project_migrates_and_invalid_source_regions_are_rejected() {
    let mut old = serde_json::to_value(Project::default()).unwrap();
    old.as_object_mut().unwrap().remove("drawings");
    assert_eq!(
        JsonAdapter
            .import(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .drawings,
        DrawingDocument::default()
    );
    let fixture = Fixture::new();
    let doc = fixture.document();
    let mut project = Project {
        drawings: doc,
        ..Project::default()
    };
    project.drawings.regions[0].story = 999;
    assert!(project.validate().is_err());
    project.drawings.regions[0].story = 0;
    project.drawings.pages[0].pixels = [u32::MAX, u32::MAX];
    assert!(project.validate().is_err());
}

#[test]
fn png_jpeg_supported_but_other_formats_and_oversized_images_rejected() {
    for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(40, 20));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        assert_eq!(
            drawings::decode(bytes.get_ref()).unwrap().dimensions(),
            (40, 20)
        );
    }
    assert!(drawings::decode(b"not an image").is_err());
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(16385, 1));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    assert!(drawings::decode(bytes.get_ref()).is_err());
}

#[test]
fn worksheet_view_scope_is_not_an_edit_and_shared_polyline_hit_snap_history_are_isolated() {
    use atelier_bim::{
        editor::Tool,
        geometry,
        model::{Material, Point, Shape},
    };
    let fixture = Fixture::new();
    let doc = fixture.document();
    let mut p = Project {
        drawings: doc,
        ..Project::default()
    };
    let model = p.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.)],
            arcs: vec![],
        },
    );
    let mut e = Editor::new(p);
    let before = e.project.clone();
    e.switch_worksheet(Some(1));
    assert!(!e.dirty());
    assert!(e.undo.is_empty());
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(1., 0.), 0.1),
        None
    );
    assert_eq!(
        geometry::snap(&e.project, 0, Point::new(0.02, 0.), 0.1, true).1,
        geometry::SnapKind::Grid
    );
    e.click(Point::new(0., 0.), 0.1);
    e.click(Point::new(1., 0.), 0.1);
    e.finish_polyline(None).unwrap();
    let sheet = e.project.elements.last().unwrap().id;
    assert_eq!(e.project.elements.last().unwrap().worksheet, Some(1));
    assert_eq!(e.project.elements[0], before.elements[0]);
    assert_eq!(e.undo.len(), 1);
    e.undo();
    assert_eq!(e.project.plan_options.active_worksheet, Some(1));
    assert!(!e.dirty());
    e.redo();
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(0.5, 0.), 0.1),
        Some(sheet)
    );
    e.switch_worksheet(None);
    e.set_tool(Tool::Select);
    e.set_selection(vec![sheet, model]);
    assert_eq!(e.selection_ids(), vec![model]);
    e.switch_worksheet(Some(1));
    e.set_selection(vec![sheet, model]);
    assert_eq!(e.selection_ids(), vec![sheet]);
    e.set_tool(Tool::Wall);
    assert_eq!(e.tool, Tool::Polyline);
    e.switch_worksheet(None);
    e.undo();
    assert!(!e.dirty());
    assert_eq!(e.project, before);
}

#[test]
fn ai_proposals_bind_source_scope_and_revision_without_model_mutation() {
    use atelier_bim::drawings::WorksheetProposal;
    let fixture = Fixture::new();
    let doc = fixture.document();
    let p = Project {
        drawings: doc,
        ..Project::default()
    };
    let proposal = WorksheetProposal {
        placement: None,
        target_worksheet: None,
        target_story: 0,
        worksheet: 1,
        source_sha256: p.drawings.pages[0].sha256.clone(),
        region: 2,
        region_revision: 0,
        scale_revision: 0,
        project_revision: 7,
        polylines: vec![vec![[20., 20.], [100., 20.]]],
    };
    assert!(proposal.validate(&p, 7).is_ok());
    assert!(p.elements.is_empty());
    assert!(proposal.validate(&p, 8).is_err());
    let mut invalid = proposal.clone();
    invalid.polylines[0][0] = [500., 0.];
    assert!(invalid.validate(&p, 7).is_err());
    let mut invalid = proposal.clone();
    invalid.source_sha256 = "0".repeat(64);
    assert!(invalid.validate(&p, 7).is_err());
    let mut changed = p.clone();
    changed.drawings.regions[0].revision = 1;
    assert!(proposal.validate(&changed, 7).is_err());
}
