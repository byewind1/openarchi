//! Read-only derived-geometry assessment over the authoritative parameter model.
//! It owns diagnostics, never another live Project. Structural model checks still
//! run first at Project::validate; these checks close the derived-result boundary.
use crate::{
    derived_geometry::{DiagnosticSeverity, GeometryDiagnostic},
    model::{Project, Shape},
};
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GeometryReport {
    /// False for source-only assessment or a rejected input/header.
    pub input_checked: bool,
    pub input_revision: Option<u64>,
    pub document_session: Option<u64>,
    pub inspected_elements: usize,
    pub explicit_empty_material: Vec<u64>,
    pub diagnostics: Vec<GeometryDiagnostic>,
}
/// Preserve legacy input/placement strings while source failures retain context.
pub(crate) fn compatibility_message(d: &GeometryDiagnostic) -> String {
    if matches!(
        d.code.as_str(),
        "INVALID_PROJECT_INPUT"
            | "INVALID_ELEMENT_INPUT"
            | "PLACEMENT_REJECTED"
            | "DUPLICATE_ELEMENT_ID"
            | "INVALID_ELEMENT_IDENTITY"
    ) {
        d.message.clone()
    } else {
        format!(
            "[{}] element={} feature={}: {}",
            d.code, d.element_id, d.feature, d.message
        )
    }
}

/// Only the legacy String boundary uses this adapter. Service callers receive
/// the original diagnostic directly, including its severity and optional location.
pub(crate) fn diagnostic_from_message(message: &str) -> Option<GeometryDiagnostic> {
    let (code, rest) = message.strip_prefix('[')?.split_once("] element=")?;
    let (id, rest) = rest.split_once(" feature=")?;
    let (feature, reason) = rest.split_once(": ")?;
    if code.is_empty()
        || !code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'_' || b.is_ascii_digit())
        || feature.is_empty()
        || !feature
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || id.is_empty()
        || !id.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some(GeometryDiagnostic::error(
        id.parse().ok()?,
        code,
        feature,
        reason,
    ))
}

