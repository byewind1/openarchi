//! Center/radius/endpoint input state shared by future UI entry points.
//! Drafts contain accepted model coordinates; they never mutate a project.
use crate::model::{ArcSegment, Point};
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BeamGeometryMethod {
    #[default]
    Single,
    CenterRadius,
}
impl BeamGeometryMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::CenterRadius => "Center and radius",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamArcCandidate {
    pub start: Point,
    pub end: Point,
    pub arc: ArcSegment,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RadiusState {
    start: Point,
    radius: f64,
    start_angle: f64,
    last_angle: Option<f64>,
    sweep: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamArcDraft {
    center: Point,
    accepted: Option<RadiusState>,
}

fn valid_point(p: Point) -> bool {
    crate::precision::GeometryContext::point_in_range(p)
}
fn invalid() -> String {
    "Invalid beam geometry or parameters".into()
}

impl BeamArcDraft {
    pub fn new(center: Point) -> Result<Self, String> {
        if !valid_point(center) {
            return Err(invalid());
        }
        Ok(Self {
            center,
            accepted: None,
        })
    }
    pub fn center(self) -> Point {
        self.center
    }
    pub fn start(self) -> Option<Point> {
        self.accepted.map(|s| s.start)
    }
    pub fn radius(self) -> Option<f64> {
        self.accepted.map(|s| s.radius as f64)
    }
    pub fn sweep_degrees(self) -> Option<f64> {
        self.accepted.map(|s| s.sweep.to_degrees())
    }

    /// Accept the snapped radius point once. Invalid input leaves the draft intact.
    pub fn accept_radius(&mut self, start: Point) -> Result<(), String> {
        if self.accepted.is_some() || !valid_point(start) {
            return Err(invalid());
        }
        let dx = start.x as f64 - self.center.x as f64;
        let dy = start.y as f64 - self.center.y as f64;
        let radius = dx.hypot(dy);
        if !crate::precision::segment_meets_minimum(
            self.center,
            start,
            crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
        ) {
            return Err(invalid());
        }
        self.accepted = Some(RadiusState {
            start,
            radius,
            start_angle: dy.atan2(dx),
            last_angle: None,
            sweep: 0.,
        });
        Ok(())
    }

    /// Radius numeric entry preserves the currently indicated bearing.
    pub fn accept_numeric_radius(
        &mut self,
        bearing_point: Point,
        radius: f64,
    ) -> Result<(), String> {
        if !valid_point(bearing_point) || !radius.is_finite() || radius < 0.01 {
            return Err(invalid());
        }
        let dx = bearing_point.x as f64 - self.center.x as f64;
        let dy = bearing_point.y as f64 - self.center.y as f64;
        let length = dx.hypot(dy);
        if length < 1e-6 {
            return Err(invalid());
        }
        self.accept_radius(Point::new(
            (self.center.x as f64 + dx / length * radius as f64) as f64,
            (self.center.y as f64 + dy / length * radius as f64) as f64,
        ))
    }

    /// Track the actual pointer path. The initial clockwise quadrant matches
    /// the bounded AC29 samples; subsequent samples unwrap continuously.
    /// Coincident endpoints/full turns stay in the draft and cannot be committed.
    pub fn update_cursor(&mut self, cursor: Point) -> Result<Option<BeamArcCandidate>, String> {
        if !valid_point(cursor) {
            return Err(invalid());
        }
        let mut state = self.accepted.ok_or_else(invalid)?;
        let dx = cursor.x as f64 - self.center.x as f64;
        let dy = cursor.y as f64 - self.center.y as f64;
        if dx.hypot(dy) < 1e-6 {
            return Err(invalid());
        }
        let angle = (dy.atan2(dx) - state.start_angle).rem_euclid(TAU);
        // Rendering the unchanged radius point is not a direction choice.
        if state.last_angle.is_none() && (angle < 1e-8 || TAU - angle < 1e-8) {
            return Ok(None);
        }
        state.sweep = match state.last_angle {
            None => {
                if angle > 1.5 * PI {
                    angle - TAU
                } else {
                    angle
                }
            }
            Some(previous) => state.sweep + (angle - previous + PI).rem_euclid(TAU) - PI,
        };
        state.last_angle = Some(angle);
        self.accepted = Some(state);
        Ok(self.candidate())
    }

    /// Explicit geometric sweep assignment; this does not define keyboard UI
    /// semantics for negative angles, which still require separate acceptance.
    pub fn set_sweep_degrees(&mut self, degrees: f64) -> Result<BeamArcCandidate, String> {
        if !degrees.is_finite() || degrees.abs() < 1e-6 || degrees.abs() >= 360. {
            return Err(invalid());
        }
        let mut next = *self;
        let state = next.accepted.as_mut().ok_or_else(invalid)?;
        state.sweep = degrees.to_radians();
        state.last_angle = Some(state.sweep.rem_euclid(TAU));
        let candidate = next.candidate().ok_or_else(invalid)?;
        *self = next;
        Ok(candidate)
    }

    pub fn candidate(self) -> Option<BeamArcCandidate> {
        let s = self.accepted?;
        if s.sweep.abs() < 1e-8 || s.sweep.abs() >= TAU - 1e-8 {
            return None;
        }
        let angle = s.start_angle + s.sweep;
        let end = Point::new(
            (self.center.x as f64 + s.radius * angle.cos()) as f64,
            (self.center.y as f64 + s.radius * angle.sin()) as f64,
        );
        if !valid_point(end) {
            return None;
        }
        let arc = ArcSegment {
            radius: s.radius as f64,
            clockwise: s.sweep < 0.,
            large_arc: s.sweep.abs() > PI,
        };
        crate::geometry::arc_center_sweep(s.start, end, arc)?;
        Some(BeamArcCandidate {
            start: s.start,
            end,
            arc,
        })
    }
}
