use atelier_bim::{
    command::ModelCommand,
    editor::Editor,
    model::{Element, OpeningKind, Point, Project, Shape, SlabContour},
    roof::RoofSpec,
    stairs::StairSpec,
    storage,
};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn building() -> Vec<Element> {
    let mut e = Editor::new(Project::default());
    e.place_wall_segments(vec![(p(0., 0.), p(8., 0.))]).unwrap();
    e.place_opening(OpeningKind::Door, p(2., 0.), 0.01).unwrap();
    e.place_opening(OpeningKind::Window, p(6., 0.), 0.01)
        .unwrap();
    e.place_slab_contour(SlabContour {
        points: vec![p(0., 0.), p(8., 0.), p(8., 6.), p(0., 6.)],
        ..Default::default()
    })
    .unwrap();
    e.place_column(p(1., 1.)).unwrap();
    e.place_beam(p(1., 1.), p(7., 1.)).unwrap();
    e.place_roof(RoofSpec {
        contour: None,
        pivot_start: p(0., 0.),
        pivot_end: p(8., 0.),
        rise_left: true,
        min: p(0., 0.),
        max: p(8., 6.),
        pitch: 20.,
        thickness: 0.2,
        pivot_offset: 3.,
    })
    .unwrap();
    e.place_stair(StairSpec {
        start: p(1., 2.),
        end: p(5.8, 2.),
        ..Default::default()
    })
    .unwrap();
    e.project.elements
}

#[test]
fn mixed_building_batch_resolves_new_hosts_in_either_order_and_roundtrips_once() {
    let source = building();
    assert_eq!(source.len(), 8);
    for reversed in [false, true] {
        for group in [false, true] {
            let mut batch = source.clone();
            if reversed {
                batch.reverse();
            }
            let ids: Vec<_> = batch.iter().map(|e| e.id).collect();
            let mut e = Editor::new(Project::default());
            let before = e.project.clone();
            let revision = e.revision;
            let selection = e.selection_ids();
            let preview = e.preview_add_elements(batch.clone(), group).unwrap();
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            assert_eq!(e.selection_ids(), selection);
            assert!(e.undo.is_empty());
            let mut affected = e
                .apply(ModelCommand::AddElements {
                    elements: batch.clone(),
                    group,
                })
                .unwrap();
            affected.sort_unstable();
            let mut expected_ids = ids.clone();
            expected_ids.sort_unstable();
            assert_eq!(affected, expected_ids);
            assert_eq!(e.project.elements, batch);
            assert_eq!(e.project, preview);
            assert_eq!(e.project.next_id, 9);
            assert_eq!(e.undo.len(), 1);
            assert_eq!(
                e.project.edit_state.groups,
                if group { vec![ids] } else { vec![] }
            );
            e.project.validate().unwrap();
            let after = e.project.clone();
            let path = std::env::temp_dir().join(format!(
                "oa-mixed-{}-{reversed}-{group}.json",
                std::process::id()
            ));
            storage::save(&after, &path).unwrap();
            assert_eq!(storage::load(&path).unwrap(), after);
            std::fs::remove_file(path).unwrap();
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, after);
        }
    }
}

#[test]
fn locked_existing_host_rejects_entire_preview_and_batch_in_either_order() {
    for layer_lock in [false, true] {
        for opening_first in [false, true] {
            let mut e = Editor::new(Project::default());
            e.place_wall_segments(vec![(p(0., 0.), p(8., 0.))]).unwrap();
            let wall = e.project.elements[0].id;
            let layer = e.project.layers.new_layer("Host guard", None);
            if layer_lock {
                e.project.elements[0].layer = atelier_bim::layers::ElementLayer(layer);
                e.project
                    .layers
                    .layers
                    .iter_mut()
                    .find(|l| l.id == layer)
                    .unwrap()
                    .status
                    .locked = true;
            } else {
                e.project.edit_state.locked.insert(wall);
            }
            e.place_column(p(-2., -2.)).unwrap();
            let redo = e.project.clone();
            e.undo();
            e.set_selection(vec![wall]);
            e.mark_saved();
            let before = e.project.clone();
            let revision = e.revision;
            let selection = e.selection_ids();
            let history = e.undo.len();
            let source = building();
            let mut batch = vec![source[1].clone(), source[7].clone()];
            if !opening_first {
                batch.reverse();
            }
            let error = e.preview_add_elements(batch.clone(), false).unwrap_err();
            assert_eq!(error, "Selection contains locked elements");
            assert_eq!(
                e.apply(ModelCommand::AddElements {
                    elements: batch.clone(),
                    group: false
                })
                .unwrap_err(),
                error
            );
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            assert_eq!(e.selection_ids(), selection);
            assert_eq!(e.undo.len(), history);
            assert!(!e.dirty());
            e.redo();
            assert_eq!(e.project, redo);
            e.undo();
            assert_eq!(e.project, before);
            if layer_lock {
                e.project
                    .layers
                    .layers
                    .iter_mut()
                    .find(|l| l.id == layer)
                    .unwrap()
                    .status
                    .locked = false;
            } else {
                e.project.edit_state.locked.remove(&wall);
            }
            let candidate = e.preview_add_elements(batch.clone(), false).unwrap();
            assert_eq!(candidate.elements.len(), 3);
            e.apply(ModelCommand::AddElements {
                elements: batch,
                group: false,
            })
            .unwrap();
            assert_eq!(e.project, candidate);
        }
    }
}

