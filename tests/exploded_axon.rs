use atelier_bim::{
    command::ModelCommand,
    diagrams::{
        self, Definition, Domain, Edit, Presentation, Status,
        axon::{DisplayOffset, Page, project},
    },
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
    zones::ZoneSpec,
};
use glam::DVec3;
fn ring(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + w, y),
        Point::new(x + w, y + h),
        Point::new(x, y + h),
    ]
}
fn fixture(at: f64) -> Editor {
    let mut p = Project::default();
    p.stories[0].elevation = 7.;
    p.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "living".into(),
                outline: ring(at, at, 8., 6.),
                holes: vec![ring(at + 2., at + 2., 2., 2.)],
                height: 3.,
                bottom_offset: 1.,
                label_position: None,
            },
        },
    );
    p.add(
        0,
        Material::Concrete,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "bedroom".into(),
                outline: ring(at + 10., at, 4., 5.),
                holes: vec![],
                height: 4.,
                bottom_offset: 0.,
                label_position: None,
            },
        },
    );
    Editor::new(p)
}
fn style(ids: &[u64], step: i32) -> Presentation {
    Presentation::ExplodedAxon {
        offsets_mm: ids
            .iter()
            .enumerate()
            .map(|(i, id)| DisplayOffset {
                element_id: *id,
                xyz: [0, 0, i as i32 * step],
            })
            .collect(),
    }
}
fn create(e: &mut Editor, presentation: Presentation) -> Result<(), String> {
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "Exploded original objects".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::Zones,
                presentation,
            },
        },
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}")
}
#[test]
fn equal_axis_projection_keeps_real_height_holes_and_model_unchanged() {
    let x = project(DVec3::X);
    let y = project(DVec3::Y);
    let z = project(DVec3::Z);
    close(x[0], 1. / 2_f64.sqrt());
    close(y[0], -x[0]);
    close(z[1], -2. / 6_f64.sqrt());
    for v in [x, y, z] {
        close(v[0] * v[0] + v[1] * v[1], 2. / 3.);
    }
    let mut e = fixture(0.);
    let before = e.project.clone();
    let ids = e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>();
    create(&mut e, style(&ids, 2000)).unwrap();
    assert_eq!(e.project.elements, before.elements);
    let f = &e.project.drawings.diagrams[0];
    close(f.total().0, 64.);
    close(f.total().1, 212.);
    assert!(f.function_groups().is_empty());
    let page = Page::new(f).unwrap();
    close(page.origin.z, 7.);
    assert_eq!(page.labels.len(), 2);
    assert!(page.faces.len() > 24);
    // Known courtyard boundary vertices at bottom=8 and top=11 exist in projected mesh.
    for z in [8., 11.] {
        let q = project(DVec3::new(2., 2., z) - page.origin);
        assert!(page.faces.iter().any(|f| {
            f.element_id == ids[0]
                && f.points
                    .iter()
                    .any(|p| (p[0] - q[0]).abs() < 1e-8 && (p[1] - q[1]).abs() < 1e-8)
        }));
    }
    // Second prism is translated only in the derived display, with actual top=11 plus 2m.
    let q = project(DVec3::new(10., 0., 13.) - page.origin);
    assert!(
        page.faces
            .iter()
            .any(|f| f.element_id == ids[1] && f.points.contains(&q))
    );
    assert_eq!(f.status(&e.project), Status::Current);
    let svg = f.svg(&e.project).unwrap();
    assert!(
        svg.contains("axon-projection")
            && svg.contains("data-axon-face")
            && svg.contains("not a paper scale")
    );
    assert!(!svg.contains("N (declared)"));
    assert!(page.faces.windows(2).all(|w| w[0].depth <= w[1].depth));
    for face in &page.faces {
        for q in face.points {
            let q = page.point(q);
            assert!((39.999..=790.001).contains(&q[0]) && (114.999..=695.001).contains(&q[1]));
        }
    }
}
#[test]
fn offset_config_preview_cancel_history_native13_and_old_marker_rejection() {
    let mut e = fixture(0.);
    let ids = e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>();
    create(&mut e, style(&ids, 1000)).unwrap();
    let before = e.project.clone();
    let f = before.drawings.diagrams[0].clone();
    let change = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::Diagram {
                edit: Edit::Configure {
                    id: f.id,
                    expected_sha256: diagrams::expected(&f),
                    presentation: style(&ids, 3000),
                },
            }],
        )
        .unwrap();
    assert_eq!(e.project, before);
    e.commit_change("display-only", &change).unwrap();
    assert_eq!(e.project.elements, before.elements);
    assert_eq!(e.project.drawings.diagrams[0].generation, 2);
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 13);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    for old in [11, 12] {
        v["schema_version"] = old.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&v).unwrap())
                .is_err()
        );
    }
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project.drawings.diagrams[0].generation, 2);
    assert_eq!(e.project.elements, before.elements);
}
#[test]
fn invalid_or_foreign_offsets_reject_without_mutation() {
    let mut e = fixture(0.);
    let ids = e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>();
    let before = e.project.clone();
    for offsets in [
        vec![DisplayOffset {
            element_id: 9999,
            xyz: [0; 3],
        }],
        vec![DisplayOffset {
            element_id: ids[0],
            xyz: [100001, 0, 0],
        }],
        vec![
            DisplayOffset {
                element_id: ids[0],
                xyz: [0; 3],
            },
            DisplayOffset {
                element_id: ids[0],
                xyz: [0; 3],
            },
        ],
        vec![
            DisplayOffset {
                element_id: ids[1],
                xyz: [0; 3],
            },
            DisplayOffset {
                element_id: ids[0],
                xyz: [0; 3],
            },
        ],
    ] {
        assert!(
            create(
                &mut e,
                Presentation::ExplodedAxon {
                    offsets_mm: offsets
                }
            )
            .is_err()
        );
        assert_eq!(e.project, before);
    }
    create(&mut e, Presentation::ExplodedAxon { offsets_mm: vec![] }).unwrap();
    assert!(Page::new(&e.project.drawings.diagrams[0]).is_ok());
}
#[test]
fn configuration_does_not_refresh_source_and_foreign_record_cannot_replace() {
    let mut e = fixture(0.);
    let ids = e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>();
    create(&mut e, style(&ids, 1000)).unwrap();
    let f = e.project.drawings.diagrams[0].clone();
    let mut source = e.project.elements[0].clone();
    if let Shape::Zone { spec } = &mut source.shape {
        spec.height += 0.5;
    }
    e.apply(ModelCommand::UpdateElement { element: source })
        .unwrap();
    assert_eq!(f.status(&e.project), Status::Stale);
    assert!(f.svg(&e.project).is_err());
    let geometry = e.project.elements.clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Configure {
            id: f.id,
            expected_sha256: diagrams::expected(&f),
            presentation: style(&ids, 2000),
        },
    })
    .unwrap();
    let configured = e.project.drawings.diagrams[0].clone();
    assert_eq!(configured.snapshot, f.snapshot);
    assert_eq!(configured.status(&e.project), Status::Stale);
    assert!(
        e.apply(ModelCommand::Diagram {
            edit: Edit::Refresh {
                id: f.id,
                expected_sha256: diagrams::expected(&f)
            }
        })
        .is_err()
    );
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: f.id,
            expected_sha256: diagrams::expected(&configured),
        },
    })
    .unwrap();
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Current
    );
    assert_eq!(e.project.elements, geometry);
    e.undo();
    assert_eq!(e.project.drawings.diagrams[0], configured);
    e.redo();
    assert_eq!(e.project.elements, geometry);
}
#[test]
fn local_projection_is_stable_for_large_project_coordinates() {
    let mut small = fixture(0.);
    let mut large = fixture(90000.);
    for e in [&mut small, &mut large] {
        let ids = e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>();
        create(e, style(&ids, 1000)).unwrap();
    }
    let a = Page::new(&small.project.drawings.diagrams[0]).unwrap();
    let b = Page::new(&large.project.drawings.diagrams[0]).unwrap();
    assert_eq!(a.faces.len(), b.faces.len());
    close(a.pixels_per_m, b.pixels_per_m);
    for (a, b) in a.faces.iter().zip(&b.faces) {
        assert_eq!(a.element_id, b.element_id);
        for (a, b) in a.points.iter().zip(b.points) {
            close(a[0], b[0]);
            close(a[1], b[1]);
        }
    }
}

