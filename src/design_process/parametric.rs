//! One bounded two-wing rule, evaluated into existing ConceptMass geometry.
//! Dimensions and storeys describe a concept envelope, never surveyed or statutory area.
use super::*;
use crate::{
    concept_mass::ConceptMassSpec,
    editor::Editor,
    evidence::{Link, LinkStatus},
    model::{Material, Point},
    model_service::PreparedChange,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub origin_mm: [i64; 2],
    pub court_width_mm: u32,
    pub court_depth_mm: u32,
    pub wing_depth_mm: u32,
    pub storeys: u32,
    pub floor_height_mm: u32,
}
impl Parameters {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .origin_mm
            .iter()
            .any(|v| v.unsigned_abs() > 1_000_000_000)
            || [self.court_width_mm, self.court_depth_mm, self.wing_depth_mm]
                .iter()
                .any(|v| !(1000..=1_000_000).contains(v))
            || !(1..=20).contains(&self.storeys)
            || !(2000..=6000).contains(&self.floor_height_mm)
            || u64::from(self.storeys) * u64::from(self.floor_height_mm) > 100_000
        {
            return Err("Two-wing parameters exceed the bounded concept range".into());
        }
        Ok(())
    }
    pub fn geometry(&self) -> Result<[ConceptMassSpec; 2], String> {
        self.validate()?;
        let [x, y] = self.origin_mm.map(|v| v as f64 / 1000.);
        let w = self.court_width_mm as f64 / 1000.;
        let d = self.court_depth_mm as f64 / 1000.;
        let t = self.wing_depth_mm as f64 / 1000.;
        let rect = |x: f64, y: f64, w: f64, d: f64| ConceptMassSpec {
            arcs: vec![],
            hole_arcs: vec![],
            outline: vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + d),
                Point::new(x, y + d),
            ],
            holes: vec![],
            height: self.storeys as f64 * self.floor_height_mm as f64 / 1000.,
            bottom_offset: 0.,
        };
        let result = [rect(x, y + d, w + t, t), rect(x + w, y, t, d)];
        for spec in &result {
            spec.validate()?;
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// Version 1 is an L-shaped pair with an open rectangular court; no rotations/holes.
    pub rule_version: u32,
    /// Initial parameters; current versions are journal state references, not these seeds.
    pub candidates: [Parameters; 2],
    pub site_sha256: String,
    pub sources: Vec<Link>,
    pub assumptions: String,
}
/// A candidate's reviewed version of the same declared input targets. Neither
/// this snapshot nor its acceptance grants write authority or certifies sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub site_sha256: String,
    pub sources: Vec<Link>,
}
impl Inputs {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::storage::valid_document_identity(&self.site_sha256) || self.sources.len() > 16 {
            return Err("Invalid reviewed parametric inputs".into());
        }
        let mut seen = BTreeSet::new();
        for source in &self.sources {
            source.target.validate()?;
            if !seen.insert(&source.target)
                || !crate::storage::valid_document_identity(&source.baseline_sha256)
            {
                return Err("Invalid reviewed parametric source binding".into());
            }
        }
        Ok(())
    }
    pub fn check_current(&self, p: &Project) -> Result<(), String> {
        self.validate()?;
        if hash(&p.site)? != self.site_sha256
            || self
                .sources
                .iter()
                .any(|s| s.status(p) != LinkStatus::Unchanged)
        {
            return Err(
                "Parametric inputs changed; review the original sources before a new proposal"
                    .into(),
            );
        }
        Ok(())
    }
    fn validate_for(&self, rule: &Definition) -> Result<(), String> {
        self.validate()?;
        if self
            .sources
            .iter()
            .map(|s| &s.target)
            .ne(rule.sources.iter().map(|s| &s.target))
        {
            return Err("Reviewed inputs cannot change the original source targets".into());
        }
        Ok(())
    }
}
impl Definition {
    pub fn inputs(&self) -> Inputs {
        Inputs {
            site_sha256: self.site_sha256.clone(),
            sources: self.sources.clone(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.candidates[0] == self.candidates[1]
            || self.rule_version != 1
            || !crate::storage::valid_document_identity(&self.site_sha256)
            || self.sources.len() > 16
            || !text(&self.assumptions, 4096, true)
        {
            return Err("Invalid two-wing definition".into());
        }
        for p in &self.candidates {
            p.validate()?;
        }
        let mut seen = BTreeSet::new();
        for source in &self.sources {
            source.target.validate()?;
            if !seen.insert(&source.target)
                || !crate::storage::valid_document_identity(&source.baseline_sha256)
            {
                return Err("Invalid two-wing source binding".into());
            }
        }
        Ok(())
    }
    pub fn check_inputs(&self, p: &Project) -> Result<(), String> {
        self.validate()?;
        self.inputs().check_current(p)
    }
}
pub fn prepare(
    editor: &Editor,
    id: &str,
    name: String,
    question: String,
    definition: Definition,
    story: u32,
    layer: u32,
) -> Result<PreparedChange, String> {
    prepare_with_entry(
        editor,
        id,
        name,
        question,
        definition,
        story,
        layer,
        Entry::ManualInterface,
    )
}
/// The caller supplies its actual entry; wire payloads never choose an audit origin.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_with_entry(
    editor: &Editor,
    id: &str,
    name: String,
    question: String,
    definition: Definition,
    story: u32,
    layer: u32,
    entry: Entry,
) -> Result<PreparedChange, String> {
    definition.check_inputs(&editor.project)?;
    let seeds = definition
        .candidates
        .iter()
        .map(|p| {
            p.geometry().map(|g| {
                g.into_iter()
                    .enumerate()
                    .map(|(i, spec)| intent::MassSeed {
                        name: format!("Wing {}", i + 1),
                        spec,
                        material: Material::Plaster,
                    })
                    .collect()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let draft = intent::MassDraft {
        name,
        question,
        expected_result:
            "Two editable L-shaped double-wing concept candidates with declared assumptions".into(),
        images: vec![],
        claims: vec![intent::Claim {
            kind: intent::ClaimKind::DesignCompletion,
            text: definition.assumptions.clone(),
        }],
        geometry_assumptions: definition.assumptions.clone(),
        statements: vec![],
        candidate_names: ["Two-wing A".into(), "Two-wing B".into()],
        candidates: seeds.try_into().map_err(|_| "Invalid rule pair")?,
        story,
        layer,
    };
    intent::prepare_proposal_with_rule(editor, id, draft, Some(definition), entry)
}
fn candidate<'a>(
    p: &'a Project,
    task_id: &str,
    option: u32,
) -> Result<(&'a Task, usize, &'a Definition), String> {
    let task = p
        .design_process
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .ok_or("Parametric task is missing")?;
    let index = task
        .candidates
        .iter()
        .position(|c| c.option == option)
        .ok_or("Activate a parametric candidate")?;
    let rule = task
        .intent
        .as_ref()
        .and_then(|v| v.parametric.as_ref())
        .ok_or("No parametric definition for this task")?;
    Ok((task, index, rule))
}
fn elements_match(p: &Project, ids: &[u64], parameters: &Parameters) -> Result<(), String> {
    if ids.len() != 2 {
        return Err("Two-wing member mapping is incomplete".into());
    }
    for (id, spec) in ids.iter().zip(parameters.geometry()?) {
        if !p
            .elements
            .iter()
            .any(|e| e.id == *id && e.shape == Shape::ConceptMass { spec: spec.clone() })
        {
            return Err("Generated geometry does not match its two-wing parameters".into());
        }
    }
    Ok(())
}
pub(super) fn validate_initial(p: &Project, task: &Task, rule: &Definition) -> Result<(), String> {
    for (index, c) in task.candidates.iter().enumerate() {
        elements_match(p, &c.initial_mass_ids, &rule.candidates[index])?;
    }
    Ok(())
}
/// A version reference in the existing journal, not geometry or write authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum State {
    Initial,
    Update { id: String },
    UnknownLegacy,
}
pub(super) fn validate_states(j: &Journal) -> Result<(), String> {
    if j.parametric_states.len() > 64 {
        return Err("Too many current parameter states".into());
    }
    for (option, state) in &j.parametric_states {
        let tasks = j
            .tasks
            .iter()
            .filter(|t| {
                t.intent.as_ref().is_some_and(|v| v.parametric.is_some())
                    && t.candidates.iter().any(|c| c.option == *option)
            })
            .collect::<Vec<_>>();
        if tasks.len() != 1 {
            return Err("Parameter state has no unique rule candidate".into());
        }
        if let State::Update { id } = state {
            if !j.updates.iter().any(|r| {
                r.id == *id
                    && r.task_id == tasks[0].id
                    && r.option == *option
                    && r.parametric_parameters.is_some()
            }) {
                return Err("Parameter state update is missing".into());
            }
        }
    }
    Ok(())
}
pub fn current_parameters(p: &Project, task_id: &str, option: u32) -> Result<Parameters, String> {
    let (task, index, rule) = candidate(p, task_id, option)?;
    let ids = &task.candidates[index].initial_mass_ids;
    let (expected,parameters)=match p.design_process.parametric_states.get(&option) {
        Some(State::Initial)=>(task.intent.as_ref().unwrap().initial_candidate_sha256[index].as_str(),&rule.candidates[index]),
        Some(State::Update{id})=>{let r=p.design_process.updates.iter().find(|r|r.id==*id&&r.task_id==task_id&&r.option==option).ok_or("Parameter state update is missing")?;
            if r.affected_ids!=*ids {return Err("Parameter state member mapping differs".into());}
            (r.after_sha256.as_str(),r.parametric_parameters.as_ref().ok_or("Parameter state update is missing")?)},
        _=>return Err("Current legacy parameters cannot be resolved uniquely; review without overwriting the model".into()),
    };
    if update::scope_hash(p, ids)? != expected {
        return Err(
            "Manual changes are protected; regeneration cannot replace these members".into(),
        );
    }
    elements_match(p, ids, parameters)?;
    Ok(parameters.clone())
}
pub fn current_inputs(p: &Project, task_id: &str, option: u32) -> Result<Inputs, String> {
    let (_, _, rule) = candidate(p, task_id, option)?;
    match p.design_process.parametric_states.get(&option) {
        Some(State::Initial) => Ok(rule.inputs()),
        Some(State::Update { id }) => {
            let record = p
                .design_process
                .updates
                .iter()
                .find(|r| r.id == *id && r.task_id == task_id && r.option == option)
                .ok_or("Parameter state update is missing")?;
            record_inputs(rule, task_id, &record.sources)
        }
        _ => Err("Current legacy parameter input version is unknown".into()),
    }
}
/// Capture is read-only. Callers must explicitly review this exact snapshot;
/// missing targets cannot be silently dropped or replaced with new identities.
pub fn capture_inputs(p: &Project, task_id: &str, option: u32) -> Result<Inputs, String> {
    let (_, _, rule) = candidate(p, task_id, option)?;
    let inputs = Inputs {
        site_sha256: hash(&p.site)?,
        sources: rule
            .sources
            .iter()
            .map(|s| {
                Ok(Link {
                    target: s.target.clone(),
                    baseline_sha256: s.target.current_hash(p).ok_or(
                        "A parametric source is missing; restore it before reviewing inputs",
                    )?,
                })
            })
            .collect::<Result<_, String>>()?,
    };
    inputs.validate_for(rule)?;
    Ok(inputs)
}
pub(super) fn record_inputs(
    rule: &Definition,
    task_id: &str,
    sources: &[update::Source],
) -> Result<Inputs, String> {
    match sources {
        [
            update::Source::ParametricDefinition {
                task_id: id,
                sha256,
            },
        ] if id == task_id && *sha256 == hash(rule)? => Ok(rule.inputs()),
        [
            update::Source::ReviewedParametricDefinition {
                task_id: id,
                sha256,
                inputs,
            },
        ] if id == task_id && *sha256 == hash(rule)? => {
            inputs.validate_for(rule)?;
            Ok(inputs.clone())
        }
        _ => Err("Parameter update scope or rule binding differs".into()),
    }
}
fn input_source(
    rule: &Definition,
    task_id: &str,
    inputs: Inputs,
) -> Result<update::Source, String> {
    Ok(if inputs == rule.inputs() {
        update::Source::ParametricDefinition {
            task_id: task_id.into(),
            sha256: hash(rule)?,
        }
    } else {
        update::Source::ReviewedParametricDefinition {
            task_id: task_id.into(),
            sha256: hash(rule)?,
            inputs,
        }
    })
}
/// Old v27 geometry-only selection did not persist an active version. Never guess
/// the last record if more than one parameter set describes the current members.
pub(crate) fn migrate_legacy_states(p: &mut Project) {
    let states = p
        .design_process
        .tasks
        .iter()
        .filter(|t| t.intent.as_ref().is_some_and(|v| v.parametric.is_some()))
        .flat_map(|t| t.candidates.iter().map(move |c| (&t.id, c.option)))
        .map(|(task, option)| (option, legacy_state(p, task, option)))
        .collect::<Vec<_>>();
    for (option, state) in states {
        p.design_process.parametric_states.insert(option, state);
    }
}
fn legacy_state(p: &Project, task_id: &str, option: u32) -> State {
    let Ok((task, index, rule)) = candidate(p, task_id, option) else {
        return State::UnknownLegacy;
    };
    let ids = &task.candidates[index].initial_mass_ids;
    let Ok(current) = update::scope_hash(p, ids) else {
        return State::UnknownLegacy;
    };
    let mut candidates = vec![];
    if task.intent.as_ref().unwrap().initial_candidate_sha256[index] == current {
        if elements_match(p, ids, &rule.candidates[index]).is_err() {
            return State::UnknownLegacy;
        }
        candidates.push((State::Initial, rule.candidates[index].clone()));
    }
    for r in p
        .design_process
        .updates
        .iter()
        .filter(|r| r.task_id == task_id && r.option == option && r.after_sha256 == current)
    {
        if let Some(parameters) = &r.parametric_parameters {
            if r.affected_ids != *ids || elements_match(p, ids, parameters).is_err() {
                return State::UnknownLegacy;
            }
            candidates.push((State::Update { id: r.id.clone() }, parameters.clone()));
        }
    }
    let Some((state, parameters)) = candidates.first() else {
        return State::UnknownLegacy;
    };
    if candidates.iter().any(|(_, other)| other != parameters) {
        State::UnknownLegacy
    } else {
        state.clone()
    }
}
pub(super) fn validate_saved_version(task: &Task, v: &Version) -> Result<(), String> {
    if let Some(inputs) = &v.parametric_inputs {
        if v.parametric_parameters.is_none() {
            return Err("Saved input versions require rule parameters".into());
        }
        let rule = task
            .intent
            .as_ref()
            .and_then(|i| i.parametric.as_ref())
            .ok_or("Saved input version has no rule definition")?;
        inputs.validate_for(rule)?;
    }
    if let Some(parameters) = &v.parametric_parameters {
        parameters.validate()?;
        let candidate = task
            .candidates
            .iter()
            .find(|c| c.option == v.option)
            .ok_or("Saved parameter candidate is missing")?;
        if task.intent.as_ref().is_none_or(|i| i.parametric.is_none())
            || candidate.initial_mass_ids.len() != 2
        {
            return Err("Saved parameters have no rule definition".into());
        }
        for (id, spec) in candidate
            .initial_mass_ids
            .iter()
            .zip(parameters.geometry()?)
        {
            if !v
                .elements
                .iter()
                .any(|e| e.id == *id && e.shape == Shape::ConceptMass { spec: spec.clone() })
            {
                return Err("Saved parameter version differs from its geometry".into());
            }
        }
    }
    Ok(())
}
pub fn readback(p: &Project) -> serde_json::Value {
    serde_json::json!(p.design_process.task().filter(|t|t.intent.as_ref().is_some_and(|v|v.parametric.is_some())).map(|t|t.candidates.iter().map(|c| {
        let inputs = current_inputs(p, &t.id, c.option);
        let status = inputs.as_ref().map_err(Clone::clone).and_then(|i| i.check_current(p));
        match current_parameters(p,&t.id,c.option){
            Ok(parameters)=>serde_json::json!({"option":c.option,"state":p.design_process.parametric_states.get(&c.option),"parameters":parameters,"status":"known", "input_version":inputs.ok(), "current_input_snapshot":capture_inputs(p,&t.id,c.option).ok(), "input_status":if status.is_ok(){"current"}else{"changed_or_missing"}, "input_diagnostic":status.err()}),
            Err(reason)=>serde_json::json!({"option":c.option,"status":"unknown","reason":reason})}
    }).collect::<Vec<_>>()))
}
pub fn prepare_update(
    editor: &Editor,
    id: &str,
    task_id: &str,
    parameters: Parameters,
    instruction: String,
) -> Result<PreparedChange, String> {
    prepare_update_with_entry(
        editor,
        id,
        task_id,
        parameters,
        instruction,
        Entry::ManualInterface,
    )
}
pub(crate) fn prepare_update_with_entry(
    editor: &Editor,
    id: &str,
    task_id: &str,
    parameters: Parameters,
    instruction: String,
    entry: Entry,
) -> Result<PreparedChange, String> {
    prepare_update_with_inputs(editor, id, task_id, parameters, instruction, None, entry)
}
pub fn prepare_reviewed_update(
    editor: &Editor,
    id: &str,
    task_id: &str,
    parameters: Parameters,
    instruction: String,
    inputs: Inputs,
) -> Result<PreparedChange, String> {
    prepare_update_with_inputs(
        editor,
        id,
        task_id,
        parameters,
        instruction,
        Some(inputs),
        Entry::ManualInterface,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_update_with_inputs(
    editor: &Editor,
    id: &str,
    task_id: &str,
    parameters: Parameters,
    instruction: String,
    reviewed_inputs: Option<Inputs>,
    entry: Entry,
) -> Result<PreparedChange, String> {
    let p = &editor.project;
    let option = p
        .plan_options
        .active_design_option
        .ok_or("Activate a parametric candidate")?;
    let (task, index, rule) = candidate(p, task_id, option)?;
    let inputs = reviewed_inputs.unwrap_or(current_inputs(p, task_id, option)?);
    inputs.validate_for(rule)?;
    inputs.check_current(p)?;
    current_parameters(p, task_id, option)?;
    let mut elements = vec![];
    for (id, spec) in task.candidates[index]
        .initial_mass_ids
        .iter()
        .zip(parameters.geometry()?)
    {
        let mut element = p
            .elements
            .iter()
            .find(|e| e.id == *id)
            .cloned()
            .ok_or("Parametric member is missing")?;
        element.shape = Shape::ConceptMass { spec };
        elements.push(element);
    }
    let draft = update::Draft {
        instruction,
        expected_result: "Regenerate only the two mapped concept wings".into(),
        sources: vec![input_source(rule, task_id, inputs)?],
        claims: vec![intent::Claim {
            kind: intent::ClaimKind::DesignCompletion,
            text: rule.assumptions.clone(),
        }],
        statements: vec![],
        elements,
        expected_comparison: task.compare(p)?.fingerprint,
        entry,
    };
    update::prepare_with_parameters(editor, None, id, task_id, draft, Some(parameters))
}
pub(super) fn validate_regeneration(
    before: &Project,
    after: &Project,
    record: &update::Record,
    parameters: &Parameters,
) -> Result<(), String> {
    let (task, index, rule) = candidate(before, &record.task_id, record.option)?;
    let inputs = record_inputs(rule, &record.task_id, &record.sources)?;
    inputs.check_current(before)?;
    if task.candidates[index]
        .initial_mass_ids
        .iter()
        .any(|id| crate::editor::Editor::element_is_locked(before, *id))
    {
        return Err("Task update is outside the editable current candidate".into());
    }
    if current_parameters(before, &record.task_id, record.option)? == *parameters
        && current_inputs(before, &record.task_id, record.option)? == inputs
    {
        return Err("Two-wing parameters are unchanged".into());
    }
    if record.affected_ids != task.candidates[index].initial_mass_ids {
        return Err("Parametric update scope or definition changed".into());
    }
    elements_match(after, &record.affected_ids, parameters)
}
