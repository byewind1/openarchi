//! AC29-style parameter transfer: pick up (eyedropper) and inject (syringe)
//! parameters between elements of the same type.
//!
//! The transfer scope follows the active named transfer set stored with the
//! project. Position, identity, story, renovation status and design-option
//! fields never transfer. The behavior contract lives in
//! `docs/features/parameter-transfer.md`.

use crate::model::{Element, OpeningDefaults, OpeningKind, Shape};
use crate::wall::WallSettings;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Element categories between which parameters may be transferred. Doors and
/// windows are separate types, matching Archicad's distinct tool families.
/// `Polyline` stands for the 2D drafting family (line, arc, polyline, spline).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransferType {
    Wall,
    Slab,
    Stair,
    Polyline,
    Door,
    Window,
}

impl TransferType {
    /// Localization key for the type name.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Slab => "Slab",
            Self::Stair => "Stair",
            Self::Polyline => "Polyline",
            Self::Door => "Door",
            Self::Window => "Window",
        }
    }

    pub fn of(element: &Element) -> Option<Self> {
        match &element.shape {
            Shape::Wall { .. } => Some(Self::Wall),
            Shape::Slab { .. } => Some(Self::Slab),
            Shape::Stair { .. } => Some(Self::Stair),
            Shape::Polyline { .. } => Some(Self::Polyline),
            Shape::Opening { kind, .. } => Some(match kind {
                OpeningKind::Door => Self::Door,
                OpeningKind::Window => Self::Window,
            }),
        }
    }
}

/// Transferable parameter groups. Each group is all-or-nothing per injection;
/// fields outside every group (identity, position, story, renovation,
/// design option) never transfer.
pub const GROUP_GEOMETRY: &str = "geometry";
pub const GROUP_MATERIAL_LAYER: &str = "material_layer";
pub const GROUP_SURFACES: &str = "surfaces";
pub const GROUP_FLOOR_PLAN_DISPLAY: &str = "floor_plan_display";
pub const GROUP_DATA: &str = "data";

pub const ALL_GROUPS: [&str; 5] = [
    GROUP_GEOMETRY,
    GROUP_MATERIAL_LAYER,
    GROUP_SURFACES,
    GROUP_FLOOR_PLAN_DISPLAY,
    GROUP_DATA,
];

/// Groups that exist for each transfer type; the Element Transfer Settings
/// dialog offers exactly these checkboxes per type.
pub fn groups_for(t: TransferType) -> &'static [&'static str] {
    match t {
        TransferType::Wall => &ALL_GROUPS,
        TransferType::Polyline => &[GROUP_MATERIAL_LAYER],
        TransferType::Slab | TransferType::Stair | TransferType::Door | TransferType::Window => {
            &[GROUP_GEOMETRY, GROUP_MATERIAL_LAYER]
        }
    }
}

/// Localization keys for the group names, in dialog order.
pub fn group_label_key(group: &str) -> &'static str {
    match group {
        GROUP_GEOMETRY => "Geometry",
        GROUP_MATERIAL_LAYER => "Material & Layer",
        GROUP_SURFACES => "Surfaces",
        GROUP_FLOOR_PLAN_DISPLAY => "Floor Plan Display",
        GROUP_DATA => "Data",
        _ => "Geometry",
    }
}

/// A named set of transferable groups. Stored with the project, like the
/// AC29 Element Transfer Settings dialog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransferSet {
    pub name: String,
    pub groups: BTreeSet<String>,
}

impl TransferSet {
    pub fn all_parameters() -> Self {
        Self {
            name: "All Parameters".into(),
            groups: ALL_GROUPS.iter().map(|group| (*group).into()).collect(),
        }
    }

    pub fn allows(&self, group: &str) -> bool {
        self.groups.contains(group)
    }
}

/// Named transfer sets plus the index of the set used by the next transfer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransferSettings {
    pub sets: Vec<TransferSet>,
    pub active: usize,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            sets: vec![TransferSet::all_parameters()],
            active: 0,
        }
    }
}

impl TransferSettings {
    pub fn active_set(&self) -> &TransferSet {
        let index = self.active.min(self.sets.len().saturating_sub(1));
        &self.sets[index]
    }

