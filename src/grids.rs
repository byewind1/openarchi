//! Construction grid and snap grid settings.
//!
//! Archicad's construction grid is a repeating cycle of main and auxiliary
//! lines. The floor-plan grid is shared by all stories. OpenArchi stores one
//! set of settings in the project document and derives both plan and 3D
//! display from it.

use crate::model::Point;
use serde::{Deserialize, Serialize};

pub const MIN_SCREEN_GAP: f32 = 3.0;
const MAX_LINES: usize = 20_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridSnapMode {
    None,
    #[default]
    SnapGrid,
    ConstructionGrid,
}

impl GridSnapMode {
    pub fn label_key(self) -> &'static str {
        match self {
            Self::None => "No snap",
            Self::SnapGrid => "Snap to Snap Grid",
            Self::ConstructionGrid => "Snap to Construction Grid",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridSettings {
    pub show_construction_grid: bool,
    pub main_spacing_x: f32,
    pub main_spacing_y: f32,
    pub main_steps_x: u32,
    pub main_steps_y: u32,
    pub aux_spacing_x: f32,
    pub aux_spacing_y: f32,
    pub aux_steps_x: u32,
    pub aux_steps_y: u32,
    pub grid_snap_enabled: bool,
    pub snap_mode: GridSnapMode,
    pub snap_spacing_x: f32,
    pub snap_spacing_y: f32,
    pub rotate_enabled: bool,
    pub rotated_display: bool,
    pub origin_x: f32,
    pub origin_y: f32,
    pub rotation_deg: f32,
    pub origin_display: bool,
    pub background: [u8; 4],
    pub grid_line_color: [u8; 4],
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            show_construction_grid: true,
            main_spacing_x: 2.0,
            main_spacing_y: 2.0,
            main_steps_x: 1,
            main_steps_y: 1,
            aux_spacing_x: 0.0,
            aux_spacing_y: 0.0,
            aux_steps_x: 0,
            aux_steps_y: 0,
            grid_snap_enabled: true,
            snap_mode: GridSnapMode::SnapGrid,
            snap_spacing_x: 0.05,
            snap_spacing_y: 0.05,
            rotate_enabled: false,
            rotated_display: false,
            origin_x: 0.0,
            origin_y: 0.0,
            rotation_deg: 45.0,
            origin_display: false,
            background: [250, 250, 248, 255],
            grid_line_color: [218, 222, 213, 255],
        }
    }
}

