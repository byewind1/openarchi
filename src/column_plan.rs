//! Derived plan geometry for constant-section columns. Callers supply resolved
//! world elevations; no local-template cut-plane default is guessed here.
use crate::{column_inclination::InclinedColumnSolid, model::Point};
use geo::{ConvexHull, MultiPoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColumnPlanDisplay {
    ProjectedWithOverhead,
    Projected,
    CutOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub enum ColumnProjectionRange {
    EntireElement,
    /// Already resolved from story indices and offsets to world elevations.
    FloorPlan {
        lower: f64,
        upper: f64,
    },
    Above {
        lower: f64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColumnProjection {
    EntireElement,
    FloorPlanRange,
    AboveAbsoluteLimit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnPlanSettings {
    pub display: ColumnPlanDisplay,
    pub projection: ColumnProjection,
}
impl Default for ColumnPlanSettings {
    fn default() -> Self {
        Self {
            display: ColumnPlanDisplay::Projected,
            projection: ColumnProjection::FloorPlanRange,
        }
    }
}
impl ColumnPlanSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
/// Current AC29 test-template values; no claim of a universal regional default.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorPlanCutSettings {
    pub height: f64,
    pub lower_story_offset: i32,
    pub upper_story_offset: i32,
    pub lower_offset: f64,
    pub upper_offset: f64,
    pub absolute_lower: f64,
}
impl Default for FloorPlanCutSettings {
    fn default() -> Self {
        Self {
            height: 1.1,
            lower_story_offset: 0,
            upper_story_offset: 1,
            lower_offset: 0.,
            upper_offset: 0.,
            absolute_lower: 0.,
        }
    }
}
impl FloorPlanCutSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(self) -> Result<(), String> {
        if [
            self.height,
            self.lower_offset,
            self.upper_offset,
            self.absolute_lower,
        ]
        .into_iter()
        .any(|v| !v.is_finite() || v.abs() >= 100000.)
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn resolve(
        self,
        project: &crate::model::Project,
        story: u32,
        settings: ColumnPlanSettings,
    ) -> Result<(f64, ColumnProjectionRange), String> {
        self.validate()?;
        let mut stories: Vec<_> = project.stories.iter().collect();
        stories.sort_by(|a, b| a.elevation.total_cmp(&b.elevation).then(a.id.cmp(&b.id)));
        let index = stories
            .iter()
            .position(|s| s.id == story)
            .ok_or("The current floor plan story does not exist")?;
        let base = stories[index].elevation;
        let range = if settings.display == ColumnPlanDisplay::CutOnly {
            ColumnProjectionRange::EntireElement
        } else {
            match settings.projection {
                ColumnProjection::EntireElement => ColumnProjectionRange::EntireElement,
                ColumnProjection::AboveAbsoluteLimit => ColumnProjectionRange::Above {
                    lower: self.absolute_lower,
                },
                ColumnProjection::FloorPlanRange => {
                    let at = |offset: i32| -> Result<f64, String> {
                        let i = index as i64 + i64::from(offset);
                        if i < 0 {
                            return Err(missing_reference());
                        }
                        stories
                            .get(i as usize)
                            .map(|s| s.elevation)
                            .ok_or_else(missing_reference)
                    };
                    ColumnProjectionRange::FloorPlan {
                        lower: at(self.lower_story_offset)? + self.lower_offset,
                        upper: at(self.upper_story_offset)? + self.upper_offset,
                    }
                }
            }
        };
        Ok((base + self.height, range))
    }
}
/// Story membership for the inclined-column All Relevant Stories path.
/// The final stored story has an open upper bound, not an extrapolated height.
pub fn intersects_story(
    project: &crate::model::Project,
    element: &crate::model::Element,
    story: u32,
) -> bool {
    let Some(level) = project.stories.iter().find(|s| s.id == story) else {
        return false;
    };
    let Some(solid) = crate::column::inclined_solid(project, element) else {
        return false;
    };
    let lower = level.elevation;
    let upper = project
        .stories
        .iter()
        .filter(|s| s.elevation > lower)
        .map(|s| s.elevation)
        .min_by(f64::total_cmp)
        .unwrap_or(f64::INFINITY);
    solid.column.bottom < upper && solid.column.bottom + solid.column.height > lower
}

/// Basic plan references, deliberately distinct from editable control nodes.
pub fn reference_points(
    project: &crate::model::Project,
    element: &crate::model::Element,
    story: u32,
) -> Vec<Point> {
    if !intersects_story(project, element, story) {
        return vec![];
    }
    let Some(Ok(plan)) = for_element(project, element, story) else {
        return vec![];
    };
    if plan.cut.is_empty() {
        return vec![];
    }
    let lo = plan
        .cut
        .iter()
        .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
            Point::new(a.x.min(p.x), a.y.min(p.y))
        });
    let hi = plan
        .cut
        .iter()
        .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
            Point::new(a.x.max(p.x), a.y.max(p.y))
        });
    let mut points = vec![Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)];
    if matches!(
        element.shape,
        crate::model::Shape::Column {
            section: crate::column::ColumnSection::Rectangular { .. },
            ..
        }
    ) {
        points.extend(plan.cut);
    }
    points
}

