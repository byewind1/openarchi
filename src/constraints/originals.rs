//! Revision-bound original observations. No model mutation or write permission.
use super::{Basis, Check};
use crate::{
    project_repository::Repository,
    rules::{Citation, RuleBook},
};
use serde_json::{Value, json};

pub struct ReadPlan {
    document_id: String,
    repository: Option<Repository>,
    book: RuleBook,
    requests: Vec<(String, String, Option<Citation>)>,
}
pub struct Observation {
    results: Vec<(String, String, Result<(), String>)>,
}
pub(crate) fn binding(book: &RuleBook, check: &Check) -> Option<String> {
    let Basis::CitedRequirement { id } = &check.definition.basis else {
        return None;
    };
    let citation = book
        .requirements
        .iter()
        .find(|r| r.id == *id)
        .map(|r| &r.citation);
    crate::automation_journal::canonical_hash(&(
        super::expected(check),
        citation,
        citation.and_then(|c| book.source(&c.source_id)),
    ))
    .ok()
}
impl ReadPlan {
    pub fn new<'a>(
        document_id: &str,
        repository: Option<&Repository>,
        book: &RuleBook,
        checks: impl Iterator<Item = &'a Check>,
    ) -> Self {
        let requests = checks
            .filter_map(|check| {
                let d = &check.definition;
                let Basis::CitedRequirement { id } = &d.basis else {
                    return None;
                };
                Some((
                    d.id.clone(),
                    binding(book, check).expect("validated check original binding"),
                    book.requirements
                        .iter()
                        .find(|r| r.id == *id)
                        .map(|r| r.citation.clone()),
                ))
            })
            .collect();
        Self {
            document_id: document_id.into(),
            repository: repository.cloned(),
            // Geometry and saved check reports do not cross to the IO worker.
            book: RuleBook {
                sources: book.sources.clone(),
                ..Default::default()
            },
            requests,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
    /// May parse originals/PDF text. Execute only on a source worker, never a GUI frame.
    pub fn execute(self) -> Observation {
        let citations = self
            .requests
            .iter()
            .filter_map(|(_, _, c)| c.clone())
            .collect::<Vec<_>>();
        let verified = match self.repository.as_ref() {
            None => Err("Open the bound project repository to verify cited originals".into()),
            Some(r) if r.manifest.document_id != self.document_id => {
                Err("Project repository belongs to another document".into())
            }
            Some(r) => r.verify_rule_citations(&self.book, &citations),
        };
        let mut verified_rows = verified.as_ref().ok().into_iter().flat_map(|v| v.iter());
        Observation {
            results: self
                .requests
                .into_iter()
                .map(|(id, binding, citation)| {
                    let result = if citation.is_none() {
                        Err("Cited requirement is missing".into())
                    } else if let Err(error) = &verified {
                        Err(error.clone())
                    } else {
                        verified_rows.next().cloned().unwrap_or_else(|| {
                            Err("Original verification result is incomplete".into())
                        })
                    };
                    (id, binding, result)
                })
                .collect(),
        }
    }
}
impl Observation {
    pub fn apply_to_entries(&self, entries: &mut [Value]) {
        for entry in entries {
            let Some(id) = entry["definition"]["id"].as_str() else {
                continue;
            };
            let Some((_, binding, result)) = self.results.iter().find(|(check, _, _)| check == id)
            else {
                continue;
            };
            if entry["original_verification_binding"].as_str() != Some(binding) {
                continue;
            }
            entry["original_verification"] = match result {
                Ok(()) => {
                    json!({"status":"verified","meaning":"Exact original bytes, not-rejected status and citation verified for this returned observation; authority and legal applicability are not certified"})
                }
                Err(reason) => {
                    json!({"status":"unavailable","reason":reason,"meaning":"Historical numerical values are retained; current cited result is unknown"})
                }
            };
            entry["state"] = if result.is_ok() {
                entry["numerical_state"].clone()
            } else {
                json!("unknown")
            };
        }
    }
}
