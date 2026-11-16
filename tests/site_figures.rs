use atelier_bim::{
    command::ModelCommand,
    diagrams::{
        self, Definition, Domain, Edit, Presentation, Status,
        site::{Kind, Page},
    },
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    site::{
        HorizontalReference, LengthUnit, SiteEvidence,
        boundary::{BoundaryOrigin, BoundaryProposal, Placement},
    },
    storage::JsonAdapter,
    zones::ZoneSpec,
};
fn fixture(outline: Vec<[f64; 2]>) -> Editor {
    let mut e = Editor::new(Project::default());
    e.apply(ModelCommand::ApplySiteBoundary {
        proposal: BoundaryProposal {
            name: "Synthetic plot; not survey or legal approval".into(),
            story: 0,
            layer: 1,
            outline_m: outline,
            origin: BoundaryOrigin {
                source_sha256: "a".repeat(64),
                entity_handle: "A1".into(),
                record_sha256: "b".repeat(64),
                unit: LengthUnit::Metre,
                placement: Placement::ModelAxes,
                centerline_width_native: None,
            },
            expected_previous: None,
        },
    })
    .unwrap();
    e
}
fn rect(x: f64, y: f64) -> Vec<[f64; 2]> {
    vec![[x, y], [x + 20., y], [x + 20., y + 30.], [x, y + 30.]]
}
fn style(n: u32) -> Presentation {
    Presentation::SetbackStudy {
        distance_mm: n,
        assumption: "Explicit synthetic design study, not a rule".into(),
    }
}
fn create(e: &mut Editor, p: Presentation) -> Result<(), String> {
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "Local site study".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::Site,
                presentation: p,
            },
        },
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}
fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "{a} vs {b}");
}
#[test]
fn boundary_only_rectangle_study_is_336_not_buildable_area_and_preserves_source() {
    let mut e = fixture(rect(0., 0.));
    let before = e.project.clone();
    create(&mut e, style(3000)).unwrap();
    let d = &e.project.drawings.diagrams[0];
    let p = Page::new(d).unwrap();
    near(p.boundary_area_m2.unwrap(), 600., 1e-4);
    near(p.inset_area_m2.unwrap(), 336., 1e-4);
    assert_eq!(p.inset_parts, 1);
    assert_eq!(d.total(), (0., 0.));
    assert!(d.snapshot.elements.is_empty());
    assert_eq!(e.project.elements, before.elements);
    assert_eq!(e.project.site, before.site);
    let v = diagrams::query(&e.project, 0, 4).unwrap();
    assert_eq!(v["figures"][0]["site_plan"]["boundary_approval"], "unknown");
    assert_eq!(v["figures"][0]["site_plan"]["compliance"], "not_evaluated");
    let svg = d.svg(&e.project).unwrap();
    assert!(
        svg.contains("site-plan")
            && svg.contains("not an approved building line")
            && svg.contains("data-site-kind=\"Inset\"")
    );
    assert_eq!(d.status(&e.project), Status::Current);
}
#[test]
fn missing_boundary_keeps_concept_exploration_unknown_and_rejects_fake_setback() {
    let mut p = Project::default();
    let id = p.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "concept".into(),
                outline: rect(0., 0.)
                    .into_iter()
                    .map(|p| Point::new(p[0], p[1]))
                    .collect(),
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
                label_position: None,
            },
        },
    );
    let mut e = Editor::new(p);
    let orig = e.project.clone();
    assert!(create(&mut e, style(3000)).is_err());
    assert_eq!(e.project, orig);
    create(&mut e, Presentation::SiteLocation).unwrap();
    let d = &e.project.drawings.diagrams[0];
    let p = Page::new(d).unwrap();
    assert!(p.boundary_area_m2.is_none() && p.inset_area_m2.is_none());
    assert_eq!(
        d.snapshot.elements.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![id]
    );
    assert!(
        d.svg(&e.project)
            .unwrap()
            .contains("Boundary unknown; no boundary invented")
    );
    assert_eq!(e.project.site, orig.site);
}
fn distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0., 1.);
    ((p.x - a.x - t * dx).powi(2) + (p.y - a.y - t * dy).powi(2)).sqrt()
}
fn inside(p: Point, v: &[Point]) -> bool {
    let mut yes = false;
    for (a, b) in v.iter().zip(v.iter().cycle().skip(1)).take(v.len()) {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            yes = !yes;
        }
    }
    yes
}
#[test]
fn concave_neck_keeps_both_parts_with_independent_distance_checks_and_empty_result() {
    let outline = vec![
        [0., 0.],
        [10., 0.],
        [10., 4.],
        [20., 4.],
        [20., 0.],
        [30., 0.],
        [30., 10.],
        [20., 10.],
        [20., 6.],
        [10., 6.],
        [10., 10.],
        [0., 10.],
    ];
    let boundary: Vec<_> = outline.iter().map(|p| Point::new(p[0], p[1])).collect();
    let mut e = fixture(outline);
    create(&mut e, style(1100)).unwrap();
    let p = Page::new(&e.project.drawings.diagrams[0]).unwrap();
    assert_eq!(p.inset_parts, 2);
    let mut samples = 0;
    for part in p.paths.iter().filter(|p| p.kind == Kind::Inset) {
        for ring in &part.rings {
            for (a, b) in ring
                .iter()
                .zip(ring.iter().cycle().skip(1))
                .take(ring.len())
            {
                for q in [*a, Point::new((a.x + b.x) / 2., (a.y + b.y) / 2.)] {
                    assert!(inside(q, &boundary));
                    let min = boundary
                        .iter()
                        .zip(boundary.iter().cycle().skip(1))
                        .take(boundary.len())
                        .map(|(a, b)| distance(q, *a, *b))
                        .fold(f64::INFINITY, f64::min);
                    assert!(min >= 1.1 - 0.002, "sample distance {min}");
                    samples += 1;
                }
            }
        }
    }
    assert!(samples > 30);
    let mut e = fixture(rect(0., 0.));
    create(&mut e, style(11000)).unwrap();
    let d = &e.project.drawings.diagrams[0];
    let p = Page::new(d).unwrap();
    assert_eq!(p.inset_parts, 0);
    assert_eq!(p.inset_area_m2, Some(0.));
    assert!(
        d.svg(&e.project)
            .unwrap()
            .contains("No interior remains at this study distance")
    );
    let mut e = fixture(rect(0., 0.));
    create(&mut e, style(0)).unwrap();
    near(
        Page::new(&e.project.drawings.diagrams[0])
            .unwrap()
            .inset_area_m2
            .unwrap(),
        600.,
        1e-4,
    );
}
#[test]
fn model_boundary_edit_stales_and_parameter_change_never_silently_refreshes_cached_source() {
    let mut e = fixture(rect(0., 0.));
    create(&mut e, style(3000)).unwrap();
    let d = e.project.drawings.diagrams[0].clone();
    let mut el = e.project.elements[0].clone();
    if let Shape::Polyline { points, .. } = &mut el.shape {
        points[1].x += 2.;
    }
    e.apply(ModelCommand::UpdateElement { element: el })
        .unwrap();
    let src = e.project.elements.clone();
    assert_eq!(d.status(&e.project), Status::Stale);
    assert!(d.svg(&e.project).is_err());
    e.apply(ModelCommand::Diagram {
        edit: Edit::Configure {
            id: d.id,
            expected_sha256: diagrams::expected(&d),
            presentation: style(5000),
        },
    })
    .unwrap();
    let stale = e.project.drawings.diagrams[0].clone();
    assert_eq!(stale.snapshot, d.snapshot);
    assert_eq!(stale.status(&e.project), Status::Stale);
    assert!(
        e.apply(ModelCommand::Diagram {
            edit: Edit::Refresh {
                id: d.id,
                expected_sha256: diagrams::expected(&d)
            }
        })
        .is_err()
    );
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: d.id,
            expected_sha256: diagrams::expected(&stale),
        },
    })
    .unwrap();
    let current = &e.project.drawings.diagrams[0];
    assert_eq!(current.status(&e.project), Status::Current);
    assert!(current.snapshot.site.as_ref().unwrap().user_modified());
    assert_eq!(e.project.elements, src);
    e.undo();
    assert_eq!(e.project.drawings.diagrams[0], stale);
    e.redo();
    assert_eq!(e.project.elements, src);
}
#[test]
fn native14_roundtrip_rejects_old_marker_bad_assumptions_and_unsupported_domain() {
    let mut e = fixture(rect(0., 0.));
    let orig = e.project.clone();
    for style in [
        style(100001),
        Presentation::SetbackStudy {
            distance_mm: 3,
            assumption: "".into(),
        },
        Presentation::ModelQuantities,
    ] {
        assert!(create(&mut e, style).is_err());
        assert_eq!(e.project, orig);
    }
    create(&mut e, style(3000)).unwrap();
    let bytes = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 14);
    for old in [11, 12, 13] {
        v["schema_version"] = old.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&v).unwrap())
                .is_err()
        );
    }
    e.undo();
    assert_eq!(e.project, orig);
    e.redo();
    assert_eq!(e.project.drawings.diagrams.len(), 1);
}
#[test]
fn local_origin_and_exact_site_dependencies_preserve_non_site_figure_hash() {
    let mut small = fixture(rect(0., 0.));
    let mut big = fixture(rect(90000., 90000.));
    for e in [&mut small, &mut big] {
        create(e, style(3000)).unwrap();
    }
    let a = Page::new(&small.project.drawings.diagrams[0]).unwrap();
    let b = Page::new(&big.project.drawings.diagrams[0]).unwrap();
    near(a.inset_area_m2.unwrap(), b.inset_area_m2.unwrap(), 1e-4);
    near(a.layout.pixels_per_m, b.layout.pixels_per_m, 1e-7);
    let mut p = Project::default();
    p.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "concept".into(),
                outline: rect(0., 0.)
                    .into_iter()
                    .map(|p| Point::new(p[0], p[1]))
                    .collect(),
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
                label_position: None,
            },
        },
    );
    let mut e = Editor::new(p);
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "Legacy".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::Zones,
                presentation: Default::default(),
            },
        },
    })
    .unwrap();
    let old = e.project.drawings.diagrams[0].clone();
    assert!(
        !serde_json::to_value(&old.snapshot)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("site")
    );
    create(&mut e, Presentation::SiteLocation).unwrap();
    let new = e.project.drawings.diagrams[1].clone();
    let mut context = e.project.site.clone();
    context.horizontal = Some(HorizontalReference {
        crs: "Synthetic declaration only".into(),
        origin_east_north_m: [500000., 1000000.],
        model_to_grid_ccw_deg: 0.,
        evidence: SiteEvidence {
            source_sha256: "a".repeat(64),
            locator: "fixture".into(),
            note: "not a survey".into(),
        },
    });
    e.apply(ModelCommand::SetSiteContext { context }).unwrap();
    assert_eq!(old.status(&e.project), Status::Current);
    assert_eq!(new.status(&e.project), Status::Stale);
    assert_eq!(e.project.drawings.diagrams[0], old);
}