/// Only inclined columns use the new view path; legacy vertical presentation is preserved.
pub fn for_element(
    project: &crate::model::Project,
    element: &crate::model::Element,
    story: u32,
) -> Option<Result<ColumnPlanGeometry, String>> {
    let crate::model::Shape::Column {
        inclination: Some(_),
        ..
    } = element.shape
    else {
        return None;
    };
    Some(
        crate::column_geometry::derive_plan(project, element, story)
            .map_err(|d| d.message)
            .and_then(|g| match g.view {
                crate::column_geometry::ColumnPlanView::Projected { geometry, .. } => Ok(geometry),
                _ => Err(invalid()),
            }),
    )
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnPlanGeometry {
    /// Real horizontal section, never the projected footprint or bounding box.
    pub cut: Vec<Point>,
    /// These projections may overlap the cut; painting order is a view concern.
    pub uncut: Vec<Point>,
    pub overhead: Vec<Point>,
}
impl ColumnPlanGeometry {
    pub fn visible_outline(&self) -> Vec<Point> {
        let points: MultiPoint<f64> = self
            .cut
            .iter()
            .chain(&self.uncut)
            .chain(&self.overhead)
            .map(|p| (f64::from(p.x), f64::from(p.y)))
            .collect();
        let hull = points.convex_hull();
        let ring = &hull.exterior().0;
        ring.iter()
            .take(ring.len().saturating_sub(1))
            .map(|p| Point::new(p.x as f64, p.y as f64))
            .collect()
    }
}
fn missing_reference() -> String {
    "The floor plan range references a missing story. Choose Entire element or adjust the range."
        .into()
}
fn invalid() -> String {
    "Invalid column dimensions or position".into()
}
impl InclinedColumnSolid {
    /// Convex orthographic projection of the solid within a world-height interval.
    pub fn projected_outline(self, lower: f64, upper: f64) -> Result<Vec<Point>, String> {
        self.bounds()?;
        if !lower.is_finite() || !upper.is_finite() || lower > upper {
            return Err(invalid());
        }
        let lower = lower.max(self.column.bottom);
        let upper = upper.min(self.column.bottom + self.column.height);
        if lower > upper {
            return Ok(Vec::new());
        }
        let mut points = self.section_at(lower)?;
        if lower == upper {
            return Ok(points);
        }
        points.extend(self.section_at(upper)?);
        let points: MultiPoint<f64> = points
            .into_iter()
            .map(|p| (f64::from(p.x), f64::from(p.y)))
            .collect();
        let hull = points.convex_hull();
        let ring = &hull.exterior().0;
        Ok(ring
            .iter()
            .take(ring.len().saturating_sub(1))
            .map(|p| Point::new(p.x as f64, p.y as f64))
            .collect())
    }
    pub fn plan_geometry(
        self,
        cut_elevation: f64,
        display: ColumnPlanDisplay,
        range: ColumnProjectionRange,
    ) -> Result<ColumnPlanGeometry, String> {
        self.bounds()?;
        if !cut_elevation.is_finite() {
            return Err(invalid());
        }
        let bottom = self.column.bottom;
        let top = bottom + self.column.height;
        // Show Projection applies only to the two projected display modes.
        if display == ColumnPlanDisplay::CutOnly {
            return Ok(ColumnPlanGeometry {
                cut: if (bottom..=top).contains(&cut_elevation) {
                    self.section_at(cut_elevation)?
                } else {
                    Vec::new()
                },
                ..Default::default()
            });
        }
        let (cut, lower, upper) = match range {
            ColumnProjectionRange::EntireElement => (cut_elevation, bottom, top),
            ColumnProjectionRange::FloorPlan { lower, upper } => {
                if !lower.is_finite() || !upper.is_finite() {
                    return Err(invalid());
                }
                let upper = upper.max(lower);
                (
                    cut_elevation.clamp(lower, upper),
                    lower.max(bottom),
                    upper.min(top),
                )
            }
            ColumnProjectionRange::Above { lower } => {
                if !lower.is_finite() {
                    return Err(invalid());
                }
                (cut_elevation, lower.max(bottom), top)
            }
        };
        if lower > upper {
            return Ok(ColumnPlanGeometry::default());
        }
        Ok(ColumnPlanGeometry {
            cut: if (lower..=upper).contains(&cut) {
                self.section_at(cut)?
            } else {
                Vec::new()
            },
            uncut: if cut > lower {
                self.projected_outline(lower, upper.min(cut))?
            } else {
                Vec::new()
            },
            overhead: if display == ColumnPlanDisplay::ProjectedWithOverhead && cut < upper {
                self.projected_outline(lower.max(cut), upper)?
            } else {
                Vec::new()
            },
        })
    }
}
