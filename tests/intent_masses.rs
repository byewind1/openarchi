//! Explicit typed fixture regression, not image interpretation or end-user feedback acceptance.
mod common;
use atelier_bim::{
    application_host::ApplicationHost,
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    design_process::{self as dp, Entry, intent, service},
    model::{Material, ModelAdapter, Point, Shape},
    storage,
};
fn id() -> String {
    storage::new_document_identity()
}
fn setup() -> (ApplicationHost, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("oa-mi-{}", &id()[..12]));
    std::fs::create_dir(&root).unwrap();
    let image = root.join("unscaled.png");
    image::RgbaImage::from_pixel(40, 30, image::Rgba([220, 205, 180, 255]))
        .save(&image)
        .unwrap();
    let mut p = common::synthetic_building_98();
    p.drawings
        .pages
        .push(atelier_bim::drawings::import(&image, &root.join("assets"), 1).unwrap());
    let mut host = ApplicationHost::new(p);
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let draft = service::Draft {
        name: "Existing space work".into(),
        question: "Preserve original space choices".into(),
        statements: vec![],
        candidate_names: ["Prior A".into(), "Prior B".into()],
        source_ids: host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
            .map(|e| e.id)
            .collect(),
        entry: Entry::ManualInterface,
    };
    let prepared = service::prepare_create(&host.document.editor, &id(), draft).unwrap();
    host.document
        .editor
        .commit_change(&id(), &prepared)
        .unwrap();
    (host, root)
}
fn seed(name: &str, x: f64, height: f64) -> intent::MassSeed {
    intent::MassSeed {
        name: name.into(),
        spec: ConceptMassSpec {
            arcs: vec![],
            hole_arcs: vec![],
            outline: vec![
                Point::new(x, 30.),
                Point::new(x + 5., 30.),
                Point::new(x + 5., 34.),
                Point::new(x, 34.),
            ],
            holes: vec![],
            height,
            bottom_offset: 0.,
        },
        material: Material::Plaster,
    }
}
fn draft(p: &atelier_bim::model::Project) -> intent::MassDraft {
    let page = &p.drawings.pages[0];
    intent::MassDraft {
        name: "Unscaled exterior study".into(),
        question: "Two editable envelopes, preserve existing work".into(),
        expected_result: "two editable mass candidates, not spaces or measured survey".into(),
        images: vec![intent::ImageReference {
            page: page.id,
            sha256: page.sha256.clone(),
        }],
        claims: vec![intent::Claim {
            kind: intent::ClaimKind::DesignCompletion,
            text: "Fixture scale/height/depth assumed; not AI interpretation".into(),
        }],
        geometry_assumptions: "all numeric dimensions and location are explicit design assumptions"
            .into(),
        statements: vec![],
        candidate_names: ["Envelope A".into(), "Envelope B".into()],
        candidates: [
            vec![seed("low", 30., 4.), seed("high", 34., 7.)],
            vec![seed("low", 30., 4.), seed("high", 35., 6.)],
        ],
        story: 0,
        layer: 0,
    }
}
#[test]
fn mass_tasks_preserve_old_model_have_distinct_metrics_scoped_edits_and_native9_history() {
    let (mut host, root) = setup();
    let before = host.document.editor.project.clone();
    let old_task = before.design_process.task().unwrap().clone();
    let old_compare = old_task.compare(&before).unwrap();
    let task_id = id();
    let prepared = intent::prepare_mass(&host.document.editor, &task_id, draft(&before)).unwrap();
    assert_eq!(host.document.editor.project, before);
    let result = host.confirm_task_prepared(
        host.context(),
        &task_id,
        "Mass intent candidate proposal",
        std::rc::Rc::new(prepared),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    let p = &host.document.editor.project;
    let task = p.design_process.task().unwrap().clone();
    assert!(task.is_mass());
    assert!(task.source_zone_ids.is_empty());
    assert!(
        task.candidates
            .iter()
            .all(|c| c.initial_zone_ids.is_empty() && c.initial_mass_ids.len() == 2)
    );
    assert_eq!(
        &p.elements[..before.elements.len()],
        before.elements.as_slice()
    );
    assert_eq!(old_task.compare(p).unwrap(), old_compare);
    let comparison = task.compare(p).unwrap();
    assert!(comparison.basis_current && comparison.different);
    assert_eq!(comparison.candidates[0].metrics, dp::Metrics::default());
    let a = comparison.candidates[0].mass_metrics.as_ref().unwrap();
    assert_eq!(a.masses, 2);
    assert_eq!(a.footprint_sum_m2, 40.);
    assert_eq!(a.envelope_volume_sum_m3, 220.);
    assert_eq!(
        comparison.candidates[1]
            .mass_metrics
            .as_ref()
            .unwrap()
            .envelope_volume_sum_m3,
        200.
    );
    let own = task.candidates[0].initial_mass_ids[1];
    let other = task.candidates[1].initial_mass_ids[1];
    assert_eq!(
        task.corresponding_element(p, own, task.candidates[1].option),
        Some(other)
    );
    assert!(
        host.document
            .editor
            .prepare_change(
                host.document.editor.revision,
                vec![ModelCommand::MoveElements {
                    ids: vec![other],
                    offset: Point::new(1., 0.)
                }]
            )
            .is_err()
    );
    let edit =
        service::prepare_edit(&host.document.editor, own, Point::new(0., 0.), Some(7.5)).unwrap();
    host.document.editor.commit_change(&id(), &edit).unwrap();
    let updated = task.compare(&host.document.editor.project).unwrap();
    assert_eq!(updated.candidates[1], comparison.candidates[1]);
    assert_eq!(
        updated.candidates[0]
            .mass_metrics
            .as_ref()
            .unwrap()
            .envelope_volume_sum_m3,
        230.
    );
    host.document.editor.undo();
    assert_eq!(
        task.compare(&host.document.editor.project).unwrap(),
        comparison
    );
    host.document.editor.redo();
    assert_eq!(
        task.compare(&host.document.editor.project).unwrap(),
        updated
    );
    let choice = id();
    let proposal = service::prepare_choose(
        &host.document.editor,
        &choice,
        &task_id,
        task.candidates[0].option,
        "manual fixture reason; not end-user acceptance".into(),
        updated.fingerprint,
        Entry::ManualInterface,
        "controlled manual fixture".into(),
    )
    .unwrap();
    host.document
        .editor
        .commit_change(&choice, &proposal)
        .unwrap();
    let saved = storage::JsonAdapter
        .export(&host.document.editor.project)
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(value["schema_version"], 9);
    assert_eq!(
        storage::JsonAdapter.import(&saved).unwrap(),
        host.document.editor.project
    );
    let activate = intent::prepare_activate(&host.document.editor, &old_task.id).unwrap();
    host.document
        .editor
        .commit_change(&id(), &activate)
        .unwrap();
    assert_eq!(
        host.document
            .editor
            .project
            .design_process
            .current_task
            .as_deref(),
        Some(old_task.id.as_str())
    );
    let back = intent::prepare_activate(&host.document.editor, &task_id).unwrap();
    host.document.editor.commit_change(&id(), &back).unwrap();
    assert_eq!(
        host.document
            .editor
            .project
            .design_process
            .current_choice
            .as_deref(),
        Some(choice.as_str())
    );
    // Source changes stale only the linked intent task, not a new authorization or a measured fact.
    host.document.editor.project.drawings.pages[0].sha256 = "a".repeat(64);
    assert!(
        !task
            .compare(&host.document.editor.project)
            .unwrap()
            .basis_current
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn undo_creation_keeps_mass_history_version9_and_never_reuses_declared_ids() {
    let (mut host, root) = setup();
    let before = host.document.editor.project.clone();
    let d = draft(&before);
    let prepared = intent::prepare_mass(&host.document.editor, &id(), d).unwrap();
    host.document
        .editor
        .commit_change(&id(), &prepared)
        .unwrap();
    let task = host
        .document
        .editor
        .project
        .design_process
        .task()
        .unwrap()
        .clone();
    host.document.editor.undo();
    assert_eq!(host.document.editor.project.elements, before.elements);
    assert!(
        host.document
            .editor
            .project
            .design_process
            .tasks
            .contains(&task)
    );
    let bytes = storage::JsonAdapter
        .export(&host.document.editor.project)
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 9);
    v["schema_version"] = serde_json::json!(8);
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&v).unwrap())
            .is_err()
    );
    let next = intent::prepare_mass(
        &host.document.editor,
        &id(),
        draft(&host.document.editor.project),
    )
    .unwrap();
    let next_task = next.project().design_process.task().unwrap();
    assert!(next_task.source_mass_ids[0] > *task.candidates[1].initial_mass_ids.last().unwrap());
    // Reject mixed source/member domains rather than quietly treating masses as Zones.
    let mut broken = task.clone();
    broken.source_zone_ids.push(1);
    assert!(broken.validate().is_err());
    let mut broken = task.clone();
    broken.candidates[0].initial_zone_ids.push(1);
    assert!(broken.validate().is_err());
    std::fs::remove_dir_all(root).unwrap();
}