#[test]
fn mass_axon_uses_same_checked_prism_and_explicit_xyz_offsets() {
    use atelier_bim::concept_mass::ConceptMassSpec;
    let mut p = Project::default();
    p.stories[0].elevation = 5.;
    let id = p.add(
        0,
        Material::Concrete,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: ring(10., 20., 6., 4.),
                holes: vec![ring(11., 21., 2., 1.)],
                height: 6.,
                bottom_offset: 2.,
            },
        },
    );
    let mut e = Editor::new(p);
    let original = e.project.elements.clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "Mass assembly".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::ConceptMasses,
                presentation: Presentation::ExplodedAxon {
                    offsets_mm: vec![DisplayOffset {
                        element_id: id,
                        xyz: [4000, -2000, 1000],
                    }],
                },
            },
        },
    })
    .unwrap();
    let d = &e.project.drawings.diagrams[0];
    close(d.total().0, 22.);
    close(d.total().1, 132.);
    let page = Page::new(d).unwrap();
    assert_eq!(page.origin, DVec3::new(10., 20., 7.));
    let top = project(DVec3::new(14., 18., 14.) - page.origin);
    assert!(page.faces.iter().any(|f| f.points.contains(&top)));
    assert_eq!(e.project.elements, original);
    let q = diagrams::query(&e.project, 0, 4).unwrap();
    assert_eq!(
        q["figures"][0]["definition"]["presentation"]["offsets_mm"][0]["xyz"],
        serde_json::json!([4000, -2000, 1000])
    );
    assert_eq!(q["figures"][0]["function_groups"], serde_json::json!([]));
}
