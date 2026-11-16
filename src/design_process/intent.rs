//! Generative spatial proposals over the existing Zone model and transaction path.
//! Numeric geometry is authored design assumption, never an image measurement.
use super::*;
use crate::{
    command::{ModelCommand, ProjectHeader},
    editor::Editor,
    model_service::PreparedChange,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageReference {
    pub page: u64,
    pub sha256: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Observation,
    Inference,
    DesignCompletion,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub kind: ClaimKind,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric: Option<super::parametric::Definition>,
    pub expected_result: String,
    pub images: Vec<ImageReference>,
    pub claims: Vec<Claim>,
    pub geometry_assumptions: String,
    pub all_numeric_geometry_is_design_assumption: bool,
    pub parent_task: Option<String>,
    pub parent_choice: Option<String>,
    pub parent_active_option: Option<u32>,
    pub parent_project_sha256: String,
    pub initial_candidate_sha256: [String; 2],
}
impl Provenance {
    pub fn validate(&self) -> Result<(), String> {
        if !text(&self.expected_result, 4096, true)
            || !text(&self.geometry_assumptions, 4096, true)
            || !self.all_numeric_geometry_is_design_assumption
            || !(1..=8).contains(&self.images.len())
                && !(self.images.is_empty() && self.parametric.is_some())
            || self.claims.len() > 32
            || self.claims.iter().any(|c| !text(&c.text, 2048, true))
            || !crate::storage::valid_document_identity(&self.parent_project_sha256)
            || self
                .initial_candidate_sha256
                .iter()
                .any(|s| !crate::storage::valid_document_identity(s))
            || [&self.parent_task, &self.parent_choice].iter().any(|v| {
                v.as_ref()
                    .is_some_and(|s| !crate::storage::valid_document_identity(s))
            })
        {
            return Err("Invalid generative spatial provenance".into());
        }
        if let Some(rule) = &self.parametric {
            rule.validate()?;
        }
        let mut pages = BTreeSet::new();
        if self.images.iter().any(|i| {
            i.page == 0
                || !pages.insert(i.page)
                || !crate::storage::valid_document_identity(&i.sha256)
        }) {
            return Err("Invalid intent image reference".into());
        }
        Ok(())
    }
    pub fn source_versions(&self, p: &Project) -> serde_json::Value {
        serde_json::json!(self.images.iter().map(|reference|{
            let actual=p.drawings.pages.iter().find(|page|page.id==reference.page);
            serde_json::json!({"page":reference.page,"expected_sha256":reference.sha256,"actual":actual.map(|page|serde_json::json!({"sha256":page.sha256,"pixels":page.pixels})),"interpretation":"declared; original-byte reading and architectural intent acceptance are separate evidence"})
        }).collect::<Vec<_>>())
    }
    fn check_images(&self, p: &Project) -> Result<(), String> {
        if self.images.iter().any(|r| {
            !p.drawings
                .pages
                .iter()
                .any(|page| page.id == r.page && page.sha256 == r.sha256)
        }) {
            return Err("Intent source image changed or is missing".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSeed {
    pub name: String,
    pub spec: crate::zones::ZoneSpec,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDraft<S> {
    pub name: String,
    pub question: String,
    pub expected_result: String,
    pub images: Vec<ImageReference>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub geometry_assumptions: String,
    pub statements: Vec<Statement>,
    pub candidate_names: [String; 2],
    pub candidates: [Vec<S>; 2],
    pub story: u32,
    pub layer: u32,
}
pub type Draft = IntentDraft<SpaceSeed>;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassSeed {
    pub name: String,
    pub spec: crate::concept_mass::ConceptMassSpec,
    pub material: crate::model::Material,
}
pub type MassDraft = IntentDraft<MassSeed>;
pub(super) trait IntentSeed {
    const MASS: bool;
    fn name(&self) -> &str;
    fn material(&self) -> crate::model::Material {
        crate::model::Material::Plaster
    }
    fn shape(&self) -> Shape;
    fn validate(&self) -> Result<(), String>;
}
impl IntentSeed for SpaceSeed {
    const MASS: bool = false;
    fn name(&self) -> &str {
        &self.name
    }
    fn shape(&self) -> Shape {
        Shape::Zone {
            spec: self.spec.clone(),
        }
    }
    fn validate(&self) -> Result<(), String> {
        self.spec.validate()
    }
}
impl IntentSeed for MassSeed {
    const MASS: bool = true;
    fn material(&self) -> crate::model::Material {
        self.material
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn shape(&self) -> Shape {
        Shape::ConceptMass {
            spec: self.spec.clone(),
        }
    }
    fn validate(&self) -> Result<(), String> {
        self.spec.validate()
    }
}
fn candidate_hash(p: &Project, candidate: &Candidate) -> Result<String, String> {
    let elements = candidate
        .element_ids()
        .iter()
        .map(|id| {
            p.elements
                .iter()
                .find(|e| e.id == *id)
                .ok_or("Initial candidate space is missing")
        })
        .collect::<Result<Vec<_>, _>>()?;
    hash(&elements)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskContext {
    pub choice: Option<String>,
    pub active_option: Option<u32>,
}
fn suspend(out: &mut Journal, active_option: Option<u32>) {
    if let Some(id) = out.current_task.take() {
        out.suspended_contexts.insert(
            id,
            TaskContext {
                choice: out.current_choice.take(),
                active_option,
            },
        );
    }
    out.current_choice = None;
}
pub(crate) fn apply_create(p: &Project, out: &mut Journal, task: &Task) -> Result<(), String> {
    let intent = task
        .intent
        .as_ref()
        .ok_or("Intent provenance is required")?;
    task.validate()?;
    intent.check_images(p)?;
    if let Some(rule) = &intent.parametric {
        rule.check_inputs(p)?;
        super::parametric::validate_initial(p, task, rule)?;
    }
    if out.current_task != intent.parent_task
        || out.current_choice != intent.parent_choice
        || out.tasks.iter().any(|t| t.id == task.id)
    {
        return Err("Intent parent task changed or request already recorded".into());
    }
    if p.design_options.name(task.baseline_option).is_none() {
        return Err("Intent baseline option is missing".into());
    }
    for (index, candidate) in task.candidates.iter().enumerate() {
        if p.design_options.name(candidate.option) != Some(candidate.name.as_str())
            || candidate_hash(p, candidate)? != intent.initial_candidate_sha256[index]
        {
            return Err("Intent initial candidate differs from its declared version".into());
        }
        for id in candidate.element_ids() {
            if !p.elements.iter().any(|e| {
                e.id == *id
                    && e.design_option == Some(candidate.option)
                    && e.story == task.story
                    && e.worksheet.is_none()
                    && task.accepts_shape(&e.shape)
            }) {
                return Err("Invalid intent candidate membership".into());
            }
        }
    }
    for (i, id) in task.source_element_ids().iter().enumerate() {
        let source = p
            .elements
            .iter()
            .find(|e| e.id == *id && e.design_option == Some(task.baseline_option))
            .ok_or("Intent baseline is missing")?;
        let first = p
            .elements
            .iter()
            .find(|e| e.id == task.candidates[0].element_ids()[i])
            .ok_or("Intent initial space is missing")?;
        let mut expected = first.clone();
        expected.id = *id;
        expected.design_option = Some(task.baseline_option);
        if &expected != source {
            return Err("Intent baseline differs from its original seed".into());
        }
    }
    if task.common_hash(p)? != task.basis_sha256 {
        return Err("Intent project basis changed".into());
    }
    suspend(out, intent.parent_active_option);
    out.tasks.push(task.clone());
    if intent.parametric.is_some() {
        for c in &task.candidates {
            out.parametric_states
                .insert(c.option, super::parametric::State::Initial);
        }
    }
    out.current_task = Some(task.id.clone());
    Ok(())
}
pub(crate) fn activate(p: &Project, out: &mut Journal, id: &str) -> Result<(), String> {
    if out.current_task.as_deref() == Some(id) {
        return Ok(());
    }
    if !out.suspended_contexts.contains_key(id) {
        return Err("Design task has no saved resumable context; use its original Undo/Redo or inspect history".into());
    }
    let task = out
        .tasks
        .iter()
        .find(|t| t.id == id)
        .ok_or("Design task is missing")?;
    if std::iter::once(task.baseline_option)
        .chain(task.candidates.iter().map(|c| c.option))
        .any(|o| p.design_options.name(o).is_none())
    {
        return Err("Design task model versions are unavailable".into());
    }
    suspend(out, p.plan_options.active_design_option);
    out.current_choice = out
        .suspended_contexts
        .remove(id)
        .and_then(|context| context.choice);
    out.current_task = Some(id.into());
    Ok(())
}
/// Only a pure append of this exact typed task may cross an old candidate's scope.
/// Existing geometry, options, headers, decisions and user choice context cannot change.
pub(crate) fn is_exact_task_append(before: &Project, after: &Project) -> Result<bool, String> {
    let added = after
        .design_process
        .tasks
        .iter()
        .filter(|t| !before.design_process.tasks.iter().any(|old| old.id == t.id))
        .collect::<Vec<_>>();
    if !added.iter().any(|t| t.intent.is_some()) {
        return Ok(false);
    }
    let reject =
        || "Generative task must append new candidates without changing existing work".to_string();
    if added.len() != 1 || after.design_process.tasks.len() != before.design_process.tasks.len() + 1
    {
        return Err(reject());
    }
    let task = added[0];
    let intent = task.intent.as_ref().ok_or_else(reject)?;
    if intent.parent_project_sha256 != crate::automation_journal::project_hash(before)?
        || intent.parent_task != before.design_process.current_task
        || intent.parent_choice != before.design_process.current_choice
        || intent.parent_active_option != before.plan_options.active_design_option
    {
        return Err(reject());
    }
    let n = before.elements.len();
    if after.elements.len() < n || after.elements[..n] != before.elements {
        return Err(reject());
    }
    let declared = task
        .source_element_ids()
        .iter()
        .chain(task.candidates.iter().flat_map(|c| c.element_ids()))
        .copied()
        .collect::<BTreeSet<_>>();
    let actual = after.elements[n..]
        .iter()
        .map(|e| e.id)
        .collect::<BTreeSet<_>>();
    if declared != actual
        || actual.len() != 3 * task.source_element_ids().len()
        || after.elements[n..].iter().any(|e| {
            e.id < before.next_id
                || e.story != task.story
                || e.worksheet.is_some()
                || !task.accepts_shape(&e.shape)
        })
    {
        return Err(reject());
    }
    let mut expected = ProjectHeader::capture(before);
    let actual_header = ProjectHeader::capture(after);
    let old_options = &before.design_options.options;
    let options = &after.design_options.options;
    if options.len() != old_options.len() + 3
        || options[..old_options.len()] != old_options[..]
        || options[old_options.len()..]
            .iter()
            .map(|o| o.id)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([
                task.baseline_option,
                task.candidates[0].option,
                task.candidates[1].option,
            ])
    {
        return Err(reject());
    }
    if after.plan_options.active_design_option != Some(task.candidates[0].option) {
        return Err(reject());
    }
    let max = actual
        .iter()
        .max()
        .ok_or_else(reject)?
        .checked_add(1)
        .ok_or_else(reject)?;
    if after.next_id != max {
        return Err(reject());
    }
    let mut journal = before.design_process.clone();
    apply_create(after, &mut journal, task)?;
    expected.design_process = journal;
    expected.next_id = max;
    expected.design_options = after.design_options.clone();
    expected.plan_options.active_design_option = Some(task.candidates[0].option);
    if expected != actual_header {
        return Err(reject());
    }
    Ok(true)
}
pub fn prepare(editor: &Editor, id: &str, draft: Draft) -> Result<PreparedChange, String> {
    prepare_proposal(editor, id, draft)
}
pub fn prepare_mass(editor: &Editor, id: &str, draft: MassDraft) -> Result<PreparedChange, String> {
    prepare_proposal(editor, id, draft)
}
fn prepare_proposal<S: IntentSeed>(
    editor: &Editor,
    id: &str,
    draft: IntentDraft<S>,
) -> Result<PreparedChange, String> {
    prepare_proposal_with_rule(editor, id, draft, None, Entry::AgentProposal)
}
pub(super) fn prepare_proposal_with_rule<S: IntentSeed>(
    editor: &Editor,
    id: &str,
    draft: IntentDraft<S>,
    rule: Option<super::parametric::Definition>,
    entry: Entry,
) -> Result<PreparedChange, String> {
    if editor.model_input_active() {
        return Err(
            "Finish or cancel the current model interaction before changing design tasks".into(),
        );
    }
    let p = &editor.project;
    if p.design_process.tasks.iter().any(|t| t.id == id) {
        return Err("Intent request already recorded; inspect its original state".into());
    }
    if p.plan_options.active_worksheet.is_some() {
        return Err("Spatial intent requires a model story, not a worksheet".into());
    }
    let count = draft.candidates[0].len();
    if !(1..=16).contains(&count)
        || draft.candidates[1].len() != count
        || draft.candidate_names[0] == draft.candidate_names[1]
    {
        return Err("Intent needs two named corresponding sets of 1–16 spaces".into());
    }
    for i in 0..count {
        for seeds in &draft.candidates {
            if !text(seeds[i].name(), 256, true) {
                return Err("Intent space name is required".into());
            }
            seeds[i].validate()?;
        }
        if draft.candidates[0][i].name() != draft.candidates[1][i].name() {
            return Err("Declare the same space identities in both initial candidates".into());
        }
    }
    let history_option = p
        .design_process
        .tasks
        .iter()
        .flat_map(|t| {
            [
                t.baseline_option,
                t.candidates[0].option,
                t.candidates[1].option,
            ]
        })
        .max()
        .unwrap_or(0);
    let base = p
        .design_options
        .next_id
        .max(
            p.design_options
                .options
                .iter()
                .map(|o| o.id)
                .max()
                .unwrap_or(0)
                .max(history_option)
                .checked_add(1)
                .ok_or("Option identity exhausted")?,
        )
        .max(1);
    let a = base.checked_add(1).ok_or("Option identity exhausted")?;
    let b = base.checked_add(2).ok_or("Option identity exhausted")?;
    let mut design = p.design_options.clone();
    design.next_id = base.checked_add(3).ok_or("Option identity exhausted")?;
    for (option, name) in [
        (base, format!("{} · Intent baseline", draft.name)),
        (a, draft.candidate_names[0].clone()),
        (b, draft.candidate_names[1].clone()),
    ] {
        design
            .options
            .push(crate::view_settings::DesignOption { id: option, name });
    }
    let history_element = p
        .design_process
        .tasks
        .iter()
        .flat_map(|t| {
            t.source_element_ids()
                .iter()
                .chain(t.candidates.iter().flat_map(|c| c.element_ids()))
        })
        .copied()
        .max()
        .unwrap_or(0);
    let mut next = p.next_id.max(
        history_element
            .checked_add(1)
            .ok_or("Element identity exhausted")?,
    );
    let mut baseline = vec![];
    let mut additions = vec![];
    let mut candidates = vec![];
    let mut fingerprints = vec![];
    for (group, option) in [base, a, b].into_iter().enumerate() {
        let seeds = &draft.candidates[usize::from(group == 2)];
        let mut group_elements = vec![];
        for seed in seeds {
            let mut element = editor.creation_candidate(
                seed.material(),
                crate::layers::ElementLayer(draft.layer),
                seed.shape(),
            );
            element.id = next;
            next = next.checked_add(1).ok_or("Element identity exhausted")?;
            element.name = seed.name().to_owned();
            element.story = draft.story;
            element.worksheet = None;
            element.design_option = Some(option);
            group_elements.push(element);
        }
        if group == 0 {
            baseline = group_elements.clone();
        } else {
            fingerprints.push(hash(&group_elements)?);
            candidates.push(Candidate {
                option,
                name: draft.candidate_names[group - 1].clone(),
                initial_zone_ids: if S::MASS {
                    vec![]
                } else {
                    group_elements.iter().map(|e| e.id).collect()
                },
                initial_mass_ids: if S::MASS {
                    group_elements.iter().map(|e| e.id).collect()
                } else {
                    vec![]
                },
            });
        }
        additions.extend(group_elements);
    }
    let provenance = Provenance {
        parametric: rule.clone(),
        expected_result: draft.expected_result,
        images: draft.images,
        claims: draft.claims,
        geometry_assumptions: if draft.geometry_assumptions.trim().is_empty() {
            "All coordinates, scale, dimensions, placement and height are design assumptions; no surveyed dimensions or approval supplied.".into()
        } else {
            draft.geometry_assumptions
        },
        all_numeric_geometry_is_design_assumption: true,
        parent_task: p.design_process.current_task.clone(),
        parent_choice: p.design_process.current_choice.clone(),
        parent_active_option: p.plan_options.active_design_option,
        parent_project_sha256: crate::automation_journal::project_hash(p)?,
        initial_candidate_sha256: fingerprints.try_into().map_err(|_| "Invalid intent pair")?,
    };
    provenance.validate()?;
    provenance.check_images(p)?;
    let mut task = Task {
        intent: Some(provenance),
        id: id.into(),
        name: draft.name,
        question: draft.question,
        statements: draft.statements,
        entry,
        source_document: editor.project_identity().into(),
        created_unix: service::now(),
        story: draft.story,
        source_zone_ids: if S::MASS {
            vec![]
        } else {
            baseline.iter().map(|e| e.id).collect()
        },
        source_mass_ids: if S::MASS {
            baseline.iter().map(|e| e.id).collect()
        } else {
            vec![]
        },
        baseline_option: base,
        basis_sha256: String::new(),
        candidates: candidates.try_into().map_err(|_| "Invalid intent pair")?,
    };
    task.basis_sha256 = task.common_hash_with_appended(p, &baseline)?;
    task.validate()?;
    let mut main = p.plan_options.clone();
    main.active_design_option = None;
    let mut active = main.clone();
    active.active_design_option = Some(a);
    let commands = vec![
        ModelCommand::SetOptions {
            plan: Some(main),
            design: Some(design),
            guide: None,
            grid: None,
        },
        ModelCommand::AddElements {
            elements: additions,
            group: false,
        },
        ModelCommand::DesignProcess {
            edit: Edit::CreateIntent { task },
        },
        ModelCommand::SetOptions {
            plan: Some(active),
            design: None,
            guide: None,
            grid: None,
        },
    ];
    let prepared = editor
        .prepare_change(editor.revision, commands)
        .map_err(|e| e.message)?;
    if !is_exact_task_append(p, prepared.project())? {
        return Err("Invalid intent append".into());
    }
    Ok(prepared)
}
pub fn prepare_activate(editor: &Editor, id: &str) -> Result<PreparedChange, String> {
    editor
        .project
        .design_process
        .tasks
        .iter()
        .find(|t| t.id == id)
        .ok_or("Design task is missing")?;
    let context = editor
        .project
        .design_process
        .suspended_contexts
        .get(id)
        .ok_or("Design task has no saved resumable context")?;
    prepare_activate_option(editor, id, context.active_option)
}
pub fn prepare_activate_option(
    editor: &Editor,
    id: &str,
    option: Option<u32>,
) -> Result<PreparedChange, String> {
    let task = editor
        .project
        .design_process
        .tasks
        .iter()
        .find(|t| t.id == id)
        .ok_or("Design task is missing")?;
    if option.is_some_and(|o| {
        o != task.baseline_option && !task.candidates.iter().any(|c| c.option == o)
    }) {
        return Err("Option is outside the requested design task".into());
    }
    let mut plan = editor.project.plan_options.clone();
    plan.active_design_option = option;
    editor
        .prepare_change(
            editor.revision,
            vec![
                ModelCommand::DesignProcess {
                    edit: Edit::ActivateTask { task_id: id.into() },
                },
                ModelCommand::SetOptions {
                    plan: Some(plan),
                    design: None,
                    guide: None,
                    grid: None,
                },
            ],
        )
        .map_err(|e| e.message)
}
pub(crate) fn is_exact_context_activation(
    before: &Project,
    after: &Project,
) -> Result<bool, String> {
    if before.design_process.current_task == after.design_process.current_task {
        return Ok(false);
    }
    let Some(id) = after.design_process.current_task.as_deref() else {
        return Ok(false);
    };
    let mut expected = ProjectHeader::capture(before);
    let mut journal = before.design_process.clone();
    if activate(before, &mut journal, id).is_err() {
        return Ok(false);
    }
    let task = journal.task().ok_or("Design task is missing")?;
    let option = after.plan_options.active_design_option;
    if option.is_some_and(|o| {
        o != task.baseline_option && !task.candidates.iter().any(|c| c.option == o)
    }) {
        return Ok(false);
    }
    expected.plan_options.active_design_option = option;
    expected.design_process = journal;
    Ok(before.elements == after.elements && expected == ProjectHeader::capture(after))
}
