//! Independent-process readback exercises the actual portable storage adapter.
use atelier_bim::{
    cad_reference as cad, drawings,
    editor::Editor,
    model::{Point, Project},
    storage,
    trace::TraceReference,
    trace_document::{TraceSettings, TraceView, TraceViewport},
};

#[test]
#[ignore = "reader subprocess, requires an explicit generated input"]
fn trace_reader_process() {
    let path = std::env::var_os("OA_TRACE_READ_PATH").expect("explicit test input");
    let project = storage::load(std::path::Path::new(&path)).unwrap();
    if let Ok(expected) = std::env::var("OA_TRACE_EXPECTED") {
        let expected: atelier_bim::trace_document::TraceDocument =
            serde_json::from_str(&expected).unwrap();
        assert_eq!(project.trace, expected);
    }
    for source in &project.drawings.cad_sources {
        assert!(
            source.data.is_some(),
            "frozen CAD source must load: {:?}",
            source.load_error
        );
    }
    for page in &project.drawings.pages {
        drawings::verified_bytes(page).unwrap();
    }
    let editor = Editor::new(project.clone());
    assert!(!editor.dirty());
    if let Some(story) = project.trace.active_story {
        assert_eq!(editor.story, story);
    }
}

#[test]
fn trace_story_cad_image_save_and_fresh_process_reopen() {
    let root = std::env::temp_dir().join(format!(
        "oa-trace-process-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    for kind in 0..3 {
        let mut project = Project::default();
        let reference = match kind {
            0 => TraceReference::Story(1),
            1 => {
                let path = root.join("original.dxf");
                std::fs::write(&path, "0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nA\n8\nSite\n10\n0\n20\n0\n11\n100\n21\n50\n0\nENDSEC\n0\nEOF\n").unwrap();
                let source = cad::prepare(&path, None, &root.join("cache")).unwrap();
                let (doc, sheet, _) = cad::attach(
                    &project.drawings,
                    source,
                    None,
                    cad::Transform::default(),
                    true,
                )
                .unwrap();
                project.drawings = doc;
                TraceReference::Worksheet(sheet)
            }
            _ => {
                let path = root.join("original.png");
                image::RgbaImage::from_pixel(8, 12, image::Rgba([17, 93, 181, 255]))
                    .save(&path)
                    .unwrap();
                let page = drawings::import(&path, &root.join("images"), 1).unwrap();
                project.drawings.pages.push(page);
                TraceReference::Worksheet(1)
            }
        };
        project
            .trace
            .set(
                TraceView::FloorPlan,
                TraceSettings {
                    enabled: true,
                    reference,
                    offset: Point::new(21., -37.),
                    rotation_deg: 43.,
                    pivot: Some(Point::new(1., 2.)),
                    reference_opacity: 71,
                    active_opacity: 203,
                    reference_color: Some([19, 79, 173]),
                    active_color: Some([173, 57, 31]),
                    ..Default::default()
                },
                1,
                0,
            )
            .unwrap();
        project.trace.plan_visible = true;
        project.trace.viewport = Some(TraceViewport {
            center: Point::new(17., 29.),
            scale: 237.5,
        });
        project.trace.remember(reference);
        let path = root.join(format!("saved-{kind}.json"));
        storage::save(&project, &path).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "trace_reader_process", "--ignored"])
            .env("OA_TRACE_READ_PATH", &path)
            .env(
                "OA_TRACE_EXPECTED",
                serde_json::to_string(&project.trace).unwrap(),
            )
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "fresh process rejected saved reference: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
