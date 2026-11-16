//! An exact source recheck, not evidence of authority or client approval.
use super::*;
use crate::evidence::Quote;
pub struct EvidenceCommitGuard {
    _lock: Lock,
}
impl Repository {
    pub fn guard_evidence_commit(&self, quote: &Quote) -> Result<EvidenceCommitGuard, String> {
        self.guard_evidence_quotes(std::slice::from_ref(quote))
    }
    pub fn guard_evidence_quotes(&self, quotes: &[Quote]) -> Result<EvidenceCommitGuard, String> {
        let lock = Lock::acquire(&self.root)?;
        self.verify_evidence_unlocked(quotes)?;
        Ok(EvidenceCommitGuard { _lock: lock })
    }
    pub(super) fn verify_evidence_unlocked(&self, quotes: &[Quote]) -> Result<(), String> {
        self.unchanged()?;
        if quotes.is_empty() || quotes.len() > 200 {
            return Err("Choose between 1 and 200 source quotes".into());
        }
        let mut by_source: std::collections::BTreeMap<&str, Vec<&Quote>> = Default::default();
        for quote in quotes {
            quote.validate()?;
            by_source
                .entry(&quote.source_sha256)
                .or_default()
                .push(quote);
        }
        for (source_id, quotes) in by_source {
            let source = self
                .manifest
                .sources
                .iter()
                .find(|s| s.id == source_id && s.format == "docx")
                .ok_or("Choose a stored DOCX original for the source note")?;
            if matches!(source.quality, SourceQuality::Rejected { .. }) {
                return Err("Rejected source cannot supply source notes".into());
            }
            let bytes = resource(&self.root, &source.blob, 32 * 1024 * 1024)?;
            let document = crate::source_document::inspect_docx(&bytes)?;
            for quote in quotes {
                if document.quote(&quote.locator) != Some(quote.text.as_str()) {
                    return Err("Source quote differs from its original locator/text".into());
                }
            }
        }
        Ok(())
    }
}
