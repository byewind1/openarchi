//! Declared source quotes and authored notes. Source truth/approval is never inferred.
use crate::model::Project;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub mod topic_tasks;
pub mod topics;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceKind")]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Question,
    Requirement,
    Observation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceQuote")]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub source_sha256: String,
    /// Supported source is DOCX body XML; not a rendered page/visibility locator.
    pub locator: String,
    pub text: String,
}
impl Quote {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::storage::valid_document_identity(&self.source_sha256)
            || !self.locator.starts_with("body/")
            || self.locator.len() > 512
            || self.text.trim().is_empty()
            || self.text.len() > 16_384
        {
            return Err("Invalid or empty source quote".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceTarget")]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Element { id: u64 },
    ProgramRow { sheet_id: String, key: String },
}
impl Target {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Element { id } if *id == 0 => Err("Invalid evidence element target".into()),
            Self::ProgramRow { sheet_id, key }
                if sheet_id.trim().is_empty()
                    || sheet_id.len() > 256
                    || key.trim().is_empty()
                    || key.len() > 256 =>
            {
                Err("Invalid evidence program target".into())
            }
            _ => Ok(()),
        }
    }
    pub fn current_hash(&self, project: &Project) -> Option<String> {
        match self {
            Self::Element { id } => project
                .elements
                .iter()
                .find(|e| e.id == *id)
                .and_then(|e| crate::automation_journal::canonical_hash(e).ok()),
            Self::ProgramRow { sheet_id, key } => project
                .program
                .sheets
                .iter()
                .find(|s| s.id == *sheet_id)
                .and_then(|s| s.rows.iter().find(|r| r.key == *key).map(|r| (s, r)))
                .and_then(|(s, r)| {
                    crate::automation_journal::canonical_hash(
                        &serde_json::json!({"source":s.source_sha256,"mapping":s.mapping,"row":r}),
                    )
                    .ok()
                }),
        }
    }
}
fn target_hashes<'a>(
    project: &Project,
    targets: impl Iterator<Item = &'a Target>,
) -> BTreeMap<Target, String> {
    let mut elements = BTreeSet::new();
    let mut rows: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for target in targets {
        match target {
            Target::Element { id } => {
                elements.insert(*id);
            }
            Target::ProgramRow { sheet_id, key } => {
                rows.entry(sheet_id).or_default().insert(key);
            }
        }
    }
    let mut hashes = BTreeMap::new();
    for element in &project.elements {
        if elements.contains(&element.id)
            && let Ok(hash) = crate::automation_journal::canonical_hash(element)
        {
            hashes.insert(Target::Element { id: element.id }, hash);
        }
    }
    for sheet in &project.program.sheets {
        if let Some(keys) = rows.get(sheet.id.as_str()) {
            for row in &sheet.rows {
                if keys.contains(row.key.as_str())
                    && let Ok(hash) = crate::automation_journal::canonical_hash(
                        &serde_json::json!({"source":sheet.source_sha256,"mapping":sheet.mapping,"row":row}),
                    )
                {
                    hashes.insert(
                        Target::ProgramRow {
                            sheet_id: sheet.id.clone(),
                            key: row.key.clone(),
                        },
                        hash,
                    );
                }
            }
        }
    }
    hashes
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceLink")]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub target: Target,
    pub baseline_sha256: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[schemars(rename = "EvidenceLinkStatus")]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    Unchanged,
    Changed,
    Missing,
}
impl Link {
    pub fn status(&self, project: &Project) -> LinkStatus {
        match self.target.current_hash(project) {
            None => LinkStatus::Missing,
            Some(hash) if hash == self.baseline_sha256 => LinkStatus::Unchanged,
            Some(_) => LinkStatus::Changed,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceNote")]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub quote: Quote,
    pub annotation: String,
    pub links: Vec<Link>,
}
impl Note {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty()
            || self.id.len() > 256
            || self.title.trim().is_empty()
            || self.title.len() > 256
            || self.annotation.len() > 4096
            || self.links.len() > 100
        {
            return Err("Invalid evidence identity, title, annotation or target count".into());
        }
        self.quote.validate()?;
        let mut targets = BTreeSet::new();
        for link in &self.links {
            link.target.validate()?;
            if !targets.insert(&link.target)
                || !crate::storage::valid_document_identity(&link.baseline_sha256)
            {
                return Err("Duplicate or invalid evidence link".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRegister {
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<topics::Topic>,
}
impl EvidenceRegister {
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"note_count":self.notes.len(),"topic_count":self.topics.len(),"topic_read":"query_source_evidence with topics=true","question_count":self.notes.iter().filter(|n|n.kind==Kind::Question).count(),"requirement_count":self.notes.iter().filter(|n|n.kind==Kind::Requirement).count(),"observation_count":self.notes.iter().filter(|n|n.kind==Kind::Observation).count(),"source_verification":"unverified declarations; not approval","read_tool":"query_source_evidence"})
    }
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty() && self.topics.is_empty()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.notes.len() > 200
            || self.topics.len() > 64
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024
        {
            return Err("Evidence register exceeds supported limits".into());
        }
        let mut ids = BTreeSet::new();
        let mut assignments = std::collections::BTreeSet::new();
        for topic in &self.topics {
            for exchange in &topic.exchanges {
                if let Some(a) = &exchange.assignment {
                    if !assignments.insert(&a.id) {
                        return Err("Repeated object task identity".into());
                    }
                }
            }
            topic.validate()?;
            if !ids.insert(&topic.id) {
                return Err("Duplicate discussion identity".into());
            }
        }
        for note in &self.notes {
            note.validate()?;
            if !ids.insert(&note.id) {
                return Err("Duplicate evidence identity".into());
            }
        }
        Ok(())
    }
    pub fn status(&self, project: &Project) -> serde_json::Value {
        let hashes = target_hashes(
            project,
            self.notes
                .iter()
                .flat_map(|n| n.links.iter().map(|l| &l.target)),
        );
        serde_json::json!({"source_verification":"unverified declarations; recheck original repository separately; not approval",
            "notes":self.notes.iter().map(|n|serde_json::json!({"id":n.id,"links":n.links.iter().map(|l|serde_json::json!({"target":l.target,"status":match hashes.get(&l.target) {None=>LinkStatus::Missing,Some(hash) if hash==&l.baseline_sha256=>LinkStatus::Unchanged,Some(_)=>LinkStatus::Changed}})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceDraft")]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub quote: Quote,
    pub annotation: String,
    pub targets: Vec<Target>,
    #[serde(default)]
    pub refresh_links: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceEdit {
    SubmitTopicTask {
        id: String,
        exchange: topics::Exchange,
        expected_previous: Option<topics::Topic>,
        request_id: String,
        mode: topic_tasks::Mode,
    },
    TopicTaskEvent {
        id: String,
        exchange_index: usize,
        expected_topic_sha256: String,
        request_id: String,
        event: topic_tasks::Event,
    },
    RecordTopicReply {
        id: String,
        exchange_index: usize,
        expected_topic_sha256: String,
        reply: topics::AgentReply,
    },
    RecordTopic {
        id: String,
        exchange: topics::Exchange,
        expected_previous: Option<topics::Topic>,
    },
    Put {
        draft: Draft,
        expected_previous: Option<Note>,
    },
    Remove {
        expected: Note,
    },
}
pub fn apply(project: &Project, edit: &EvidenceEdit) -> Result<EvidenceRegister, String> {
    let mut next = project.evidence.clone();
    match edit {
        EvidenceEdit::SubmitTopicTask {
            id,
            exchange,
            expected_previous,
            request_id,
            mode,
        } => topic_tasks::submit(
            project,
            &mut next,
            id,
            exchange,
            expected_previous,
            request_id,
            mode,
        )?,
        EvidenceEdit::TopicTaskEvent {
            id,
            exchange_index,
            expected_topic_sha256,
            request_id,
            event,
        } => topic_tasks::event(
            project,
            &mut next,
            id,
            *exchange_index,
            expected_topic_sha256,
            request_id,
            event,
        )?,
        EvidenceEdit::RecordTopicReply {
            id,
            exchange_index,
            expected_topic_sha256,
            reply,
        } => {
            topics::record_reply(
                project,
                &mut next,
                id,
                *exchange_index,
                expected_topic_sha256,
                reply,
            )?;
        }
        EvidenceEdit::RecordTopic {
            id,
            exchange,
            expected_previous,
        } => {
            topics::record(project, &mut next, id, exchange, expected_previous)?;
        }
        EvidenceEdit::Remove { expected } => {
            let index = next
                .notes
                .iter()
                .position(|n| n.id == expected.id)
                .ok_or("Source note no longer exists")?;
            if &next.notes[index] != expected {
                return Err("Source note changed; reload it".into());
            }
            next.notes.remove(index);
        }
        EvidenceEdit::Put {
            draft,
            expected_previous,
        } => {
            let previous = next.notes.iter().find(|n| n.id == draft.id);
            if previous != expected_previous.as_ref() {
                return Err("Source note changed; reload it".into());
            }
            if draft.targets.len() > 100 {
                return Err("Evidence note exceeds 100 targets".into());
            }
            let mut links = vec![];
            let hashes = target_hashes(project, draft.targets.iter());
            for target in &draft.targets {
                target.validate()?;
                let previous_link =
                    previous.and_then(|n| n.links.iter().find(|l| &l.target == target));
                let baseline_sha256 = if !draft.refresh_links
                    && let Some(link) = previous_link
                {
                    link.baseline_sha256.clone()
                } else {
                    hashes
                        .get(target)
                        .cloned()
                        .ok_or("Evidence target is missing; cannot establish a baseline")?
                };
                links.push(Link {
                    target: target.clone(),
                    baseline_sha256,
                });
            }
            let note = Note {
                id: draft.id.clone(),
                title: draft.title.clone(),
                kind: draft.kind,
                quote: draft.quote.clone(),
                annotation: draft.annotation.clone(),
                links,
            };
            note.validate()?;
            if let Some(index) = next.notes.iter().position(|n| n.id == draft.id) {
                next.notes[index] = note;
            } else {
                next.notes.push(note);
            }
        }
    }
    next.validate()?;
    Ok(next)
}
