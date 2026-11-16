use atelier_bim::{
    column::ColumnSection,
    column_inclination::ColumnInclination,
    column_plan::ColumnPlanDisplay,
    editor::{Editor, Tool},
    model::{ArcSegment, Material, Point, Project, Shape},
    roof::RoofSpec,
    roof_contour::RoofContour,
    storage,
    transfer::{
        self, GROUP_FLOOR_PLAN_DISPLAY, GROUP_GEOMETRY, GROUP_MATERIAL_LAYER, TransferSettings,
        TransferType,
    },
};
fn roof(x: f64, pitch: f64) -> RoofSpec {
    RoofSpec {
        contour: Some(RoofContour {
            outer: vec![
                Point::new(x, 0.),
                Point::new(x + 6., 0.),
                Point::new(x + 6., 6.),
                Point::new(x, 6.),
            ],
            holes: vec![vec![
                Point::new(x + 2., 2.),
                Point::new(x + 2., 3.),
                Point::new(x + 3., 3.),
                Point::new(x + 3., 2.),
            ]],
        }),
        pivot_start: Point::new(x, 0.),
        pivot_end: Point::new(x + 6., 0.),
        rise_left: true,
        min: Point::new(x, 0.),
        max: Point::new(x + 6., 6.),
        pitch,
        thickness: 0.3,
        pivot_offset: 1.2,
    }
}
fn fixture(t: TransferType) -> (Editor, u64, u64) {
    let mut e = Editor::new(Project::default());
    let (s, t) = match t {
        TransferType::Column => {
            e.column_section = ColumnSection::Circular { diameter: 0.8 };
            e.column_height = 4.;
            e.column_axis_rotation = 35.;
            e.column_bottom_offset = -0.3;
            e.column_slanted = true;
            e.column_inclination = ColumnInclination {
                slant_degrees: 70.,
                direction_degrees: 25.,
            };
            e.column_plan.display = ColumnPlanDisplay::CutOnly;
            let s = e.place_column(Point::new(0., 0.)).unwrap();
            e.column_section = ColumnSection::Rectangular {
                width: 0.3,
                depth: 0.4,
            };
            e.column_height = 3.;
            e.column_slanted = false;
            e.column_bottom_offset = 0.;
            e.column_axis_rotation = 0.;
            e.column_plan = Default::default();
            (s, e.place_column(Point::new(20., 0.)).unwrap())
        }
        TransferType::Beam => {
            e.beam_size = [0.7, 0.9, 2.];
            let s = e
                .place_beam(Point::new(0., 0.), Point::new(6., 0.))
                .unwrap();
            e.beam_size = [0.3, 0.5, 3.];
            (
                s,
                e.place_curved_beam(
                    Point::new(20., 0.),
                    Point::new(24., 0.),
                    ArcSegment {
                        radius: 3.,
                        clockwise: false,
                        large_arc: true,
                    },
                )
                .unwrap(),
            )
        }
        TransferType::Roof => {
            let s = e.place_roof(roof(0., 35.)).unwrap();
            let mut target = roof(20., 10.);
            target.rise_left = false;
            target.thickness = 0.2;
            target.pivot_offset = 0.;
            (s, e.place_roof(target).unwrap())
        }
        _ => unreachable!(),
    };
    e.project.elements[0].material = Material::Timber;
    (e, s, t)
}
fn only(group: &str) -> TransferSettings {
    let mut s = TransferSettings::default();
    s.sets[0].groups.clear();
    s.sets[0].groups.insert(group.into());
    s
}
#[test]
fn model_types_transfer_defaults_preserve_placement_and_roundtrip_atomically() {
    for t in [TransferType::Column, TransferType::Beam, TransferType::Roof] {
        let (mut e, source, target) = fixture(t);
        let set = TransferSettings::default();
        let before = e.project.clone();
        let history = e.undo.len();
        let revision = e.revision;
        assert_eq!(e.pick_up_from(source, &set).unwrap(), t);
        assert_eq!(e.project, before);
        assert_eq!(e.undo.len(), history);
        assert_eq!(e.revision, revision);
        let tool = match t {
            TransferType::Column => Tool::Column,
            TransferType::Beam => Tool::Beam,
            TransferType::Roof => Tool::Roof,
            _ => unreachable!(),
        };
        assert_eq!(e.tool, tool);
        match t {
            TransferType::Column => {
                assert_eq!(e.column_material, Material::Timber);
                assert_eq!(e.column_section, ColumnSection::Circular { diameter: 0.8 });
                assert!(e.column_slanted);
                assert_eq!(e.column_plan.display, ColumnPlanDisplay::CutOnly);
                let v = e.column_creation_preview(Point::new(40., 0.)).unwrap();
                assert!(
                    matches!(v.shape,Shape::Column{center,height:4.,..} if center==Point::new(40.,0.))
                );
            }
            TransferType::Beam => {
                assert_eq!(e.beam_material, Material::Timber);
                assert_eq!(e.beam_size, [0.7, 0.9, 2.]);
            }
            TransferType::Roof => {
                assert_eq!(e.roof_material, Material::Timber);
                assert_eq!(e.roof_size, [35., 0.3, 1.2]);
            }
            _ => unreachable!(),
        }
        assert_eq!(e.inject_into(&[target], &set).unwrap(), 1);
        assert_eq!(e.undo.len(), history + 1);
        let after = e.project.clone();
        let a = &after.elements[1];
        let b = &before.elements[1];
        assert_eq!(a.id, b.id);
        assert_eq!(a.name, b.name);
        assert_eq!(a.story, b.story);
        assert_eq!(a.material, Material::Timber);
        match (&a.shape, &b.shape) {
            (
                Shape::Column {
                    center,
                    section,
                    height,
                    axis_rotation,
                    inclination,
                    ..
                },
                Shape::Column { center: bc, .. },
            ) => {
                assert_eq!(center, bc);
                assert_eq!(*section, ColumnSection::Circular { diameter: 0.8 });
                assert_eq!(*height, 4.);
                assert_eq!(*axis_rotation, 35.);
                assert_eq!(inclination.unwrap().slant_degrees, 70.);
            }
            (
                Shape::Beam {
                    start,
                    end,
                    arc,
                    width,
                    height,
                    top_offset,
                    ..
                },
                Shape::Beam {
                    start: bs,
                    end: be,
                    arc: ba,
                    ..
                },
            ) => {
                assert_eq!((start, end, arc), (bs, be, ba));
                assert_eq!((*width, *height, *top_offset), (0.7, 0.9, 2.));
            }
            (Shape::Roof { spec: a }, Shape::Roof { spec: b }) => {
                let mut expected = b.clone();
                expected.pitch = 35.;
                expected.thickness = 0.3;
                expected.pivot_offset = 1.2;
                assert_eq!(*a, expected);
            }
            _ => panic!(),
        }
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, after);
        let path = std::env::temp_dir().join(format!(
            "oa-model-transfer-{}-{:?}.json",
            std::process::id(),
            t
        ));
        storage::save(&after, &path).unwrap();
        assert_eq!(storage::load(&path).unwrap(), after);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn group_switches_preserve_unselected_parameters_and_creation_methods() {
    for t in [TransferType::Column, TransferType::Beam, TransferType::Roof] {
        let (mut e, source, target) = fixture(t);
        let source_element = e.project.elements[0].clone();
        let target_element = e.project.elements[1].clone();
        let mut material_target = target_element.clone();
        transfer::apply_transfer(
            &source_element,
            &mut material_target,
            &only(GROUP_MATERIAL_LAYER),
        )
        .unwrap();
        assert_eq!(material_target.shape, target_element.shape);
        assert_eq!(material_target.material, Material::Timber);
        let mut geom_target = target_element.clone();
        transfer::apply_transfer(&source_element, &mut geom_target, &only(GROUP_GEOMETRY)).unwrap();
        assert_eq!(geom_target.material, target_element.material);
        let column = e.column_section;
        let beam = e.beam_size;
        let roof = e.roof_size;
        e.column_two_click = true;
        e.pick_up_from(source, &only(GROUP_MATERIAL_LAYER)).unwrap();
        assert_eq!(
            (e.column_section, e.beam_size, e.roof_size),
            (column, beam, roof)
        );
        assert!(e.column_two_click);
        if t == TransferType::Column {
            e.pick_up_from(source, &only(GROUP_FLOOR_PLAN_DISPLAY))
                .unwrap();
            assert_eq!(e.column_section, column);
            assert_eq!(e.column_plan.display, ColumnPlanDisplay::CutOnly);
            e.inject_into(&[target], &only(GROUP_FLOOR_PLAN_DISPLAY))
                .unwrap();
            let mut expected = target_element.clone();
            if let Shape::Column { plan, .. } = &mut expected.shape {
                plan.display = ColumnPlanDisplay::CutOnly;
            }
            assert_eq!(e.project.elements[1], expected);
        }
    }
}
#[test]
fn incompatible_curved_beam_batch_preserves_model_and_real_redo() {
    let (mut e, source, target) = fixture(TransferType::Beam);
    // Source inclination is valid for a straight beam, but not for the curved target.
    if let Shape::Beam { slant_angle, .. } = &mut e.project.elements[0].shape {
        *slant_angle = 15.;
    }
    let straight = e
        .place_beam(Point::new(30., 0.), Point::new(36., 0.))
        .unwrap();
    e.place_column(Point::new(40., 0.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    e.pick_up_from(source, &TransferSettings::default())
        .unwrap();
    let before = e.project.clone();
    let undo = e.undo.len();
    let revision = e.revision;
    assert!(
        e.inject_into(&[straight, target], &TransferSettings::default())
            .is_err()
    );
    assert_eq!(e.project, before);
    assert_eq!(e.undo.len(), undo);
    assert_eq!(e.revision, revision);
    e.redo();
    assert_eq!(e.project, redo);
}

#[test]
fn protected_new_types_reject_mixed_batch_without_consuming_redo() {
    for kind in [TransferType::Column, TransferType::Beam, TransferType::Roof] {
        let (mut e, source, target) = fixture(kind);
        let mut extra = e.project.elements[1].clone();
        extra.id = e.project.next_id;
        extra.name = "Other target".into();
        let extra_id = extra.id;
        e.apply(atelier_bim::command::ModelCommand::AddElement { element: extra })
            .unwrap();
        e.project.edit_state.locked.insert(target);
        e.place_column(Point::new(60., 0.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        e.pick_up_from(source, &TransferSettings::default())
            .unwrap();
        let before = e.project.clone();
        let revision = e.revision;
        let undo = e.undo.len();
        for ids in [[extra_id, target], [target, extra_id]] {
            assert!(e.inject_into(&ids, &TransferSettings::default()).is_err());
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            assert_eq!(e.undo.len(), undo);
        }
        e.redo();
        assert_eq!(e.project, redo);
    }
}
