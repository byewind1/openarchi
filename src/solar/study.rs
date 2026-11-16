use super::*;
use glam::DVec3;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
/// One point with an oriented receiving surface, all in model metres.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receiver {
    pub id: String,
    pub point_m: [f64; 3],
    pub outward_normal: [f64; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub location: Location,
    pub location_basis: LocationBasis,
    pub start_utc: Utc,
    pub end_utc: Utc,
    pub step_seconds: u32,
    pub ray_offset_m: f64,
    pub receivers: Vec<Receiver>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Sunlit,
    Occluded,
    BelowHorizon,
    BackFacing,
    /// Contact lies within the explicit ray tolerance; not counted as sunlight.
    NearSurface,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub start_seconds: u32,
    pub end_seconds: u32,
    pub sample_seconds: f64,
    pub elevation_deg: f64,
    pub state: State,
    pub blocker_id: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub start_seconds: u32,
    pub end_seconds: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointResult {
    pub receiver_id: String,
    pub cells: Vec<Cell>,
    pub sunlit_segments: Vec<Segment>,
    pub sampled_sunlit_seconds: u32,
    #[serde(default)]
    pub uncertain_seconds: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultSet {
    pub schema_version: u32,
    pub algorithm: String,
    pub semantics: String,
    pub snapshot_sha256: String,
    pub input_sha256: String,
    pub request: Request,
    pub points: Vec<PointResult>,
    pub interval_method: String,
}
impl ResultSet {
    /// Input identity only; this does not certify the numerical contents of a saved result.
    pub fn matches_inputs(&self, s: &Snapshot, r: &Request) -> Result<bool, String> {
        Ok(self.schema_version == 1
            && self.algorithm == ALGORITHM
            && self.snapshot_sha256 == hash(s)?
            && self.input_sha256 == hash(&(ALGORITHM, s, r))?
            && self.request == *r)
    }
}
fn valid_receiver(r: &Receiver) -> bool {
    let p = DVec3::from_array(r.point_m);
    let n = DVec3::from_array(r.outward_normal);
    !r.id.trim().is_empty()
        && r.id.len() <= 128
        && !r.id.contains('\0')
        && p.is_finite()
        && p.abs().max_element() <= 1e9
        && n.is_finite()
        && (n.length() - 1.).abs() <= 1e-6
}
/// Shared ray kernel against physical surfaces, not selection proxies or AABBs.
#[cfg(test)]
fn visibility(s: &Snapshot, r: &Receiver, enu: [f64; 3], offset: f64) -> (State, Option<u64>) {
    visibility_controlled(s, r, enu, offset, &AtomicBool::new(false)).unwrap()
}
fn visibility_controlled(
    s: &Snapshot,
    r: &Receiver,
    enu: [f64; 3],
    offset: f64,
    cancel: &AtomicBool,
) -> Result<(State, Option<u64>), String> {
    let d = DVec3::from_array(s.north.model_direction(enu));
    if d.z <= 0. {
        return Ok((State::BelowHorizon, None));
    }
    if DVec3::from_array(r.outward_normal).dot(d) <= 0. {
        return Ok((State::BackFacing, None));
    }
    let origin = DVec3::from_array(r.point_m);
    let mut nearest: Option<(f64, u64)> = None;
    for b in &s.blockers {
        for (index, t) in b.triangles.iter().enumerate() {
            if index % 256 == 0 && cancel.load(Ordering::Acquire) {
                return Err("Job cancelled".into());
            }
            let [a, c, e] = t.map(DVec3::from_array);
            if let Some(distance) = crate::geometry::ray_triangle(origin, d, a, c, e) {
                if nearest.is_none_or(|(old, _)| distance < old) {
                    nearest = Some((distance, b.element_id));
                }
            }
        }
    }
    Ok(match nearest {
        Some((distance, id)) if distance <= offset => (State::NearSurface, Some(id)),
        Some((_, id)) => (State::Occluded, Some(id)),
        None => (State::Sunlit, None),
    })
}
fn sample_cells(duration: u32, step: u32) -> impl Iterator<Item = (u32, u32, f64)> {
    (0..duration.div_ceil(step)).map(move |i| {
        let a = i * step;
        let b = (a + step).min(duration);
        (a, b, (a as f64 + b as f64) * 0.5)
    })
}
fn merge_sunlit(cells: &[Cell]) -> (Vec<Segment>, u32) {
    let mut segments: Vec<Segment> = vec![];
    let mut seconds = 0;
    for c in cells.iter().filter(|c| c.state == State::Sunlit) {
        seconds += c.end_seconds - c.start_seconds;
        if let Some(last) = segments
            .last_mut()
            .filter(|s| s.end_seconds == c.start_seconds)
        {
            last.end_seconds = c.end_seconds;
        } else {
            segments.push(Segment {
                start_seconds: c.start_seconds,
                end_seconds: c.end_seconds,
            });
        }
    }
    (segments, seconds)
}
impl Request {
    fn interval(&self) -> Result<(f64, u32, u32), String> {
        self.location.validate()?;
        self.location_basis.validate()?;
        let start = self.start_utc.julian_day()?;
        let end = self.end_utc.julian_day()?;
        let duration = ((end - start) * 86400.).round();
        if !(1. ..=86400.).contains(&duration)
            || !(1..=3600).contains(&self.step_seconds)
            || self.receivers.is_empty()
            || self.receivers.len() > 256
            || !self.ray_offset_m.is_finite()
            || !(1e-6..=0.01).contains(&self.ray_offset_m)
        {
            return Err("Invalid solar interval, receivers, step or ray offset".into());
        }
        let duration = duration as u32;
        let cells = duration.div_ceil(self.step_seconds);
        if cells > 1440 || cells as usize * self.receivers.len() > 32768 {
            return Err("Solar sampling exceeds bounded cells".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        if self
            .receivers
            .iter()
            .any(|p| !valid_receiver(p) || !ids.insert(&p.id))
        {
            return Err(
                "Solar receivers require unique IDs, finite points and unit normals".into(),
            );
        }
        Ok((start, duration, cells))
    }
    pub fn work_units(&self) -> Result<usize, String> {
        Ok(self.interval()?.2 as usize * self.receivers.len())
    }
}
pub fn run(s: &Snapshot, r: &Request) -> Result<ResultSet, String> {
    run_with_control(s, r, &AtomicBool::new(false), &AtomicUsize::new(0))
}
pub fn run_with_control(
    s: &Snapshot,
    r: &Request,
    cancel: &AtomicBool,
    progress: &AtomicUsize,
) -> Result<ResultSet, String> {
    s.validate()?;
    let (start, duration, cells) = r.interval()?;
    if cancel.load(Ordering::Acquire) {
        return Err("Job cancelled".into());
    }
    let triangles = s
        .blockers
        .iter()
        .map(|b| b.triangles.len() as u64)
        .sum::<u64>();
    if cells > 1440
        || cells as usize * r.receivers.len() > 32768
        || (cells as u64) * (r.receivers.len() as u64) * triangles.max(1) > 10_000_000
    {
        return Err("Solar sampling exceeds1440 cells,32768 point-cells or10million ray/triangle budget; choose an explicit smaller scope".into());
    }
    let mut samples = vec![];
    for (a, b, mid) in sample_cells(duration, r.step_seconds) {
        if cancel.load(Ordering::Acquire) {
            return Err("Job cancelled".into());
        }
        let sun = super::position::at_julian(r.location, start + mid / 86400.)?;
        samples.push((a, b, mid, sun));
    }
    let mut points = vec![];
    for receiver in &r.receivers {
        let mut rows = vec![];
        for (a, b, mid, sun) in &samples {
            if cancel.load(Ordering::Acquire) {
                return Err("Job cancelled".into());
            }
            let (state, blocker_id) =
                visibility_controlled(s, receiver, sun.east_north_up, r.ray_offset_m, cancel)?;
            progress.fetch_add(1, Ordering::Release);
            rows.push(Cell {
                start_seconds: *a,
                end_seconds: *b,
                sample_seconds: *mid,
                elevation_deg: sun.elevation_deg,
                state,
                blocker_id,
            });
        }
        let (segments, seconds) = merge_sunlit(&rows);
        points.push(PointResult {
            receiver_id: receiver.id.clone(),
            uncertain_seconds: rows
                .iter()
                .filter(|c| c.state == State::NearSurface)
                .map(|c| c.end_seconds - c.start_seconds)
                .sum(),
            cells: rows,
            sunlit_segments: segments,
            sampled_sunlit_seconds: seconds,
        });
    }
    Ok(ResultSet{schema_version:1,algorithm:ALGORITHM.into(),semantics:SEMANTICS.into(),snapshot_sha256:hash(s)?,input_sha256:hash(&(ALGORITHM,s,r))?,request:r.clone(),points,interval_method:"Half-open midpoint-sampled cells; adjacent sunlit cells merged. Each isolated transition has at most one step temporal discretization error; sub-step obstructions may be missed. Not a convergence certificate or statutory duration.".into()})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn midpoint_merge_has_frozen_step_error_bound_against_analytic_interval() {
        // Independent continuous truth [270,690) has duration420s. No solar solver used.
        let mut previous: Option<(i32, u32)> = None;
        for step in [300, 150, 75, 30] {
            let rows = sample_cells(1000, step)
                .map(|(a, b, t)| Cell {
                    start_seconds: a,
                    end_seconds: b,
                    sample_seconds: t,
                    elevation_deg: 30.,
                    state: if (270. ..690.).contains(&t) {
                        State::Sunlit
                    } else {
                        State::Occluded
                    },
                    blocker_id: None,
                })
                .collect::<Vec<_>>();
            let (segments, total) = merge_sunlit(&rows);
            assert_eq!(segments.len(), 1);
            assert!((total as i32 - 420).unsigned_abs() <= 2 * step);
            if let Some((old, old_step)) = previous {
                assert!((total as i32 - old).unsigned_abs() <= 2 * old_step);
            }
            previous = Some((total as i32, step));
            assert_eq!(rows.last().unwrap().end_seconds, 1000);
        }
    }
    #[test]
    fn actual_shared_concept_courtyard_passes_light_without_bbox_occlusion() {
        use crate::{
            concept_mass::ConceptMassSpec,
            model::{Material, Point, Project, Shape},
        };
        let rect = |v: f64| {
            vec![
                Point::new(-v, -v),
                Point::new(v, -v),
                Point::new(v, v),
                Point::new(-v, v),
            ]
        };
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: rect(2.),
                    holes: vec![rect(1.)],
                    height: 2.,
                    bottom_offset: 0.,
                },
            },
        );
        let s = capture(
            &p,
            &[id],
            Some(North::DesignAssumption {
                clockwise_from_model_y_deg: 0.,
                note: "Synthetic courtyard".into(),
            }),
        )
        .unwrap();
        let mut r = Receiver {
            id: "courtyard".into(),
            point_m: [0., 0., -1.],
            outward_normal: [0., 0., 1.],
        };
        assert_eq!(visibility(&s, &r, [0., 0., 1.], 1e-4).0, State::Sunlit);
        r.point_m = [1.5, 0., -1.];
        assert_eq!(
            visibility(&s, &r, [0., 0., 1.], 1e-4),
            (State::Occluded, Some(id))
        );
        r.point_m = [1.5, 0., 2.];
        assert_eq!(visibility(&s, &r, [0., 0., 1.], 1e-4).0, State::NearSurface);
    }
    #[test]
    fn analytic_visibility_uses_triangles_not_box_and_handles_two_sides_and_north() {
        let s = Snapshot {
            schema_version: 1,
            algorithm: ALGORITHM.into(),
            source_sha256: "a".repeat(64),
            north: North::DesignAssumption {
                clockwise_from_model_y_deg: 0.,
                note: "Analytic fixture".into(),
            },
            blockers: vec![super::super::snapshot::Blocker {
                element_id: 1,
                triangles: vec![[[-2., -2., 2.], [2., -2., 2.], [0., 2., 2.]]],
            }],
            excluded_element_ids: vec![],
        };
        s.validate().unwrap();
        let mut r = Receiver {
            id: "p".into(),
            point_m: [0., 0., 0.],
            outward_normal: [0., 0., 1.],
        };
        assert_eq!(
            visibility(&s, &r, [0., 0., 1.], 1e-4),
            (State::Occluded, Some(1))
        );
        r.point_m = [1.8, 1.8, 0.];
        assert_eq!(visibility(&s, &r, [0., 0., 1.], 1e-4).0, State::Sunlit); // inside AABB, outside triangle
        r.point_m = [0., 0., 4.];
        assert_eq!(visibility(&s, &r, [0., 0., 1.], 1e-4).0, State::Sunlit);
        assert_eq!(
            visibility(&s, &r, [0., 0., -1.], 1e-4).0,
            State::BelowHorizon
        );
        r.outward_normal = [0., 0., -1.];
        assert_eq!(visibility(&s, &r, [0., 0., 1.], 1e-4).0, State::BackFacing);
        let mut reversed = s.clone();
        reversed.blockers[0].triangles[0].swap(1, 2);
        r.point_m = [0., 0., 0.];
        r.outward_normal = [0., 0., 1.];
        assert_eq!(
            visibility(&s, &r, [0., 0., 1.], 1e-4),
            visibility(&reversed, &r, [0., 0., 1.], 1e-4)
        );
        let north = North::DesignAssumption {
            clockwise_from_model_y_deg: 90.,
            note: "Analytic orientation".into(),
        };
        let v = north.model_direction([0., 1., 0.]);
        assert!((v[0] - 1.).abs() < 1e-12 && v[1].abs() < 1e-12);
    }
}
