//! Typed embedded-native panel groups; logical points, no model dependency.
use serde::{Deserialize, Serialize};
pub const TOOLBOX: &str = "panel.toolbox";
pub const NAVIGATOR: &str = "panel.navigator";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Placement {
    Left,
    Right,
    Floating,
}
impl Placement {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Dock group left",
            Self::Right => "Dock group right",
            Self::Floating => "Float panel group",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FloatRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl Default for FloatRect {
    fn default() -> Self {
        Self {
            x: 80,
            y: 100,
            width: 320,
            height: 460,
        }
    }
}
impl FloatRect {
    /// Outer window rectangle including egui frame/title; not OS monitor coordinates.
    pub fn recovered(self, viewport: [i32; 4]) -> Self {
        let [x, y, w, h] = viewport;
        let width = self.width.min(w.max(32) as u32);
        let height = self.height.min(h.max(32) as u32);
        Self {
            width,
            height,
            x: self
                .x
                .clamp(x, x.saturating_add(w.saturating_sub(width as i32).max(0))),
            y: self
                .y
                .clamp(y, y.saturating_add(h.saturating_sub(height as i32).max(0))),
        }
    }
    fn validate(self) -> Result<(), String> {
        if self.x.unsigned_abs() > 1_000_000
            || self.y.unsigned_abs() > 1_000_000
            || !(32..=8192).contains(&self.width)
            || !(32..=8192).contains(&self.height)
        {
            return Err("Invalid floating panel geometry".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PanelGroup {
    pub id: String,
    pub panels: Vec<String>,
    pub active: String,
    pub placement: Placement,
    pub extent: u32,
    pub floating: FloatRect,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeWorkspace {
    pub groups: Vec<PanelGroup>,
}
impl Default for NativeWorkspace {
    fn default() -> Self {
        Self {
            groups: vec![
                PanelGroup {
                    id: "workspace.toolbox".into(),
                    panels: vec![TOOLBOX.into()],
                    active: TOOLBOX.into(),
                    placement: Placement::Left,
                    extent: 188,
                    floating: FloatRect::default(),
                },
                PanelGroup {
                    id: "workspace.navigator".into(),
                    panels: vec![NAVIGATOR.into()],
                    active: NAVIGATOR.into(),
                    placement: Placement::Right,
                    extent: 318,
                    floating: FloatRect {
                        x: 440,
                        ..Default::default()
                    },
                },
            ],
        }
    }
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}
impl NativeWorkspace {
    pub fn validate(&self) -> Result<(), String> {
        if self.groups.is_empty() || self.groups.len() > 16 {
            return Err("Invalid panel group count".into());
        }
        let mut groups = std::collections::BTreeSet::new();
        let mut panels = std::collections::BTreeSet::new();
        for g in &self.groups {
            if !id(&g.id)
                || !groups.insert(&g.id)
                || g.panels.is_empty()
                || g.panels.len() > 32
                || !g.panels.contains(&g.active)
                || !(80..=2048).contains(&g.extent)
            {
                return Err("Invalid panel group".into());
            }
            g.floating.validate()?;
            for p in &g.panels {
                if !id(p) || !panels.insert(p.as_str()) {
                    return Err("Invalid or duplicate panel slot".into());
                }
            }
        }
        if !panels.contains(TOOLBOX) || !panels.contains(NAVIGATOR) {
            return Err("Required native panel slot is missing".into());
        }
        Ok(())
    }
    pub fn place(&mut self, group: &str, placement: Placement) -> Result<(), String> {
        self.groups
            .iter_mut()
            .find(|g| g.id == group)
            .ok_or("Panel group is missing")?
            .placement = placement;
        Ok(())
    }
    pub fn group_with(&mut self, panel: &str, target: &str) -> Result<(), String> {
        let mut next = self.clone();
        let from = next
            .groups
            .iter()
            .position(|g| g.panels.iter().any(|p| p == panel))
            .ok_or("Panel slot is missing")?;
        let to = next
            .groups
            .iter()
            .position(|g| g.panels.iter().any(|p| p == target))
            .ok_or("Panel slot is missing")?;
        if from == to {
            return Ok(());
        }
        next.groups[from].panels.retain(|p| p != panel);
        if !next.groups[from].panels.contains(&next.groups[from].active) {
            next.groups[from].active = next.groups[from]
                .panels
                .first()
                .cloned()
                .unwrap_or_default();
        }
        next.groups[to].panels.push(panel.into());
        next.groups[to].active = panel.into();
        next.groups.retain(|g| !g.panels.is_empty());
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn ungroup(&mut self, panel: &str, new_id: &str) -> Result<(), String> {
        let mut next = self.clone();
        let from = next
            .groups
            .iter()
            .position(|g| g.panels.iter().any(|p| p == panel))
            .ok_or("Panel slot is missing")?;
        if next.groups[from].panels.len() == 1 {
            next.groups[from].placement = Placement::Floating;
            *self = next;
            return Ok(());
        }
        let mut group = next.groups[from].clone();
        group.id = new_id.into();
        group.panels = vec![panel.into()];
        group.active = panel.into();
        group.placement = Placement::Floating;
        next.groups[from].panels.retain(|p| p != panel);
        if next.groups[from].active == panel {
            next.groups[from].active = next.groups[from].panels[0].clone();
        }
        next.groups.push(group);
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn recover(&mut self, viewport: [i32; 4]) -> bool {
        let mut changed = false;
        for group in &mut self.groups {
            if group.placement == Placement::Floating {
                let rect = group.floating.recovered(viewport);
                changed |= rect != group.floating;
                group.floating = rect;
            }
        }
        changed
    }
}
pub fn panel_label(id: &str) -> Option<&'static str> {
    match id {
        TOOLBOX => Some("Toolbox"),
        NAVIGATOR => Some("Navigator & Properties"),
        _ => None,
    }
}
