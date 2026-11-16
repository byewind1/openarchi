use atelier_bim::{
    editor::Editor,
    model::{Point, Project, SlabContour},
    view_settings::RenovationStatus,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

#[test]
fn slab_and_arc_path_candidates_share_commit_and_preserve_real_redo() {
    for slab in [false, true] {
        let mut e = Editor::new(Project::default());
        e.place_column(p(9., 9.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        e.wall_renovation = RenovationStatus::Existing;
        let contour = SlabContour {
            points: vec![p(0., 0.), p(8., 0.), p(8., 6.), p(0., 6.)],
            holes: vec![SlabContour {
                points: vec![p(2., 2.), p(2., 4.), p(4., 4.), p(4., 2.)],
                ..Default::default()
            }],
            elevation: 1.25,
            ..Default::default()
        };
        let points = vec![p(-1., 0.), p(1., 0.)];
        let arcs = vec![atelier_bim::geometry::arc_segment_through(
            points[0],
            points[1],
            p(0., 1.),
        )];
        let before = e.project.clone();
        let revision = e.revision;
        let selection = e.selection_ids();
        let candidate = if slab {
            e.slab_creation_candidate(contour.clone()).unwrap()
        } else {
            e.polyline_creation_candidate(points.clone(), arcs.clone())
        };
        assert_eq!(
            candidate.renovation,
            if slab {
                RenovationStatus::Existing
            } else {
                RenovationStatus::New
            }
        );
        if slab {
            assert_eq!(candidate.slab_contour.as_ref().unwrap().holes.len(), 1);
            assert_eq!(candidate.slab_contour.as_ref().unwrap().elevation, 1.25);
        }
        let preview = e
            .preview_add_elements(vec![candidate.clone()], false)
            .unwrap();
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selection);
        e.redo();
        assert_eq!(e.project, redo);
        e.undo();
        assert_eq!(e.project, before);
        let id = if slab {
            e.place_slab_contour(contour).unwrap()
        } else {
            e.place_polyline(points, arcs).unwrap()
        };
        assert_eq!(e.project, preview);
        assert_eq!(
            e.project.elements.iter().find(|x| x.id == id),
            Some(&candidate)
        );
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&preview).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, preview);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, preview);
    }
}

#[test]
fn invalid_path_candidate_is_not_accepted_and_keeps_redo() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(2., 2.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    for points in [
        vec![p(0., 0.)],
        vec![p(0., 0.), p(f64::NAN, 1.)],
        vec![p(0., 0.), p(0., 0.)],
    ] {
        let candidate = e.polyline_creation_candidate(points.clone(), vec![]);
        let preview_error = e.preview_add_elements(vec![candidate], false).unwrap_err();
        let commit_error = e.place_polyline(points, vec![]).unwrap_err();
        assert_eq!(preview_error, commit_error);
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
    }
    assert!(
        e.slab_creation_candidate(SlabContour {
            points: vec![p(0., 0.), p(1., 0.)],
            ..Default::default()
        })
        .is_err()
    );
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, redo);
}

#[test]
fn independently_built_candidates_compose_one_atomic_grouped_model_command() {
    use atelier_bim::{
        command::ModelCommand, roof::RoofSpec, roof_contour::RoofContour, stairs::StairSpec,
        storage,
    };
    for reverse in [false, true] {
        for group in [false, true] {
            let mut e = Editor::new(Project::default());
            e.place_column(p(12., 12.)).unwrap();
            let redo = e.project.clone();
            e.undo();
            let before = e.project.clone();
            let revision = e.revision;
            let outer = vec![p(0., 0.), p(8., 0.), p(8., 6.), p(0., 6.)];
            let hole = vec![p(2., 2.), p(2., 4.), p(4., 4.), p(4., 2.)];
            let arc = atelier_bim::geometry::arc_segment_through(p(0., 0.), p(6., 0.), p(3., 3.))
                .unwrap();
            let mut candidates = vec![
                e.column_creation_preview(p(1., 1.)).unwrap(),
                e.beam_path_preview(p(0., 0.), p(6., 0.), 0., Some(arc))
                    .unwrap(),
                e.slab_creation_candidate(SlabContour {
                    points: outer.clone(),
                    holes: vec![SlabContour {
                        points: hole.clone(),
                        ..Default::default()
                    }],
                    elevation: 0.75,
                    ..Default::default()
                })
                .unwrap(),
                e.roof_creation_candidate(RoofSpec {
                    contour: Some(RoofContour {
                        outer,
                        holes: vec![hole],
                    }),
                    min: p(0., 0.),
                    max: p(8., 6.),
                    pivot_start: p(-1., 0.),
                    pivot_end: p(9., 0.),
                    rise_left: true,
                    pitch: 20.,
                    thickness: 0.2,
                    pivot_offset: 3.,
                })
                .unwrap(),
                e.stair_creation_candidate(StairSpec {
                    start: p(0., 7.),
                    end: p(4.8, 7.),
                    ..Default::default()
                })
                .unwrap(),
                e.polyline_creation_candidate(vec![p(0., 0.), p(6., 0.)], vec![Some(arc)]),
            ];
            assert!(candidates.iter().all(|x| x.id == before.next_id));
            let error = e
                .preview_add_elements(candidates.clone(), group)
                .unwrap_err();
            assert_eq!(
                e.apply(ModelCommand::AddElements {
                    elements: candidates.clone(),
                    group
                })
                .unwrap_err(),
                error
            );
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            e.redo();
            assert_eq!(e.project, redo);
            e.undo();
            assert_eq!(e.project, before);
            // Batch identities are explicit; candidate builders do not reserve IDs.
            for (index, element) in candidates.iter_mut().enumerate() {
                element.id = before.next_id.checked_add(index as u64).unwrap();
                element.name = format!("{} {:03}", element.kind(), element.id);
            }
            if reverse {
                candidates.reverse();
            }
            let ids: Vec<_> = candidates.iter().map(|x| x.id).collect();
            let preview = e.preview_add_elements(candidates.clone(), group).unwrap();
            assert_eq!(e.project, before);
            assert!(e.undo.is_empty());
            e.apply(ModelCommand::AddElements {
                elements: candidates.clone(),
                group,
            })
            .unwrap();
            assert_eq!(e.project, preview);
            assert_eq!(e.project.elements, candidates);
            assert_eq!(e.undo.len(), 1);
            assert_eq!(
                e.project.edit_state.groups,
                if group { vec![ids] } else { vec![] }
            );
            let path = std::env::temp_dir().join(format!(
                "oa-candidate-batch-{}-{reverse}-{group}.json",
                std::process::id()
            ));
            storage::save(&e.project, &path).unwrap();
            assert_eq!(storage::load(&path).unwrap(), preview);
            std::fs::remove_file(path).unwrap();
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, preview);
        }
    }
}
