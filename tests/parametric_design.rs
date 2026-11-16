//! Bounded rule regression. Synthetic task data is not real-project/AI/native acceptance.
mod common;
use atelier_bim::{
    application_host::ApplicationHost,
    design_process::{
        parametric::{self, Definition, Parameters},
        service,
    },
    model::{ModelAdapter, Point},
    storage::{self, JsonAdapter},
};
fn id() -> String {
    storage::new_document_identity()
}
fn params() -> Parameters {
    Parameters {
        origin_mm: [30000, 30000],
        court_width_mm: 20000,
        court_depth_mm: 14000,
        wing_depth_mm: 6000,
        storeys: 2,
        floor_height_mm: 3300,
    }
}
#[test]
fn rule_shared_transactions_parameters_roundtrip_manual_protection_and_undo() {
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let root = std::env::temp_dir().join(format!("oa-param-{}", id()));
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let before = host.document.editor.project.clone();
    let a = params();
    let mut b = a.clone();
    b.court_width_mm -= 2000;
    let def = Definition {
        rule_version: 1,
        candidates: [a.clone(), b],
        site_sha256: atelier_bim::automation_journal::canonical_hash(&before.site).unwrap(),
        sources: vec![],
        assumptions: "All dimensions are authored concept assumptions, no survey or approval."
            .into(),
    };
    let task_id = id();
    let change = parametric::prepare(
        &host.document.editor,
        &task_id,
        "Two-wing study".into(),
        "Keep original work, compare two envelopes".into(),
        def,
        0,
        0,
    )
    .unwrap();
    assert_eq!(host.document.editor.project, before);
    let result = host.confirm_task_prepared(
        host.context(),
        &task_id,
        "Two-wing candidates",
        std::rc::Rc::new(change),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    let initial = host.document.editor.project.clone();
    assert_eq!(
        &initial.elements[..before.elements.len()],
        before.elements.as_slice()
    );
    let task = initial.design_process.task().unwrap().clone();
    let option = task.candidates[0].option;
    let metrics = task.compare(&initial).unwrap().candidates[0]
        .mass_metrics
        .clone()
        .unwrap();
    assert_eq!(metrics.footprint_sum_m2, 240.);
    assert_eq!(metrics.envelope_volume_sum_m3, 1584.);
    let mut changed = a.clone();
    changed.wing_depth_mm = 8000;
    let update_id = id();
    let update = parametric::prepare_update(
        &host.document.editor,
        &update_id,
        &task_id,
        changed.clone(),
        "Widen both wings to 8m".into(),
    )
    .unwrap();
    let r = host.confirm_task_prepared(
        host.context(),
        &update_id,
        "Adjust two wings",
        std::rc::Rc::new(update),
    );
    assert_eq!(r["result"]["status"], "committed", "{r}");
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        changed
    );
    assert_eq!(
        task.compare(&host.document.editor.project)
            .unwrap()
            .candidates[0]
            .mass_metrics
            .as_ref()
            .unwrap()
            .footprint_sum_m2,
        336.
    );
    let bytes = JsonAdapter.export(&host.document.editor.project).unwrap();
    let mut raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(raw["schema_version"], 28);
    let read = JsonAdapter.import(&bytes).unwrap();
    assert_eq!(read, host.document.editor.project);
    raw["schema_version"] = 27.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&raw).unwrap())
            .is_err()
    );
    host.document.editor.undo();
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        a
    );
    host.document.editor.redo();
    let member = task.candidates[0].initial_mass_ids[0];
    let manual =
        service::prepare_edit(&host.document.editor, member, Point::new(1., 0.), None).unwrap();
    host.document.editor.commit_change(&id(), &manual).unwrap();
    assert!(
        parametric::prepare_update(
            &host.document.editor,
            &id(),
            &task_id,
            params(),
            "Regenerate".into()
        )
        .err()
        .unwrap()
        .contains("Manual changes")
    );
    host.document.editor.undo();
    assert!(
        parametric::prepare_update(
            &host.document.editor,
            &id(),
            &task_id,
            params(),
            "Return initial dimensions".into()
        )
        .is_ok()
    );
    // Existing explicit locks survive the exact same native storage route.
    let mut locked = host.document.editor.project.clone();
    locked.edit_state.locked.insert(member);
    let locked = JsonAdapter
        .import(&JsonAdapter.export(&locked).unwrap())
        .unwrap();
    let locked_editor = atelier_bim::editor::Editor::new(locked);
    assert!(
        parametric::prepare_update(
            &locked_editor,
            &id(),
            &task_id,
            params(),
            "Respect the saved lock".into()
        )
        .is_err()
    );
    // A corrupted declared parameter history must not become regeneration authority.
    let mut forged = host.document.editor.project.clone();
    forged
        .design_process
        .updates
        .last_mut()
        .unwrap()
        .parametric_parameters
        .as_mut()
        .unwrap()
        .court_width_mm += 1000;
    assert!(parametric::current_parameters(&forged, &task_id, option).is_err());
    // Source change check is tested against a read-only copy; no original is rewritten.
    let mut stale = host.document.editor.project.clone();
    stale.site.control_tolerance_m = Some(0.02);
    assert!(
        stale
            .design_process
            .task()
            .unwrap()
            .intent
            .as_ref()
            .unwrap()
            .parametric
            .as_ref()
            .unwrap()
            .check_inputs(&stale)
            .is_err()
    );
}