#[test]
fn successful_empty_and_locked_batch_previews_preserve_actual_redo() {
    for locked in [false, true] {
        let mut e = Editor::new(Project::default());
        let layer = e.project.layers.new_layer("Batch guard", None);
        e.project
            .layers
            .layers
            .iter_mut()
            .find(|l| l.id == layer)
            .unwrap()
            .status
            .locked = true;
        e.place_column(p(-2., -2.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let revision = e.revision;
        let selected = e.selection_ids();
        assert_eq!(e.preview_add_elements(vec![], true).unwrap(), before);
        assert!(
            e.apply(ModelCommand::AddElements {
                elements: vec![],
                group: true
            })
            .unwrap()
            .is_empty()
        );
        let mut batch = building();
        if locked {
            batch.last_mut().unwrap().layer = atelier_bim::layers::ElementLayer(layer);
            let error = e.preview_add_elements(batch.clone(), true).unwrap_err();
            assert_eq!(
                e.apply(ModelCommand::AddElements {
                    elements: batch,
                    group: true
                })
                .unwrap_err(),
                error
            );
        } else {
            let candidate = e.preview_add_elements(batch, true).unwrap();
            candidate.validate().unwrap();
            assert_eq!(candidate.elements.len(), 8);
        }
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selected);
        assert!(e.undo.is_empty());
        e.redo();
        assert_eq!(e.project, redo);
    }
}

#[test]
fn invalid_mixed_batch_preserves_project_selection_revision_and_actual_redo() {
    for failure in 0..3 {
        let mut e = Editor::new(Project::default());
        e.place_column(p(-2., -2.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let revision = e.revision;
        let selection = e.selection_ids();
        let mut batch = building();
        match failure {
            0 => {
                if let Shape::Opening {
                    ref mut host_wall, ..
                } = batch[1].shape
                {
                    *host_wall = 999;
                }
            }
            1 => batch[7].id = batch[0].id,
            _ => {
                if let Shape::Stair { ref mut spec } = batch[7].shape {
                    spec.width = 0.;
                }
            }
        }
        let preview_error = e.preview_add_elements(batch.clone(), true).unwrap_err();
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selection);
        assert_eq!(
            e.apply(ModelCommand::AddElements {
                elements: batch,
                group: true
            })
            .unwrap_err(),
            preview_error
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selection);
        assert!(e.undo.is_empty());
        e.redo();
        assert_eq!(e.project, redo);
        e.undo();
        assert_eq!(e.project, before);
    }
}

#[test]
fn pending_wall_and_openings_compose_without_intermediate_model_writes() {
    for group in [false, true] {
        for reversed in [false, true] {
            let mut e = Editor::new(Project::default());
            e.story = 1;
            e.material = atelier_bim::model::Material::Timber;
            e.wall_renovation = atelier_bim::view_settings::RenovationStatus::Existing;
            e.place_column(p(9., 9.)).unwrap();
            let redo = e.project.clone();
            e.undo();
            let before = e.project.clone();
            let revision = e.revision;
            let status = e.status.clone();
            let selection = e.selection_ids();
            let mut batch = e
                .wall_creation_candidates(vec![(p(0., 0.), p(8., 0.))])
                .unwrap();
            let host = batch[0].id;
            for (kind, offset) in [(OpeningKind::Door, 2.), (OpeningKind::Window, 6.)] {
                let candidate = e
                    .opening_in_batch_candidate(&batch, kind, host, offset, kind.defaults())
                    .unwrap();
                assert_eq!(candidate.story, 1);
                assert_eq!(candidate.material, atelier_bim::model::Material::Timber);
                assert_eq!(candidate.renovation, e.wall_renovation);
                assert!(!batch.iter().any(|x| x.id == candidate.id));
                batch.push(candidate);
            }
            assert!(
                e.opening_in_batch_candidate(
                    &batch,
                    OpeningKind::Window,
                    host,
                    2.,
                    OpeningKind::Window.defaults()
                )
                .is_err()
            );
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            assert_eq!(e.status, status);
            assert_eq!(e.selection_ids(), selection);
            e.redo();
            assert_eq!(e.project, redo);
            e.undo();
            if reversed {
                batch.reverse();
            }
            let expected = e.preview_add_elements(batch.clone(), group).unwrap();
            e.apply(ModelCommand::AddElements {
                elements: batch,
                group,
            })
            .unwrap();
            assert_eq!(e.project, expected);
            assert_eq!(e.undo.len(), 1);
            let path = std::env::temp_dir().join(format!(
                "oa-pending-host-{}-{group}-{reversed}.json",
                std::process::id()
            ));
            storage::save(&e.project, &path).unwrap();
            assert_eq!(storage::load(&path).unwrap(), expected);
            std::fs::remove_file(path).unwrap();
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, expected);
        }
    }
}

#[test]
fn pending_host_candidate_rejects_invalid_batch_without_consuming_redo() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(9., 9.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let wall = e
        .wall_creation_candidates(vec![(p(0., 0.), p(8., 0.))])
        .unwrap()[0]
        .clone();
    let host = wall.id;
    let mut invalid = wall.clone();
    if let Shape::Wall {
        ref mut thickness, ..
    } = invalid.shape
    {
        *thickness = -1.;
    }
    let mut exhausted = wall.clone();
    exhausted.id = u64::MAX - 1;
    for (batch, target) in [
        (vec![], host),
        (vec![wall.clone(), wall], host),
        (vec![invalid], host),
        (vec![exhausted], u64::MAX - 1),
    ] {
        assert!(
            e.opening_in_batch_candidate(
                &batch,
                OpeningKind::Door,
                target,
                2.,
                OpeningKind::Door.defaults()
            )
            .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert!(e.undo.is_empty());
    }
    e.redo();
    assert_eq!(e.project, redo);
}

#[test]
fn mixed_advanced_geometry_batch_preserves_holes_inclination_and_failure_atomicity() {
    let mut batch = building();
    for e in &mut batch {
        match &mut e.shape {
            Shape::Roof { spec } => {
                spec.contour = Some(atelier_bim::roof_contour::RoofContour {
                    outer: vec![
                        p(0., 0.),
                        p(6., 0.),
                        p(6., 2.),
                        p(2., 2.),
                        p(2., 6.),
                        p(0., 6.),
                    ],
                    holes: vec![vec![p(0.5, 0.5), p(1.5, 0.5), p(1.5, 1.5), p(0.5, 1.5)]],
                });
                spec.max = p(6., 6.);
                spec.pitch = 30.;
            }
            Shape::Column { inclination, .. } => {
                *inclination = Some(atelier_bim::column_inclination::ColumnInclination {
                    slant_degrees: 60.,
                    direction_degrees: 35.,
                });
            }
            _ => {}
        }
    }
    for reverse in [false, true] {
        let mut batch = batch.clone();
        if reverse {
            batch.reverse();
        }
        let mut e = Editor::new(Project::default());
        let before = e.project.clone();
        let preview = e.preview_add_elements(batch.clone(), true).unwrap();
        assert_eq!(e.project, before);
        e.apply(ModelCommand::AddElements {
            elements: batch.clone(),
            group: true,
        })
        .unwrap();
        assert_eq!(e.project, preview);
        assert_eq!(e.undo.len(), 1);
        let roof = e
            .project
            .elements
            .iter()
            .find(|x| matches!(x.shape, Shape::Roof { .. }))
            .unwrap();
        assert!((roof.area() - 19. / 30_f64.to_radians().cos()).abs() < 1e-4);
        for (q, inside) in [(p(1., 1.), false), (p(4., 4.), false), (p(1., 3.), true)] {
            let Shape::Roof { spec } = &roof.shape else {
                unreachable!()
            };
            assert_eq!(spec.footprint_contour().contains(q), inside);
        }
        let column = e
            .project
            .elements
            .iter()
            .find(|x| matches!(x.shape, Shape::Column { .. }))
            .unwrap();
        atelier_bim::column::inclined_solid(&e.project, column)
            .unwrap()
            .mesh()
            .unwrap();
        let path = std::env::temp_dir().join(format!(
            "oa-mixed-advanced-{}-{reverse}.json",
            std::process::id()
        ));
        storage::save(&e.project, &path).unwrap();
        assert_eq!(storage::load(&path).unwrap(), preview);
        std::fs::remove_file(path).unwrap();
        e.undo();
        assert_eq!(e.project, before);
        let revision = e.revision;
        let mut invalid = batch;
        for item in &mut invalid {
            if let Shape::Roof { spec } = &mut item.shape {
                spec.contour.as_mut().unwrap().holes[0][0] = p(9., 9.);
            }
        }
        assert!(e.preview_add_elements(invalid.clone(), true).is_err());
        assert!(
            e.apply(ModelCommand::AddElements {
                elements: invalid,
                group: true
            })
            .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        e.redo();
        assert_eq!(e.project, preview);
    }
}
