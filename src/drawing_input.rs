//! Local drawing transitions. The existing Editor drafts remain authoritative;
//! this service never applies model commands or reads/writes global history.
use crate::{
    editor::{Editor, Tool},
    wall::WallGeometryMethod,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingBacktrack {
    /// Another command (including single-click placement) owns the input.
    NotDrawing,
    /// Drawing owns the key, including at the empty-session boundary.
    Empty,
    Changed,
}

/// Shared accepted-step primitive, also used by non-model canvas drafts.
pub fn backtrack_point<T>(points: &mut Vec<T>) -> bool {
    points.pop().is_some()
}

impl Editor {
    pub fn backtrack_drawing(&mut self) -> DrawingBacktrack {
        let changed = match self.tool {
            Tool::Wall => {
                let active = self.wall_draft_active();
                match self.wall_settings.geometry.geometry_method {
                    WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal => {
                        backtrack_point(&mut self.wall_points);
                        self.anchor = self.wall_points.last().copied();
                    }
                    WallGeometryMethod::Single | WallGeometryMethod::Rectangular => {
                        self.wall_points.clear();
                        self.anchor = None;
                    }
                }
                active
            }
            Tool::Polyline => backtrack_point(&mut self.polyline_points),
            Tool::Slab => {
                if let Some(draft) = self.slab_polygon_draft.as_mut() {
                    let changed = draft.backtrack();
                    self.anchor = draft.points().last().copied();
                    if self.anchor.is_none() {
                        self.slab_polygon_draft = None;
                    }
                    changed
                } else {
                    // A rectangle is one atomic construction, including its
                    // rotation baseline. This is not a polygon-node stack.
                    let changed = self.slab_draft.take().is_some();
                    self.anchor.take().is_some() || changed
                }
            }
            Tool::Roof => {
                if let Some(draft) = self.roof_polygon_draft.as_mut() {
                    let changed = draft.backtrack();
                    self.anchor = draft.points().last().copied();
                    if self.anchor.is_none() {
                        self.clear_roof_creation();
                        true
                    } else {
                        changed
                    }
                } else {
                    let changed = self.clear_roof_creation();
                    self.anchor.take().is_some() || changed
                }
            }
            Tool::ConceptMass | Tool::Zone => {
                if self.zone_polygon {
                    backtrack_point(&mut self.polyline_points)
                } else {
                    self.anchor.take().is_some()
                }
            }
            Tool::Beam => {
                // Retract the unfinished arc as a construction, not a model
                // element or a sample in its derived sweep preview.
                let changed = self.beam_arc_draft.take().is_some();
                self.anchor.take().is_some() || changed
            }
            Tool::Stair => self.anchor.take().is_some(),
            Tool::Column if self.column_two_click => self.anchor.take().is_some(),
            _ => return DrawingBacktrack::NotDrawing,
        };
        self.status = match self.tool {
            Tool::Wall => self.wall_settings.geometry.geometry_method.status_key(),
            Tool::Slab => self.slab_method.status_key(),
            Tool::Roof => if self.roof_polygon_draft.is_some() {
                "Roof · Click the next polygon node"
            } else { "Roof · Click the pivot line start" },
            Tool::Polyline => "Polyline · Click start, left-click nodes, right-click to finish · click start to close",
            Tool::Zone => "Space · Draw a rectangle or polygon",
            Tool::ConceptMass => "Concept mass · Draw a rectangle or polygon",
            Tool::Column => "Column · Click the base axis point",
            Tool::Stair => "Stair · Set parameters, then click run start and end",
            Tool::Beam => { self.refresh_beam_status(); return if changed { DrawingBacktrack::Changed } else { DrawingBacktrack::Empty }; }
            _ => unreachable!(),
        }.into();
        if changed {
            DrawingBacktrack::Changed
        } else {
            DrawingBacktrack::Empty
        }
    }
}