    pub fn allows(&self, group: &str) -> bool {
        self.active_set().allows(group)
    }

    /// Select the set with this name for the next transfer; returns whether a
    /// set matched.
    pub fn activate(&mut self, name: &str) -> bool {
        for (index, set) in self.sets.iter().enumerate() {
            if set.name == name {
                self.active = index;
                return true;
            }
        }
        false
    }
}

/// Parameters picked up by the eyedropper, remembered per transfer type until
/// replaced. Session state: not persisted and not part of undo history.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PickUpBuffer {
    pub entries: BTreeMap<TransferType, Element>,
}

impl PickUpBuffer {
    pub fn store(&mut self, element: &Element) -> Option<TransferType> {
        let t = TransferType::of(element)?;
        self.entries.insert(t, element.clone());
        Some(t)
    }

    pub fn get(&self, t: TransferType) -> Option<&Element> {
        self.entries.get(&t)
    }
}

/// Apply the active transfer set from `source` onto `target`. Both elements
/// must be the same [`TransferType`]. Returns the group keys that were
/// applied, so callers can report the scope of a no-op versus a real change.
pub fn apply_transfer(
    source: &Element,
    target: &mut Element,
    set: &TransferSettings,
) -> Result<Vec<&'static str>, String> {
    let Some(source_type) = TransferType::of(source) else {
        return Err("Unsupported element type".into());
    };
    let Some(target_type) = TransferType::of(target) else {
        return Err("Unsupported element type".into());
    };
    if source_type != target_type {
        return Err("Parameters can only be transferred between elements of the same type".into());
    }
    let mut applied = Vec::new();
    if set.allows(GROUP_MATERIAL_LAYER) {
        target.material = source.material;
        target.layer = source.layer;
        applied.push(GROUP_MATERIAL_LAYER);
    }
    if set.allows(GROUP_GEOMETRY) {
        match (&mut target.shape, &source.shape) {
            (
                Shape::Wall {
                    thickness, height, ..
                },
                Shape::Wall {
                    thickness: st,
                    height: sh,
                    ..
                },
            ) => {
                *thickness = *st;
                *height = *sh;
            }
            (Shape::Slab { thickness, .. }, Shape::Slab { thickness: st, .. }) => {
                *thickness = *st;
            }
            (Shape::Stair { spec }, Shape::Stair { spec: source_spec }) => {
                let start = spec.start;
                let end = spec.end;
                *spec = source_spec.clone();
                spec.start = start;
                spec.end = end;
            }
            (
                Shape::Opening {
                    width,
                    height,
                    sill,
                    ..
                },
                Shape::Opening {
                    width: sw,
                    height: sh,
                    sill: ss,
                    ..
                },
            ) => {
                *width = *sw;
                *height = *sh;
                *sill = *ss;
            }
            // Same transfer type implies the same shape variant.
            _ => {}
        }
        if target_type == TransferType::Wall {
            let fields = |target: &mut WallSettings, source: &WallSettings| {
                target.geometry.reference_line = source.geometry.reference_line;
                target.geometry.reference_offset = source.geometry.reference_offset;
                target.geometry.top_offset = source.geometry.top_offset;
                target.geometry.bottom_offset = source.geometry.bottom_offset;
                target.geometry.to_project_zero = source.geometry.to_project_zero;
                target.profile = source.profile.clone();
            };
            match (&mut target.wall_settings, &source.wall_settings) {
                (Some(settings), Some(source_settings)) => fields(settings, source_settings),
                (settings @ None, Some(source_settings)) => {
                    let mut defaults = WallSettings::default();
                    fields(&mut defaults, source_settings);
                    *settings = Some(defaults);
                }
                (Some(_), None) | (None, None) => {}
            }
        }
        applied.push(GROUP_GEOMETRY);
    }
    if target_type == TransferType::Wall
        && let Some(source_settings) = &source.wall_settings
    {
        let Some(settings) = target.wall_settings.as_mut() else {
            return Ok(applied);
        };
        if set.allows(GROUP_SURFACES) {
            settings.model = source_settings.model.clone();
            applied.push(GROUP_SURFACES);
        }
        if set.allows(GROUP_FLOOR_PLAN_DISPLAY) {
            settings.floor_plan = source_settings.floor_plan.clone();
            applied.push(GROUP_FLOOR_PLAN_DISPLAY);
        }
        if set.allows(GROUP_DATA) {
            settings.structural = source_settings.structural.clone();
            settings.classification = source_settings.classification.clone();
            applied.push(GROUP_DATA);
        }
    }
    Ok(applied)
}

