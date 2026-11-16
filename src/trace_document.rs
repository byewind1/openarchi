//! Project-owned Trace settings. UI pointer modes and dialogs are deliberately
//! absent. Floor-plan stories share a window; worksheets have independent ones.
use crate::{
    model::{Point, Project},
    trace::{TraceReference, TraceState, TraceVisibility},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceView {
    #[default]
    FloorPlan,
    Worksheet(u64),
}

/// The legacy image namespace can reuse an ID after deletion. Its accepted
/// content digest distinguishes a different image with the same number. Native
/// worksheets have their own persistent record and survive source updates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceSourceIdentity {
    Worksheet,
    Image(String),
}
impl TraceSourceIdentity {
    pub fn for_reference(project: &Project, reference: TraceReference) -> Option<Self> {
        let TraceReference::Worksheet(id) = reference else {
            return None;
        };
        if project.drawings.worksheets.iter().any(|w| w.id == id) {
            return Some(Self::Worksheet);
        }
        project
            .drawings
            .pages
            .iter()
            .find(|p| p.id == id && p.placement.is_none())
            .map(|p| Self::Image(p.sha256.clone()))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraceSettings {
    pub enabled: bool,
    pub reference: TraceReference,
    pub source_identity: Option<TraceSourceIdentity>,
    pub reference_opacity: u8,
    pub active_opacity: u8,
    pub reference_color: Option<[u8; 3]>,
    pub active_color: Option<[u8; 3]>,
    pub on_top: bool,
    pub transparent_fills: bool,
    pub pivot: Option<Point>,
    pub offset: Point,
    pub rotation_deg: f64,
    pub visibility: TraceVisibility,
}
impl Default for TraceSettings {
    fn default() -> Self {
        Self::capture(&TraceState::default())
    }
}
impl TraceSettings {
    /// Snapshot only confirmed placement, even if Save occurs during a gesture.
    pub fn capture(trace: &TraceState) -> Self {
        let (offset, rotation_deg) = trace.gesture_origin.unwrap_or((
            trace.displace_origin.unwrap_or(trace.offset),
            trace.rotation_deg,
        ));
        Self {
            enabled: trace.enabled,
            reference: trace.reference,
            source_identity: trace.source_identity.clone(),
            reference_opacity: trace.reference_opacity,
            active_opacity: trace.active_opacity,
            reference_color: trace.reference_color,
            active_color: trace.active_color,
            on_top: trace.on_top,
            transparent_fills: trace.transparent_fills,
            pivot: trace.pivot,
            offset,
            rotation_deg,
            visibility: trace.visibility,
        }
    }
    pub fn runtime(&self) -> TraceState {
        TraceState {
            enabled: self.enabled,
            reference: self.reference,
            source_identity: self.source_identity.clone(),
            reference_opacity: self.reference_opacity,
            active_opacity: self.active_opacity,
            reference_color: self.reference_color,
            active_color: self.active_color,
            on_top: self.on_top,
            transparent_fills: self.transparent_fills,
            pivot: self.pivot,
            offset: self.offset,
            rotation_deg: self.rotation_deg,
            visibility: self.visibility,
            ..Default::default()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.source_identity.is_some() && !matches!(self.reference, TraceReference::Worksheet(_))
            || matches!(&self.source_identity, Some(TraceSourceIdentity::Image(hash)) if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("Invalid Trace source identity".into());
        }
        if self
            .pivot
            .is_some_and(|p| !p.finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12)
            || !self.offset.finite()
            || self.offset.x.abs() > 1e12
            || self.offset.y.abs() > 1e12
            || !self.rotation_deg.is_finite()
            || self.rotation_deg.abs() > 1e9
        {
            return Err("Invalid Trace placement".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceWindow {
    pub view: TraceView,
    pub settings: TraceSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceViewport {
    pub center: Point,
    pub scale: f64,
}
impl TraceViewport {
    pub fn validate(&self) -> Result<(), String> {
        if !self.center.finite()
            || self.center.x.abs() > 1e12
            || self.center.y.abs() > 1e12
            || !self.scale.is_finite()
            || !(1e-6..=1e9).contains(&self.scale)
        {
            return Err("Invalid Trace viewport".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct TraceContext {
    pub view: TraceView,
    pub story: u32,
    pub previous_story: u32,
    pub plan_visible: bool,
    pub viewport: TraceViewport,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraceDocument {
    pub windows: Vec<TraceWindow>,
    pub recent: Vec<TraceReference>,
    pub active_view: TraceView,
    pub active_story: Option<u32>,
    pub previous_story: Option<u32>,
    pub plan_visible: bool,
    pub viewport: Option<TraceViewport>,
}
impl TraceDocument {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty() && self.recent.is_empty()
    }
    pub fn settings(&self, view: TraceView) -> Option<&TraceSettings> {
        self.windows
            .iter()
            .find(|w| w.view == view)
            .map(|w| &w.settings)
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(viewport) = self.viewport {
            viewport.validate()?;
        }
        if self.windows.len() > 4096 || self.recent.len() > 12 {
            return Err("Invalid Trace settings size".into());
        }
        for (i, window) in self.windows.iter().enumerate() {
            window.settings.validate()?;
            if self.windows[..i].iter().any(|w| w.view == window.view) {
                return Err("Duplicate Trace window".into());
            }
        }
        for (i, reference) in self.recent.iter().enumerate() {
            if self.recent[..i].contains(reference) {
                return Err("Duplicate recent Trace reference".into());
            }
        }
        // Dangling source IDs are preserved as unresolved references, never
        // rebound by name or silently replaced with another project's source.
        Ok(())
    }
    pub fn set(
        &mut self,
        view: TraceView,
        settings: TraceSettings,
        story: u32,
        previous: u32,
    ) -> Result<(), String> {
        settings.validate()?;
        if let Some(window) = self.windows.iter_mut().find(|w| w.view == view) {
            window.settings = settings;
        } else {
            if self.windows.len() >= 4096 {
                return Err("Invalid Trace settings size".into());
            }
            self.windows.push(TraceWindow { view, settings });
        }
        self.active_view = view;
        self.active_story = Some(story);
        self.previous_story = Some(previous);
        Ok(())
    }
    pub fn remember(&mut self, reference: TraceReference) {
        self.recent.retain(|r| *r != reference);
        self.recent.insert(0, reference);
        self.recent.truncate(12);
    }
    pub fn source_available(&self, project: &Project, reference: TraceReference) -> bool {
        reference
            .resolve(
                project,
                self.active_story.unwrap_or(0),
                self.previous_story.unwrap_or(0),
            )
            .is_some()
    }
}
