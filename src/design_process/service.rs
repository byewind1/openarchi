//! Shared typed proposals. No live geometry replica and no transport permission changes.
use super::*;
use crate::{
    command::{ModelCommand, ProjectDelta},
    editor::Editor,
    model::Point,
    model_service::PreparedChange,
};
#[derive(Clone, Debug)]
pub struct Draft {
    pub name: String,
    pub question: String,
    pub statements: Vec<Statement>,
    pub candidate_names: [String; 2],
    pub source_ids: Vec<u64>,
    pub entry: Entry,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_secs())
        .unwrap_or(0)
}
pub fn prepare_create(editor: &Editor, id: &str, draft: Draft) -> Result<PreparedChange, String> {
    if draft.entry != Entry::ManualInterface {
        return Err("Exploration creation requires explicit manual adoption".into());
    }
    let p = &editor.project;
    if p.design_process.current_task.is_some() {
        return Err("Continue the current exploration before starting another".into());
    }
    if p.design_process.tasks.iter().any(|t| t.id == id) {
        return Err("Exploration request already recorded; inspect its original state".into());
    }
    if p.plan_options.active_design_option.is_some() || p.plan_options.active_worksheet.is_some() {
        return Err("Start this exploration from the main model floor plan".into());
    }
    if !(1..=16).contains(&draft.source_ids.len())
        || draft.source_ids.iter().collect::<BTreeSet<_>>().len() != draft.source_ids.len()
    {
        return Err(
            "Choose 1–16 main-model Zones or concept masses of one type on one story".into(),
        );
    }
    if draft.candidate_names[0] == draft.candidate_names[1] {
        return Err("Give the candidates different names".into());
    }
    let source = draft
        .source_ids
        .iter()
        .map(|id| {
            p.elements
                .iter()
                .find(|e| e.id == *id)
                .cloned()
                .ok_or("Exploration source is missing".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let story = source[0].story;
    let mass = matches!(source[0].shape, Shape::ConceptMass { .. });
    if p.elements.iter().any(|e| {
        e.story == story && e.design_option.is_some() && matches!(e.shape, Shape::Wall { .. })
    }) {
        return Err("Exploration with option-specific walls is unsupported".into());
    }

    for e in &source {
        if e.story != story
            || e.worksheet.is_some()
            || e.design_option.is_some()
            || !if mass {
                matches!(e.shape, Shape::ConceptMass { .. })
            } else {
                matches!(e.shape, Shape::Zone { .. })
            }
        {
            return Err(
                "Choose 1–16 main-model Zones or concept masses of one type on one story".into(),
            );
        }
        if editor.locked_element(e.id) {
            return Err("Exploration source is locked".into());
        }
    }
    let mut design = p.design_options.clone();
    let max = design.options.iter().map(|o| o.id).max().unwrap_or(0);
    let base = design
        .next_id
        .max(
            max.checked_add(1)
                .ok_or("Design option identities exhausted")?,
        )
        .max(1);
    let a = base
        .checked_add(1)
        .ok_or("Design option identities exhausted")?;
    let b = base
        .checked_add(2)
        .ok_or("Design option identities exhausted")?;
    design.next_id = base
        .checked_add(3)
        .ok_or("Design option identities exhausted")?;
    for (option, name) in [
        (base, format!("{} · Baseline", draft.name)),
        (a, draft.candidate_names[0].clone()),
        (b, draft.candidate_names[1].clone()),
    ] {
        design
            .options
            .push(crate::view_settings::DesignOption { id: option, name });
    }
    let mut next = p.next_id;
    let mut copies = vec![];
    let mut candidates = vec![];
    for (option, name) in [
        (a, draft.candidate_names[0].clone()),
        (b, draft.candidate_names[1].clone()),
    ] {
        let mut ids = vec![];
        for e in &source {
            let mut copy = e.clone();
            copy.id = next;
            next = next.checked_add(1).ok_or("Element identities exhausted")?;
            copy.design_option = Some(option);
            ids.push(copy.id);
            copies.push(copy);
        }
        candidates.push(Candidate {
            option,
            name,
            initial_zone_ids: if mass { vec![] } else { ids.clone() },
            initial_mass_ids: if mass { ids } else { vec![] },
        });
    }
    let mut task = Task {
        intent: None,
        id: id.into(),
        name: draft.name,
        question: draft.question,
        statements: draft.statements,
        entry: draft.entry,
        source_document: editor.project_identity().into(),
        created_unix: now(),
        story,
        source_zone_ids: if mass {
            vec![]
        } else {
            draft.source_ids.clone()
        },
        source_mass_ids: if mass { draft.source_ids } else { vec![] },
        baseline_option: base,
        basis_sha256: String::new(),
        candidates: candidates
            .try_into()
            .map_err(|_| "Invalid candidate pair")?,
    };
    task.basis_sha256 = task.common_hash(p)?;
    task.validate()?;
    let mut commands = vec![ModelCommand::SetOptions {
        plan: None,
        design: Some(design),
        guide: None,
        grid: None,
    }];
    for mut e in source {
        e.design_option = Some(base);
        commands.push(ModelCommand::UpdateElement { element: e });
    }
    commands.push(ModelCommand::AddElements {
        elements: copies,
        group: false,
    });
    commands.push(ModelCommand::DesignProcess {
        edit: Edit::Create { task },
    });
    let mut plan = p.plan_options.clone();
    plan.active_design_option = Some(a);
    commands.push(ModelCommand::SetOptions {
        plan: Some(plan),
        design: None,
        guide: None,
        grid: None,
    });
    editor
        .prepare_change(editor.revision, commands)
        .map_err(|e| e.message)
}
pub fn prepare_edit(
    editor: &Editor,
    id: u64,
    offset: Point,
    height: Option<f64>,
) -> Result<PreparedChange, String> {
    let scope =
        working_scope(&editor.project).ok_or("Activate a candidate before editing its spaces")?;
    if !scope.ids.contains(&id) {
        return Err("Space is outside the active candidate".into());
    }
    let old = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == id)
        .ok_or("Exploration space is missing")?;
    let mut e = old.clone();
    e.translate(offset);
    if let Shape::Zone { spec } = &mut e.shape {
        if let Some(height) = height {
            spec.height = height;
        }
        spec.validate()?;
    } else if let Shape::ConceptMass { spec } = &mut e.shape {
        if let Some(height) = height {
            spec.height = height;
        }
        spec.validate()?;
    } else {
        return Err("Unsupported exploration object".into());
    }
    if e == *old {
        return Err("Change a spatial parameter before previewing".into());
    }
    editor
        .prepare_change(
            editor.revision,
            vec![ModelCommand::UpdateElement { element: e }],
        )
        .map_err(|e| e.message)
}
pub fn prepare_choose(
    editor: &Editor,
    id: &str,
    task_id: &str,
    option: u32,
    reason: String,
    expected_comparison: String,
    entry: Entry,
    operator_label: String,
) -> Result<PreparedChange, String> {
    prepare_choice(
        editor,
        id,
        task_id,
        option,
        reason,
        expected_comparison,
        entry,
        operator_label,
        None,
    )
}
/// Only the local UI can acknowledge a changed basis. Reading a new model
/// revision or a comparison through automation never enters this path.
pub(crate) fn prepare_reviewed_choice(
    editor: &Editor,
    id: &str,
    task_id: &str,
    option: u32,
    reason: String,
    expected_comparison: String,
    reviewed_basis: String,
) -> Result<PreparedChange, String> {
    prepare_choice(
        editor,
        id,
        task_id,
        option,
        reason,
        expected_comparison,
        Entry::ManualInterface,
        "Local manual interface; changed basis explicitly reviewed, not client approval".into(),
        Some(reviewed_basis),
    )
}
fn prepare_choice(
    editor: &Editor,
    id: &str,
    task_id: &str,
    option: u32,
    reason: String,
    expected_comparison: String,
    entry: Entry,
    operator_label: String,
    reviewed_basis: Option<String>,
) -> Result<PreparedChange, String> {
    if editor
        .project
        .design_process
        .decisions
        .iter()
        .any(|d| d.id == id)
    {
        return Err("Design choice already recorded; inspect its original state".into());
    }
    let mut commands = vec![ModelCommand::DesignProcess {
        edit: Edit::Choose {
            id: id.into(),
            task_id: task_id.into(),
            option,
            reason,
            entry,
            operator_label,
            expected_comparison,
            reviewed_basis,
            created_unix: now(),
        },
    }];
    if entry == Entry::ManualInterface {
        let mut plan = editor.project.plan_options.clone();
        plan.active_design_option = Some(option);
        commands.push(ModelCommand::SetOptions {
            plan: Some(plan),
            design: None,
            guide: None,
            grid: None,
        });
    }
    editor
        .prepare_change(editor.revision, commands)
        .map_err(|e| e.message)
}
pub fn prepare_lock(
    editor: &Editor,
    task_id: &str,
    option: u32,
    locked: bool,
) -> Result<PreparedChange, String> {
    editor
        .prepare_change(
            editor.revision,
            vec![ModelCommand::DesignProcess {
                edit: Edit::Lock {
                    task_id: task_id.into(),
                    option,
                    locked,
                },
            }],
        )
        .map_err(|e| e.message)
}
pub fn prepare_activate(editor: &Editor, option: Option<u32>) -> Result<PreparedChange, String> {
    if let Some(id) = option {
        if let Some(task) = editor.project.design_process.tasks.iter().find(|task| {
            (Some(&task.id) == editor.project.design_process.current_task.as_ref()
                || editor
                    .project
                    .design_process
                    .suspended_contexts
                    .contains_key(&task.id))
                && (task.baseline_option == id || task.candidates.iter().any(|c| c.option == id))
        }) {
            if Some(&task.id) != editor.project.design_process.current_task.as_ref() {
                return super::intent::prepare_activate_option(editor, &task.id, option);
            }
        }
    }
    if option.is_some_and(|id| editor.project.design_options.name(id).is_none()) {
        return Err("Design option is missing".into());
    }
    let mut plan = editor.project.plan_options.clone();
    plan.active_design_option = option;
    editor
        .prepare_change(
            editor.revision,
            vec![ModelCommand::SetOptions {
                plan: Some(plan),
                design: None,
                guide: None,
                grid: None,
            }],
        )
        .map_err(|e| e.message)
}
pub(crate) fn selected_space(editor: &Editor) -> Option<(u64, f64)> {
    let task = editor.project.design_process.task()?;
    let active = editor.project.plan_options.active_design_option?;
    let selected = editor.selection_ids();
    if selected.len() != 1 || !task.candidates.iter().any(|c| c.option == active) {
        return None;
    }
    editor
        .project
        .elements
        .iter()
        .find(|e| {
            e.id == selected[0]
                && e.design_option == Some(active)
                && e.story == task.story
                && e.worksheet.is_none()
        })
        .and_then(|e| match &e.shape {
            Shape::Zone { spec } => Some((e.id, spec.height)),
            Shape::ConceptMass { spec } => Some((e.id, spec.height)),
            _ => None,
        })
}
#[derive(Clone)]
pub(crate) struct WorkScope {
    pub option: u32,
    pub story: u32,
    pub ids: BTreeSet<u64>,
    pub mass: bool,
}
pub(crate) fn working_scope(p: &Project) -> Option<WorkScope> {
    let task = p.design_process.task()?;
    let option = p.plan_options.active_design_option?;
    task.candidates.iter().find(|c| c.option == option)?;
    Some(WorkScope {
        mass: task.is_mass(),
        option,
        story: task.story,
        ids: p
            .elements
            .iter()
            .filter(|e| e.design_option == Some(option))
            .map(|e| e.id)
            .collect(),
    })
}
fn guard_element(
    scope: &WorkScope,
    before: Option<&Element>,
    after: Option<&Element>,
    mass_space_creation: bool,
) -> Result<(), String> {
    if before.is_some_and(|e| !scope.ids.contains(&e.id))
        || after.is_some_and(|e| {
            !(if scope.mass {
                matches!(e.shape, Shape::ConceptMass { .. })
                    || (matches!(e.shape, Shape::Zone { .. })
                        && (mass_space_creation
                            || before.is_some_and(|old| matches!(old.shape, Shape::Zone { .. }))))
            } else {
                matches!(e.shape, Shape::Zone { .. })
            }) || e.design_option != Some(scope.option)
                || e.story != scope.story
                || e.worksheet.is_some()
        })
    {
        return Err("Edit only the active candidate spaces; switch to the baseline to change the common model".into());
    }
    Ok(())
}
#[derive(Clone)]
pub(crate) enum ContextScope {
    Candidate(WorkScope),
    Common(Vec<u32>),
}
pub(crate) fn context_scope(p: &Project) -> Option<ContextScope> {
    let t = p.design_process.task()?;
    Some(
        working_scope(p)
            .map(ContextScope::Candidate)
            .unwrap_or_else(|| {
                ContextScope::Common(
                    p.design_process
                        .tasks
                        .iter()
                        .filter(|task| {
                            task.id == t.id
                                || p.design_process.suspended_contexts.contains_key(&task.id)
                        })
                        .flat_map(|task| {
                            let mut options =
                                vec![task.candidates[0].option, task.candidates[1].option];
                            if task.id != t.id {
                                options.push(task.baseline_option);
                            }
                            options
                        })
                        .collect(),
                )
            }),
    )
}
pub(crate) fn guard_delta(
    scope: Option<&ContextScope>,
    delta: &ProjectDelta,
    mass_space_creation: bool,
) -> Result<(), String> {
    match scope {
        None => Ok(()),
        Some(ContextScope::Candidate(scope)) => {
            guard_candidate_delta(Some(scope), delta, mass_space_creation)
        }
        Some(ContextScope::Common(options)) => {
            if delta.elements.iter().any(|e| {
                e.before
                    .iter()
                    .chain(e.after.iter())
                    .any(|e| e.design_option.is_some_and(|id| options.contains(&id)))
            }) {
                return Err("Activate the owning candidate before editing its spaces".into());
            }
            Ok(())
        }
    }
}
fn guard_candidate_delta(
    scope: Option<&WorkScope>,
    delta: &ProjectDelta,
    mass_space_creation: bool,
) -> Result<(), String> {
    let Some(scope) = scope else {
        return Ok(());
    };
    if let (Some(before), Some(after)) = (&delta.before_header, &delta.after_header) {
        if before.stories != after.stories {
            return Err("Switch to the baseline before changing common story elevations".into());
        }
    }
    let added = delta
        .elements
        .iter()
        .filter(|e| e.before.is_none() && e.after.is_some())
        .count();
    let removed = delta
        .elements
        .iter()
        .filter(|e| e.before.is_some() && e.after.is_none())
        .count();
    if scope
        .ids
        .len()
        .checked_add(added)
        .and_then(|n| n.checked_sub(removed))
        .is_none_or(|n| n > 32)
    {
        return Err("Exploration candidate is limited to 32 spaces".into());
    }
    for e in &delta.elements {
        guard_element(
            scope,
            e.before.as_ref(),
            e.after.as_ref(),
            mass_space_creation,
        )?;
    }
    Ok(())
}
/// Association metadata must obey the same active candidate/common boundary even
/// when geometry is unchanged. This does not grant creation or remote authority.
pub(crate) fn guard_mass_space_scope(
    p: &Project,
    story: u32,
    option: Option<u32>,
) -> Result<(), String> {
    match context_scope(p) {
        Some(ContextScope::Candidate(scope)) if story != scope.story || option != Some(scope.option) =>
            Err("Edit only the active candidate spaces; switch to the baseline to change the common model".into()),
        Some(ContextScope::Common(options)) if option.is_some_and(|id| options.contains(&id)) =>
            Err("Activate the owning candidate before editing its spaces".into()),
        _ => Ok(()),
    }
}
pub(crate) fn guard_transition(
    before: &Project,
    after: &Project,
    mass_space_creation: bool,
) -> Result<(), String> {
    after
        .design_process
        .assert_append_only_from(&before.design_process)?;
    if super::intent::is_exact_task_append(before, after)? {
        return Ok(());
    }
    let Some(scope) = working_scope(before) else {
        if let Some(ContextScope::Common(options)) = context_scope(before) {
            let old = before
                .elements
                .iter()
                .map(|e| (e.id, e))
                .collect::<std::collections::BTreeMap<_, _>>();
            let new = after
                .elements
                .iter()
                .map(|e| (e.id, e))
                .collect::<std::collections::BTreeMap<_, _>>();
            for id in old.keys().chain(new.keys()) {
                let a = old.get(id).copied();
                let b = new.get(id).copied();
                if a != b
                    && a.into_iter()
                        .chain(b)
                        .any(|e| e.design_option.is_some_and(|id| options.contains(&id)))
                {
                    return Err("Activate the owning candidate before editing its spaces".into());
                }
            }
        }
        return Ok(());
    };
    if before.stories != after.stories {
        return Err("Switch to the baseline before changing common story elevations".into());
    }
    if after
        .elements
        .iter()
        .filter(|e| e.design_option == Some(scope.option))
        .count()
        > 32
    {
        return Err("Exploration candidate is limited to 32 spaces".into());
    }
    let after_ids = after
        .elements
        .iter()
        .map(|e| (e.id, e))
        .collect::<std::collections::BTreeMap<_, _>>();
    for old in &before.elements {
        let new = after_ids.get(&old.id).copied();
        if new != Some(old) {
            guard_element(&scope, Some(old), new, mass_space_creation)?;
        }
    }
    let old_ids = before
        .elements
        .iter()
        .map(|e| e.id)
        .collect::<BTreeSet<_>>();
    for new in &after.elements {
        if !old_ids.contains(&new.id) {
            guard_element(&scope, None, Some(new), mass_space_creation)?;
        }
    }
    Ok(())
}
