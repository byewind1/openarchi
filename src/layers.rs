//! AC29-style dynamic layer document: layer status (visibility, lock, 3D
//! display), intersection groups, discipline categories and layer
//! combinations. All UI and model operations go through this typed document
//! so layer changes stay validated, undoable and inspectable.
//!
//! Evidence: `docs/features/layer-settings/contract.md` (AC29 runtime
//! observation + official help 025_Attributes-4/-5/-6/-8).

use crate::model::Element;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

pub type LayerId = u32;
pub type CategoryId = u32;
pub type CombinationId = u32;

/// The catch-all layer: cannot be deleted, hidden or locked (AC29 help,
/// Layer status). Elements whose layer is missing land here.
pub const ARCHICAD_LAYER: LayerId = 0;
pub const HIDDEN_LAYER: LayerId = 1;

pub const CATEGORY_DESIGN: CategoryId = 2;
pub const CATEGORY_ARCHITECTURAL: CategoryId = 3;
pub const CATEGORY_MEP: CategoryId = 4;
pub const CATEGORY_STRUCTURAL: CategoryId = 5;
pub const CATEGORY_DOCUMENTATION: CategoryId = 6;
pub const CATEGORY_VIEWPOINT: CategoryId = 7;

/// Per-layer status shown as eye / lock / 3D-display icons in Layer Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerStatus {
    pub visible: bool,
    pub locked: bool,
    /// `true` = solid model, `false` = wireframe; affects 3D views only.
    pub solid_3d: bool,
}

impl Default for LayerStatus {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
            solid_3d: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    /// Intersection group ("Extension" column). Layers with the same non-zero
    /// group intersect; two elements both on group 0 never intersect.
    #[serde(default = "default_group")]
    pub group: u8,
    #[serde(default)]
    pub status: LayerStatus,
    /// `None` sits at the tree root (Archicad Layer, - Hidden).
    #[serde(default)]
    pub category: Option<CategoryId>,
    /// Protected layers ignore hide/lock/delete attempts.
    #[serde(default)]
    pub protected: bool,
}

fn default_group() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub parent: Option<CategoryId>,
}

/// A named snapshot of every layer's status (AC29 help, Layer Combinations).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerCombination {
    pub id: CombinationId,
    pub name: String,
    pub statuses: BTreeMap<LayerId, LayerStatus>,
}

/// Project-wide layer data. Old projects without this field deserialize to
/// the AC29-template-like default document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerDocument {
    pub categories: Vec<Category>,
    pub layers: Vec<Layer>,
    pub combinations: Vec<LayerCombination>,
    pub active_combination: Option<CombinationId>,
    pub next_id: u32,
}

impl Default for LayerDocument {
    fn default() -> Self {
        Self::template()
    }
}