#[test]
fn same_envelope_parameters_are_a_real_undoable_saved_version() {
    let mut host = ApplicationHost::new(atelier_bim::model::Project::default());
    let root = std::env::temp_dir().join(format!("oa-param-state-{}", id()));
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let a = params();
    let mut b = a.clone();
    b.court_width_mm -= 2000;
    let def = Definition {
        rule_version: 1,
        candidates: [a.clone(), b.clone()],
        site_sha256: atelier_bim::automation_journal::canonical_hash(
            &host.document.editor.project.site,
        )
        .unwrap(),
        sources: vec![],
        assumptions: "Explicit concept fixture, not measured or code approved.".into(),
    };
    let task_id = id();
    let change = parametric::prepare(
        &host.document.editor,
        &task_id,
        "Same envelope study".into(),
        "Change storeys without changing the authored envelope".into(),
        def,
        0,
        0,
    )
    .unwrap();
    let r = host.confirm_task_prepared(
        host.context(),
        &task_id,
        "Two wings",
        std::rc::Rc::new(change),
    );
    assert_eq!(r["result"]["status"], "committed", "{r}");
    let initial = host.document.editor.project.clone();
    let task = initial.design_process.task().unwrap().clone();
    let option = task.candidates[0].option;
    let before_fingerprint = task.compare(&initial).unwrap().fingerprint;
    let discussion = atelier_bim::evidence::topics::capture(
        &initial,
        &[atelier_bim::evidence::Target::Element {
            id: task.candidates[0].initial_mass_ids[0],
        }],
        "Review this wing and its current concept storeys".into(),
    )
    .unwrap();
    let mut changed = a.clone();
    changed.storeys = 3;
    changed.floor_height_mm = 2200;
    assert_eq!(changed.geometry().unwrap(), a.geometry().unwrap());
    let update_id = id();
    let prepared = parametric::prepare_update(
        &host.document.editor,
        &update_id,
        &task_id,
        changed.clone(),
        "Three 2.2m concept storeys in the same 6.6m envelope".into(),
    );
    assert!(
        prepared.is_ok(),
        "Same envelope must still save parameters: {}",
        prepared.err().unwrap()
    );
    let r = host.confirm_task_prepared(
        host.context(),
        &update_id,
        "Parameter-only update",
        std::rc::Rc::new(prepared.unwrap()),
    );
    assert_eq!(r["result"]["status"], "committed", "{r}");
    assert_eq!(host.document.editor.project.elements, initial.elements);
    assert_eq!(
        discussion.objects[0]
            .link
            .status(&host.document.editor.project),
        atelier_bim::evidence::LinkStatus::Unchanged
    );
    assert_eq!(
        discussion.scope.input_status(&host.document.editor.project),
        Some("Discussion design inputs changed; old discussion retained")
    );
    assert!(
        host.document
            .editor
            .prepare_change(
                host.document.editor.revision,
                vec![atelier_bim::command::ModelCommand::Evidence {
                    edit: atelier_bim::evidence::EvidenceEdit::RecordTopic {
                        id: id(),
                        exchange: discussion.clone(),
                        expected_previous: None
                    }
                }]
            )
            .is_err()
    );
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        changed
    );
    assert_eq!(
        parametric::current_parameters(
            &host.document.editor.project,
            &task_id,
            task.candidates[1].option
        )
        .unwrap(),
        b
    );
    assert_ne!(
        task.compare(&host.document.editor.project)
            .unwrap()
            .fingerprint,
        before_fingerprint
    );
    let read = JsonAdapter
        .import(&JsonAdapter.export(&host.document.editor.project).unwrap())
        .unwrap();
    assert_eq!(read, host.document.editor.project);
    // Raw typed RecordUpdate must use the same lock guard, even with no geometry commands.
    let mut locked = initial.clone();
    locked
        .edit_state
        .locked
        .insert(task.candidates[0].initial_mass_ids[0]);
    let e = atelier_bim::editor::Editor::new(locked);
    let record = host
        .document
        .editor
        .project
        .design_process
        .updates
        .last()
        .unwrap()
        .clone();
    assert!(
        e.prepare_change(
            e.revision,
            vec![atelier_bim::command::ModelCommand::DesignProcess {
                edit: atelier_bim::design_process::Edit::RecordUpdate { record }
            }]
        )
        .is_err()
    );
    host.document.editor.undo();
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        a
    );
    assert_eq!(host.document.editor.project.design_process.updates.len(), 1);
    let read = JsonAdapter
        .import(&JsonAdapter.export(&host.document.editor.project).unwrap())
        .unwrap();
    assert_eq!(
        parametric::current_parameters(&read, &task_id, option).unwrap(),
        a
    );
    host.document.editor.redo();
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        changed
    );
    let mut moved = changed.clone();
    moved.origin_mm[0] += 1000;
    let update_id = id();
    let change = parametric::prepare_update(
        &host.document.editor,
        &update_id,
        &task_id,
        moved,
        "Move later".into(),
    )
    .unwrap();
    let r = host.confirm_task_prepared(
        host.context(),
        &update_id,
        "Later geometry",
        std::rc::Rc::new(change),
    );
    assert_eq!(r["result"]["status"], "committed");
    host.document.editor.undo();
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        changed
    );
    host.document.editor.undo();
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task_id, option).unwrap(),
        a
    );
}

