//! Explicit single-record extraction. It never filters a drawing into a claimed full import.
use super::inventory;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClosedOutline {
    pub handle: String,
    pub layer: Option<String>,
    pub record_sha256: String,
    pub points: Vec<[f64; 2]>,
    pub constant_width_native: f64,
}
fn handle(value: &str) -> Option<u64> {
    if value.is_empty() || value.len() > 16 {
        return None;
    }
    u64::from_str_radix(value, 16).ok().filter(|v| *v > 0)
}
pub fn closed_outline(bytes: &[u8], selected: &str) -> Result<ClosedOutline, String> {
    closed_outline_with_width(bytes, selected, false)
}
/// Explicit vertex-path interpretation. Nonzero constant width is retained as a loss.
pub fn closed_outline_with_width(
    bytes: &[u8],
    selected: &str,
    allow_constant_width: bool,
) -> Result<ClosedOutline, String> {
    extract_closed_outline(bytes, selected, allow_constant_width, false).map(|(outline, _)| outline)
}
/// Source observation only. The plane's original Z is returned separately,
/// never discarded or applied as a model elevation. Import callers stay strict.
pub fn observe_closed_outline(
    bytes: &[u8],
    selected: &str,
    allow_constant_width: bool,
) -> Result<(ClosedOutline, f64), String> {
    extract_closed_outline(bytes, selected, allow_constant_width, true)
}
fn extract_closed_outline(
    bytes: &[u8],
    selected: &str,
    allow_constant_width: bool,
    observe_plane_elevation: bool,
) -> Result<(ClosedOutline, f64), String> {
    let wanted = handle(selected.trim()).ok_or("Invalid DXF entity handle")?;
    let inv = inventory::inspect(bytes)?;
    let candidates = inv
        .entities
        .iter()
        .filter(|r| r.handle.as_deref().and_then(handle) == Some(wanted))
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err("DXF handle is missing or ambiguous".into());
    }
    let metadata = candidates[0];
    if metadata.kind != "LWPOLYLINE" {
        return Err("Site boundary requires an explicit closed LWPOLYLINE".into());
    }
    let mut lines = bytes.split(|b| *b == b'\n');
    let mut index = 0;
    let mut groups = Vec::new();
    let mut digest = Sha256::new();
    while let Some(code) = lines.next() {
        let Some(value) = lines.next() else {
            break;
        };
        let n = std::str::from_utf8(code)
            .ok()
            .and_then(|s| s.trim().parse::<u16>().ok())
            .ok_or("Invalid DXF group code")?;
        if index >= metadata.group_pair_index {
            if index > metadata.group_pair_index && n == 0 {
                break;
            }
            digest.update(code);
            digest.update(b"\n");
            digest.update(value);
            digest.update(b"\n");
            groups.push((n, value));
        }
        index += 1;
    }
    let scalar = |code: u16, default: Option<f64>| -> Result<f64, String> {
        let matches = groups
            .iter()
            .filter(|(c, _)| *c == code)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err("Duplicate scalar DXF field".into());
        }
        let value = matches
            .first()
            .map(|(_, v)| {
                std::str::from_utf8(v)
                    .ok()
                    .and_then(|s| s.trim().parse::<f64>().ok())
            })
            .unwrap_or(default);
        value
            .filter(|v| v.is_finite())
            .ok_or("Invalid DXF numeric field".into())
    };
    let integer = |code: u16, default: Option<u32>| -> Result<u32, String> {
        let value = groups
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| {
                std::str::from_utf8(v)
                    .ok()
                    .and_then(|v| v.trim().parse::<u32>().ok())
            })
            .unwrap_or(default);
        value.ok_or("Invalid integer DXF field".into())
    };
    let count = scalar(90, None)?;
    let flags = scalar(70, Some(0.))?;
    integer(90, None)?;
    integer(70, Some(0))?;
    if count.fract() != 0.
        || !(3.0..=10000.0).contains(&count)
        || flags.fract() != 0.
        || !(0.0..=129.0).contains(&flags)
        || (flags as u16) & !129 != 0
        || (flags as u16) & 1 == 0
    {
        return Err("Boundary needs a closed polyline with a valid vertex count".into());
    }
    let width = scalar(43, Some(0.))?;
    let elevation = scalar(38, Some(0.))?;
    if !(0.0..=1e12).contains(&width) || (width != 0. && !allow_constant_width) {
        return Err("Nonzero constant width requires explicit centerline interpretation".into());
    }
    for (code, value) in &groups {
        if [30, 38, 39, 40, 41, 42, 67, 210, 220, 230].contains(code) {
            let v = std::str::from_utf8(value)
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or("Invalid DXF numeric field")?;
            if !(*code == 38 && observe_plane_elevation) && v != if *code == 230 { 1. } else { 0. }
            {
                return Err(
                    "Boundary curves, widths, 3D, paper space and custom OCS are unsupported"
                        .into(),
                );
            }
        }
    }
    let mut points = Vec::new();
    let mut x = None;
    for (code, value) in groups {
        if code == 10 || code == 20 {
            let value = std::str::from_utf8(value)
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite() && v.abs() <= 1e12)
                .ok_or("Invalid DXF coordinate")?;
            if code == 10 {
                if x.replace(value).is_some() {
                    return Err("Incomplete DXF vertex".into());
                }
            } else {
                points.push([x.take().ok_or("Incomplete DXF vertex")?, value]);
            }
        }
    }
    if x.is_some() || points.len() != count as usize {
        return Err("DXF vertex count mismatch".into());
    }
    if points.first() == points.last() {
        points.pop();
    }
    if points.len() < 3 {
        return Err("Boundary needs three distinct vertices".into());
    }
    Ok((
        ClosedOutline {
            handle: format!("{wanted:X}"),
            layer: metadata.layer.clone(),
            record_sha256: format!("{:x}", digest.finalize()),
            points,
            constant_width_native: width,
        },
        elevation,
    ))
}
