//! Public synthetic requirement/version regressions, not real-site or AI acceptance.
mod common;
use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    command::ModelCommand,
    design_process::{
        parametric::{self, Definition, Parameters},
        service,
    },
    evidence::{Link, Target},
    model::{ModelAdapter, Point},
    program::{AreaBasis, AreaUnit, Mapping, ProgramEdit, workbook},
    storage::{self, JsonAdapter},
};
fn id() -> String {
    storage::new_document_identity()
}
fn fixture() -> (ApplicationHost, String, u32) {
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    host.configure_operation_journal(Some(
        std::env::temp_dir().join(format!("oa-reviewed-{}", id())),
    ))
    .unwrap();
    let mapping = Mapping {
        sheet: "CSV".into(),
        first_row: 2,
        last_row: 2,
        key_column: 1,
        label_column: 2,
        area_column: None,
        quantity_column: None,
        scope_column: None,
        note_column: None,
        area_unit: AreaUnit::SquareMetre,
        area_basis: AreaBasis::RowTotal,
    };
    let proposal = workbook::map(
        &workbook::inspect(b"ID,Name\nTEACH,Teaching wing\n", "csv").unwrap(),
        &"a".repeat(64),
        "brief".into(),
        "Declared concept requirements".into(),
        mapping,
        None,
    )
    .unwrap();
    host.document
        .editor
        .apply(ModelCommand::Program {
            edit: ProgramEdit::Import { proposal },
        })
        .unwrap();
    let target = Target::ProgramRow {
        sheet_id: "brief".into(),
        key: "TEACH".into(),
    };
    let p = &host.document.editor.project;
    let a = Parameters {
        origin_mm: [30000, 30000],
        court_width_mm: 20000,
        court_depth_mm: 14000,
        wing_depth_mm: 6000,
        storeys: 2,
        floor_height_mm: 3300,
    };
    let mut b = a.clone();
    b.wing_depth_mm = 7000;
    let definition = Definition {
        rule_version: 1,
        candidates: [a, b],
        site_sha256: atelier_bim::automation_journal::canonical_hash(&p.site).unwrap(),
        sources: vec![Link {
            baseline_sha256: target.current_hash(p).unwrap(),
            target,
        }],
        assumptions: "Concept only: requirement areas, capacity and approval remain unknown".into(),
    };
    let task = id();
    let change = parametric::prepare(
        &host.document.editor,
        &task,
        "Teaching courtyard".into(),
        "Explore shallow wings, retain existing work".into(),
        definition,
        0,
        0,
    )
    .unwrap();
    let response = host.confirm_task_prepared(
        host.context(),
        &task,
        "Create two rule candidates",
        std::rc::Rc::new(change),
    );
    assert_eq!(response["result"]["status"], "committed", "{response}");
    let option = host
        .document
        .editor
        .project
        .plan_options
        .active_design_option
        .unwrap();
    (host, task, option)
}
fn change_requirement(host: &mut ApplicationHost, note: &str) {
    let old = host.document.editor.project.program.sheets[0].rows[0]
        .values
        .clone();
    let mut values = old.clone();
    values.note = note.into();
    host.dispatch(
        host.context(),
        HostCommand::Apply {
            request_id: id(),
            commands: vec![ModelCommand::Program {
                edit: ProgramEdit::UpdateRow {
                    sheet_id: "brief".into(),
                    key: "TEACH".into(),
                    expected: old,
                    values,
                },
            }],
        },
    )
    .unwrap();
}
#[test]
fn reviewed_requirement_versions_limit_update_keep_history_undo_and_reopen() {
    let (mut host, task, option) = fixture();
    let initial = host.document.editor.project.clone();
    let rule = initial
        .design_process
        .task()
        .unwrap()
        .intent
        .as_ref()
        .unwrap()
        .parametric
        .clone()
        .unwrap();
    change_requirement(
        &mut host,
        "Authored change: test wing depth 6.4m; no measured or statutory area inferred",
    );
    let changed = host.document.editor.project.clone();
    let mut next = parametric::current_parameters(&changed, &task, option).unwrap();
    next.wing_depth_mm = 6400;
    assert!(
        parametric::prepare_update(
            &host.document.editor,
            &id(),
            &task,
            next.clone(),
            "Apply current brief locally".into()
        )
        .err()
        .unwrap()
        .contains("inputs changed")
    );
    let inputs = parametric::capture_inputs(&changed, &task, option).unwrap();
    assert_ne!(inputs, rule.inputs());
    assert_eq!(inputs.sources[0].target, rule.sources[0].target);
    let update = id();
    let prepared = parametric::prepare_reviewed_update(
        &host.document.editor,
        &update,
        &task,
        next.clone(),
        "Use the reviewed teaching requirement only for A".into(),
        inputs.clone(),
    )
    .unwrap();
    assert_eq!(
        host.document.editor.project, changed,
        "preview/cancel is nonmutating"
    );
    let answer = host.confirm_task_prepared(
        host.context(),
        &update,
        "Review input version and update A",
        std::rc::Rc::new(prepared),
    );
    assert_eq!(answer["result"]["status"], "committed", "{answer}");
    let after = host.document.editor.project.clone();
    let a = &after.design_process.task().unwrap().candidates[0];
    for old in &changed.elements {
        if !a.initial_mass_ids.contains(&old.id) {
            assert_eq!(after.elements.iter().find(|e| e.id == old.id), Some(old));
        }
    }
    assert_eq!(after.program, changed.program);
    assert_eq!(after.design_process.tasks, initial.design_process.tasks);
    assert_eq!(after.evidence, changed.evidence);
    assert_eq!(
        parametric::current_inputs(&after, &task, option).unwrap(),
        inputs
    );
    assert_eq!(
        parametric::current_inputs(
            &after,
            &task,
            initial.design_process.task().unwrap().candidates[1].option
        )
        .unwrap(),
        rule.inputs()
    );
    let readback = parametric::readback(&after);
    assert_eq!(readback[0]["input_status"], "current");
    assert_eq!(readback[1]["input_status"], "changed_or_missing");
    assert!(
        after
            .design_process
            .task()
            .unwrap()
            .compare(&after)
            .unwrap()
            .candidates[0]
            .parametric_inputs
            .is_some()
    );
    let bytes = JsonAdapter.export(&after).unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["schema_version"], 32);
    for old in [27, 28, 29, 30, 31] {
        wire["schema_version"] = old.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&wire).unwrap())
                .is_err()
        );
    }
    let root = std::env::temp_dir().join(format!("oa-reviewed-save-{}", id()));
    std::fs::create_dir(&root).unwrap();
    let file = root.join("reviewed.openarchi.json");
    storage::save_document(&after, &file, host.document.editor.project_identity()).unwrap();
    let reopened = atelier_bim::editor::Editor::from_stored_document(&file).unwrap();
    assert_eq!(reopened.project.elements, after.elements);
    assert_eq!(
        parametric::current_inputs(&reopened.project, &task, option).unwrap(),
        inputs
    );
    let mut more = next.clone();
    more.court_width_mm += 500;
    assert!(
        parametric::prepare_update(
            &reopened,
            &id(),
            &task,
            more,
            "Continue with the same reviewed inputs".into()
        )
        .is_ok()
    );
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project.elements, changed.elements);
    assert_eq!(
        parametric::current_inputs(&host.document.editor.project, &task, option).unwrap(),
        rule.inputs()
    );
    assert_eq!(
        host.document.editor.project.design_process.updates.len(),
        after.design_process.updates.len(),
        "immutable attempt survives Undo"
    );
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(
        parametric::current_parameters(&host.document.editor.project, &task, option).unwrap(),
        next
    );
    assert_eq!(
        parametric::current_inputs(&host.document.editor.project, &task, option).unwrap(),
        inputs
    );
    let activate = service::prepare_activate(
        &host.document.editor,
        Some(initial.design_process.task().unwrap().candidates[1].option),
    )
    .unwrap();
    host.commit_prepared(host.context(), &id(), &activate)
        .unwrap();
    assert!(
        parametric::current_inputs(
            &host.document.editor.project,
            &task,
            initial.design_process.task().unwrap().candidates[1].option
        )
        .unwrap()
        .check_current(&host.document.editor.project)
        .is_err()
    );
}
#[test]
fn reviewed_inputs_never_drop_sources_accept_stale_snapshot_or_overwrite_manual_members() {
    let (mut host, task, option) = fixture();
    change_requirement(&mut host, "First declared change");
    let inputs = parametric::capture_inputs(&host.document.editor.project, &task, option).unwrap();
    let mut next =
        parametric::current_parameters(&host.document.editor.project, &task, option).unwrap();
    next.wing_depth_mm += 300;
    let mut dropped = inputs.clone();
    dropped.sources.clear();
    assert!(
        parametric::prepare_reviewed_update(
            &host.document.editor,
            &id(),
            &task,
            next.clone(),
            "Reject dropped source".into(),
            dropped
        )
        .is_err()
    );
    let mut forged = inputs.clone();
    forged.sources[0].baseline_sha256 = "b".repeat(64);
    assert!(
        parametric::prepare_reviewed_update(
            &host.document.editor,
            &id(),
            &task,
            next.clone(),
            "Reject fake current binding".into(),
            forged
        )
        .is_err()
    );
    change_requirement(&mut host, "Second change after capture");
    let before = host.document.editor.project.clone();
    assert!(
        parametric::prepare_reviewed_update(
            &host.document.editor,
            &id(),
            &task,
            next.clone(),
            "Reject stale review".into(),
            inputs
        )
        .is_err()
    );
    assert_eq!(host.document.editor.project, before);
    let inputs = parametric::capture_inputs(&before, &task, option).unwrap();
    let prepared = parametric::prepare_reviewed_update(
        &host.document.editor,
        &id(),
        &task,
        next.clone(),
        "Before human edit".into(),
        inputs.clone(),
    )
    .unwrap();
    let member = before.design_process.task().unwrap().candidates[0].initial_mass_ids[0];
    host.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![member],
            offset: Point::new(0.1, 0.),
        })
        .unwrap();
    let manual = host.document.editor.project.clone();
    assert!(
        host.commit_prepared(host.context(), &id(), &prepared)
            .is_err()
    );
    assert!(
        parametric::prepare_reviewed_update(
            &host.document.editor,
            &id(),
            &task,
            next,
            "Never renew over manual edits".into(),
            inputs
        )
        .err()
        .unwrap()
        .contains("Manual changes are protected")
    );
    assert_eq!(host.document.editor.project, manual);
    host.document.editor.undo();
    host.document.editor.set_selection(vec![member]);
    host.document
        .editor
        .edit(atelier_bim::editing::EditCommand::Lock)
        .unwrap();
    let locked = host.document.editor.project.clone();
    let inputs = parametric::capture_inputs(&locked, &task, option).unwrap();
    let mut next = parametric::current_parameters(&locked, &task, option).unwrap();
    next.wing_depth_mm += 300;
    assert!(
        parametric::prepare_reviewed_update(
            &host.document.editor,
            &id(),
            &task,
            next,
            "Never replace a locked member".into(),
            inputs
        )
        .is_err()
    );
    assert_eq!(host.document.editor.project, locked);
}
