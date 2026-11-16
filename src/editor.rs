use crate::{
    command::{CommandRecord, ElementEdit, ModelCommand, ProjectDelta, ProjectHeader},
    geometry,
    layers::{ARCHICAD_LAYER, ElementLayer, LayerDocument},
    model::*,
    view_settings::RenovationStatus,
    wall::{WallGeometryMethod, WallSettings},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    ConceptMass,
    Zone,
    Roof,
    Beam,
    Column,
    Select,
    Marquee,
    Wall,
    Slab,
    Stair,
    Polyline,
    Door,
    Window,
}

fn polygon_signed_area(points: &[Point]) -> f64 {
    geometry::polygon_area(points)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarqueeScope {
    CurrentStory,
    AllStories,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarqueeArea {
    pub vertices: Vec<Point>,
    pub scope: MarqueeScope,
    pub story: u32,
    /// Optional vertical limits used by a 3D-window marquee volume.
    pub z_range: Option<(f64, f64)>,
}

impl MarqueeArea {
    pub fn new(vertices: Vec<Point>, scope: MarqueeScope, story: u32) -> Option<Self> {
        if vertices.len() < 3 || vertices.iter().any(|p| !p.finite()) {
            return None;
        }
        if crate::geometry_validation::validate_clip_context(Some(&vertices), None).is_err() {
            return None;
        }
        let area = geometry::polygon_area(&vertices) * 2.;
        (area.abs() > 1e-5).then_some(Self {
            vertices,
            scope,
            story,
            z_range: None,
        })
    }

    pub fn includes_story(&self, story: u32) -> bool {
        self.scope == MarqueeScope::AllStories || story == self.story
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarqueeGeometry {
    #[default]
    Rectangle,
    RotatedRectangle,
    Polygon,
}

/// A viewport's allowed set is transient selection state, never project data.
/// Bind it to the raw selection as well as the document: legacy entry points
/// can still replace `selected`/`selected_ids` directly without inheriting it.
struct SelectionScope {
    document_session: u64,
    worksheet: Option<u64>,
    selected: Option<u64>,
    selected_ids: Vec<u64>,
    allowed: std::collections::BTreeSet<u64>,
}

pub struct Editor {
    pub(crate) derivation_cache: std::cell::RefCell<crate::derivation_cache::DerivationCache>,
    pub(crate) model_service: crate::model_service::SessionState,
    pub last_edit: Option<crate::editing::EditCommand>,
    pub project: Project,
    pub selected: Option<u64>,
    pub selected_ids: Vec<u64>,
    selection_scope: Option<SelectionScope>,
    pub clipboard: Vec<Element>,
    pub clipboard_groups: Vec<Vec<u64>>,
    pub story: u32,
    pub previous_story: u32,
    pub tool: Tool,
    previous_tool: Tool,
    pub marquee: Option<MarqueeArea>,
    pub anchor: Option<Point>,
    pub polyline_points: Vec<Point>,
    /// Wall nodes accumulated by the Continuous and Polygonal geometry
    /// methods. Single and Rectangular keep only `anchor` until they commit.
    pub wall_points: Vec<Point>,
    pub slab_method: crate::slab_creation::SlabGeometryMethod,
    pub slab_draft: Option<crate::slab_creation::SlabDraft>,
    pub(crate) slab_polygon_draft: Option<crate::slab_creation::SlabPolygonDraft>,
    pub zone_name: String,
    pub zone_purpose: String,
    pub zone_height: f64,
    pub concept_mass_height: f64,
    pub concept_mass_bottom_offset: f64,
    pub zone_polygon: bool,
    pub slab_layer: ElementLayer,
    /// Creation default: slab top relative to its home story, in metres.
    pub slab_elevation: f64,
    pub roof_method: crate::roof::RoofGeometryMethod,
    pub roof_draft: Option<crate::roof::RoofDraft>,
    pub(crate) roof_polygon_draft: Option<crate::roof_creation::RoofPolygonDraft>,
    pub roof_size: [f64; 3],
    pub roof_material: Material,
    pub roof_layer: ElementLayer,
    pub beam_method: crate::beam_creation::BeamGeometryMethod,
    pub beam_arc_draft: Option<crate::beam_creation::BeamArcDraft>,
    pub beam_size: [f64; 3],
    pub beam_slant_angle: f64,
    pub beam_material: Material,
    pub beam_layer: ElementLayer,
    pub column_section: crate::column::ColumnSection,
    pub column_height: f64,
    pub column_bottom_offset: f64,
    pub column_axis_rotation: f64,
    pub column_two_click: bool,
    pub column_slanted: bool,
    pub column_inclination: crate::column_inclination::ColumnInclination,
    pub column_plan: crate::column_plan::ColumnPlanSettings,
    pub column_material: Material,
    pub column_layer: ElementLayer,
    pub thickness: f64,
    pub height: f64,
    pub stair_spec: crate::stairs::StairSpec,
    pub door_defaults: OpeningDefaults,
    pub opening_joinery: crate::opening_joinery::OpeningJoinery,
    pub window_defaults: OpeningDefaults,
    pub wall_settings: WallSettings,
    pub wall_layer: ElementLayer,
    pub wall_renovation: RenovationStatus,
    pub material: Material,
    pub snap: bool,
    pub orthogonal: bool,
    pub undo: Vec<CommandRecord>,
    pub redo: Vec<CommandRecord>,
    /// Parameters picked up with the AC29-style eyedropper, per element type.
    pub pick_up_buffer: crate::transfer::PickUpBuffer,
    pub saved: Project,
    /// Monotonic model revision, incremented by commits and undo/redo.
    pub revision: u64,
    /// True after restoring a crash-recovery snapshot that was never saved.
    pub recovered_unsaved: bool,
    pub status: String,
    /// Diagnostic for the last rejected proposal, never a second Project.
    pub(crate) last_validation_diagnostic: Option<crate::derived_geometry::GeometryDiagnostic>,
}
impl Editor {
    fn restore_worksheet_scope(&mut self, scope: Option<u64>) {
        let scope = scope.filter(|id| self.project.drawings.worksheet_exists(*id));
        self.project.plan_options.active_worksheet = scope;
        self.saved.plan_options.active_worksheet = scope;
    }

    pub fn switch_worksheet(&mut self, scope: Option<u64>) {
        if scope.is_some_and(|id| !self.project.drawings.worksheet_exists(id)) {
            return;
        }
        self.restore_worksheet_scope(scope);
        self.beam_arc_draft = None;
        self.anchor = None;
        self.clear_roof_creation();
        self.clear_slab_creation();
        self.polyline_points.clear();
        self.wall_points.clear();
        self.selected = None;
        self.selected_ids.clear();
        self.selection_scope = None;
        self.marquee = None;
        if scope.is_some() {
            self.set_tool(Tool::Polyline);
        }
    }
    pub fn new(project: Project) -> Self {
        let story = project
            .trace
            .active_story
            .filter(|id| project.stories.iter().any(|s| s.id == *id))
            .unwrap_or_else(|| project.stories.first().map_or(0, |s| s.id));
        let previous_story = project
            .trace
            .previous_story
            .filter(|id| project.stories.iter().any(|s| s.id == *id))
            .unwrap_or(story);
        Self {
            last_edit: None,
            saved: project.clone(),
            project,
            selected: None,
            selected_ids: Vec::new(),
            selection_scope: None,
            clipboard: Vec::new(),
            clipboard_groups: Vec::new(),
            story,
            previous_story,
            zone_name: String::new(),
            zone_purpose: "general".into(),
            zone_height: 3.,
            concept_mass_height: 3.,
            concept_mass_bottom_offset: 0.,
            zone_polygon: false,
            tool: Tool::Select,
            previous_tool: Tool::Select,
            marquee: None,
            anchor: None,
            polyline_points: Vec::new(),
            wall_points: Vec::new(),
            slab_method: Default::default(),
            slab_draft: None,
            slab_polygon_draft: None,
            slab_layer: ElementLayer(ARCHICAD_LAYER),
            slab_elevation: 0.,
            roof_method: Default::default(),
            roof_draft: None,
            roof_polygon_draft: None,
            roof_size: [20., 0.2, 3.],
            roof_material: Material::Concrete,
            roof_layer: ElementLayer(ARCHICAD_LAYER),
            beam_method: Default::default(),
            beam_arc_draft: None,
            beam_size: [0.3, 0.5, 3.],
            beam_slant_angle: 0.,
            beam_material: Material::Concrete,
            beam_layer: ElementLayer(ARCHICAD_LAYER),
            column_section: crate::column::ColumnSection::Rectangular {
                width: 0.4,
                depth: 0.4,
            },
            column_height: 3.,
            column_bottom_offset: 0.,
            column_axis_rotation: 0.,
            column_two_click: false,
            column_slanted: false,
            column_inclination: crate::column_inclination::ColumnInclination {
                slant_degrees: 90.,
                direction_degrees: 0.,
            },
            column_plan: Default::default(),
            column_material: Material::Concrete,
            column_layer: ElementLayer(ARCHICAD_LAYER),
            thickness: 0.22,
            height: 3.,
            stair_spec: crate::stairs::StairSpec::default(),
            door_defaults: OpeningKind::Door.defaults(),
            opening_joinery: Default::default(),
            window_defaults: OpeningKind::Window.defaults(),
            wall_settings: WallSettings::default(),
            wall_layer: ElementLayer(ARCHICAD_LAYER),
            wall_renovation: RenovationStatus::New,
            material: Material::Plaster,
            snap: true,
            orthogonal: false,
            undo: vec![],
            redo: vec![],
            pick_up_buffer: crate::transfer::PickUpBuffer::default(),
            revision: 0,
            model_service: crate::model_service::SessionState::default(),
            derivation_cache: Default::default(),
            recovered_unsaved: false,
            status: "Select an element or start drawing".into(),
            last_validation_diagnostic: None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.recovered_unsaved || self.saved != self.project
    }

    /// Mark the current project as the saved baseline.
    pub fn mark_saved(&mut self) {
        self.saved = self.project.clone();
        self.recovered_unsaved = false;
    }

    /// Replace the editor with a recovered snapshot that remains dirty until saved.
    pub fn restore_unsaved(&mut self, project: Project) {
        *self = Self::new(project);
        self.recovered_unsaved = true;
        self.status = "Recovered unsaved project".into();
    }
    pub fn commit(&mut self, before: Project) {
        self.commit_checked(before);
    }

    /// Preserve legacy commit semantics while reporting rejection to UI callers.
    pub fn commit_checked(&mut self, before: Project) -> bool {
        self.commit_checked_admitted(before, false, false)
    }
    /// Only opaque crate-private recovery snapshots can use this admission.
    /// All ordinary candidate and legacy writes retain their original guard.
    pub(crate) fn commit_recovery_checked(&mut self, before: Project) -> bool {
        self.commit_checked_admitted(before, true, false)
    }
    /// Only the shared service's opaque, already validated PreparedChange can
    /// carry typed mass-space creation through the final legacy snapshot guard.
    pub(crate) fn commit_mass_space_checked(&mut self, before: Project) -> bool {
        self.commit_checked_admitted(before, false, true)
    }
    fn commit_checked_admitted(
        &mut self,
        before: Project,
        recovery: bool,
        mass_space_creation: bool,
    ) -> bool {
        self.last_validation_diagnostic = None;
        let scope_check = if recovery {
            self.project
                .design_process
                .assert_append_only_from(&before.design_process)
        } else {
            crate::design_process::service::guard_transition(
                &before,
                &self.project,
                mass_space_creation,
            )
        };
        if let Err(error) = scope_check {
            self.project = before;
            self.status = error;
            return false;
        }
        // Final guard for legacy pointer/property paths which edit a draft directly.
        if before.elements.iter().any(|old| {
            (before.edit_state.locked.contains(&old.id)
                || before.layers.is_locked(old.layer.0)
                || before.drawings.reference_owner(old.id).is_some())
                && self.project.elements.iter().find(|e| e.id == old.id) != Some(old)
        }) {
            self.project = before;
            self.status = "Selection contains locked elements".into();
            return false;
        }
        // Legacy creation paths must use the same placement rules as typed
        // commands. Check against the baseline before recording any history.
        let existing: std::collections::HashSet<u64> =
            before.elements.iter().map(|element| element.id).collect();
        for element in &self.project.elements {
            if !existing.contains(&element.id)
                && let Err(error) = Self::validate_placement(&before, element)
            {
                self.project = before;
                self.status = error;
                return false;
            }
        }
        if before != self.project {
            // Legacy callers edit a candidate in place. Validate before creating
            // an ID-based delta: invalid duplicate IDs cannot be rolled back by
            // a delta reliably, so restore the caller's complete baseline.
            if let Some(d) = self
                .validate_geometry_snapshot(&self.project)
                .first_error()
                .cloned()
            {
                self.status = crate::geometry_validation::compatibility_message(&d);
                self.last_validation_diagnostic = Some(d);
                self.project = before;
                return false;
            }
            self.revision = self.revision.wrapping_add(1);
            let delta = ProjectDelta::between(&before, &self.project);
            self.undo.push(CommandRecord::from_delta(delta));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        true
    }

    /// Build a fully validated creation batch without changing editor state.
    /// Explicit identities and host references have exactly the same meaning as AddElements.
    pub fn preview_add_elements(
        &self,
        elements: Vec<Element>,
        group: bool,
    ) -> Result<Project, String> {
        self.preview_add_elements_diagnostic(elements, group)
            .map_err(|d| crate::geometry_validation::compatibility_message(&d))
    }
    fn preview_add_elements_diagnostic(
        &self,
        elements: Vec<Element>,
        group: bool,
    ) -> Result<Project, crate::derived_geometry::GeometryDiagnostic> {
        for element in &elements {
            Self::validate_placement(&self.project, element).map_err(|m| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    element.id,
                    "PLACEMENT_REJECTED",
                    "placement",
                    m,
                )
            })?;
        }
        if elements.is_empty() {
            return Ok(self.project.clone());
        }
        let mut candidate = self.project.clone();
        let mut seen: std::collections::HashSet<_> =
            candidate.elements.iter().map(|e| e.id).collect();
        let mut ids = Vec::with_capacity(elements.len());
        for element in elements {
            if !seen.insert(element.id) {
                return Err(crate::derived_geometry::GeometryDiagnostic::error(
                    element.id,
                    "DUPLICATE_ELEMENT_ID",
                    "identity",
                    format!("Element {} already exists", element.id),
                ));
            }
            let next = element.id.checked_add(1).ok_or_else(|| {
                crate::derived_geometry::GeometryDiagnostic::error(
                    element.id,
                    "INVALID_ELEMENT_IDENTITY",
                    "identity",
                    "Invalid element identity or story",
                )
            })?;
            candidate.next_id = candidate.next_id.max(next);
            ids.push(element.id);
            candidate.elements.push(element);
        }
        if group && ids.len() > 1 {
            candidate.edit_state.groups.insert(0, ids.clone());
        }
        if let Some(d) = self
            .validate_geometry_snapshot(&candidate)
            .first_error()
            .cloned()
        {
            return Err(d);
        }
        Ok(candidate)
    }

    /// Apply one typed model command, validate it and record a reversible
    /// change. Returns the affected element ids; an empty list means the
    /// command was a no-op. Invalid commands leave the project untouched.
    pub fn apply(&mut self, command: ModelCommand) -> Result<Vec<u64>, String> {
        self.last_validation_diagnostic = None;
        let design_scope = crate::design_process::service::context_scope(&self.project);
        // AC29 Layer status: locked layers reject edits, deletions and new
        // element placement (help 025_Attributes-5).
        match &command {
            ModelCommand::SetStoryElevation { story, elevation } => {
                let current = self
                    .project
                    .stories
                    .iter()
                    .find(|s| s.id == *story)
                    .ok_or("Invalid story")?;
                if !elevation.is_finite() || elevation.abs() > 10000. {
                    return Err("Invalid story".into());
                }
                if current.elevation != *elevation {
                    if let Some(element) = self
                        .project
                        .elements
                        .iter()
                        .find(|e| e.story == *story && self.locked_element(e.id))
                    {
                        return Err(self.locked_element_error(element.id));
                    }
                }
            }
            ModelCommand::AddElement { element } => {
                Self::validate_placement(&self.project, element)?;
            }
            ModelCommand::RemoveElements { ids } => {
                if let Some(id) = ids.iter().find(|id| self.locked_element(**id)) {
                    return Err(self.locked_element_error(*id));
                }
            }
            ModelCommand::UpdateElement { element } => {
                if self.locked_element(element.id) {
                    return Err(self.locked_element_error(element.id));
                }
            }
            ModelCommand::MoveElements { ids, .. } => {
                if let Some(id) = ids.iter().find(|id| self.locked_element(**id)) {
                    return Err(self.locked_element_error(*id));
                }
            }
            ModelCommand::RestoreElements { elements } => {
                // Executing a restore request places elements just like AddElement.
                // History undo/redo uses deltas directly and does not pass here.
                for (_, element) in elements {
                    Self::validate_placement(&self.project, element)?;
                }
            }
            _ => {}
        }
        let record_command = command.clone();
        let mut delta = ProjectDelta::default();
        let inverse;
        match command {
            ModelCommand::MassSpaces { edit } => {
                delta = crate::mass_spaces::apply(&mut self.project, &edit)?;
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::DesignProcess { edit } => {
                let (journal, locks) = crate::design_process::apply(&self.project, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                self.project.design_process = journal;
                if let Some(locks) = locks {
                    self.project.edit_state = locks;
                }
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::SetStoryElevation { story, elevation } => {
                let before = ProjectHeader::capture(&self.project);
                let current = self
                    .project
                    .stories
                    .iter_mut()
                    .find(|s| s.id == story)
                    .ok_or("Invalid story")?;
                inverse = ModelCommand::SetStoryElevation {
                    story,
                    elevation: current.elevation,
                };
                current.elevation = elevation;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
            }
            ModelCommand::External(edit) => {
                let before = self.project.clone();
                self.project = crate::external::apply(&before, edit)?;
                delta = ProjectDelta::between(&before, &self.project);
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::AddElement { element } => {
                if self
                    .project
                    .elements
                    .iter()
                    .any(|existing| existing.id == element.id)
                {
                    return Err(format!("Element {} already exists", element.id));
                }
                let header_before = ProjectHeader::capture(&self.project);
                let index = self.project.elements.len();
                let next_id = element
                    .id
                    .checked_add(1)
                    .ok_or_else(|| "Invalid element identity or story".to_string())?;
                self.project.next_id = self.project.next_id.max(next_id);
                self.project.elements.push(element.clone());
                delta.elements.push(ElementEdit {
                    id: element.id,
                    before_index: None,
                    before: None,
                    after_index: Some(index),
                    after: Some(element.clone()),
                });
                if self.project.next_id != header_before.next_id {
                    delta.before_header = Some(Box::new(header_before));
                    delta.after_header = Some(Box::new(ProjectHeader::capture(&self.project)));
                }
                inverse = ModelCommand::RemoveElements {
                    ids: vec![element.id],
                };
            }
            ModelCommand::AddElements { elements, group } => {
                if elements.is_empty() {
                    return Ok(Vec::new());
                }
                let ids = elements.iter().map(|element| element.id).collect();
                let candidate = self
                    .preview_add_elements_diagnostic(elements, group)
                    .map_err(|d| {
                        let message = crate::geometry_validation::compatibility_message(&d);
                        self.last_validation_diagnostic = Some(d);
                        message
                    })?;
                delta = ProjectDelta::between(&self.project, &candidate);
                self.project = candidate;
                inverse = ModelCommand::RemoveElements { ids };
            }
            ModelCommand::RemoveElements { ids } => {
                let mut requested: std::collections::HashSet<u64> = ids.into_iter().collect();
                if requested.is_empty() {
                    return Ok(Vec::new());
                }
                for element in &self.project.elements {
                    if let Shape::Opening { host_wall, .. } = element.shape
                        && requested.contains(&host_wall)
                    {
                        requested.insert(element.id);
                    }
                }
                if requested.iter().any(|id| self.locked_element(*id)) {
                    return Err("Selection contains locked elements".into());
                }
                let mut removed: Vec<(usize, Element)> = self
                    .project
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, element)| requested.contains(&element.id))
                    .map(|(index, element)| (index, element.clone()))
                    .collect();
                removed.sort_by_key(|(index, _)| *index);
                if removed.is_empty() {
                    return Ok(Vec::new());
                }
                self.project
                    .elements
                    .retain(|element| !requested.contains(&element.id));
                delta.elements = removed
                    .iter()
                    .map(|(index, element)| ElementEdit {
                        id: element.id,
                        before_index: Some(*index),
                        before: Some(element.clone()),
                        after_index: None,
                        after: None,
                    })
                    .collect();
                inverse = ModelCommand::RestoreElements { elements: removed };
            }
            ModelCommand::RestoreElements { elements } => {
                let mut inserts = elements;
                inserts.sort_by_key(|(index, _)| *index);
                let restored: Vec<u64> = inserts.iter().map(|(_, element)| element.id).collect();
                let existing: std::collections::HashSet<u64> = self
                    .project
                    .elements
                    .iter()
                    .map(|element| element.id)
                    .collect();
                for (_, element) in &inserts {
                    if existing.contains(&element.id) {
                        return Err(format!("Element {} already exists", element.id));
                    }
                }
                for (index, element) in inserts {
                    delta.elements.push(ElementEdit {
                        id: element.id,
                        before_index: None,
                        before: None,
                        after_index: Some(index),
                        after: Some(element.clone()),
                    });
                    let index = index.min(self.project.elements.len());
                    self.project.elements.insert(index, element);
                }
                inverse = ModelCommand::RemoveElements { ids: restored };
            }
            ModelCommand::UpdateElement { element } => {
                let Some(index) = self
                    .project
                    .elements
                    .iter()
                    .position(|existing| existing.id == element.id)
                else {
                    return Err(format!("Element {} no longer exists", element.id));
                };
                let previous =
                    std::mem::replace(&mut self.project.elements[index], element.clone());
                if previous != element {
                    delta.elements.push(ElementEdit {
                        id: element.id,
                        before_index: Some(index),
                        before: Some(previous.clone()),
                        after_index: Some(index),
                        after: Some(element),
                    });
                }
                inverse = ModelCommand::UpdateElement { element: previous };
            }
            ModelCommand::MoveElements { ids, offset } => {
                if !offset.finite() {
                    return Err("Move offset must be finite".into());
                }
                if ids.is_empty() {
                    return Ok(Vec::new());
                }
                let requested: std::collections::HashSet<u64> = ids.iter().copied().collect();
                let moved: Vec<(usize, Element)> = self
                    .project
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, element)| requested.contains(&element.id))
                    .map(|(index, element)| (index, element.clone()))
                    .collect();
                if moved.is_empty() {
                    return Ok(Vec::new());
                }
                let moved_ids: Vec<u64> = moved.iter().map(|(_, element)| element.id).collect();
                self.project.translate_many(&moved_ids, offset);
                for (index, previous) in moved {
                    let after = self.project.elements[index].clone();
                    if after != previous {
                        delta.elements.push(ElementEdit {
                            id: previous.id,
                            before_index: Some(index),
                            before: Some(previous),
                            after_index: Some(index),
                            after: Some(after),
                        });
                    }
                }
                inverse = ModelCommand::MoveElements {
                    ids: ids.clone(),
                    offset: Point::new(-offset.x, -offset.y),
                };
            }
            ModelCommand::SetOptions {
                plan,
                design,
                guide,
                grid,
            } => {
                let header_before = ProjectHeader::capture(&self.project);
                inverse = ModelCommand::SetOptions {
                    plan: plan.as_ref().map(|_| self.project.plan_options.clone()),
                    design: design.as_ref().map(|_| self.project.design_options.clone()),
                    guide: guide.as_ref().map(|_| self.project.guide_settings.clone()),
                    grid: grid.as_ref().map(|_| self.project.grid_settings.clone()),
                };
                if let Some(mut plan) = plan {
                    plan.active_worksheet = self.project.plan_options.active_worksheet;
                    self.project.plan_options = plan;
                }
                if let Some(design) = design {
                    self.project.design_options = design;
                }
                if let Some(guide) = guide {
                    self.project.guide_settings = guide;
                }
                if let Some(grid) = grid {
                    self.project.grid_settings = grid;
                }
                let header_after = ProjectHeader::capture(&self.project);
                if header_before != header_after {
                    delta.before_header = Some(Box::new(header_before));
                    delta.after_header = Some(Box::new(header_after));
                }
            }
            ModelCommand::Terrain { edit } => {
                let terrain = crate::site::terrain::apply(&self.project.site, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                self.project.site.terrain = terrain;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::Constraint { edit } => {
                let checks = crate::constraints::apply(&self.project, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                self.project.rules.checks = checks;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::Rules { edit } => {
                let rules = crate::rules::apply(&self.project.rules, &edit)?;
                rules.validate_edit_against_project(&self.project, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                self.project.rules = rules;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::Evidence { edit } => {
                let evidence = crate::evidence::apply(&self.project, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                self.project.evidence = evidence;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::Program { edit } => {
                let program = crate::program::apply(&self.project.program, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                self.project.program = program;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::ApplySiteBoundary { proposal } => {
                let before = self.project.clone();
                self.project = crate::site::boundary::apply(&before, &proposal)?;
                delta = ProjectDelta::between(&before, &self.project);
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::DetachSiteBoundary => {
                let before = ProjectHeader::capture(&self.project);
                self.project.site.boundary = None;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::ApplySiteControlRegion { proposal } => {
                let before = self.project.clone();
                self.project = crate::site::boundary::apply_for(
                    &before,
                    &proposal,
                    crate::site::boundary::Role::ControlRegion,
                )?;
                delta = ProjectDelta::between(&before, &self.project);
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::DetachSiteControlRegion => {
                let before = ProjectHeader::capture(&self.project);
                self.project.site.control_region = None;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
                inverse = ModelCommand::ChangeSet;
            }
            ModelCommand::SetSiteContext { context } => {
                if context.terrain != self.project.site.terrain {
                    return Err(
                        "Use the terrain command to change terrain geometry or provenance".into(),
                    );
                }
                if context.boundary != self.project.site.boundary
                    || context.control_region != self.project.site.control_region
                {
                    return Err(
                        "Use the boundary command to change site boundary provenance".into(),
                    );
                }

                context.validate()?;
                let before = ProjectHeader::capture(&self.project);
                inverse = ModelCommand::SetSiteContext {
                    context: self.project.site.clone(),
                };
                self.project.site = context;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
            }
            ModelCommand::Diagram { edit } => {
                let document = crate::diagrams::apply(&self.project, &edit)?;
                let before = ProjectHeader::capture(&self.project);
                inverse = ModelCommand::SetDrawings {
                    document: self.project.drawings.clone(),
                };
                self.project.drawings = document;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
            }
            ModelCommand::SetDrawings { document } => {
                let before = ProjectHeader::capture(&self.project);
                inverse = ModelCommand::SetDrawings {
                    document: self.project.drawings.clone(),
                };
                self.project.drawings = document;
                let after = ProjectHeader::capture(&self.project);
                if before != after {
                    delta.before_header = Some(Box::new(before));
                    delta.after_header = Some(Box::new(after));
                }
            }
            ModelCommand::ChangeSet => {
                return Err("ChangeSet is recorded internally and is not executable".into());
            }
        }
        if delta.is_empty() {
            return Ok(Vec::new());
        }
        if let Err(error) = crate::design_process::service::guard_delta(
            design_scope.as_ref(),
            &delta,
            matches!(&record_command, ModelCommand::MassSpaces { .. }),
        ) {
            delta.revert(&mut self.project);
            return Err(error);
        }
        if let Some(d) = self
            .validate_geometry_snapshot(&self.project)
            .first_error()
            .cloned()
        {
            let error = crate::geometry_validation::compatibility_message(&d);
            self.last_validation_diagnostic = Some(d);
            delta.revert(&mut self.project);
            self.status = error.clone();
            return Err(error);
        }
        self.revision = self.revision.wrapping_add(1);
        let mut affected = delta.affected_ids();
        if let (Some(before), Some(after)) = (&delta.before_header, &delta.after_header) {
            let stories: std::collections::HashSet<_> = before
                .stories
                .iter()
                .filter(|old| {
                    after
                        .stories
                        .iter()
                        .any(|new| new.id == old.id && new.elevation != old.elevation)
                })
                .map(|s| s.id)
                .collect();
            affected.extend(
                self.project
                    .elements
                    .iter()
                    .filter(|e| stories.contains(&e.story))
                    .map(|e| e.id),
            );
            affected.sort_unstable();
            affected.dedup();
        }
        self.undo
            .push(CommandRecord::new(record_command, inverse, delta));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(affected)
    }
    pub fn set_tool(&mut self, t: Tool) {
        let t = if self.project.plan_options.active_worksheet.is_some()
            && !matches!(t, Tool::Select | Tool::Marquee | Tool::Polyline)
        {
            Tool::Polyline
        } else {
            t
        };
        if self.tool != t {
            self.previous_tool = self.tool;
        } else if t == Tool::Select {
            self.previous_tool = Tool::Select;
        }
        self.tool = t;
        self.beam_arc_draft = None;
        self.anchor = None;
        self.clear_roof_creation();
        self.clear_slab_creation();
        self.polyline_points.clear();
        self.wall_points.clear();
        self.status = match t {
            Tool::Zone => "Space · Draw a rectangle or polygon",
            Tool::ConceptMass => "Concept mass · Draw a rectangle or polygon",
            Tool::Roof => "Roof · Click the pivot line start",
            Tool::Beam => if self.beam_method == crate::beam_creation::BeamGeometryMethod::CenterRadius { "Beam · Click the arc center" } else { "Beam · Click start and end of the top-center axis" },
            Tool::Column => if self.column_two_click { "Column · Click the base axis point" } else { "Column · Click to place a centered column" },
            Tool::Select => "Select an element · Drag to move · Delete to remove",
            Tool::Marquee => "Marquee · Drag a rectangle or choose Polygon / Rotated Rectangle",
            Tool::Wall => self.wall_settings.geometry.geometry_method.status_key(),
            Tool::Slab => self.slab_method.status_key(),
            Tool::Stair => "Stair · Set parameters, then click run start and end",
            Tool::Polyline => "Polyline · Click start, left-click nodes, right-click to finish · click start to close",
            Tool::Door => "Door · Click a wall to place a rectangular door opening",
            Tool::Window => "Window · Click a wall to place a rectangular window opening",
        }
        .into();
    }
    pub fn return_to_previous_tool(&mut self) -> Tool {
        let target = if self.previous_tool == self.tool {
            Tool::Select
        } else {
            self.previous_tool
        };
        self.set_tool(target);
        // One Escape returns to the last command; the next Escape goes to Select.
        self.previous_tool = Tool::Select;
        target
    }
    pub fn switch_story(&mut self, id: u32) {
        if self.project.stories.iter().any(|s| s.id == id) {
            self.switch_worksheet(None);
            if self.story != id {
                self.previous_story = self.story;
            }
            self.story = id;
            self.clear_slab_creation();
            self.anchor = None;
            self.clear_roof_creation();
            self.wall_points.clear();
            self.selected = None;
            self.selected_ids.clear();
        }
    }
    pub(crate) fn model_input_active(&self) -> bool {
        self.anchor.is_some()
            || !self.polyline_points.is_empty()
            || !self.wall_points.is_empty()
            || self.slab_polygon_draft.is_some()
            || self.roof_polygon_draft.is_some()
            || self.roof_draft.is_some()
            || self.slab_draft.is_some()
            || self.beam_arc_draft.is_some()
    }
    /// Build an uncommitted primitive without allocating an ID. AddElement
    /// owns checked identity advancement and atomic model validation.
    pub(crate) fn creation_candidate(
        &self,
        material: Material,
        layer: ElementLayer,
        shape: Shape,
    ) -> Element {
        let mut element = Element {
            worksheet: self.project.plan_options.active_worksheet,
            id: self.project.next_id,
            name: String::new(),
            story: self.story,
            material,
            layer,
            renovation: self.wall_renovation,
            design_option: if matches!(shape, Shape::Zone { .. } | Shape::ConceptMass { .. }) {
                crate::design_process::service::working_scope(&self.project).map(|s| s.option)
            } else {
                None
            },
            wall_settings: None,
            wall_contour: None,
            slab_contour: None,
            opening_joinery: None,
            shape,
        };
        element.name = format!("{} {:03}", element.kind(), element.id);
        element
    }
    pub fn stair_creation_candidate(
        &self,
        spec: crate::stairs::StairSpec,
    ) -> Result<Element, String> {
        spec.validate().map_err(str::to_owned)?;
        Ok(self.creation_candidate(
            self.material,
            ElementLayer(ARCHICAD_LAYER),
            Shape::Stair { spec },
        ))
    }
    pub fn place_stair(&mut self, spec: crate::stairs::StairSpec) -> Result<u64, String> {
        let element = self.stair_creation_candidate(spec)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }
    /// Build a roof candidate without allocating an ID or changing history.
    /// Placement permissions and whole-project checks belong to the model command.
    pub fn roof_creation_candidate(&self, spec: crate::roof::RoofSpec) -> Result<Element, String> {
        spec.validate()?;
        Ok(self.creation_candidate(self.roof_material, self.roof_layer, Shape::Roof { spec }))
    }

    pub fn place_roof(&mut self, spec: crate::roof::RoofSpec) -> Result<u64, String> {
        let element = self.roof_creation_candidate(spec)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }
    pub fn roof_preview(&self, point: Point) -> Option<crate::roof::RoofSpec> {
        if let Some(draft) = &self.roof_polygon_draft {
            return self.roof_polygon_candidate(draft, Some(point)).ok();
        }
        use crate::{roof::RoofDraft, slab_creation::SlabDraft};
        let (start, end, rise_left, min, max, contour) = match self.roof_draft? {
            RoofDraft::ContourEnd {
                start,
                end,
                rise_left,
                first,
            } => (
                start,
                end,
                rise_left,
                Point::new(first.x.min(point.x), first.y.min(point.y)),
                Point::new(first.x.max(point.x), first.y.max(point.y)),
                None,
            ),
            RoofDraft::RotatedContour {
                start,
                end,
                rise_left,
                rectangle:
                    SlabDraft::Width {
                        start: first,
                        end: baseline_end,
                    },
            } => {
                let contour = crate::roof_contour::RoofContour {
                    outer: crate::geometry::rotated_rectangle_polygon(first, baseline_end, point)?,
                    holes: vec![],
                };
                contour.validate().ok()?;
                let (min, max) = contour.bounds()?;
                (start, end, rise_left, min, max, Some(contour))
            }
            _ => return None,
        };
        let [pitch, thickness, pivot_offset] = self.roof_size;
        Some(crate::roof::RoofSpec {
            contour,
            pivot_start: start,
            pivot_end: end,
            rise_left,
            min,
            max,
            pitch,
            thickness,
            pivot_offset,
        })
    }
    fn click_roof(&mut self, p: Point, tolerance: f64) {
        use crate::roof::RoofDraft;
        if let Some(draft) = self.roof_polygon_draft.as_ref() {
            let closing = self.roof_method == crate::roof::RoofGeometryMethod::Polygonal
                && draft
                    .points()
                    .first()
                    .is_some_and(|first| first.distance(p) <= tolerance);
            let result = if closing {
                self.finish_roof_polygon().map(|_| ())
            } else {
                self.accept_roof_polygon_vertex(p)
            };
            if let Err(error) = result {
                self.status = error;
            }
            return;
        }
        match self.roof_draft {
            None => {
                self.roof_draft = Some(RoofDraft::PivotEnd { start: p });
                self.anchor = Some(p);
                self.status = "Roof · Click the pivot line end".into();
            }
            Some(RoofDraft::PivotEnd { start }) => {
                if !crate::precision::segment_meets_minimum(
                    start,
                    p,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                ) {
                    self.status = "Roof pivot line is too short".into();
                    return;
                }
                self.roof_draft = Some(RoofDraft::RiseSide { start, end: p });
                self.anchor = None;
                self.status = "Roof · Click the slope side".into();
            }
            Some(RoofDraft::RiseSide { start, end }) => {
                let cross =
                    (end.x - start.x) * (p.y - start.y) - (end.y - start.y) * (p.x - start.x);
                if !crate::precision::length_meets_minimum(
                    cross.abs() / start.distance(end),
                    0.01,
                    crate::precision::coordinate_scale([start, end, p]),
                ) {
                    self.status = "Choose a side away from the roof pivot line".into();
                    return;
                }
                self.roof_draft = Some(RoofDraft::ContourStart {
                    start,
                    end,
                    rise_left: cross > 0.,
                });
                if self.roof_method == crate::roof::RoofGeometryMethod::Polygonal {
                    if let Err(error) = self.begin_roof_polygon() {
                        self.status = error;
                    }
                } else {
                    self.status = "Roof · Click the first rectangle corner".into();
                }
            }
            Some(RoofDraft::ContourStart {
                start,
                end,
                rise_left,
            }) => {
                self.roof_draft = Some(
                    if self.roof_method == crate::roof::RoofGeometryMethod::RotatedRectangle {
                        RoofDraft::RotatedContour {
                            start,
                            end,
                            rise_left,
                            rectangle: crate::slab_creation::SlabDraft::BaselineEnd { start: p },
                        }
                    } else {
                        RoofDraft::ContourEnd {
                            start,
                            end,
                            rise_left,
                            first: p,
                        }
                    },
                );
                self.anchor = Some(p);
                self.status =
                    if self.roof_method == crate::roof::RoofGeometryMethod::RotatedRectangle {
                        "Roof · Click the rotation baseline end"
                    } else {
                        "Roof · Click the opposite rectangle corner"
                    }
                    .into();
            }
            Some(RoofDraft::RotatedContour {
                start,
                end,
                rise_left,
                rectangle: crate::slab_creation::SlabDraft::BaselineEnd { start: first },
            }) => {
                if first.distance(p) < 0.01 {
                    self.status = "Invalid roof geometry or parameters".into();
                    return;
                }
                self.roof_draft = Some(RoofDraft::RotatedContour {
                    start,
                    end,
                    rise_left,
                    rectangle: crate::slab_creation::SlabDraft::Width {
                        start: first,
                        end: p,
                    },
                });
                self.status = "Roof · Click the perpendicular width".into();
            }
            Some(RoofDraft::ContourEnd { .. } | RoofDraft::RotatedContour { .. }) => {
                let Some(spec) = self.roof_preview(p) else {
                    self.status = "Invalid roof geometry or parameters".into();
                    return;
                };
                match self.place_roof(spec) {
                    Ok(id) => {
                        self.set_selection(vec![id]);
                        self.clear_roof_creation();
                        self.anchor = None;
                        self.status = "Roof placed".into();
                    }
                    Err(error) => self.status = error,
                }
            }
        }
    }
    pub fn set_beam_method(&mut self, method: crate::beam_creation::BeamGeometryMethod) {
        if self.beam_method == method {
            return;
        }
        self.beam_method = method;
        let center = self
            .beam_arc_draft
            .map(|draft| draft.center())
            .or(self.anchor);
        self.anchor = center;
        self.beam_arc_draft = if method == crate::beam_creation::BeamGeometryMethod::CenterRadius {
            center.and_then(|point| crate::beam_creation::BeamArcDraft::new(point).ok())
        } else {
            None
        };
        self.refresh_beam_status();
    }
    pub fn refresh_beam_status(&mut self) {
        self.status = match self.beam_arc_draft {
            Some(draft) if draft.start().is_some() => "Beam · Set the arc sweep",
            Some(_) => "Beam · Set the radius",
            None if self.beam_method == crate::beam_creation::BeamGeometryMethod::CenterRadius => {
                "Beam · Click the arc center"
            }
            None if self.anchor.is_some() => "Beam · Click the endpoint",
            None => "Beam · Click start and end of the top-center axis",
        }
        .into();
    }

    /// Advance the center/radius/end session. UI callers supply accepted model
    /// coordinates; only the completed candidate enters the shared command path.
    pub fn click_beam_arc(&mut self, p: Point) -> Result<Option<u64>, String> {
        if self.tool != Tool::Beam || self.project.plan_options.active_worksheet.is_some() {
            return Err("Invalid beam geometry or parameters".into());
        }
        let Some(mut draft) = self.beam_arc_draft else {
            self.beam_arc_draft = Some(crate::beam_creation::BeamArcDraft::new(p)?);
            self.anchor = Some(p);
            self.refresh_beam_status();
            return Ok(None);
        };
        if draft.start().is_none() {
            draft.accept_radius(p)?;
            self.beam_arc_draft = Some(draft);
            self.refresh_beam_status();
            return Ok(None);
        }
        draft.update_cursor(p)?;
        self.beam_arc_draft = Some(draft);
        self.commit_beam_arc().map(Some)
    }

    /// Return the current candidate using the same creation defaults as commit.
    /// No ID, history entry or project state is allocated by a preview.
    pub fn beam_arc_preview(&self) -> Option<Element> {
        let candidate = self.beam_arc_draft?.candidate()?;
        self.beam_path_preview(candidate.start, candidate.end, 0., Some(candidate.arc))
    }

    /// Numeric and pointer input share exactly one validated commit operation.
    pub fn commit_beam_arc(&mut self) -> Result<u64, String> {
        if self.tool != Tool::Beam || self.project.plan_options.active_worksheet.is_some() {
            return Err("Invalid beam geometry or parameters".into());
        }
        let candidate = self
            .beam_arc_draft
            .and_then(|d| d.candidate())
            .ok_or_else(|| "Invalid beam geometry or parameters".to_owned())?;
        let id = self.place_curved_beam(candidate.start, candidate.end, candidate.arc)?;
        self.set_selection(vec![id]);
        self.beam_arc_draft = None;
        self.anchor = None;
        self.status = "Beam placed".into();
        Ok(id)
    }

    pub fn place_beam(&mut self, start: Point, end: Point) -> Result<u64, String> {
        self.place_slanted_beam(start, end, self.beam_slant_angle)
    }
    pub fn place_slanted_beam(
        &mut self,
        start: Point,
        end: Point,
        slant_angle: f64,
    ) -> Result<u64, String> {
        self.place_beam_path(start, end, slant_angle, None)
    }
    pub fn place_curved_beam(
        &mut self,
        start: Point,
        end: Point,
        arc: crate::model::ArcSegment,
    ) -> Result<u64, String> {
        self.place_beam_path(start, end, 0., Some(arc))
    }
    fn place_beam_path(
        &mut self,
        start: Point,
        end: Point,
        slant_angle: f64,
        arc: Option<crate::model::ArcSegment>,
    ) -> Result<u64, String> {
        let element = self.beam_creation_candidate(start, end, slant_angle, arc);
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }
    /// Geometry-only preview; placement permissions and full-project validation
    /// remain with preview_add_elements / AddElement.
    pub fn beam_path_preview(
        &self,
        start: Point,
        end: Point,
        slant_angle: f64,
        arc: Option<ArcSegment>,
    ) -> Option<Element> {
        let element = self.beam_creation_candidate(start, end, slant_angle, arc);
        crate::beam_geometry::derive(&self.project, &element).ok()?;
        Some(element)
    }

    fn beam_creation_candidate(
        &self,
        start: Point,
        end: Point,
        slant_angle: f64,
        arc: Option<ArcSegment>,
    ) -> Element {
        let [width, height, top_offset] = self.beam_size;
        self.creation_candidate(
            self.beam_material,
            self.beam_layer,
            Shape::Beam {
                start,
                end,
                width,
                height,
                top_offset,
                slant_angle,
                arc,
            },
        )
    }

    pub fn column_creation_preview(&self, center: Point) -> Result<Element, String> {
        self.column_creation_candidate(
            center,
            self.column_slanted.then_some(self.column_inclination),
        )
    }

    /// Preview accepted base/top plan-axis points without changing creation defaults.
    pub fn column_axis_preview(&self, base: Point, top: Point) -> Result<Element, String> {
        let inclination = crate::column_inclination::ColumnInclination::from_plan_axis(
            base,
            top,
            self.column_height,
        )?;
        self.column_creation_candidate(base, Some(inclination))
    }

    fn column_creation_candidate(
        &self,
        center: Point,
        inclination: Option<crate::column_inclination::ColumnInclination>,
    ) -> Result<Element, String> {
        let element = self.creation_candidate(
            self.column_material,
            self.column_layer,
            Shape::Column {
                center,
                section: self.column_section,
                height: self.column_height,
                bottom_offset: self.column_bottom_offset,
                axis_rotation: self.column_axis_rotation,
                inclination,
                plan: self.column_plan,
            },
        );
        if inclination.is_some() {
            crate::column::inclined_solid(&self.project, &element)
                .ok_or("Invalid column dimensions or position")?
                .mesh()?;
            if let Some(plan) = crate::column_plan::for_element(&self.project, &element, self.story)
            {
                plan?;
            }
        } else {
            crate::column::solid(&self.project, &element)
                .ok_or("Invalid column dimensions or position")?
                .mesh()?;
        }
        Ok(element)
    }
    pub fn place_column(&mut self, center: Point) -> Result<u64, String> {
        let element = self.column_creation_preview(center)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    /// Commit accepted two-point geometry through the common atomic model command.
    pub fn place_column_axis(&mut self, base: Point, top: Point) -> Result<u64, String> {
        let element = self.column_axis_preview(base, top)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    pub fn place_column_section(
        &mut self,
        center: Point,
        section: crate::column::ColumnSection,
        height: f64,
    ) -> Result<u64, String> {
        self.place_column_with_inclination(center, section, height, None)
    }

    /// Typed model entry shared by future UI and AI commands; uses the same
    /// atomic AddElement validation and history as vertical column creation.
    pub fn place_column_with_inclination(
        &mut self,
        center: Point,
        section: crate::column::ColumnSection,
        height: f64,
        inclination: Option<crate::column_inclination::ColumnInclination>,
    ) -> Result<u64, String> {
        let element = self.creation_candidate(
            self.column_material,
            self.column_layer,
            Shape::Column {
                center,
                section,
                height,
                bottom_offset: self.column_bottom_offset,
                axis_rotation: self.column_axis_rotation,
                inclination,
                plan: self.column_plan,
            },
        );
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    pub fn click(&mut self, p: Point, tolerance: f64) {
        if !p.finite() || p.x.abs() >= 100000. || p.y.abs() >= 100000. {
            self.status = "Coordinates must be finite and within the local modeling range".into();
            return;
        }
        if self.tool == Tool::Marquee {
            return;
        }
        if self.tool == Tool::Polyline {
            if self.polyline_points.last().is_none_or(|last| {
                crate::precision::segment_meets_minimum(
                    *last,
                    p,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                )
            }) {
                self.polyline_points.push(p);
                self.status = format!(
                    "Polyline · {} node(s) · right-click to finish",
                    self.polyline_points.len()
                );
            }
            return;
        }
        if self.tool == Tool::ConceptMass {
            self.click_concept_mass(p, tolerance);
            return;
        }
        if self.tool == Tool::Zone {
            self.click_zone(p, tolerance);
            return;
        }
        if self.tool == Tool::Roof {
            self.click_roof(p, tolerance);
            return;
        }
        if self.tool == Tool::Beam {
            if self.beam_arc_draft.is_some()
                || self.beam_method == crate::beam_creation::BeamGeometryMethod::CenterRadius
            {
                if let Err(error) = self.click_beam_arc(p) {
                    self.status = error;
                }
                return;
            }
            if let Some(start) = self.anchor {
                match self.place_beam(start, p) {
                    Ok(id) => {
                        self.set_selection(vec![id]);
                        self.anchor = None;
                        self.clear_roof_creation();
                        self.status = "Beam placed".into();
                    }
                    Err(error) => self.status = error,
                }
            } else {
                self.anchor = Some(p);
                self.status = "Beam · Click the endpoint".into();
            }
            return;
        }
        if self.tool == Tool::Column {
            if self.column_two_click && self.anchor.is_none() {
                self.anchor = Some(p);
                self.status = "Column · Click the top axis point in plan".into();
                return;
            }
            let result = if self.column_two_click {
                self.place_column_axis(self.anchor.unwrap(), p)
            } else {
                self.place_column(p)
            };
            match result {
                Ok(id) => {
                    self.set_selection(vec![id]);
                    self.anchor = None;
                    self.status = "Column placed".into();
                }
                Err(error) => self.status = error,
            }
            return;
        }
        if self.tool == Tool::Slab {
            self.click_slab(p, tolerance);
            return;
        }
        if self.tool == Tool::Wall {
            self.click_wall(p, tolerance);
            return;
        }
        if matches!(self.tool, Tool::Door | Tool::Window) {
            let kind = if self.tool == Tool::Door {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            };
            if let Err(error) = self.click_opening(kind, p, tolerance) {
                self.status = error;
            }
            return;
        }
        match self.tool {
            Tool::Select => {
                self.selected = geometry::hit_plan(&self.project, self.story, p, tolerance);
                self.selected_ids = self.selected.into_iter().collect();
            }
            Tool::Stair => {
                if let Some(start) = self.anchor {
                    let mut spec = self.stair_spec.clone();
                    spec.start = start;
                    spec.end = p;
                    match self.place_stair(spec) {
                        Ok(id) => {
                            self.set_selection(vec![id]);
                            self.anchor = None;
                            self.clear_roof_creation();
                            self.status = format!("Created element {id:03}");
                        }
                        Err(error) => self.status = error,
                    }
                } else {
                    self.anchor = Some(p);
                }
            }
            _ => unreachable!("other tools dispatch before straight stair creation"),
        }
    }
    /// Unvalidated, uncommitted path candidate. Use preview_add_elements or a
    /// model command to validate it before treating it as an accepted element.
    pub fn polyline_creation_candidate(
        &self,
        points: Vec<Point>,
        arcs: Vec<Option<ArcSegment>>,
    ) -> Element {
        let mut element = self.creation_candidate(
            self.material,
            ElementLayer(ARCHICAD_LAYER),
            Shape::Polyline { points, arcs },
        );
        // Preserve Project::add's existing polyline default independently of
        // the current wall renovation setting.
        element.renovation = RenovationStatus::New;
        element
    }

    /// Pointer completion and shared callers commit through the same transaction.
    pub fn place_polyline(
        &mut self,
        points: Vec<Point>,
        arcs: Vec<Option<ArcSegment>>,
    ) -> Result<u64, String> {
        let element = self.polyline_creation_candidate(points, arcs);
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }
    pub fn finish_polyline(&mut self, endpoint: Option<Point>) -> Result<Option<u64>, String> {
        if self.tool != Tool::Polyline {
            return Ok(None);
        }
        if let Some(point) = endpoint {
            if !point.finite() || point.x.abs() >= 100000. || point.y.abs() >= 100000. {
                return Err(
                    "Coordinates must be finite and within the local modeling range".into(),
                );
            }
            if self.polyline_points.last().is_none_or(|last| {
                crate::precision::segment_meets_minimum(
                    *last,
                    point,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                )
            }) {
                self.polyline_points.push(point);
            }
        }
        if self.polyline_points.len() < 2 {
            self.status = "Polyline needs at least two distinct nodes".into();
            return Err(self.status.clone());
        }
        let id = match self.place_polyline(self.polyline_points.clone(), vec![]) {
            Ok(id) => id,
            Err(error) => {
                self.status = error.clone();
                return Err(error);
            }
        };
        self.polyline_points.clear();
        self.set_selection(vec![id]);
        self.status = format!("Created Polyline {id:03}");
        Ok(Some(id))
    }
    pub fn cancel_polyline(&mut self) {
        self.polyline_points.clear();
    }

    pub fn wall_draft_active(&self) -> bool {
        self.tool == Tool::Wall && (self.anchor.is_some() || !self.wall_points.is_empty())
    }

    /// First node of the current wall chain when clicking it should end the
    /// draft. Continuous chains can end after one segment; a polygonal ring
    /// needs at least three nodes before it can close.
    pub fn wall_close_target(&self) -> Option<Point> {
        if self.tool != Tool::Wall {
            return None;
        }
        let minimum = match self.wall_settings.geometry.geometry_method {
            WallGeometryMethod::Continuous => 2,
            WallGeometryMethod::Polygonal => 3,
            WallGeometryMethod::Single | WallGeometryMethod::Rectangular => return None,
        };
        let first = *self.wall_points.first()?;
        (self.wall_points.len() >= minimum).then_some(first)
    }

    /// Change the pending construction without committing or discarding nodes.
    pub fn set_wall_geometry_method(&mut self, method: WallGeometryMethod) {
        if self.wall_settings.geometry.geometry_method == method {
            return;
        }
        if self.wall_draft_active()
            && self.wall_points.is_empty()
            && let Some(anchor) = self.anchor
        {
            self.wall_points.push(anchor);
        }
        self.wall_settings.geometry.geometry_method = method;
        if self.tool == Tool::Wall {
            self.status = method.status_key().into();
        }
    }

    pub fn cancel_wall_draft(&mut self) {
        self.beam_arc_draft = None;
        self.anchor = None;
        self.clear_roof_creation();
        self.clear_slab_creation();
        self.wall_points.clear();
    }

    /// Restore the status hint that belongs to the current wall geometry
    /// method. Used after the Info Box changes the creation defaults.
    pub fn refresh_wall_status(&mut self) {
        if self.tool == Tool::Wall && !self.wall_draft_active() {
            self.status = self
                .wall_settings
                .geometry
                .geometry_method
                .status_key()
                .into();
        }
    }

    fn click_wall(&mut self, p: Point, tolerance: f64) {
        match self.wall_settings.geometry.geometry_method {
            WallGeometryMethod::Single => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points.clear();
                    self.status = WallGeometryMethod::Single.status_key().into();
                } else {
                    let _ = self.finish_wall(Some(p));
                }
            }
            WallGeometryMethod::Rectangular => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points.clear();
                    self.status = WallGeometryMethod::Rectangular.status_key().into();
                } else {
                    let _ = self.finish_wall(Some(p));
                }
            }
            WallGeometryMethod::Continuous => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points = vec![p];
                    self.status = WallGeometryMethod::Continuous.status_key().into();
                } else if let Some(start) = self.wall_points.first().copied()
                    && self.wall_points.len() >= 2
                    && start.distance(p) <= tolerance.max(0.001)
                {
                    // Clicking the first node ends the chain. With two nodes
                    // the chain is a single wall, so no zero-length closing
                    // segment is added; with three or more the closing wall
                    // is part of the completed contour.
                    let _ = if self.wall_points.len() >= 3 {
                        self.finish_wall(Some(start))
                    } else {
                        self.finish_wall(None)
                    };
                } else if self.wall_points.last().is_none_or(|last| {
                    crate::precision::segment_meets_minimum(
                        *last,
                        p,
                        crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                    )
                }) {
                    self.wall_points.push(p);
                    self.anchor = Some(p);
                }
            }
            WallGeometryMethod::Polygonal => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points = vec![p];
                    self.status = WallGeometryMethod::Polygonal.status_key().into();
                } else if let Some(start) = self.wall_points.first().copied()
                    && self.wall_points.len() >= 2
                    && start.distance(p) <= tolerance.max(0.001)
                {
                    if self.wall_points.len() >= 3 {
                        let _ = self.finish_wall(None);
                    } else {
                        self.status = "Polygon wall needs at least three nodes".into();
                    }
                } else if self.wall_points.last().is_none_or(|last| {
                    crate::precision::segment_meets_minimum(
                        *last,
                        p,
                        crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                    )
                }) {
                    self.wall_points.push(p);
                    self.anchor = Some(p);
                }
            }
        }
    }

    pub fn opening_defaults(&self, kind: OpeningKind) -> OpeningDefaults {
        match kind {
            OpeningKind::Door => self.door_defaults,
            OpeningKind::Window => self.window_defaults,
        }
    }

    pub fn set_opening_defaults(&mut self, kind: OpeningKind, defaults: OpeningDefaults) {
        match kind {
            OpeningKind::Door => self.door_defaults = defaults,
            OpeningKind::Window => self.window_defaults = defaults,
        }
    }

    /// Build a candidate opening for the current pointer position without
    /// changing the model. Shared by the click commit and the plan preview.
    pub fn opening_placement(
        &self,
        kind: OpeningKind,
        p: Point,
        tolerance: f64,
    ) -> Result<Element, String> {
        let host = self
            .project
            .elements
            .iter()
            .rev()
            .find(|element| {
                if element.story != self.story {
                    return false;
                }
                let Shape::Wall {
                    start,
                    end,
                    thickness,
                    ..
                } = element.shape
                else {
                    return false;
                };
                geometry::segment_distance(p, start, end) <= thickness * 0.5 + tolerance
            })
            .ok_or_else(|| format!("Click on a wall to place a {}", kind.name().to_lowercase()))?;
        let Shape::Wall {
            start,
            end,
            height: wall_height,
            ..
        } = host.shape
        else {
            return Err("Opening host must be a wall".into());
        };
        let length = start.distance(end);
        let defaults = self.opening_defaults(kind);
        defaults.validate()?;
        if length + 1e-4 < defaults.width {
            return Err("The host wall is shorter than the opening width".into());
        }
        let (bottom_offset, top_offset) = host
            .wall_settings
            .as_ref()
            .map_or((0., 0.), |settings| settings.vertical_offsets());
        if defaults.sill < bottom_offset - 1e-4 {
            return Err("The opening is below the host wall".into());
        }
        if defaults.sill + defaults.height > wall_height + top_offset + 1e-4 {
            return Err("The opening is taller than the host wall".into());
        }
        // The existing width tolerance permits a slightly wider opening.
        // Match translate_many's non-inverted interval at that boundary.
        let half = defaults.width * 0.5;
        let high = (length - half).max(half);
        let offset = geometry::wall_offset_of(&self.project, host.id, p)
            .unwrap_or(length * 0.5)
            .clamp(half, high);
        let placement = Element {
            worksheet: None,
            id: 0,
            name: String::new(),
            story: self.story,
            material: host.material,
            layer: ElementLayer(ARCHICAD_LAYER),
            renovation: self.wall_renovation,
            design_option: None,
            wall_contour: None,
            slab_contour: None,
            opening_joinery: Some(self.opening_joinery),
            wall_settings: None,
            shape: Shape::Opening {
                host_wall: host.id,
                kind,
                width: defaults.width,
                height: defaults.height,
                sill: defaults.sill,
                offset,
            },
        };
        Self::validate_placement(&self.project, &placement)?;
        Ok(placement)
    }

    /// Build an uncommitted opening using the existing pointer host lookup.
    /// The ID is a snapshot, not a reservation; full command validation is
    /// still required before accepting this candidate into the project.
    pub fn opening_creation_candidate(
        &self,
        kind: OpeningKind,
        point: Point,
        tolerance: f64,
    ) -> Result<Element, String> {
        let mut element = self.opening_placement(kind, point, tolerance)?;
        element.id = self.project.next_id;
        element.name = format!("{} {:03}", element.kind(), element.id);
        // Match the old Project::add_with_status scope; the model validator
        // rejects non-drawing elements in a worksheet.
        element.worksheet = self.project.plan_options.active_worksheet;
        Ok(element)
    }

    /// Place a hosted opening through the same candidate used by model callers.
    pub fn place_opening(
        &mut self,
        kind: OpeningKind,
        point: Point,
        tolerance: f64,
    ) -> Result<u64, String> {
        let element = self.opening_creation_candidate(kind, point, tolerance)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    /// Build an opening on an explicit existing wall. Offset is the exact
    /// centre station in metres, without pointer-placement clamping. The host
    /// supplies story/material; UI defaults and active story remain unchanged.
    /// Full current-project validation runs without reserving an identity.
    pub fn opening_on_wall_candidate(
        &self,
        kind: OpeningKind,
        host_wall: u64,
        offset: f64,
        dimensions: OpeningDefaults,
    ) -> Result<Element, String> {
        dimensions.validate()?;
        let host = self
            .project
            .elements
            .iter()
            .find(|e| e.id == host_wall)
            .ok_or_else(|| "Opening host must be an existing wall".to_owned())?;
        if !matches!(host.shape, Shape::Wall { .. }) {
            return Err("Opening host must be a wall".into());
        }
        let mut element = self.creation_candidate(
            host.material,
            ElementLayer(ARCHICAD_LAYER),
            Shape::Opening {
                host_wall,
                kind,
                width: dimensions.width,
                height: dimensions.height,
                sill: dimensions.sill,
                offset,
            },
        );
        element.story = host.story;
        self.preview_add_elements(vec![element.clone()], false)?;
        Ok(element)
    }

    /// Build an opening against the project plus an uncommitted creation batch.
    /// Pass the updated batch when building subsequent openings. Existing IDs
    /// and references are never rewritten, and no real identity is reserved.
    pub fn opening_in_batch_candidate(
        &self,
        pending: &[Element],
        kind: OpeningKind,
        host_wall: u64,
        offset: f64,
        dimensions: OpeningDefaults,
    ) -> Result<Element, String> {
        let project = self.preview_add_elements(pending.to_vec(), false)?;
        let mut staged = Self::new(project);
        staged.wall_renovation = self.wall_renovation;
        staged.opening_on_wall_candidate(kind, host_wall, offset, dimensions)
    }

    /// Commit an explicit-host opening through the shared atomic command.
    pub fn place_opening_on_wall(
        &mut self,
        kind: OpeningKind,
        host_wall: u64,
        offset: f64,
        dimensions: OpeningDefaults,
    ) -> Result<u64, String> {
        let element = self.opening_on_wall_candidate(kind, host_wall, offset, dimensions)?;
        let id = element.id;
        self.apply(ModelCommand::AddElement { element })?;
        Ok(id)
    }

    fn click_opening(&mut self, kind: OpeningKind, p: Point, tolerance: f64) -> Result<(), String> {
        let id = self.place_opening(kind, p, tolerance)?;
        self.set_selection(vec![id]);
        self.status = format!("Created {} {id:03}", kind.name());
        Ok(())
    }

    /// Complete the active wall geometry method. `endpoint` is the Tracker or
    /// double-click point; Continuous and Polygonal append it as their final
    /// node, while Single and Rectangular use it as the second defining point.
    /// Every method commits one undo transaction.
    pub fn finish_wall(&mut self, endpoint: Option<Point>) -> Result<Vec<u64>, String> {
        if let Some(point) = endpoint
            && (!point.finite() || point.x.abs() >= 100000. || point.y.abs() >= 100000.)
        {
            let error = "Coordinates must be finite and within the local modeling range".to_owned();
            self.status = error.clone();
            return Err(error);
        }
        let method = self.wall_settings.geometry.geometry_method;
        match method {
            WallGeometryMethod::Single => {
                let Some(start) = self.anchor else {
                    return Err("Wall needs a start point".into());
                };
                let Some(end) = endpoint else {
                    return Err("Wall needs an end point".into());
                };
                if !crate::precision::segment_meets_minimum(
                    start,
                    end,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                ) {
                    self.status = "Wall must be at least 10 mm".into();
                    return Err(self.status.clone());
                }
                let mut segments: Vec<_> = self
                    .wall_points
                    .windows(2)
                    .map(|pair| (pair[0], pair[1]))
                    .collect();
                segments.push((start, end));
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created element {:03}", ids[0]);
                Ok(ids)
            }
            WallGeometryMethod::Rectangular => {
                let Some(a) = self.anchor else {
                    return Err("Rectangle needs a first corner".into());
                };
                let Some(b) = endpoint else {
                    return Err("Rectangle needs an opposite corner".into());
                };
                let min = Point::new(a.x.min(b.x), a.y.min(b.y));
                let max = Point::new(a.x.max(b.x), a.y.max(b.y));
                if !crate::precision::axis_meets_minimum(
                    min.x,
                    max.x,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                ) || !crate::precision::axis_meets_minimum(
                    min.y,
                    max.y,
                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                ) {
                    self.status = "Rectangle needs a non-zero width and depth".into();
                    return Err(self.status.clone());
                }
                let corners = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];
                let segments = self
                    .wall_points
                    .windows(2)
                    .map(|pair| (pair[0], pair[1]))
                    .chain((0..4).map(|index| (corners[index], corners[(index + 1) % 4])))
                    .collect();
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created {} rectangular wall segments", ids.len());
                Ok(ids)
            }
            WallGeometryMethod::Continuous => {
                if self.wall_points.is_empty()
                    && let Some(start) = self.anchor
                {
                    self.wall_points.push(start);
                }
                if let Some(point) = endpoint
                    && self.wall_points.last().is_none_or(|last| {
                        crate::precision::segment_meets_minimum(
                            *last,
                            point,
                            crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                        )
                    })
                {
                    self.wall_points.push(point);
                }
                if self.wall_points.len() < 2 {
                    self.status = "Continuous wall needs at least two nodes".into();
                    return Err(self.status.clone());
                }
                let segments = self
                    .wall_points
                    .windows(2)
                    .map(|pair| (pair[0], pair[1]))
                    .collect();
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created {} connected wall segments", ids.len());
                Ok(ids)
            }
            WallGeometryMethod::Polygonal => {
                if self.wall_points.is_empty()
                    && let Some(start) = self.anchor
                {
                    self.wall_points.push(start);
                }
                if let Some(point) = endpoint {
                    let closes_on_start = self
                        .wall_points
                        .first()
                        .is_some_and(|start| start.distance(point) < 0.01);
                    if !closes_on_start
                        && self.wall_points.last().is_none_or(|last| {
                            crate::precision::segment_meets_minimum(
                                *last,
                                point,
                                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                            )
                        })
                    {
                        self.wall_points.push(point);
                    }
                }
                if self.wall_points.len() < 3 {
                    self.status = "Polygon wall needs at least three nodes".into();
                    return Err(self.status.clone());
                }
                let area = polygon_signed_area(&self.wall_points);
                if area.abs() < 1e-5 {
                    self.status = "Polygon wall needs a non-zero area".into();
                    return Err(self.status.clone());
                }
                // Keep the ring counter-clockwise so Inside/Outside reference
                // line offsets consistently describe the enclosed interior.
                if area < 0. {
                    self.wall_points.reverse();
                }
                let count = self.wall_points.len();
                let segments = (0..count)
                    .map(|index| {
                        (
                            self.wall_points[index],
                            self.wall_points[(index + 1) % count],
                        )
                    })
                    .collect();
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created {} polygon wall segments", ids.len());
                Ok(ids)
            }
        }
    }

    /// Backspace during a wall draft removes the last input node/segment
    /// without touching the selected model elements.
    pub fn backspace_wall(&mut self) -> bool {
        if !self.wall_draft_active() {
            return false;
        }
        self.backtrack_drawing();
        true
    }

    /// Build a wall batch without changing the editor or reserving identities.
    /// IDs are consecutive snapshots; full project validation is still required.
    pub fn wall_creation_candidates(
        &self,
        segments: Vec<(Point, Point)>,
    ) -> Result<Vec<Element>, String> {
        if self.project.layers.is_locked(self.wall_layer.0) {
            let name = self
                .project
                .layers
                .layer(self.wall_layer.0)
                .map(|layer| layer.name.clone())
                .unwrap_or_else(|| "unknown".into());
            return Err(format!("Layer {name} is locked"));
        }
        if u64::try_from(segments.len())
            .ok()
            .and_then(|count| self.project.next_id.checked_add(count))
            .is_none()
        {
            return Err("Invalid element identity or story".into());
        }
        let mut elements = Vec::with_capacity(segments.len());
        for (index, (start, end)) in segments.into_iter().enumerate() {
            if !crate::precision::segment_meets_minimum(
                start,
                end,
                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
            ) {
                return Err("Wall must be at least 10 mm".into());
            }
            let mut element = self.creation_candidate(
                self.material,
                self.wall_layer,
                Shape::Wall {
                    start,
                    end,
                    thickness: self.thickness,
                    height: self.height,
                },
            );
            element.id += index as u64; // whole-batch capacity was checked above
            element.name = format!("Wall {:03}", element.id);
            element.wall_settings = Some(self.wall_settings.clone());
            elements.push(element);
        }
        Ok(elements)
    }

    /// Create one wall batch with current defaults as a single model command.
    pub fn place_wall_segments(
        &mut self,
        segments: Vec<(Point, Point)>,
    ) -> Result<Vec<u64>, String> {
        let elements = self
            .wall_creation_candidates(segments)
            .inspect_err(|error| {
                self.status = error.clone();
            })?;
        let ids = elements.iter().map(|element| element.id).collect();
        self.apply(ModelCommand::AddElements {
            elements,
            group: self.project.edit_state.autogroup,
        })?;
        Ok(ids)
    }

    fn create_wall_segments(&mut self, segments: Vec<(Point, Point)>) -> Result<Vec<u64>, String> {
        let ids = self.place_wall_segments(segments).inspect_err(|error| {
            self.status = error.clone();
        })?;
        self.anchor = None;
        self.clear_roof_creation();
        self.wall_points.clear();
        self.selected = ids.last().copied();
        self.selected_ids = self.selected.into_iter().collect();
        Ok(ids)
    }
    /// Pick up the transferable parameters of one element: remember a full
    /// snapshot per element type and apply the active transfer set onto the
    /// tool creation defaults, like the AC29 eyedropper. Not a model change,
    /// so no undo record is written.
    pub fn pick_up_from(
        &mut self,
        id: u64,
        set: &crate::transfer::TransferSettings,
    ) -> Result<crate::transfer::TransferType, String> {
        let Some(source) = self
            .project
            .elements
            .iter()
            .find(|element| element.id == id)
            .cloned()
        else {
            return Err("No element under the cursor".into());
        };
        let Some(transfer_type) = crate::transfer::TransferType::of(&source) else {
            return Err("Unsupported element type".into());
        };
        let picked = crate::transfer::apply_pick_up_defaults(&source, set);
        if let Some(material) = picked.material {
            self.material = material;
        }
        if let Some(layer) = picked.layer
            && matches!(
                transfer_type,
                crate::transfer::TransferType::Wall | crate::transfer::TransferType::ConceptMass
            )
        {
            self.wall_layer = layer;
        }
        match transfer_type {
            crate::transfer::TransferType::Column => {
                if let Some(v) = picked.material {
                    self.column_material = v;
                }
                if let Some(v) = picked.layer {
                    self.column_layer = v;
                }
            }
            crate::transfer::TransferType::Beam => {
                if let Some(v) = picked.material {
                    self.beam_material = v;
                }
                if let Some(v) = picked.layer {
                    self.beam_layer = v;
                }
            }
            crate::transfer::TransferType::Roof => {
                if let Some(v) = picked.material {
                    self.roof_material = v;
                }
                if let Some(v) = picked.layer {
                    self.roof_layer = v;
                }
            }
            _ => {}
        }
        match picked.model_shape {
            Some(Shape::ConceptMass { spec }) => {
                self.concept_mass_height = spec.height;
                self.concept_mass_bottom_offset = spec.bottom_offset;
            }
            Some(Shape::Column {
                section,
                height,
                bottom_offset,
                axis_rotation,
                inclination,
                ..
            }) => {
                self.column_section = section;
                self.column_height = height;
                self.column_bottom_offset = bottom_offset;
                self.column_axis_rotation = axis_rotation;
                self.column_slanted = inclination.is_some();
                if let Some(v) = inclination {
                    self.column_inclination = v;
                }
            }
            Some(Shape::Beam {
                width,
                height,
                top_offset,
                slant_angle,
                ..
            }) => {
                self.beam_size = [width, height, top_offset];
                self.beam_slant_angle = slant_angle;
            }
            Some(Shape::Roof { spec }) => {
                self.roof_size = [spec.pitch, spec.thickness, spec.pivot_offset];
            }
            _ => {}
        }
        if let Some(plan) = picked.column_plan {
            self.column_plan = plan;
        }
        if let Some(thickness) = picked.thickness {
            self.thickness = thickness;
        }
        if let Some(height) = picked.height {
            self.height = height;
        }
        if let Some(settings) = picked.wall_settings {
            self.wall_settings = settings;
        }
        if let Some(spec) = picked.stair_spec {
            self.stair_spec = spec;
        }
        if let Some((kind, defaults)) = picked.opening_defaults {
            match kind {
                crate::model::OpeningKind::Door => self.door_defaults = defaults,
                crate::model::OpeningKind::Window => self.window_defaults = defaults,
            }
        }
        self.pick_up_buffer.store(&source);
        // AC29 semantics: a pick-up arms the matching tool with the picked
        // defaults, ready to draw. The caller reports the pick-up in the
        // status bar; the toolbox shows the activated tool.
        self.set_tool(match transfer_type {
            crate::transfer::TransferType::ConceptMass => Tool::ConceptMass,
            crate::transfer::TransferType::Column => Tool::Column,
            crate::transfer::TransferType::Beam => Tool::Beam,
            crate::transfer::TransferType::Roof => Tool::Roof,
            crate::transfer::TransferType::Wall => Tool::Wall,
            crate::transfer::TransferType::Slab => Tool::Slab,
            crate::transfer::TransferType::Stair => Tool::Stair,
            crate::transfer::TransferType::Polyline => Tool::Polyline,
            crate::transfer::TransferType::Door => Tool::Door,
            crate::transfer::TransferType::Window => Tool::Window,
        });
        Ok(transfer_type)
    }

    /// Inject the picked-up parameters of each matching element type into the
    /// given elements. The whole injection is validated and committed as one
    /// undo record; a validation failure leaves the project untouched.
    pub fn inject_into(
        &mut self,
        ids: &[u64],
        set: &crate::transfer::TransferSettings,
    ) -> Result<usize, String> {
        let mut updated: Vec<Element> = Vec::new();
        for id in ids {
            let Some(target) = self.project.elements.iter().find(|e| e.id == *id) else {
                continue;
            };
            let Some(transfer_type) = crate::transfer::TransferType::of(target) else {
                continue;
            };
            let Some(source) = self.pick_up_buffer.get(transfer_type).cloned() else {
                continue;
            };
            let mut candidate = target.clone();
            crate::transfer::apply_transfer(&source, &mut candidate, set)?;
            if candidate != *target {
                updated.push(candidate);
            }
        }
        if updated.is_empty() {
            return Err("No picked-up parameters for the selected element type".into());
        }
        let before = self.project.clone();
        for element in &updated {
            if let Some(target) = self
                .project
                .elements
                .iter_mut()
                .find(|e| e.id == element.id)
            {
                *target = element.clone();
            }
        }
        if let Err(error) = self.project.validate() {
            self.project = before;
            return Err(error);
        }
        if !self.commit_checked(before) {
            return Err(self.status.clone());
        }
        Ok(updated.len())
    }

    /// Replace the named transfer sets with one reversible project change.
    pub fn set_transfer_settings(&mut self, settings: crate::transfer::TransferSettings) {
        if self.project.transfer_settings == settings {
            return;
        }
        let before = self.project.clone();
        self.project.transfer_settings = settings;
        self.commit(before);
    }

    fn validate_placement(project: &Project, element: &Element) -> Result<(), String> {
        if matches!(element.shape, Shape::Opening { host_wall, .. } if Self::element_is_locked(project, host_wall))
        {
            return Err("Selection contains locked elements".into());
        }
        if project.layers.is_locked(element.layer.0) {
            return Err(format!(
                "Layer {} is locked",
                project
                    .layers
                    .layer(element.layer.0)
                    .map(|layer| layer.name.as_str())
                    .unwrap_or("unknown")
            ));
        }
        Ok(())
    }

    fn locked_element_error(&self, id: u64) -> String {
        if self
            .project
            .elements
            .iter()
            .find(|element| element.id == id)
            .is_some_and(|element| self.project.layers.is_locked(element.layer.0))
        {
            format!("Element {id} is on a locked layer")
        } else {
            format!("Element {id} is locked or read-only")
        }
    }

    /// True when an existing element is protected by a layer, explicit lock,
    /// or an external reference owner.
    pub fn locked_element(&self, id: u64) -> bool {
        Self::element_is_locked(&self.project, id)
    }

    pub(crate) fn element_is_locked(project: &Project, id: u64) -> bool {
        project
            .elements
            .iter()
            .find(|element| element.id == id)
            .is_some_and(|element| {
                project.layers.is_locked(element.layer.0)
                    || project.edit_state.locked.contains(&id)
                    || project.drawings.reference_owner(id).is_some()
            })
    }

    /// Replaces the layer document and remaps listed elements in one
    /// reversible project change (Layer Settings OK).
    pub fn set_layer_document(
        &mut self,
        layers: LayerDocument,
        remap: &[(u64, ElementLayer)],
    ) -> bool {
        let before = self.project.clone();
        self.project.layers = layers;
        for (id, layer) in remap {
            if let Some(element) = self
                .project
                .elements
                .iter_mut()
                .find(|element| element.id == *id)
            {
                element.layer = *layer;
            }
        }
        self.commit_checked(before)
    }

    pub fn undo(&mut self) {
        self.last_validation_diagnostic = None;
        if self.tool == Tool::Column {
            self.anchor = None;
        }
        if self.beam_arc_draft.take().is_some() {
            self.anchor = None;
        }
        self.clear_slab_creation();
        if self.clear_roof_creation() {
            self.anchor = None;
        }
        let scope = self.project.plan_options.active_worksheet;
        let candidate = self.history_candidate_context();
        if let Some(record) = self.undo.pop() {
            let selected = self.selection_ids();
            record
                .delta
                .undo_preserving_design_history(&mut self.project);
            self.redo.push(record);
            self.revision = self.revision.wrapping_add(1);
            if candidate != self.history_candidate_context() {
                self.restore_history_candidate_selection(selected);
            } else {
                self.set_selection(
                    selected
                        .into_iter()
                        .filter(|id| self.project.elements.iter().any(|e| e.id == *id))
                        .collect(),
                );
            }
            self.anchor = None;
            self.clear_roof_creation();
            self.wall_points.clear();
            self.status = "Undo".into();
        }
        self.restore_worksheet_scope(scope);
    }
    pub fn redo(&mut self) {
        self.last_validation_diagnostic = None;
        if self.tool == Tool::Column {
            self.anchor = None;
        }
        if self.beam_arc_draft.take().is_some() {
            self.anchor = None;
        }
        self.clear_slab_creation();
        if self.clear_roof_creation() {
            self.anchor = None;
        }
        let scope = self.project.plan_options.active_worksheet;
        let candidate = self.history_candidate_context();
        if let Some(record) = self.redo.pop() {
            let selected = self.selection_ids();
            record
                .delta
                .redo_preserving_design_history(&mut self.project);
            self.undo.push(record);
            self.revision = self.revision.wrapping_add(1);
            if candidate != self.history_candidate_context() {
                self.restore_history_candidate_selection(selected);
            } else {
                self.selected = None;
                self.selected_ids.clear();
                self.selection_scope = None;
            }
            self.anchor = None;
            self.clear_roof_creation();
            self.wall_points.clear();
            self.status = "Redo".into();
        }
        self.restore_worksheet_scope(scope);
    }
    fn history_candidate_context(&self) -> (Option<String>, Option<u32>) {
        (
            self.project.design_process.current_task.clone(),
            self.project.plan_options.active_design_option,
        )
    }
    // History may change the visible candidate without removing its members.
    // Only declared correspondence can retarget selection; never guess by shape/name.
    fn restore_history_candidate_selection(&mut self, selected: Vec<u64>) {
        let option = self.project.plan_options.active_design_option;
        let selection: Option<Vec<_>> =
            selected
                .into_iter()
                .map(|id| {
                    if self.project.elements.iter().any(|e| {
                        e.id == id && (e.design_option.is_none() || e.design_option == option)
                    }) {
                        Some(id)
                    } else {
                        self.project
                            .design_process
                            .task()
                            .and_then(|task| task.corresponding_element(&self.project, id, option?))
                    }
                })
                .collect();
        self.set_selection(selection.unwrap_or_default());
    }
    pub fn delete(&mut self) {
        let ids = self.selection_ids();
        if !ids.is_empty() {
            match self.apply(ModelCommand::RemoveElements { ids }) {
                Ok(removed) if !removed.is_empty() => {
                    self.selected = None;
                    self.selected_ids.clear();
                    self.selection_scope = None;
                    self.status = format!("Deleted {} element(s)", removed.len());
                }
                Ok(_) => {}
                Err(error) => self.status = error,
            }
        }
    }
    pub fn selection_ids(&self) -> Vec<u64> {
        if self.selected_ids.is_empty() {
            self.expanded_selection(self.selected.into_iter().collect())
        } else {
            self.expanded_selection(self.selected_ids.clone())
        }
    }
    pub fn set_selection(&mut self, ids: Vec<u64>) {
        self.selection_scope = None;
        let mut ids = self.expanded_selection(ids);
        ids.sort_unstable();
        ids.dedup();
        ids.retain(|id| {
            self.project
                .elements
                .iter()
                .any(|e| e.id == *id && e.worksheet == self.project.plan_options.active_worksheet)
        });
        self.selected = ids.last().copied();
        self.selected_ids = ids;
    }

    /// Select from an explicit displayed/allowed set while retaining group
    /// expansion within that set. Repeated selection reads cannot reintroduce
    /// hidden, off-story or forbidden members of a selected group.
    /// The caller owns view eligibility; do not apply the main plan's active
    /// design option here, because comparison panes display other options.
    pub fn set_scoped_selection(&mut self, ids: Vec<u64>, allowed_ids: &[u64]) {
        let allowed = self.scoped_selection_allowed_ids(allowed_ids);
        let ids = self.expand_allowed_selection(ids, &allowed);
        self.assign_scoped_selection(ids, allowed);
    }

    /// Toggle each hit's complete selectable group once. Removing only the
    /// clicked member would let the next group expansion select it again.
    pub fn toggle_scoped_selection(&mut self, ids: Vec<u64>, allowed_ids: &[u64]) {
        let allowed = self.scoped_selection_allowed_ids(allowed_ids);
        let mut selected: std::collections::BTreeSet<_> = self
            .selection_ids()
            .into_iter()
            .filter(|id| allowed.contains(id))
            .collect();
        let mut processed = std::collections::BTreeSet::new();
        for id in ids {
            if processed.contains(&id) {
                continue;
            }
            let group = self.expand_allowed_selection(vec![id], &allowed);
            processed.extend(group.iter().copied());
            if group.iter().all(|id| selected.contains(id)) {
                for id in group {
                    selected.remove(&id);
                }
            } else {
                selected.extend(group);
            }
        }
        let ids = self.expand_allowed_selection(selected.into_iter().collect(), &allowed);
        self.assign_scoped_selection(ids, allowed);
    }

    fn scoped_selection_allowed_ids(&self, allowed_ids: &[u64]) -> std::collections::BTreeSet<u64> {
        allowed_ids
            .iter()
            .copied()
            .filter(|id| {
                self.project.elements.iter().any(|e| {
                    e.id == *id && e.worksheet == self.project.plan_options.active_worksheet
                })
            })
            .collect()
    }

    fn expand_allowed_selection(
        &self,
        mut ids: Vec<u64>,
        allowed: &std::collections::BTreeSet<u64>,
    ) -> Vec<u64> {
        ids.retain(|id| allowed.contains(id));
        let mut ids = self.expanded_selection_unscoped(ids);
        ids.retain(|id| allowed.contains(id));
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn assign_scoped_selection(&mut self, ids: Vec<u64>, allowed: std::collections::BTreeSet<u64>) {
        self.selection_scope = None;
        self.selected_ids = ids;
        self.selected = self.selected_ids.last().copied();
        if !self.selected_ids.is_empty() {
            self.selection_scope = Some(SelectionScope {
                document_session: self.document_session(),
                worksheet: self.project.plan_options.active_worksheet,
                selected: self.selected,
                selected_ids: self.selected_ids.clone(),
                allowed,
            });
        }
    }

    pub(crate) fn selection_scope_allows(&self, id: u64) -> bool {
        self.selection_scope.as_ref().is_none_or(|scope| {
            scope.document_session != self.document_session()
                || scope.worksheet != self.project.plan_options.active_worksheet
                || scope.selected != self.selected
                || scope.selected_ids != self.selected_ids
                || scope.allowed.contains(&id)
        })
    }
    pub fn select_all_in_marquee(&mut self) -> usize {
        let Some(area) = &self.marquee else { return 0 };
        let ids = self
            .project
            .elements
            .iter()
            .filter(|e| {
                if e.worksheet != self.project.plan_options.active_worksheet
                    || !crate::view_settings::DesignOptionScope::active(
                        self.project.plan_options.active_design_option,
                    )
                    .contains(e.design_option)
                {
                    return false;
                }
                if matches!(
                    e.shape,
                    Shape::Column {
                        inclination: Some(_),
                        ..
                    }
                ) {
                    self.project
                        .plan_options
                        .element_visible(e, &self.project.layers)
                        && self
                            .project
                            .stories
                            .iter()
                            .filter(|story| area.includes_story(story.id))
                            .any(|story| {
                                crate::column_plan::reference_points(&self.project, e, story.id)
                                    .into_iter()
                                    .any(|point| geometry::contains(point, &area.vertices))
                            })
                } else {
                    area.includes_story(e.story)
                        && geometry::element_has_node_in_polygon(&self.project, e, &area.vertices)
                }
            })
            .filter(|e| {
                area.z_range
                    .is_none_or(|range| geometry::element_overlaps_z_range(&self.project, e, range))
            })
            .map(|e| e.id)
            .collect();
        self.set_selection(ids);
        self.selection_ids().len()
    }
    pub fn move_selection(&mut self, delta: Point) {
        let ids = self.selection_ids();
        if ids.is_empty() || !delta.finite() {
            return;
        }
        match self.apply(ModelCommand::MoveElements { ids, offset: delta }) {
            Ok(moved) if !moved.is_empty() => {
                self.status = format!("Moved {} element(s)", moved.len());
            }
            Ok(_) => {}
            Err(error) => self.status = error,
        }
    }
    pub fn remove_marquee(&mut self) {
        self.marquee = None;
        self.status = "Marquee removed".into();
    }
    pub fn move_marquee(&mut self, delta: Point) {
        if !delta.finite() {
            return;
        }
        if let Some(area) = &mut self.marquee {
            for p in &mut area.vertices {
                p.x += delta.x;
                p.y += delta.y;
            }
        }
    }
    pub fn stretch_nodes_in_marquee(&mut self, delta: Point) -> Result<usize, String> {
        let Some(area) = self.marquee.clone() else {
            return Ok(0);
        };
        if !delta.finite() {
            return Err("Stretch vector must be finite".into());
        }
        // A partial endpoint stretch needs an explicit contour-editing policy.
        // Reject it before mutating any element; whole-wall moves are exact.
        for e in self.project.elements.iter().filter(|e| {
            area.includes_story(e.story)
                && e.worksheet == self.project.plan_options.active_worksheet
        }) {
            if e.wall_contour.is_some()
                && let Shape::Wall { start, end, .. } = e.shape
                && geometry::point_in_polygon(start, &area.vertices)
                    != geometry::point_in_polygon(end, &area.vertices)
            {
                return Err("Partial stretch requires a straight wall".into());
            }
        }
        let before = self.project.clone();
        let mut changed = 0;
        for e in self.project.elements.iter_mut().filter(|e| {
            area.includes_story(e.story)
                && e.worksheet == self.project.plan_options.active_worksheet
        }) {
            if e.slab_contour.is_some() {
                if crate::slab_geometry::rings(e)
                    .iter()
                    .flat_map(|r| r.points.iter())
                    .any(|p| geometry::point_in_polygon(*p, &area.vertices))
                {
                    crate::slab_geometry::transform(
                        e,
                        |p| {
                            if geometry::point_in_polygon(p, &area.vertices) {
                                Point::new(p.x + delta.x, p.y + delta.y)
                            } else {
                                p
                            }
                        },
                        1.,
                        false,
                    );
                    changed += 1;
                }
                continue;
            }
            if e.wall_contour.is_some()
                && let Shape::Wall { start, end, .. } = e.shape
                && geometry::point_in_polygon(start, &area.vertices)
                && geometry::point_in_polygon(end, &area.vertices)
            {
                e.translate(delta);
                changed += 1;
                continue;
            }
            let whole_roof_inside = matches!(e.shape, Shape::Roof { .. })
                && geometry::element_fully_inside_polygon(&before, e, &area.vertices);
            match &mut e.shape {
                Shape::ConceptMass { spec } => {
                    if spec
                        .outline
                        .iter()
                        .all(|p| geometry::contains(*p, &area.vertices))
                    {
                        spec.transform(|p| Point::new(p.x + delta.x, p.y + delta.y));
                        changed += 1;
                    }
                }
                Shape::Zone { spec } => {
                    if spec
                        .outline
                        .iter()
                        .all(|p| geometry::contains(*p, &area.vertices))
                    {
                        spec.transform(|p| Point::new(p.x + delta.x, p.y + delta.y));
                        changed += 1;
                    }
                }
                Shape::Roof { spec } => {
                    if whole_roof_inside {
                        spec.translate(delta);
                        changed += 1;
                    }
                }
                Shape::Beam {
                    start,
                    end,
                    width,
                    arc,
                    ..
                } => {
                    let curve_inside = arc.is_none_or(|arc| {
                        crate::beam::BeamGeometry {
                            start: *start,
                            end: *end,
                            width: *width,
                            height: 1.,
                            start_top: 0.,
                            slant_angle: 0.,
                            arc: Some(arc),
                        }
                        .outline()
                        .is_ok_and(|points| {
                            points
                                .iter()
                                .all(|p| geometry::point_in_polygon(*p, &area.vertices))
                        })
                    });
                    if curve_inside
                        && geometry::point_in_polygon(*start, &area.vertices)
                        && geometry::point_in_polygon(*end, &area.vertices)
                    {
                        start.x += delta.x;
                        start.y += delta.y;
                        end.x += delta.x;
                        end.y += delta.y;
                        changed += 1;
                    }
                }
                Shape::Column { center, .. } => {
                    if geometry::point_in_polygon(*center, &area.vertices) {
                        center.x += delta.x;
                        center.y += delta.y;
                        changed += 1;
                    }
                }
                Shape::Wall { start, end, .. } => {
                    let a = geometry::point_in_polygon(*start, &area.vertices);
                    let b = geometry::point_in_polygon(*end, &area.vertices);
                    if a {
                        start.x += delta.x;
                        start.y += delta.y;
                    }
                    if b {
                        end.x += delta.x;
                        end.y += delta.y;
                    }
                    if a || b {
                        changed += 1;
                    }
                }
                Shape::Slab { min, max, .. } => {
                    let points = [
                        *min,
                        Point::new(max.x, min.y),
                        *max,
                        Point::new(min.x, max.y),
                    ];
                    let moved: Vec<Point> = points
                        .iter()
                        .map(|p| {
                            if geometry::point_in_polygon(*p, &area.vertices) {
                                Point::new(p.x + delta.x, p.y + delta.y)
                            } else {
                                *p
                            }
                        })
                        .collect();
                    if moved != points {
                        let lo = moved
                            .iter()
                            .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, b| {
                                Point::new(a.x.min(b.x), a.y.min(b.y))
                            });
                        let hi = moved
                            .iter()
                            .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, b| {
                                Point::new(a.x.max(b.x), a.y.max(b.y))
                            });
                        *min = lo;
                        *max = hi;
                        changed += 1;
                    }
                }
                Shape::Stair { spec } => {
                    let a = geometry::point_in_polygon(spec.start, &area.vertices);
                    let b = geometry::point_in_polygon(spec.end, &area.vertices);
                    if a {
                        spec.start.x += delta.x;
                        spec.start.y += delta.y;
                    }
                    if b {
                        spec.end.x += delta.x;
                        spec.end.y += delta.y;
                    }
                    if a || b {
                        changed += 1;
                    }
                }
                Shape::Polyline { points, .. } => {
                    let mut moved = false;
                    for point in points {
                        if geometry::point_in_polygon(*point, &area.vertices) {
                            point.x += delta.x;
                            point.y += delta.y;
                            moved = true;
                        }
                    }
                    if moved {
                        changed += 1;
                    }
                }
                Shape::Opening { .. } => {}
            }
        }
        if changed == 0 {
            return Ok(0);
        }
        if let Err(error) = self.project.validate() {
            self.project = before;
            return Err(error);
        }
        if !self.commit_checked(before) {
            return Err(self.status.clone());
        }
        self.status = format!("Stretched nodes on {changed} element(s)");
        Ok(changed)
    }
    pub fn duplicate(&mut self) {
        self.copy_selection();
        self.paste(Point::new(0.5, 0.5));
    }
    pub fn copy_selection(&mut self) -> usize {
        let mut ids = self.selection_ids();
        let hosts = ids.clone();
        ids.extend(self.project.elements.iter().filter_map(|e| match e.shape {
            Shape::Opening { host_wall, .. } if hosts.contains(&host_wall) => Some(e.id),
            _ => None,
        }));
        self.clipboard = self
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .cloned()
            .collect();
        self.clipboard_groups = self.project.edit_state.groups.clone();
        self.clipboard.len()
    }
    pub fn cut_selection(&mut self) -> usize {
        let count = self.copy_selection();
        if count > 0 {
            self.delete();
        }
        count
    }
    pub fn paste(&mut self, offset: Point) -> usize {
        if self.clipboard.is_empty() || !offset.finite() {
            return 0;
        }
        let before = self.project.clone();
        let mut candidate = self.project.clone();
        let mut map = std::collections::BTreeMap::new();
        for e in &self.clipboard {
            if candidate.layers.is_locked(e.layer.0) {
                self.status = "Selection contains locked elements".into();
                return 0;
            }
            let Some(next) = candidate.next_id.checked_add(1) else {
                self.status = "Invalid element identity or story".into();
                return 0;
            };
            map.insert(e.id, candidate.next_id);
            candidate.next_id = next;
        }
        let mut copies = Vec::new();
        for mut e in self.clipboard.clone() {
            e.worksheet = self.project.plan_options.active_worksheet;
            e.id = map[&e.id];
            if let Shape::Opening { host_wall, .. } = &mut e.shape {
                let Some(new_host) = map.get(host_wall) else {
                    self.status = "Select the host wall with its openings".into();
                    return 0;
                };
                *host_wall = *new_host;
            } else {
                e.translate(offset);
            }
            copies.push(e.id);
            candidate.elements.push(e);
        }
        for group in &self.clipboard_groups {
            let group: Vec<_> = group.iter().filter_map(|id| map.get(id).copied()).collect();
            if group.len() > 1 {
                candidate.edit_state.groups.push(group);
            }
        }
        if let Err(error) = candidate.validate() {
            self.status = error;
            return 0;
        }
        self.project = candidate;
        self.set_selection(copies);
        self.commit(before);
        self.selection_ids().len()
    }
    pub fn mirror_selection(&mut self, mirror_x: bool) {
        self.transform_selection_around_center(if mirror_x { 0. } else { 180. }, true);
    }
    pub fn rotate_selection_quarter_turn(&mut self, clockwise: bool) {
        self.transform_selection_around_center(if clockwise { -90. } else { 90. }, false);
    }
    fn transform_selection_around_center(&mut self, angle: f64, mirror: bool) {
        let ids = self.selection_ids();
        let points: Vec<_> = self
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .flat_map(|e| geometry::element_plan_outline(&self.project, e))
            .collect();
        if points.is_empty() {
            return;
        }
        let lo = points
            .iter()
            .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
                Point::new(a.x.min(p.x), a.y.min(p.y))
            });
        let hi = points
            .iter()
            .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
                Point::new(a.x.max(p.x), a.y.max(p.y))
            });
        if let Err(error) = self.edit(crate::editing::EditCommand::Transform {
            offset: Point::default(),
            origin: Point::new((lo.x + hi.x) / 2., (lo.y + hi.y) / 2.),
            angle,
            scale: 1.,
            mirror,
            copies: 0,
        }) {
            self.status = error;
        }
    }
    pub fn move_selection_to_story(&mut self, direction: i32) {
        let ids = self.selection_ids();
        if ids.is_empty() || direction == 0 {
            return;
        }
        let mut stories = self.project.stories.clone();
        stories.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        let before = self.project.clone();
        for e in self
            .project
            .elements
            .iter_mut()
            .filter(|e| ids.contains(&e.id))
        {
            if let Shape::Opening { host_wall, .. } = e.shape
                && !ids.contains(&host_wall)
            {
                // An opening must stay on its host wall's story.
                continue;
            }
            if let Some(index) = stories.iter().position(|s| s.id == e.story) {
                let next = index as i32 + direction.signum();
                if let Some(story) = usize::try_from(next).ok().and_then(|i| stories.get(i)) {
                    e.story = story.id;
                }
            }
        }
        if self.project != before {
            if let Err(error) = self.project.validate() {
                self.project = before;
                self.status = error;
                return;
            }
            if self.commit_checked(before) {
                self.status = "Moved selection to adjacent story".into();
            }
        }
    }
    pub fn replace_element(&mut self, e: Element) -> Result<(), String> {
        if self.locked_element(e.id) {
            return Err("Selection contains locked elements".into());
        }
        let before = self.project.clone();
        if let Some(slot) = self.project.elements.iter_mut().find(|v| v.id == e.id) {
            *slot = e;
        } else {
            return Err("Element no longer exists".into());
        }
        if let Err(err) = self.project.validate() {
            self.project = before;
            return Err(err);
        }
        self.commit(before);
        Ok(())
    }
    /// Replace one wall's stored reference-line endpoints. This is the model
    /// operation behind interactive baseline editing; callers that preview a
    /// drag pass their gesture-start snapshot to `commit` only on release.
    pub fn set_wall_reference_line(
        &mut self,
        id: u64,
        start: Point,
        end: Point,
    ) -> Result<(), String> {
        if self
            .project
            .elements
            .iter()
            .any(|e| e.id == id && e.wall_contour.is_some())
        {
            return Err("Reference line changes require a straight wall".into());
        }
        self.project.validate_wall_reference_line(id, start, end)?;
        let Some(element) = self
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == id)
        else {
            return Err("Missing wall".into());
        };
        let Shape::Wall {
            start: stored_start,
            end: stored_end,
            ..
        } = &mut element.shape
        else {
            return Err("Element is not a wall".into());
        };
        *stored_start = start;
        *stored_end = end;
        Ok(())
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn custom_wall_marquee_moves_complete_contour_but_rejects_partial_stretch() {
        let mut p = Project::default();
        let mut e = wall(1, 0.);
        e.wall_contour = Some(crate::model::WallContour::Trapezoid {
            outer_start: Point::new(0., 0.3),
            outer_end: Point::new(2., 0.6),
            outer_nodes: vec![],
            outer_positive: true,
        });
        p.next_id = 2;
        p.elements.push(e);
        let before = p.clone();
        let mut editor = Editor::new(p);
        let marquee = |right| {
            MarqueeArea::new(
                vec![
                    Point::new(-1., -1.),
                    Point::new(right, -1.),
                    Point::new(right, 2.),
                    Point::new(-1., 2.),
                ],
                MarqueeScope::CurrentStory,
                0,
            )
        };
        editor.marquee = marquee(1.);
        assert!(editor.stretch_nodes_in_marquee(Point::new(1., 1.)).is_err());
        assert_eq!(editor.project, before);
        assert!(editor.undo.is_empty());
        assert!(
            editor
                .set_wall_reference_line(1, Point::new(1., 0.), Point::new(3., 0.))
                .is_err()
        );
        assert_eq!(editor.project, before);
        editor.marquee = marquee(3.);
        assert_eq!(
            editor.stretch_nodes_in_marquee(Point::new(1., 1.)).unwrap(),
            1
        );
        let expected: Vec<_> = crate::wall_geometry::outline(&before.elements[0])
            .into_iter()
            .map(|p| Point::new(p.x + 1., p.y + 1.))
            .collect();
        assert_eq!(
            crate::wall_geometry::outline(&editor.project.elements[0]),
            expected
        );
        let after = editor.project.clone();
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, after);
    }

    fn wall(id: u64, x: f64) -> Element {
        Element {
            worksheet: None,
            id,
            name: format!("Wall {id}"),
            story: 0,
            material: Material::Plaster,
            layer: ElementLayer(ARCHICAD_LAYER),
            renovation: RenovationStatus::New,
            design_option: None,
            wall_contour: None,
            slab_contour: None,
            opening_joinery: None,
            wall_settings: None,
            shape: Shape::Wall {
                start: Point::new(x, 0.),
                end: Point::new(x + 2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        }
    }

    #[test]
    fn typed_commands_are_one_undo_each_and_redo_exactly() {
        let mut editor = Editor::new(Project::default());
        let first = wall(1, 0.);
        assert_eq!(
            editor
                .apply(ModelCommand::AddElement {
                    element: first.clone()
                })
                .unwrap(),
            vec![1]
        );
        editor
            .apply(ModelCommand::UpdateElement {
                element: wall(1, 3.),
            })
            .unwrap();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset: Point::new(2., 1.),
            })
            .unwrap();
        let mut options = editor.project.plan_options.clone();
        options.drawing_scale = 50;
        editor
            .apply(ModelCommand::SetOptions {
                plan: Some(options),
                design: None,
                guide: None,
                grid: None,
            })
            .unwrap();
        assert_eq!(editor.undo.len(), 4);
        let after = editor.project.clone();
        for _ in 0..4 {
            editor.undo();
        }
        assert!(editor.project.elements.is_empty());
        assert_eq!(editor.project.plan_options.drawing_scale, 100);
        assert_eq!(editor.redo.len(), 4);
        for _ in 0..4 {
            editor.redo();
        }
        assert_eq!(editor.project, after);
    }

    #[test]
    fn no_op_and_invalid_commands_leave_history_unchanged() {
        let mut editor = Editor::new(Project::default());
        editor
            .apply(ModelCommand::AddElement {
                element: wall(1, 0.),
            })
            .unwrap();
        let before = editor.project.clone();
        assert!(
            editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![99],
                    offset: Point::new(1., 0.),
                })
                .unwrap()
                .is_empty()
        );
        assert!(
            editor
                .apply(ModelCommand::UpdateElement {
                    element: wall(9, 0.)
                })
                .is_err()
        );
        assert_eq!(editor.project, before);
        assert_eq!(editor.undo.len(), 1);
    }

    #[test]
    fn a_new_typed_command_after_undo_clears_redo() {
        let mut editor = Editor::new(Project::default());
        editor
            .apply(ModelCommand::AddElement {
                element: wall(1, 0.),
            })
            .unwrap();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset: Point::new(1., 0.),
            })
            .unwrap();
        editor.undo();
        assert_eq!(editor.redo.len(), 1);
        editor
            .apply(ModelCommand::UpdateElement {
                element: wall(1, 8.),
            })
            .unwrap();
        assert!(editor.redo.is_empty());
        editor.undo();
        assert_eq!(editor.project.elements[0].shape, wall(1, 0.).shape);
    }

    #[test]
    fn delete_uses_the_typed_remove_command_and_undo_restores_order() {
        let mut editor = Editor::new(Project::default());
        for index in 0..3 {
            editor
                .apply(ModelCommand::AddElement {
                    element: wall(index + 1, index as f64 * 5.),
                })
                .unwrap();
        }
        let before = editor.project.clone();
        editor.set_selection(vec![2]);
        editor.delete();
        assert_eq!(editor.undo.len(), 4);
        assert_eq!(editor.project.elements.len(), 2);
        assert_eq!(editor.undo.last().unwrap().affected_ids(), vec![2]);
        editor.undo();
        assert_eq!(editor.project, before);
    }

    #[test]
    fn undo_history_keeps_affected_elements_only_and_caps_at_100() {
        let mut editor = Editor::new(Project::default());
        for index in 0..120 {
            editor
                .apply(ModelCommand::AddElement {
                    element: wall(index + 1, index as f64 * 3.),
                })
                .unwrap();
        }
        assert_eq!(editor.undo.len(), 100);
        assert!(
            editor
                .undo
                .iter()
                .all(|record| record.affected_ids().len() == 1)
        );
    }
}