#[test]
fn upper_distance_round_sampling_stays_within_frozen_study_tolerance() {
    let source = vec![
        [0., 0.],
        [500., 0.],
        [500., 200.],
        [1000., 200.],
        [1000., 0.],
        [1500., 0.],
        [1500., 500.],
        [1000., 500.],
        [1000., 300.],
        [500., 300.],
        [500., 500.],
        [0., 500.],
    ];
    let boundary: Vec<_> = source.iter().map(|p| Point::new(p[0], p[1])).collect();
    let mut e = fixture(source);
    create(&mut e, style(100000)).unwrap();
    let page = Page::new(&e.project.drawings.diagrams[0]).unwrap();
    assert_eq!(page.inset_parts, 2);
    let mut samples = 0;
    for p in page.paths.iter().filter(|p| p.kind == Kind::Inset) {
        for ring in &p.rings {
            for (a, b) in ring
                .iter()
                .zip(ring.iter().cycle().skip(1))
                .take(ring.len())
            {
                let q = Point::new((a.x + b.x) / 2., (a.y + b.y) / 2.);
                assert!(inside(q, &boundary));
                let distance = boundary
                    .iter()
                    .zip(boundary.iter().cycle().skip(1))
                    .take(boundary.len())
                    .map(|(a, b)| distance(q, *a, *b))
                    .fold(f64::INFINITY, f64::min);
                assert!(distance >= 100. - 0.025, "{distance}");
                samples += 1;
            }
        }
    }
    assert!(samples > 30);
}

