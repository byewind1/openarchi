//! Versioned native module/reference operations. File adapters remain explicit.
use crate::{
    drawings::{DrawingDocument, DrawingPlacement},
    model::{Element, Point, Project, Shape, Story},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceKind {
    Hotlink,
    Xref,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceSource {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub sha256: String,
    pub kind: ReferenceKind,
    pub elements: Vec<Element>,
    pub stories: Vec<Story>,
    pub layers: crate::layers::LayerDocument,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceInstance {
    pub id: u64,
    pub source: u64,
    pub placement: DrawingPlacement,
    pub elements: Vec<u64>,
    pub loaded: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ReferenceEdit {
    Place {
        source: ReferenceSource,
        placement: DrawingPlacement,
    },
    ReplaceSelection {
        ids: Vec<u64>,
        source: ReferenceSource,
        story: u32,
    },
    Update {
        source: ReferenceSource,
    },
    Reload {
        source: ReferenceSource,
    },
    Replace {
        instance: u64,
        source: ReferenceSource,
    },
    Break {
        instance: u64,
    },
    Delete {
        instance: u64,
    },
    Loaded {
        source: u64,
        loaded: bool,
    },
    Bind {
        source: u64,
    },
}
impl DrawingDocument {
    pub fn reference_owner(&self, id: u64) -> Option<u64> {
        self.reference_instances
            .iter()
            .find(|v| v.elements.contains(&id))
            .map(|v| v.id)
    }
}
fn p_layer(project: &mut Project, name: &str) -> u32 {
    project
        .layers
        .layers
        .iter()
        .find(|v| v.name == name)
        .map(|v| v.id)
        .unwrap_or_else(|| project.layers.new_layer(name, None))
}
fn populate(project: &mut Project, instance: &mut ReferenceInstance) -> Result<(), String> {
    let source = project
        .drawings
        .reference_sources
        .iter()
        .find(|v| v.id == instance.source)
        .cloned()
        .ok_or("Reference source missing")?;
    let mut map = HashMap::new();
    for element in &source.elements {
        let id = project.next_id;
        project.next_id = project
            .next_id
            .checked_add(1)
            .ok_or("Invalid element identity or story")?;
        map.insert(element.id, id);
    }
    let base = source
        .stories
        .iter()
        .position(|s| s.id == source.elements.first().map_or(0, |e| e.story))
        .unwrap_or(0);
    let target = project
        .stories
        .iter()
        .position(|s| s.id == instance.placement.story)
        .ok_or("Invalid story")?;
    for mut element in source.elements {
        let source_index = source
            .stories
            .iter()
            .position(|s| s.id == element.story)
            .ok_or("Invalid story")?;
        let index = target as isize + source_index as isize - base as isize;
        let story = usize::try_from(index)
            .ok()
            .and_then(|i| project.stories.get(i))
            .ok_or("Create matching host stories before placing this module")?
            .clone();

        element.id = map[&element.id];
        element.story = story.id;
        element.worksheet = instance.placement.worksheet;
        let original = source
            .layers
            .layer(element.layer.0)
            .map_or("Archicad Layer", |v| v.name.as_str());
        let layer_name = format!("{}#{}|{}", source.name, source.id, original);
        let layer = p_layer(project, &layer_name);
        if project.layers.is_locked(layer) {
            return Err("Selection contains locked elements".into());
        }
        element.layer = crate::layers::ElementLayer::new(layer);
        if let Shape::Opening { host_wall, .. } = &mut element.shape {
            *host_wall = *map
                .get(host_wall)
                .ok_or("Select the host wall with its openings")?;
        }
        crate::editing::transform(
            &mut element,
            Point::default(),
            instance.placement.angle,
            1.,
            false,
            instance.placement.origin,
        )?;
        element.design_option = None;
        instance.elements.push(element.id);
        project.elements.push(element);
    }
    Ok(())
}
pub fn apply(project: &Project, edit: ReferenceEdit) -> Result<Project, String> {
    let mut p = project.clone();
    let remove = |p: &mut Project, ids: &[u64]| -> Result<(), String> {
        if p.elements.iter().any(|e| {
            ids.contains(&e.id)
                && (p.layers.is_locked(e.layer.0) || p.edit_state.locked.contains(&e.id))
        }) {
            return Err("Selection contains locked elements".into());
        }
        p.elements.retain(|e| !ids.contains(&e.id));
        Ok(())
    };
    match edit {
        ReferenceEdit::ReplaceSelection { ids, source, story } => {
            if ids
                .iter()
                .any(|id| p.drawings.reference_owner(*id).is_some())
            {
                return Err("Selection contains linked elements".into());
            }
            remove(&mut p, &ids)?;
            p = apply(
                &p,
                ReferenceEdit::Place {
                    source,
                    placement: DrawingPlacement {
                        worksheet: None,
                        story,
                        origin: Point::default(),
                        angle: 0.,
                        visible: true,
                        opacity: 255,
                        frame: None,
                    },
                },
            )?;
        }
        ReferenceEdit::Place {
            mut source,
            placement,
        } => {
            if source.elements.is_empty() {
                return Err("Select elements to create a module".into());
            }
            if let Some(existing) = p.drawings.reference_sources.iter().find(|s| {
                s.path == source.path && s.sha256 == source.sha256 && s.kind == source.kind
            }) {
                source.id = existing.id;
            } else {
                source.id = p.drawings.next_id()?;
                p.drawings.reference_sources.push(source.clone());
            }
            let mut instance = ReferenceInstance {
                id: p.drawings.next_id()?,
                source: source.id,
                placement,
                elements: vec![],
                loaded: true,
            };
            populate(&mut p, &mut instance)?;
            p.drawings.reference_instances.push(instance);
        }
        ReferenceEdit::Reload { source } => {
            let id = source.id;
            p = apply(&p, ReferenceEdit::Update { source })?;
            p = apply(
                &p,
                ReferenceEdit::Loaded {
                    source: id,
                    loaded: true,
                },
            )?;
        }
        ReferenceEdit::Update { source } => {
            let id = source.id;
            if !p.drawings.reference_sources.iter().any(|s| s.id == id) {
                return Err("Reference source missing".into());
            }
            let indexes: Vec<_> = p
                .drawings
                .reference_instances
                .iter()
                .enumerate()
                .filter(|(_, v)| v.source == id)
                .map(|(i, _)| i)
                .collect();
            for i in &indexes {
                let ids = p.drawings.reference_instances[*i].elements.clone();
                remove(&mut p, &ids)?;
            }
            *p.drawings
                .reference_sources
                .iter_mut()
                .find(|s| s.id == id)
                .unwrap() = source;
            for i in indexes {
                let mut instance = p.drawings.reference_instances[i].clone();
                instance.elements.clear();
                if instance.loaded {
                    populate(&mut p, &mut instance)?;
                }
                p.drawings.reference_instances[i] = instance;
            }
        }
        ReferenceEdit::Replace { instance, source } => {
            let placement = p
                .drawings
                .reference_instances
                .iter()
                .find(|v| v.id == instance)
                .ok_or("Select a reference instance")?
                .placement
                .clone();
            p = apply(&p, ReferenceEdit::Delete { instance })?;
            p = apply(&p, ReferenceEdit::Place { source, placement })?;
        }
        ReferenceEdit::Break { instance } => {
            let v = p
                .drawings
                .reference_instances
                .iter()
                .find(|v| v.id == instance)
                .ok_or("Select a reference instance")?;
            if p.elements
                .iter()
                .any(|e| v.elements.contains(&e.id) && p.layers.is_locked(e.layer.0))
            {
                return Err("Selection contains locked elements".into());
            }
            let members = v.elements.clone();
            p.drawings.reference_instances.retain(|v| v.id != instance);
            if members.len() > 1 {
                p.edit_state.groups.push(members);
            }
        }
        ReferenceEdit::Delete { instance } => {
            let ids = p
                .drawings
                .reference_instances
                .iter()
                .find(|v| v.id == instance)
                .ok_or("Select a reference instance")?
                .elements
                .clone();
            remove(&mut p, &ids)?;
            p.drawings.reference_instances.retain(|v| v.id != instance);
        }
        ReferenceEdit::Loaded { source, loaded } => {
            let indexes: Vec<_> = p
                .drawings
                .reference_instances
                .iter()
                .enumerate()
                .filter(|(_, v)| v.source == source)
                .map(|(i, _)| i)
                .collect();
            for i in indexes {
                let mut v = p.drawings.reference_instances[i].clone();
                remove(&mut p, &v.elements)?;
                v.elements.clear();
                v.loaded = loaded;
                if loaded {
                    populate(&mut p, &mut v)?;
                }
                p.drawings.reference_instances[i] = v;
            }
        }
        ReferenceEdit::Bind { source } => {
            if p.drawings
                .reference_instances
                .iter()
                .any(|v| v.source == source && !v.loaded)
            {
                p = apply(
                    &p,
                    ReferenceEdit::Loaded {
                        source,
                        loaded: true,
                    },
                )?;
            }
            if p.drawings
                .reference_instances
                .iter()
                .filter(|v| v.source == source)
                .flat_map(|v| &v.elements)
                .any(|id| {
                    p.elements.iter().any(|e| {
                        e.id == *id
                            && (p.layers.is_locked(e.layer.0) || p.edit_state.locked.contains(id))
                    })
                })
            {
                return Err("Selection contains locked elements".into());
            }
            for v in p
                .drawings
                .reference_instances
                .iter()
                .filter(|v| v.source == source && v.elements.len() > 1)
            {
                p.edit_state.groups.push(v.elements.clone());
            }
            if let Some(s) = p
                .drawings
                .reference_sources
                .iter()
                .find(|s| s.id == source && s.kind == ReferenceKind::Xref)
            {
                let prefix = format!("{}#{}|", s.name, s.id);
                for layer in &mut p.layers.layers {
                    if layer.name.starts_with(&prefix) {
                        layer.name =
                            layer
                                .name
                                .replacen(&prefix, &format!("{}${}$", s.name, s.id), 1);
                    }
                }
            }
            p.drawings
                .reference_instances
                .retain(|v| v.source != source);
            p.drawings.reference_sources.retain(|v| v.id != source);
        }
    }
    p.validate()?;
    Ok(p)
}
pub fn validate(project: &Project) -> Result<(), String> {
    let mut ids: HashSet<_> = project
        .drawings
        .pages
        .iter()
        .map(|p| p.id)
        .chain(project.drawings.regions.iter().map(|r| r.id))
        .chain(project.drawings.worksheets.iter().map(|w| w.id))
        .collect();
    let mut members = HashSet::new();
    for source in &project.drawings.reference_sources {
        if source.id == 0
            || !ids.insert(source.id)
            || source.path.as_os_str().is_empty()
            || source.sha256.len() != 64
            || !source
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || source.elements.len() > 100_000
            || source.stories.is_empty()
        {
            return Err("Invalid external reference".into());
        }
        let snapshot = Project {
            elements: source.elements.clone(),
            stories: source.stories.clone(),
            layers: source.layers.clone(),
            next_id: source
                .elements
                .iter()
                .map(|e| e.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("Invalid external reference")?,
            ..Default::default()
        };
        snapshot.validate()?;
    }
    for instance in &project.drawings.reference_instances {
        if instance.id == 0
            || !ids.insert(instance.id)
            || !project
                .drawings
                .reference_sources
                .iter()
                .any(|s| s.id == instance.source)
            || !instance.placement.origin.finite()
            || !instance.placement.angle.is_finite()
            || !project
                .stories
                .iter()
                .any(|s| s.id == instance.placement.story)
            || instance
                .placement
                .worksheet
                .is_some_and(|id| !project.drawings.worksheet_exists(id))
            || (!instance.loaded && !instance.elements.is_empty())
        {
            return Err("Invalid external reference".into());
        }
        for id in &instance.elements {
            if !members.insert(*id) || !project.elements.iter().any(|e| e.id == *id) {
                return Err("Invalid external reference".into());
            }
        }
    }
    Ok(())
}
/// Native modules are OpenArchi JSON projects, never mislabeled MOD/PLN files.
pub fn source(path: &std::path::Path, kind: ReferenceKind) -> Result<ReferenceSource, String> {
    let bytes = crate::drawings::source_bytes(path)?;
    let mut p: Project = if kind == ReferenceKind::Xref
        && path
            .extension()
            .is_some_and(|v| v.eq_ignore_ascii_case("dxf"))
    {
        crate::dxf::parse(&bytes)?
    } else {
        serde_json::from_slice(&bytes).map_err(|_| {
            "Choose an OpenArchi JSON module (or supported ASCII DXF for Xref)".to_string()
        })?
    };
    if kind == ReferenceKind::Xref
        && path
            .extension()
            .is_some_and(|v| v.eq_ignore_ascii_case("dxf"))
    {
        p.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("DXF")
            .to_owned();
    }
    p.validate()?;
    if !p.drawings.reference_instances.is_empty() {
        return Err("Embed nested references before creating this module".into());
    }
    Ok(ReferenceSource {
        id: 0,
        name: p.name,
        path: path.canonicalize().map_err(|e| e.to_string())?,
        sha256: crate::drawings::digest(&bytes),
        kind,
        elements: p
            .elements
            .into_iter()
            .filter(|e| e.worksheet.is_none())
            .collect(),
        stories: p.stories,
        layers: p.layers,
    })
}
