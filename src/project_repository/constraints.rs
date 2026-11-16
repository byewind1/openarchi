//! Short-lived source admission for cited numerical runs. Never a write grant.
use super::*;
use crate::{
    command::ModelCommand,
    constraints::{Basis, Edit},
    rules::{Citation, RuleBook},
};
use std::sync::{Arc, Weak};

pub struct ConstraintCommitGuard(Arc<Admission>);
pub(crate) struct Admission {
    _lock: Lock,
    document_id: String,
    book_sha256: String,
    commands: Vec<ModelCommand>,
}
impl Admission {
    pub(crate) fn verify(&self, editor: &Editor, commands: &[ModelCommand]) -> Result<(), String> {
        if self.document_id != editor.project_identity()
            || self.book_sha256 != crate::automation_journal::canonical_hash(&editor.project.rules)?
            || self.commands != commands
        {
            return Err(
                "Cited original admission does not match this document or command batch".into(),
            );
        }
        Ok(())
    }
}
impl ConstraintCommitGuard {
    pub(crate) fn verify(&self, editor: &Editor, commands: &[ModelCommand]) -> Result<(), String> {
        self.0.verify(editor, commands)
    }
    pub(crate) fn lease(&self) -> Weak<Admission> {
        Arc::downgrade(&self.0)
    }
}

/// Resolve sequential Put/Run/Remove without evaluating geometry or cloning the
/// model. A definition changed earlier in this batch cannot bypass admission.
pub fn cited_run_citations(
    book: &RuleBook,
    commands: &[ModelCommand],
) -> Result<Vec<Citation>, String> {
    let mut definitions = book
        .checks
        .iter()
        .map(|c| (c.definition.id.clone(), c.definition.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut citations = Vec::new();
    for command in commands {
        let ModelCommand::Constraint { edit } = command else {
            continue;
        };
        match edit {
            Edit::Put { definition, .. } => {
                definitions.insert(definition.id.clone(), definition.clone());
            }
            Edit::Remove { id, .. } => {
                definitions.remove(id);
            }
            Edit::Run { id, .. } => {
                let definition = definitions.get(id).ok_or("Quantified check is missing")?;
                if let Basis::CitedRequirement { id } = &definition.basis {
                    let citation = &book
                        .requirements
                        .iter()
                        .find(|r| r.id == *id)
                        .ok_or("Cited requirement is missing")?
                        .citation;
                    if !citations.contains(citation) {
                        citations.push(citation.clone());
                    }
                }
            }
        }
    }
    Ok(citations)
}

/// Call on the existing source worker. Keep this guard only through an immediate
/// prepare/commit; dropping it invalidates every prepared lease and releases the
/// repository lock. A user-facing preview never owns a strong lease.
pub fn guard_cited_runs(
    repository: &Repository,
    document_id: &str,
    book: &RuleBook,
    commands: &[ModelCommand],
) -> Result<ConstraintCommitGuard, String> {
    if repository.manifest.document_id != document_id {
        return Err("Project repository belongs to another document".into());
    }
    let citations = cited_run_citations(book, commands)?;
    if citations.is_empty() {
        return Err("No cited numerical run requires admission".into());
    }
    let lock = Lock::acquire(&repository.root)?;
    for result in repository.verify_rule_citations_unlocked(book, &citations)? {
        result?;
    }
    Ok(ConstraintCommitGuard(Arc::new(Admission {
        _lock: lock,
        document_id: document_id.into(),
        book_sha256: crate::automation_journal::canonical_hash(book)?,
        commands: commands.to_vec(),
    })))
}
