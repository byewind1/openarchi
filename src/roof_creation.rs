//! Polygon input data and strict model candidates. This layer does not select
//! regions of self-crossing input or decide how a UI closes/cancels a command.
use crate::{
    editor::Editor,
    model::Point,
    roof::{RoofDraft, RoofSpec},
    roof_contour::RoofContour,
};

/// One read-only analysis: indices in `choice` refer to these exact candidates.
/// This exposes the shared mathematical rule, not an automatic roof commit or
/// a claim that every Archicad roof selection case has been observed.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofPolygonAnalysis {
    candidates: Vec<RoofSpec>,
    choice: crate::slab_geometry::CreationRegionChoice,
}
impl RoofPolygonAnalysis {
    pub fn candidates(&self) -> &[RoofSpec] {
        &self.candidates
    }
    pub fn choice(&self) -> &crate::slab_geometry::CreationRegionChoice {
        &self.choice
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoofPolygonDraft {
    pivot_start: Point,
    pivot_end: Point,
    rise_left: bool,
    input: crate::creation_input::PolygonCreationInput,
}
impl RoofPolygonDraft {
    pub(crate) fn backtrack(&mut self) -> bool {
        self.input.backtrack()
    }
    /// Reuse the already accepted pivot and slope side from the roof command.
    pub fn from_contour_start(draft: RoofDraft) -> Option<Self> {
        let RoofDraft::ContourStart {
            start,
            end,
            rise_left,
        } = draft
        else {
            return None;
        };
        Some(Self {
            pivot_start: start,
            pivot_end: end,
            rise_left,
            input: Default::default(),
        })
    }
    pub fn points(&self) -> &[Point] {
        self.input.points()
    }
    /// Resolve an explicit model-space length/angle from the last accepted node.
    /// This does not accept a point or decide polygon closure/region selection.
    pub fn numeric_target(&self, length: f64, angle_degrees: f64) -> Result<Point, String> {
        let start = self
            .points()
            .last()
            .copied()
            .ok_or("Invalid roof geometry or parameters")?;
        crate::slab_creation::SlabDraft::BaselineEnd { start }
            .numeric_target(length, angle_degrees)
            .map_err(|_| "Invalid roof geometry or parameters".into())
    }
    pub fn accept_vertex(&mut self, point: Point) -> Result<(), String> {
        self.accept_precise_vertex(point.into())
    }
    pub fn accept_precise_vertex(
        &mut self,
        point: crate::creation_input::CreationPoint,
    ) -> Result<(), String> {
        self.input
            .accept(point)
            .map_err(|_| "Invalid roof geometry or parameters".into())
    }
    pub fn precise_points(&self) -> &[crate::creation_input::CreationPoint] {
        self.input.precise_points()
    }
    pub fn precise_numeric_target(
        &self,
        length: f64,
        angle: f64,
    ) -> Result<crate::creation_input::CreationPoint, String> {
        self.input
            .numeric_target(length, angle)
            .map_err(|_| "Invalid roof geometry or parameters".into())
    }
    /// A hovering point belongs only to this candidate, never to accepted input.
    /// Invalid topology remains in the draft for a future creation policy to
    /// handle; it must not be interpreted as a UI instruction to reject closure.
    pub fn candidate(
        &self,
        cursor: Option<Point>,
        parameters: [f64; 3],
    ) -> Result<RoofSpec, String> {
        let mut outer = self.points().to_vec();
        if let Some(point) = cursor
            && outer.last() != Some(&point)
        {
            outer.push(point);
        }
        if outer.len() > 1 && outer.first() == outer.last() {
            outer.pop();
        }
        let contour = RoofContour {
            outer,
            holes: vec![],
        };
        self.spec_for_contour(contour, parameters)
    }

    /// Return all mathematical regions; ordering is not a selection policy.
    pub fn regularized_candidates(
        &self,
        cursor: Option<Point>,
        parameters: [f64; 3],
    ) -> Result<Vec<RoofSpec>, String> {
        self.regions_for_input(&self.input.with_cursor(cursor), parameters)
    }
    pub fn precise_regularized_candidates(
        &self,
        cursor: Option<crate::creation_input::CreationPoint>,
        parameters: [f64; 3],
    ) -> Result<Vec<RoofSpec>, String> {
        self.regions_for_input(&self.input.with_precise_cursor(cursor), parameters)
    }
    pub fn precise_region_analysis(
        &self,
        cursor: Option<crate::creation_input::CreationPoint>,
        parameters: [f64; 3],
    ) -> Result<RoofPolygonAnalysis, String> {
        let input = self.input.with_precise_cursor(cursor);
        let regions = crate::slab_geometry::regularize_precise_creation_regions(&input, 0.)
            .map_err(|_| "Invalid roof geometry or parameters".to_string())?;
        let choice = crate::slab_geometry::inspect_creation_region_choice(&regions, &input);
        let candidates = self.specs_for_regions(regions, parameters)?;
        Ok(RoofPolygonAnalysis { candidates, choice })
    }
    fn regions_for_input(
        &self,
        input: &[crate::creation_input::CreationPoint],
        parameters: [f64; 3],
    ) -> Result<Vec<RoofSpec>, String> {
        let regions = crate::slab_geometry::regularize_precise_creation_regions(input, 0.)
            .map_err(|_| "Invalid roof geometry or parameters".to_string())?;
        self.specs_for_regions(regions, parameters)
    }
    fn specs_for_regions(
        &self,
        regions: Vec<crate::slab_geometry::RegularizedSlabRegion>,
        parameters: [f64; 3],
    ) -> Result<Vec<RoofSpec>, String> {
        regions
            .into_iter()
            .map(|region| {
                self.spec_for_contour(
                    RoofContour {
                        outer: region.contour.points,
                        holes: region
                            .contour
                            .holes
                            .into_iter()
                            .map(|hole| hole.points)
                            .collect(),
                    },
                    parameters,
                )
            })
            .collect()
    }

    fn spec_for_contour(
        &self,
        contour: RoofContour,
        parameters: [f64; 3],
    ) -> Result<RoofSpec, String> {
        contour.validate()?;
        let (min, max) = contour
            .bounds()
            .ok_or("Invalid roof geometry or parameters")?;
        let [pitch, thickness, pivot_offset] = parameters;
        let spec = RoofSpec {
            contour: Some(contour),
            pivot_start: self.pivot_start,
            pivot_end: self.pivot_end,
            rise_left: self.rise_left,
            min,
            max,
            pitch,
            thickness,
            pivot_offset,
        };
        spec.validate()?;
        Ok(spec)
    }
}
impl Editor {
    pub(crate) fn clear_roof_creation(&mut self) -> bool {
        let pivot_active = self.roof_draft.take().is_some();
        let polygon_active = self.roof_polygon_draft.take().is_some();
        pivot_active || polygon_active
    }

    /// Internal session entry until polygon closure policy and UI are complete.
    pub fn begin_roof_polygon(&mut self) -> Result<(), String> {
        if self.tool != crate::editor::Tool::Roof
            || self.project.plan_options.active_worksheet.is_some()
        {
            return Err("Invalid roof geometry or parameters".into());
        }
        if self.roof_polygon_draft.is_some() {
            return Ok(());
        }
        let draft = self
            .roof_draft
            .and_then(RoofPolygonDraft::from_contour_start)
            .ok_or("Invalid roof geometry or parameters")?;
        self.roof_polygon_draft = Some(draft);
        self.status = "Roof · Click the next polygon node".into();
        Ok(())
    }
    pub fn roof_polygon_draft(&self) -> Option<&RoofPolygonDraft> {
        self.roof_polygon_draft.as_ref()
    }
    pub fn finish_roof_polygon(&mut self) -> Result<Option<u64>, String> {
        let draft = self
            .roof_polygon_draft
            .as_ref()
            .ok_or("Invalid roof geometry or parameters")?;
        if draft.precise_points().len() < 3 {
            return Err("Roof polygon needs at least three nodes".into());
        }
        let analysis = draft.precise_region_analysis(None, self.roof_size)?;
        match *analysis.choice() {
            crate::slab_geometry::CreationRegionChoice::Empty => {
                self.clear_roof_creation();
                self.anchor = None;
                self.status = "Roof · Click the pivot line start".into();
                Ok(None)
            }
            crate::slab_geometry::CreationRegionChoice::Unique(i) => self
                .commit_roof_polygon_region(&analysis.candidates()[i])
                .map(Some),
            crate::slab_geometry::CreationRegionChoice::Ambiguous(_) => {
                Err("Roof polygon has unresolved equal regions; edit or cancel the draft".into())
            }
        }
    }
    pub fn accept_roof_polygon_vertex(&mut self, point: Point) -> Result<(), String> {
        self.accept_roof_polygon_precise_vertex(point.into())
    }
    pub fn accept_roof_polygon_precise_vertex(
        &mut self,
        point: crate::creation_input::CreationPoint,
    ) -> Result<(), String> {
        let draft = self
            .roof_polygon_draft
            .as_mut()
            .ok_or("Invalid roof geometry or parameters")?;
        draft.accept_precise_vertex(point)?;
        self.anchor = Some(point.model_point());
        self.status = "Roof · Click the next polygon node".into();
        Ok(())
    }
    /// Numeric and pointer callers share the same accepted-node transition.
    pub fn accept_roof_polygon_numeric_vertex(
        &mut self,
        length: f64,
        angle_degrees: f64,
    ) -> Result<(), String> {
        let point = self
            .roof_polygon_draft
            .as_ref()
            .ok_or("Invalid roof geometry or parameters")?
            .numeric_target(length, angle_degrees)?;
        self.accept_roof_polygon_vertex(point)
    }

    /// Numeric model/AI input without an intermediate f64 coordinate conversion.
    pub fn accept_roof_polygon_precise_numeric_vertex(
        &mut self,
        length: f64,
        angle_degrees: f64,
    ) -> Result<(), String> {
        let point = self
            .roof_polygon_draft
            .as_ref()
            .ok_or("Invalid roof geometry or parameters")?
            .precise_numeric_target(length, angle_degrees)?;
        self.accept_roof_polygon_precise_vertex(point)
    }
    /// Explicit valid-model commit only, not a self-crossing UI closing policy.
    pub fn commit_roof_polygon(&mut self) -> Result<u64, String> {
        let draft = self
            .roof_polygon_draft
            .as_ref()
            .ok_or("Invalid roof geometry or parameters")?;
        let spec = self.roof_polygon_candidate(draft, None)?;
        let id = self.place_roof(spec)?;
        self.clear_roof_creation();
        self.anchor = None;
        self.set_selection(vec![id]);
        self.status = "Roof placed".into();
        Ok(id)
    }

    pub fn roof_polygon_candidates(&self, cursor: Option<Point>) -> Result<Vec<RoofSpec>, String> {
        self.roof_polygon_draft
            .as_ref()
            .ok_or("Invalid roof geometry or parameters")?
            .regularized_candidates(cursor, self.roof_size)
    }

    pub fn roof_polygon_precise_candidates(
        &self,
        cursor: Option<crate::creation_input::CreationPoint>,
    ) -> Result<Vec<RoofSpec>, String> {
        self.roof_polygon_draft
            .as_ref()
            .ok_or("Invalid roof geometry or parameters")?
            .precise_regularized_candidates(cursor, self.roof_size)
    }

    /// The caller supplies a current explicit region; no implicit region choice.
    pub fn commit_roof_polygon_region(&mut self, region: &RoofSpec) -> Result<u64, String> {
        if !self.roof_polygon_candidates(None)?.contains(region) {
            return Err("Invalid roof geometry or parameters".into());
        }
        let id = self.place_roof(region.clone())?;
        self.clear_roof_creation();
        self.anchor = None;
        self.set_selection(vec![id]);
        self.status = "Roof placed".into();
        Ok(id)
    }
    pub fn roof_polygon_candidate(
        &self,
        draft: &RoofPolygonDraft,
        cursor: Option<Point>,
    ) -> Result<RoofSpec, String> {
        draft.candidate(cursor, self.roof_size)
    }
    /// Commit an explicitly valid polygon through the same typed model command
    /// as rectangular roofs. This does not define self-crossing UI behavior.
    pub fn place_roof_polygon(&mut self, draft: &RoofPolygonDraft) -> Result<u64, String> {
        self.place_roof(self.roof_polygon_candidate(draft, None)?)
    }
}
