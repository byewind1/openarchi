//! Authored fixtures, not survey, actual Agent interpretation or native GPU acceptance.
use atelier_bim::{
    automation_registry::{NativeBatch, describe},
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    diagrams::{self, Definition, Domain, Edit, Status},
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};
fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + w, y),
        Point::new(x + w, y + h),
        Point::new(x, y + h),
    ]
}
fn fixture() -> Editor {
    let mut p = Project::default();
    p.add(
        0,
        Material::Timber,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: rect(90000., -90000., 8., 6.),
                holes: vec![rect(90002., -89998., 2., 2.)],
                height: 6.,
                bottom_offset: 0.5,
            },
        },
    );
    Editor::new(p)
}
fn create(e: &mut Editor) {
    let raw:NativeBatch=serde_json::from_value(serde_json::json!({"version":1,"edits":[{"operation":"diagram","edit":{"action":"create","name":"方案 & <A> \"Courtyard\"","definition":{"story":0,"option":null,"domain":"concept_masses"}}}]})).unwrap();
    let c = e
        .prepare_batch_change(e.revision, &raw.lower(e).unwrap())
        .unwrap();
    e.commit_change("figure-create", &c).unwrap();
}
#[test]
fn shared_registry_figure_contour_hole_units_undo_and_native11_reader() {
    let mut e = fixture();
    let before = e.project.clone();
    assert!(describe("diagram").is_some());
    create(&mut e);
    let f = &e.project.drawings.diagrams[0];
    assert_eq!(f.total(), (44., 264.));
    assert_eq!(f.status(&e.project), Status::Current);
    let svg = f.svg(&e.project).unwrap();
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(svg.contains("方案 &amp; &lt;A&gt; &quot;Courtyard&quot;"));
    assert!(svg.contains("North Unknown"));
    assert!(!svg.contains("N (declared)"));
    assert!(svg.contains("44.000 m² / 264.000 m³"));
    let path = svg
        .split("data-element-id=")
        .nth(1)
        .unwrap()
        .split("fill=")
        .next()
        .unwrap();
    assert_eq!(path.matches("M ").count(), 2);
    assert_eq!(path.matches("L ").count(), 6);
    assert!(!path.contains("90000"));
    let saved = JsonAdapter.export(&e.project).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(value["schema_version"], 11);
    assert_eq!(JsonAdapter.import(&saved).unwrap(), e.project);
    for v in 2..=10 {
        value["schema_version"] = v.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
    assert_eq!(e.project.elements, before.elements);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project.drawings.diagrams.len(), 1);
}
#[test]
fn source_changes_are_local_explicit_refresh_is_guarded_and_missing_sources_stay_readable() {
    let mut e = fixture();
    create(&mut e);
    let f = e.project.drawings.diagrams[0].clone();
    let mut other = e.project.elements[0].clone();
    other.id = 99;
    other.story = 1;
    assert!(e.project.stories.iter().any(|s| s.id == 1));
    e.apply(ModelCommand::AddElement { element: other })
        .unwrap();
    assert_eq!(f.status(&e.project), Status::Current);
    let mut changed = e.project.elements[0].clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        spec.height = 7.;
    }
    e.apply(ModelCommand::UpdateElement { element: changed })
        .unwrap();
    assert_eq!(f.status(&e.project), Status::Stale);
    assert!(f.svg(&e.project).is_err());
    let wrong = Edit::Refresh {
        id: f.id,
        expected_sha256: f.source_sha256.clone(),
    };
    assert!(e.apply(ModelCommand::Diagram { edit: wrong }).is_err());
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: f.id,
            expected_sha256: diagrams::expected(&f),
        },
    })
    .unwrap();
    let updated = e.project.drawings.diagrams[0].clone();
    assert_eq!(updated.generation, 2);
    assert_eq!(updated.id, f.id);
    assert_eq!(updated.total(), (44., 308.));
    assert_eq!(updated.status(&e.project), Status::Current);
    e.undo();
    assert_eq!(e.project.drawings.diagrams[0], f);
    e.redo();
    assert_eq!(e.project.drawings.diagrams[0], updated);
    e.apply(ModelCommand::RemoveElements {
        ids: vec![e.project.elements[0].id],
    })
    .unwrap();
    assert_eq!(updated.status(&e.project), Status::Unavailable);
    assert!(updated.svg(&e.project).is_err());
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
        e.project
    );
}
#[test]
fn exact_option_scope_never_silently_includes_other_candidates_and_tampered_snapshots_fail() {
    let mut e = fixture();
    let a = e.project.design_options.create("A".into());
    let mut duplicate = e.project.elements[0].clone();
    duplicate.id = 2;
    duplicate.design_option = Some(a);
    e.apply(ModelCommand::AddElement { element: duplicate })
        .unwrap();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "A".into(),
            definition: Definition {
                presentation: Default::default(),
                story: 0,
                option: Some(a),
                domain: Domain::ConceptMasses,
            },
        },
    })
    .unwrap();
    let f = &e.project.drawings.diagrams[0];
    assert_eq!(f.snapshot.elements.len(), 1);
    assert_eq!(f.snapshot.elements[0].id, 2);
    let mut bad = f.clone();
    bad.snapshot.elements[0].name = "tampered".into();
    assert!(bad.validate().is_err());
    e.project.design_options.options[0].name = "A renamed".into();
    assert_eq!(f.clone().status(&e.project), Status::Stale);
}

