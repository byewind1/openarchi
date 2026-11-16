//! Static-original inspection and exact quote guards; authority remains a declaration.
use super::*;
use crate::rules::{Citation, RuleBook, RuleEdit};
use crate::source_document::{Paragraph, SourceDocument};
pub mod pdf;

pub struct RuleCommitGuard {
    _lock: Lock,
    document_id: String,
    book_sha256: String,
    edit_sha256: String,
}
impl RuleCommitGuard {
    pub(crate) fn verify(
        &self,
        editor: &crate::editor::Editor,
        edit: &RuleEdit,
    ) -> Result<(), String> {
        if editor.project_identity() != self.document_id
            || crate::automation_journal::canonical_hash(&editor.project.rules)? != self.book_sha256
            || crate::automation_journal::canonical_hash(edit)? != self.edit_sha256
        {
            return Err("Rule source admission does not match this document or edit".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct RuleOriginal {
    pub source_sha256: String,
    pub document: SourceDocument,
    pub limits: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf: Option<pdf::PdfTextEvidence>,
}
pub fn inspect(bytes: &[u8], format: &str) -> Result<SourceDocument, String> {
    if format == "pdf" {
        return pdf::inspect(bytes).map(|(doc, _)| doc);
    }
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Rule original exceeds 2 MiB text inspection limit".into());
    }
    if format == "docx" {
        return crate::source_document::inspect_docx(bytes);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "Rule text original requires UTF-8")?;
    let paragraphs = if matches!(format, "html" | "htm") {
        let document = scraper::Html::parse_document(text);
        let selector = scraper::Selector::parse("p,h1,h2,h3,h4,h5,h6,li,td,th").unwrap();
        let mut paragraphs = Vec::new();
        for (index, element) in document.select(&selector).enumerate() {
            let text = element
                .descendants()
                .filter_map(|node| {
                    let t = node.value().as_text()?;
                    if node
                        .ancestors()
                        .filter_map(scraper::ElementRef::wrap)
                        .any(|a| {
                            matches!(
                                a.value().name(),
                                "script" | "style" | "template" | "noscript"
                            )
                        })
                    {
                        return None;
                    }
                    Some(t.to_string())
                })
                .collect::<String>();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                paragraphs.push(Paragraph {
                    locator: format!("html/static[{}]", index + 1),
                    text,
                });
            }
        }
        paragraphs
    } else if matches!(format, "txt" | "md") {
        text.lines()
            .enumerate()
            .map(|(i, t)| Paragraph {
                locator: format!("text/line[{}]", i + 1),
                text: t.trim_end_matches('\r').into(),
            })
            .collect()
    } else {
        return Err("Choose a stored DOCX, PDF, UTF-8 text or static HTML rule original".into());
    };
    if paragraphs.len() > 10_000 || paragraphs.iter().any(|p| p.text.len() > 16_384) {
        return Err("Rule original exceeds paragraph inspection limit".into());
    }
    if !paragraphs.iter().any(|p| !p.text.trim().is_empty()) {
        return Err("Rule original has no supported static text".into());
    }
    Ok(SourceDocument {
        paragraphs,
        tables: vec![],
    })
}
impl Repository {
    fn rule_original_unlocked(&self, id: &str, admit: bool) -> Result<RuleOriginal, String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id)
            .ok_or("Rule original is not stored in this repository")?;
        if admit && matches!(source.quality, SourceQuality::Rejected { .. }) {
            return Err("Rejected source cannot supply rule citations".into());
        }
        if source.id != source.blob.sha256 {
            return Err("Rule original identity differs from its stored blob".into());
        }
        let bytes = resource(&self.root, &source.blob, 32 * 1024 * 1024)?;
        let (document, pdf) = if source.format == "pdf" {
            let (doc, proof) = pdf::inspect(&bytes)?;
            (doc, Some(proof))
        } else {
            (inspect(&bytes, &source.format)?, None)
        };
        Ok(RuleOriginal{source_sha256:id.into(),document,pdf,limits:"Static source text only; not page layout, visibility, authority, current legal effect or approval".into()})
    }
    pub fn inspect_rule_original(&self, id: &str) -> Result<RuleOriginal, String> {
        let _lock = Lock::acquire(&self.root)?;
        self.rule_original_unlocked(id, false)
    }
    fn rule_citation_unlocked(&self, book: &RuleBook, citation: &Citation) -> Result<(), String> {
        let source = book
            .source(&citation.source_id)
            .ok_or("Rule original revision is missing")?;
        let original = self.rule_original_unlocked(&source.original_sha256, true)?;
        if original.document.quote(&citation.locator) != Some(citation.text.as_str()) {
            return Err("Rule citation differs from its stored original locator/text".into());
        }
        Ok(())
    }
    pub fn guard_rule_edit(
        &self,
        book: &RuleBook,
        edit: &RuleEdit,
    ) -> Result<RuleCommitGuard, String> {
        let lock = Lock::acquire(&self.root)?;
        self.verify_rule_unlocked(book, edit)?;
        Ok(RuleCommitGuard {
            _lock: lock,
            document_id: self.manifest.document_id.clone(),
            book_sha256: crate::automation_journal::canonical_hash(book)?,
            edit_sha256: crate::automation_journal::canonical_hash(edit)?,
        })
    }
    pub(super) fn verify_rule_unlocked(
        &self,
        book: &RuleBook,
        edit: &RuleEdit,
    ) -> Result<(), String> {
        self.unchanged()?;
        let next = crate::rules::apply(book, edit)?;
        match edit {
            RuleEdit::AddSource { source } => {
                self.rule_original_unlocked(&source.original_sha256, true)?;
            }
            RuleEdit::PutLifecycle { record, .. } => {
                self.rule_citation_unlocked(&next, &record.citation)?;
                for c in &record.corroboration {
                    self.rule_citation_unlocked(&next, c)?;
                }
            }
            RuleEdit::PutCondition { condition, .. } => {
                if let Some(c) = &condition.citation {
                    self.rule_citation_unlocked(&next, c)?;
                }
                for id in &condition.requirement_ids {
                    let r = next
                        .requirements
                        .iter()
                        .find(|r| &r.id == id)
                        .ok_or("Condition requirement is missing")?;
                    self.rule_citation_unlocked(&next, &r.citation)?;
                }
            }
            RuleEdit::PutRequirement { requirement, .. } => {
                self.rule_citation_unlocked(&next, &requirement.citation)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn verify_rule_citation(&self, book: &RuleBook, citation: &Citation) -> Result<(), String> {
        let _lock = Lock::acquire(&self.root)?;
        self.rule_citation_unlocked(book, citation)
    }
    /// Bounded callers run this on the existing background source reader. One
    /// repository lock and one parse per original for a returned observation.
    pub fn verify_rule_citations(
        &self,
        book: &RuleBook,
        citations: &[Citation],
    ) -> Result<Vec<Result<(), String>>, String> {
        if citations.len() > 128 {
            return Err("Original verification exceeds 128 citations".into());
        }
        let _lock = Lock::acquire(&self.root)?;
        self.verify_rule_citations_unlocked(book, citations)
    }
    pub(super) fn verify_rule_citations_unlocked(
        &self,
        book: &RuleBook,
        citations: &[Citation],
    ) -> Result<Vec<Result<(), String>>, String> {
        if citations.len() > 128 {
            return Err("Original verification exceeds 128 citations".into());
        }
        self.unchanged()?;
        let mut originals =
            std::collections::BTreeMap::<String, Result<RuleOriginal, String>>::new();
        let results = citations
            .iter()
            .map(|citation| {
                let source = book
                    .source(&citation.source_id)
                    .ok_or("Rule original revision is missing")?;
                let original = originals
                    .entry(source.original_sha256.clone())
                    .or_insert_with(|| self.rule_original_unlocked(&source.original_sha256, true));
                match original {
                    Err(error) => Err(error.clone()),
                    Ok(original)
                        if original.document.quote(&citation.locator)
                            == Some(citation.text.as_str()) =>
                    {
                        Ok(())
                    }
                    Ok(_) => {
                        Err("Rule citation differs from its stored original locator/text".into())
                    }
                }
            })
            .collect();
        self.unchanged()?;
        Ok(results)
    }
}
