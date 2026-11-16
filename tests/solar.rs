use atelier_bim::{
    concept_mass::ConceptMassSpec,
    model::{Material, Point, Project, Shape},
    solar::*,
};
fn north() -> North {
    North::DesignAssumption {
        clockwise_from_model_y_deg: 0.,
        note: "Synthetic software fixture; not surveyed north".into(),
    }
}
fn time(day: u8, hour: u8) -> Utc {
    Utc {
        year: 2003,
        month: 10,
        day,
        hour,
        minute: 0,
        second: 0,
    }
}
fn request() -> Request {
    Request {
        location: Location {
            latitude_deg: 39.742476,
            longitude_deg: -105.1786,
        },
        location_basis: LocationBasis::DesignAssumption {
            note: "Synthetic independent reference site, not current project location".into(),
        },
        start_utc: time(17, 18),
        end_utc: time(17, 19),
        step_seconds: 600,
        ray_offset_m: 0.0001,
        receivers: vec![Receiver {
            id: "roof".into(),
            point_m: [0., 0., 100.],
            outward_normal: [0., 0., 1.],
        }],
    }
}
fn fixture() -> (Project, u64) {
    let mut p = Project::default();
    let id = p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(-2., -2.),
                    Point::new(2., -2.),
                    Point::new(2., 2.),
                    Point::new(-2., 2.),
                ],
                holes: vec![vec![
                    Point::new(-1., -1.),
                    Point::new(1., -1.),
                    Point::new(1., 1.),
                    Point::new(-1., 1.),
                ]],
                height: 2.,
                bottom_offset: 0.,
            },
        },
    );
    (p, id)
}
#[test]
fn original_position_matches_independent_unrefracted_reference_with_frozen_tolerance() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/solar/golden.json")).unwrap();
    let location: Location = serde_json::from_value(data["location"].clone()).unwrap();
    let tolerance = data["frozen_max_angular_error_deg"].as_f64().unwrap();
    for case in data["cases"].as_array().unwrap() {
        let t = serde_json::from_value(case["time"].clone()).unwrap();
        let p = position(location, t).unwrap();
        let expected = case["elevation_deg"].as_f64().unwrap();
        let azimuth = case["azimuth_deg"].as_f64().unwrap();
        println!(
            "independent {t:?}: elevation{} reference{} error{}; azimuth{} reference{} error{}",
            p.elevation_deg,
            expected,
            (p.elevation_deg - expected).abs(),
            p.azimuth_deg.unwrap(),
            azimuth,
            (p.azimuth_deg.unwrap() - azimuth).abs()
        );
        assert!((p.elevation_deg - expected).abs() < tolerance);
        assert!((p.azimuth_deg.unwrap() - azimuth).abs() < tolerance);
        let len = p.east_north_up.iter().map(|x| x * x).sum::<f64>();
        assert!((len - 1.).abs() < 1e-12);
    }
}
#[test]
fn utc_calendar_validation_j2000_and_instant_conventions() {
    let noon = Utc {
        year: 2000,
        month: 1,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
    };
    assert_eq!(noon.julian_day().unwrap(), 2451545.);
    assert_eq!(
        Utc {
            year: 1970,
            month: 1,
            day: 1,
            hour: 0,
            ..noon
        }
        .julian_day()
        .unwrap(),
        2440587.5
    );
    let leap = Utc {
        year: 2000,
        month: 2,
        day: 29,
        ..noon
    };
    assert!(leap.julian_day().is_ok());
    assert!(Utc { year: 1900, ..leap }.julian_day().is_err());
    assert!(Utc { second: 60, ..noon }.julian_day().is_err());
    assert!(Utc { day: 0, ..noon }.julian_day().is_err());
    assert!(Utc { year: 2101, ..noon }.julian_day().is_err());
    let p = position(
        Location {
            latitude_deg: 0.,
            longitude_deg: 0.,
        },
        noon,
    )
    .unwrap();
    assert!(p.elevation_deg > 60.);
    let opposite = position(
        Location {
            latitude_deg: 0.,
            longitude_deg: 180.,
        },
        noon,
    )
    .unwrap();
    assert!(opposite.elevation_deg < -60.);
    assert!((p.elevation_deg + opposite.elevation_deg).abs() < 1e-10);
    for bad in [
        Location {
            latitude_deg: f64::NAN,
            longitude_deg: 0.,
        },
        Location {
            latitude_deg: 0.,
            longitude_deg: 181.,
        },
    ] {
        assert!(position(bad, noon).is_err());
    }
}
#[test]
fn capture_reuses_exact_prism_and_unknown_north_does_not_fabricate_a_project_result() {
    let (mut p, id) = fixture();
    let before = p.clone();
    assert!(
        capture(&p, &[id], None)
            .unwrap_err()
            .contains("True north is unknown")
    );
    assert_eq!(p, before);
    let s = capture(&p, &[id], Some(north())).unwrap();
    assert_eq!(
        s.blockers[0].triangles.len(),
        atelier_bim::concept_mass::derive(&p.elements[0], p.elevation(0))
            .unwrap()
            .triangles()
            .count()
    );
    assert!(s.is_current(&p).unwrap());
    let mut tampered = s.clone();
    tampered.blockers[0].triangles[0][0][0] += 0.1;
    assert!(!tampered.is_current(&p).unwrap());
    if let Shape::ConceptMass { spec } = &mut p.elements[0].shape {
        spec.height += 1.;
    }
    assert!(!s.is_current(&p).unwrap());
    p = before;
    p.elements[0].shape = Shape::Slab {
        min: Point::new(0., 0.),
        max: Point::new(2., 2.),
        thickness: 0.3,
    };
    assert!(
        capture(&p, &[id], Some(north()))
            .unwrap_err()
            .contains("only")
    );
    assert!(capture(&p, &[id, id], Some(north())).is_err());
}
#[test]
fn explicit_declared_north_wins_and_its_source_changes_invalidate() {
    let (mut p, id) = fixture();
    p.site.true_north = Some(atelier_bim::site::TrueNorth {
        clockwise_from_model_y_deg: 90.,
        evidence: atelier_bim::site::SiteEvidence {
            source_sha256: "a".repeat(64),
            locator: "Synthetic test site".into(),
            note: "Not surveyed".into(),
        },
    });
    let s = capture(&p, &[id], Some(north())).unwrap();
    assert!(matches!(s.north, North::Declared(_)));
    assert_eq!(s.north.angle(), 90.);
    p.site
        .true_north
        .as_mut()
        .unwrap()
        .evidence
        .note
        .push_str(" edited");
    assert!(!s.is_current(&p).unwrap());
}
#[test]
fn bounded_study_midpoints_terminal_partial_cell_saved_result_and_input_changes() {
    let (p, id) = fixture();
    let s = capture(&p, &[id], Some(north())).unwrap();
    let mut r = request();
    r.end_utc.minute = 1;
    r.end_utc.second = 10; //3670s, not exact cells
    let before = p.clone();
    let result = run(&s, &r).unwrap();
    assert_eq!(p, before);
    assert_eq!(result.points[0].cells.len(), 7);
    assert_eq!(result.points[0].sampled_sunlit_seconds, 3670);
    assert_eq!(
        result.points[0].sunlit_segments,
        vec![Segment {
            start_seconds: 0,
            end_seconds: 3670
        }]
    );
    let last = result.points[0].cells.last().unwrap();
    assert_eq!(
        (last.start_seconds, last.end_seconds, last.sample_seconds),
        (3600, 3670, 3635.)
    );
    let encoded = serde_json::to_vec(&result).unwrap();
    let saved: ResultSet = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(saved, result);
    assert!(saved.matches_inputs(&s, &r).unwrap());
    let mut changed = r.clone();
    changed.step_seconds = 300;
    assert!(!saved.matches_inputs(&s, &changed).unwrap());
    let mut changed = s.clone();
    changed.source_sha256 = "b".repeat(64);
    assert!(!saved.matches_inputs(&changed, &r).unwrap());
    r.start_utc.hour = 6;
    r.end_utc.hour = 7;
    let night = run(&s, &r).unwrap();
    assert!(night.points[0].sunlit_segments.is_empty());
    assert!(
        night.points[0]
            .cells
            .iter()
            .all(|c| c.state == State::BelowHorizon)
    );
}
#[test]
fn invalid_or_over_budget_requests_refuse_instead_of_changing_sampling() {
    let (p, id) = fixture();
    let s = capture(&p, &[id], Some(north())).unwrap();
    let mut r = request();
    r.location_basis = LocationBasis::DesignAssumption {
        note: String::new(),
    };
    assert!(run(&s, &r).is_err());
    r = request();
    r.receivers[0].outward_normal = [0., 0., 2.];
    assert!(run(&s, &r).is_err());
    r = request();
    r.receivers.push(r.receivers[0].clone());
    assert!(run(&s, &r).is_err());
    r = request();
    r.step_seconds = 1;
    assert!(run(&s, &r).is_err());
    r = request();
    r.ray_offset_m = 1.;
    assert!(run(&s, &r).is_err());
    r = request();
    r.end_utc = r.start_utc;
    assert!(run(&s, &r).is_err());
}
#[test]
fn current_saved_case_is_read_only_and_cannot_silently_assume_true_north() {
    use atelier_bim::model::ModelAdapter;
    use atelier_bim::storage::JsonAdapter;
    let input = std::env::var_os("OPENARCHI_SOLAR_CASE_INPUT").map(std::path::PathBuf::from);
    let (p, raw) = if let Some(path) = input {
        let mut stored = atelier_bim::storage::load(&path).unwrap();
        for page in &stored.drawings.pages {
            atelier_bim::drawings::verified_bytes(page).unwrap();
        }
        let bytes = JsonAdapter.export(&stored).unwrap();
        (std::mem::take(&mut stored), bytes)
    } else {
        let p = atelier_bim::storage::startup_example();
        let raw = JsonAdapter.export(&p).unwrap();
        (p, raw)
    };
    let before = p.clone();
    let mut ids = p
        .elements
        .iter()
        .filter(|e| {
            matches!(e.shape, Shape::ConceptMass { .. })
                && e.design_option == p.plan_options.active_design_option
        })
        .map(|e| e.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    assert!(!ids.is_empty());
    assert!(
        p.site.true_north.is_none(),
        "current fixture assumption changed; review input, never overwrite user north"
    );
    assert!(capture(&p, &ids, None).is_err());
    let snapshot = capture(&p, &ids, Some(north())).unwrap();
    assert!(snapshot.is_current(&p).unwrap());
    assert_eq!(p, before);
    assert_eq!(JsonAdapter.export(&p).unwrap(), raw);
}