#[test]
fn public_figure_reader_bounds_and_exact_previous_record_protect_names_and_missing_scope() {
    assert!(
        serde_json::from_value::<Definition>(serde_json::json!({"story":0,"domain":"zones"}))
            .is_err()
    );
    let mut e = fixture();
    create(&mut e);
    let f = e.project.drawings.diagrams[0].clone();
    let q = diagrams::query(&e.project, 0, 1).unwrap();
    assert_eq!(q["figures"][0]["expected_sha256"], diagrams::expected(&f));
    assert_eq!(q["figures"][0]["area_m2"], 44.);
    assert!(diagrams::query(&e.project, 2, 1).is_err());
    assert!(diagrams::query(&e.project, 0, 5).is_err());
    e.apply(ModelCommand::Diagram {
        edit: Edit::Rename {
            id: f.id,
            expected_sha256: diagrams::expected(&f),
            name: "用户命名 & A".into(),
        },
    })
    .unwrap();
    let before = e.project.clone();
    assert!(
        e.apply(ModelCommand::Diagram {
            edit: Edit::Delete {
                id: f.id,
                expected_sha256: diagrams::expected(&f)
            }
        })
        .is_err()
    );
    assert_eq!(e.project, before);
    let current = e.project.drawings.diagrams[0].clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Delete {
            id: f.id,
            expected_sha256: diagrams::expected(&current),
        },
    })
    .unwrap();
    e.undo();
    assert_eq!(e.project, before);
}

