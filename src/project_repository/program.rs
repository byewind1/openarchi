//! Functional-source reads and commit guard; the model stays host-owned.
use super::*;
use crate::program::{
    ImportProposal, Mapping, ProgramSheet,
    workbook::{self, Workbook},
};
pub struct ProgramCommitGuard {
    _lock: Lock,
}
impl Repository {
    fn program_source(&self, id: &str, admit: bool) -> Result<(String, Vec<u8>), String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id && matches!(s.format.as_str(), "csv" | "xlsx" | "docx"))
            .ok_or("Choose a stored CSV, XLSX or DOCX source")?;
        if admit && matches!(source.quality, SourceQuality::Rejected { .. }) {
            return Err("Rejected source cannot supply functional requirements".into());
        }
        Ok((
            source.format.clone(),
            resource(&self.root, &source.blob, 32 * 1024 * 1024)?,
        ))
    }
    pub fn inspect_program(&self, id: &str) -> Result<Workbook, String> {
        let _lock = Lock::acquire(&self.root)?;
        let (format, bytes) = self.program_source(id, false)?;
        workbook::inspect(&bytes, &format)
    }
    pub fn preview_program(
        &self,
        source: &str,
        id: String,
        name: String,
        mapping: Mapping,
        expected_previous: Option<ProgramSheet>,
    ) -> Result<ImportProposal, String> {
        let _lock = Lock::acquire(&self.root)?;
        let (format, bytes) = self.program_source(source, true)?;
        workbook::map(
            &workbook::inspect(&bytes, &format)?,
            source,
            id,
            name,
            mapping,
            expected_previous,
        )
    }
    pub fn guard_program_commit(
        &self,
        proposal: &ImportProposal,
    ) -> Result<ProgramCommitGuard, String> {
        let lock = Lock::acquire(&self.root)?;
        self.verify_program_unlocked(proposal)?;
        Ok(ProgramCommitGuard { _lock: lock })
    }
    pub(super) fn verify_program_unlocked(&self, proposal: &ImportProposal) -> Result<(), String> {
        let (format, bytes) = self.program_source(&proposal.sheet.source_sha256, true)?;
        let expected = workbook::map(
            &workbook::inspect(&bytes, &format)?,
            &proposal.sheet.source_sha256,
            proposal.sheet.id.clone(),
            proposal.sheet.name.clone(),
            proposal.sheet.mapping.clone(),
            proposal.expected_previous.clone(),
        )?;
        if &expected != proposal {
            return Err("Program proposal differs from its source cells".into());
        }
        Ok(())
    }
}