impl LayerDocument {
    /// Default document mirroring the observed AC29 INT template structure:
    /// root Archicad Layer and - Hidden; Design > Architectural (observed 15
    /// layers), MEP / Structural / Documentation / Viewpoint (inference);
    /// the ten default combinations. See the contract evidence table.
    pub fn template() -> Self {
        let categories = vec![
            Category {
                id: CATEGORY_DESIGN,
                name: "Design".into(),
                parent: None,
            },
            Category {
                id: CATEGORY_ARCHITECTURAL,
                name: "Architectural".into(),
                parent: Some(CATEGORY_DESIGN),
            },
            Category {
                id: CATEGORY_MEP,
                name: "MEP".into(),
                parent: Some(CATEGORY_DESIGN),
            },
            Category {
                id: CATEGORY_STRUCTURAL,
                name: "Structural".into(),
                parent: Some(CATEGORY_DESIGN),
            },
            Category {
                id: CATEGORY_DOCUMENTATION,
                name: "Documentation".into(),
                parent: None,
            },
            Category {
                id: CATEGORY_VIEWPOINT,
                name: "Viewpoint".into(),
                parent: None,
            },
        ];

        // Architectural layers were observed at runtime; the other category
        // layer lists are documented inference.
        let architectural = [
            "Finish - Ceiling",
            "Finish - Floor",
            "Finish - Wall",
            "Interior - Equipment",
            "Interior - Furniture",
            "Interior - Lamp",
            "Interior - Partition",
            "Interior - Stair & Railing",
            "Model Unit - Module",
            "Model Unit - Zone",
            "Morph - General",
            "Shell - General",
            "Shell - Roof",
            "Site & Landscape - General",
            "Site & Landscape - Terrain",
        ];
        let mep = [
            "Mechanical - General",
            "Mechanical - Ductwork",
            "Plumbing - General",
            "Electrical - General",
            "Electrical - Lighting",
        ];
        let structural = [
            "Structural - Column",
            "Structural - Beam",
            "Structural - Slab",
            "Structural - Foundation",
            "Structural - Wall",
        ];
        let documentation = [
            "Annotation - General",
            "Annotation - Dimension",
            "Annotation - Label",
        ];
        let viewpoint = ["Sections & Details", "3D Views"];

        let mut layers = vec![
            Layer {
                id: ARCHICAD_LAYER,
                name: "Archicad Layer".into(),
                group: 1,
                status: LayerStatus::default(),
                category: None,
                protected: true,
            },
            Layer {
                id: HIDDEN_LAYER,
                name: "- Hidden".into(),
                group: 1,
                status: LayerStatus {
                    visible: false,
                    ..LayerStatus::default()
                },
                category: None,
                protected: false,
            },
        ];
        let mut next_id = (layers.len() as LayerId).max(2);
        for (category, names) in [
            (CATEGORY_ARCHITECTURAL, &architectural[..]),
            (CATEGORY_MEP, &mep[..]),
            (CATEGORY_STRUCTURAL, &structural[..]),
            (CATEGORY_DOCUMENTATION, &documentation[..]),
            (CATEGORY_VIEWPOINT, &viewpoint[..]),
        ] {
            for name in names {
                layers.push(Layer {
                    id: next_id,
                    name: (*name).into(),
                    group: 1,
                    status: LayerStatus::default(),
                    category: Some(category),
                    protected: false,
                });
                next_id += 1;
            }
        }

        let names = [
            "01 Site",
            "02 Drafting",
            "03 Plans - Preliminary",
            "04 Plans - Approval",
            "05 Plans - Detailed",
            "06 Plans - Mechanical",
            "07 Plans - Structural",
            "08 Rendering",
            "09 Layouting",
            "10 Show 3D Zones as Solid",
        ];
        let combinations = names
            .iter()
            .enumerate()
            .map(|(index, name)| LayerCombination {
                id: index as CombinationId + 1,
                name: (*name).into(),
                statuses: layers
                    .iter()
                    .map(|layer| {
                        (
                            layer.id,
                            LayerStatus {
                                visible: template_combo_visible(index, layer),
                                ..LayerStatus::default()
                            },
                        )
                    })
                    .collect(),
            })
            .collect();

        Self {
            categories,
            layers,
            combinations,
            // The template ships with every layer (except - Hidden) visible,
            // which is exactly the "03 Plans - Preliminary" snapshot.
            active_combination: Some(3),
            next_id,
        }
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.id == id)
    }

    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    /// Intersection group of a layer; missing layers report group 1.
    pub fn group_of(&self, id: LayerId) -> u8 {
        self.layer(id).map_or(1, |layer| layer.group)
    }

    /// AC29 help, Use Layers to Prevent Intersections: only same non-zero
    /// groups intersect.
    pub fn layers_intersect(&self, a: LayerId, b: LayerId) -> bool {
        let group = self.group_of(a);
        group != 0 && group == self.group_of(b)
    }

    pub fn is_visible(&self, id: LayerId) -> bool {
        self.layer(id).is_none_or(|layer| layer.status.visible)
    }

    pub fn is_locked(&self, id: LayerId) -> bool {
        self.layer(id).is_some_and(|layer| layer.status.locked)
    }

    /// Sets one status flag while keeping the protected Archicad Layer
    /// visible and unlocked.
    pub fn set_status(&mut self, id: LayerId, status: LayerStatus) {
        let Some(layer) = self.layer_mut(id) else {
            return;
        };
        if layer.protected {
            return;
        }
        layer.status = status;
    }

    /// Applies every stored status of a combination (AC29: selecting a
    /// combination and confirming overwrites all layer statuses).
    pub fn apply_combination(&mut self, id: CombinationId) -> bool {
        let Some(combination) = self.combinations.iter().find(|c| c.id == id) else {
            return false;
        };
        let statuses = combination.statuses.clone();
        for layer in &mut self.layers {
            if let Some(status) = statuses.get(&layer.id) {
                layer.status = *status;
            }
        }
        self.active_combination = Some(id);
        true
    }

    /// Snapshot of the current statuses of every layer.
    pub fn statuses_snapshot(&self) -> BTreeMap<LayerId, LayerStatus> {
        self.layers
            .iter()
            .map(|layer| (layer.id, layer.status))
            .collect()
    }

    /// Writes the current statuses into a combination ("Update", AC29 help).
    pub fn update_combination(&mut self, id: CombinationId) -> bool {
        let snapshot = self.statuses_snapshot();
        let Some(combination) = self.combinations.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        combination.statuses = snapshot;
        true
    }

    /// The combination whose snapshot exactly matches the current statuses,
    /// preferring the active one (the Status checkmark in the dialog).
    pub fn effective_combination(&self) -> Option<CombinationId> {
        let snapshot = self.statuses_snapshot();
        let matches = |combination: &LayerCombination| combination.statuses == snapshot;
        if let Some(id) = self.active_combination {
            if self.combinations.iter().any(|c| c.id == id && matches(c)) {
                return Some(id);
            }
        }
        self.combinations.iter().find(|c| matches(c)).map(|c| c.id)
    }

    /// Creates a combination from the current statuses and selects it.
    pub fn new_combination(&mut self, name: &str) -> CombinationId {
        let id = self.next_id;
        self.next_id += 1;
        let mut name = name.to_string();
        if self.combinations.iter().any(|c| c.name == name) {
            name = format!("{name} {}", id);
        }
        self.combinations.push(LayerCombination {
            id,
            name,
            statuses: self.statuses_snapshot(),
        });
        self.active_combination = Some(id);
        id
    }

    pub fn delete_combination(&mut self, id: CombinationId) -> bool {
        let len_before = self.combinations.len();
        self.combinations.retain(|c| c.id != id);
        if self.active_combination == Some(id) {
            self.active_combination = self.combinations.first().map(|c| c.id);
        }
        self.combinations.len() != len_before
    }

    pub fn set_all_visible(&mut self, visible: bool) {
        for layer in &mut self.layers {
            if !layer.protected {
                layer.status.visible = visible;
            }
        }
    }

    /// Layers of one category subtree (inclusive), in document order.
    pub fn subtree_layer_ids(&self, category: CategoryId) -> Vec<LayerId> {
        let mut wanted = vec![category];
        let mut index = 0;
        while index < wanted.len() {
            let current = wanted[index];
            for child in &self.categories {
                if child.parent == Some(current) && !wanted.contains(&child.id) {
                    wanted.push(child.id);
                }
            }
            index += 1;
        }
        self.layers
            .iter()
            .filter(|layer| layer.category.is_some_and(|id| wanted.contains(&id)))
            .map(|layer| layer.id)
            .collect()
    }

    /// Root-level layers shown next to the top categories in the root view.
    pub fn root_layer_ids(&self) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|layer| layer.category.is_none())
            .map(|layer| layer.id)
            .collect()
    }

    /// Direct child categories of a parent (`None` = tree root).
    pub fn child_categories(&self, parent: Option<CategoryId>) -> Vec<CategoryId> {
        self.categories
            .iter()
            .filter(|category| category.parent == parent)
            .map(|category| category.id)
            .collect()
    }

    pub fn category_name(&self, id: Option<CategoryId>) -> String {
        match id {
            None => "Layers".into(),
            Some(id) => {
                let own = self
                    .category(id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                match self.category(id).and_then(|c| c.parent) {
                    Some(parent) => format!("{} / {}", self.category_name(Some(parent)), own),
                    None => own,
                }
            }
        }
    }

    /// Case-insensitive name filter used by the dialog search field.
    pub fn name_matches(&self, id: LayerId, needle: &str) -> bool {
        let needle = needle.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        self.layer(id).is_some_and(|layer| {
            layer.name.to_lowercase().contains(&needle)
                || self
                    .layer(id)
                    .and_then(|l| l.category)
                    .map(|c| self.category_name(Some(c)).to_lowercase())
                    .is_some_and(|path| path.contains(&needle))
        })
    }

    /// Adds a layer with a unique name and returns its id.
    pub fn new_layer(&mut self, name: &str, category: Option<CategoryId>) -> LayerId {
        let id = self.next_id;
        self.next_id += 1;
        let mut name = name.trim().to_string();
        if name.is_empty() || self.layers.iter().any(|l| l.name == name) {
            let mut suffix = 1;
            let base = if name.is_empty() {
                "New Layer".to_string()
            } else {
                name.clone()
            };
            loop {
                let candidate = format!("{base} {suffix}");
                if !self.layers.iter().any(|l| l.name == candidate) {
                    name = candidate;
                    break;
                }
                suffix += 1;
            }
        }
        self.layers.push(Layer {
            id,
            name,
            group: 1,
            status: LayerStatus::default(),
            category,
            protected: false,
        });
        id
    }

    pub fn new_category(&mut self, name: &str, parent: Option<CategoryId>) -> CategoryId {
        let id = self.next_id;
        self.next_id += 1;
        let mut name = name.trim().to_string();
        if name.is_empty() || self.categories.iter().any(|c| c.name == name) {
            name = format!("New Folder {}", id);
        }
        self.categories.push(Category { id, name, parent });
        id
    }

    pub fn rename_layer(&mut self, id: LayerId, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Layer name cannot be empty".into());
        }
        if self
            .layers
            .iter()
            .any(|l| l.id != id && l.name.eq_ignore_ascii_case(name))
        {
            return Err(format!("Layer name already in use: {name}"));
        }
        let Some(layer) = self.layer_mut(id) else {
            return Err("Layer not found".into());
        };
        layer.name = name.to_string();
        Ok(())
    }

    /// Removes a layer; callers move elements off it first. The protected
    /// Archicad Layer cannot be deleted.
    pub fn delete_layer(&mut self, id: LayerId) -> Result<(), String> {
        if id == ARCHICAD_LAYER {
            return Err("The Archicad Layer cannot be deleted".into());
        }
        let len_before = self.layers.len();
        self.layers.retain(|layer| layer.id != id);
        if self.layers.len() == len_before {
            return Err("Layer not found".into());
        }
        for combination in &mut self.combinations {
            combination.statuses.remove(&id);
        }
        Ok(())
    }

    /// Maps legacy `Layer` enum names (pre-dynamic projects) onto template
    /// layers; unknown names land on the Archicad Layer like AC's catch-all.
    pub fn legacy_layer_id(name: &str) -> LayerId {
        let target = match name {
            "Site" => "Site & Landscape - General",
            "Drafting" | "Layouting" => "Annotation - General",
            "Structural" => "Structural - Wall",
            "Mechanical" => "Mechanical - General",
            "Electrical" => "Electrical - General",
            _ => return ARCHICAD_LAYER,
        };
        Self::template()
            .layers
            .iter()
            .find(|layer| layer.name == target)
            .map(|layer| layer.id)
            .unwrap_or(ARCHICAD_LAYER)
    }

    /// Intersection groups of a wall batch, parallel to the input order, for
    /// the plan/3D join passes.
    pub fn wall_groups(&self, walls: &[&Element]) -> Vec<u8> {
        walls
            .iter()
            .map(|wall| self.group_of(wall.layer.0))
            .collect()
    }

    /// Number of elements sitting on each layer, for delete warnings and
    /// status readouts.
    pub fn element_counts(&self, elements: &[Element]) -> BTreeMap<LayerId, usize> {
        let mut counts = BTreeMap::new();
        for element in elements {
            *counts.entry(element.layer.0).or_default() += 1;
        }
        counts
    }
}

