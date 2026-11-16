//! Presentation profiles and stable first-party command identities. No model ownership here.
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum WorkMode {
    #[default]
    Architecture,
    SiteConcept,
    ReportsDrawings,
}
impl WorkMode {
    pub const ALL: &'static [Self] =
        &[Self::Architecture, Self::SiteConcept, Self::ReportsDrawings];
    pub fn label(self) -> &'static str {
        match self {
            Self::Architecture => "Architectural design",
            Self::SiteConcept => "Site and concept",
            Self::ReportsDrawings => "Reports and drawings",
        }
    }
    pub fn profile(self) -> WorkspaceProfile {
        let source = match self {
            Self::Architecture => include_str!("../assets/workspaces/architecture.json"),
            Self::SiteConcept => include_str!("../assets/workspaces/site-concept.json"),
            Self::ReportsDrawings => include_str!("../assets/workspaces/reports-drawings.json"),
        };
        serde_json::from_str(source).expect("bundled workspace profile must be valid")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkbenchCommand {
    Drag,
    Rotate,
    Mirror,
    Elevate,
    Resize,
    Multiply,
    ToolSettings,
    InputX,
    InputY,
    InputDistance,
    InputAngle,
    Checks,
    Conditions,
    Reports,
    Drawings,
    Select,
    Plugins,
    Wall,
    ConceptMass,
    Zone,
    Spaces,
    Explore,
    Sources,
    Program,
    Evidence,
    Rules,
    Terrain,
    Site,
    Plan,
    Model,
    Save,
    Undo,
    Redo,
    Environment,
    SaveWorkspace,
    ResetWorkspace,
}
impl WorkbenchCommand {
    /// The same project-level entry points in native and in-window menus.
    /// Preparation command IDs below remain available for shortcuts and search.
    pub const PROJECT_MENU: [Self; 2] = [Self::Explore, Self::Conditions];

    pub const ALL: &'static [Self] = &[
        Self::Conditions,
        Self::Reports,
        Self::Drawings,
        Self::Select,
        Self::Plugins,
        Self::Wall,
        Self::ConceptMass,
        Self::Zone,
        Self::Spaces,
        Self::Explore,
        Self::Sources,
        Self::Program,
        Self::Evidence,
        Self::Rules,
        Self::Terrain,
        Self::Site,
        Self::Plan,
        Self::Model,
        Self::Save,
        Self::Undo,
        Self::Redo,
        Self::Environment,
        Self::SaveWorkspace,
        Self::ResetWorkspace,
        Self::Checks,
        Self::Drag,
        Self::Rotate,
        Self::Mirror,
        Self::Elevate,
        Self::Resize,
        Self::Multiply,
        Self::ToolSettings,
        Self::InputX,
        Self::InputY,
        Self::InputDistance,
        Self::InputAngle,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Drag => "edit.drag",
            Self::Rotate => "edit.rotate",
            Self::Mirror => "edit.mirror",
            Self::Elevate => "edit.elevate",
            Self::Resize => "edit.resize",
            Self::Multiply => "edit.multiply",
            Self::ToolSettings => "edit.tool_settings",
            Self::InputX => "input.coordinate.x",
            Self::InputY => "input.coordinate.y",
            Self::InputDistance => "input.distance",
            Self::InputAngle => "input.angle",
            Self::Checks => "analysis.constraints",
            Self::Conditions => "project.conditions",
            Self::Reports => "project.report_templates",
            Self::Drawings => "project.drawings",
            Self::Select => "tool.select",
            Self::Plugins => "workspace.plugins_jobs",
            Self::Wall => "tool.wall",
            Self::ConceptMass => "tool.concept_mass",
            Self::Zone => "tool.zone",
            Self::Spaces => "query.spaces",
            Self::Explore => "project.space_options",
            Self::Sources => "project.sources",
            Self::Program => "project.program",
            Self::Evidence => "project.evidence",
            Self::Rules => "project.rules",
            Self::Terrain => "project.terrain",
            Self::Site => "project.site_reference",
            Self::Plan => "view.plan",
            Self::Model => "view.model",
            Self::Save => "file.save",
            Self::Undo => "model.undo",
            Self::Redo => "model.redo",
            Self::Environment => "workspace.environment",
            Self::SaveWorkspace => "workspace.layout.save",
            Self::ResetWorkspace => "workspace.layout.reset",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Drag => "Drag",
            Self::Rotate => "Rotate",
            Self::Mirror => "Mirror",
            Self::Elevate => "Elevate…",
            Self::Resize => "Resize…",
            Self::Multiply => "Multiply…",
            Self::ToolSettings => "Tool Settings",
            Self::InputX => "X coordinate",
            Self::InputY => "Y coordinate",
            Self::InputDistance => "Distance input",
            Self::InputAngle => "Angle input",
            Self::Checks => "Check current design",
            Self::Conditions => "Project conditions",
            Self::Reports => "Report templates",
            Self::Drawings => "Drawing Manager",
            Self::Select => "Select",
            Self::Plugins => "Plugins and jobs…",
            Self::Wall => "Wall",
            Self::ConceptMass => "Concept mass",
            Self::Zone => "Zone",
            Self::Spaces => "Analyze spaces",
            Self::Explore => "Explore space options…",
            Self::Sources => "Project sources",
            Self::Program => "Functional program",
            Self::Evidence => "Source evidence",
            Self::Rules => "Conditions and rules",
            Self::Terrain => "Terrain surface",
            Self::Site => "Site reference",
            Self::Plan => "2D Plan",
            Self::Model => "3D Perspective",
            Self::Save => "Save project",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Environment => "Customize work environment…",
            Self::SaveWorkspace => "Save workspace layout",
            Self::ResetWorkspace => "Reset workspace layout",
        }
    }
    pub fn remote_invocation_supported(self) -> bool {
        if self.edit_label().is_some() || self.is_coordinate_input() {
            // These adapters start an existing native edit session. They do not
            // grant remote model writes or bypass typed model transactions.
            return false;
        }
        !matches!(
            self,
            Self::Save
                | Self::Plugins
                | Self::Undo
                | Self::Redo
                | Self::Environment
                | Self::SaveWorkspace
                | Self::ResetWorkspace
        )
    }
    /// Field activation belongs to the current transient input session.
    /// Defaults and personal replacements still use Profile.shortcuts.
    pub fn is_coordinate_input(self) -> bool {
        matches!(
            self,
            Self::InputX | Self::InputY | Self::InputDistance | Self::InputAngle
        )
    }
    pub fn disabled_reason(self, editor: &crate::editor::Editor) -> Option<&'static str> {
        match self {
            Self::Drag
            | Self::Rotate
            | Self::Mirror
            | Self::Elevate
            | Self::Resize
            | Self::Multiply
                if editor.selection_ids().is_empty() =>
            {
                Some("Select elements first")
            }
            Self::Undo if editor.undo.is_empty() => Some("Nothing to undo"),
            Self::Redo if editor.redo.is_empty() => Some("Nothing to redo"),
            _ => None,
        }
    }
    /// Stable adapters to the existing edit dispatcher; no second edit engine.
    pub fn edit_label(self) -> Option<&'static str> {
        matches!(
            self,
            Self::Drag
                | Self::Rotate
                | Self::Mirror
                | Self::Elevate
                | Self::Resize
                | Self::Multiply
                | Self::ToolSettings
        )
        .then(|| self.label())
    }
    pub fn from_edit_label(label: &str) -> Option<Self> {
        match label {
            "Undo" => Some(Self::Undo),
            "Redo" => Some(Self::Redo),
            _ => Self::ALL
                .iter()
                .copied()
                .find(|c| c.edit_label() == Some(label)),
        }
    }
}

