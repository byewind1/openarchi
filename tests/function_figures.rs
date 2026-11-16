//! Exact authored purpose expression, not automatic use classification or approval.
use atelier_bim::{
    automation_registry::NativeBatch,
    command::ModelCommand,
    diagrams::{self, Definition, Domain, Edit, Presentation, Status},
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
    zones::ZoneSpec,
};
fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + w, y),
        Point::new(x + w, y + h),
        Point::new(x, y + h),
    ]
}
fn zone(purpose: &str, x: f64, w: f64, h: f64) -> ZoneSpec {
    ZoneSpec {
        surface: Default::default(),
        purpose: purpose.into(),
        outline: rect(x, 0., w, h),
        holes: vec![],
        height: 3.,
        bottom_offset: 0.,
        label_position: None,
    }
}
fn fixture() -> Editor {
    let mut p = Project::default();
    let mut courtyard = zone("living", 0., 8., 6.);
    courtyard.holes = vec![rect(2., 2., 2., 2.)];
    p.add(0, Material::Plaster, Shape::Zone { spec: courtyard });
    p.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: zone("living", 10., 4., 5.),
        },
    );
    p.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: zone("待定用途", 16., 3., 4.),
        },
    );
    let option = p.design_options.create("B".into());
    let id = p.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: zone("living", 20., 3., 4.),
        },
    );
    p.elements
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
        .design_option = Some(option);
    Editor::new(p)
}
fn create(e: &mut Editor, option: Option<u32>) -> u64 {
    let batch:NativeBatch=serde_json::from_value(serde_json::json!({"version":1,"edits":[{"operation":"diagram","edit":{"action":"create","name":"功能 & courtyard","definition":{"story":0,"option":option,"domain":"zones","presentation":{"type":"zone_functions","unknown_purposes":[]}}}}]})).unwrap();
    let c = e
        .prepare_batch_change(e.revision, &batch.lower(e).unwrap())
        .unwrap();
    e.commit_change(&format!("function-{}", e.revision), &c)
        .unwrap();
    e.project.drawings.diagrams.last().unwrap().id
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a}!={b}")
}
#[test]
fn exact_groups_keep_hole_area_members_same_candidate_colours_and_printable_svg() {
    let mut e = fixture();
    let original = e.project.elements.clone();
    let a = create(&mut e, None);
    let b = create(&mut e, Some(1));
    let fa = e
        .project
        .drawings
        .diagrams
        .iter()
        .find(|f| f.id == a)
        .unwrap();
    let fb = e
        .project
        .drawings
        .diagrams
        .iter()
        .find(|f| f.id == b)
        .unwrap();
    let groups = fa.function_groups();
    assert_eq!(groups.len(), 2);
    let living = groups.iter().find(|g| g.purpose == "living").unwrap();
    assert_eq!(living.element_ids, vec![1, 2]);
    close(living.area_m2, 64.);
    close(living.volume_m3, 192.);
    assert!(!living.unknown);
    assert_eq!(living.key, fb.function_groups()[0].key);
    assert_eq!(living.color, fb.function_groups()[0].color);
    assert_ne!(fa.total(), fb.total());
    let svg = fa.svg(&e.project).unwrap();
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        xml.descendants()
            .filter(|n| n.has_tag_name("path") && n.attribute("data-element-id").is_some())
            .count(),
        3
    );
    let metadata: serde_json::Value = serde_json::from_str(
        xml.descendants()
            .find(|n| n.has_tag_name("metadata"))
            .unwrap()
            .text()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["function_groups"][0]["area_m2"], 64.);
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(svg.contains("no approval verdict"));
    assert_eq!(e.project.elements, original);
}
#[test]
fn unknown_annotation_is_only_diagram_configuration_native12_is_required_and_exact_record_guarded()
{
    let mut e = fixture();
    create(&mut e, None);
    let before = e.project.clone();
    let f = before.drawings.diagrams[0].clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Configure {
            id: f.id,
            expected_sha256: diagrams::expected(&f),
            presentation: Presentation::ZoneFunctions {
                unknown_purposes: vec!["待定用途".into()],
            },
        },
    })
    .unwrap();
    let updated = &e.project.drawings.diagrams[0];
    assert_eq!(updated.generation, 2);
    assert_eq!(e.project.elements, before.elements);
    let groups = updated.function_groups();
    let unknown = groups.iter().find(|g| g.unknown).unwrap();
    assert_eq!(unknown.purpose, "待定用途");
    assert_eq!(unknown.color, [160, 160, 160]);
    close(unknown.area_m2, 12.);
    assert!(groups.iter().find(|g| g.purpose == "living").unwrap().color != unknown.color);
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::Diagram {
                edit: Edit::Delete {
                    id: f.id,
                    expected_sha256: diagrams::expected(&f)
                }
            }]
        )
        .is_err()
    );
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["schema_version"], 12);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    json["schema_version"] = 11.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&json).unwrap())
            .is_err()
    );
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project.drawings.diagrams[0].generation, 2);
}
#[test]
fn changing_purpose_stales_source_and_annotation_does_not_silently_refresh_it() {
    let mut e = fixture();
    create(&mut e, None);
    let f = e.project.drawings.diagrams[0].clone();
    let mut changed = e.project.elements[0].clone();
    if let Shape::Zone { spec } = &mut changed.shape {
        spec.purpose = "kitchen".into();
    }
    e.apply(ModelCommand::UpdateElement { element: changed })
        .unwrap();
    assert_eq!(f.status(&e.project), Status::Stale);
    assert!(f.svg(&e.project).is_err());
    e.apply(ModelCommand::Diagram {
        edit: Edit::Configure {
            id: f.id,
            expected_sha256: diagrams::expected(&f),
            presentation: Presentation::ZoneFunctions {
                unknown_purposes: vec!["living".into()],
            },
        },
    })
    .unwrap();
    let annotation = e.project.drawings.diagrams[0].clone();
    assert_eq!(annotation.status(&e.project), Status::Stale);
    assert_eq!(annotation.function_groups()[0].element_ids, vec![1, 2]);
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: f.id,
            expected_sha256: diagrams::expected(&annotation),
        },
    })
    .unwrap();
    let current = e.project.drawings.diagrams[0].clone();
    assert_eq!(current.id, f.id);
    assert_eq!(current.generation, 3);
    assert_eq!(current.status(&e.project), Status::Current);
    let groups = current.function_groups();
    assert_eq!(
        groups
            .iter()
            .find(|g| g.purpose == "kitchen")
            .unwrap()
            .element_ids,
        vec![1]
    );
    assert_eq!(
        groups
            .iter()
            .find(|g| g.purpose == "living")
            .unwrap()
            .element_ids,
        vec![2]
    );
    close(groups.iter().map(|g| g.area_m2).sum(), 76.);
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(e.project.drawings.diagrams[0], f);
    assert_eq!(f.status(&e.project), Status::Current);
    e.redo();
    e.redo();
    e.redo();
    assert_eq!(e.project.drawings.diagrams[0], current);
}
#[test]
fn unsupported_mass_duplicate_unknowns_and_nonprinting_labels_are_rejected_without_mutation() {
    let mut e = fixture();
    let before = e.project.clone();
    for domain in [Domain::ConceptMasses, Domain::Zones] {
        let d = Definition {
            story: 0,
            option: None,
            domain,
            presentation: Presentation::ZoneFunctions {
                unknown_purposes: if domain == Domain::Zones {
                    vec!["living".into(), "living".into()]
                } else {
                    vec![]
                },
            },
        };
        assert!(
            e.apply(ModelCommand::Diagram {
                edit: Edit::Create {
                    name: "invalid".into(),
                    definition: d
                }
            })
            .is_err()
        );
        assert_eq!(e.project, before);
    }
    let mut element = e.project.elements[0].clone();
    if let Shape::Zone { spec } = &mut element.shape {
        spec.purpose = "bad\u{1}label".into();
    }
    e.apply(ModelCommand::UpdateElement { element }).unwrap();
    let before = e.project.clone();
    assert!(
        e.apply(ModelCommand::Diagram {
            edit: Edit::Create {
                name: "invalid".into(),
                definition: Definition {
                    story: 0,
                    option: None,
                    domain: Domain::Zones,
                    presentation: Presentation::ZoneFunctions {
                        unknown_purposes: vec![]
                    }
                }
            }
        })
        .is_err()
    );
    assert_eq!(e.project, before);
}
#[test]
fn compatible_quantity_definition_stays_native11_and_unchanged_source_hash() {
    let mut e = fixture();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "quantity".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::Zones,
                presentation: Presentation::ModelQuantities,
            },
        },
    })
    .unwrap();
    let original = e.project.drawings.diagrams[0].clone();
    let bytes = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        11
    );
    assert!(
        !serde_json::to_string(&original.definition)
            .unwrap()
            .contains("presentation")
    );
    e.apply(ModelCommand::Diagram {
        edit: Edit::Configure {
            id: original.id,
            expected_sha256: diagrams::expected(&original),
            presentation: Presentation::ZoneFunctions {
                unknown_purposes: vec![],
            },
        },
    })
    .unwrap();
    let styled = e.project.drawings.diagrams[0].clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Configure {
            id: styled.id,
            expected_sha256: diagrams::expected(&styled),
            presentation: Presentation::ModelQuantities,
        },
    })
    .unwrap();
    let restored = &e.project.drawings.diagrams[0];
    assert_eq!(restored.source_sha256, original.source_sha256);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
        e.project
    );
}

#[test]
fn built_in_labels_have_distinct_styles_without_aliasing_or_classification() {
    let mut p = Project::default();
    let labels = [
        "living",
        "dining",
        "kitchen",
        "circulation",
        "stair_hall",
        "bedroom",
        "bathroom",
        "Living",
        " living ",
    ];
    for (i, label) in labels.iter().enumerate() {
        p.add(
            0,
            Material::Plaster,
            Shape::Zone {
                spec: zone(label, i as f64 * 4., 3., 3.),
            },
        );
    }
    let mut e = Editor::new(p);
    create(&mut e, None);
    let f = &e.project.drawings.diagrams[0];
    let groups = f.function_groups();
    assert_eq!(groups.len(), labels.len());
    let mut colors = std::collections::BTreeSet::new();
    for label in &labels[..7] {
        let g = groups.iter().find(|g| &g.purpose == label).unwrap();
        assert!(
            colors.insert(g.color),
            "built-in purpose labels must remain visually distinct"
        );
        assert!(!g.unknown);
    }
    assert_ne!(
        groups.iter().find(|g| g.purpose == "living").unwrap().key,
        groups.iter().find(|g| g.purpose == "Living").unwrap().key
    );
}