/// Visibility matrix for the ten default combinations (documented inference;
/// combination names are runtime-observed).
fn template_combo_visible(index: usize, layer: &Layer) -> bool {
    if layer.id == HIDDEN_LAYER {
        return false;
    }
    if layer.protected {
        return true;
    }
    let Some(category) = layer.category else {
        return true;
    };
    let name = layer.name.as_str();
    match index {
        // 01 Site: site model and structure, no interiors or MEP.
        0 => {
            category == CATEGORY_STRUCTURAL
                || category == CATEGORY_DOCUMENTATION
                || category == CATEGORY_VIEWPOINT
                || name.starts_with("Site & Landscape")
        }
        // 02 Drafting: base model plus annotations, no MEP.
        1 | 6 => category != CATEGORY_MEP,
        // 03-05 plans and 09-10: everything except the hidden layer.
        2 | 3 | 4 | 8 | 9 => true,
        // 06 Plans - Mechanical: mechanical scopes visible, electrical dimmed.
        5 => category != CATEGORY_MEP || !name.starts_with("Electrical"),
        // 08 Rendering: model only, annotations off.
        7 => category != CATEGORY_DOCUMENTATION,
        _ => true,
    }
}

/// Element→layer reference. New projects store numeric layer ids; files from
/// the fixed-enum era deserialize their old layer names onto template layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ElementLayer(pub LayerId);