/// Map a picked-up element onto the tool creation defaults held by the
/// editor. Mirrors AC29: the picked parameters become the default for that
/// element type. Fields that never transfer stay untouched.
pub fn apply_pick_up_defaults(source: &Element, set: &TransferSettings) -> PickedDefaults {
    let mut picked = PickedDefaults::default();
    if set.allows(GROUP_MATERIAL_LAYER) {
        picked.material = Some(source.material);
        picked.layer = Some(source.layer);
    }
    if set.allows(GROUP_GEOMETRY) {
        match &source.shape {
            Shape::Wall {
                thickness, height, ..
            } => {
                picked.thickness = Some(*thickness);
                picked.height = Some(*height);
                if let Some(settings) = &source.wall_settings {
                    let mut settings = settings.clone();
                    // The geometry method is a drawing input method, not an
                    // element parameter, and stays with the active tool.
                    settings.geometry.geometry_method = crate::wall::WallGeometryMethod::Single;
                    picked.wall_settings = Some(settings);
                }
            }
            Shape::Slab { thickness, .. } => picked.thickness = Some(*thickness),
            Shape::Stair { spec } => {
                let mut spec = spec.clone();
                // The stair run itself is position, not a transferable default.
                spec.start = crate::stairs::StairSpec::default().start;
                spec.end = crate::stairs::StairSpec::default().end;
                picked.stair_spec = Some(spec);
            }
            Shape::Opening {
                kind,
                width,
                height,
                sill,
                ..
            } => {
                picked.opening_defaults = Some((
                    *kind,
                    OpeningDefaults {
                        width: *width,
                        height: *height,
                        sill: *sill,
                    },
                ));
            }
            Shape::Polyline { .. } => {}
        }
    }
    picked
}

