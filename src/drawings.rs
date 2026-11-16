//! Original source images and independent, metric drawing regions. No BIM inference.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingDocument {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cad_sources: Vec<crate::cad_reference::Source>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cad_underlays: Vec<crate::cad_reference::Underlay>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reports: Vec<crate::diagrams::report::Template>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagrams: Vec<crate::diagrams::Diagram>,
    pub pages: Vec<SourcePage>,
    pub regions: Vec<DrawingRegion>,
    #[serde(default)]
    pub worksheets: Vec<Worksheet>,
    #[serde(default)]
    pub reference_sources: Vec<crate::external::ReferenceSource>,
    #[serde(default)]
    pub reference_instances: Vec<crate::external::ReferenceInstance>,
}

impl Default for DrawingDocument {
    fn default() -> Self {
        Self {
            schema_version: 1,
            cad_sources: vec![],
            cad_underlays: vec![],
            reports: vec![],
            diagrams: vec![],
            pages: vec![],
            regions: vec![],
            worksheets: vec![],
            reference_sources: vec![],
            reference_instances: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Worksheet {
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingPlacement {
    pub worksheet: Option<u64>,
    pub story: u32,
    pub origin: crate::model::Point,
    pub angle: f64,
    pub visible: bool,
    pub opacity: u8,
    #[serde(default)]
    pub frame: Option<[f64; 4]>,
}
impl DrawingPlacement {
    pub fn world(&self, pixel: [f64; 2], scale: f64) -> crate::model::Point {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (pixel[0] as f64 * scale, -pixel[1] as f64 * scale);
        crate::model::Point::new(
            self.origin.x + x * cos - y * sin,
            self.origin.y + x * sin + y * cos,
        )
    }
    pub fn pixel(&self, p: crate::model::Point, scale: f64) -> [f64; 2] {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (p.x - self.origin.x, p.y - self.origin.y);
        [
            f64::from((x * cos + y * sin) / scale),
            f64::from(-(-x * sin + y * cos) / scale),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourcePage {
    pub id: u64,
    pub name: String,
    pub sha256: String,
    pub path: PathBuf,
    pub pixels: [u32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<DrawingPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_source: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingRegion {
    pub id: u64,
    pub page: u64,
    pub story: u32,
    /// Source pixels, top-left origin, Y down: left, top, right, bottom.
    pub bounds: [f64; 4],
    pub calibration: Option<Calibration>,
    pub revision: u64,
    pub needs_review: bool,
    pub visible: bool,
    pub opacity: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub points: [[f64; 2]; 2],
    pub distance_mm: f64,
}

/// Provider-neutral pixel-space proposals. Importing does not create elements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorksheetProposal {
    #[serde(default)]
    pub placement: Option<DrawingPlacement>,
    #[serde(default)]
    pub target_worksheet: Option<u64>,
    #[serde(default)]
    pub target_story: u32,
    pub worksheet: u64,
    pub source_sha256: String,
    pub region: u64,
    pub region_revision: u64,
    pub scale_revision: u64,
    pub project_revision: u64,
    pub polylines: Vec<Vec<[f64; 2]>>,
}

impl WorksheetProposal {
    pub fn validate(&self, project: &crate::model::Project, revision: u64) -> Result<(), String> {
        let page = project
            .drawings
            .pages
            .iter()
            .find(|p| p.id == self.worksheet)
            .ok_or("Select a source page")?;
        let region = project
            .drawings
            .regions
            .iter()
            .find(|r| r.id == self.region && r.page == self.worksheet)
            .ok_or("Select a drawing region")?;
        let scale = project
            .drawings
            .regions
            .iter()
            .find(|r| r.page == self.worksheet)
            .ok_or("Select a drawing region")?;
        if self.placement != page.placement
            || (self
                .target_worksheet
                .is_some_and(|id| !project.drawings.worksheet_exists(id)))
            || !project.stories.iter().any(|s| s.id == self.target_story)
            || self.source_sha256 != page.sha256
            || self.region_revision != region.revision
            || self.scale_revision != scale.revision
            || self.project_revision != revision
        {
            return Err("AI proposal is stale; export a new worksheet request".into());
        }
        if self.polylines.is_empty()
            || self.polylines.len() > 10000
            || self.polylines.iter().map(Vec::len).sum::<usize>() > 100000
            || self
                .polylines
                .iter()
                .any(|line| line.len() < 2 || line.iter().any(|p| !region.contains(*p)))
        {
            return Err("AI proposal contains invalid or out-of-region geometry".into());
        }
        Ok(())
    }
}

impl Calibration {
    pub fn mm_per_pixel(&self) -> Result<f64, String> {
        let distance =
            (self.points[1][0] - self.points[0][0]).hypot(self.points[1][1] - self.points[0][1]);
        if self.points.iter().flatten().any(|v| !v.is_finite())
            || distance < 1.
            || !self.distance_mm.is_finite()
            || self.distance_mm <= 0.
        {
            return Err("Choose two distinct image points and a positive distance".into());
        }
        let scale = self.distance_mm / distance;
        if !scale.is_finite() || scale <= 0. {
            return Err("Choose two distinct image points and a positive distance".into());
        }
        Ok(scale)
    }
}

impl DrawingRegion {
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p.iter().all(|v| v.is_finite())
            && p[0] >= self.bounds[0]
            && p[0] <= self.bounds[2]
            && p[1] >= self.bounds[1]
            && p[1] <= self.bounds[3]
    }

    pub fn world(&self, p: [f64; 2]) -> Option<[f64; 2]> {
        let scale = self.calibration.as_ref()?.mm_per_pixel().ok()? / 1000.;
        Some([
            (p[0] - self.bounds[0]) * scale,
            -(p[1] - self.bounds[1]) * scale,
        ])
    }

    pub fn check_distance(
        &self,
        points: [[f64; 2]; 2],
        known_mm: f64,
    ) -> Result<(f64, f64), String> {
        if !points.iter().all(|p| self.contains(*p)) || !known_mm.is_finite() || known_mm <= 0. {
            return Err("Choose two distinct image points and a positive distance".into());
        }
        let check = Calibration {
            points,
            distance_mm: known_mm,
        };
        check.mm_per_pixel()?;
        let measured = self
            .calibration
            .as_ref()
            .ok_or("Calibrate this region first")?
            .mm_per_pixel()?
            * (points[1][0] - points[0][0]).hypot(points[1][1] - points[0][1]);
        Ok((measured, measured - known_mm))
    }

    pub fn calibrate(&mut self, calibration: Calibration) -> Result<(), String> {
        calibration.mm_per_pixel()?;
        if !calibration.points.iter().all(|p| self.contains(*p)) {
            return Err("Points must be inside the selected region".into());
        }
        if self.calibration.as_ref() == Some(&calibration) {
            return Ok(());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Drawing revision exhausted")?;
        self.needs_review |= self.calibration.is_some();
        self.calibration = Some(calibration);
        self.revision = revision;
        Ok(())
    }
}

impl DrawingDocument {
    pub fn worksheet_name(&self, id: u64) -> Option<&str> {
        self.worksheets
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.name.as_str())
            .or_else(|| {
                self.pages
                    .iter()
                    .find(|p| p.id == id && p.placement.is_none())
                    .map(|p| p.name.as_str())
            })
    }

    pub fn worksheet_exists(&self, id: u64) -> bool {
        self.worksheets.iter().any(|w| w.id == id)
            || self
                .pages
                .iter()
                .any(|p| p.id == id && p.placement.is_none())
    }
    pub fn search_sources(&self, query: &str) -> Vec<&SourcePage> {
        let terms = query.to_lowercase();
        self.pages
            .iter()
            .filter(|p| {
                terms.split_whitespace().all(|term| {
                    format!(
                        "{} {} {}",
                        p.id,
                        p.name,
                        p.linked_source.as_ref().unwrap_or(&p.path).display()
                    )
                    .to_lowercase()
                    .contains(term)
                })
            })
            .collect()
    }

    pub fn next_id(&self) -> Result<u64, String> {
        self.pages
            .iter()
            .map(|p| p.id)
            .chain(self.diagrams.iter().map(|r| r.id))
            .chain(self.reports.iter().map(|r| r.id))
            .chain(self.regions.iter().map(|r| r.id))
            .chain(self.worksheets.iter().map(|w| w.id))
            .chain(self.reference_sources.iter().map(|v| v.id))
            .chain(self.reference_instances.iter().map(|v| v.id))
            .chain(self.cad_sources.iter().map(|v| v.id))
            .chain(self.cad_underlays.iter().map(|v| v.id))
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "Drawing IDs exhausted".into())
    }

    pub fn add_region(&mut self, page: u64, story: u32, bounds: [f64; 4]) -> Result<u64, String> {
        let id = self.next_id()?;
        self.regions.push(DrawingRegion {
            id,
            page,
            story,
            bounds,
            calibration: None,
            revision: 0,
            needs_review: false,
            visible: true,
            opacity: 140,
        });
        Ok(id)
    }

    pub fn validate(&self, stories: &[crate::model::Story]) -> Result<(), String> {
        crate::cad_reference::validate(self)?;
        if self.schema_version != 1 {
            return Err("Unsupported drawing schema".into());
        }
        if self.pages.len() > 100
            || self.regions.len() > 1000
            || self.worksheets.len() > 100
            || self.reference_sources.len() > 100
            || self.reference_instances.len() > 1000
        {
            return Err("Too many drawing regions".into());
        }
        if self.diagrams.len() > 64 {
            return Err("Too many model figures".into());
        }
        let mut ids = std::collections::HashSet::new();
        if self.reports.len() > 8 {
            return Err("Report template limit reached".into());
        }
        for report in &self.reports {
            report.validate()?;
            if !ids.insert(report.id) {
                return Err("Duplicate report identity".into());
            }
        }
        for diagram in &self.diagrams {
            diagram.validate()?;
            if !ids.insert(diagram.id) {
                return Err("Duplicate figure identity".into());
            }
        }
        for worksheet in &self.worksheets {
            if worksheet.id == 0 || !ids.insert(worksheet.id) || worksheet.name.trim().is_empty() {
                return Err("Invalid worksheet".into());
            }
        }
        for page in &self.pages {
            if let Some(p) = &page.placement {
                if p.frame.is_some_and(|b| {
                    b.iter().any(|v| !v.is_finite())
                        || b[0] < 0.
                        || b[1] < 0.
                        || b[2] > page.pixels[0] as f64
                        || b[3] > page.pixels[1] as f64
                        || b[2] - b[0] < 1.
                        || b[3] - b[1] < 1.
                }) || !p.origin.finite()
                    || !p.angle.is_finite()
                    || !stories.iter().any(|s| s.id == p.story)
                    || p.worksheet.is_some_and(|id| !self.worksheet_exists(id))
                {
                    return Err("Invalid drawing placement".into());
                }
            }
            if page.id == 0
                || !ids.insert(page.id)
                || page.path.as_os_str().is_empty()
                || page.sha256.len() != 64
                || !page
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || page.pixels.contains(&0)
                || page.pixels.iter().any(|v| *v > 16384)
                || u64::from(page.pixels[0]) * u64::from(page.pixels[1]) > 16_000_000
            {
                return Err("Invalid drawing source".into());
            }
        }
        if self
            .pages
            .iter()
            .map(|p| u64::from(p.pixels[0]) * u64::from(p.pixels[1]))
            .sum::<u64>()
            > 32_000_000
        {
            return Err("Drawing document exceeds pixel limits".into());
        }
        for region in &self.regions {
            let page = self
                .pages
                .iter()
                .find(|p| p.id == region.page)
                .ok_or("Invalid drawing region")?;
            let b = region.bounds;
            if region.id == 0
                || !ids.insert(region.id)
                || !stories.iter().any(|s| s.id == region.story)
                || b.iter().any(|v| !v.is_finite())
                || b[0] < 0.
                || b[1] < 0.
                || b[2] > f64::from(page.pixels[0])
                || b[3] > f64::from(page.pixels[1])
                || b[2] - b[0] < 1.
                || b[3] - b[1] < 1.
            {
                return Err("Invalid drawing region".into());
            }
            if let Some(calibration) = &region.calibration {
                let scale = calibration.mm_per_pixel()?;
                if !calibration.points.iter().all(|p| region.contains(*p))
                    || (b[2] - b[0]).max(b[3] - b[1]) * scale > 100_000_000.
                {
                    return Err("Invalid drawing calibration extent".into());
                }
            }
        }
        Ok(())
    }
}

pub fn source_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Drawing exceeds 32 MiB".into());
    }
    Ok(bytes)
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn decode(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("Only single-page PNG and JPEG are supported".into());
    }
    if format == image::ImageFormat::Png
        && image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(|e| e.to_string())?
            .is_apng()
            .map_err(|e| e.to_string())?
    {
        return Err("Only single-page PNG and JPEG are supported".into());
    }
    let reader = || image::ImageReader::with_format(Cursor::new(bytes), format);
    let (w, h) = reader().into_dimensions().map_err(|e| e.to_string())?;
    if w == 0 || h == 0 || w > 16384 || h > 16384 || u64::from(w) * u64::from(h) > 16_000_000 {
        return Err("Drawing exceeds pixel limits".into());
    }
    let mut reader = reader();
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}

/// Publish complete, synced content without truncating existing objects.
pub fn store(bytes: &[u8], directory: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = directory.join(format!("{}.image", digest(bytes)));
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (temp, mut file) = loop {
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = directory.join(format!(".drawing-{}-{id}.tmp", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };
    let write = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let publish = write.and_then(|()| std::fs::hard_link(&temp, &path));
    let _ = std::fs::remove_file(&temp);
    match publish {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if source_bytes(&path)? != bytes {
                return Err("Drawing asset hash mismatch".into());
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    std::fs::canonicalize(path).map_err(|e| e.to_string())
}

pub fn import(path: &Path, cache: &Path, id: u64) -> Result<SourcePage, String> {
    let bytes = source_bytes(path)?;
    let image = decode(&bytes)?;
    Ok(SourcePage {
        placement: None,
        linked_source: None,
        id,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        sha256: digest(&bytes),
        path: store(&bytes, cache)?,
        pixels: [image.width(), image.height()],
    })
}

pub fn verified_bytes(page: &SourcePage) -> Result<Vec<u8>, String> {
    let bytes = source_bytes(&page.path)?;
    if digest(&bytes) != page.sha256 {
        return Err("Drawing asset hash mismatch".into());
    }
    Ok(bytes)
}
