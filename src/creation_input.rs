//! Transient polygon input. Keep numeric precision until model conversion.
use crate::model::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreationPoint {
    pub x: f64,
    pub y: f64,
}
impl CreationPoint {
    pub fn model_point(self) -> Point {
        Point::new(self.x, self.y)
    }
    pub fn valid(self) -> bool {
        crate::precision::GeometryContext::point_in_range(self.model_point())
    }
}
impl From<Point> for CreationPoint {
    fn from(p: Point) -> Self {
        Self { x: p.x, y: p.y }
    }
}

/// The model cache supports existing display/snap callers; acceptance and
/// backtracking mutate both representations together. Rejected input changes neither.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PolygonCreationInput {
    precise: Vec<CreationPoint>,
    model: Vec<Point>,
}
impl PolygonCreationInput {
    /// Retract accepted input in both representations; cursor input is derived.
    pub fn backtrack(&mut self) -> bool {
        if self.precise.pop().is_none() {
            return false;
        }
        self.model.pop();
        true
    }
    pub fn points(&self) -> &[Point] {
        &self.model
    }
    pub fn precise_points(&self) -> &[CreationPoint] {
        &self.precise
    }
    pub fn accept(&mut self, point: CreationPoint) -> Result<(), String> {
        if !point.valid() || self.precise.len() >= 10_000 {
            return Err("Invalid edit parameters".into());
        }
        self.precise.push(point);
        self.model.push(point.model_point());
        Ok(())
    }
    pub fn with_cursor(&self, cursor: Option<Point>) -> Vec<CreationPoint> {
        // Display rounding must not append a substitute for the last exact node.
        self.with_precise_cursor(
            cursor
                .filter(|p| self.model.last() != Some(p))
                .map(Into::into),
        )
    }
    pub fn with_precise_cursor(&self, cursor: Option<CreationPoint>) -> Vec<CreationPoint> {
        let mut points = self.precise.clone();
        if let Some(cursor) = cursor
            && points.last() != Some(&cursor)
        {
            points.push(cursor);
        }
        points
    }
    pub fn numeric_target(&self, length: f64, angle_degrees: f64) -> Result<CreationPoint, String> {
        let start = self.precise.last().ok_or("Invalid edit parameters")?;
        if !length.is_finite() || length < 0.01 || !angle_degrees.is_finite() {
            return Err("Invalid edit parameters".into());
        }
        let angle = angle_degrees.to_radians();
        let point = CreationPoint {
            x: start.x + length * angle.cos(),
            y: start.y + length * angle.sin(),
        };
        if !point.valid() {
            return Err("Invalid edit parameters".into());
        }
        Ok(point)
    }
}
