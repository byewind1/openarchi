use crate::model::{Element, Material, Point, Project, Shape};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceReference {
    AboveCurrentStory,
    #[default]
    BelowCurrentStory,
    PreviousStory,
    EntireModel,
    Story(u32),
}

impl TraceReference {
    pub fn resolve(
        self,
        project: &Project,
        active_story: u32,
        previous_story: u32,
    ) -> Option<TraceSelection> {
        let stories = ordered_story_ids(project);
        match self {
            Self::EntireModel => Some(TraceSelection::EntireModel),
            Self::Story(id) => stories.contains(&id).then_some(TraceSelection::Story(id)),
            Self::PreviousStory => (previous_story != active_story
                && stories.contains(&previous_story))
            .then_some(TraceSelection::Story(previous_story)),
            Self::AboveCurrentStory => stories
                .iter()
                .position(|id| *id == active_story)
                .and_then(|index| stories.get(index + 1))
                .copied()
                .map(TraceSelection::Story),
            Self::BelowCurrentStory => stories
                .iter()
                .position(|id| *id == active_story)
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| stories.get(index))
                .copied()
                .map(TraceSelection::Story),
        }
    }

    pub fn is_relative(self) -> bool {
        !matches!(self, Self::Story(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceSelection {
    Story(u32),
    EntireModel,
}

impl TraceSelection {
    pub fn story(self) -> Option<u32> {
        match self {
            Self::Story(id) => Some(id),
            Self::EntireModel => None,
        }
    }

    pub fn story_ids(self, project: &Project) -> Vec<u32> {
        match self {
            Self::Story(id) => vec![id],
            Self::EntireModel => ordered_story_ids(project),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceVisibility {
    pub walls: bool,
    pub slabs: bool,
    pub stairs: bool,
    pub polylines: bool,
}

impl Default for TraceVisibility {
    fn default() -> Self {
        Self {
            walls: true,
            slabs: true,
            stairs: true,
            polylines: true,
        }
    }
}

impl TraceVisibility {
    pub fn shows(self, element: &Element) -> bool {
        match element.shape {
            Shape::Wall { .. } => self.walls,
            Shape::Slab { .. } => self.slabs,
            Shape::Stair { .. } => self.stairs,
            Shape::Polyline { .. } => self.polylines,
            Shape::Opening { .. } => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraceState {
    pub enabled: bool,
    pub reference: TraceReference,
    pub reference_opacity: u8,
    pub active_opacity: u8,
    pub reference_color: Option<[u8; 3]>,
    pub on_top: bool,
    pub transparent_fills: bool,
    pub offset: Point,
    pub rotation_deg: f32,
    pub visibility: TraceVisibility,
    pub palette_open: bool,
    pub browse_open: bool,
    pub browse_choice: TraceReference,
    pub drag_mode: bool,
    pub rotate_mode: bool,
    pub displace_mode: bool,
    pub displace_origin: Option<Point>,
    pub splitter: Option<f32>,
}

impl Default for TraceState {
    fn default() -> Self {
        Self {
            enabled: false,
            reference: TraceReference::default(),
            reference_opacity: 120,
            active_opacity: 255,
            reference_color: Some([116, 137, 145]),
            on_top: false,
            transparent_fills: false,
            offset: Point::default(),
            rotation_deg: 0.,
            visibility: TraceVisibility::default(),
            palette_open: false,
            browse_open: false,
            browse_choice: TraceReference::default(),
            drag_mode: false,
            rotate_mode: false,
            displace_mode: false,
            displace_origin: None,
            splitter: None,
        }
    }
}

impl TraceState {
    pub fn selection(
        &self,
        project: &Project,
        active_story: u32,
        previous_story: u32,
    ) -> Option<TraceSelection> {
        self.reference
            .resolve(project, active_story, previous_story)
    }

    pub fn pointer_mode_active(&self) -> bool {
        self.drag_mode || self.rotate_mode || self.displace_mode
    }

    pub fn cancel_pointer_modes(&mut self) -> bool {
        let active = self.pointer_mode_active();
        if let Some(origin) = self.displace_origin.take() {
            self.offset = origin;
        }
        self.drag_mode = false;
        self.rotate_mode = false;
        self.displace_mode = false;
        active
    }

    pub fn reset_position(&mut self) {
        self.offset = Point::default();
        self.rotation_deg = 0.;
        self.displace_origin = None;
    }

    pub fn transform(&self, point: Point, pivot: Point) -> Point {
        let radians = self.rotation_deg.to_radians();
        let (sin, cos) = radians.sin_cos();
        let x = point.x - pivot.x;
        let y = point.y - pivot.y;
        Point::new(
            pivot.x + x * cos - y * sin + self.offset.x,
            pivot.y + x * sin + y * cos + self.offset.y,
        )
    }

    pub fn color_for(&self, material: Material) -> [u8; 3] {
        self.reference_color.unwrap_or_else(|| {
            material
                .color()
                .map(|value| (value.clamp(0., 1.) * 255.) as u8)
        })
    }

    pub fn is_valid(&self) -> bool {
        self.offset.finite()
            && self.rotation_deg.is_finite()
            && self
                .splitter
                .is_none_or(|ratio| ratio.is_finite() && (0.05..=0.95).contains(&ratio))
    }
}

pub fn ordered_story_ids(project: &Project) -> Vec<u32> {
    let mut stories: Vec<_> = project
        .stories
        .iter()
        .map(|story| (story.id, story.elevation))
        .collect();
    stories.sort_by(|(left_id, left_elevation), (right_id, right_elevation)| {
        left_elevation
            .partial_cmp(right_elevation)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left_id.cmp(right_id))
    });
    stories.into_iter().map(|(id, _)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        Project::default()
    }

    #[test]
    fn relative_references_resolve_adjacent_and_previous_stories() {
        let project = project();
        assert_eq!(
            TraceReference::AboveCurrentStory.resolve(&project, 0, 0),
            Some(TraceSelection::Story(1))
        );
        assert_eq!(
            TraceReference::BelowCurrentStory.resolve(&project, 1, 1),
            Some(TraceSelection::Story(0))
        );
        assert_eq!(
            TraceReference::AboveCurrentStory.resolve(&project, 2, 2),
            None
        );
        assert_eq!(
            TraceReference::BelowCurrentStory.resolve(&project, 0, 0),
            None
        );
        assert_eq!(
            TraceReference::PreviousStory.resolve(&project, 2, 0),
            Some(TraceSelection::Story(0))
        );
        assert_eq!(TraceReference::PreviousStory.resolve(&project, 0, 0), None);
    }

    #[test]
    fn ordered_stories_follow_elevation_not_array_order() {
        let mut project = project();
        project.stories.swap(0, 2);
        assert_eq!(ordered_story_ids(&project), vec![0, 1, 2]);
        assert_eq!(
            TraceReference::AboveCurrentStory.resolve(&project, 0, 0),
            Some(TraceSelection::Story(1))
        );
    }

    #[test]
    fn entire_model_and_absolute_story_resolution_are_explicit() {
        let project = project();
        assert_eq!(
            TraceReference::EntireModel.resolve(&project, 0, 0),
            Some(TraceSelection::EntireModel)
        );
        assert_eq!(
            TraceReference::Story(1).resolve(&project, 0, 0),
            Some(TraceSelection::Story(1))
        );
        assert_eq!(TraceReference::Story(99).resolve(&project, 0, 0), None);
        assert_eq!(
            TraceSelection::EntireModel.story_ids(&project),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn transform_rotates_about_pivot_then_offsets() {
        let mut trace = TraceState {
            rotation_deg: 90.,
            offset: Point::new(2., -1.),
            ..Default::default()
        };
        let transformed = trace.transform(Point::new(2., 0.), Point::new(1., 0.));
        assert!((transformed.x - 3.).abs() < 1e-5);
        assert!((transformed.y + 0.).abs() < 1e-5);
        trace.reset_position();
        assert_eq!(trace.offset, Point::default());
        assert_eq!(trace.rotation_deg, 0.);
    }

    #[test]
    fn cancelling_pointer_modes_restores_temporary_displacement() {
        let mut trace = TraceState {
            offset: Point::new(3., 4.),
            drag_mode: true,
            displace_mode: true,
            displace_origin: Some(Point::new(1., 2.)),
            ..Default::default()
        };
        assert!(trace.pointer_mode_active());
        assert!(trace.cancel_pointer_modes());
        assert!(!trace.pointer_mode_active());
        assert_eq!(trace.offset, Point::new(1., 2.));
    }
}
