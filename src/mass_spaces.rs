//! Explicit concept-envelope/space relations. Live geometry remains in Project.elements.
use crate::{
    command::{ElementEdit, ProjectDelta, ProjectHeader},
    model::{Element, Project, Shape},
    zones::ZoneSpec,
};
use geo::{Area, BooleanOps, MapCoords};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Derivation {
    Footprint { height: f64 },
    Existing,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub mass: u64,
    pub zone: u64,
    pub story: u32,
    pub option: Option<u32>,
    pub source_sha256: String,
    pub zone_sha256: String,
    pub derivation: Derivation,
    pub assumption: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Create {
        mass: u64,
        expected_source_sha256: String,
        name: String,
        purpose: String,
        height: f64,
        assumption: String,
    },
    LinkExisting {
        mass: u64,
        zone: u64,
        expected_source_sha256: String,
        expected_zone_sha256: String,
        assumption: String,
    },
    Refresh {
        zone: u64,
        expected_sha256: String,
        #[serde(default)]
        height: Option<f64>,
    },
    Detach {
        zone: u64,
        expected_sha256: String,
    },
}
fn text(s: &str, n: usize) -> bool {
    !s.trim().is_empty() && s.len() <= n && !s.chars().any(char::is_control)
}
pub fn expected(link: &Link) -> String {
    crate::automation_journal::canonical_hash(link).expect("valid link serializes")
}
pub fn element_stamp(p: &Project, e: &Element) -> Result<String, String> {
    // A common story elevation change is a source-frame change, not an edit
    // to the Zone's authored relative geometry. Its own story/option still count.
    if matches!(e.shape, Shape::Zone { .. }) {
        return crate::automation_journal::canonical_hash(e);
    }

    let elevation = p
        .stories
        .iter()
        .find(|s| s.id == e.story)
        .ok_or("Linked object story is missing")?
        .elevation;
    crate::automation_journal::canonical_hash(&(e, elevation))
}
pub fn validate(links: &[Link]) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    if links.len() > 128 {
        return Err("At most128 mass-space links are supported".into());
    }
    for l in links {
        if l.mass == 0
            || l.zone == 0
            || l.mass == l.zone
            || !ids.insert(l.zone)
            || !text(&l.assumption, 512)
            || !crate::storage::valid_document_identity(&l.source_sha256)
            || !crate::storage::valid_document_identity(&l.zone_sha256)
            || matches!(l.derivation,Derivation::Footprint{height} if !height.is_finite()||!(0.01..=100.).contains(&height))
        {
            return Err("Invalid mass-space link".into());
        }
    }
    Ok(())
}
fn object(p: &Project, id: u64) -> Result<&Element, String> {
    p.elements
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| "Linked source or space is missing".into())
}
fn writable(p: &Project, e: &Element) -> Result<(), String> {
    if p.layers.is_locked(e.layer.0)
        || p.edit_state.locked.contains(&e.id)
        || p.drawings.reference_owner(e.id).is_some()
    {
        return Err("Linked object is locked or externally owned".into());
    }
    Ok(())
}
fn mass(p: &Project, id: u64) -> Result<(&Element, &crate::concept_mass::ConceptMassSpec), String> {
    let e = object(p, id)?;
    if e.worksheet.is_some() {
        return Err("Mass-space links require floor-model objects".into());
    }
    let Shape::ConceptMass { spec } = &e.shape else {
        return Err("Source must be an authored concept mass".into());
    };
    spec.validate()?;
    Ok((e, spec))
}
fn same_scope(m: &Element, z: &Element) -> bool {
    m.story == z.story
        && m.design_option == z.design_option
        && m.worksheet.is_none()
        && z.worksheet.is_none()
}
fn check_stamp(actual: String, expected: &str) -> Result<(), String> {
    if actual != expected {
        Err("Linked object changed; inspect the original proposal".into())
    } else {
        Ok(())
    }
}
fn fits(m: &crate::concept_mass::ConceptMassSpec, z: &ZoneSpec) -> Result<(), String> {
    z.validate()?;
    let policy = crate::precision::TolerancePolicy::default();
    let eps = policy.length_m.absolute;
    if z.bottom_offset < m.bottom_offset - eps
        || z.bottom_offset + z.height > m.bottom_offset + m.height + eps
    {
        return Err("Space height or elevation exceeds its source envelope".into());
    }
    let origin = m.outline[0];
    let local = |p: geo::Coord<f64>| geo::Coord {
        x: p.x - origin.x,
        y: p.y - origin.y,
    };
    let boundary = ZoneSpec {
        surface: Default::default(),
        purpose: "concept envelope".into(),
        outline: m.paths().remove(0),
        holes: m.paths().into_iter().skip(1).collect(),
        height: m.height,
        bottom_offset: m.bottom_offset,
        label_position: None,
    };
    let outside = z
        .polygon()
        .map_coords(local)
        .difference(&boundary.polygon().map_coords(local))
        .unsigned_area();
    if !policy.area_m2.accepts(outside, 0.) {
        return Err("Space footprint exceeds its source envelope or enters a courtyard".into());
    }
    Ok(())
}
fn derived(
    m: &crate::concept_mass::ConceptMassSpec,
    height: f64,
    purpose: String,
) -> Result<ZoneSpec, String> {
    if m.arcs
        .iter()
        .chain(m.hole_arcs.iter().flatten())
        .any(Option::is_some)
    {
        return Err("Linked spaces from curved masses are not supported yet".into());
    }
    let z = ZoneSpec {
        surface: Default::default(),
        purpose,
        outline: m.paths().remove(0),
        holes: m.paths().into_iter().skip(1).collect(),
        height,
        bottom_offset: m.bottom_offset,
        label_position: None,
    };
    fits(m, &z)?;
    Ok(z)
}
pub fn status(p: &Project, l: &Link) -> &'static str {
    let Ok((m, _)) = mass(p, l.mass) else {
        return "Mass-space source is unavailable";
    };
    let Ok(z) = object(p, l.zone) else {
        return "Linked concept space is missing";
    };
    if !matches!(z.shape, Shape::Zone { .. }) {
        return "Linked concept space is missing";
    }
    if !same_scope(m, z) || m.story != l.story || m.design_option != l.option {
        return "Mass-space scenario changed";
    }
    if element_stamp(p, z).ok().as_deref() != Some(l.zone_sha256.as_str()) {
        return "Linked space has user edits";
    }
    if element_stamp(p, m).ok().as_deref() != Some(l.source_sha256.as_str()) {
        return "Mass-space source changed";
    }
    "Mass-space link is current"
}
pub(crate) fn apply(p: &mut Project, edit: &Edit) -> Result<ProjectDelta, String> {
    let mut links = p.design_process.mass_spaces.clone();
    let (story, option) = match edit {
        Edit::Create { mass, .. } | Edit::LinkExisting { mass, .. } => {
            let source = object(p, *mass)?;
            (source.story, source.design_option)
        }
        Edit::Refresh { zone, .. } | Edit::Detach { zone, .. } => {
            let link = links
                .iter()
                .find(|link| link.zone == *zone)
                .ok_or("Mass-space link is missing")?;
            (link.story, link.option)
        }
    };
    crate::design_process::service::guard_mass_space_scope(p, story, option)?;
    let mut change = None;
    let mut append = None;
    match edit {
        Edit::Create {
            mass: id,
            expected_source_sha256,
            name,
            purpose,
            height,
            assumption,
        } => {
            if links.len() >= 128
                || !text(name, 160)
                || !text(purpose, 256)
                || !text(assumption, 512)
            {
                return Err(
                    "Give the concept space a bounded name, purpose and design assumption".into(),
                );
            }
            let (m, spec) = mass(p, *id)?;
            writable(p, m)?;
            check_stamp(element_stamp(p, m)?, expected_source_sha256)?;
            if p.elements.iter().any(|e| e.id == p.next_id) {
                return Err("Element identity is already occupied".into());
            }
            let mut z = m.clone();
            z.id = p.next_id;
            z.id.checked_add(1).ok_or("Element identities exhausted")?;
            z.name = name.clone();
            z.shape = Shape::Zone {
                spec: derived(spec, *height, purpose.clone())?,
            };
            z.wall_settings = None;
            z.wall_contour = None;
            z.slab_contour = None;
            z.opening_joinery = None;
            links.push(Link {
                mass: *id,
                zone: z.id,
                story: m.story,
                option: m.design_option,
                source_sha256: element_stamp(p, m)?,
                zone_sha256: element_stamp(p, &z)?,
                derivation: Derivation::Footprint { height: *height },
                assumption: assumption.clone(),
            });
            append = Some(z);
        }
        Edit::LinkExisting {
            mass: id,
            zone,
            expected_source_sha256,
            expected_zone_sha256,
            assumption,
        } => {
            if links.len() >= 128 || links.iter().any(|l| l.zone == *zone) || !text(assumption, 512)
            {
                return Err("Space already linked or link assumptions are invalid".into());
            }
            let (m, spec) = mass(p, *id)?;
            let z = object(p, *zone)?;
            writable(p, m)?;
            writable(p, z)?;
            if !same_scope(m, z) {
                return Err("Mass and space must share one story and design option".into());
            }
            let Shape::Zone { spec: zs } = &z.shape else {
                return Err("Target must be an authored Zone".into());
            };
            fits(spec, zs)?;
            check_stamp(element_stamp(p, m)?, expected_source_sha256)?;
            check_stamp(element_stamp(p, z)?, expected_zone_sha256)?;
            links.push(Link {
                mass: *id,
                zone: *zone,
                story: m.story,
                option: m.design_option,
                source_sha256: element_stamp(p, m)?,
                zone_sha256: element_stamp(p, z)?,
                derivation: Derivation::Existing,
                assumption: assumption.clone(),
            });
        }
        Edit::Refresh {
            zone,
            expected_sha256,
            height,
        } => {
            let l = links
                .iter_mut()
                .find(|l| l.zone == *zone)
                .ok_or("Mass-space link is missing")?;
            check_stamp(expected(l), expected_sha256)?;
            let Derivation::Footprint { height: old_height } = l.derivation else {
                return Err(
                    "Existing space binding does not authorize geometry regeneration".into(),
                );
            };
            let (m, spec) = mass(p, l.mass)?;
            let z = object(p, l.zone)?;
            writable(p, m)?;
            writable(p, z)?;
            if !same_scope(m, z) || m.story != l.story || m.design_option != l.option {
                return Err("Mass-space scenario changed".into());
            }
            check_stamp(element_stamp(p, z)?, &l.zone_sha256)
                .map_err(|_| "Linked space has user edits; regeneration refused".to_string())?;
            let Shape::Zone { spec: zs } = &z.shape else {
                return Err("Target must be an authored Zone".into());
            };
            let h = height.unwrap_or(old_height);
            let mut next = z.clone();
            let mut refreshed = derived(spec, h, zs.purpose.clone())?;
            refreshed.surface = zs.surface.clone();
            next.shape = Shape::Zone { spec: refreshed };
            l.source_sha256 = element_stamp(p, m)?;
            l.zone_sha256 = element_stamp(p, &next)?;
            l.derivation = Derivation::Footprint { height: h };
            change = Some(next);
        }
        Edit::Detach {
            zone,
            expected_sha256,
        } => {
            let i = links
                .iter()
                .position(|l| l.zone == *zone)
                .ok_or("Mass-space link is missing")?;
            check_stamp(expected(&links[i]), expected_sha256)?;
            if let Ok(z) = object(p, *zone) {
                writable(p, z)?;
            }
            links.remove(i);
        }
    }
    validate(&links)?;
    let mut journal = p.design_process.clone();
    journal.mass_spaces = links;
    journal.validate()?;
    let before = ProjectHeader::capture(p);
    let mut delta = ProjectDelta::default();
    if let Some(z) = append {
        let index = p.elements.len();
        p.next_id = z.id + 1;
        delta.elements.push(ElementEdit {
            id: z.id,
            before_index: None,
            before: None,
            after_index: Some(index),
            after: Some(z.clone()),
        });
        p.elements.push(z);
    }
    if let Some(z) = change {
        let i = p
            .elements
            .iter()
            .position(|e| e.id == z.id)
            .expect("validated space");
        delta.elements.push(ElementEdit {
            id: z.id,
            before_index: Some(i),
            before: Some(p.elements[i].clone()),
            after_index: Some(i),
            after: Some(z.clone()),
        });
        p.elements[i] = z;
    }
    p.design_process = journal;
    delta.before_header = Some(Box::new(before));
    delta.after_header = Some(Box::new(ProjectHeader::capture(p)));
    Ok(delta)
}
fn info(p: &Project, id: u64, is_mass: bool) -> Result<serde_json::Value, String> {
    let e = object(p, id)?;
    if (is_mass && !matches!(e.shape, Shape::ConceptMass { .. }))
        || (!is_mass && !matches!(e.shape, Shape::Zone { .. }))
    {
        return Err("Requested mass-space object has another type".into());
    }
    let (height, bottom) = match &e.shape {
        Shape::ConceptMass { spec } => (spec.height, spec.bottom_offset),
        Shape::Zone { spec } => (spec.height, spec.bottom_offset),
        _ => unreachable!(),
    };
    Ok(
        serde_json::json!({"id":id,"name":e.name,"story":e.story,"option":e.design_option,"expected_sha256":element_stamp(p,e)?,"locked":writable(p,e).is_err(),"height_m":height,"bottom_offset_m":bottom,"authored_area_m2":e.area()}),
    )
}
pub fn query(
    p: &Project,
    mass_filter: Option<u64>,
    zone_filter: Option<u64>,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, String> {
    let links = p
        .design_process
        .mass_spaces
        .iter()
        .filter(|l| {
            mass_filter.is_none_or(|m| l.mass == m) && zone_filter.is_none_or(|z| l.zone == z)
        })
        .collect::<Vec<_>>();
    if !(1..=4).contains(&limit) || offset > links.len() {
        return Err("Invalid mass-space page".into());
    }
    let source = mass_filter.map(|id| info(p, id, true)).transpose()?;
    let space = zone_filter.map(|id| info(p, id, false)).transpose()?;
    let end = (offset + limit).min(links.len());
    Ok(
        serde_json::json!({"source":source,"space":space,"links":links[offset..end].iter().map(|l|serde_json::json!({"link":l,"expected_sha256":expected(l),"status":status(p,l),"source_current_sha256":object(p,l.mass).ok().and_then(|e|element_stamp(p,e).ok()),"space_current_sha256":object(p,l.zone).ok().and_then(|e|element_stamp(p,e).ok()),"authored_space_area_m2":object(p,l.zone).ok().map(Element::area)})).collect::<Vec<_>>(),"total":links.len(),"next_offset":(end<links.len()).then_some(end),"semantics":"single-story concept association; original outlines/holes and editable live Zone; assumptions are not measurements or approved use; no net/statutory floor-area verdict"}),
    )
}
