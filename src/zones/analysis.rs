//! Read-only spatial checks, not a building-code or door-connectivity solver.
use super::ZoneSpec;
use crate::model::{Point, Project, Shape};
use crate::view_settings::DesignOptionScope;
use geo::{Area, BooleanOps, Intersects};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisOptions {
    pub story: Option<u32>,
    #[serde(default)]
    pub design_scope: DesignOptionScope,
    /// Explicit analysis domain on the selected story, excluding supplied holes.
    pub boundary: Option<Vec<Point>>,
    #[serde(default)]
    pub holes: Vec<Vec<Point>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpaceRow {
    pub id: u64,
    pub number: String,
    pub name: String,
    pub purpose: String,
    pub story: u32,
    pub area_m2: f64,
    pub space_volume_m3: f64,
    pub outside_boundary_m2: Option<f64>,
    pub wall_footprint_overlap_m2: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Relation {
    pub a: u64,
    pub b: u64,
    pub overlap_m2: f64,
    pub boundary_distance_m: f64,
    pub relation: &'static str,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Total {
    pub count: usize,
    pub area_m2: f64,
    pub space_volume_m3: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpaceAnalysis {
    pub design_scope: DesignOptionScope,
    pub method: &'static str,
    pub near_distance_m: f64,
    pub area_tolerance_m2: f64,
    pub spaces: Vec<SpaceRow>,
    pub by_story: std::collections::BTreeMap<u32, Total>,
    pub by_purpose: std::collections::BTreeMap<String, Total>,
    pub relations: Vec<Relation>,
    pub relation_count: usize,
    pub relations_truncated: bool,
    pub domain_area_m2: Option<f64>,
    pub assigned_union_m2: Option<f64>,
    pub unassigned_m2: Option<f64>,
    pub limitations: Vec<&'static str>,
}
pub fn analyze(project: &Project, options: &AnalysisOptions) -> Result<SpaceAnalysis, String> {
    if let DesignOptionScope::Option { id } = options.design_scope {
        if id == 0 || project.design_options.name(id).is_none() {
            return Err("Space analysis design option does not exist".into());
        }
    }
    if options.design_scope != (DesignOptionScope::All {})
        && project.elements.iter().any(|e| {
            matches!(e.shape, Shape::Wall { .. })
                && e.design_option.is_some()
                && options.story.is_none_or(|s| s == e.story)
        })
    {
        return Err("Option-scoped wall joins are not supported by this analysis".into());
    }
    if options
        .story
        .is_some_and(|id| !project.stories.iter().any(|s| s.id == id))
    {
        return Err("Space analysis story does not exist".into());
    }
    if (!options.holes.is_empty() && options.boundary.is_none())
        || (options.boundary.is_some() && options.story.is_none())
    {
        return Err("Analysis boundary requires one explicit story".into());
    }
    let domain = options.boundary.as_ref().map(|p| ZoneSpec {
        surface: Default::default(),
        purpose: "analysis".into(),
        outline: p.clone(),
        holes: options.holes.clone(),
        height: 1.,
        bottom_offset: 0.,
        label_position: None,
    });
    if let Some(d) = &domain {
        d.validate()?;
    }
    let elements: Vec<_> = project
        .elements
        .iter()
        .filter(|e| options.design_scope.contains(e.design_option))
        .filter_map(|e| match &e.shape {
            Shape::Zone { spec } if options.story.is_none_or(|s| s == e.story) => Some((e, spec)),
            _ => None,
        })
        .collect();
    if elements.len() > 256 {
        return Err("Space analysis is limited to 256 spaces per request; filter by story".into());
    }
    let mut report = SpaceAnalysis {
        design_scope: options.design_scope,
        method: "explicit_boundary_2d",
        near_distance_m: 0.25,
        area_tolerance_m2: 1e-6,
        spaces: vec![],
        by_story: Default::default(),
        by_purpose: Default::default(),
        relations: vec![],
        relation_count: 0,
        relations_truncated: false,
        domain_area_m2: None,
        assigned_union_m2: None,
        unassigned_m2: None,
        limitations: vec![
            "Near boundaries do not prove a door, circulation or accessibility.",
            "Explicit zones do not follow walls automatically.",
            "Unassigned domain may include walls or voids unless excluded in the supplied boundary.",
            "Area totals are authored-space area, not statutory building area; overlapping spaces double-count in sums.",
        ],
    };
    if options.design_scope == (DesignOptionScope::All {})
        && !project.design_options.options.is_empty()
    {
        report.limitations.push(
            "All alternatives are included together; these totals are not a candidate comparison.",
        );
    }
    let mut walls_by_story = std::collections::BTreeMap::<u32, geo::MultiPolygon<f64>>::new();
    let mut wall_query = crate::wall_solid::WallQuery::new(project);
    for e in project.elements.iter().filter(|e| {
        matches!(e.shape, Shape::Wall { .. })
            && options.story.is_none_or(|s| s == e.story)
            && options.design_scope.contains(e.design_option)
    }) {
        let wall = wall_query.wall(e.id).map_err(|d| d.message)?;
        let union = walls_by_story
            .entry(e.story)
            .or_insert_with(|| geo::MultiPolygon(vec![]));
        *union = union.union(&wall.geometry.gross_footprint);
    }
    report.limitations.push("Wall overlap uses gross 2D footprints (including door thresholds), not 3D collision or clearance certification.");
    let mut polygons = vec![];
    let mut union = geo::MultiPolygon::<f64>(vec![]);
    for (e, spec) in &elements {
        spec.validate()?;
        let polygon = spec.polygon();
        let area = spec.area();
        let volume = area * spec.height;
        report.spaces.push(SpaceRow {
            id: e.id,
            number: format!("Z{:03}", e.id),
            name: e.name.clone(),
            purpose: spec.purpose.clone(),
            story: e.story,
            area_m2: area,
            space_volume_m3: volume,
            wall_footprint_overlap_m2: walls_by_story.get(&e.story).map_or(0., |w| {
                geo::MultiPolygon(vec![polygon.clone()])
                    .intersection(w)
                    .unsigned_area()
            }),
            outside_boundary_m2: domain
                .as_ref()
                .map(|d| polygon.difference(&d.polygon()).unsigned_area()),
        });
        for t in [
            report.by_story.entry(e.story).or_default(),
            report.by_purpose.entry(spec.purpose.clone()).or_default(),
        ] {
            t.count += 1;
            t.area_m2 += area;
            t.space_volume_m3 += volume;
        }
        if domain.is_some() {
            union = union.union(&geo::MultiPolygon(vec![polygon.clone()]));
        }
        polygons.push(polygon);
    }
    for i in 0..elements.len() {
        for j in i + 1..elements.len() {
            if elements[i].0.story != elements[j].0.story {
                continue;
            }
            let overlap = polygons[i].intersection(&polygons[j]).unsigned_area();
            let distance = if polygons[i].intersects(&polygons[j]) {
                0.
            } else {
                boundary_distance(elements[i].1, elements[j].1)
            };
            if overlap > report.area_tolerance_m2 || distance <= report.near_distance_m {
                report.relation_count += 1;
                if report.relations.len() < 512 {
                    report.relations.push(Relation {
                        a: elements[i].0.id,
                        b: elements[j].0.id,
                        overlap_m2: overlap,
                        boundary_distance_m: distance,
                        relation: if overlap > report.area_tolerance_m2 {
                            "overlap"
                        } else {
                            "near_boundary"
                        },
                    });
                }
            }
        }
    }
    report.relations_truncated = report.relation_count > report.relations.len();
    if let Some(domain) = domain {
        let p = geo::MultiPolygon(vec![domain.polygon()]);
        report.domain_area_m2 = Some(domain.area());
        report.assigned_union_m2 = Some(p.intersection(&union).unsigned_area());
        report.unassigned_m2 = Some(p.difference(&union).unsigned_area());
    }
    Ok(report)
}
fn boundary_distance(a: &ZoneSpec, b: &ZoneSpec) -> f64 {
    let rings = |s: &ZoneSpec| {
        std::iter::once(s.outline.clone())
            .chain(s.holes.clone())
            .collect::<Vec<_>>()
    };
    let ar = rings(a);
    let br = rings(b);
    let one = |a: &Vec<Vec<Point>>, b: &Vec<Vec<Point>>| {
        a.iter()
            .flatten()
            .map(|p| {
                b.iter()
                    .map(|r| crate::geometry::polygon_edge_distance(*p, r))
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(f64::INFINITY, f64::min)
    };
    one(&ar, &br).min(one(&br, &ar))
}