fn update_draft(p: &atelier_bim::model::Project, target: u64) -> dp::update::Draft {
    let mut element = p.elements.iter().find(|e| e.id == target).unwrap().clone();
    match &mut element.shape {
        Shape::ConceptMass { spec } => spec.height += 0.25,
        Shape::Zone { spec } => spec.height += 0.25,
        _ => panic!("fixture domain"),
    }
    dp::update::Draft {
        instruction: "Keep the other candidate and existing villa; locally adjust this member"
            .into(),
        expected_result: "one editable geometry update with source/assumption history".into(),
        sources: vec![dp::update::Source::Drawing {
            page: p.drawings.pages[0].id,
            sha256: p.drawings.pages[0].sha256.clone(),
        }],
        claims: vec![intent::Claim {
            kind: intent::ClaimKind::DesignCompletion,
            text: "Controlled assumed height, not measurement or AI interpretation".into(),
        }],
        statements: vec![dp::Statement {
            kind: dp::StatementKind::Unknown,
            text: "Client approval is unknown".into(),
        }],
        expected_comparison: p
            .design_process
            .task()
            .unwrap()
            .compare(p)
            .unwrap()
            .fingerprint,
        elements: vec![element],
        entry: Entry::AgentProposal,
    }
}
#[test]
fn scoped_mass_update_is_atomic_keeps_history_through_undo_and_native10_roundtrip() {
    let (mut host, root) = setup();
    let tid = id();
    let prepared = intent::prepare_mass(
        &host.document.editor,
        &tid,
        draft(&host.document.editor.project),
    )
    .unwrap();
    host.document.editor.commit_change(&tid, &prepared).unwrap();
    let before = host.document.editor.project.clone();
    let task = before.design_process.task().unwrap();
    let own = task.candidates[0].initial_mass_ids[1];
    let other = task.candidates[1].option;
    let update = id();
    let prepared = dp::update::prepare(
        &host.document.editor,
        None,
        &update,
        &tid,
        update_draft(&before, own),
    )
    .unwrap();
    assert_eq!(host.document.editor.project, before);
    assert_eq!(prepared.affected_ids(), vec![own]);
    host.document
        .editor
        .commit_change(&update, &prepared)
        .unwrap();
    let after = host.document.editor.project.clone();
    let changed = before
        .elements
        .iter()
        .zip(&after.elements)
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.id)
        .collect::<Vec<_>>();
    assert_eq!(changed, vec![own]);
    assert_eq!(before.design_process.tasks, after.design_process.tasks);
    assert_eq!(
        before.design_process.decisions,
        after.design_process.decisions
    );
    let record = after.design_process.updates[0].clone();
    assert_eq!(record.id, update);
    assert_eq!(
        record.effect(&after),
        "Task update matches current elements"
    );
    assert_eq!(
        record.source_status(&after, None),
        "Task update source versions are catalogued"
    );
    assert_eq!(
        task.compare(&before).unwrap().candidates[1],
        task.compare(&after).unwrap().candidates[1]
    );
    assert!(
        after
            .elements
            .iter()
            .filter(|e| e.design_option == Some(other))
            .all(|e| before.elements.contains(e))
    );
    let bytes = storage::JsonAdapter.export(&after).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 10);
    assert_eq!(storage::JsonAdapter.import(&bytes).unwrap(), after);
    for version in 2..=9 {
        value["schema_version"] = serde_json::json!(version);
        assert!(
            storage::JsonAdapter
                .import(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
    host.document.editor.undo();
    assert_eq!(host.document.editor.project.elements, before.elements);
    assert_eq!(
        host.document.editor.project.design_process.updates,
        vec![record.clone()]
    );
    assert_eq!(
        record.effect(&host.document.editor.project),
        "Task update is not applied"
    );
    let undone = storage::JsonAdapter
        .export(&host.document.editor.project)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&undone).unwrap()["schema_version"],
        10
    );
    storage::JsonAdapter.import(&undone).unwrap();
    host.document.editor.redo();
    assert_eq!(host.document.editor.project, after);
    let mut changed = after.clone();
    changed.drawings.pages[0].sha256 = "d".repeat(64);
    assert_eq!(
        record.source_status(&changed, None),
        "Task update sources changed or are missing"
    );
    let row = dp::update::query(&after, None, 0, 1).unwrap();
    assert_eq!(row["records"][0]["record"]["id"], update);
    assert_eq!(row["next_offset"], serde_json::Value::Null);
    assert!(dp::update::query(&after, None, 0, 5).is_err());
    host.document.editor.undo();
    host.document.editor.undo();
    assert_eq!(
        record.effect(&host.document.editor.project),
        "Task update elements changed or are missing"
    );
    storage::JsonAdapter
        .import(
            &storage::JsonAdapter
                .export(&host.document.editor.project)
                .unwrap(),
        )
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn scoped_updates_reject_wrong_scope_forged_metadata_stale_and_missing_sources() {
    let (mut host, root) = setup();
    // The same service supports the existing Zone workflow, without accepting masses in a Zone task.
    let p = host.document.editor.project.clone();
    let task = p.design_process.task().unwrap();
    let target = task.candidates[0].initial_zone_ids[0];
    let tid = task.id.clone();
    let d = update_draft(&p, target);
    let okay = dp::update::prepare(&host.document.editor, None, &id(), &tid, d.clone()).unwrap();
    let record = okay
        .project()
        .design_process
        .updates
        .last()
        .unwrap()
        .clone();
    let forged = host.document.editor.prepare_change(
        host.document.editor.revision,
        vec![ModelCommand::DesignProcess {
            edit: dp::Edit::RecordUpdate {
                record: record.clone(),
            },
        }],
    );
    assert!(forged.is_err());
    let mut wrong = d.clone();
    wrong.elements[0].name = "undeclared rename".into();
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let mut wrong = d.clone();
    wrong.elements.push(wrong.elements[0].clone());
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let wrong = update_draft(&p, task.candidates[1].initial_zone_ids[0]);
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let mut wrong = d.clone();
    wrong.expected_comparison = "a".repeat(64);
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let mut wrong = d.clone();
    wrong.sources = vec![dp::update::Source::Repository {
        sha256: "a".repeat(64),
    }];
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let mut wrong = d.clone();
    wrong.sources[0] = dp::update::Source::Drawing {
        page: p.drawings.pages[0].id,
        sha256: "a".repeat(64),
    };
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let mut wrong = d.clone();
    if let Shape::Zone { spec } = &mut wrong.elements[0].shape {
        spec.height = -1.;
    }
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    let mut wrong = d.clone();
    wrong.elements[0] = p.elements.iter().find(|e| e.id == target).unwrap().clone();
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, wrong).is_err());
    // A trusted prepared transaction must not smuggle a second member outside its declared record.
    let undeclared = task.candidates[0].initial_zone_ids[1];
    let commands = vec![
        ModelCommand::UpdateElement {
            element: d.elements[0].clone(),
        },
        ModelCommand::MoveElements {
            ids: vec![undeclared],
            offset: Point::new(0.1, 0.),
        },
        ModelCommand::DesignProcess {
            edit: dp::Edit::RecordUpdate { record },
        },
    ];
    assert!(
        host.document
            .editor
            .prepare_change(host.document.editor.revision, commands)
            .is_err()
    );
    assert_eq!(host.document.editor.project, p);
    host.document
        .editor
        .project
        .edit_state
        .locked
        .insert(target);
    assert!(dp::update::prepare(&host.document.editor, None, &id(), &tid, d).is_err());
    host.document
        .editor
        .project
        .edit_state
        .locked
        .remove(&target);
    let uid = id();
    let prepared = dp::update::prepare(
        &host.document.editor,
        None,
        &uid,
        &tid,
        update_draft(&p, target),
    )
    .unwrap();
    host.document.editor.commit_change(&uid, &prepared).unwrap();
    assert_eq!(
        host.document.editor.project.design_process.updates[0]
            .effect(&host.document.editor.project),
        "Task update matches current elements"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn linked_spaces_share_mass_candidate_without_loosening_new_shape_or_other_candidate_guards() {
    use atelier_bim::mass_spaces::{self, Edit};
    let (mut host, root) = setup();
    let task_id = id();
    let prepared = intent::prepare_mass(
        &host.document.editor,
        &task_id,
        draft(&host.document.editor.project),
    )
    .unwrap();
    host.document
        .editor
        .commit_change(&id(), &prepared)
        .unwrap();
    let task = host
        .document
        .editor
        .project
        .design_process
        .task()
        .unwrap()
        .clone();
    let before = host.document.editor.project.clone();
    let comparison = task.compare(&before).unwrap();
    let a = task.candidates[0].option;
    let b = task.candidates[1].option;
    let mass = before
        .elements
        .iter()
        .find(|e| e.design_option == Some(a) && matches!(e.shape, Shape::ConceptMass { .. }))
        .unwrap()
        .clone();
    let command = ModelCommand::MassSpaces {
        edit: Edit::Create {
            mass: mass.id,
            expected_source_sha256: mass_spaces::element_stamp(&before, &mass).unwrap(),
            name: "Editable derived space".into(),
            purpose: "Unknown".into(),
            height: 3.,
            assumption: "Single concept level, explicit design assumption".into(),
        },
    };
    let proposal = host
        .document
        .editor
        .prepare_change(host.document.editor.revision, vec![command])
        .unwrap();
    host.document
        .editor
        .commit_change(&id(), &proposal)
        .unwrap();
    let linked = host.document.editor.project.clone();
    let link = linked.design_process.mass_spaces[0].clone();
    assert_eq!(
        task.compare(&linked).unwrap(),
        comparison,
        "linked Zone must not be included in mass metrics or choice snapshots"
    );
    let zone = linked
        .elements
        .iter()
        .find(|e| e.id == link.zone)
        .unwrap()
        .clone();
    let mut extra = zone.clone();
    extra.id = linked.next_id;
    assert!(
        host.document
            .editor
            .apply(ModelCommand::AddElement {
                element: extra.clone()
            })
            .is_err()
    );
    let mut forged = linked.clone();
    forged.next_id += 1;
    forged.elements.push(extra);
    host.document.editor.project = forged;
    assert!(!host.document.editor.commit_checked(linked.clone()));
    assert_eq!(host.document.editor.project, linked);
    let mut converted = mass.clone();
    converted.shape = zone.shape.clone();
    assert!(
        host.document
            .editor
            .apply(ModelCommand::UpdateElement { element: converted })
            .is_err()
    );
    let mut edited = zone.clone();
    edited.name = "User-editable linked space".into();
    host.document
        .editor
        .replace_element(edited.clone())
        .unwrap(); // existing legacy/property path
    let user = host.document.editor.project.clone();
    assert!(
        host.document
            .editor
            .apply(ModelCommand::MassSpaces {
                edit: Edit::Refresh {
                    zone: link.zone,
                    expected_sha256: mass_spaces::expected(&link),
                    height: None
                }
            })
            .is_err()
    );
    assert_eq!(host.document.editor.project, user);
    host.document
        .editor
        .apply(ModelCommand::MassSpaces {
            edit: Edit::Detach {
                zone: link.zone,
                expected_sha256: mass_spaces::expected(&link),
            },
        })
        .unwrap();
    edited.name = "Detached geometry still editable".into();
    host.document
        .editor
        .replace_element(edited.clone())
        .unwrap();
    assert_eq!(
        host.document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == edited.id),
        Some(&edited)
    );
    host.document.editor.undo();
    host.document.editor.undo();
    host.document.editor.undo();
    assert_eq!(host.document.editor.project, linked);
    let mut plan = host.document.editor.project.plan_options.clone();
    plan.active_design_option = Some(b);
    host.document
        .editor
        .apply(ModelCommand::SetOptions {
            plan: Some(plan),
            design: None,
            guide: None,
            grid: None,
        })
        .unwrap();
    let other = host.document.editor.project.clone();
    for edit in [
        Edit::Detach {
            zone: link.zone,
            expected_sha256: mass_spaces::expected(&link),
        },
        Edit::Refresh {
            zone: link.zone,
            expected_sha256: mass_spaces::expected(&link),
            height: None,
        },
    ] {
        assert!(
            host.document
                .editor
                .apply(ModelCommand::MassSpaces { edit })
                .is_err()
        );
        assert_eq!(host.document.editor.project, other);
    }
    assert!(
        host.document
            .editor
            .apply(ModelCommand::UpdateElement { element: edited })
            .is_err()
    );
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
