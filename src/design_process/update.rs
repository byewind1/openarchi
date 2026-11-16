//! Bounded task updates over existing candidate elements and the shared transaction.
//! Source bindings and interpretation are declarations, not proof of reading or approval.
use super::*;
use crate::{
    command::ModelCommand, editor::Editor, model_service::PreparedChange,
    project_repository::Repository,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Drawing {
        page: u64,
        sha256: String,
    },
    Repository {
        sha256: String,
    },
    ParametricDefinition {
        task_id: String,
        sha256: String,
    },
    ReviewedParametricDefinition {
        task_id: String,
        sha256: String,
        inputs: super::parametric::Inputs,
    },
}
impl Source {
    fn valid(&self) -> bool {
        match self {
            Self::Drawing { page, sha256 } => {
                *page != 0 && crate::storage::valid_document_identity(sha256)
            }
            Self::Repository { sha256 } => crate::storage::valid_document_identity(sha256),
            Self::ParametricDefinition { task_id, sha256 } => {
                crate::storage::valid_document_identity(task_id)
                    && crate::storage::valid_document_identity(sha256)
            }
            Self::ReviewedParametricDefinition {
                task_id,
                sha256,
                inputs,
            } => {
                crate::storage::valid_document_identity(task_id)
                    && crate::storage::valid_document_identity(sha256)
                    && inputs.validate().is_ok()
            }
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Drawing { page, sha256 } => format!("drawing:{page} · {sha256}"),
            Self::Repository { sha256 } => format!("repository:{sha256}"),
            Self::ParametricDefinition { task_id, sha256 } => {
                format!("parametric:{task_id}:{sha256}")
            }
            Self::ReviewedParametricDefinition {
                task_id,
                sha256,
                inputs,
            } => {
                format!(
                    "parametric:{task_id}:{sha256}:{}",
                    hash(inputs).unwrap_or_default()
                )
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric_parameters: Option<super::parametric::Parameters>,
    pub id: String,
    pub task_id: String,
    pub option: u32,
    pub instruction: String,
    pub expected_result: String,
    pub sources: Vec<Source>,
    pub claims: Vec<intent::Claim>,
    pub statements: Vec<Statement>,
    pub affected_ids: Vec<u64>,
    pub before_sha256: String,
    pub after_sha256: String,
    pub entry: Entry,
    pub created_unix: u64,
}
impl Record {
    pub(super) fn validate(&self, journal: &Journal) -> Result<(), String> {
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 16 * 1024
            || !crate::storage::valid_document_identity(&self.id)
            || !crate::storage::valid_document_identity(&self.before_sha256)
            || !crate::storage::valid_document_identity(&self.after_sha256)
            || self.before_sha256 == self.after_sha256 && self.parametric_parameters.is_none()
            || !text(&self.instruction, 4096, true)
            || !text(&self.expected_result, 4096, true)
            || !(1..=8).contains(&self.sources.len())
            || self.sources.iter().any(|s| !s.valid())
            || self
                .sources
                .iter()
                .map(Source::label)
                .collect::<BTreeSet<_>>()
                .len()
                != self.sources.len()
            || self.claims.is_empty()
            || self.claims.len() > 32
            || self.claims.iter().any(|c| !text(&c.text, 2048, true))
            || self.statements.len() > 32
            || self.statements.iter().any(|s| !text(&s.text, 2048, true))
            || !(1..=16).contains(&self.affected_ids.len())
            || self.affected_ids.iter().any(|id| *id == 0)
            || self.affected_ids.windows(2).any(|w| w[0] >= w[1])
            || !journal.tasks.iter().any(|t| {
                t.id == self.task_id && t.candidates.iter().any(|c| c.option == self.option)
            })
        {
            return Err("Invalid scoped task update".into());
        }
        if let Some(parameters) = &self.parametric_parameters {
            if !journal.tasks.iter().any(|t| {
                t.id == self.task_id && t.intent.as_ref().is_some_and(|v| v.parametric.is_some())
            }) {
                return Err("Parameter update requires a rule task".into());
            }
            parameters.validate()?;
            let task = journal
                .tasks
                .iter()
                .find(|t| t.id == self.task_id)
                .ok_or("Parameter task is missing")?;
            let rule = task
                .intent
                .as_ref()
                .and_then(|v| v.parametric.as_ref())
                .ok_or("Parameter update requires a rule task")?;
            let candidate = task
                .candidates
                .iter()
                .find(|c| c.option == self.option)
                .ok_or("Parameter candidate is missing")?;
            super::parametric::record_inputs(rule, &self.task_id, &self.sources)?;
            if self.affected_ids != candidate.initial_mass_ids {
                return Err("Parameter update scope or rule binding differs".into());
            }
        } else if self
            .sources
            .iter()
            .any(|s| matches!(s, Source::ReviewedParametricDefinition { .. }))
        {
            return Err("Reviewed parametric inputs require a parameter update".into());
        }
        Ok(())
    }
    pub fn effect(&self, project: &Project) -> &'static str {
        if self.parametric_parameters.is_some()
            && project.design_process.parametric_states.get(&self.option)
                != Some(&super::parametric::State::Update {
                    id: self.id.clone(),
                })
        {
            return "Task parameter update is historical, not the current version";
        }
        match scope_hash(project, &self.affected_ids) {
            Ok(hash) if hash == self.after_sha256 => "Task update matches current elements",
            Ok(hash) if hash == self.before_sha256 => "Task update is not applied",
            _ => "Task update elements changed or are missing",
        }
    }
    pub fn source_status(&self, p: &Project, repo: Option<&Repository>) -> &'static str {
        let mut unknown = false;
        for source in &self.sources {
            match source {
                Source::ParametricDefinition { task_id, sha256 } => {
                    let rule = p
                        .design_process
                        .tasks
                        .iter()
                        .find(|t| t.id == *task_id)
                        .and_then(|t| t.intent.as_ref())
                        .and_then(|v| v.parametric.as_ref());
                    if rule.is_none_or(|r| {
                        hash(r).ok().as_ref() != Some(sha256) || r.check_inputs(p).is_err()
                    }) {
                        return "Task update sources changed or are missing";
                    }
                }
                Source::ReviewedParametricDefinition { task_id, .. } => {
                    let rule = p
                        .design_process
                        .tasks
                        .iter()
                        .find(|t| t.id == *task_id)
                        .and_then(|t| t.intent.as_ref())
                        .and_then(|i| i.parametric.as_ref());
                    if rule.is_none_or(|rule| {
                        super::parametric::record_inputs(
                            rule,
                            task_id,
                            std::slice::from_ref(source),
                        )
                        .and_then(|inputs| inputs.check_current(p))
                        .is_err()
                    }) {
                        return "Task update sources changed or are missing";
                    }
                }
                Source::Drawing { page, sha256 } => {
                    if !p
                        .drawings
                        .pages
                        .iter()
                        .any(|x| x.id == *page && x.sha256 == *sha256)
                    {
                        return "Task update sources changed or are missing";
                    }
                }
                Source::Repository { sha256 } => match repo {
                    Some(repo)
                        if repo.manifest.document_id
                            == p.design_process
                                .tasks
                                .iter()
                                .find(|t| t.id == self.task_id)
                                .map(|t| t.source_document.as_str())
                                .unwrap_or("") =>
                    {
                        if !repo
                            .manifest
                            .sources
                            .iter()
                            .any(|s| s.id == *sha256 && s.blob.sha256 == *sha256)
                        {
                            return "Task update sources changed or are missing";
                        }
                    }
                    _ => unknown = true,
                },
            }
        }
        if unknown {
            "Task update repository sources need review"
        } else {
            "Task update source versions are catalogued"
        }
    }
}
pub fn scope_hash(project: &Project, ids: &[u64]) -> Result<String, String> {
    let elements = ids
        .iter()
        .map(|id| {
            project
                .elements
                .iter()
                .find(|e| e.id == *id)
                .ok_or("Task update element is missing")
        })
        .collect::<Result<Vec<_>, _>>()?;
    hash(&elements)
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub instruction: String,
    pub expected_result: String,
    pub sources: Vec<Source>,
    pub claims: Vec<intent::Claim>,
    pub statements: Vec<Statement>,
    pub elements: Vec<Element>,
    pub expected_comparison: String,
    pub entry: Entry,
}
pub fn prepare(
    editor: &Editor,
    repo: Option<&Repository>,
    id: &str,
    task_id: &str,
    draft: Draft,
) -> Result<PreparedChange, String> {
    prepare_with_parameters(editor, repo, id, task_id, draft, None)
}
pub(super) fn prepare_with_parameters(
    editor: &Editor,
    repo: Option<&Repository>,
    id: &str,
    task_id: &str,
    draft: Draft,
    parameters: Option<super::parametric::Parameters>,
) -> Result<PreparedChange, String> {
    let p = &editor.project;
    if p.plan_options.active_worksheet.is_some() {
        return Err("Return to the model before a scoped task update".into());
    }
    let task = p
        .design_process
        .task()
        .filter(|t| t.id == task_id)
        .ok_or("Current exploration changed")?;
    let option = p
        .plan_options
        .active_design_option
        .filter(|id| task.candidates.iter().any(|c| c.option == *id))
        .ok_or("Activate a candidate before its scoped update")?;
    if p.design_process.updates.iter().any(|r| r.id == id) {
        return Err("Task update already recorded; inspect its original operation".into());
    }
    if task.compare(p)?.fingerprint != draft.expected_comparison {
        return Err("Exploration comparison changed; refresh before updating".into());
    }
    let mut elements = draft.elements;
    elements.sort_by_key(|e| e.id);
    let ids = elements.iter().map(|e| e.id).collect::<Vec<_>>();
    let mut commands = Vec::new();
    for after in &elements {
        let before = p
            .elements
            .iter()
            .find(|e| e.id == after.id)
            .ok_or("Task update element is missing")?;
        if before.design_option != Some(option)
            || before.story != task.story
            || before.worksheet.is_some()
            || !task.accepts_shape(&before.shape)
            || !task.accepts_shape(&after.shape)
            || editor.locked_element(before.id)
        {
            return Err("Task update is outside the editable current candidate".into());
        }
        let mut expected = before.clone();
        expected.shape = after.shape.clone();
        if *after != expected || (before == after && parameters.is_none()) {
            return Err("Task update must change only declared existing geometry".into());
        }
        if before != after {
            commands.push(ModelCommand::UpdateElement {
                element: after.clone(),
            });
        }
    }
    let record = Record {
        parametric_parameters: parameters,
        id: id.into(),
        task_id: task_id.into(),
        option,
        instruction: draft.instruction,
        expected_result: draft.expected_result,
        sources: draft.sources,
        claims: draft.claims,
        statements: draft.statements,
        affected_ids: ids.clone(),
        before_sha256: scope_hash(p, &ids)?,
        after_sha256: hash(&elements)?,
        entry: draft.entry,
        created_unix: service::now(),
    };
    record.validate(&p.design_process)?;
    if record.source_status(p, repo) != "Task update source versions are catalogued" {
        return Err("Task update source versions are not available in this project".into());
    }
    commands.push(ModelCommand::DesignProcess {
        edit: Edit::RecordUpdate { record },
    });
    editor
        .prepare_change(editor.revision, commands)
        .map_err(|e| e.message)
}
pub(super) fn apply(p: &Project, journal: &mut Journal, record: &Record) -> Result<(), String> {
    record.validate(journal)?;
    if journal.current_task.as_deref() != Some(record.task_id.as_str())
        || p.plan_options.active_design_option != Some(record.option)
        || journal.updates.iter().any(|r| r.id == record.id)
        || scope_hash(p, &record.affected_ids)? != record.after_sha256
    {
        return Err("Task update scope or result changed".into());
    }
    let task = journal.task().ok_or("Current exploration changed")?;
    if p.elements
        .iter()
        .filter(|e| record.affected_ids.contains(&e.id))
        .any(|e| {
            e.design_option != Some(record.option)
                || e.story != task.story
                || e.worksheet.is_some()
                || !task.accepts_shape(&e.shape)
        })
    {
        return Err("Task update is outside the editable current candidate".into());
    }
    journal.updates.push(record.clone());
    if record.parametric_parameters.is_some() {
        journal.parametric_states.insert(
            record.option,
            super::parametric::State::Update {
                id: record.id.clone(),
            },
        );
    }
    Ok(())
}
pub fn summary(p: &Project) -> serde_json::Value {
    serde_json::json!({"count":p.design_process.updates.len(),"history":p.design_process.updates.iter().map(|r|serde_json::json!({"id":r.id,"task_id":r.task_id,"option":r.option,"affected_ids":r.affected_ids})).collect::<Vec<_>>(),"read_tool":"query_task_updates","interpretation":"immutable declared task/source history; not task authority or operation recovery"})
}
pub fn query(
    p: &Project,
    repo: Option<&Repository>,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, String> {
    if !(1..=4).contains(&limit) || offset > p.design_process.updates.len() {
        return Err("Invalid task update range".into());
    }
    let end = (offset + limit).min(p.design_process.updates.len());
    let comparison=p.design_process.task().map(|task|match task.compare(p) { Ok(c)=>serde_json::json!({"task_id":task.id,"fingerprint":c.fingerprint,"basis_current":c.basis_current,"active_option":p.plan_options.active_design_option,"candidates":c.candidates.iter().map(|v|serde_json::json!({"option":v.option,"name":v.name,"metrics":v.metrics,"mass_metrics":v.mass_metrics})).collect::<Vec<_>>()}),Err(message)=>serde_json::json!({"error":message}) });
    Ok(
        serde_json::json!({"comparison":comparison,"records":p.design_process.updates[offset..end].iter().map(|r|serde_json::json!({"record":r,"current_effect":r.effect(p),"source_status":r.source_status(p,repo)})).collect::<Vec<_>>(),"total":p.design_process.updates.len(),"next_offset":(end<p.design_process.updates.len()).then_some(end),"interpretation":"source bindings/claims are declarations, not proof of reading, approval or task authority; scope effects are current geometry, not overall operation recovery"}),
    )
}

// A parameter update may atomically append its exact pre-edit object question.
// Reuse the normal recorder so this exception cannot replace history or other evidence.
fn validate_bound_question(before: &Project, after: &Project, r: &Record) -> Result<(), String> {
    let changed = after
        .evidence
        .topics
        .iter()
        .filter(|topic| {
            before.evidence.topics.iter().find(|old| old.id == topic.id) != Some(*topic)
        })
        .collect::<Vec<_>>();
    if changed.len() != 1 {
        return Err("Parameter update requires exactly one bound object question".into());
    }
    let topic = changed[0];
    let previous = before
        .evidence
        .topics
        .iter()
        .find(|old| old.id == topic.id)
        .cloned();
    if topic.exchanges.len() != previous.as_ref().map_or(0, |old| old.exchanges.len()) + 1 {
        return Err("Parameter update cannot replace discussion history".into());
    }
    let exchange = topic
        .exchanges
        .last()
        .ok_or("Bound object question is missing")?;
    if exchange.question != r.instruction
        || exchange.scope.task.as_ref() != Some(&r.task_id)
        || exchange.scope.option != Some(r.option)
        || exchange
            .objects
            .iter()
            .any(|object| match &object.link.target {
                crate::evidence::Target::Element { id } => !r.affected_ids.contains(id),
                crate::evidence::Target::ProgramRow { .. } => false,
            })
    {
        return Err("Object question differs from the parameter update scope".into());
    }
    let mut expected = before.evidence.clone();
    crate::evidence::topics::record(before, &mut expected, &topic.id, exchange, &previous)?;
    if expected != after.evidence {
        return Err("Parameter update changes undeclared evidence or history".into());
    }
    Ok(())
}

/// Validate the complete prepared batch, including all geometry and header changes.
pub(crate) fn validate_transition(before: &Project, after: &Project) -> Result<(), String> {
    let added = after
        .design_process
        .updates
        .iter()
        .filter(|r| {
            !before
                .design_process
                .updates
                .iter()
                .any(|old| old.id == r.id)
        })
        .collect::<Vec<_>>();
    if added.is_empty() {
        return Ok(());
    }
    if added.len() != 1 {
        return Err("One scoped task update per transaction is required".into());
    }
    let r = added[0];
    if let Some(parameters) = &r.parametric_parameters {
        super::parametric::validate_regeneration(before, after, r, parameters)?;
    }
    let mut journal = after.design_process.clone();
    if r.parametric_parameters.is_some() {
        let mut expected = before.design_process.parametric_states.clone();
        expected.insert(
            r.option,
            super::parametric::State::Update { id: r.id.clone() },
        );
        if journal.parametric_states != expected {
            return Err("Task update changes undeclared parameter states".into());
        }
        journal.parametric_states = before.design_process.parametric_states.clone();
    }
    journal.updates.retain(|record| record.id != r.id);
    let mut header = crate::command::ProjectHeader::capture(after);
    header.design_process = before.design_process.clone();
    if before.evidence != after.evidence && r.parametric_parameters.is_some() {
        validate_bound_question(before, after, r)?;
        header.evidence = before.evidence.clone();
    }
    if journal != before.design_process
        || header != crate::command::ProjectHeader::capture(before)
        || before.elements.len() != after.elements.len()
        || before.design_process.current_task.as_ref() != Some(&r.task_id)
        || before.plan_options.active_design_option != Some(r.option)
        || scope_hash(before, &r.affected_ids)? != r.before_sha256
        || scope_hash(after, &r.affected_ids)? != r.after_sha256
    {
        return Err("Task update changes undeclared context or input".into());
    }
    let changed = before
        .elements
        .iter()
        .zip(&after.elements)
        .filter_map(|(old, new)| (old != new).then_some(old.id))
        .collect::<BTreeSet<_>>();
    let declared = r.affected_ids.iter().copied().collect::<BTreeSet<_>>();
    if if r.parametric_parameters.is_some() {
        !changed.is_subset(&declared)
    } else {
        changed != declared
    } {
        return Err("Task update changes undeclared elements".into());
    }
    for (old, new) in before.elements.iter().zip(&after.elements) {
        let mut expected = old.clone();
        expected.shape = new.shape.clone();
        if new != &expected || old.id != new.id {
            return Err("Task update must change only declared existing geometry".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod bound_question_tests {

    #[test]
    fn bound_question_batch_rejects_other_scope_question_history_and_evidence() {
        use crate::design_process::parametric::{self, Definition};
        let id = crate::storage::new_document_identity;
        use crate::{
            command::ModelCommand,
            editor::Editor,
            evidence::{EvidenceEdit, Target, topics},
        };
        let mut e = Editor::new(crate::model::Project::default());
        let a = crate::design_process::parametric::Parameters {
            origin_mm: [30000, 30000],
            court_width_mm: 20000,
            court_depth_mm: 14000,
            wing_depth_mm: 6000,
            storeys: 2,
            floor_height_mm: 3300,
        };
        let task_id = id();
        let mut b = a.clone();
        b.court_width_mm -= 1000;
        let definition = Definition {
            rule_version: 1,
            candidates: [a.clone(), b],
            site_sha256: crate::automation_journal::canonical_hash(&e.project.site).unwrap(),
            sources: vec![],
            assumptions: "Authored concept fixture; approval unknown".into(),
        };
        let create = parametric::prepare(
            &e,
            &task_id,
            "Rule".into(),
            "Explore".into(),
            definition,
            0,
            0,
        )
        .unwrap();
        e.commit_change("create", &create).unwrap();
        let task = e.project.design_process.task().unwrap().clone();
        let question = "Deepen current wings only";
        let exchange = topics::capture(
            &e.project,
            &[Target::Element {
                id: task.candidates[0].initial_mass_ids[0],
            }],
            question.into(),
        )
        .unwrap();
        let topic_id = id();
        let record = |tid: String, exchange: topics::Exchange, previous| ModelCommand::Evidence {
            edit: EvidenceEdit::RecordTopic {
                id: tid,
                exchange,
                expected_previous: previous,
            },
        };
        let first = e
            .prepare_change(
                e.revision,
                vec![record(topic_id.clone(), exchange.clone(), None)],
            )
            .unwrap();
        e.commit_change("first-question", &first).unwrap();
        let before = e.project.clone();
        let previous = before.evidence.topics[0].clone();
        let mut parameters = a;
        parameters.wing_depth_mm += 200;
        let update =
            parametric::prepare_update(&e, &id(), &task_id, parameters.clone(), question.into())
                .unwrap();
        let commands = |exchange, previous| {
            let mut commands = vec![record(topic_id.clone(), exchange, previous)];
            commands.extend_from_slice(update.commands());
            commands
        };
        let valid = e
            .prepare_change(
                e.revision,
                commands(exchange.clone(), Some(previous.clone())),
            )
            .unwrap();
        let mut wrong = exchange.clone();
        wrong.question = "Unrelated instruction".into();
        assert!(
            e.prepare_change(e.revision, commands(wrong, Some(previous.clone())))
                .is_err()
        );
        let other = topics::capture(
            &e.project,
            &[Target::Element {
                id: task.candidates[1].initial_mass_ids[0],
            }],
            question.into(),
        )
        .unwrap();
        assert!(
            e.prepare_change(e.revision, commands(other, Some(previous.clone())))
                .is_err(),
            "other candidate cannot be included"
        );
        let mut forged = exchange.clone();
        forged.objects[0].data["name"] = "forged".into();
        assert!(
            e.prepare_change(e.revision, commands(forged, Some(previous.clone())))
                .is_err()
        );
        let mut extra = commands(exchange.clone(), Some(previous.clone()));
        extra.insert(0, record(id(), exchange.clone(), None));
        assert!(
            e.prepare_change(e.revision, extra).is_err(),
            "no extra discussion can hitchhike"
        );
        let mut altered = previous.clone();
        altered.exchanges[0].question = "Replaced history".into();
        assert!(
            e.prepare_change(e.revision, commands(exchange.clone(), Some(altered)))
                .is_err()
        );
        assert_eq!(e.project, before, "all rejected previews are read-only");
        e.commit_change("bound-question-update", &valid).unwrap();
        assert_eq!(e.project.evidence.topics[0].exchanges.len(), 2);
        assert_eq!(
            e.project.evidence.topics[0].exchanges[0],
            previous.exchanges[0]
        );
        assert_eq!(e.project.evidence.topics[0].exchanges[1], exchange);
        let option = task.candidates[0].option;
        assert_eq!(
            parametric::current_parameters(&e.project, &task_id, option).unwrap(),
            parameters
        );
        let after = e.project.clone();
        e.undo();
        let mut expected = before.clone();
        expected.design_process = crate::design_process::Journal::restore_with_history(
            &before.design_process,
            &after.design_process,
        );
        assert_eq!(
            e.project, expected,
            "single undo restores live question and parameters; audit remains"
        );
    }
}