#[test]
fn legacy_parameter_geometry_resolves_only_unique_versions_and_rejects_forged_state() {
    let mut editor = atelier_bim::editor::Editor::new(atelier_bim::model::Project::default());
    let a = params();
    let mut b = a.clone();
    b.court_width_mm -= 2000;
    let def = Definition {
        rule_version: 1,
        candidates: [a.clone(), b],
        site_sha256: atelier_bim::automation_journal::canonical_hash(&editor.project.site).unwrap(),
        sources: vec![],
        assumptions: "Declared fixture only".into(),
    };
    let task = id();
    let prepared = parametric::prepare(
        &editor,
        &task,
        "Legacy study".into(),
        "Preserve ambiguity rather than guess".into(),
        def,
        0,
        0,
    )
    .unwrap();
    editor.commit_change(&id(), &prepared).unwrap();
    let option = editor.project.design_process.task().unwrap().candidates[0].option;
    let mut changed = a.clone();
    changed.wing_depth_mm = 8000;
    changed.storeys = 3;
    changed.floor_height_mm = 2200;
    let prepared = parametric::prepare_update(
        &editor,
        &id(),
        &task,
        changed.clone(),
        "Geometric and parameter update".into(),
    )
    .unwrap();
    editor.commit_change(&id(), &prepared).unwrap();
    let mut raw: serde_json::Value =
        serde_json::from_slice(&JsonAdapter.export(&editor.project).unwrap()).unwrap();
    raw["schema_version"] = 27.into();
    raw["design_process"]
        .as_object_mut()
        .unwrap()
        .remove("parametric_states");
    let unique = JsonAdapter
        .import(&serde_json::to_vec(&raw).unwrap())
        .unwrap();
    assert_eq!(unique.elements, editor.project.elements);
    assert_eq!(
        unique.design_process.updates,
        editor.project.design_process.updates
    );
    assert_eq!(
        parametric::current_parameters(&unique, &task, option).unwrap(),
        changed
    );
    changed.wing_depth_mm = a.wing_depth_mm;
    let prepared = parametric::prepare_update(
        &editor,
        &id(),
        &task,
        changed,
        "Return to the original envelope, keep changed storeys".into(),
    )
    .unwrap();
    editor.commit_change(&id(), &prepared).unwrap();
    let mut raw: serde_json::Value =
        serde_json::from_slice(&JsonAdapter.export(&editor.project).unwrap()).unwrap();
    let mut forged = raw.clone();
    forged["design_process"]["parametric_states"][option.to_string()] =
        serde_json::json!({"state":"update","id":"f".repeat(64)});
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&forged).unwrap())
            .is_err()
    );
    let mut downgraded = raw.clone();
    downgraded["schema_version"] = 27.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&downgraded).unwrap())
            .is_err()
    );
    raw["schema_version"] = 27.into();
    raw["design_process"]
        .as_object_mut()
        .unwrap()
        .remove("parametric_states");
    let ambiguous = JsonAdapter
        .import(&serde_json::to_vec(&raw).unwrap())
        .unwrap();
    assert_eq!(ambiguous.elements, editor.project.elements);
    assert_eq!(
        ambiguous.design_process.updates,
        editor.project.design_process.updates
    );
    assert!(
        parametric::current_parameters(&ambiguous, &task, option)
            .err()
            .unwrap()
            .contains("uniquely")
    );
    assert_eq!(parametric::readback(&ambiguous)[0]["status"], "unknown");
    let again = JsonAdapter
        .import(&JsonAdapter.export(&ambiguous).unwrap())
        .unwrap();
    assert_eq!(again, ambiguous);
}
