//! Shared, validated slab creation. UI drafts never write to the project.
use crate::{
    command::ModelCommand,
    editor::Editor,
    geometry,
    model::{Element, Point, Shape, SlabContour},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlabGeometryMethod {
    #[default]
    Rectangular,
    RotatedRectangle,
    Polygonal,
}

impl SlabGeometryMethod {
    pub fn status_key(self) -> &'static str {
        match self {
            Self::Rectangular => "Slab · Click two opposite corners · Enter commits Tracker values",
            Self::RotatedRectangle => "Slab · Click the rotation baseline start",
            Self::Polygonal => "Slab · Click polygon nodes and close at the first node",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlabDraft {
    BaselineEnd { start: Point },
    Width { start: Point, end: Point },
}

impl SlabDraft {
    /// Numeric counterpart of the same two pointer stages. Positive width is
    /// to the left of the accepted baseline, independent of cursor location.
    pub fn numeric_target(self, value: f64, angle_degrees: f64) -> Result<Point, String> {
        if !value.is_finite() {
            return Err("Slab dimensions must be finite".into());
        }
        let point = match self {
            Self::BaselineEnd { start } => {
                if value < 0.01 || !angle_degrees.is_finite() {
                    return Err("Slab rotation baseline needs a valid length and angle".into());
                }
                let angle = angle_degrees.to_radians();
                Point::new(start.x + value * angle.cos(), start.y + value * angle.sin())
            }
            Self::Width { start, end } => {
                if value.abs() < 0.01 {
                    return Err("Slab needs a non-zero perpendicular width".into());
                }
                let length = start.distance(end);
                if !crate::precision::segment_meets_minimum(
                    start,
                    end,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                ) {
                    return Err("Slab rotation baseline needs two distinct points".into());
                }
                Point::new(
                    start.x - (end.y - start.y) * value / length,
                    start.y + (end.x - start.x) * value / length,
                )
            }
        };
        if !crate::precision::GeometryContext::point_in_range(point) {
            return Err("Coordinates must be finite and within the local modeling range".into());
        }
        Ok(point)
    }

    pub fn numeric_values(self, point: Point) -> (f64, f64) {
        match self {
            Self::BaselineEnd { start } => (
                start.distance(point),
                (point.y - start.y).atan2(point.x - start.x).to_degrees(),
            ),
            Self::Width { start, end } => (
                ((end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x))
                    / start.distance(end),
                0.,
            ),
        }
    }

    pub fn baseline(self) -> (Point, Option<Point>) {
        match self {
            Self::BaselineEnd { start } => (start, None),
            Self::Width { start, end } => (start, Some(end)),
        }
    }
}

/// Accepted polygon input; candidates expose every regularized region unchanged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlabPolygonDraft {
    input: crate::creation_input::PolygonCreationInput,
}

impl SlabPolygonDraft {
    pub(crate) fn backtrack(&mut self) -> bool {
        self.input.backtrack()
    }
    pub fn points(&self) -> &[Point] {
        self.input.points()
    }

    pub fn accept_vertex(&mut self, point: Point) -> Result<(), String> {
        self.accept_precise_vertex(point.into())
    }
    pub fn accept_precise_vertex(
        &mut self,
        point: crate::creation_input::CreationPoint,
    ) -> Result<(), String> {
        self.input.accept(point)
    }
    pub fn precise_points(&self) -> &[crate::creation_input::CreationPoint] {
        self.input.precise_points()
    }
    pub fn precise_numeric_target(
        &self,
        length: f64,
        angle: f64,
    ) -> Result<crate::creation_input::CreationPoint, String> {
        self.input.numeric_target(length, angle)
    }

    pub fn numeric_target(&self, length: f64, angle: f64) -> Result<Point, String> {
        let start = *self.points().last().ok_or("Invalid edit parameters")?;
        SlabDraft::BaselineEnd { start }.numeric_target(length, angle)
    }

    pub fn candidates(
        &self,
        cursor: Option<Point>,
        elevation: f64,
    ) -> Result<Vec<SlabContour>, String> {
        self.regions(cursor, elevation)
            .map(|regions| regions.into_iter().map(|region| region.contour).collect())
    }
    pub fn regions(
        &self,
        cursor: Option<Point>,
        elevation: f64,
    ) -> Result<Vec<crate::slab_geometry::RegularizedSlabRegion>, String> {
        crate::slab_geometry::regularize_precise_creation_regions(
            &self.input.with_cursor(cursor),
            elevation,
        )
    }
    pub fn precise_regions(
        &self,
        cursor: Option<crate::creation_input::CreationPoint>,
        elevation: f64,
    ) -> Result<Vec<crate::slab_geometry::RegularizedSlabRegion>, String> {
        crate::slab_geometry::regularize_precise_creation_regions(
            &self.input.with_precise_cursor(cursor),
            elevation,
        )
    }
}

impl Editor {
    pub(crate) fn clear_slab_creation(&mut self) {
        self.slab_draft = None;
        if self.slab_polygon_draft.take().is_some() {
            self.anchor = None;
        }
    }

    /// Internal entry only: ordinary UI closure/region selection is not defined here.
    pub fn begin_slab_polygon(&mut self) -> Result<(), String> {
        if self.tool != crate::editor::Tool::Slab
            || self.project.plan_options.active_worksheet.is_some()
            || self.slab_draft.is_some()
            || (self.anchor.is_some() && self.slab_polygon_draft.is_none())
        {
            return Err("Invalid edit parameters".into());
        }
        self.slab_polygon_draft
            .get_or_insert_with(SlabPolygonDraft::default);
        Ok(())
    }

    pub fn slab_polygon_draft(&self) -> Option<&SlabPolygonDraft> {
        self.slab_polygon_draft.as_ref()
    }

    pub fn accept_slab_polygon_vertex(&mut self, point: Point) -> Result<(), String> {
        self.accept_slab_polygon_precise_vertex(point.into())
    }
    pub fn accept_slab_polygon_precise_vertex(
        &mut self,
        point: crate::creation_input::CreationPoint,
    ) -> Result<(), String> {
        self.slab_polygon_draft
            .as_mut()
            .ok_or("Invalid edit parameters")?
            .accept_precise_vertex(point)?;
        self.anchor = Some(point.model_point());
        Ok(())
    }

    /// Accept a numeric node through the same validated path as pointer input.
    pub fn accept_slab_polygon_numeric_vertex(
        &mut self,
        length: f64,
        angle_degrees: f64,
    ) -> Result<(), String> {
        let point = self
            .slab_polygon_draft
            .as_ref()
            .ok_or("Invalid edit parameters")?
            .numeric_target(length, angle_degrees)?;
        self.accept_slab_polygon_vertex(point)
    }

    /// Numeric model/AI input without an intermediate f64 coordinate conversion.
    pub fn accept_slab_polygon_precise_numeric_vertex(
        &mut self,
        length: f64,
        angle_degrees: f64,
    ) -> Result<(), String> {
        let point = self
            .slab_polygon_draft
            .as_ref()
            .ok_or("Invalid edit parameters")?
            .precise_numeric_target(length, angle_degrees)?;
        self.accept_slab_polygon_precise_vertex(point)
    }

    pub fn slab_polygon_candidates(
        &self,
        cursor: Option<Point>,
    ) -> Result<Vec<SlabContour>, String> {
        self.slab_polygon_draft
            .as_ref()
            .ok_or("Invalid edit parameters")?
            .candidates(cursor, self.slab_elevation)
    }

    pub fn slab_polygon_precise_candidates(
        &self,
        cursor: Option<crate::creation_input::CreationPoint>,
    ) -> Result<Vec<SlabContour>, String> {
        self.slab_polygon_draft
            .as_ref()
            .ok_or("Invalid edit parameters")?
            .precise_regions(cursor, self.slab_elevation)
            .map(|regions| regions.into_iter().map(|region| region.contour).collect())
    }

    /// Read-only preview elements share the next ID; they are not a commit batch.
    pub fn slab_polygon_preview(&self, cursor: Point) -> Result<Vec<Element>, String> {
        self.slab_polygon_candidates(Some(cursor))?
            .into_iter()
            .map(|contour| self.slab_creation_candidate(contour))
            .collect()
    }

    pub fn finish_slab_polygon(&mut self) -> Result<Option<u64>, String> {
        let draft = self
            .slab_polygon_draft
            .as_ref()
            .ok_or("Invalid edit parameters")?;
        if draft.precise_points().len() < 3 {
            return Err("Slab polygon needs at least three nodes".into());
        }
        let regions = draft.precise_regions(None, self.slab_elevation)?;
        match crate::slab_geometry::inspect_creation_region_choice(&regions, draft.precise_points())
        {
            crate::slab_geometry::CreationRegionChoice::Empty => {
                self.clear_slab_creation();
                Ok(None)
            }
            crate::slab_geometry::CreationRegionChoice::Unique(i) => self
                .commit_slab_polygon_region(&regions[i].contour)
                .map(Some),
            crate::slab_geometry::CreationRegionChoice::Ambiguous(_) => {
                Err("Slab polygon has unresolved equal regions; edit or cancel the draft".into())
            }
        }
    }

    /// Caller chooses a current explicit region; no implicit ordering or tie policy.
    pub fn commit_slab_polygon_region(&mut self, region: &SlabContour) -> Result<u64, String> {
        if !self.slab_polygon_candidates(None)?.contains(region) {
            return Err("Invalid edit parameters".into());
        }
        let id = self.place_slab_contour(region.clone())?;
        self.clear_slab_creation();
        self.set_selection(vec![id]);
        Ok(id)
    }

    /// Geometry candidate only; full placement checks remain with model commands.
    pub fn slab_creation_candidate(&self, contour: SlabContour) -> Result<Element, String> {
        let mut element = self.creation_candidate(
            self.material,
            self.slab_layer,
            Shape::Slab {
                min: Point::default(),
                max: Point::default(),
                thickness: self.thickness,
            },
        );
        element.worksheet = None;
        crate::slab_geometry::sync(&mut element, contour)?;
        Ok(element)
    }

    /// Create one actual contour, including supported holes/arcs, in one
    /// validated model transaction. This is independent of pointer input.
    pub fn place_slab_contour(&mut self, contour: SlabContour) -> Result<u64, String> {
        let element = self.slab_creation_candidate(contour)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    /// Geometry-only rectangular candidate shared by the pointer preview and commit.
    /// The caller must still pass placement permissions and project validation.
    pub fn rectangular_slab_preview(&self, start: Point, end: Point) -> Result<Element, String> {
        let min = Point::new(start.x.min(end.x), start.y.min(end.y));
        let max = Point::new(start.x.max(end.x), start.y.max(end.y));
        if !crate::precision::axis_meets_minimum(
            min.x,
            max.x,
            crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
        ) || !crate::precision::axis_meets_minimum(
            min.y,
            max.y,
            crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
        ) {
            return Err("Slab needs a non-zero width and depth".into());
        }
        let mut element = self.slab_creation_candidate(SlabContour {
            points: geometry::rectangle_polygon(min, max),
            elevation: self.slab_elevation,
            ..Default::default()
        })?;
        // Preserve the legacy rectangular representation and JSON roundtrip.
        if self.slab_elevation == 0. {
            element.slab_contour = None;
        }
        Ok(element)
    }

    fn place_rectangular_slab(&mut self, start: Point, end: Point) -> Result<u64, String> {
        let element = self.rectangular_slab_preview(start, end)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    pub(crate) fn click_slab(&mut self, pointer: Point, tolerance: f64) {
        if self.slab_method == SlabGeometryMethod::Polygonal {
            let result = if let Some(draft) = self.slab_polygon_draft.as_ref() {
                if draft
                    .points()
                    .first()
                    .is_some_and(|start| start.distance(pointer) <= tolerance.max(0.))
                {
                    self.finish_slab_polygon().map(|_| ())
                } else {
                    self.accept_slab_polygon_vertex(pointer)
                }
            } else {
                self.begin_slab_polygon()
                    .and_then(|_| self.accept_slab_polygon_vertex(pointer))
            };
            if let Err(error) = result {
                self.status = error;
            }
            return;
        }
        if self.slab_polygon_draft.is_some() {
            if let Err(error) = self.accept_slab_polygon_vertex(pointer) {
                self.status = error;
            }
        } else if self.slab_method == SlabGeometryMethod::RotatedRectangle {
            self.click_rotated_slab(pointer);
        } else if let Some(start) = self.anchor {
            match self.place_rectangular_slab(start, pointer) {
                Ok(id) => {
                    self.anchor = None;
                    self.set_selection(vec![id]);
                    self.status = format!("Created element {id:03}");
                }
                Err(error) => self.status = error,
            }
        } else {
            self.anchor = Some(pointer);
        }
    }

    pub fn rotated_slab_preview(&self, pointer: Point) -> Option<Element> {
        let SlabDraft::Width { start, end } = self.slab_draft? else {
            return None;
        };
        let points = geometry::rotated_rectangle_polygon(start, end, pointer)?;
        self.slab_creation_candidate(SlabContour {
            points,
            elevation: self.slab_elevation,
            ..Default::default()
        })
        .ok()
    }

    pub(crate) fn click_rotated_slab(&mut self, pointer: Point) {
        match self.slab_draft {
            None => {
                self.slab_draft = Some(SlabDraft::BaselineEnd { start: pointer });
                self.status = "Slab · Click the rotation baseline end".into();
            }
            Some(SlabDraft::BaselineEnd { start }) => {
                if !crate::precision::segment_meets_minimum(
                    start,
                    pointer,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                ) {
                    self.status = "Slab rotation baseline needs two distinct points".into();
                    return;
                }
                self.slab_draft = Some(SlabDraft::Width {
                    start,
                    end: pointer,
                });
                self.status = "Slab · Click to set the perpendicular width".into();
            }
            Some(SlabDraft::Width { start, end }) => {
                let Some(points) = geometry::rotated_rectangle_polygon(start, end, pointer) else {
                    self.status = "Slab needs a non-zero perpendicular width".into();
                    return;
                };
                match self.place_slab_contour(SlabContour {
                    points,
                    elevation: self.slab_elevation,
                    ..Default::default()
                }) {
                    Ok(id) => {
                        self.slab_draft = None;
                        self.set_selection(vec![id]);
                        self.status = format!("Created element {id:03}");
                    }
                    Err(error) => self.status = error,
                }
            }
        }
    }
}
