//! Loss-aware record inventory. This does not import or validate entity geometry.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DXF_BYTES: usize = 256 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub group_pair_index: usize,
    pub kind: String,
    pub handle: Option<String>,
    pub layer: Option<String>,
    pub block: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySummary {
    pub records: usize,
    pub by_type: BTreeMap<String, usize>,
    pub by_layer: BTreeMap<String, usize>,
    pub declared_units: Vec<i32>,
    pub acad_versions: Vec<String>,
    pub code_pages: Vec<String>,
    pub missing_handles: usize,
    pub duplicate_handles: usize,
    pub non_utf8_values: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub schema_version: u32,
    pub summary: InventorySummary,
    pub entities: Vec<EntityRecord>,
}
fn trim(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}
fn label(value: &[u8], non_utf8: &mut usize) -> String {
    match std::str::from_utf8(value) {
        Ok(value) => value.to_owned(),
        Err(_) => {
            *non_utf8 += 1;
            format!(
                "hex:{}",
                value.iter().map(|b| format!("{b:02x}")).collect::<String>()
            )
        }
    }
}
pub fn inspect(bytes: &[u8]) -> Result<Inventory, String> {
    if bytes.len() > MAX_DXF_BYTES {
        return Err("DXF inventory exceeds 256 MiB limit".into());
    }
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err("Binary DXF inventory is unsupported".into());
    }
    let mut lines = bytes.split(|&byte| byte == b'\n').peekable();
    let mut section: Option<Vec<u8>> = None;
    let mut expect_section = false;
    let mut eof = false;
    let mut found_entities = false;
    let mut header_key = Vec::new();
    let mut units = Vec::new();
    let mut versions = Vec::new();
    let mut pages = Vec::new();
    let mut non_utf8 = 0;
    let mut entities = Vec::new();
    let mut current: Option<EntityRecord> = None;
    let mut index = 0;
    while let Some(code_line) = lines.next() {
        // A single terminal split remainder is not a group-code line.
        if code_line.is_empty() && lines.peek().is_none() {
            break;
        }
        let value = trim(lines.next().ok_or("Incomplete DXF group pair")?);
        let code = std::str::from_utf8(trim(code_line))
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|c| *c <= 1071)
            .ok_or("Invalid DXF group code")?;
        if eof {
            return Err("DXF contains records after EOF".into());
        }
        if expect_section {
            if code != 2 || value.is_empty() {
                return Err("DXF SECTION needs a name".into());
            }
            if value == b"ENTITIES" {
                if found_entities {
                    return Err("Duplicate DXF ENTITIES section".into());
                }
                found_entities = true;
            }
            section = Some(value.to_vec());
            expect_section = false;
        } else if code == 0 && value == b"SECTION" {
            if section.is_some() {
                return Err("Nested DXF SECTION".into());
            }
            expect_section = true;
        } else if code == 0 && value == b"ENDSEC" {
            if section.take().is_none() {
                return Err("Unmatched DXF ENDSEC".into());
            }
            if let Some(record) = current.take() {
                entities.push(record);
            }
            header_key.clear();
        } else if code == 0 && value == b"EOF" {
            if section.is_some() {
                return Err("DXF EOF inside SECTION".into());
            }
            eof = true;
        } else if section.as_deref() == Some(b"HEADER") {
            if code == 9 {
                header_key = value.to_vec();
            } else if header_key == b"$INSUNITS" && code == 70 {
                units.push(
                    std::str::from_utf8(value)
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .ok_or("Invalid INSUNITS declaration")?,
                );
            } else if header_key == b"$ACADVER" && code == 1 {
                versions.push(label(value, &mut non_utf8));
            } else if header_key == b"$DWGCODEPAGE" && code == 3 {
                pages.push(label(value, &mut non_utf8));
            }
        } else if section.as_deref() == Some(b"ENTITIES") {
            if code == 0 {
                if value.is_empty() {
                    return Err("Empty DXF entity type".into());
                }
                if let Some(record) = current.take() {
                    entities.push(record);
                }
                current = Some(EntityRecord {
                    group_pair_index: index,
                    kind: label(value, &mut non_utf8),
                    handle: None,
                    layer: None,
                    block: None,
                });
            } else if let Some(record) = current.as_mut() {
                match code {
                    5 => record.handle = Some(label(value, &mut non_utf8)),
                    8 => record.layer = Some(label(value, &mut non_utf8)),
                    2 => record.block = Some(label(value, &mut non_utf8)),
                    _ => {}
                }
            } else {
                return Err("DXF entity data has no record".into());
            }
        } else if section.is_none() && code != 999 {
            return Err("DXF record outside SECTION".into());
        }
        index += 1;
    }
    if !eof || expect_section || section.is_some() || !found_entities {
        return Err("DXF needs complete sections, ENTITIES and EOF".into());
    }
    let mut summary = InventorySummary {
        records: entities.len(),
        by_type: BTreeMap::new(),
        by_layer: BTreeMap::new(),
        declared_units: units,
        acad_versions: versions,
        code_pages: pages,
        missing_handles: 0,
        duplicate_handles: 0,
        non_utf8_values: non_utf8,
    };
    let mut handles = BTreeSet::new();
    for record in &entities {
        *summary.by_type.entry(record.kind.clone()).or_default() += 1;
        *summary
            .by_layer
            .entry(record.layer.clone().unwrap_or_else(|| "<missing>".into()))
            .or_default() += 1;
        match &record.handle {
            Some(handle) if !handle.is_empty() => {
                if !handles.insert(handle.to_ascii_uppercase()) {
                    summary.duplicate_handles += 1;
                }
            }
            _ => summary.missing_handles += 1,
        }
    }
    Ok(Inventory {
        schema_version: 1,
        summary,
        entities,
    })
}