impl GridSettings {
    /// Enables or disables grid snapping while keeping the remembered snap mode.
    /// Enabling from `None` restores the normal snap grid, matching `Alt+S` and
    /// the Quick Options button.
    pub fn set_snap_enabled(&mut self, enabled: bool) {
        self.grid_snap_enabled = enabled;
        if enabled && matches!(self.snap_mode, GridSnapMode::None) {
            self.snap_mode = GridSnapMode::SnapGrid;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLine {
    pub start: Point,
    pub end: Point,
    pub main: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct GridPattern {
    period: f32,
    offsets: Vec<(f32, bool)>,
}

impl GridPattern {
    fn nearest(&self, value: f32) -> Option<(f32, bool)> {
        let period = self.period;
        if !period.is_finite() || period <= 0.0 || self.offsets.is_empty() {
            return None;
        }
        let base = (value / period).floor();
        let mut best: Option<(f32, f32, bool)> = None;
        for shift in -1..=1 {
            let period_start = (base + shift as f32) * period;
            for (offset, main) in &self.offsets {
                let candidate = period_start + *offset;
                let error = (candidate - value).abs();
                if best.is_none_or(|(best_error, _, _)| error < best_error) {
                    best = Some((error, candidate, *main));
                }
            }
        }
        best.map(|(_, position, main)| (position, main))
    }

    fn between(&self, min: f32, max: f32) -> Vec<(f32, bool)> {
        if !min.is_finite() || !max.is_finite() || max < min {
            return Vec::new();
        }
        let start_k = (min / self.period).floor() as i64 - 1;
        let end_k = (max / self.period).ceil() as i64 + 1;
        let mut out = Vec::new();
        for k in start_k..=end_k {
            let base = k as f32 * self.period;
            for (offset, main) in &self.offsets {
                let position = base + *offset;
                if position >= min - 1e-4 && position <= max + 1e-4 {
                    out.push((position, *main));
                }
            }
            if out.len() > MAX_LINES {
                return Vec::new();
            }
        }
        out.sort_by(|left, right| left.0.total_cmp(&right.0));
        out
    }
}

impl GridSettings {
    pub fn active_angle_radians(&self) -> f32 {
        if self.rotate_enabled && self.rotated_display {
            self.rotation_deg.to_radians()
        } else {
            0.0
        }
    }

    pub fn local_point(&self, point: Point) -> Point {
        let angle = self.active_angle_radians();
        let (sin, cos) = angle.sin_cos();
        let dx = point.x - self.origin_x;
        let dy = point.y - self.origin_y;
        Point::new(dx * cos + dy * sin, -dx * sin + dy * cos)
    }

    pub fn world_point(&self, u: f32, v: f32) -> Point {
        let angle = self.active_angle_radians();
        let (sin, cos) = angle.sin_cos();
        Point::new(
            self.origin_x + u * cos - v * sin,
            self.origin_y + u * sin + v * cos,
        )
    }

    fn cycle(&self, x_axis: bool) -> Vec<(f32, bool)> {
        let (main_spacing, main_steps, aux_spacing, aux_steps) = if x_axis {
            (
                self.main_spacing_x,
                self.main_steps_x,
                self.aux_spacing_x,
                self.aux_steps_x,
            )
        } else {
            (
                self.main_spacing_y,
                self.main_steps_y,
                self.aux_spacing_y,
                self.aux_steps_y,
            )
        };
        let mut cycle = Vec::new();
        if main_steps > 0 && main_spacing > 0.0 {
            for _ in 0..main_steps {
                cycle.push((main_spacing, true));
            }
        }
        if aux_steps > 0 && aux_spacing > 0.0 {
            for _ in 0..aux_steps {
                cycle.push((aux_spacing, false));
            }
        }
        cycle
    }

    fn pattern(&self, x_axis: bool) -> Option<GridPattern> {
        let cycle = self.cycle(x_axis);
        if cycle.is_empty() {
            return None;
        }
        let period: f32 = cycle.iter().map(|(spacing, _)| *spacing).sum();
        if !period.is_finite() || period <= 0.0 {
            return None;
        }
        let mut offsets = Vec::with_capacity(cycle.len());
        let mut position = 0.0;
        let mut index = 0;
        offsets.push((0.0, cycle[0].1));
        loop {
            position += cycle[index].0;
            if position >= period - 1e-4 {
                break;
            }
            index = (index + 1) % cycle.len();
            offsets.push((position, cycle[index].1));
        }
        Some(GridPattern { period, offsets })
    }

    pub fn minimum_gap(&self, x_axis: bool) -> Option<f32> {
        self.cycle(x_axis)
            .into_iter()
            .map(|(spacing, _)| spacing)
            .reduce(f32::min)
    }

    pub fn grid_visible_at(&self, pixels_per_meter: f32) -> bool {
        if !self.show_construction_grid || !pixels_per_meter.is_finite() {
            return false;
        }
        let x_ok = self
            .minimum_gap(true)
            .is_some_and(|gap| gap * pixels_per_meter >= MIN_SCREEN_GAP);
        let y_ok = self
            .minimum_gap(false)
            .is_some_and(|gap| gap * pixels_per_meter >= MIN_SCREEN_GAP);
        x_ok && y_ok
    }

    pub fn lines_in_local(&self, min_u: f32, max_u: f32, min_v: f32, max_v: f32) -> Vec<GridLine> {
        if !self.show_construction_grid {
            return Vec::new();
        }
        let Some(u_pattern) = self.pattern(true) else {
            return Vec::new();
        };
        let Some(v_pattern) = self.pattern(false) else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        for (u, main) in u_pattern.between(min_u, max_u) {
            lines.push(GridLine {
                start: self.world_point(u, min_v),
                end: self.world_point(u, max_v),
                main,
            });
            if lines.len() > MAX_LINES {
                return Vec::new();
            }
        }
        for (v, main) in v_pattern.between(min_v, max_v) {
            lines.push(GridLine {
                start: self.world_point(min_u, v),
                end: self.world_point(max_u, v),
                main,
            });
            if lines.len() > MAX_LINES {
                return Vec::new();
            }
        }
        lines
    }

    pub fn snap_point(&self, point: Point) -> Option<Point> {
        if !self.grid_snap_enabled {
            return None;
        }
        let local = self.local_point(point);
        match self.snap_mode {
            GridSnapMode::None => None,
            GridSnapMode::SnapGrid => {
                if self.snap_spacing_x <= 0.0 || self.snap_spacing_y <= 0.0 {
                    return None;
                }
                let u = (local.x / self.snap_spacing_x).round() * self.snap_spacing_x;
                let v = (local.y / self.snap_spacing_y).round() * self.snap_spacing_y;
                Some(self.world_point(u, v))
            }
            GridSnapMode::ConstructionGrid => {
                let (u, _) = self.pattern(true)?.nearest(local.x)?;
                let (v, _) = self.pattern(false)?.nearest(local.y)?;
                Some(self.world_point(u, v))
            }
        }
    }

    pub fn nudge_distance(&self, x_axis: bool, large: bool) -> f32 {
        if large {
            if x_axis {
                self.main_spacing_x
            } else {
                self.main_spacing_y
            }
        } else if x_axis {
            self.snap_spacing_x
        } else {
            self.snap_spacing_y
        }
    }

    pub fn background_color(&self) -> [u8; 4] {
        self.background
    }

    pub fn line_color(&self, main: bool) -> [u8; 4] {
        let mut color = self.grid_line_color;
        if !main {
            color[3] = (color[3] as f32 * 0.55).round() as u8;
        }
        color
    }

    pub fn angle_from_vector(start: Point, end: Point) -> Option<f32> {
        let delta = Point::new(end.x - start.x, end.y - start.y);
        if delta.x.hypot(delta.y) < 1e-6 {
            return None;
        }
        Some(normalize_angle(delta.y.atan2(delta.x).to_degrees()))
    }

    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("main spacing X", self.main_spacing_x),
            ("main spacing Y", self.main_spacing_y),
            ("auxiliary spacing X", self.aux_spacing_x),
            ("auxiliary spacing Y", self.aux_spacing_y),
            ("snap spacing X", self.snap_spacing_x),
            ("snap spacing Y", self.snap_spacing_y),
            ("local origin X", self.origin_x),
            ("local origin Y", self.origin_y),
            ("rotation angle", self.rotation_deg),
        ] {
            if !value.is_finite() {
                return Err(format!("{name} must be a finite number"));
            }
        }
        if self.main_spacing_x <= 0.0 || self.main_spacing_y <= 0.0 {
            return Err("Main grid spacing must be greater than zero".into());
        }
        if self.main_spacing_x > 1000.0
            || self.main_spacing_y > 1000.0
            || self.aux_spacing_x > 1000.0
            || self.aux_spacing_y > 1000.0
        {
            return Err("Grid spacing must not exceed 1000 m".into());
        }
        if self.aux_spacing_x < 0.0 || self.aux_spacing_y < 0.0 {
            return Err("Auxiliary grid spacing cannot be negative".into());
        }
        for (name, steps) in [
            ("main grid steps X", self.main_steps_x),
            ("main grid steps Y", self.main_steps_y),
            ("auxiliary grid steps X", self.aux_steps_x),
            ("auxiliary grid steps Y", self.aux_steps_y),
        ] {
            if steps > 100 {
                return Err(format!("{name} must be between 0 and 100"));
            }
        }
        if self.aux_steps_x > 0 && self.aux_spacing_x <= 0.0 {
            return Err("Auxiliary grid spacing X must be greater than zero".into());
        }
        if self.aux_steps_y > 0 && self.aux_spacing_y <= 0.0 {
            return Err("Auxiliary grid spacing Y must be greater than zero".into());
        }
        if self.snap_spacing_x <= 0.0 || self.snap_spacing_y <= 0.0 {
            return Err("Snap grid spacing must be greater than zero".into());
        }
        if self.snap_spacing_x > 1000.0 || self.snap_spacing_y > 1000.0 {
            return Err("Snap grid spacing must not exceed 1000 m".into());
        }
        if self.rotation_deg.abs() > 3600.0 {
            return Err("Rotation angle must be between -3600° and 3600°".into());
        }
        Ok(())
    }
}

pub fn normalize_angle(degrees: f32) -> f32 {
    let mut angle = degrees % 360.0;
    if angle <= -180.0 {
        angle += 360.0;
    } else if angle > 180.0 {
        angle -= 360.0;
    }
    if angle.abs() < 1e-4 { 0.0 } else { angle }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_auxiliary_cycle_starts_at_origin() {
        let mut settings = GridSettings::default();
        settings.aux_spacing_x = 1.0;
        settings.aux_steps_x = 2;
        settings.aux_spacing_y = 1.0;
        settings.aux_steps_y = 2;
        let pattern = settings.pattern(true).unwrap();
        assert_eq!(pattern.period, 4.0);
        assert_eq!(
            pattern.offsets,
            vec![(0.0, true), (2.0, false), (3.0, false)]
        );
        let forward = pattern.between(0.0, 6.0);
        assert_eq!(
            forward,
            vec![
                (0.0, true),
                (2.0, false),
                (3.0, false),
                (4.0, true),
                (6.0, false),
            ]
        );
    }

    #[test]
    fn zero_steps_hide_that_grid_kind() {
        let mut settings = GridSettings::default();
        settings.main_steps_x = 0;
        settings.aux_spacing_x = 0.5;
        settings.aux_steps_x = 2;
        assert_eq!(
            settings.pattern(true).unwrap().between(0.0, 1.6),
            vec![(0.0, false), (0.5, false), (1.0, false), (1.5, false)]
        );
        settings.aux_steps_x = 0;
        assert!(settings.pattern(true).is_none());
    }

    #[test]
    fn rotated_snap_grid_rounds_in_local_coordinates() {
        let mut settings = GridSettings::default();
        settings.grid_snap_enabled = true;
        settings.snap_spacing_x = 0.5;
        settings.snap_spacing_y = 0.5;
        settings.rotate_enabled = true;
        settings.rotated_display = true;
        settings.rotation_deg = 90.0;
        let snapped = settings.snap_point(Point::new(0.26, 0.24)).unwrap();
        assert!((snapped.x - 0.5).abs() < 1e-4);
        assert!((snapped.y - 0.0).abs() < 1e-4);
    }

    #[test]
    fn construction_snap_uses_nearest_main_or_aux_line() {
        let mut settings = GridSettings::default();
        settings.main_spacing_x = 2.0;
        settings.main_spacing_y = 2.0;
        settings.aux_spacing_x = 1.0;
        settings.aux_spacing_y = 1.0;
        settings.aux_steps_x = 2;
        settings.aux_steps_y = 2;
        settings.snap_mode = GridSnapMode::ConstructionGrid;
        assert_eq!(
            settings.snap_point(Point::new(1.2, 3.9)),
            Some(Point::new(2.0, 4.0))
        );
        settings.grid_snap_enabled = false;
        assert_eq!(settings.snap_point(Point::new(1.2, 3.9)), None);
    }

    #[test]
    fn display_density_and_validation() {
        let settings = GridSettings::default();
        assert!(settings.grid_visible_at(10.0));
        assert!(!settings.grid_visible_at(1.0));
        let mut invalid = settings.clone();
        invalid.main_spacing_x = 0.0;
        assert!(invalid.validate().is_err());
        invalid = settings.clone();
        invalid.aux_steps_x = 2;
        assert!(invalid.validate().is_err());
        invalid = settings;
        invalid.snap_spacing_y = 0.0;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn vector_angle_is_normalized() {
        assert_eq!(
            GridSettings::angle_from_vector(Point::new(0.0, 0.0), Point::new(1.0, -1.0)),
            Some(-45.0)
        );
        assert_eq!(
            GridSettings::angle_from_vector(Point::new(0.0, 0.0), Point::new(-1.0, 0.0)),
            Some(180.0)
        );
        assert_eq!(
            GridSettings::angle_from_vector(Point::new(0.0, 0.0), Point::new(0.0, 0.0)),
            None
        );
    }
}
