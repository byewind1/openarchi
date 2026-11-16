//! Immutable CAD assets and one explicit drawing-to-model transform. No BIM elements.
use crate::{
    drawings::{self, DrawingDocument},
    model::Point,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
pub const PARSER_VERSION: u32 = 3;
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blob {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub id: u64,
    pub name: String,
    pub original: Blob,
    pub dxf: Blob,
    pub parsed: Blob,
    pub linked_path: PathBuf,
    pub report: Report,
    #[serde(skip)]
    pub data: Option<Arc<Data>>,
    #[serde(skip)]
    pub load_error: Option<String>,
}
impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.original == other.original
            && self.dxf == other.dxf
            && self.parsed == other.parsed
            && self.linked_path == other.linked_path
            && self.report == other.report
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub entities: BTreeMap<String, usize>,
    pub omitted: BTreeMap<String, usize>,
    pub warnings: BTreeSet<String>,
    pub units: Option<i32>,
    #[serde(default)]
    pub text_encoding: Option<String>,
    pub bounds: Option<[f64; 4]>,
    pub rendered_entities: usize,
    pub parse_ms: f64,
    pub conversion_ms: f64,
    pub converter: Option<crate::project_repository::conversion::ToolIdentity>,
    pub diagnostics: String,
}
// Tool identity is provenance, compared via its serializable fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub anchor: [f64; 2],
    pub metres_per_unit: f64,
    pub xy: Point,
    pub angle: f64,
    pub elevation: f64,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            anchor: [0.; 2],
            metres_per_unit: 1.,
            xy: Point::default(),
            angle: 0.,
            elevation: 0.,
        }
    }
}
impl Transform {
    pub fn world(&self, p: [f64; 2]) -> Point {
        let (s, c) = self.angle.to_radians().sin_cos();
        let x = (p[0] - self.anchor[0]) * self.metres_per_unit;
        let y = (p[1] - self.anchor[1]) * self.metres_per_unit;
        Point::new(self.xy.x + x * c - y * s, self.xy.y + x * s + y * c)
    }
    pub fn native(&self, p: Point) -> [f64; 2] {
        let (s, c) = self.angle.to_radians().sin_cos();
        let x = p.x - self.xy.x;
        let y = p.y - self.xy.y;
        [
            self.anchor[0] + (x * c + y * s) / self.metres_per_unit,
            self.anchor[1] + (-x * s + y * c) / self.metres_per_unit,
        ]
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.xy.finite()
            || self.anchor.iter().any(|v| !v.is_finite())
            || !self.angle.is_finite()
            || !self.elevation.is_finite()
            || !self.metres_per_unit.is_finite()
            || !(1e-9..=1e6).contains(&self.metres_per_unit)
        {
            Err("Invalid CAD transform".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Underlay {
    pub id: u64,
    pub worksheet: u64,
    pub source: u64,
    pub transform: Transform,
    pub visible: bool,
    pub show_3d: bool,
    #[serde(default)]
    pub show_plan: bool,
    pub locked: bool,
    pub fade: u8,
    #[serde(default)]
    pub xray: bool,
    pub hidden_layers: BTreeSet<String>,
    pub partial_accepted: bool,
}
impl Underlay {
    /// The exact line segments displayed in 3D. Text/report extents are not geometry.
    pub fn model_segments<'a>(
        &'a self,
        data: &'a Data,
    ) -> impl Iterator<Item = [glam::DVec3; 2]> + 'a {
        data.primitives
            .iter()
            .filter(|p| p.text.is_none() && !self.hidden_layers.contains(&p.layer))
            .flat_map(|p| p.points.windows(2))
            .map(|pair| {
                std::array::from_fn(|i| {
                    let p = self.transform.world(pair[i]);
                    glam::DVec3::new(p.x, p.y, self.transform.elevation)
                })
            })
    }
    pub fn grey(&self) -> u8 {
        40u8.saturating_add(self.fade)
    }
    pub fn visible_in_plan(&self, worksheet: Option<u64>) -> bool {
        self.visible
            && match worksheet {
                Some(w) => self.worksheet == w,
                None => self.show_plan,
            }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Primitive {
    pub identity: String,
    pub layer: String,
    pub points: Vec<[f64; 2]>,
    pub endpoints: Vec<[f64; 2]>,
    pub text: Option<Text>,
    pub fill: bool,
    #[serde(skip)]
    pub(crate) display_levels: Vec<DisplayLevel>,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DisplayLevel {
    error: f64,
    indices: Vec<usize>,
}
impl Primitive {
    pub fn display_points(&self, max_error: f64) -> impl ExactSizeIterator<Item = [f64; 2]> + '_ {
        let level = self
            .display_levels
            .iter()
            .rev()
            .find(|l| l.error <= max_error);
        let count = level.map_or(self.points.len(), |l| l.indices.len());
        (0..count).map(move |i| self.points[level.map_or(i, |l| l.indices[i])])
    }
    fn prepare_display(&mut self) {
        if self.text.is_some()
            || !(9..=4096).contains(&self.points.len())
            || !self.display_levels.is_empty()
        {
            return;
        }
        use geo::SimplifyIdx;
        let line: geo::LineString<f64> = self.points.iter().map(|p| (p[0], p[1])).collect();
        let mut previous = self.points.len();
        for error in [0.01, 0.1, 1., 10., 100., 1000., 10000., 100000.] {
            let indices = line.simplify_idx(error);
            if indices.len() < previous {
                previous = indices.len();
                self.display_levels.push(DisplayLevel { error, indices });
            }
            if previous <= 2 {
                break;
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub value: String,
    pub height: f64,
    pub angle: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Data {
    pub version: u32,
    #[serde(default)]
    pub source_sha256: String,
    pub primitives: Vec<Primitive>,
    pub report: Report,
    #[serde(skip)]
    pub index: rstar::RTree<Indexed>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Indexed {
    pub bounds: rstar::AABB<[f64; 2]>,
    pub primitive: usize,
}
impl rstar::RTreeObject for Indexed {
    type Envelope = rstar::AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}
impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.source_sha256 == other.source_sha256
            && self.primitives == other.primitives
            && self.report == other.report
    }
}
impl Data {
    pub fn rebuild_index(&mut self) {
        for p in &mut self.primitives {
            p.prepare_display();
        }
        self.index = rstar::RTree::bulk_load(
            self.primitives
                .iter()
                .enumerate()
                .filter_map(|(primitive, p)| {
                    primitive_bounds(p).map(|b| Indexed {
                        bounds: rstar::AABB::from_corners([b[0], b[1]], [b[2], b[3]]),
                        primitive,
                    })
                })
                .collect(),
        );
    }
    pub fn visible(&self, b: [f64; 4]) -> impl Iterator<Item = &Primitive> {
        self.index
            .locate_in_envelope_intersecting(&rstar::AABB::from_corners([b[0], b[1]], [b[2], b[3]]))
            .map(|i| &self.primitives[i.primitive])
    }
}
/// Conservative display bounds include substituted, rotated multiline text.
fn primitive_bounds(p: &Primitive) -> Option<[f64; 4]> {
    let mut b = bounds(&p.points)?;
    if let Some(text) = &p.text {
        let anchor = p.points[0];
        let rows: Vec<_> = text.value.lines().collect();
        let width =
            rows.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f64 * text.height * 1.25;
        let depth = rows.len().max(1) as f64 * text.height * 2.;
        let (s, c) = text.angle.to_radians().sin_cos();
        for x in [0., width] {
            for y in [-depth, text.height * 2.] {
                let q = [anchor[0] + x * c - y * s, anchor[1] + x * s + y * c];
                b[0] = b[0].min(q[0]);
                b[1] = b[1].min(q[1]);
                b[2] = b[2].max(q[0]);
                b[3] = b[3].max(q[1]);
            }
        }
    }
    Some(b)
}
/// Rebuildable overview bitmap only. It never supplies geometry or snap points.
pub fn thumbnail(data: &Data, width: u32, height: u32) -> image::RgbaImage {
    let width = width.clamp(32, 2048);
    let height = height.clamp(32, 2048);
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([248, 248, 248, 255]));
    let Some(b) = data.report.bounds else {
        return image;
    };
    let scale = ((width as f64 - 16.) / (b[2] - b[0]).max(1e-9))
        .min((height as f64 - 16.) / (b[3] - b[1]).max(1e-9));
    let pixel = |p: [f64; 2]| {
        (
            (8. + (p[0] - b[0]) * scale)
                .round()
                .clamp(0., width as f64 - 1.) as i32,
            (height as f64 - 9. - (p[1] - b[1]) * scale)
                .round()
                .clamp(0., height as f64 - 1.) as i32,
        )
    };
    for p in data.primitives.iter().filter(|p| p.text.is_none()) {
        for pair in p.points.windows(2) {
            let (mut x, mut y) = pixel(pair[0]);
            let (end_x, end_y) = pixel(pair[1]);
            let dx = (end_x - x).abs();
            let dy = -(end_y - y).abs();
            let sx = if x < end_x { 1 } else { -1 };
            let sy = if y < end_y { 1 } else { -1 };
            let mut error = dx + dy;
            loop {
                image.put_pixel(x as u32, y as u32, image::Rgba([50, 50, 50, 255]));
                if x == end_x && y == end_y {
                    break;
                }
                let twice = 2 * error;
                if twice >= dy {
                    error += dy;
                    x += sx;
                }
                if twice <= dx {
                    error += dx;
                    y += sy;
                }
            }
        }
    }
    image
}
pub fn bounds(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let p = points.first()?;
    let mut b = [p[0], p[1], p[0], p[1]];
    for p in points {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    Some(b)
}
pub fn unit_scale(unit: Option<i32>) -> Option<f64> {
    Some(match unit? {
        1 => 0.0254,
        2 => 0.3048,
        4 => 0.001,
        5 => 0.01,
        6 => 1.,
        _ => return None,
    })
}
pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    let mut b = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() as u64 > MAX_BYTES {
        return Err("CAD asset exceeds 256 MiB".into());
    }
    Ok(b)
}
impl Blob {
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let b = read(&self.path)?;
        if drawings::digest(&b) != self.sha256 {
            return Err("CAD asset content hash mismatch".into());
        }
        Ok(b)
    }
}
pub fn store(bytes: &[u8], directory: &Path, extension: &str) -> Result<Blob, String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("CAD asset exceeds 256 MiB".into());
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let sha256 = drawings::digest(bytes);
    let path = directory.join(format!("{sha256}.{extension}"));
    if path.exists() {
        let blob = Blob { path, sha256 };
        blob.bytes()?;
        return Ok(blob);
    }
    let temp = directory.join(format!(
        ".cad-{}.tmp",
        crate::storage::new_document_identity()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::hard_link(&temp, &path)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(temp);
    result?;
    let blob = Blob { path, sha256 };
    blob.bytes()?;
    Ok(blob)
}
fn cached_data(blob: &Blob, dxf_hash: &str, allow_previous: bool) -> Result<Data, String> {
    let bytes = blob.bytes()?;
    let data: Data = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if (data.version != PARSER_VERSION && !(allow_previous && data.version == 2))
        || data.source_sha256 != dxf_hash
    {
        return Err("CAD cache does not match its source".into());
    }
    validate_data(&data)?;
    Ok(data)
}
impl Source {
    pub fn load(&mut self) -> Result<(), String> {
        self.data = None;
        self.load_error = None;
        let result: Result<(), String> = (|| {
            self.original.bytes()?;
            let dxf = self.dxf.bytes()?;
            let mut data = cached_data(&self.parsed, &self.dxf.sha256, true).or_else(
                |_| -> Result<Data, String> {
                    let data = crate::dxf::reference::parse(&dxf)?;
                    if data.report.text_encoding.as_deref() != Some("UTF-8")
                        && data.report.text_encoding != self.report.text_encoding
                    {
                        return Err(
                            "CAD text interpretation changed; update the source to review layers"
                                .into(),
                        );
                    }
                    Ok(data)
                },
            )?;
            data.rebuild_index();
            self.data = Some(Arc::new(data));
            Ok(())
        })();
        if let Err(error) = &result {
            self.load_error = Some(error.clone());
        }
        result
    }
}
pub fn validate_data(data: &Data) -> Result<(), String> {
    if data
        .report
        .bounds
        .is_some_and(|b| b.iter().any(|v| !v.is_finite()) || b[0] > b[2] || b[1] > b[3])
        || (!data.primitives.is_empty() && data.report.bounds.is_none())
        || !data.report.parse_ms.is_finite()
        || !data.report.conversion_ms.is_finite()
        || data.source_sha256.len() != 64
        || !data
            .source_sha256
            .bytes()
            .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
        || data.primitives.len() > 1_000_000
        || data
            .primitives
            .iter()
            .map(|p| p.points.len())
            .sum::<usize>()
            > 4_000_000
        || data.primitives.iter().any(|p| {
            p.points.is_empty()
                || (p.text.is_none() && p.points.len() < 2)
                || p.points
                    .iter()
                    .chain(&p.endpoints)
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1e12)
                || p.text.as_ref().is_some_and(|t| {
                    !t.height.is_finite() || t.height <= 0. || !t.angle.is_finite()
                })
        })
    {
        return Err("Invalid or oversized CAD geometry".into());
    }
    Ok(())
}
pub fn needs_extended_format(doc: &DrawingDocument) -> bool {
    doc.cad_underlays.iter().any(|u| u.show_plan)
        || doc.cad_sources.iter().any(|s| {
            s.report
                .text_encoding
                .as_deref()
                .is_some_and(|e| e != "UTF-8")
        })
}
pub fn validate(doc: &DrawingDocument) -> Result<(), String> {
    let mut ids: std::collections::HashSet<_> = doc
        .pages
        .iter()
        .map(|p| p.id)
        .chain(doc.worksheets.iter().map(|w| w.id))
        .chain(doc.regions.iter().map(|r| r.id))
        .chain(doc.reference_sources.iter().map(|s| s.id))
        .chain(doc.reference_instances.iter().map(|s| s.id))
        .chain(doc.diagrams.iter().map(|s| s.id))
        .chain(doc.reports.iter().map(|s| s.id))
        .collect();
    if doc.cad_sources.len() > 100 || doc.cad_underlays.len() > 1000 {
        return Err("Too many CAD references".into());
    }
    for s in &doc.cad_sources {
        if s.id == 0
            || s.report.bounds.is_some_and(|b| {
                b.iter().any(|v| !v.is_finite() || v.abs() > 1e12) || b[0] > b[2] || b[1] > b[3]
            })
            || !s.report.parse_ms.is_finite()
            || !s.report.conversion_ms.is_finite()
            || !ids.insert(s.id)
            || [&s.original, &s.dxf, &s.parsed].iter().any(|b| {
                b.path.as_os_str().is_empty()
                    || b.sha256.len() != 64
                    || !b
                        .sha256
                        .bytes()
                        .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
            })
        {
            return Err("Invalid CAD source".into());
        }
    }
    for u in &doc.cad_underlays {
        u.transform.validate()?;
        if u.id == 0
            || !ids.insert(u.id)
            || !doc.worksheet_exists(u.worksheet)
            || !doc.cad_sources.iter().any(|s| s.id == u.source)
            || u.fade > 220
            || !u.partial_accepted
        {
            return Err("Invalid or unconfirmed CAD underlay".into());
        }
    }
    Ok(())
}
/// Shared endpoint query, independent of placement lock and building object picking.
pub fn snap(
    doc: &DrawingDocument,
    worksheet: Option<u64>,
    p: Point,
    tolerance: f64,
) -> Option<Point> {
    let mut best = None;
    let mut distance = tolerance;
    for u in &doc.cad_underlays {
        if !u.visible_in_plan(worksheet) {
            continue;
        }
        let Some(data) = doc
            .cad_sources
            .iter()
            .find(|s| s.id == u.source)
            .and_then(|s| s.data.as_ref())
        else {
            continue;
        };
        if let Some(w) = snap_underlay(data, u, &u.transform, p, distance) {
            distance = p.distance(w);
            best = Some(w);
        }
    }
    best
}

/// Query existing read-only endpoint data using the caller's displayed placement.
/// This does not alter source coordinates, layers or the project's saved placement.
pub fn snap_underlay(
    data: &Data,
    underlay: &Underlay,
    transform: &Transform,
    point: Point,
    tolerance: f64,
) -> Option<Point> {
    let q = transform.native(point);
    let radius = tolerance / transform.metres_per_unit;
    let mut best = None;
    let mut distance = tolerance;
    for primitive in data.visible([q[0] - radius, q[1] - radius, q[0] + radius, q[1] + radius]) {
        if underlay.hidden_layers.contains(&primitive.layer) {
            continue;
        }
        for &endpoint in &primitive.endpoints {
            let world = transform.world(endpoint);
            let d = world.distance(point);
            if d < distance {
                distance = d;
                best = Some(world);
            }
        }
    }
    best
}

/// Worker entry: all filesystem/conversion/parse work completes before project mutation.
pub fn prepare(path: &Path, converter: Option<&Path>, cache: &Path) -> Result<Source, String> {
    let original_bytes = read(path)?;
    let original_hash = drawings::digest(&original_bytes);
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "dwg" | "dxf") {
        return Err("Choose DWG or ASCII DXF".into());
    }
    let original = store(&original_bytes, cache, &ext)?;
    let mut tool = None;
    let mut diagnostics = String::new();
    let mut conversion_ms = 0.;
    let dxf_bytes = if ext == "dwg" {
        let exe = converter.ok_or(
            "DWG converter unavailable. Choose a local LibreDWG dwg2dxf executable in this dialog.",
        )?;
        let exe = exe
            .canonicalize()
            .map_err(|e| format!("Converter unavailable: {e}"))?;
        let version = read(&exe).map(|b| drawings::digest(&b))?;
        let identity = crate::project_repository::conversion::run_bounded(
            &exe,
            &["--version".into()],
            cache,
            std::time::Duration::from_secs(5),
            64 * 1024,
        )?;
        if identity.exit_code != Some(0)
            || identity.timed_out
            || !String::from_utf8_lossy(&identity.stdout.bytes)
                .to_ascii_lowercase()
                .contains("dwg2dxf")
        {
            return Err("Incompatible converter protocol: choose LibreDWG dwg2dxf; ODA File Converter needs a separate adapter".into());
        }
        let job = cache.join(format!("dwg-{original_hash}-{version}"));
        let mut repo = if job.join("project.json").exists() {
            crate::project_repository::Repository::open(&job)?
        } else {
            crate::project_repository::Repository::create(
                &job,
                &crate::model::Project::default(),
                &crate::storage::new_document_identity(),
                0,
            )?
        };
        let id = repo.add_source(path)?;
        if id != original_hash {
            return Err("DWG source changed during import; preview again".into());
        }
        let begin = std::time::Instant::now();
        let receipt = match repo.latest_conversion()? {
            Some(r)
                if r.source_id == id
                    && r.tool.sha256 == version
                    && r.derived_source_id.is_some() =>
            {
                r
            }
            _ => repo.convert_dwg(&id, &exe, std::time::Duration::from_secs(120))?,
        };
        conversion_ms = begin.elapsed().as_secs_f64() * 1000.;
        if !receipt
            .tool
            .version
            .to_ascii_lowercase()
            .contains("dwg2dxf")
        {
            return Err("Incompatible converter protocol: this entry requires LibreDWG dwg2dxf, not ODA File Converter".into());
        }
        tool = Some(receipt.tool.clone());
        if let Some(c) = receipt.capture {
            diagnostics = String::from_utf8_lossy(&c.stderr.bytes).into_owned();
            if c.stderr.truncated() {
                diagnostics.push_str(
                    "\n[diagnostics truncated; full byte count retained in conversion receipt]",
                );
            }
        }
        let derived = receipt
            .derived_source_id
            .ok_or_else(|| receipt.error.unwrap_or("DWG conversion failed".into()))?;
        repo.read_original(&derived, MAX_BYTES as usize)?
    } else {
        original_bytes
    };
    let dxf = store(&dxf_bytes, cache, "dxf")?;
    let key = cache.join(format!("{}-v{}.cache", dxf.sha256, PARSER_VERSION));
    let cached = read(&key)
        .and_then(|bytes| serde_json::from_slice::<Blob>(&bytes).map_err(|e| e.to_string()))
        .and_then(|blob| cached_data(&blob, &dxf.sha256, false));
    let mut data = match cached {
        Ok(data) => data,
        Err(_) => crate::dxf::reference::parse(&dxf_bytes)?,
    };
    data.report.conversion_ms = conversion_ms;
    data.report.converter = tool;
    data.report.diagnostics = diagnostics;
    if ext == "dwg" {
        data.report.warnings.insert("DWG provider may omit proxy objects, fonts or external references; conversion diagnostics require review".into());
    }
    let parsed = store(
        &serde_json::to_vec(&data).map_err(|e| e.to_string())?,
        cache,
        "cad.json",
    )?;
    // Cache publication is derived, never the authority for source/placement identities.
    let _ = std::fs::write(
        &key,
        serde_json::to_vec(&parsed).map_err(|e| e.to_string())?,
    );
    data.rebuild_index();
    Ok(Source {
        id: 0,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        original,
        dxf,
        parsed,
        linked_path: path.canonicalize().map_err(|e| e.to_string())?,
        report: data.report.clone(),
        data: Some(Arc::new(data)),
        load_error: None,
    })
}
pub fn portable(
    doc: &mut DrawingDocument,
    directory: &Path,
    relative: &Path,
) -> Result<(), String> {
    for s in &mut doc.cad_sources {
        for (b, ext) in [
            (&mut s.original, "source"),
            (&mut s.dxf, "dxf"),
            (&mut s.parsed, "cad.json"),
        ] {
            let bytes = match b.bytes() {
                Ok(b) => b,
                Err(error) => {
                    if ext == "cad.json" {
                        serde_json::to_vec(s.data.as_deref().ok_or(error)?)
                            .map_err(|e| e.to_string())?
                    } else {
                        return Err(error);
                    }
                }
            };
            let stored = store(&bytes, directory, ext)?;
            b.sha256 = stored.sha256;
            b.path = relative.join(stored.path.file_name().ok_or("Invalid CAD asset path")?);
        }
    }
    Ok(())
}
pub fn restore(doc: &mut DrawingDocument, parent: &Path) {
    for s in &mut doc.cad_sources {
        for b in [&mut s.original, &mut s.dxf, &mut s.parsed] {
            if b.path.is_relative() {
                b.path = parent.join(&b.path);
            }
        }
        if let Err(error) = s.load() {
            s.load_error = Some(error);
        }
    }
}

/// Application service used by UI and typed host commands; no UI-owned CAD model creation.
pub fn attach(
    doc: &DrawingDocument,
    mut source: Source,
    worksheet: Option<u64>,
    transform: Transform,
    accepted: bool,
) -> Result<(DrawingDocument, u64, u64), String> {
    transform.validate()?;
    if !accepted || source.data.as_ref().is_none_or(|d| d.primitives.is_empty()) {
        return Err("Review and accept a nonempty CAD preview first".into());
    }
    let mut doc = doc.clone();
    let worksheet = if let Some(w) = worksheet {
        if !doc.worksheet_exists(w) {
            return Err("Worksheet no longer exists".into());
        }
        w
    } else {
        let id = doc.next_id()?;
        doc.worksheets.push(crate::drawings::Worksheet {
            id,
            name: source.name.clone(),
        });
        id
    };
    if let Some(old) = doc
        .cad_sources
        .iter()
        .find(|s| s.original.sha256 == source.original.sha256 && s.dxf.sha256 == source.dxf.sha256)
    {
        if old.data.as_ref().map(|d| d.version) != source.data.as_ref().map(|d| d.version) {
            return Err(
                "CAD parser changed; update the existing source before placing it again".into(),
            );
        }
        source.id = old.id;
    } else {
        source.id = doc.next_id()?;
        doc.cad_sources.push(source.clone());
    }
    let id = doc.next_id()?;
    doc.cad_underlays.push(Underlay {
        id,
        worksheet,
        source: source.id,
        transform,
        visible: true,
        show_3d: false,
        show_plan: false,
        locked: true,
        fade: 60,
        xray: false,
        hidden_layers: BTreeSet::new(),
        partial_accepted: true,
    });
    validate(&doc)?;
    Ok((doc, worksheet, id))
}

/// Explicit replacement retains worksheet/underlay/source identity and every placement.
/// Bounds or declared-unit changes require the same acknowledgement even for a seemingly compatible source.
pub fn replace_source(
    doc: &DrawingDocument,
    id: u64,
    mut replacement: Source,
    alignment_reviewed: bool,
) -> Result<DrawingDocument, String> {
    if !alignment_reviewed
        || replacement
            .data
            .as_ref()
            .is_none_or(|d| d.primitives.is_empty())
    {
        return Err("Review source alignment changes before updating".into());
    }
    let mut next = doc.clone();
    let source = next
        .cad_sources
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or("CAD source no longer exists")?;
    // If only decoding changed, raw entity identity still maps prior hidden layers.
    let mut layer_map: std::collections::BTreeMap<String, BTreeSet<String>> = Default::default();
    if source.original.sha256 == replacement.original.sha256 {
        if let (Some(old), Some(new)) = (&source.data, &replacement.data) {
            let identities: std::collections::HashMap<_, _> = new
                .primitives
                .iter()
                .map(|p| (p.identity.as_str(), p.layer.as_str()))
                .collect();
            for p in &old.primitives {
                if let Some(new_layer) = identities.get(p.identity.as_str()) {
                    layer_map
                        .entry(p.layer.clone())
                        .or_default()
                        .insert((*new_layer).into());
                }
            }
        }
    }
    replacement.id = id;
    *source = replacement;
    for u in next.cad_underlays.iter_mut().filter(|u| u.source == id) {
        let previous = u.hidden_layers.clone();
        for layer in previous {
            if let Some(mapped) = layer_map.get(&layer) {
                u.hidden_layers.extend(mapped.iter().cloned());
            }
        }
    }
    validate(&next)?;
    Ok(next)
}
