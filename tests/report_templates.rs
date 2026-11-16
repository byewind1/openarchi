use atelier_bim::{
    automation_registry::{NativeBatch, describe},
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    diagrams::{
        self, Definition, Domain, Edit, Status,
        report::{Edit as ReportEdit, Section},
    },
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};
fn fixture() -> Editor {
    let mut p = Project::default();
    for (story, width, height) in [(0, 6., 3.), (1, 2., 4.)] {
        p.add(
            story,
            Material::Timber,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: vec![
                        Point::new(0., 0.),
                        Point::new(width, 0.),
                        Point::new(width, 4.),
                        Point::new(0., 4.),
                    ],
                    holes: vec![],
                    height,
                    bottom_offset: 0.,
                },
            },
        );
    }
    let mut e = Editor::new(p);
    for story in [0, 1] {
        e.apply(ModelCommand::Diagram {
            edit: Edit::Create {
                name: format!("Floor{story}"),
                definition: Definition {
                    story,
                    option: None,
                    domain: Domain::ConceptMasses,
                    presentation: Default::default(),
                },
            },
        })
        .unwrap();
    }
    e
}
fn sections(e: &Editor) -> Vec<Section> {
    e.project
        .drawings
        .diagrams
        .iter()
        .rev()
        .map(|f| Section {
            heading: format!("Review {} & <原图>", f.name),
            figure: f.id,
        })
        .collect()
}
fn create(e: &mut Editor) {
    e.apply(ModelCommand::Diagram {
        edit: Edit::Report {
            edit: ReportEdit::Create {
                name: "院内概念汇报 & review".into(),
                sections: sections(e),
            },
        },
    })
    .unwrap();
}
#[test]
fn discovered_template_preview_shared_commit_order_native16_and_history() {
    let mut e = fixture();
    let before = e.project.clone();
    let descriptor = describe("diagram").unwrap();
    assert!(descriptor.to_string().contains("report"));
    let requested = sections(&e);
    let raw:NativeBatch=serde_json::from_value(serde_json::json!({"version":1,"edits":[{"operation":"diagram","edit":{"action":"report","edit":{"action":"create","name":"我的有序汇报","sections":requested}}}]})).unwrap();
    let prepared = e
        .prepare_batch_change(e.revision, &raw.lower(&e).unwrap())
        .unwrap();
    assert_eq!(e.project, before);
    e.commit_change("report-real-schema", &prepared).unwrap();
    let report = &e.project.drawings.reports[0];
    assert_eq!(report.sections, requested);
    assert_eq!(e.project.elements, before.elements);
    assert_eq!(e.project.drawings.diagrams, before.drawings.diagrams);
    let svg = report.svg(&e.project).unwrap();
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let pages: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("data-report-page").is_some())
        .collect();
    assert_eq!(pages.len(), 2);
    assert_eq!(
        pages[0].attribute("data-figure-id").unwrap(),
        requested[0].figure.to_string()
    );
    for page in pages {
        let viewport = page.children().find(|n| n.has_tag_name("svg")).unwrap();
        assert_eq!(viewport.attribute("viewBox"), Some("0 0 1200 800"));
        assert_eq!(viewport.attribute("width"), Some("1200"));
    }
    assert!(svg.contains("72.000 m³") && svg.contains("32.000 m³"));
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 16);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    for version in [2, 11, 14, 15] {
        value["schema_version"] = version.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project.drawings.reports[0].sections, requested);
    let query = diagrams::query(&e.project, 0, 4).unwrap();
    assert_eq!(
        query["report_templates"][0]["template"]["sections"],
        serde_json::to_value(requested).unwrap()
    );
}
#[test]
fn source_changes_missing_figures_and_explicit_local_refresh_never_silently_drop_pages() {
    let mut e = fixture();
    create(&mut e);
    let report = e.project.drawings.reports[0].clone();
    let untouched = e.project.drawings.diagrams[1].clone();
    let first = e.project.drawings.diagrams[0].clone();
    let mut changed = e.project.elements[0].clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        spec.height = 5.;
    }
    e.apply(ModelCommand::UpdateElement { element: changed })
        .unwrap();
    assert_eq!(report.status(&e.project), Status::Stale);
    assert!(report.svg(&e.project).is_err());
    e.apply(ModelCommand::Diagram {
        edit: Edit::Refresh {
            id: first.id,
            expected_sha256: diagrams::expected(&first),
        },
    })
    .unwrap();
    assert_eq!(e.project.drawings.diagrams[1], untouched);
    assert_eq!(e.project.drawings.reports[0], report);
    assert_eq!(report.status(&e.project), Status::Current);
    assert!(report.svg(&e.project).unwrap().contains("120.000 m³"));
    let current = e.project.drawings.diagrams[0].clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Delete {
            id: current.id,
            expected_sha256: diagrams::expected(&current),
        },
    })
    .unwrap();
    assert_eq!(report.status(&e.project), Status::Unavailable);
    assert!(report.svg(&e.project).is_err());
    assert_eq!(e.project.drawings.reports[0].sections.len(), 2);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
        e.project
    );
    e.undo();
    assert_eq!(report.status(&e.project), Status::Current);
}
#[test]
fn original_template_guard_and_invalid_reference_rejection_preserve_user_edit_and_history() {
    let mut e = fixture();
    create(&mut e);
    let original = e.project.drawings.reports[0].clone();
    let mut reordered = original.sections.clone();
    reordered.reverse();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Report {
            edit: ReportEdit::Update {
                id: original.id,
                expected_sha256: original.expected(),
                name: original.name.clone(),
                sections: reordered.clone(),
            },
        },
    })
    .unwrap();
    let before = e.project.clone();
    let history = e.undo.len();
    let old = ReportEdit::Update {
        id: original.id,
        expected_sha256: original.expected(),
        name: "old Agent draft".into(),
        sections: original.sections.clone(),
    };
    assert!(
        e.apply(ModelCommand::Diagram {
            edit: Edit::Report { edit: old }
        })
        .is_err()
    );
    for bad in [
        vec![],
        vec![Section {
            heading: "Missing".into(),
            figure: 999,
        }],
        vec![reordered[0].clone(), reordered[0].clone()],
        vec![Section {
            heading: "".into(),
            figure: reordered[0].figure,
        }],
    ] {
        assert!(
            e.apply(ModelCommand::Diagram {
                edit: Edit::Report {
                    edit: ReportEdit::Create {
                        name: "bad".into(),
                        sections: bad
                    }
                }
            })
            .is_err()
        );
    }
    assert_eq!(e.project, before);
    assert_eq!(e.undo.len(), history);
}
#[test]
fn long_chapter_titles_are_complete_xml_safe_and_no_template_keeps_legacy_marker() {
    let mut e = fixture();
    let legacy = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&legacy).unwrap()["schema_version"],
        11
    );
    let title = "A<&\"'中".repeat(10);
    let sections = vec![Section {
        figure: e.project.drawings.diagrams[0].id,
        heading: title.clone(),
    }];
    e.apply(ModelCommand::Diagram {
        edit: Edit::Report {
            edit: ReportEdit::Create {
                name: title.clone(),
                sections,
            },
        },
    })
    .unwrap();
    let svg = e.project.drawings.reports[0].svg(&e.project).unwrap();
    let doc = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("title"))
            .unwrap()
            .text(),
        Some(title.as_str())
    );
    let page = doc
        .descendants()
        .find(|n| n.attribute("data-report-page") == Some("1"))
        .unwrap();
    let text = page
        .children()
        .filter(|n| n.has_tag_name("text"))
        .filter_map(|n| n.text())
        .collect::<String>();
    assert_eq!(text, format!("1. {title}"));
    assert!(
        page.children().filter(|n| n.has_tag_name("text")).all(|n| n
            .attribute("y")
            .unwrap()
            .parse::<f32>()
            .unwrap()
            < 120.)
    );
    let report = e.project.drawings.reports[0].clone();
    e.apply(ModelCommand::Diagram {
        edit: Edit::Report {
            edit: ReportEdit::Delete {
                id: report.id,
                expected_sha256: report.expected(),
            },
        },
    })
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&JsonAdapter.export(&e.project).unwrap())
            .unwrap()["schema_version"],
        11
    );
}
