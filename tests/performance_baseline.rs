//! Opt-in performance baseline for reversible-delta undo.
//!
//! Run with:
//! `./scripts/cargo.sh test --release --test performance_baseline -- --ignored --nocapture`
//!
//! These numbers are a local comparison baseline, not a supported performance
//! promise. The 10k and 50k cases move a fixed 1,000-element selection so the
//! measurement reflects clone/validate/history cost rather than an artificial
//! O(n²) selection scan.

use atelier_bim::{
    editor::Editor,
    geometry,
    model::{Element, Material, Point, Project, Shape},
};
use std::hint::black_box;
use std::time::{Duration, Instant};

fn walls(count: usize) -> Project {
    let mut project = Project::default();
    for index in 0..count {
        let x = (index % 1_000) as f32 * 2.0;
        let y = (index / 1_000) as f32 * 2.0;
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(x, y),
                end: Point::new(x + 1.5, y),
                thickness: 0.2,
                height: 3.0,
            },
        );
    }
    project
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

#[test]
#[ignore = "manual performance baseline"]
fn snapshot_undo_baseline_for_1k_10k_50k_walls() {
    let sizes = std::env::var("OPENARCHI_PERF_SIZES")
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter_map(|item| item.trim().parse::<usize>().ok())
                .collect::<Vec<_>>()
        })
        .filter(|sizes| !sizes.is_empty())
        .unwrap_or_else(|| vec![1_000, 10_000, 50_000]);
    for count in sizes {
        let start = Instant::now();
        let project = walls(count);
        let create = elapsed_ms(start.elapsed());
        assert_eq!(project.elements.len(), count);

        let mut editor = Editor::new(project);
        let selection = editor
            .project
            .elements
            .iter()
            .take(count.min(1_000))
            .map(|element| element.id)
            .collect::<Vec<_>>();
        editor.set_selection(selection);
        let selected = editor.selected_ids.len();

        let start = Instant::now();
        editor.move_selection(Point::new(0.1, 0.0));
        let move_ms = elapsed_ms(start.elapsed());
        assert_eq!(editor.undo.len(), 1);
        let history_snapshots: usize = editor
            .undo
            .iter()
            .map(|record| record.retained_element_snapshots())
            .sum();

        let start = Instant::now();
        editor.undo();
        let undo_ms = elapsed_ms(start.elapsed());
        assert!(editor.undo.is_empty());

        let start = Instant::now();
        editor.redo();
        let redo_ms = elapsed_ms(start.elapsed());
        assert_eq!(editor.undo.len(), 1);

        let start = Instant::now();
        let walls: Vec<&Element> = editor
            .project
            .elements
            .iter()
            .filter(|element| matches!(element.shape, Shape::Wall { .. }))
            .collect();
        let footprints = geometry::plan_wall_footprints(&walls);
        let footprint_ms = elapsed_ms(start.elapsed());

        let start = Instant::now();
        let paths = geometry::wall_outline_paths(&footprints);
        let outline_ms = elapsed_ms(start.elapsed());

        let start = Instant::now();
        let mut triangle_count = 0usize;
        for wall in &walls {
            black_box(geometry::corners(&editor.project, wall));
        }
        let corners_ms = elapsed_ms(start.elapsed());

        let start = Instant::now();
        for footprint in &footprints {
            triangle_count += geometry::triangulate_polygon(footprint).len();
        }
        let triangulate_ms = elapsed_ms(start.elapsed());
        let mesh_ms = footprint_ms + outline_ms + corners_ms + triangulate_ms;
        assert!(triangle_count > 0);
        black_box(paths);

        let start = Instant::now();
        let hit = geometry::hit_plan(&editor.project, 0, Point::new(0.75, 0.0), 0.01);
        let pick_ms = elapsed_ms(start.elapsed());
        assert!(hit.is_some());

        println!(
            "baseline walls={count} selection={selected} create_ms={create:.2} move_ms={move_ms:.2} undo_ms={undo_ms:.2} redo_ms={redo_ms:.2} history_element_snapshots={history_snapshots} mesh_ms={mesh_ms:.2} footprint_ms={footprint_ms:.2} outline_ms={outline_ms:.2} corners_ms={corners_ms:.2} triangulate_ms={triangulate_ms:.2} pick_ms={pick_ms:.2}"
        );
    }
}
