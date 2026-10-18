//! Typed model commands and reversible diffs.
//!
//! An edit is recorded as a [`CommandRecord`] containing the requested
//! command, its inverse and an exact [`ProjectDelta`]. The delta keeps only
//! changed elements and project headers, so undo history no longer clones the
//! whole project for every edit.

use crate::{
    grids::GridSettings,
    guides::{GuideSettings, PersistentGuide},
    model::{Element, Point, Project, Story},
    view_settings::{DesignOptions, PlanOptions},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// One element's before/after state inside a [`ProjectDelta`].
#[derive(Clone, Debug, PartialEq)]
pub struct ElementEdit {
    pub id: u64,
    pub before_index: Option<usize>,
    pub before: Option<Element>,
    pub after_index: Option<usize>,
    pub after: Option<Element>,
}

/// Project fields outside `elements`, captured only when they change.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectHeader {
    pub schema_version: u32,
    pub name: String,
    pub next_id: u64,
    pub stories: Vec<Story>,
    pub guides: Vec<PersistentGuide>,
    pub grid_settings: GridSettings,
    pub guide_settings: GuideSettings,
    pub plan_options: PlanOptions,
    pub design_options: DesignOptions,
}

impl ProjectHeader {
    pub(crate) fn capture(project: &Project) -> Self {
        Self {
            schema_version: project.schema_version,
            name: project.name.clone(),
            next_id: project.next_id,
            stories: project.stories.clone(),
            guides: project.guides.clone(),
            grid_settings: project.grid_settings.clone(),
            guide_settings: project.guide_settings.clone(),
            plan_options: project.plan_options.clone(),
            design_options: project.design_options.clone(),
        }
    }

    fn restore(&self, project: &mut Project) {
        project.schema_version = self.schema_version;
        project.name.clone_from(&self.name);
        project.next_id = self.next_id;
        project.stories.clone_from(&self.stories);
        project.guides.clone_from(&self.guides);
        project.grid_settings.clone_from(&self.grid_settings);
        project.guide_settings.clone_from(&self.guide_settings);
        project.plan_options.clone_from(&self.plan_options);
        project.design_options.clone_from(&self.design_options);
    }
}

/// Minimal reversible difference between two project states.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectDelta {
    pub elements: Vec<ElementEdit>,
    /// Full id order is stored only when the relative order of surviving
    /// elements changed; ordinary add/remove/update edits rely on indices.
    pub before_order: Option<Arc<Vec<u64>>>,
    pub after_order: Option<Arc<Vec<u64>>>,
    pub before_header: Option<Box<ProjectHeader>>,
    pub after_header: Option<Box<ProjectHeader>>,
}

impl ProjectDelta {
    pub fn between(before: &Project, after: &Project) -> Self {
        let mut delta = Self::default();
        if before.schema_version != after.schema_version
            || before.name != after.name
            || before.next_id != after.next_id
            || before.stories != after.stories
            || before.guides != after.guides
            || before.grid_settings != after.grid_settings
            || before.guide_settings != after.guide_settings
            || before.plan_options != after.plan_options
            || before.design_options != after.design_options
        {
            delta.before_header = Some(Box::new(ProjectHeader::capture(before)));
            delta.after_header = Some(Box::new(ProjectHeader::capture(after)));
        }

        let before_positions: HashMap<u64, usize> = before
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| (element.id, index))
            .collect();
        let after_positions: HashMap<u64, usize> = after
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| (element.id, index))
            .collect();

        for element in &before.elements {
            match after_positions.get(&element.id) {
                Some(&after_index) => {
                    let after_element = &after.elements[after_index];
                    if after_element != element {
                        delta.elements.push(ElementEdit {
                            id: element.id,
                            before_index: before_positions.get(&element.id).copied(),
                            before: Some(element.clone()),
                            after_index: Some(after_index),
                            after: Some(after_element.clone()),
                        });
                    }
                }
                None => {
                    delta.elements.push(ElementEdit {
                        id: element.id,
                        before_index: before_positions.get(&element.id).copied(),
                        before: Some(element.clone()),
                        after_index: None,
                        after: None,
                    });
                }
            }
        }
        for (after_index, element) in after.elements.iter().enumerate() {
            if !before_positions.contains_key(&element.id) {
                delta.elements.push(ElementEdit {
                    id: element.id,
                    before_index: None,
                    before: None,
                    after_index: Some(after_index),
                    after: Some(element.clone()),
                });
            }
        }

        let before_survivors: Vec<u64> = before
            .elements
            .iter()
            .map(|element| element.id)
            .filter(|id| after_positions.contains_key(id))
            .collect();
        let after_survivors: Vec<u64> = after
            .elements
            .iter()
            .map(|element| element.id)
            .filter(|id| before_positions.contains_key(id))
            .collect();
        if before_survivors != after_survivors {
            delta.before_order = Some(Arc::new(
                before.elements.iter().map(|element| element.id).collect(),
            ));
            delta.after_order = Some(Arc::new(
                after.elements.iter().map(|element| element.id).collect(),
            ));
        }

        delta
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty() && self.before_order.is_none() && self.before_header.is_none()
    }

    pub fn affected_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.elements.iter().map(|edit| edit.id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Number of element copies retained by this delta. Used by the
    /// performance baseline to show history growth is proportional to edits.
    pub fn retained_element_snapshots(&self) -> usize {
        self.elements
            .iter()
            .map(|edit| usize::from(edit.before.is_some()) + usize::from(edit.after.is_some()))
            .sum()
    }

    /// Restore the state before this delta.
    pub fn revert(&self, project: &mut Project) {
        self.apply(project, false);
    }

    /// Re-apply the state after this delta.
    pub fn replay(&self, project: &mut Project) {
        self.apply(project, true);
    }

    fn apply(&self, project: &mut Project, forward: bool) {
        let header = if forward {
            self.after_header.as_deref()
        } else {
            self.before_header.as_deref()
        };
        if let Some(header) = header {
            header.restore(project);
        }
        if self.elements.is_empty() {
            if let Some(order) = self.order(forward) {
                reorder(&mut project.elements, order);
            }
            return;
        }

        let removed: HashSet<u64> = self
            .elements
            .iter()
            .filter(|edit| {
                if forward {
                    edit.after.is_none()
                } else {
                    edit.before.is_none()
                }
            })
            .map(|edit| edit.id)
            .collect();
        let mut elements = std::mem::take(&mut project.elements);
        elements.retain(|element| !removed.contains(&element.id));
        let mut positions: HashMap<u64, usize> = elements
            .iter()
            .enumerate()
            .map(|(index, element)| (element.id, index))
            .collect();
        let mut inserts: Vec<(usize, Element)> = Vec::new();
        for edit in &self.elements {
            let (target, target_index) = if forward {
                (edit.after.as_ref(), edit.after_index)
            } else {
                (edit.before.as_ref(), edit.before_index)
            };
            let Some(target) = target else { continue };
            if let Some(&index) = positions.get(&edit.id) {
                elements[index] = target.clone();
            } else {
                inserts.push((target_index.unwrap_or(elements.len()), target.clone()));
            }
        }
        inserts.sort_by_key(|(index, _)| *index);
        for (index, element) in inserts {
            let index = index.min(elements.len());
            positions.insert(element.id, index);
            elements.insert(index, element);
        }
        project.elements = elements;
        if let Some(order) = self.order(forward) {
            reorder(&mut project.elements, order);
        }
    }

    fn order(&self, forward: bool) -> Option<&Arc<Vec<u64>>> {
        if forward {
            self.after_order.as_ref()
        } else {
            self.before_order.as_ref()
        }
    }
}

