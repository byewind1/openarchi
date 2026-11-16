//! Functional requirements with immutable source baselines; independent of drawing worksheets.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub mod workbook;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AreaUnit {
    SquareMetre,
    SquareFoot,
}
impl AreaUnit {
    pub fn square_metres(self) -> f64 {
        match self {
            Self::SquareMetre => 1.,
            Self::SquareFoot => 0.09290304,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AreaBasis {
    RowTotal,
    PerUnit,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub sheet: String,
    pub first_row: u32,
    pub last_row: u32,
    pub key_column: u16,
    pub label_column: u16,
    pub area_column: Option<u16>,
    pub quantity_column: Option<u16>,
    #[serde(default)]
    pub scope_column: Option<u16>,
    #[serde(default)]
    pub note_column: Option<u16>,
    pub area_unit: AreaUnit,
    pub area_basis: AreaBasis,
}
impl Mapping {
    pub fn validate(&self) -> Result<(), String> {
        if self.sheet.trim().is_empty()
            || self.sheet.len() > 256
            || self.first_row == 0
            || self.last_row < self.first_row
            || self.last_row > 1_048_576
            || self.last_row - self.first_row >= 10000
        {
            return Err("Choose a named sheet and at most 10000 consecutive rows".into());
        }
        for c in [
            Some(self.key_column),
            Some(self.label_column),
            self.area_column,
            self.quantity_column,
            self.scope_column,
            self.note_column,
        ]
        .into_iter()
        .flatten()
        {
            if c == 0 || c > 16384 {
                return Err("Invalid program column".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Area {
    Known { m2: f64 },
    Unknown { reason: String },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Values {
    pub label: String,
    pub scope: String,
    pub note: String,
    pub quantity: u32,
    pub area: Area,
}
impl Values {
    pub fn validate(&self) -> Result<(), String> {
        if self.scope.trim().is_empty()
            || self.scope.len() > 256
            || self.note.len() > 4096
            || self.label.trim().is_empty()
            || self.label.len() > 1024
            || self.quantity == 0
            || self.quantity > 1_000_000
        {
            return Err("Program name or quantity is invalid".into());
        }
        match &self.area {
            Area::Known { m2 } if !m2.is_finite() || *m2 < 0. || *m2 > 1e9 => {
                Err("Program area is invalid".into())
            }
            Area::Unknown { reason } if reason.trim().is_empty() || reason.len() > 4096 => {
                Err("Explain the unknown program area".into())
            }
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub key: String,
    pub source_row: u32,
    pub values: Values,
    pub imported: Values,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProgramSheet {
    pub id: String,
    pub name: String,
    pub source_sha256: String,
    pub mapping: Mapping,
    pub rows: Vec<Requirement>,
    pub blank_rows: Vec<u32>,
}
impl ProgramSheet {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty()
            || self.id.len() > 256
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || !crate::storage::valid_document_identity(&self.source_sha256)
            || self.rows.is_empty()
            || self.rows.len() > 10000
        {
            return Err("Invalid program sheet identity or size".into());
        }
        self.mapping.validate()?;
        let mut keys = BTreeSet::new();
        let mut rows = BTreeSet::new();
        for row in &self.rows {
            if row.key.trim().is_empty()
                || row.key.len() > 256
                || !keys.insert(&row.key)
                || row.source_row < self.mapping.first_row
                || row.source_row > self.mapping.last_row
                || !rows.insert(row.source_row)
            {
                return Err("Invalid or duplicate requirement key or source row".into());
            }
            row.values.validate()?;
            row.imported.validate()?;
        }
        for row in &self.blank_rows {
            if *row < self.mapping.first_row || *row > self.mapping.last_row || !rows.insert(*row) {
                return Err("Invalid blank program row".into());
            }
        }
        if rows.len() != (self.mapping.last_row - self.mapping.first_row + 1) as usize {
            return Err("Every mapped source row must be accounted for".into());
        }
        Ok(())
    }
    pub fn user_modified(&self) -> bool {
        self.rows.iter().any(|r| r.values != r.imported)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub sheets: Vec<ProgramSheet>,
}
impl Program {
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        if self.sheets.len() > 100 {
            return Err("Program exceeds 100 sheets".into());
        }
        for sheet in &self.sheets {
            sheet.validate()?;
            if !ids.insert(&sheet.id) {
                return Err("Duplicate program sheet identity".into());
            }
        }
        Ok(())
    }
    pub fn summary(&self) -> serde_json::Value {
        let mut known = 0.;
        let mut unknown = 0;
        let mut rows = 0;
        let mut by_scope: std::collections::BTreeMap<String, f64> = Default::default();
        for s in &self.sheets {
            for r in &s.rows {
                rows += 1;
                match r.values.area {
                    Area::Known { m2 } => {
                        known += m2;
                        *by_scope.entry(r.values.scope.clone()).or_default() += m2;
                    }
                    Area::Unknown { .. } => unknown += 1,
                }
            }
        }
        serde_json::json!({"requirement_rows":rows,"known_area_m2":known,"known_area_by_scope_m2":by_scope,"unknown_area_rows":unknown,"all_areas_known":unknown==0,"approval":"unverified declarations; not statutory area"})
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportProposal {
    pub sheet: ProgramSheet,
    pub expected_previous: Option<ProgramSheet>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgramEdit {
    Import {
        proposal: ImportProposal,
    },
    UpdateRow {
        sheet_id: String,
        key: String,
        expected: Values,
        values: Values,
    },
    RemoveSheet {
        expected: ProgramSheet,
    },
}
pub fn apply(program: &Program, edit: &ProgramEdit) -> Result<Program, String> {
    let mut next = program.clone();
    match edit {
        ProgramEdit::Import { proposal } => {
            proposal.sheet.validate()?;
            if proposal.sheet.user_modified() {
                return Err("Import must retain exact source values".into());
            }
            let index = next.sheets.iter().position(|s| s.id == proposal.sheet.id);
            if index.map(|i| &next.sheets[i]) != proposal.expected_previous.as_ref() {
                return Err("Program changed after preview".into());
            }
            if let Some(i) = index {
                let old = &next.sheets[i];
                if old.user_modified() {
                    return Err("Reimport refused: program contains user edits".into());
                }
                let next_keys = proposal
                    .sheet
                    .rows
                    .iter()
                    .map(|r| r.key.as_str())
                    .collect::<BTreeSet<_>>();
                if old.rows.iter().any(|r| !next_keys.contains(r.key.as_str())) {
                    return Err(
                        "Reimport would remove requirements; review the missing keys".into(),
                    );
                }
                next.sheets[i] = proposal.sheet.clone();
            } else {
                next.sheets.push(proposal.sheet.clone());
            }
        }
        ProgramEdit::UpdateRow {
            sheet_id,
            key,
            expected,
            values,
        } => {
            values.validate()?;
            let row = next
                .sheets
                .iter_mut()
                .find(|s| &s.id == sheet_id)
                .and_then(|s| s.rows.iter_mut().find(|r| &r.key == key))
                .ok_or("Requirement no longer exists")?;
            if &row.values != expected {
                return Err("Requirement changed after editing began".into());
            }
            row.values = values.clone();
        }
        ProgramEdit::RemoveSheet { expected } => {
            let i = next
                .sheets
                .iter()
                .position(|s| s.id == expected.id)
                .ok_or("Program sheet no longer exists")?;
            if &next.sheets[i] != expected {
                return Err("Program changed after preview".into());
            }
            next.sheets.remove(i);
        }
    }
    next.validate()?;
    Ok(next)
}
