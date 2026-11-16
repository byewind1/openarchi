//! Loss-aware model-space DXF reference adapter. Expanded once, never into Elements.
use crate::cad_reference::{Data, Primitive, Report, Text};
use std::collections::BTreeMap;
type Group<'a> = (u16, &'a str);
#[derive(Clone, Debug)]
struct Record<'a> {
    kind: &'a str,
    g: Vec<Group<'a>>,
}
impl Record<'_> {
    fn s(&self, c: u16) -> &str {
        self.g.iter().find(|p| p.0 == c).map_or("", |p| p.1)
    }
    fn n(&self, c: u16, d: f64) -> f64 {
        let value = self.s(c);
        if value.is_empty() {
            d
        } else {
            value.parse().unwrap_or(f64::NAN)
        }
    }
    fn point(&self, c: u16) -> [f64; 2] {
        [self.n(c, 0.), self.n(c + 10, 0.)]
    }
}
#[derive(Clone, Copy)]
struct Affine([f64; 6]);
impl Affine {
    fn identity() -> Self {
        Self([1., 0., 0., 1., 0., 0.])
    }
    fn point(self, p: [f64; 2]) -> [f64; 2] {
        let a = self.0;
        [
            a[0] * p[0] + a[2] * p[1] + a[4],
            a[1] * p[0] + a[3] * p[1] + a[5],
        ]
    }
    fn then(self, b: Self) -> Self {
        let a = self.0;
        let b = b.0;
        Self([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }
}
struct Block<'a> {
    base: [f64; 2],
    flags: u32,
    path: String,
    records: Vec<Record<'a>>,
}
struct Builder<'a> {
    blocks: BTreeMap<String, Block<'a>>,
    data: Data,
    points: usize,
}
pub fn parse(bytes: &[u8]) -> Result<Data, String> {
    let start = std::time::Instant::now();
    let inv = super::inventory::inspect(bytes)?;
    // Retain any decoding loss in the mandatory quality report.
    let (text, text_encoding, text_errors) = decode_source(bytes, &inv.summary.code_pages);

    let mut b = Builder {
        blocks: BTreeMap::new(),
        data: Data {
            version: crate::cad_reference::PARSER_VERSION,
            source_sha256: crate::drawings::digest(bytes),
            primitives: vec![],
            report: Report::default(),
            index: Default::default(),
        },
        points: 0,
    };
    b.data.report.text_encoding = Some(text_encoding);
    if b.data.report.text_encoding.as_deref() != Some("UTF-8") {
        b.data
            .report
            .warnings
            .insert("Legacy CAD text decoded using its declared code page; inspect labels".into());
    }
    b.data.report.units = inv.summary.declared_units.first().copied();
    b.data.report.entities = inv.summary.by_type;
    b.data.report.warnings.insert(
        "Conversion completeness is unverified; compare critical contours with the original DWG"
            .into(),
    );
    if text_errors {
        b.data
            .report
            .warnings
            .insert("Legacy text encoding: replacement characters present".into());
    }
    if inv.summary.duplicate_handles > 0 || inv.summary.missing_handles > 0 {
        b.data
            .report
            .warnings
            .insert("Missing or duplicate source entity handles; record paths retained".into());
    }
    let mut it = text.lines();
    let mut section = "";
    let mut current: Option<Record<'_>> = None;
    let mut entities = vec![];
    let mut block_records = vec![];
    while let Some(code) = it.next() {
        let Some(value) = it.next() else {
            if code.trim().is_empty() {
                break;
            } else {
                return Err("Incomplete DXF group pair".into());
            }
        };
        let c = code
            .trim()
            .parse::<u16>()
            .map_err(|_| "Invalid DXF group code")?;
        let value = value.trim();
        if c == 0 {
            if let Some(r) = current.take() {
                match section {
                    "ENTITIES" => entities.push(r),
                    "BLOCKS" => block_records.push(r),
                    _ => {}
                }
            }
            if value == "SECTION" {
                section = "?";
            } else if value == "ENDSEC" {
                section = "";
            } else if matches!(section, "ENTITIES" | "BLOCKS") {
                current = Some(Record {
                    kind: value,
                    g: vec![],
                });
            }
        } else if section == "?" && c == 2 {
            section = value;
        } else if let Some(r) = &mut current {
            r.g.push((c, value));
        }
    }
    let mut records = block_records.into_iter();
    while let Some(r) = records.next() {
        if r.kind != "BLOCK" {
            continue;
        }
        let name = r.s(2).to_owned();
        let mut block = Block {
            base: r.point(10),
            flags: r.n(70, 0.) as u32,
            path: r.s(1).into(),
            records: vec![],
        };
        for r in records.by_ref() {
            if r.kind == "ENDBLK" {
                break;
            }
            block.records.push(r);
        }
        if block.flags & 12 != 0 {
            b.data.report.warnings.insert(format!(
                "External reference not resolved: {name} ({})",
                block.path
            ));
        }
        b.blocks.insert(name, block);
    }
    b.expand(&entities, Affine::identity(), "0", "", &mut vec![])?;
    let all: Vec<_> = b
        .data
        .primitives
        .iter()
        .flat_map(|p| p.points.iter().copied())
        .collect();
    b.data.report.bounds = crate::cad_reference::bounds(&all);
    b.data.report.parse_ms = start.elapsed().as_secs_f64() * 1000.;
    crate::cad_reference::validate_data(&b.data)?;
    b.data.rebuild_index();
    Ok(b.data)
}
fn decode_source<'a>(
    bytes: &'a [u8],
    code_pages: &[String],
) -> (std::borrow::Cow<'a, str>, String, bool) {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (std::borrow::Cow::Borrowed(text), "UTF-8".into(), false);
    }
    let declared = code_pages
        .first()
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    let encoding = match declared.as_str() {
        "ANSI_936" => Some(encoding_rs::GBK),
        "ANSI_950" => Some(encoding_rs::BIG5),
        "ANSI_932" => Some(encoding_rs::SHIFT_JIS),
        "ANSI_949" => Some(encoding_rs::EUC_KR),
        "ANSI_874" => Some(encoding_rs::WINDOWS_874),
        _ => declared
            .strip_prefix("ANSI_")
            .and_then(|n| encoding_rs::Encoding::for_label(format!("windows-{n}").as_bytes())),
    };
    if let Some(encoding) = encoding {
        let (text, errors) = encoding.decode_without_bom_handling(bytes);
        return (text, encoding.name().into(), errors);
    }
    (String::from_utf8_lossy(bytes), "unknown".into(), true)
}
impl Builder<'_> {
    fn omit(&mut self, label: impl Into<String>) {
        *self.data.report.omitted.entry(label.into()).or_default() += 1;
    }
    fn emit(
        &mut self,
        r: &Record<'_>,
        m: Affine,
        layer: &str,
        path: &str,
        points: Vec<[f64; 2]>,
        endpoints: Vec<[f64; 2]>,
        text: Option<Text>,
        fill: bool,
    ) -> Result<(), String> {
        let points: Vec<_> = points.into_iter().map(|p| m.point(p)).collect();
        let endpoints: Vec<_> = endpoints.into_iter().map(|p| m.point(p)).collect();
        if points
            .iter()
            .chain(&endpoints)
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1e12)
        {
            self.omit(format!("{}: invalid/out-of-range geometry", r.kind));
            return Ok(());
        }
        if points.is_empty() || (text.is_none() && points.len() < 2) {
            self.omit(format!("{}: incomplete geometry", r.kind));
            return Ok(());
        }
        self.points += points.len();
        if self.points > 4_000_000 || self.data.primitives.len() >= 1_000_000 {
            return Err("CAD expansion exceeds safe geometry budget".into());
        }
        let mut text = text;
        if let Some(t) = &mut text {
            let a = m.0;
            let theta = t.angle.to_radians();
            let x = a[0] * theta.cos() + a[2] * theta.sin();
            let y = a[1] * theta.cos() + a[3] * theta.sin();
            let sx = x.hypot(y);
            let sy = (a[0] * a[3] - a[1] * a[2]).abs() / sx.max(1e-12);
            t.height *= sy;
            t.angle = y.atan2(x).to_degrees();
            if ((sx - sy).abs() > sx * 1e-6) || (a[0] * a[3] - a[1] * a[2] < 0.) {
                self.data.report.warnings.insert(
                    "Mirrored/nonuniform text uses approximate substituted glyph layout".into(),
                );
            }
        }
        if text
            .as_ref()
            .is_some_and(|t| !t.height.is_finite() || t.height <= 0. || !t.angle.is_finite())
        {
            self.omit(format!("{}: invalid text transform", r.kind));
            return Ok(());
        }
        self.data.primitives.push(Primitive {
            identity: format!("{path}/{}", r.s(5)),
            layer: layer.into(),
            points,
            endpoints,
            text,
            fill,
            display_levels: vec![],
        });
        Ok(())
    }
    fn expand(
        &mut self,
        records: &[Record<'_>],
        m: Affine,
        inherited: &str,
        path: &str,
        stack: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut i = 0;
        while i < records.len() {
            let r = &records[i];
            i += 1;
            if r.n(67, 0.) != 0. {
                self.omit("Paper space entity");
                continue;
            }
            if r.n(60, 0.) != 0. {
                continue;
            }
            let layer = if r.s(8).is_empty() || r.s(8) == "0" {
                inherited
            } else {
                r.s(8)
            };
            let identity = format!("{path}/{}:{}", i, r.s(5));
            if r.n(210, 0.) != 0. || r.n(220, 0.) != 0. || r.n(230, 1.) != 1. {
                self.omit(format!("{}: non-XY OCS", r.kind));
                continue;
            }
            if r.n(30, 0.) != 0. || r.n(38, 0.) != 0. || r.n(31, 0.) != 0. {
                self.data.report.warnings.insert(
                    "Native Z values projected to horizontal reference plane; source retained"
                        .into(),
                );
            }
            if r.n(39, 0.) != 0.
                || r.n(43, 0.) != 0.
                || r.g.iter().any(|(c, v)| {
                    [40, 41].contains(c)
                        && r.kind.contains("POLYLINE")
                        && v.parse::<f64>().unwrap_or(0.) != 0.
                })
            {
                self.data
                    .report
                    .warnings
                    .insert("Polyline widths/thickness shown as vector paths".into());
            }
            match r.kind {
                "LINE" => {
                    let a = r.point(10);
                    let z = r.point(11);
                    self.emit(r, m, layer, &identity, vec![a, z], vec![a, z], None, false)?;
                }
                "LWPOLYLINE" => {
                    let mut vertices = vec![];
                    for &(code, value) in &r.g {
                        if code == 10 {
                            vertices.push(([value.parse().unwrap_or(f64::NAN), 0.], 0.));
                        } else if code == 20 {
                            if let Some((p, _)) = vertices.last_mut() {
                                p[1] = value.parse().unwrap_or(f64::NAN);
                            }
                        } else if code == 42 {
                            if let Some((_, b)) = vertices.last_mut() {
                                *b = value.parse().unwrap_or(f64::NAN);
                            }
                        }
                    }
                    let points = polyline(&vertices, r.n(70, 0.) as u32 & 1 != 0);
                    let ends = vertices.iter().map(|v| v.0).collect();
                    self.emit(r, m, layer, &identity, points, ends, None, false)?;
                }
                "POLYLINE" => {
                    let mut vertices = vec![];
                    while let Some(v) = records.get(i) {
                        if v.kind == "SEQEND" {
                            i += 1;
                            break;
                        }
                        if v.kind != "VERTEX" {
                            break;
                        }
                        vertices.push((v.point(10), v.n(42, 0.)));
                        i += 1;
                    }
                    if r.n(70, 0.) as u32 & 80 != 0 {
                        self.omit("POLYLINE mesh/polyface");
                        continue;
                    }
                    let points = polyline(&vertices, r.n(70, 0.) as u32 & 1 != 0);
                    let ends = vertices.iter().map(|v| v.0).collect();
                    self.emit(r, m, layer, &identity, points, ends, None, false)?;
                }
                "ARC" | "CIRCLE" => {
                    let radius = r.n(40, 0.);
                    if radius <= 0. || !radius.is_finite() {
                        self.omit("Invalid circle radius");
                        continue;
                    }
                    let start = r.n(50, 0.).to_radians();
                    let sweep = if r.kind == "CIRCLE" {
                        std::f64::consts::TAU
                    } else {
                        let s =
                            (r.n(51, 0.).to_radians() - start).rem_euclid(std::f64::consts::TAU);
                        if s == 0. { std::f64::consts::TAU } else { s }
                    };
                    let pts = arc(r.point(10), radius, start, sweep);
                    let ends = if r.kind == "ARC" {
                        vec![pts[0], *pts.last().unwrap()]
                    } else {
                        vec![]
                    };
                    self.emit(r, m, layer, &identity, pts, ends, None, false)?;
                }
                "TEXT" | "MTEXT" | "ATTRIB" => {
                    let value = if r.kind == "MTEXT" {
                        let mut s = String::new();
                        for (c, v) in &r.g {
                            if *c == 3 || *c == 1 {
                                s.push_str(v)
                            }
                        }
                        s
                    } else {
                        r.s(1).to_owned()
                    };
                    let (value, formatting) = decode_text(&value);
                    if value.is_empty() {
                        continue;
                    }
                    self.data.report.warnings.insert("Text uses OpenArchi font substitution; original SHX/TTF fonts not resolved".into());
                    if formatting || r.n(72, 0.) != 0. || r.n(73, 0.) != 0. {
                        self.data.report.warnings.insert(
                            "Text formatting/alignment approximated; inspect labels".into(),
                        );
                    }
                    let mut angle = r.n(50, 0.);
                    if r.kind == "MTEXT" && r.g.iter().any(|p| p.0 == 11) {
                        angle = r.n(21, 0.).atan2(r.n(11, 1.)).to_degrees();
                    }
                    let height = r.n(40, 1.).abs().max(1e-8);
                    self.emit(
                        r,
                        m,
                        layer,
                        &identity,
                        vec![r.point(10)],
                        vec![],
                        Some(Text {
                            value,
                            height,
                            angle,
                        }),
                        false,
                    )?;
                }
                "INSERT" => {
                    let name = r.s(2);
                    let Some(block) = self.blocks.get(name) else {
                        self.omit(format!("Missing block: {name}"));
                        continue;
                    };
                    if block.flags & 12 != 0 {
                        self.omit(format!("External reference: {name}"));
                        continue;
                    }
                    if stack.len() >= 32 || stack.iter().any(|v| v == name) {
                        self.omit("Recursive/deep block");
                        continue;
                    }
                    let base = block.base;
                    let children = block.records.clone();
                    let (s, c) = r.n(50, 0.).to_radians().sin_cos();
                    let sx = r.n(41, 1.);
                    let sy = r.n(42, 1.);
                    let position = r.point(10);
                    let rows = r.n(71, 1.) as usize;
                    let cols = r.n(70, 1.) as usize;
                    if rows == 0 || cols == 0 || rows.saturating_mul(cols) > 10000 {
                        self.omit("Invalid/oversized block array");
                        continue;
                    }
                    stack.push(name.into());
                    for row in 0..rows {
                        for col in 0..cols {
                            let x = col as f64 * r.n(44, 0.);
                            let y = row as f64 * r.n(45, 0.);
                            let a = Affine([
                                sx * c,
                                sx * s,
                                -sy * s,
                                sy * c,
                                position[0] + x * c - y * s - sx * c * base[0] + sy * s * base[1],
                                position[1] + x * s + y * c - sx * s * base[0] - sy * c * base[1],
                            ]);
                            self.expand(
                                &children,
                                m.then(a),
                                layer,
                                &format!("{identity}[{row},{col}]"),
                                stack,
                            )?;
                        }
                    }
                    stack.pop();
                }
                "SOLID" | "TRACE" => {
                    self.data.report.warnings.insert("SOLID and TRACE fills are shown as outlines; filled display is not supported".into());
                    let mut p = vec![r.point(10), r.point(11), r.point(13), r.point(12)];
                    if p[2] == [0., 0.] && !r.g.iter().any(|v| v.0 == 13) {
                        p.remove(2);
                    }
                    p.push(p[0]);
                    self.emit(r, m, layer, &identity, p, vec![], None, true)?;
                }
                "HATCH" => {
                    let mut loops: Vec<Vec<([f64; 2], f64)>> = vec![];
                    let mut active = false;
                    let mut closed = false;
                    let mut vertices = vec![];
                    let mut unsupported = false;
                    for &(code, val) in &r.g {
                        if code == 92 {
                            if !vertices.is_empty() {
                                loops.push(std::mem::take(&mut vertices));
                            }
                            let flags = val.parse::<u32>().unwrap_or(0);
                            if flags & 2 == 0 {
                                unsupported = true;
                                break;
                            }
                            active = true;
                            closed = false;
                        } else if active {
                            if code == 73 {
                                closed = val == "1";
                            } else if code == 10 {
                                vertices.push(([val.parse().unwrap_or(f64::NAN), 0.], 0.));
                            } else if code == 20 {
                                if let Some((p, _)) = vertices.last_mut() {
                                    p[1] = val.parse().unwrap_or(f64::NAN);
                                }
                            } else if code == 42 {
                                if let Some((_, b)) = vertices.last_mut() {
                                    *b = val.parse().unwrap_or(f64::NAN);
                                }
                            } else if code == 97 {
                                if !vertices.is_empty() {
                                    if !closed {
                                        self.data.report.warnings.insert(
                                            "Open HATCH boundary shown as a closed preview outline"
                                                .into(),
                                        );
                                    }
                                    loops.push(std::mem::take(&mut vertices));
                                }
                                active = false;
                            }
                        }
                    }
                    if unsupported {
                        self.omit("HATCH edge boundary/pattern");
                    } else {
                        if !vertices.is_empty() {
                            loops.push(vertices);
                        }
                        for (n, vertices) in loops.iter().enumerate() {
                            self.emit(
                                r,
                                m,
                                layer,
                                &format!("{identity}/loop{n}"),
                                polyline(vertices, true),
                                vec![],
                                None,
                                false,
                            )?;
                        }
                        self.data
                            .report
                            .warnings
                            .insert("HATCH boundary preview only; fills/patterns omitted".into());
                    }
                }
                "ATTDEF" | "SEQEND" => {}
                _ => self.omit(r.kind),
            }
        }
        self.data.report.rendered_entities = self.data.primitives.len();
        Ok(())
    }
}
fn arc(c: [f64; 2], r: f64, start: f64, sweep: f64) -> Vec<[f64; 2]> {
    let n = ((sweep.abs() / std::f64::consts::TAU * 192.).ceil() as usize).clamp(2, 384);
    (0..=n)
        .map(|i| {
            let a = start + sweep * i as f64 / n as f64;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}
fn polyline(vertices: &[([f64; 2], f64)], closed: bool) -> Vec<[f64; 2]> {
    let mut out = vec![];
    if vertices.is_empty() {
        return out;
    }
    for i in 0..vertices.len() - usize::from(!closed) {
        let (a, b) = vertices[i];
        let z = vertices[(i + 1) % vertices.len()].0;
        if b.abs() < 1e-12 {
            out.push(a);
        } else {
            let d = (z[0] - a[0]).hypot(z[1] - a[1]);
            if d < 1e-12 {
                out.push(a);
                continue;
            }
            let theta = 4. * b.atan();
            let offset = d * (1. - b * b) / (4. * b);
            let c = [
                (a[0] + z[0]) / 2. - (z[1] - a[1]) / d * offset,
                (a[1] + z[1]) / 2. + (z[0] - a[0]) / d * offset,
            ];
            let mut p = arc(
                c,
                (a[0] - c[0]).hypot(a[1] - c[1]),
                (a[1] - c[1]).atan2(a[0] - c[0]),
                theta,
            );
            p.pop();
            out.extend(p);
        }
    }
    out.push(if closed {
        vertices[0].0
    } else {
        vertices.last().unwrap().0
    });
    out
}
fn decode_text(s: &str) -> (String, bool) {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    let mut formatted = false;
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('P') => out.push('\n'),
                Some('~') => out.push(' '),
                Some('U') if it.peek() == Some(&'+') => {
                    it.next();
                    let hex: String = it.by_ref().take(4).collect();
                    out.push(
                        u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .unwrap_or('\u{fffd}'),
                    );
                }
                Some('\\') => out.push('\\'),
                Some(c) if "HWCQFTApc".contains(c) => {
                    formatted = true;
                    for c in it.by_ref() {
                        if c == ';' {
                            break;
                        }
                    }
                }
                Some(c) if "LlOoKk".contains(c) => {
                    formatted = true;
                }
                Some(c) => {
                    formatted = true;
                    out.push(c)
                }
                None => {}
            }
        } else if c == '{' || c == '}' {
            formatted = true;
        } else {
            out.push(c)
        }
    }
    (
        out.replace("%%d", "°")
            .replace("%%D", "°")
            .replace("%%p", "±")
            .replace("%%P", "±")
            .replace("%%c", "Ø")
            .replace("%%C", "Ø"),
        formatted,
    )
}