/// Tool creation defaults produced by a pick-up; the editor applies the
/// present fields to its shared tool state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PickedDefaults {
    pub thickness: Option<f32>,
    pub height: Option<f32>,
    pub material: Option<crate::model::Material>,
    pub layer: Option<crate::layers::ElementLayer>,
    pub wall_settings: Option<WallSettings>,
    pub stair_spec: Option<crate::stairs::StairSpec>,
    pub opening_defaults: Option<(OpeningKind, OpeningDefaults)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Material, Point, Project};

    fn wall(start: (f32, f32), end: (f32, f32), thickness: f32) -> Element {
        let mut project = Project::default();
        let id = project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(start.0, start.1),
                end: Point::new(end.0, end.1),
                thickness,
                height: 3.,
            },
        );
        project.elements.into_iter().find(|e| e.id == id).unwrap()
    }

    fn slab(thickness: f32) -> Element {
        let mut project = Project::default();
        let id = project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(2., 2.),
                thickness,
            },
        );
        project.elements.into_iter().find(|e| e.id == id).unwrap()
    }

    #[test]
    fn same_type_transfer_copies_geometry_and_material_but_not_position() {
        let source = wall((0., 0.), (4., 0.), 0.4);
        let mut target = wall((1., 1.), (2., 1.), 0.2);
        let (target_id, target_name) = (target.id, target.name.clone());
        let applied = apply_transfer(&source, &mut target, &TransferSettings::default()).unwrap();
        assert!(applied.contains(&GROUP_GEOMETRY));
        assert!(applied.contains(&GROUP_MATERIAL_LAYER));
        match &target.shape {
            Shape::Wall {
                start,
                end,
                thickness,
                height,
            } => {
                assert_eq!((start.x, start.y), (1., 1.));
                assert_eq!((end.x, end.y), (2., 1.));
                assert_eq!(*thickness, 0.4);
                assert_eq!(*height, 3.);
            }
            _ => panic!("wall shape changed"),
        }
        assert_eq!(target.material, Material::Plaster);
        // Identity never transfers.
        assert_eq!(target.id, target_id);
        assert_eq!(target.name, target_name);
    }

    #[test]
    fn cross_type_transfer_is_rejected() {
        let source = wall((0., 0.), (4., 0.), 0.4);
        let mut target = slab(0.2);
        let error = apply_transfer(&source, &mut target, &TransferSettings::default()).unwrap_err();
        assert_eq!(
            error,
            "Parameters can only be transferred between elements of the same type"
        );
        assert_eq!(target.thickness(), 0.2);
    }

    #[test]
    fn unchecking_a_group_leaves_its_fields_unchanged() {
        let source = wall((0., 0.), (4., 0.), 0.4);
        let mut target = wall((1., 1.), (2., 1.), 0.2);
        let mut settings = TransferSettings::default();
        settings.sets[0].groups.remove(GROUP_GEOMETRY);
        apply_transfer(&source, &mut target, &settings).unwrap();
        assert_eq!(target.thickness(), 0.2);
        assert_eq!(target.material, source.material);
    }

    #[test]
    fn opening_transfer_keeps_host_offset_and_kind() {
        let mut project = Project::default();
        let host = project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(6., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let mut door = None;
        let mut window = None;
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let id = project.add(
                0,
                Material::Timber,
                Shape::Opening {
                    host_wall: host,
                    kind,
                    width: 1.0,
                    height: 2.0,
                    sill: 0.5,
                    offset: 3.0,
                },
            );
            if kind == OpeningKind::Door {
                door = Some(id);
            } else {
                window = Some(id);
            }
        }
        let elements: Vec<Element> = project.elements.clone();
        let source = elements.iter().find(|e| e.id == door.unwrap()).unwrap();
        let error = apply_transfer(
            source,
            &mut elements
                .iter()
                .find(|e| e.id == window.unwrap())
                .unwrap()
                .clone(),
            &TransferSettings::default(),
        )
        .unwrap_err();
        assert!(error.contains("same type"));
        let other_door = elements
            .iter()
            .find(|e| e.id == door.unwrap())
            .unwrap()
            .clone();
        // Renaming the kind of the target is out of scope; inject door→door.
        let mut project2 = Project::default();
        let host2 = project2.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(6., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let target_id = project2.add(
            0,
            Material::Concrete,
            Shape::Opening {
                host_wall: host2,
                kind: OpeningKind::Door,
                width: 0.8,
                height: 1.9,
                sill: 0.0,
                offset: 2.0,
            },
        );
        let mut target = project2
            .elements
            .into_iter()
            .find(|e| e.id == target_id)
            .unwrap();
        apply_transfer(&other_door, &mut target, &TransferSettings::default()).unwrap();
        match target.shape {
            Shape::Opening {
                host_wall,
                kind,
                width,
                height,
                sill,
                offset,
            } => {
                assert_eq!(kind, OpeningKind::Door);
                assert_eq!(host_wall, host2);
                assert_eq!((width, height, sill), (1.0, 2.0, 0.5));
                assert_eq!(offset, 2.0);
            }
            _ => panic!("opening shape changed"),
        }
        assert_eq!(target.material, Material::Timber);
    }

    #[test]
    fn stair_transfer_keeps_its_run() {
        let mut project = Project::default();
        let source_id = project.add(
            0,
            Material::Concrete,
            Shape::Stair {
                spec: crate::stairs::StairSpec {
                    start: Point::new(0., 0.),
                    end: Point::new(4.8, 0.),
                    width: 1.3,
                    total_rise: 3.0,
                    risers: 17,
                    ..Default::default()
                },
            },
        );
        let target_id = project.add(
            0,
            Material::Timber,
            Shape::Stair {
                spec: crate::stairs::StairSpec {
                    start: Point::new(10., 10.),
                    end: Point::new(12., 10.),
                    ..Default::default()
                },
            },
        );
        let elements: Vec<Element> = project.elements.clone();
        let source = elements.iter().find(|e| e.id == source_id).unwrap();
        let mut target = elements.iter().find(|e| e.id == target_id).unwrap().clone();
        apply_transfer(source, &mut target, &TransferSettings::default()).unwrap();
        match &target.shape {
            Shape::Stair { spec } => {
                assert_eq!((spec.start.x, spec.start.y), (10., 10.));
                assert_eq!((spec.end.x, spec.end.y), (12., 10.));
                assert_eq!(spec.width, 1.3);
                assert_eq!(spec.total_rise, 3.0);
                assert_eq!(spec.risers, 17);
            }
            _ => panic!("stair shape changed"),
        }
        assert_eq!(target.material, Material::Concrete);
    }

    #[test]
    fn transfer_settings_activate_switches_active_set() {
        let mut settings = TransferSettings::default();
        settings.sets.push(TransferSet {
            name: "Geometry only".into(),
            groups: [GROUP_GEOMETRY.to_string()].into_iter().collect(),
        });
        assert!(settings.activate("Geometry only"));
        assert_eq!(settings.active_set().name, "Geometry only");
        assert!(!settings.allows(GROUP_MATERIAL_LAYER));
        assert!(!settings.activate("Missing"));
        assert_eq!(settings.active_set().name, "Geometry only");
    }

    #[test]
    fn pick_up_buffer_remembers_latest_per_type() {
        let mut buffer = PickUpBuffer::default();
        let first = wall((0., 0.), (1., 0.), 0.2);
        let second = wall((0., 0.), (2., 0.), 0.3);
        let panel = slab(0.25);
        assert_eq!(buffer.store(&first), Some(TransferType::Wall));
        assert_eq!(buffer.store(&panel), Some(TransferType::Slab));
        assert_eq!(buffer.store(&second), Some(TransferType::Wall));
        assert_eq!(buffer.get(TransferType::Wall).unwrap().thickness(), 0.3);
        assert_eq!(buffer.get(TransferType::Slab).unwrap().thickness(), 0.25);
        assert!(buffer.get(TransferType::Door).is_none());
    }

    #[test]
    fn pick_up_defaults_skip_drawing_method() {
        let mut source = wall((0., 0.), (4., 0.), 0.4);
        if let Some(settings) = source.wall_settings.as_mut() {
            settings.geometry.geometry_method = crate::wall::WallGeometryMethod::Polygonal;
            settings.geometry.reference_line = crate::wall::WallReferenceLine::Inside;
            settings.geometry.reference_offset = 0.1;
        }
        let picked = apply_pick_up_defaults(&source, &TransferSettings::default());
        assert_eq!(picked.thickness, Some(0.4));
        let settings = picked.wall_settings.unwrap();
        assert_eq!(
            settings.geometry.geometry_method,
            crate::wall::WallGeometryMethod::Single
        );
        assert_eq!(
            settings.geometry.reference_line,
            crate::wall::WallReferenceLine::Inside
        );
        assert_eq!(settings.geometry.reference_offset, 0.1);
    }

    #[test]
    fn wall_choice_values_transfer_with_their_group() {
        let mut source = wall((0., 0.), (4., 0.), 0.4);
        if let Some(settings) = source.wall_settings.as_mut() {
            settings.floor_plan.uncut_lines = crate::wall::WallUncutLines::Dashed;
            settings.model.override_surfaces = true;
            settings.structural.structural_function = crate::wall::StructuralFunction::Bearing;
        }
        let target = wall((1., 1.), (2., 1.), 0.2);
        for group in [GROUP_FLOOR_PLAN_DISPLAY, GROUP_SURFACES, GROUP_DATA] {
            let mut settings = TransferSettings::default();
            settings.sets[0].groups = [group.to_string()].into_iter().collect();
            let mut target = target.clone();
            apply_transfer(&source, &mut target, &settings).unwrap();
            let transferred = match group {
                GROUP_FLOOR_PLAN_DISPLAY => {
                    target
                        .wall_settings
                        .as_ref()
                        .unwrap()
                        .floor_plan
                        .uncut_lines
                        == crate::wall::WallUncutLines::Dashed
                }
                GROUP_SURFACES => {
                    target
                        .wall_settings
                        .as_ref()
                        .unwrap()
                        .model
                        .override_surfaces
                }
                GROUP_DATA => {
                    target
                        .wall_settings
                        .as_ref()
                        .unwrap()
                        .structural
                        .structural_function
                        == crate::wall::StructuralFunction::Bearing
                }
                _ => false,
            };
            assert!(transferred, "group {group} did not transfer");
        }
    }
}
