//! Versioned wire commands; schemas and execution share these Rust types.
//! Converts to the existing BatchChange, never owns a live Project.
use crate::{
    editor::Editor,
    model::{Element, Material, OpeningKind, Point, Shape, SlabContour},
    model_service::{
        BatchChange, ElementReference, ElementTemplate, NamedElement, ReferencedCommand,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
mod discovered;
pub use discovered::{DiscoveredBatch, discovered_schema};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XY {
    pub x: f64,
    pub y: f64,
}
impl From<XY> for Point {
    fn from(p: XY) -> Self {
        Self::new(p.x, p.y)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Reference {
    Existing(u64),
    Temporary(String),
}
impl From<&Reference> for ElementReference {
    fn from(r: &Reference) -> Self {
        match r {
            Reference::Existing(id) => Self::Existing(*id),
            Reference::Temporary(name) => Self::Temporary(name.clone()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "section", rename_all = "snake_case", deny_unknown_fields)]
pub enum Section {
    Rectangular { width: f64, depth: f64 },
    Circular { diameter: f64 },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Inclination {
    /// Degrees above horizontal; 90 is vertical.
    pub slant_degrees: f64,
    /// Degrees from +Y; -90 points to +X.
    pub direction_degrees: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Geometry {
    ConceptMass {
        outline: Vec<XY>,
        #[serde(default)]
        holes: Vec<Vec<XY>>,
        height: f64,
        #[serde(default)]
        bottom_offset: f64,
    },
    Zone {
        /// Unicode room name; creates.name remains the ASCII transaction reference.
        space_name: String,
        purpose: String,
        outline: Vec<XY>,
        #[serde(default)]
        holes: Vec<Vec<XY>>,
        #[serde(default = "crate::zones::default_height")]
        height: f64,
        #[serde(default)]
        bottom_offset: f64,
        #[serde(default)]
        label_position: Option<XY>,
        #[serde(default)]
        surface: Option<crate::zones::surfaces::ZoneSurface>,
    },
    Wall {
        start: XY,
        end: XY,
        thickness: f64,
        height: f64,
    },
    Slab {
        outline: Vec<XY>,
        #[serde(default)]
        holes: Vec<Vec<XY>>,
        thickness: f64,
        #[serde(default)]
        elevation: f64,
    },
    Column {
        center: XY,
        section: Section,
        /// Vertical rise in metres, not inclined axis length. Section dimensions are normal to the axis.
        height: f64,
        #[serde(default)]
        bottom_offset: f64,
        #[serde(default)]
        axis_rotation_degrees: f64,
        inclination: Option<Inclination>,
    },
    Beam {
        start: XY,
        end: XY,
        width: f64,
        height: f64,
        #[serde(default)]
        top_offset: f64,
        #[serde(default)]
        slant_angle_degrees: f64,
    },
    Roof {
        outline: Vec<XY>,
        #[serde(default)]
        holes: Vec<Vec<XY>>,
        pivot_start: XY,
        pivot_end: XY,
        rise_left: bool,
        pitch_degrees: f64,
        thickness: f64,
        #[serde(default)]
        pivot_offset: f64,
    },
    Stair {
        start: XY,
        end: XY,
        width: f64,
        total_rise: f64,
        risers: u16,
        slab_thickness: f64,
        handrail_height: f64,
    },
    Opening {
        host: Reference,
        opening: Opening,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        joinery: Option<crate::opening_joinery::OpeningJoinery>,
        width: f64,
        height: f64,
        sill: f64,
        offset: f64,
    },
    Polyline {
        points: Vec<XY>,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Opening {
    Door,
    Window,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NativeMaterial {
    Concrete,
    Plaster,
    Timber,
}
impl From<NativeMaterial> for Material {
    fn from(m: NativeMaterial) -> Self {
        match m {
            NativeMaterial::Concrete => Self::Concrete,
            NativeMaterial::Plaster => Self::Plaster,
            NativeMaterial::Timber => Self::Timber,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub name: String,
    pub story: u32,
    pub layer: u32,
    pub material: NativeMaterial,
    pub geometry: Geometry,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Constraint {
        edit: crate::constraints::Edit,
    },
    MassSpace {
        edit: crate::mass_spaces::Edit,
    },
    Diagram {
        edit: crate::diagrams::Edit,
    },
    Terrain {
        edit: crate::site::terrain::Edit,
    },
    Evidence {
        edit: crate::evidence::EvidenceEdit,
    },
    Program {
        edit: crate::program::ProgramEdit,
    },
    ApplySiteBoundary {
        proposal: crate::site::boundary::BoundaryProposal,
    },
    DetachSiteBoundary,
    ApplySiteControlRegion {
        proposal: crate::site::boundary::BoundaryProposal,
    },
    DetachSiteControlRegion,
    SetSiteContext {
        context: crate::site::SiteContext,
    },
    Set3dStyle {
        style: crate::view_settings::Model3dStyle,
    },
    SetStoryElevation {
        story: u32,
        elevation: f64,
    },
    Move {
        targets: Vec<Reference>,
        offset: XY,
    },
    Remove {
        targets: Vec<Reference>,
    },
    Update {
        target: Reference,
        geometry: Geometry,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeBatch {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    #[serde(default)]
    pub creates: Vec<Create>,
    #[serde(default)]
    pub edits: Vec<Edit>,
}
fn points(input: &[XY]) -> Vec<Point> {
    input.iter().copied().map(Into::into).collect()
}
impl Geometry {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ConceptMass { .. } => "concept_mass",
            Self::Zone { .. } => "Zone",
            Self::Wall { .. } => "wall",
            Self::Slab { .. } => "slab",
            Self::Column { .. } => "column",
            Self::Beam { .. } => "beam",
            Self::Roof { .. } => "roof",
            Self::Stair { .. } => "stair",
            Self::Opening { .. } => "opening",
            Self::Polyline { .. } => "polyline",
        }
    }
    fn template(
        &self,
        editor: &Editor,
        existing: Option<&Element>,
    ) -> Result<ElementTemplate, String> {
        if let Some(old) = existing {
            let unsupported = old.wall_contour.is_some()
                || old.slab_contour.as_ref().is_some_and(|c| {
                    c.arcs.iter().any(Option::is_some)
                        || c.holes.iter().any(|h| h.arcs.iter().any(Option::is_some))
                })
                || matches!(&old.shape, Shape::Beam { arc: Some(_), .. })
                || matches!(&old.shape,Shape::Polyline{arcs,..} if arcs.iter().any(Option::is_some));
            if unsupported {
                return Err("This native schema cannot update an existing curved variant; use its supported explicit command".into());
            }
        }
        let mut contour = None;
        let mut host = None;
        let shape = match self {
            Self::ConceptMass {
                outline,
                holes,
                height,
                bottom_offset,
            } => Shape::ConceptMass {
                spec: crate::concept_mass::ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: points(outline),
                    holes: holes.iter().map(|p| points(p)).collect(),
                    height: *height,
                    bottom_offset: *bottom_offset,
                },
            },
            Self::Zone {
                space_name: _,
                purpose,
                outline,
                holes,
                height,
                bottom_offset,
                label_position,
                surface,
            } => Shape::Zone {
                spec: crate::zones::ZoneSpec {
                    surface: surface.clone().unwrap_or_else(|| {
                        if let Some(Element {
                            shape: Shape::Zone { spec },
                            ..
                        }) = existing
                        {
                            spec.surface.clone()
                        } else {
                            Default::default()
                        }
                    }),
                    purpose: purpose.clone(),
                    outline: points(outline),
                    holes: holes.iter().map(|p| points(p)).collect(),
                    height: *height,
                    bottom_offset: *bottom_offset,
                    label_position: label_position.map(Into::into),
                },
            },
            Self::Wall {
                start,
                end,
                thickness,
                height,
            } => Shape::Wall {
                start: (*start).into(),
                end: (*end).into(),
                thickness: *thickness,
                height: *height,
            },
            Self::Slab {
                outline,
                holes,
                thickness,
                elevation,
            } => {
                let c = SlabContour {
                    points: points(outline),
                    holes: holes
                        .iter()
                        .map(|h| SlabContour {
                            points: points(h),
                            ..Default::default()
                        })
                        .collect(),
                    elevation: *elevation,
                    ..Default::default()
                };
                let mut e = editor.slab_creation_candidate(c)?;
                contour = e.slab_contour.take();
                match e.shape {
                    Shape::Slab { min, max, .. } => Shape::Slab {
                        min,
                        max,
                        thickness: *thickness,
                    },
                    _ => unreachable!(),
                }
            }
            Self::Column {
                center,
                section,
                height,
                bottom_offset,
                axis_rotation_degrees,
                inclination,
            } => Shape::Column {
                center: (*center).into(),
                section: match section {
                    Section::Rectangular { width, depth } => {
                        crate::column::ColumnSection::Rectangular {
                            width: *width,
                            depth: *depth,
                        }
                    }
                    Section::Circular { diameter } => crate::column::ColumnSection::Circular {
                        diameter: *diameter,
                    },
                },
                height: *height,
                bottom_offset: *bottom_offset,
                axis_rotation: *axis_rotation_degrees,
                inclination: inclination.as_ref().map(|i| {
                    crate::column_inclination::ColumnInclination {
                        slant_degrees: i.slant_degrees,
                        direction_degrees: i.direction_degrees,
                    }
                }),
                plan: existing
                    .and_then(|e| {
                        if let Shape::Column { plan, .. } = &e.shape {
                            Some(plan.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default(),
            },
            Self::Beam {
                start,
                end,
                width,
                height,
                top_offset,
                slant_angle_degrees,
            } => Shape::Beam {
                start: (*start).into(),
                end: (*end).into(),
                width: *width,
                height: *height,
                top_offset: *top_offset,
                slant_angle: *slant_angle_degrees,
                arc: None,
            },
            Self::Roof {
                outline,
                holes,
                pivot_start,
                pivot_end,
                rise_left,
                pitch_degrees,
                thickness,
                pivot_offset,
            } => {
                let c = crate::roof_contour::RoofContour {
                    outer: points(outline),
                    holes: holes.iter().map(|h| points(h)).collect(),
                };
                let (min, max) = c.bounds().ok_or("Roof outline is empty")?;
                Shape::Roof {
                    spec: crate::roof::RoofSpec {
                        contour: Some(c),
                        min,
                        max,
                        pivot_start: (*pivot_start).into(),
                        pivot_end: (*pivot_end).into(),
                        rise_left: *rise_left,
                        pitch: *pitch_degrees,
                        thickness: *thickness,
                        pivot_offset: *pivot_offset,
                    },
                }
            }
            Self::Stair {
                start,
                end,
                width,
                total_rise,
                risers,
                slab_thickness,
                handrail_height,
            } => Shape::Stair {
                spec: crate::stairs::StairSpec {
                    start: (*start).into(),
                    end: (*end).into(),
                    width: *width,
                    total_rise: *total_rise,
                    risers: *risers,
                    slab_thickness: *slab_thickness,
                    handrail_height: *handrail_height,
                },
            },
            Self::Opening {
                host: h,
                opening,
                joinery: _,
                width,
                height,
                sill,
                offset,
            } => {
                host = Some(h.into());
                Shape::Opening {
                    host_wall: 0,
                    kind: match opening {
                        Opening::Door => OpeningKind::Door,
                        Opening::Window => OpeningKind::Window,
                    },
                    width: *width,
                    height: *height,
                    sill: *sill,
                    offset: *offset,
                }
            }
            Self::Polyline { points: p } => Shape::Polyline {
                points: points(p),
                arcs: vec![],
            },
        };
        if let Some(old) = existing {
            let same = std::mem::discriminant(&old.shape) == std::mem::discriminant(&shape);
            if !same {
                return Err("Update cannot change the native element type".into());
            }
        }
        let mut element = existing.cloned().unwrap_or_else(|| {
            editor.creation_candidate(editor.material, editor.wall_layer, shape.clone())
        });
        element.id = 0;
        element.shape = shape;
        if let Self::Zone { space_name, .. } = self {
            if space_name.trim().is_empty() || space_name.len() > 256 {
                return Err("Invalid space name".into());
            }
            element.name = space_name.clone();
        }
        if let Self::Opening {
            joinery: Some(settings),
            ..
        } = self
        {
            settings.validate()?;
            element.opening_joinery = Some(*settings);
        }
        element.slab_contour = contour;
        element.wall_contour = None;
        Ok(ElementTemplate { element, host })
    }
}
impl NativeBatch {
    pub fn lower(&self, editor: &Editor) -> Result<BatchChange, String> {
        if self.version != 1 {
            return Err("Unsupported native command schema version".into());
        }
        if self.creates.len() + self.edits.len() > 256 {
            return Err("Native batch exceeds 256 commands".into());
        }
        let mut creates = Vec::new();
        for c in &self.creates {
            let mut template = c.geometry.template(editor, None)?;
            if !matches!(c.geometry, Geometry::Zone { .. }) {
                template.element.name = c.name.clone();
            }
            template.element.story = c.story;
            template.element.layer = crate::layers::ElementLayer(c.layer);
            template.element.material = c.material.into();
            creates.push(NamedElement {
                name: c.name.clone(),
                template,
            });
        }
        let mut edits = Vec::new();
        for edit in &self.edits {
            edits.push(match edit {
                Edit::Terrain { edit } => ReferencedCommand::Terrain { edit: edit.clone() },
                Edit::Evidence { edit } => ReferencedCommand::Evidence { edit: edit.clone() },
                Edit::MassSpace { edit } => ReferencedCommand::MassSpaces { edit: edit.clone() },
                Edit::Constraint { edit } => ReferencedCommand::Constraint { edit: edit.clone() },
                Edit::Diagram { edit } => ReferencedCommand::Diagram { edit: edit.clone() },
                Edit::Program { edit } => ReferencedCommand::Program { edit: edit.clone() },
                Edit::ApplySiteBoundary { proposal } => ReferencedCommand::ApplySiteBoundary {
                    proposal: proposal.clone(),
                },
                Edit::DetachSiteBoundary => ReferencedCommand::DetachSiteBoundary,
                Edit::ApplySiteControlRegion { proposal } => {
                    ReferencedCommand::ApplySiteControlRegion {
                        proposal: proposal.clone(),
                    }
                }
                Edit::DetachSiteControlRegion => ReferencedCommand::DetachSiteControlRegion,
                Edit::SetSiteContext { context } => ReferencedCommand::SetSiteContext {
                    context: context.clone(),
                },
                Edit::Set3dStyle { style } => ReferencedCommand::Set3dStyle { style: *style },
                Edit::SetStoryElevation { story, elevation } => {
                    ReferencedCommand::SetStoryElevation {
                        story: *story,
                        elevation: *elevation,
                    }
                }
                Edit::Move { targets, offset } => ReferencedCommand::Move {
                    targets: targets.iter().map(Into::into).collect(),
                    offset: (*offset).into(),
                },
                Edit::Remove { targets } => ReferencedCommand::Remove {
                    targets: targets.iter().map(Into::into).collect(),
                },
                Edit::Update { target, geometry } => {
                    let original = match target {
                        Reference::Existing(id) => editor
                            .project
                            .elements
                            .iter()
                            .find(|e| e.id == *id)
                            .cloned(),
                        Reference::Temporary(name) => creates
                            .iter()
                            .find(|e| e.name == *name)
                            .map(|e| e.template.element.clone()),
                    }
                    .ok_or("Update target not found")?;
                    ReferencedCommand::Update {
                        target: target.into(),
                        template: geometry.template(editor, Some(&original))?,
                    }
                }
            });
        }
        Ok(BatchChange {
            version: 1,
            creates,
            edits,
        })
    }
}
pub const COMMANDS: [&str; 10] = [
    "wall",
    "slab",
    "column",
    "beam",
    "roof",
    "stair",
    "opening",
    "polyline",
    "zone",
    "concept_mass",
];
/// Geometry creates include authored spaces alongside eight construction/drawing types; project edits are discoverable
/// separately and cannot masquerade as registered creation geometry.
pub fn command_names() -> Vec<&'static str> {
    COMMANDS
        .into_iter()
        .chain([
            "constraint",
            "story_elevation",
            "3d_style",
            "mass_space",
            "site_context",
            "diagram",
            "program",
            "evidence",
            "terrain",
            "apply_site_boundary",
            "detach_site_boundary",
            "apply_site_control_region",
            "detach_site_control_region",
        ])
        .collect()
}
pub fn schema() -> Value {
    serde_json::to_value(schemars::schema_for!(NativeBatch)).expect("schema serializes")
}
pub fn preview_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(
        crate::automation_change::PreviewResult
    ))
    .expect("schema serializes")
}
fn referenced_defs(variant: &Value, definitions: &Value) -> Value {
    fn collect(value: &Value, refs: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                if let Some(name) = map
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|s| s.strip_prefix("#/$defs/"))
                {
                    refs.insert(name.to_owned());
                }
                for value in map.values() {
                    collect(value, refs);
                }
            }
            Value::Array(values) => {
                for value in values {
                    collect(value, refs);
                }
            }
            _ => {}
        }
    }
    let mut pending = std::collections::BTreeSet::new();
    let mut result = serde_json::Map::new();
    collect(variant, &mut pending);
    while let Some(name) = pending.pop_first() {
        if result.contains_key(&name) {
            continue;
        }
        if let Some(definition) = definitions.get(&name) {
            collect(definition, &mut pending);
            result.insert(name, definition.clone());
        }
    }
    Value::Object(result)
}
pub fn describe(name: &str) -> Option<Value> {
    if name == "evidence" {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"semantics":"Shared undoable declared source quote/annotation and typed element/program-row links. Exact previous note required. Annotation edits preserve link baseline; refresh is explicit. Changed/missing links remain readable and do not block model edits. Model-only input is unverified; native repository import separately rechecks original DOCX text/hash. No source/client/survey approval is inferred."}),
        );
    }
    if name == "terrain" {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"units":{"length":"m","area":"m2"},"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"semantics":"Shared undoable terrain import, numeric vertex edit or removal. Explicit source unit/frame and exact expected prior terrain; no survey approval. Rejects XY overlap, degeneracy and nonmanifold edges. User-edited geometry cannot be reimported. Missing/hole/outside terrain elevation remains unknown."}),
        );
    }
    if name == "constraint" {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"units":{"height":"m","clearance":"m","footprint_area_sum":"m2"},"semantics":"Quantified design conditions over explicit ConceptMass or Zone objects / one candidate. Parameter height is not statutory building height. Area sums use real footprint holes, overlap counted separately, never net/statutory floor area. Spacing requires two explicit objects in one story/candidate, with a bounded ConceptMass-only exception for one Main shared object and one candidate member; different nonempty alternatives and mixed-scope area/height remain unsupported; reference boundary clearance uses the declared editable boundary and does not certify legal status. Missing thresholds, targets, sources/applicability remain unknown; NA requires a current explicit cited declaration. Cited Run additionally requires current original-byte and exact-quote admission at preparation and confirmation; an expired source lease cannot commit. Put alone preserves the previous report without executing. Default remote raw/native/discovered prepare and commit verify originals on the existing background source reader, recheck the original context/authorization after IO, and retain the server preview/payload/request identity. Missing, rejected or changed originals return CITED_ORIGINAL_UNAVAILABLE before intent; queries of an already committed request remain available without re-execution. Design-assumption Runs do not require originals. Put and Run use shared validation/preview/commit/undo, exact check SHA guards; stored reports have input fingerprints and become stale after dependencies change. Query current state/freshness via query_project_rules section checks, not a stale row's historic result. No original-source authority, mandatory coverage or project compliance pass; native19 when checks exist."}),
        );
    }
    if name == "mass_space" {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"units":{"length":"m","area":"m2"},"semantics":"Explicit concept mass to editable Zone association in the same story/option. Create preserves the exact footprint/holes, uses explicit assumed clear height, leaves source geometry intact. LinkExisting checks containment but never authorizes geometry regeneration. Refresh requires the exact link SHA and original unchanged Zone fingerprint; user-edited spaces are never overwritten by reading a fresh revision. Missing/source/scope changes remain inspectable; detach keeps geometry. Shared preview/commit/undo, native17 when links exist. No inferred floor count, wall thickness, surveyed facts, approved use or statutory area."}),
        );
    }
    if name == "diagram" {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"units":{"length":"m","area":"m2","volume":"m3"},"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"semantics":"Read-only linked Zone/concept-mass figure from an exact option/story/domain (not main-plus-option). Optional zone_functions presentation groups exact declared purpose text, stable colour/IDs/area; explicit unknown-purpose annotation is neutral and never edits Zone declarations or implies approved use. Optional exploded_axon uses checked authored prisms in fixed isometric projection with explicit bounded xyz display offsets in mm, never moving model coordinates or changing quantities; object separation is not a storey elevation or measured distance. Optional constraint_overlay (native20) derives the current candidate ConceptMass/Zone check snapshot using shared check definitions and row states; known failures/unknowns are never a project compliance pass. Stored cited results remain unknown without a live original read; original availability is not a permanent figure certificate. Condition/report/model changes make the figure stale; refresh retains its identity. Other/unlocated scopes remain explicit. Optional site_location/setback_study capture existing Site declarations and editable boundary Polyline; site domain allows exact current story/option authored spaces/masses or boundary-only context. A setback study requires source boundary, 0..100000 mm and explicit design assumption, sampled round inward buffer with all parts/holes/empty result retained; not a legal line, approved boundary, buildable area or compliance check. Missing boundary/reference/north remains unknown. Configure uses the exact prior record SHA and does not refresh stale source. Report edits create/update/delete a project-bound ordered template of existing linked figure IDs and chapter headings, using the exact prior report SHA for update/delete; missing references remain explicit, without new geometry, automatic refresh, PDF layout or compliance. Shared preview, commit, undo and persistence; exact record SHA guard for refresh/rename/delete. Snapshot remains inspectable when stale/missing; export requires current inputs. Authored quantities include overlaps separately, never compliance/net floor area. Catalogue declarations do not prove original-byte reads or approvals."}),
        );
    }
    if name == "program" {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"units":{"area":"m2"},"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"semantics":"Shared undoable functional program import/edit/remove. Exact expected state, stable sheet/key and imported baselines protect user edits. Source declarations are unapproved; formulas are unknown. Native import verifies immutable source mapping; no Zone creation or statutory area claim."}),
        );
    }
    if matches!(
        name,
        "apply_site_boundary"
            | "detach_site_boundary"
            | "apply_site_control_region"
            | "detach_site_control_region"
    ) {
        let root = schema();
        let mut input = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == name)?
            .clone();
        input["$defs"] = referenced_defs(&input, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"units":{"length":"m","area":"m2"},"parameters_schema":input,"input_schema":input,"output_schema":preview_schema(),"usage":"Use this edit in change.edits. Native source preview verifies bytes/units before declaring the same proposal.","semantics":"One shared undoable Polyline+site provenance transaction. Declaration is not survey approval. Replace requires exact expected previous boundary and refuses user-edited or locked outlines. Detach keeps the Polyline. Projection frame must match the current site reference."}),
        );
    }
    if name == "site_context" {
        let root = schema();
        let edit = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == "set_site_context")?
            .clone();
        let mut definitions = referenced_defs(&edit, &root["$defs"]);
        let mut input = edit;
        input["$defs"] = definitions.take();
        return Some(
            json!({"name":name,"version":1,"native_type":false,"units":{"length":"m","angle":"degrees"},"input_schema":input,"parameters_schema":input,"output_schema":preview_schema(),"usage":"Use set_site_context in change.edits. Values are declared references, never survey approval.","semantics":"One undoable validated project-header change. Preserve every model coordinate and element. Grid rotation is model XY to projected EN counterclockwise; true north is separate and clockwise from model +Y. Missing references stay unknown; preview site_context_change shows full before/after. Control residual consistency is not source approval.","effects":{"prepare":"no mutation","commit":"shared Editor transaction and journal","cancel":"discard immutable preview","undo":"unchanged top transaction only"}}),
        );
    }
    if name == "3d_style" {
        let root = schema();
        let mut edit = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == "set3d_style")?
            .clone();
        edit["$defs"] = referenced_defs(&edit, &root["$defs"]);
        return Some(
            json!({"name":name,"version":1,"native_type":false,"input_schema":edit,"parameters_schema":edit,"output_schema":preview_schema(),"usage":"Set view presentation through change.edits; no geometry change. Shared preview, commit, cancel and Undo."}),
        );
    }
    if name == "story_elevation" {
        let root = schema();
        let edit = root["$defs"]["Edit"]["oneOf"]
            .as_array()?
            .iter()
            .find(|v| v["properties"]["operation"]["const"] == "set_story_elevation")?
            .clone();
        return Some(
            json!({"name":name,"version":1,"native_type":false,"units":{"length":"m","area":"m2","angle":"degrees"},
            "input_schema":edit,"parameters_schema":edit,"output_schema":preview_schema(),
            "usage":"Put this object in change.edits of prepare_native_change or prepare_discovered_change. It is an edit, never a creates.geometry command.",
            "semantics":"Set one existing story's absolute project elevation in metres. Preserve story ID/name and every element's relative parameters. No automatic upper-story cascade or wall resizing; explicitly batch required wall updates. Refuse missing story, nonfinite/out-of-range elevation, or locked elements on a moved story. One shared atomic transaction/Undo. Preview story_changes lists before/after elevations and world-geometry affected IDs; receipt changed_ids includes those IDs. No-op elevation changes produce no standalone transaction.",
            "preconditions":["explicit current target and expected revision","existing story; absolute elevation within -10000..10000m","fresh context guard for context-dependent input"],
            "effects":{"prepare":"immutable preview, no model/history change","commit":"existing Editor transaction and durable operation journal","cancel":"existing preview cancellation","undo":"unchanged top transaction only"}}),
        );
    }
    if !COMMANDS.contains(&name) {
        return None;
    }
    let root = schema();
    let variant = root["$defs"]["Geometry"]["oneOf"]
        .as_array()?
        .iter()
        .find(|v| v["properties"]["type"]["const"] == name)?
        .clone();
    let definitions = referenced_defs(&variant, &root["$defs"]);
    let mut parameters = variant.clone();
    parameters["properties"].as_object_mut()?.remove("type");
    if let Some(required) = parameters["required"].as_array_mut() {
        required.retain(|v| v != "type");
    }
    let parameters_schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","allOf":[parameters],"$defs":definitions});
    Some(
        json!({"name":name,"version":1,"units":{"length":"m","area":"m2","angle":"degrees"},"coordinate_space":"project","input_schema":{"$schema":"https://json-schema.org/draft/2020-12/schema","allOf":[variant],"$defs":definitions},"parameters_schema":parameters_schema,"output_schema":preview_schema(),"output_applies_to":"prepare_native_change success structured result, not an element created by describe_command","preconditions":["explicit instance/document target","expected revision","fresh expected_context_revision for selection/current-story dependent input","existing writable story/layer","prepare then immutable commit"],"concept_mass_semantics":if name=="concept_mass" {json!({"boundary":"authored straight polygon minus through holes; not a bounding box proxy", "height":"vertical envelope rise in metres; bottom_offset relative to home story", "footprint":"geometric footprint minus through holes, not floor or space area", "volume":"geometric envelope volume, not construction material or habitable space", "relationships":"no automatic Zone association; no floor count or facade/opening inference", "scope":"explicit concept intent, not surveyed/approved facts"})}else{Value::Null},"zone_semantics":if name=="zone" {json!({"space_name":"Unicode geometry.space_name; creates.name is only the ASCII transaction reference", "number":"Z plus stable element ID", "boundary":"explicit outline with holes; not associated with walls", "height":"clear height, default 3m; bottom_offset relative to home story", "label":"optional label_position; null finds an interior point automatically", "surface":"optional opaque solid RGB/name or inherit construction material. Omitted on update preserves the existing surface. Explicit inherit resets it. Separate from purpose/category and construction material; no texture or transparency support.", "area":"geometric boundary area minus holes", "volume":"area times clear height; not material volume", "analysis":"use analyze_spaces for overlap, near boundaries and explicit-domain coverage; no door/code compliance claim"})} else {Value::Null},"semantics":"native type; existing shared geometry, validation, atomic BatchChange, one undo; creates resolve together then edits in order. Update is a complete geometry replacement preserving identity/story/layer/material/other metadata; every geometry field is required unless schema supplies a default. Read shape plus slab_contour for complete slab outline, holes, arcs, edge settings and elevation; wall_contour exposes optional authored wall contours. Do not infer editable contours from display bounds. Remove of a host wall cascades to its hosted openings; preview changes and dependencies list the affected elements; any locked dependent rejects the batch. Move of a wall carries hosted geometry through its host reference; moving an opening alone follows the existing host-axis rules.","effects":{"prepare":"immutable preview; no IDs consumed, no history change","commit":"atomic shared typed transaction; one Undo; dependent opening hosts resolved together","cancel":"removes uncommitted preview; no rollback after commit","undo":"only unchanged top transaction; later edits cause UNDO_CONFLICT"},"errors":["TARGET_MISMATCH","REVISION_CONFLICT","CONTEXT_CONFLICT","WRITE_NOT_AUTHORIZED","WRITE_AUTHORIZATION_SUSPENDED","INVALID_NATIVE_COMMAND","INVALID_PROPOSAL","PREVIEW_CAPACITY","BATCH_LIMIT"],"unsupported":"variants absent from schema are not silently approximated; advanced curved creates, multi-plane roofs, multi-flight stairs are outside this v1. Existing unsupported curved parameters cannot be replaced through this schema."}),
    )
}