impl ElementLayer {
    pub const fn new(id: LayerId) -> Self {
        Self(id)
    }
}

impl Default for ElementLayer {
    fn default() -> Self {
        Self(ARCHICAD_LAYER)
    }
}

impl From<LayerId> for ElementLayer {
    fn from(id: LayerId) -> Self {
        Self(id)
    }
}

impl std::ops::Deref for ElementLayer {
    type Target = LayerId;

    fn deref(&self) -> &LayerId {
        &self.0
    }
}

struct ElementLayerVisitor;

impl serde::de::Visitor<'_> for ElementLayerVisitor {
    type Value = ElementLayer;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a numeric layer id or a legacy layer name")
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<ElementLayer, E> {
        Ok(ElementLayer(value as LayerId))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<ElementLayer, E> {
        Ok(ElementLayer(value as LayerId))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<ElementLayer, E> {
        Ok(ElementLayer(LayerDocument::legacy_layer_id(value)))
    }
}

impl<'de> Deserialize<'de> for ElementLayer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ElementLayerVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Point;
    use crate::wall::WallSettings;

    fn wall(layer: LayerId) -> Element {
        Element {
            id: 1,
            name: "Wall".into(),
            story: 0,
            material: crate::model::Material::Concrete,
            layer: ElementLayer(layer),
            renovation: crate::view_settings::RenovationStatus::default(),
            design_option: None,
            wall_contour: None,
            slab_contour: None,
            wall_settings: Some(WallSettings::default()),
            shape: crate::model::Shape::Wall {
                start: Point::default(),
                end: Point::default(),
                height: 3.,
                thickness: 0.2,
            },
        }
    }

    #[test]
    fn template_matches_observed_ac29_structure() {
        let doc = LayerDocument::template();
        assert_eq!(
            doc.layer(ARCHICAD_LAYER).map(|l| l.name.as_str()),
            Some("Archicad Layer")
        );
        assert!(doc.layer(ARCHICAD_LAYER).unwrap().protected);
        assert_eq!(
            doc.layer(HIDDEN_LAYER).map(|l| l.name.as_str()),
            Some("- Hidden")
        );
        assert!(!doc.is_visible(HIDDEN_LAYER));
        assert_eq!(doc.categories.len(), 6);
        let design = doc.category(CATEGORY_DESIGN).unwrap();
        assert_eq!(design.parent, None);
        assert_eq!(
            doc.category(CATEGORY_ARCHITECTURAL).unwrap().parent,
            Some(CATEGORY_DESIGN)
        );
        let architectural = doc.subtree_layer_ids(CATEGORY_ARCHITECTURAL);
        assert_eq!(architectural.len(), 15);
        assert!(
            architectural.contains(
                &doc.layers
                    .iter()
                    .find(|l| l.name == "Finish - Ceiling")
                    .unwrap()
                    .id
            )
        );
        assert_eq!(doc.combinations.len(), 10);
        assert_eq!(doc.combinations[2].name, "03 Plans - Preliminary");
        assert_eq!(doc.combinations[9].name, "10 Show 3D Zones as Solid");
    }

    #[test]
    fn intersection_groups_follow_ac29_rules() {
        let mut doc = LayerDocument::template();
        let a = doc.new_layer("A", None);
        let b = doc.new_layer("B", None);
        assert!(doc.layers_intersect(a, b));
        doc.layer_mut(b).unwrap().group = 2;
        assert!(!doc.layers_intersect(a, b));
        doc.layer_mut(a).unwrap().group = 2;
        assert!(doc.layers_intersect(a, b));
        doc.layer_mut(a).unwrap().group = 0;
        doc.layer_mut(b).unwrap().group = 0;
        assert!(!doc.layers_intersect(a, b), "group 0 never intersects");
    }

    #[test]
    fn archicad_layer_cannot_be_hidden_locked_or_deleted() {
        let mut doc = LayerDocument::template();
        doc.set_status(
            ARCHICAD_LAYER,
            LayerStatus {
                visible: false,
                locked: true,
                solid_3d: false,
            },
        );
        assert!(doc.is_visible(ARCHICAD_LAYER));
        assert!(!doc.is_locked(ARCHICAD_LAYER));
        assert!(doc.delete_layer(ARCHICAD_LAYER).is_err());
    }

    #[test]
    fn apply_and_update_combination_round_trip() {
        let mut doc = LayerDocument::template();
        let target = doc
            .layers
            .iter()
            .find(|l| l.name == "Finish - Wall")
            .unwrap()
            .id;
        assert!(doc.is_visible(target));
        doc.apply_combination(1); // 01 Site hides interiors
        assert!(!doc.is_visible(target));
        assert_eq!(doc.effective_combination(), Some(1));

        // Manual change detaches from the combination; Update writes it back.
        doc.set_status(target, LayerStatus::default());
        assert_ne!(doc.effective_combination(), Some(1));
        assert!(doc.update_combination(1));
        assert_eq!(doc.effective_combination(), Some(1));
        doc.apply_combination(1);
        assert!(doc.is_visible(target));
    }

    #[test]
    fn new_combination_snapshots_current_statuses() {
        let mut doc = LayerDocument::template();
        let target = doc
            .layers
            .iter()
            .find(|l| l.name == "Morph - General")
            .unwrap()
            .id;
        doc.set_status(
            target,
            LayerStatus {
                visible: false,
                locked: true,
                solid_3d: false,
            },
        );
        let id = doc.new_combination("11 Custom");
        assert_eq!(doc.active_combination, Some(id));
        assert!(doc.layer(target).unwrap().status.locked);
        doc.set_status(target, LayerStatus::default());
        assert_ne!(doc.effective_combination(), Some(id));
        doc.apply_combination(id);
        let layer = doc.layer(target).unwrap();
        assert!(!layer.status.visible && layer.status.locked && !layer.status.solid_3d);
    }

    #[test]
    fn delete_layer_moves_combination_entries_and_refuses_archicad_layer() {
        let mut doc = LayerDocument::template();
        let id = doc.new_layer("Temp", None);
        doc.update_combination(1);
        assert!(doc.combinations[0].statuses.contains_key(&id));
        doc.delete_layer(id).unwrap();
        assert!(!doc.combinations[0].statuses.contains_key(&id));
    }

    #[test]
    fn legacy_names_map_onto_template_layers() {
        assert_eq!(
            LayerDocument::legacy_layer_id("Architectural"),
            ARCHICAD_LAYER
        );
        assert_eq!(LayerDocument::legacy_layer_id("Nonsense"), ARCHICAD_LAYER);
        let doc = LayerDocument::template();
        let structural = doc
            .layers
            .iter()
            .find(|l| l.name == "Structural - Wall")
            .unwrap()
            .id;
        assert_eq!(LayerDocument::legacy_layer_id("Structural"), structural);
    }

    #[test]
    fn element_layer_deserializes_numbers_and_legacy_names() {
        assert_eq!(
            serde_json::from_str::<ElementLayer>("7").unwrap(),
            ElementLayer(7)
        );
        assert_eq!(
            serde_json::from_str::<ElementLayer>("\"Architectural\"").unwrap(),
            ElementLayer(ARCHICAD_LAYER)
        );
        assert_eq!(serde_json::to_string(&ElementLayer(7)).unwrap(), "7");
    }

    #[test]
    fn search_and_show_counts() {
        let doc = LayerDocument::template();
        let architectural = doc.subtree_layer_ids(CATEGORY_ARCHITECTURAL);
        let shown = architectural
            .iter()
            .filter(|id| doc.name_matches(**id, "wall"))
            .count();
        assert_eq!(shown, 1, "Finish - Wall only");
        assert!(doc.name_matches(ARCHICAD_LAYER, ""));
        assert!(
            doc.name_matches(
                doc.layers
                    .iter()
                    .find(|l| l.name == "Morph - General")
                    .unwrap()
                    .id,
                "design"
            )
        );
    }

    #[test]
    fn element_counts_group_by_layer() {
        let doc = LayerDocument::template();
        let elements = vec![
            wall(ARCHICAD_LAYER),
            wall(ARCHICAD_LAYER),
            wall(HIDDEN_LAYER),
        ];
        let counts = doc.element_counts(&elements);
        assert_eq!(counts.get(&ARCHICAD_LAYER), Some(&2));
        assert_eq!(counts.get(&HIDDEN_LAYER), Some(&1));
    }
}
