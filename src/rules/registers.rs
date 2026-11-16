//! Cited lifecycle declarations and explicit verification needs. No legal verdict.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentRef {
    Recorded {
        source_id: String,
    },
    Missing {
        title: String,
        code_hint: String,
        edition_hint: String,
        jurisdiction_hint: String,
    },
}
impl Default for DocumentRef {
    fn default() -> Self {
        Self::Missing {
            title: String::new(),
            code_hint: String::new(),
            edition_hint: String::new(),
            jurisdiction_hint: String::new(),
        }
    }
}
impl DocumentRef {
    pub fn label(&self, book: &RuleBook) -> String {
        match self {
            Self::Recorded { source_id } => book
                .source(source_id)
                .map(|s| format!("{} · {} · {}", s.code, s.edition, s.title))
                .unwrap_or_else(|| source_id.clone()),
            Self::Missing {
                title, code_hint, ..
            } => format!("{title} · {code_hint}"),
        }
    }
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
    fn validate(&self, book: &RuleBook) -> Result<(), String> {
        match self {
            Self::Recorded { source_id } => {
                if book.source(source_id).is_none() {
                    return Err("Referenced rule original revision is missing".into());
                }
            }
            Self::Missing {
                title,
                code_hint,
                edition_hint,
                jurisdiction_hint,
            } => {
                if !bounded(title, 512, true)
                    || !bounded(code_hint, 256, false)
                    || !bounded(edition_hint, 256, false)
                    || !bounded(jurisdiction_hint, 512, false)
                {
                    return Err("Invalid requested original reference".into());
                }
            }
        }
        Ok(())
    }
    fn refers_to(&self, id: &str) -> bool {
        matches!(self,Self::Recorded{source_id} if source_id==id)
    }
    fn matches_source(&self, book: &RuleBook, id: &str) -> bool {
        match self {
            Self::Recorded { source_id } => book.edition_root(source_id) == book.edition_root(id),
            Self::Missing {
                code_hint,
                edition_hint,
                jurisdiction_hint,
                ..
            } => book.source(id).is_some_and(|s| {
                !code_hint.trim().is_empty()
                    && code_hint.split_whitespace().collect::<String>()
                        == s.code.split_whitespace().collect::<String>()
                    && (edition_hint.is_empty() || edition_hint == &s.edition)
                    && (jurisdiction_hint.is_empty() || jurisdiction_hint == &s.jurisdiction)
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleKind {
    Withdrawn,
    Replaced,
    Amended,
    Expires,
}
impl LifecycleKind {
    pub const ALL: [Self; 4] = [
        Self::Withdrawn,
        Self::Replaced,
        Self::Amended,
        Self::Expires,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Withdrawn => "Reported withdrawal",
            Self::Replaced => "Reported replacement",
            Self::Amended => "Reported amendment",
            Self::Expires => "Reported expiry",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleScope {
    Whole,
    Partial,
    Unresolved,
}
impl LifecycleScope {
    pub const ALL: [Self; 3] = [Self::Whole, Self::Partial, Self::Unresolved];
    pub fn label(self) -> &'static str {
        match self {
            Self::Whole => "Declared whole-document scope",
            Self::Partial => "Declared partial scope",
            Self::Unresolved => "Lifecycle scope unresolved",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    Instrument,
    Explanation,
    Report,
}
impl EvidenceRole {
    pub const ALL: [Self; 3] = [Self::Instrument, Self::Explanation, Self::Report];
    pub fn label(self) -> &'static str {
        match self {
            Self::Instrument => "Declared original instrument",
            Self::Explanation => "Declared official explanation",
            Self::Report => "Unverified lifecycle report",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleRecord {
    pub id: String,
    pub title: String,
    pub subject: DocumentRef,
    pub successor: Option<DocumentRef>,
    pub kind: LifecycleKind,
    pub scope: LifecycleScope,
    pub effective_on: String,
    pub role: EvidenceRole,
    pub citation: Citation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corroboration: Vec<Citation>,
    pub scope_note: String,
    pub missing: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConditionStatus {
    Missing,
    Review,
    Addressed,
    NotApplicable,
}
impl ConditionStatus {
    pub const ALL: [Self; 4] = [
        Self::Missing,
        Self::Review,
        Self::Addressed,
        Self::NotApplicable,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Missing => "Missing condition evidence",
            Self::Review => "Condition evidence under review",
            Self::Addressed => "Declared condition addressed",
            Self::NotApplicable => "Declared condition not applicable",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConditionNeed {
    pub id: String,
    pub title: String,
    pub topic: Topic,
    pub question: String,
    pub originals: Vec<DocumentRef>,
    pub requirement_ids: Vec<String>,
    pub citation: Option<Citation>,
    pub status: ConditionStatus,
    pub rationale: String,
    pub missing: String,
    pub basis_sha256: String,
}
impl RuleBook {
    pub fn lifecycle(&self, id: &str) -> Option<&LifecycleRecord> {
        self.lifecycle.iter().find(|r| r.id == id)
    }
    pub fn condition(&self, id: &str) -> Option<&ConditionNeed> {
        self.conditions.iter().find(|r| r.id == id)
    }
    pub fn validate_citation(&self, c: &Citation) -> Result<(), String> {
        if self.source(&c.source_id).is_none()
            || !bounded(&c.locator, 512, true)
            || !bounded(&c.text, 16384, true)
        {
            return Err("Invalid exact lifecycle or condition citation".into());
        }
        Ok(())
    }
    pub(super) fn lifecycle_for(&self, id: &str) -> Vec<&LifecycleRecord> {
        let mut rows = self
            .lifecycle
            .iter()
            .filter(|r| r.subject.matches_source(self, id))
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        rows
    }
    pub fn lifecycle_state(&self, r: &LifecycleRecord) -> &'static str {
        if r.subject.is_missing() {
            return "Lifecycle subject original not acquired";
        }
        if r.role != EvidenceRole::Instrument {
            return "Lifecycle report needs the original instrument";
        }
        if r.effective_on.is_empty() {
            return "Lifecycle effective date is unknown";
        }
        if self.context.assessment_date.is_empty() {
            return "Assessment date is unknown";
        }
        if r.effective_on > self.context.assessment_date {
            return "Lifecycle event is after the assessment date";
        }
        if r.scope != LifecycleScope::Whole {
            return "Partial or unresolved lifecycle scope needs review";
        }
        if r.successor.as_ref().is_some_and(DocumentRef::is_missing) {
            return "Lifecycle successor original not acquired";
        }
        match r.kind {
            LifecycleKind::Withdrawn => "Declared withdrawn on assessment date",
            LifecycleKind::Replaced => "Declared replaced on assessment date",
            LifecycleKind::Expires => "Declared expired on assessment date",
            LifecycleKind::Amended => "Amendment scope needs clause review",
        }
    }
    pub fn lifecycle_restriction(&self, id: &str) -> Option<&'static str> {
        self.lifecycle_for(id).into_iter().find_map(|r| {
            if r.subject.is_missing() {
                return Some("Potential lifecycle target is not bound to this original");
            }
            let state = self.lifecycle_state(r);
            if state == "Lifecycle event is after the assessment date" {
                None
            } else {
                Some(state)
            }
        })
    }
    pub fn condition_basis(&self, r: &ConditionNeed) -> Result<String, String> {
        crate::automation_journal::canonical_hash(
            &serde_json::json!({"context":self.context,"sources":self.sources,"requirements":self.requirements,"lifecycle":self.lifecycle,"criterion":{"id":r.id,"title":r.title,"topic":r.topic,"question":r.question,"originals":r.originals,"requirements":r.requirement_ids,"citation":r.citation}}),
        )
    }
    pub fn condition_state(&self, project: &Project, r: &ConditionNeed) -> &'static str {
        if self.condition_basis(r).ok().as_ref() != Some(&r.basis_sha256) {
            return "Condition review is stale";
        }
        if r.originals.iter().any(DocumentRef::is_missing) {
            return "Condition original not acquired";
        }
        if matches!(
            r.status,
            ConditionStatus::Addressed | ConditionStatus::NotApplicable
        ) {
            if !r.missing.trim().is_empty() {
                return "Condition still reports missing information";
            }
            if !self.context.complete() {
                return "Project applicability context is incomplete";
            }
            if r.requirement_ids.iter().any(|id| {
                self.requirements
                    .iter()
                    .find(|req| &req.id == id)
                    .is_none_or(|req| !self.requirement_current_for_review(project, req))
            }) {
                return "Linked requirements changed or remain unreviewed";
            }
        }
        r.status.label()
    }
    fn requirement_current_for_review(&self, project: &Project, r: &Requirement) -> bool {
        self.requirement_state(project, r) == r.decision.applicability.label()
            && matches!(
                (r.decision.applicability, r.decision.adoption),
                (Applicability::Applicable, Adoption::Adopted)
                    | (Applicability::NotApplicable, Adoption::NotAdopted)
            )
    }
    pub fn validate_edit_against_project(
        &self,
        project: &Project,
        edit: &RuleEdit,
    ) -> Result<(), String> {
        if let RuleEdit::PutCondition { condition, .. } = edit
            && matches!(
                condition.status,
                ConditionStatus::Addressed | ConditionStatus::NotApplicable
            )
            && self.condition_state(project, condition) != condition.status.label()
        {
            return Err("Condition declaration has stale, missing or unreviewed evidence".into());
        }
        Ok(())
    }
    pub(super) fn referenced_by_registers(&self, id: &str) -> bool {
        self.lifecycle.iter().any(|r| {
            r.citation.source_id == id
                || r.corroboration.iter().any(|c| c.source_id == id)
                || r.subject.refers_to(id)
                || r.successor.as_ref().is_some_and(|d| d.refers_to(id))
        }) || self.conditions.iter().any(|r| {
            r.citation.as_ref().is_some_and(|c| c.source_id == id)
                || r.originals.iter().any(|d| d.refers_to(id))
        })
    }
    pub(super) fn validate_registers(&self) -> Result<(), String> {
        if self.lifecycle.len() > 1000 || self.conditions.len() > 1000 {
            return Err("Lifecycle or condition register exceeds1000 records".into());
        }
        let mut ids = BTreeSet::new();
        for r in &self.lifecycle {
            if !identity(&r.id)
                || !ids.insert(&r.id)
                || !bounded(&r.title, 512, true)
                || !bounded(&r.scope_note, 4096, true)
                || !bounded(&r.missing, 4096, false)
                || !date(&r.effective_on)
            {
                return Err("Invalid lifecycle record, scope note or effective date".into());
            }
            self.validate_citation(&r.citation)?;
            if r.corroboration.len() > 16 {
                return Err("Lifecycle evidence exceeds16 corroborating citations".into());
            }
            for c in &r.corroboration {
                self.validate_citation(c)?;
            }
            r.subject.validate(self)?;
            if let Some(d) = &r.successor {
                d.validate(self)?;
            }
            if (r.kind == LifecycleKind::Replaced && r.successor.is_none())
                || (matches!(r.kind, LifecycleKind::Withdrawn | LifecycleKind::Expires)
                    && r.successor.is_some())
            {
                return Err("Lifecycle successor does not match the event kind".into());
            }
            if (r.role != EvidenceRole::Instrument
                || r.scope == LifecycleScope::Unresolved
                || r.effective_on.is_empty()
                || r.subject.is_missing()
                || r.successor.as_ref().is_some_and(DocumentRef::is_missing))
                && r.missing.trim().is_empty()
            {
                return Err(
                    "Unresolved lifecycle evidence needs explicit missing information".into(),
                );
            }
        }
        let mut graph = std::collections::BTreeMap::<String, Vec<String>>::new();
        for s in &self.sources {
            graph
                .entry(self.edition_root(&s.id).into())
                .or_default()
                .extend(s.replaces.iter().map(|id| self.edition_root(id).into()));
        }
        for r in &self.lifecycle {
            if r.kind == LifecycleKind::Replaced
                && let (
                    DocumentRef::Recorded { source_id: old },
                    Some(DocumentRef::Recorded { source_id: new }),
                ) = (&r.subject, &r.successor)
            {
                let old = self.edition_root(old).to_owned();
                let new = self.edition_root(new).to_owned();
                if old == new {
                    return Err("Lifecycle replacement cannot replace the same edition".into());
                }
                graph.entry(new).or_default().push(old);
            }
        }
        for start in graph.keys() {
            let mut stack = graph[start].clone();
            let mut seen = BTreeSet::new();
            while let Some(id) = stack.pop() {
                if &id == start {
                    return Err("Cyclic cited lifecycle replacement".into());
                }
                if seen.insert(id.clone()) {
                    stack.extend(graph.get(&id).into_iter().flatten().cloned())
                }
            }
        }
        ids.clear();
        for r in &self.conditions {
            if !identity(&r.id)
                || !ids.insert(&r.id)
                || !bounded(&r.title, 512, true)
                || !bounded(&r.question, 4096, true)
                || !bounded(&r.rationale, 4096, false)
                || !bounded(&r.missing, 4096, false)
                || !crate::storage::valid_document_identity(&r.basis_sha256)
                || r.originals.len() > 32
                || r.requirement_ids.len() > 100
            {
                return Err("Invalid condition verification need".into());
            }
            for d in &r.originals {
                d.validate(self)?
            }
            if let Some(c) = &r.citation {
                self.validate_citation(c)?
            }
            let mut linked = BTreeSet::new();
            for id in &r.requirement_ids {
                if !linked.insert(id) || !self.requirements.iter().any(|req| &req.id == id) {
                    return Err("Condition references a missing or duplicate requirement".into());
                }
            }
            if matches!(r.status, ConditionStatus::Missing | ConditionStatus::Review)
                && r.missing.trim().is_empty()
            {
                return Err("Unresolved condition needs explicit missing information".into());
            }
            // Structural validation preserves old declarations after inputs change; current
            // admission is checked on PutCondition, and reads expose a stale review.
            if matches!(
                r.status,
                ConditionStatus::Addressed | ConditionStatus::NotApplicable
            ) && (r.rationale.trim().is_empty()
                || (r.status == ConditionStatus::Addressed && r.requirement_ids.is_empty())
                || (r.status == ConditionStatus::NotApplicable
                    && r.citation.is_none()
                    && r.requirement_ids.is_empty()))
            {
                return Err("Condition declaration needs cited evidence and rationale".into());
            }
        }
        Ok(())
    }
    pub fn lifecycle_entry(&self, r: &LifecycleRecord) -> serde_json::Value {
        serde_json::json!({"record":r,"state":self.lifecycle_state(r),"declarations_only":true})
    }
    pub fn condition_entry(&self, project: &Project, r: &ConditionNeed) -> serde_json::Value {
        serde_json::json!({"condition":r,"state":self.condition_state(project,r),"declarations_only":true})
    }
    pub fn coverage(&self, project: &Project) -> Vec<serde_json::Value> {
        Topic::ALL.iter().map(|topic|{let needs=self.conditions.iter().filter(|n|n.topic==*topic).collect::<Vec<_>>();let req_count=self.requirements.iter().filter(|r|r.topic==*topic).count();serde_json::json!({"topic":topic,"recorded_requirements":req_count,"condition_needs":needs.len(),"unresolved_conditions":needs.iter().filter(|n|self.condition_state(project,n)!=n.status.label()||matches!(n.status,ConditionStatus::Missing|ConditionStatus::Review)).count(),"status":if req_count==0&&needs.is_empty(){"missing"}else{"recorded_not_verified"},"coverage_is_exhaustive":false})}).collect()
    }
}
