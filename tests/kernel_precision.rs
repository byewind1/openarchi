use atelier_bim::{
    command::ModelCommand,
    creation_input::CreationPoint,
    editor::Editor,
    model::{ModelAdapter, Point, Project, Shape, SlabContour},
    precision::{GeometryContext, RenderOrigin, TolerancePolicy},
    slab_geometry,
    storage::JsonAdapter,
};
fn contour(origin: f64) -> SlabContour {
    let p = |x, y| Point::new(origin + x, origin + y);
    SlabContour {
        points: vec![p(0., 0.), p(8.013, 0.), p(8.013, 6.019), p(0., 6.019)],
        holes: vec![SlabContour {
            points: vec![p(0.5, 0.5), p(1.513, 0.5), p(1.513, 1.519), p(0.5, 1.519)],
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn holed_slab_exact_input_model_history_json_and_translation_meet_si_budget() {
    let tolerance = TolerancePolicy::default();
    for origin in [0., 1000., -1000., 99900., -99900.] {
        let mut e = Editor::new(Project::default());
        let mut c = contour(origin);
        c.points[1] = CreationPoint {
            x: origin + 8.013,
            y: origin,
        }
        .model_point();
        let element = e.slab_creation_candidate(c.clone()).unwrap();
        let prepared = e
            .prepare_change(0, vec![ModelCommand::AddElement { element }])
            .unwrap();
        e.commit_change("precise-slab", &prepared).unwrap();
        let expected = e.project.clone();
        let stored = &e.project.elements[0];
        assert!(
            tolerance
                .area_m2
                .accepts(slab_geometry::actual_area(stored), 47.198)
        );
        let c2 = slab_geometry::contour(stored);
        assert_eq!(c2, c);
        assert!(
            tolerance
                .length_m
                .accepts(c2.points[0].distance(c2.points[1]), 8.013)
        );
        let bytes = JsonAdapter.export(&e.project).unwrap();
        assert_eq!(JsonAdapter.import(&bytes).unwrap(), expected);
        e.undo();
        assert!(e.project.elements.is_empty());
        e.redo();
        assert_eq!(e.project, expected);
        for offset in [Point::new(-origin, -origin), Point::new(origin, origin)] {
            e.apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset,
            })
            .unwrap();
        }
        assert!(
            tolerance
                .area_m2
                .accepts(slab_geometry::actual_area(&e.project.elements[0]), 47.198)
        );
        let after = slab_geometry::contour(&e.project.elements[0]);
        assert_eq!(after.holes.len(), 1);
        for (a, b) in c.points.iter().zip(after.points) {
            assert!(a.distance(b) <= 0.0001);
        }
    }
}
#[test]
fn legacy_files_widen_existing_values_and_future_versions_are_rejected() {
    let mut old = serde_json::to_value(Project::default()).unwrap();
    old["schema_version"] = 1.into();
    let loaded = JsonAdapter
        .import(&serde_json::to_vec(&old).unwrap())
        .unwrap();
    assert_eq!(loaded.schema_version, 2);
    let mut expected = Project::default();
    expected.stories[0].elevation = 1.2345678901234567;
    let mut seed = 0x6d75_f3aa_u64;
    for _ in 0..512 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        expected.stories[0].elevation = (seed as f64 / u64::MAX as f64 - 0.5) * 2000.;
        assert_eq!(
            JsonAdapter
                .import(&JsonAdapter.export(&expected).unwrap())
                .unwrap(),
            expected
        );
    }
    for version in [0, 3, u32::MAX] {
        old["schema_version"] = version.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&old).unwrap())
                .is_err()
        );
    }
    // Already quantized values remain quantized; migration must not fabricate intent.
    let saved = 99908.013_f64 as f32;
    let mut legacy = Project::default();
    legacy.add(
        0,
        atelier_bim::model::Material::Concrete,
        Shape::Wall {
            start: Point::new(99900., 0.),
            end: Point::new(f64::from(saved), 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let mut value = serde_json::to_value(&legacy).unwrap();
    value["schema_version"] = 1.into();
    assert_eq!(
        JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .unwrap(),
        legacy
    );
}
#[test]
fn render_origin_narrows_local_coordinates_and_units_have_distinct_budgets() {
    let budget = TolerancePolicy::default();
    for x in [0., 1000., -1000., 99900., -99900.] {
        let origin = glam::DVec3::new(x, x, 100.);
        let local = RenderOrigin(origin).position(origin + glam::DVec3::new(8.013, 6.019, 0.2));
        for (actual, expected) in local.into_iter().zip([8.013, 6.019, 0.2]) {
            assert!(budget.length_m.accepts(f64::from(actual), expected));
        }
    }
    for p in [
        Point::new(f64::NAN, 0.),
        Point::new(f64::INFINITY, 0.),
        Point::new(100000., 0.),
        Point::new(0., -100000.),
    ] {
        assert!(!GeometryContext::point_in_range(p));
    }
    assert!(GeometryContext::point_in_range(Point::new(
        99999.99, -99999.99
    )));
    assert!(!budget.area_m2.accepts(47.1875, 47.198));
    assert!(!budget.length_m.accepts(f64::NAN, 1.));
}

#[test]
fn camera_projection_matches_previous_small_coordinate_convention_and_large_translation() {
    use atelier_bim::geometry::Camera;
    let camera = Camera {
        yaw: 0.7,
        pitch: 0.5,
        span: 20.,
        target: glam::DVec3::new(6., 3., 0.8),
    };
    let aspect = 1.5;
    let matrix = camera.matrix(aspect);
    let fov = 45_f32.to_radians();
    let distance = camera.vertical_span(aspect) as f32 / (2. * (fov * 0.5).tan());
    let dir = glam::Vec3::new(
        (camera.yaw as f32).cos() * (camera.pitch as f32).cos(),
        (camera.yaw as f32).sin() * (camera.pitch as f32).cos(),
        (camera.pitch as f32).sin(),
    );
    let far = (distance * 4.).max(1000.);
    let near = (far / 20000.).max(0.05);
    let target = camera.target.as_vec3();
    let previous = glam::camera::rh::proj::directx::perspective(fov, aspect as f32, near, far)
        * glam::camera::rh::view::look_at_mat4(target + dir * distance, target, glam::Vec3::Z);
    for point in [
        camera.target,
        glam::DVec3::new(3., 4., 2.),
        glam::DVec3::new(10., -2., 0.),
    ] {
        let new = matrix.project_point3(point);
        let old = previous.project_point3(point.as_vec3()).as_dvec3();
        assert!((new - old).length() < 1e-5);
        for shift in [
            glam::DVec3::new(99900., -99900., 0.),
            glam::DVec3::new(-99900., 99900., 0.),
        ] {
            let mut moved = camera.clone();
            moved.target += shift;
            assert!((moved.matrix(aspect).project_point3(point + shift) - new).length() < 1e-9);
        }
    }
    let projected = matrix.project_point3(camera.target);
    assert!(projected.x.abs() < 1e-12 && projected.y.abs() < 1e-12);
    assert!(projected.z > 0. && projected.z < 1.);
}

#[test]
fn minimum_stair_run_accepts_exact_decimal_boundary_without_accepting_real_undersize() {
    use atelier_bim::stairs::StairSpec;
    for origin in [0., 4., 1000., -1000., 99900., -99900.] {
        let mut spec = StairSpec {
            start: Point::new(origin, 0.),
            end: Point::new(origin + 0.3, 0.),
            ..Default::default()
        };
        assert!(spec.validate().is_ok(), "origin={origin}");
        spec.end.x = origin + 0.3 - 1e-6;
        assert!(spec.validate().is_err(), "undersize at origin={origin}");
        spec.end = spec.start;
        assert!(spec.validate().is_err());
    }
}