/// Shared view-input contract: a finite, simple, nonzero-area ring; reversed
/// heights and an empty equal-height intersection are both legitimate.
/// No snapping, topology welding, or minimum modelling dimension is applied.
pub fn validate_clip_context(
    polygon: Option<&[crate::model::Point]>,
    heights: Option<(f64, f64)>,
) -> Result<(), String> {
    use geo::Intersects;
    if heights.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite()) {
        return Err("Clipping heights must be finite".into());
    }
    let Some(mut ring) = polygon else {
        return Ok(());
    };
    if ring.len() > 1 && ring.first() == ring.last() {
        ring = &ring[..ring.len() - 1];
    }
    if ring.len() < 3 || ring.iter().any(|p| !p.finite()) {
        return Err("Clipping region needs at least three finite vertices".into());
    }
    let area = crate::geometry::polygon_area(ring);
    if !area.is_finite() || area == 0. {
        return Err("Clipping region must have a finite nonzero area".into());
    }
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        if a == b {
            return Err("Clipping region contains a zero-length edge".into());
        }
        let c = ring[(i + 2) % ring.len()];
        let u = [b.x - a.x, b.y - a.y];
        let v = [c.x - b.x, c.y - b.y];
        if u[0] * v[1] - u[1] * v[0] == 0. && u[0] * v[0] + u[1] * v[1] < 0. {
            return Err("Clipping region contains overlapping edges".into());
        }
        let line = geo::Line::new((a.x, a.y), (b.x, b.y));
        for j in (i + 2)..ring.len() {
            if i == 0 && j + 1 == ring.len() {
                continue;
            }
            let c = ring[j];
            let d = ring[(j + 1) % ring.len()];
            if line.intersects(&geo::Line::new((c.x, c.y), (d.x, d.y))) {
                return Err("Clipping region crosses or touches itself".into());
            }
        }
    }
    Ok(())
}
impl GeometryReport {
    pub fn first_error(&self) -> Option<&GeometryDiagnostic> {
        self.diagnostics
            .iter()
            .find(|d| d.severity == DiagnosticSeverity::Error)
    }
    pub fn into_result(self) -> Result<(), String> {
        match self.first_error() {
            None => Ok(()),
            Some(d) => Err(compatibility_message(d)),
        }
    }
}
fn note_polyline(
    report: &mut GeometryReport,
    id: u64,
    geometry: &crate::polyline_geometry::PolylineSnapshot,
) {
    let scale = geometry
        .bounds
        .iter()
        .flat_map(|p| p.to_array())
        .map(f64::abs)
        .fold(1., f64::max);
    if geometry.max_endpoint_radius_error_m > 16. * f64::EPSILON * scale {
        let mut d = GeometryDiagnostic::error(
            id,
            "ARC_ENDPOINT_RADIUS_TOLERANCE",
            "curve",
            format!(
                "Stored endpoints differ from declared radius by {} m; endpoints are retained under the existing diameter allowance",
                geometry.max_endpoint_radius_error_m
            ),
        );
        d.severity = DiagnosticSeverity::Warning;
        report.diagnostics.push(d);
    }
}
/// Full original input gate, then source assessment reused by semantic input.
/// The fresh Project::validation_report remains an independent no-cache reference.
pub fn validate_cached(
    project: &Project,
    cache: &mut crate::derivation_cache::DerivationCache,
    revision: u64,
    session: u64,
) -> GeometryReport {
    let mut report = GeometryReport {
        input_revision: Some(revision),
        document_session: Some(session),
        ..Default::default()
    };
    if let Err(d) = project.validate_input_diagnostic() {
        report.diagnostics.push(d);
        return report;
    }
    report.input_checked = true;
    let mut read = match cache.read(project, revision, session) {
        Ok(read) => read,
        Err(d) => {
            report.diagnostics.push(d);
            return report;
        }
    };
    for e in &project.elements {
        report.inspected_elements += 1;
        match read.source(e.id) {
            Ok(handle) => match handle.source.as_ref() {
                crate::derivation_cache::GeometrySource::Wall(g)
                    if g.geometry.empty_by_openings =>
                {
                    report.explicit_empty_material.push(e.id)
                }
                crate::derivation_cache::GeometrySource::Polyline(g) => {
                    note_polyline(&mut report, e.id, &g.geometry)
                }
                _ => {}
            },
            Err(d) => report.diagnostics.push(d),
        }
    }
    report
}
/// Source validity only. Project input/header/permission checks are not replaced.
pub fn inspect(project: &Project) -> GeometryReport {
    let mut report = GeometryReport::default();
    if project.elements.len() > 100_000 {
        report.diagnostics.push(GeometryDiagnostic::error(
            0,
            "INVALID_PROJECT_SIZE",
            "project",
            "Project exceeds 100000 elements",
        ));
        return report;
    }
    let mut walls = crate::wall_solid::WallQuery::new(project);
    for e in &project.elements {
        report.inspected_elements += 1;
        if !project.stories.iter().any(|s| s.id == e.story) {
            report.diagnostics.push(GeometryDiagnostic::error(
                e.id,
                "GEOMETRY_STORY_MISSING",
                "story",
                "Element references a missing story",
            ));
            continue;
        }
        let result = match &e.shape {
            Shape::ConceptMass { .. } => {
                crate::concept_mass::derive(e, project.elevation(e.story)).map(|_| ())
            }
            Shape::Zone { .. } => crate::zones::derive(e, project.elevation(e.story)).map(|_| ()),
            Shape::Wall { .. } => walls.wall(e.id).map(|g| {
                if g.geometry.empty_by_openings {
                    report.explicit_empty_material.push(e.id);
                }
            }),
            Shape::Opening { .. } => walls.opening(e.id).map(|_| ()),
            Shape::Slab { .. } => {
                crate::slab_geometry::derive(e, project.elevation(e.story)).map(|_| ())
            }
            Shape::Roof { .. } => {
                crate::roof_geometry::derive(e, project.elevation(e.story)).map(|_| ())
            }
            Shape::Beam { .. } => crate::beam_geometry::derive(project, e).map(|_| ()),
            Shape::Column { .. } => crate::column_geometry::derive(project, e).map(|_| ()),
            Shape::Stair { .. } => crate::stair_geometry::derive(project, e).map(|_| ()),
            Shape::Polyline { .. } => crate::polyline_geometry::derive(project, e).map(|g| {
                note_polyline(&mut report, e.id, &g.geometry);
            }),
        };
        if let Err(d) = result {
            report.diagnostics.push(d);
        }
    }
    report
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::model::Point;
    #[test]
    fn only_encoded_diagnostics_cross_the_legacy_string_adapter() {
        let d = GeometryDiagnostic::error(
            42,
            "INVALID_POLYLINE_GEOMETRY",
            "curve",
            "radius: nonfinite result",
        );
        assert_eq!(diagnostic_from_message(&compatibility_message(&d)), Some(d));
        for text in [
            "file [INVALID_BEAM] element=3 feature=solid: reason",
            "[invalid] element=1 feature=x: reason",
            "[INVALID_BEAM] element=-1 feature=x: reason",
            "[INVALID_BEAM] element=1 feature=: reason",
        ] {
            assert!(diagnostic_from_message(text).is_none());
        }
    }
    #[test]
    fn clip_contract_rejects_ambiguous_regions_and_preserves_empty_or_reversed_heights() {
        let p = |x, y| Point::new(x, y);
        let valid = vec![
            p(0., 0.),
            p(4., 0.),
            p(4., 4.),
            p(2., 4.),
            p(2., 2.),
            p(0., 2.),
        ];
        for range in [None, Some((2., 1.)), Some((1., 1.))] {
            validate_clip_context(Some(&valid), range).unwrap();
        }
        for bad in [
            vec![p(0., 0.), p(4., 4.), p(0., 3.), p(4., 0.)], // crossing, nonzero signed area
            vec![p(0., 0.), p(2., 0.), p(2., 0.), p(0., 2.)],
            vec![p(0., 0.), p(4., 0.), p(2., 0.), p(2., 4.), p(0., 4.)],
            vec![p(0., 0.), p(2., 0.), p(f64::NAN, 2.)],
            vec![p(0., 0.), p(1., 0.), p(2., 0.)],
        ] {
            assert!(validate_clip_context(Some(&bad), None).is_err(), "{bad:?}");
        }
        assert!(validate_clip_context(Some(&valid), Some((f64::INFINITY, 1.))).is_err());
        let mut collinear = vec![p(0., 0.), p(1., 0.), p(2., 0.), p(2., 2.), p(0., 2.)];
        validate_clip_context(Some(&collinear), None).unwrap();
        collinear.push(collinear[0]);
        validate_clip_context(Some(&collinear), None).unwrap();
    }
}