#[test]
fn site_labels_respect_holes_and_geometry_clears_scale_text() {
    let mut e = fixture(rect(0., 0.));
    e.project.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "Label fixture".into(),
                outline: vec![
                    Point::new(2., 2.),
                    Point::new(18., 2.),
                    Point::new(18., 28.),
                    Point::new(2., 28.),
                ],
                holes: vec![vec![
                    Point::new(8., 12.),
                    Point::new(12., 12.),
                    Point::new(12., 18.),
                    Point::new(8., 18.),
                ]],
                height: 3.,
                bottom_offset: 0.,
                label_position: None,
            },
        },
    );
    create(&mut e, Presentation::SiteLocation).unwrap();
    let d = &e.project.drawings.diagrams[0];
    let page = Page::new(d).unwrap();
    let bottom = page.layout.point(page.layout.min)[1];
    assert!(
        bottom <= 675.001,
        "geometry must leave room above scale text at y693: {bottom}"
    );
    let zone = &d.snapshot.elements[0];
    let Shape::Zone { spec } = &zone.shape else {
        unreachable!()
    };
    let label = page
        .labels()
        .into_iter()
        .find(|(id, _, _)| *id == zone.id)
        .unwrap();
    assert!(label.2);
    let tx = 40. + (750. - (page.layout.max.x - page.layout.min.x) * page.layout.pixels_per_m) / 2.;
    let ty =
        115. + (580. - (page.layout.max.y - page.layout.min.y) * page.layout.pixels_per_m) / 2.;
    let world = Point::new(
        page.layout.min.x + (label.1[0] - tx) / page.layout.pixels_per_m,
        page.layout.max.y - (label.1[1] - ty) / page.layout.pixels_per_m,
    );
    assert!(inside(world, &spec.outline) && !inside(world, &spec.holes[0]));
    let svg = d.svg(&e.project).unwrap();
    for id in [
        e.project.site.boundary.as_ref().unwrap().element_id,
        zone.id,
    ] {
        assert!(svg.contains(&format!("data-site-object-label=\"{id}\"")));
    }
    let mut empty = Editor::new(Project::default());
    let before = empty.project.clone();
    assert!(create(&mut empty, Presentation::SiteLocation).is_err());
    assert_eq!(empty.project, before);
}