fn reorder(elements: &mut [Element], order: &[u64]) {
    let rank: HashMap<u64, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    elements.sort_by_key(|element| rank.get(&element.id).copied().unwrap_or(usize::MAX));
}

/// One user-level model command. `ChangeSet` is reserved for existing complex
/// edits that are migrated over time; its exact payload lives in the record.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelCommand {
    AddElement {
        element: Element,
    },
    RemoveElements {
        ids: Vec<u64>,
    },
    RestoreElements {
        elements: Vec<(usize, Element)>,
    },
    UpdateElement {
        element: Element,
    },
    MoveElements {
        ids: Vec<u64>,
        offset: Point,
    },
    SetOptions {
        plan: Option<PlanOptions>,
        design: Option<DesignOptions>,
        guide: Option<GuideSettings>,
        grid: Option<GridSettings>,
    },
    ChangeSet,
}

/// A command together with its inverse and the exact delta used by undo/redo.
#[derive(Clone, Debug)]
pub struct CommandRecord {
    pub command: ModelCommand,
    pub inverse: ModelCommand,
    pub(crate) delta: ProjectDelta,
}

impl CommandRecord {
    pub fn new(command: ModelCommand, inverse: ModelCommand, delta: ProjectDelta) -> Self {
        Self {
            command,
            inverse,
            delta,
        }
    }

    pub fn affected_ids(&self) -> Vec<u64> {
        self.delta.affected_ids()
    }

    pub fn retained_element_snapshots(&self) -> usize {
        self.delta.retained_element_snapshots()
    }

    pub fn from_delta(delta: ProjectDelta) -> Self {
        Self {
            command: ModelCommand::ChangeSet,
            inverse: ModelCommand::ChangeSet,
            delta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Material, Shape};

    fn project_with_walls(count: usize) -> Project {
        let mut project = Project::default();
        for index in 0..count {
            project.add(
                0,
                Material::Plaster,
                Shape::Wall {
                    start: Point::new(index as f32, 0.),
                    end: Point::new(index as f32 + 1., 0.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
        }
        project
    }

    #[test]
    fn delta_roundtrip_restores_elements_and_headers() {
        let before = project_with_walls(3);
        let mut after = before.clone();
        after.elements[1].translate(Point::new(2., 1.));
        after.elements.remove(0);
        after.guides.push(PersistentGuide {
            story: 0,
            primitive: crate::guides::GuidePrimitive::Circle {
                center: Point::new(1., 1.),
                radius: 2.,
            },
        });
        let delta = ProjectDelta::between(&before, &after);
        assert_eq!(delta.affected_ids(), vec![1, 2]);

        let mut state = after.clone();
        delta.revert(&mut state);
        assert_eq!(state, before);
        delta.replay(&mut state);
        assert_eq!(state, after);
    }

    #[test]
    fn delta_is_proportional_to_changed_elements() {
        let before = project_with_walls(10_000);
        let mut after = before.clone();
        after.elements[4_321].translate(Point::new(1., 0.));
        let delta = ProjectDelta::between(&before, &after);
        assert_eq!(delta.elements.len(), 1);
        let mut state = after.clone();
        delta.revert(&mut state);
        assert_eq!(state, before);
    }

    #[test]
    fn unchanged_projects_produce_an_empty_delta() {
        let project = project_with_walls(4);
        let delta = ProjectDelta::between(&project, &project.clone());
        assert!(delta.is_empty());
    }
}
