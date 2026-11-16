//! Bounded, read-only source cells; cached formula results never become literal inputs.
use super::*;
use calamine::{DataRef, Reader, Xlsx};
use std::{collections::BTreeMap, io::Cursor};
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// A result cell in an array/table may have no individual f tag. Refuse that
/// unsupported scope rather than treating its numeric cache as a literal.
fn formula_scope_supported(bytes: &[u8]) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|e| e.to_string())?;
        if !file.name().map_err(|e| e.to_string())?.ends_with(".xml") {
            continue;
        }
        let mut xml = quick_xml::Reader::from_reader(std::io::BufReader::new(file));
        let mut buffer = Vec::new();
        loop {
            use quick_xml::events::Event;
            match xml
                .read_event_into(&mut buffer)
                .map_err(|e| e.to_string())?
            {
                Event::Start(e) | Event::Empty(e) => {
                    let name = e.local_name();
                    if matches!(
                        name.as_ref(),
                        "calculatedColumnFormula" | "totalsRowFormula"
                    ) {
                        return Err("Table-defined formulas are unsupported; preserve the source and use a verified literal copy".into());
                    }
                    for attribute in e.attributes() {
                        let attribute = attribute.map_err(|e| e.to_string())?;
                        let key = attribute.key.local_name();
                        if (name.as_ref() == "f"
                            && key.as_ref() == "t"
                            && !matches!(attribute.value.as_ref(), "normal" | "shared"))
                            || (name.as_ref() == "c" && key.as_ref() == "cm")
                        {
                            return Err("Array, data-table and metadata-dependent formulas are unsupported; preserve the source and use a verified literal copy".into());
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Cell {
    pub value: String,
    pub formula: bool,
    pub numeric_allowed: bool,
}
#[derive(Clone, Debug)]
pub struct Sheet {
    pub name: String,
    pub cells: BTreeMap<(u32, u16), Cell>,
    pub last_row: u32,
}
#[derive(Clone, Debug)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
    pub document: Option<crate::source_document::SourceDocument>,
}
impl Sheet {
    pub fn cell(&self, row: u32, col: u16) -> Option<&Cell> {
        self.cells.get(&(row, col))
    }
}
fn put(sheet: &mut Sheet, row: u32, col: u32, cell: Cell) -> Result<(), String> {
    if row == 0
        || row > 1_048_576
        || col == 0
        || col > 16384
        || cell.value.len() > 16384
        || sheet.cells.len() >= 200000
    {
        return Err("Worksheet exceeds supported cell limits".into());
    }
    if sheet.cells.insert((row, col as u16), cell).is_some() {
        return Err("Duplicate worksheet cell".into());
    }
    sheet.last_row = sheet.last_row.max(row);
    Ok(())
}
pub fn inspect(bytes: &[u8], format: &str) -> Result<Workbook, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Functional worksheet exceeds 32 MiB".into());
    }
    match format {
        "csv" => {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| "CSV must be UTF-8; convert a copy explicitly")?
                .trim_start_matches('\u{feff}');
            let mut sheet = Sheet {
                name: "CSV".into(),
                cells: BTreeMap::new(),
                last_row: 0,
            };
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(false)
                .flexible(true)
                .from_reader(text.as_bytes());
            for (i, record) in reader.records().enumerate() {
                let record = record.map_err(|e| e.to_string())?;
                for (c, value) in record.iter().enumerate() {
                    put(
                        &mut sheet,
                        i as u32 + 1,
                        c as u32 + 1,
                        Cell {
                            value: value.into(),
                            formula: value.starts_with('='),
                            numeric_allowed: true,
                        },
                    )?;
                }
            }
            Ok(Workbook {
                sheets: vec![sheet],
                document: None,
            })
        }
        "xlsx" => {
            crate::source_archive::open(bytes)?;
            formula_scope_supported(bytes)?;
            let mut book = Xlsx::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
            let names = book.sheet_names().to_vec();
            if names.len() > 100 {
                return Err("Workbook exceeds 100 sheets".into());
            }
            let mut sheets = vec![];
            let mut total_cells = 0;
            for name in names {
                let mut sheet = Sheet {
                    name: name.clone(),
                    cells: BTreeMap::new(),
                    last_row: 0,
                };
                let mut cells = book
                    .worksheet_cells_reader(&name)
                    .map_err(|e| e.to_string())?;
                while let Some(cell) = cells
                    .next_cell_with_formula_metadata()
                    .map_err(|e| e.to_string())?
                {
                    total_cells += 1;
                    if total_cells > 200000 {
                        return Err("Workbook exceeds 200000 cells".into());
                    }
                    let numeric_allowed = matches!(
                        cell.value,
                        DataRef::Int(..)
                            | DataRef::Float(..)
                            | DataRef::String(..)
                            | DataRef::SharedString(..)
                            | DataRef::Empty
                    );
                    put(
                        &mut sheet,
                        cell.pos.0 + 1,
                        cell.pos.1 + 1,
                        Cell {
                            value: calamine::Data::from(cell.value).to_string(),
                            formula: cell.formula.is_some(),
                            numeric_allowed,
                        },
                    )?;
                }
                sheets.push(sheet);
            }
            Ok(Workbook {
                sheets,
                document: None,
            })
        }
        "docx" => {
            let document = crate::source_document::inspect_docx(bytes)?;
            let mut sheets = vec![];
            for (index, table) in document.tables.iter().enumerate() {
                let mut sheet = Sheet {
                    name: format!("Table {}", index + 1),
                    cells: BTreeMap::new(),
                    last_row: 0,
                };
                for (r, row) in table.rows.iter().enumerate() {
                    for (c, value) in row.iter().enumerate() {
                        put(
                            &mut sheet,
                            r as u32 + 1,
                            c as u32 + 1,
                            Cell {
                                value: value.clone(),
                                formula: false,
                                numeric_allowed: true,
                            },
                        )?;
                    }
                }
                sheets.push(sheet);
            }
            Ok(Workbook {
                sheets,
                document: Some(document),
            })
        }
        _ => Err("Choose a CSV, XLSX or DOCX functional source".into()),
    }
}
fn text(sheet: &Sheet, row: u32, col: u16) -> Result<String, String> {
    let c = sheet
        .cell(row, col)
        .ok_or_else(|| format!("Missing program key/name at row {row}, column {col}"))?;
    if c.formula {
        return Err(format!("Program keys/names must be literal at row {row}"));
    }
    Ok(c.value.trim().into())
}
fn number(cell: Option<&Cell>) -> Option<f64> {
    cell.filter(|c| !c.formula && c.numeric_allowed)
        .and_then(|c| c.value.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}
pub fn map(
    book: &Workbook,
    source: &str,
    id: String,
    name: String,
    mapping: Mapping,
    expected_previous: Option<ProgramSheet>,
) -> Result<ImportProposal, String> {
    mapping.validate()?;
    let sheet = book
        .sheets
        .iter()
        .find(|s| s.name == mapping.sheet)
        .ok_or("Worksheet no longer exists")?;
    if mapping.last_row > sheet.last_row {
        return Err("Mapped range exceeds source rows".into());
    }
    let mut rows = vec![];
    let mut blank_rows = vec![];
    for r in mapping.first_row..=mapping.last_row {
        if !sheet
            .cells
            .range((r, 0)..=(r, u16::MAX))
            .any(|(_, c)| !c.value.trim().is_empty() || c.formula)
        {
            blank_rows.push(r);
            continue;
        }
        let key = text(sheet, r, mapping.key_column)?;
        let label = text(sheet, r, mapping.label_column)?;
        let quantity = if let Some(col) = mapping.quantity_column {
            let q = number(sheet.cell(r, col))
                .filter(|v| v.fract() == 0. && *v >= 1. && *v <= 1_000_000.)
                .ok_or_else(|| format!("Invalid literal quantity at row {r}"))?;
            q as u32
        } else {
            1
        };
        let area = if let Some(col) = mapping.area_column {
            match number(sheet.cell(r, col)) {
                Some(value) if value >= 0. => Area::Known {
                    m2: value
                        * mapping.area_unit.square_metres()
                        * if mapping.area_basis == AreaBasis::PerUnit {
                            quantity as f64
                        } else {
                            1.
                        },
                },
                Some(_) => return Err(format!("Negative area at row {r}")),
                None => Area::Unknown {
                    reason: format!(
                        "No literal numeric area at {} row {r}, column {col}; formulas are not recalculated",
                        mapping.sheet
                    ),
                },
            }
        } else {
            Area::Unknown {
                reason: "No area column mapped".into(),
            }
        };
        let scope = mapping
            .scope_column
            .map(|col| {
                if sheet.cell(r, col).is_some() {
                    text(sheet, r, col)
                } else {
                    Ok(String::new())
                }
            })
            .transpose()?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unspecified".into());
        let note = mapping
            .note_column
            .map(|col| {
                if sheet.cell(r, col).is_some() {
                    text(sheet, r, col)
                } else {
                    Ok(String::new())
                }
            })
            .transpose()?
            .unwrap_or_default();
        let values = Values {
            label,
            scope,
            note,
            quantity,
            area,
        };
        rows.push(Requirement {
            key,
            source_row: r,
            imported: values.clone(),
            values,
        });
    }
    let sheet = ProgramSheet {
        id,
        name,
        source_sha256: source.into(),
        mapping,
        rows,
        blank_rows,
    };
    sheet.validate()?;
    Ok(ImportProposal {
        sheet,
        expected_previous,
    })
}
pub fn address(row: u32, mut column: u16) -> String {
    let mut s = String::new();
    while column > 0 {
        column -= 1;
        s.insert(0, (b'A' + (column % 26) as u8) as char);
        column /= 26;
    }
    format!("{s}{row}")
}