#[test]
fn linked_space_quantity_figure_follows_original_assumptions_and_requires_resolved_sources() {
    use atelier_bim::mass_spaces;
    let mut e = fixture();
    create(&mut e);
    let mass_figure = e.project.drawings.diagrams[0].clone();
    let original_mass = e.project.elements[0].clone();
    e.apply(ModelCommand::MassSpaces {
        edit: mass_spaces::Edit::Create {
            mass: original_mass.id,
            expected_source_sha256: mass_spaces::element_stamp(&e.project, &original_mass).unwrap(),
            name: "Concept space".into(),
            purpose: "Unknown".into(),
            height: 3.,
            assumption: "Assumed concept height, no measured or approved use".into(),
        },
    })
    .unwrap();
    assert_eq!(mass_figure.status(&e.project), Status::Current);
    assert_eq!(e.project.drawings.diagrams[0], mass_figure);
    let link = e.project.design_process.mass_spaces[0].clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "Space quantities".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::Zones,
                presentation: Default::default(),
            },
        },
    })
    .unwrap();
    let f = e.project.drawings.diagrams.last().unwrap().clone();
    assert_eq!(f.total(), (44., 132.));
    assert_eq!(
        f.snapshot.elements.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![link.zone]
    );
    assert_eq!(
        f.snapshot.context["mass_space_links"],
        serde_json::json!([link])
    );
    assert_eq!(f.status(&e.project), Status::Current);
    assert!(
        f.svg(&e.project)
            .unwrap()
            .contains("Assumed concept height")
    );
    let mut changed = original_mass.clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        spec.height = 7.;
    }
    e.apply(ModelCommand::UpdateElement { element: changed })
        .unwrap();
    assert_eq!(f.status(&e.project), Status::Stale);
    assert_eq!(
        f.total(),
        (44., 132.),
        "source-height change must invalidate even though the Zone quantity is unchanged"
    );
    assert!(f.svg(&e.project).is_err());
    let before = e.project.clone();
    let error = e
        .apply(ModelCommand::Diagram {
            edit: Edit::Refresh {
                id: f.id,
                expected_sha256: diagrams::expected(&f),
            },
        })
        .unwrap_err();
    assert!(error.contains("Update or detach"), "{error}");
    assert_eq!(e.project, before);
    e.apply(ModelCommand::MassSpaces {
        edit: mass_spaces::Edit::Refresh {
            zone: link.zone,
            expected_sha256: mass_spaces::expected(&link),
            height: None,
        },
    })
    .unwrap();
    assert_eq!(f.status(&e.project), Status::Stale);
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: f.id,
            expected_sha256: diagrams::expected(&f),
        },
    })
    .unwrap();
    let fresh = e.project.drawings.diagrams.last().unwrap().clone();
    assert_eq!(fresh.id, f.id);
    assert_eq!(fresh.generation, f.generation + 1);
    assert_eq!(fresh.total(), f.total());
    assert_eq!(fresh.status(&e.project), Status::Current);
    e.undo();
    assert_eq!(e.project.drawings.diagrams.last().unwrap(), &f);
    e.redo();
    assert_eq!(e.project.drawings.diagrams.last().unwrap(), &fresh);
    let mut zone = e
        .project
        .elements
        .iter()
        .find(|v| v.id == link.zone)
        .unwrap()
        .clone();
    if let Shape::Zone { spec } = &mut zone.shape {
        spec.height = 2.8;
    }
    e.apply(ModelCommand::UpdateElement { element: zone })
        .unwrap();
    assert_eq!(fresh.status(&e.project), Status::Stale);
    assert!(
        e.apply(ModelCommand::Diagram {
            edit: Edit::Refresh {
                id: fresh.id,
                expected_sha256: diagrams::expected(&fresh)
            }
        })
        .unwrap_err()
        .contains("Update or detach")
    );
    let current_link = e.project.design_process.mass_spaces[0].clone();
    e.apply(ModelCommand::MassSpaces {
        edit: mass_spaces::Edit::Detach {
            zone: link.zone,
            expected_sha256: mass_spaces::expected(&current_link),
        },
    })
    .unwrap();
    // The same unsupported-model boundary protects durable operation images.
    {
        use atelier_bim::automation_journal::{Journal, OperationIdentity};
        let root = std::env::temp_dir().join(format!(
            "oa-space-figure-record-{}",
            atelier_bim::storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("operations.json");
        let identity = OperationIdentity {
            caller_id: "fixture".into(),
            project_id: "authored".into(),
            document_session: "original-session".into(),
            request_id: "space-figure".into(),
        };
        let mut journal = Journal::open(path.clone()).unwrap();
        let record = journal
            .begin(
                identity,
                "authored-input".into(),
                &fixture().project,
                &e.project,
            )
            .unwrap();
        assert!(record.mass_space_figure_sources);
        drop(journal);
        let original = std::fs::read(&path).unwrap();
        assert!(
            serde_json::to_value(Journal::open(path.clone()).unwrap().records().unwrap())
                .unwrap()
                .to_string()
                .contains("mass_space_figure_sources")
        );
        let mut wire: serde_json::Value = serde_json::from_slice(&original).unwrap();
        for record in wire["records"].as_object_mut().unwrap().values_mut() {
            record
                .as_object_mut()
                .unwrap()
                .remove("mass_space_figure_sources");
        }
        std::fs::write(&path, serde_json::to_vec(&wire).unwrap()).unwrap();
        assert!(
            matches!(Journal::open(path.clone()),Err(error) if error=="JOURNAL_UNSUPPORTED_MODEL_FEATURE")
        );
        std::fs::write(&path, &original).unwrap();
        drop(Journal::open(path).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    // Cached source assumptions require18 even after the live relation is removed.
    let saved = JsonAdapter.export(&e.project).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(value["schema_version"], 18);
    assert_eq!(JsonAdapter.import(&saved).unwrap(), e.project);
    value["schema_version"] = 17.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    value["schema_version"] = 11.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: fresh.id,
            expected_sha256: diagrams::expected(&fresh),
        },
    })
    .unwrap();
    let detached = e.project.drawings.diagrams.last().unwrap();
    assert!(detached.snapshot.context.get("mass_space_links").is_none());
    assert!((detached.total().1 - 123.2).abs() < 1e-8);
    assert_eq!(detached.status(&e.project), Status::Current);
}