#[derive(Default)]
pub struct EnvironmentState {
    pub mode: WorkMode,
}
impl EnvironmentState {
    /// Switching a profile deliberately has no document parameter.
    pub fn switch_mode(&mut self, mode: WorkMode) {
        self.mode = mode;
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceProfile {
    pub schema_version: u32,
    pub id: String,
    pub mode: WorkMode,
    pub commands: Vec<WorkbenchCommand>,
}

pub mod creation_presets;
pub mod environment;
pub mod preferences;
pub mod shortcuts;
pub mod workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandEffect {
    ToolSession,
    View,
    Query,
    FileDialog,
    ModelHistory,
    ConfigurationWrite,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct CommandDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub effect: CommandEffect,
    pub parameter_schema: serde_json::Value,
    pub context_requirements: &'static [&'static str],
    pub disabled_reason: Option<&'static str>,
    /// Discovery never implies a new write permission or a transport entry point.
    pub execution_adapter: &'static str,
    pub remote_invocation: bool,
}

pub struct CommandRegistry;
impl CommandRegistry {
    pub fn commands() -> &'static [WorkbenchCommand] {
        WorkbenchCommand::ALL
    }
    pub fn resolve(id: &str) -> Option<WorkbenchCommand> {
        Self::commands()
            .iter()
            .copied()
            .find(|command| command.id() == id)
    }
    pub fn describe(
        command: WorkbenchCommand,
        editor: &crate::editor::Editor,
    ) -> CommandDescriptor {
        use WorkbenchCommand::*;
        let effect = match command {
            Select | Wall | ConceptMass | Zone | Drag | Rotate | Mirror | Elevate | Resize
            | Multiply | ToolSettings | InputX | InputY | InputDistance | InputAngle => {
                CommandEffect::ToolSession
            }
            Checks | Conditions | Reports | Drawings | Plugins | Plan | Model | Sources | Site
            | Program | Terrain | Evidence | Rules | Environment | ResetWorkspace | Explore => {
                CommandEffect::View
            }
            SaveWorkspace => CommandEffect::ConfigurationWrite,
            Spaces => CommandEffect::Query,
            Save => CommandEffect::FileDialog,
            Undo | Redo => CommandEffect::ModelHistory,
        };
        CommandDescriptor {
            id: command.id(),
            label: command.label(),
            effect,
            parameter_schema: serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
            context_requirements: if command.is_coordinate_input() {
                &["document_session", "revision", "precision_input_session"]
            } else {
                &["document_session", "revision"]
            },
            disabled_reason: command.disabled_reason(editor),
            execution_adapter: "native_workbench",
            remote_invocation: command.remote_invocation_supported(),
        }
    }
    pub fn catalog(editor: &crate::editor::Editor) -> Vec<CommandDescriptor> {
        Self::commands()
            .iter()
            .map(|&command| Self::describe(command, editor))
            .collect()
    }
}
