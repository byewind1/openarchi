use crate::{
    cursor::CursorFeedback,
    editor::{Editor, MarqueeArea, MarqueeGeometry, MarqueeScope, Tool},
    geometry::{self, Camera, SnapKind},
    localization::UiLanguage,
    model::{Element, Material, OpeningDefaults, OpeningKind, Point, Project, Shape, Story},
    render::SceneRenderer,
    storage,
    view_settings::{
        DimensionStyle, GraphicOverrides, Layer, LayerCombination, ModelViewOptions,
        ORIENTATION_PRESETS, PartialStructureDisplay, PenSet, PlanOptions, RenovationFilter,
        RenovationStatus, SCALE_PRESETS, ZOOM_PRESETS,
    },
    wall::{StructuralFunction, WallChoice, WallGeometryMethod, WallSettings},
    wall_ui::{
        WallDialogAction, WallSettingsDialog, WallSettingsDraft, layer_label, localized_error,
        top_link_label,
    },
};
use eframe::egui::{self, *};
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod dialogs;
mod file_io;
mod info_box;
mod inspector;
mod model_view;
mod plan;
mod quick_options;
mod shell;
#[cfg(test)]
mod ui_tests;

const INK: Color32 = Color32::from_rgb(225, 229, 230);
const MUTED: Color32 = Color32::from_rgb(155, 164, 167);
const ACCENT: Color32 = Color32::from_rgb(74, 160, 211);
const PAPER: Color32 = Color32::from_rgb(250, 250, 248);
const CHROME: Color32 = Color32::from_rgb(37, 40, 42);
const PANEL: Color32 = Color32::from_rgb(49, 52, 54);
const PANEL_ALT: Color32 = Color32::from_rgb(61, 65, 67);
const LINE: Color32 = Color32::from_rgb(77, 82, 84);
// Archicad 29 wall-selection appearance, sampled from the running INT build:
// pale green fill, dark green outline, a pure-blue reference line and black
// square reference-line handles. OpenArchi paints these with its own code.
const WALL_SELECTION_FILL: Color32 = Color32::from_rgb(195, 213, 155);
const WALL_SELECTION_OUTLINE: Color32 = Color32::from_rgb(0, 102, 0);
const WALL_REFERENCE_LINE: Color32 = Color32::from_rgb(0, 0, 245);
const WALL_REFERENCE_LINE_WIDTH: f32 = 2.5;
const WALL_REFERENCE_HANDLE_SIZE: f32 = 6.0;

/// Paint the selected wall's reference line and its two square handles.
/// Kept independent of project state so other plan views and tests can reuse
/// the same Archicad 29-inspired selection treatment.
fn paint_selected_wall_baseline(painter: &Painter, start: Pos2, end: Pos2) {
    painter.line_segment(
        [start, end],
        Stroke::new(WALL_REFERENCE_LINE_WIDTH, WALL_REFERENCE_LINE),
    );
    for handle in [start, end] {
        painter.rect_filled(
            Rect::from_center_size(handle, Vec2::splat(WALL_REFERENCE_HANDLE_SIZE)),
            0.0,
            Color32::BLACK,
        );
    }
}

fn rectangle_wall_segments(a: Point, b: Point) -> Vec<(Point, Point)> {
    let min = Point::new(a.x.min(b.x), a.y.min(b.y));
    let max = Point::new(a.x.max(b.x), a.y.max(b.y));
    let corners = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];
    (0..4)
        .map(|index| (corners[index], corners[(index + 1) % 4]))
        .collect()
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum View {
    Plan,
    Split,
    Model,
}
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Properties,
    Library,
    Assistant,
}
#[derive(Clone, Copy, PartialEq)]
enum NavigatorView {
    ProjectMap,
    ViewMap,
    LayoutBook,
    Publisher,
}
#[derive(Clone, Copy, PartialEq)]
enum FileAction {
    Save,
    Open,
    New,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlanOptionKind {
    Layers,
    PenSets,
    PartialStructure,
    ModelView,
    GraphicOverrides,
    Renovation,
    Dimensions,
    DesignOptions,
    Scale,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolylineEditCommand {
    InsertNode,
    MoveNode,
    Chamfer,
    Fillet,
    OffsetPolyline,
    CurveEdge,
    Move,
    Rotate,
    Mirror,
    Multiply,
}
#[derive(Clone, Debug)]
struct PolylineEdit {
    element_id: u64,
    node: Option<usize>,
    edge: Option<usize>,
    command: PolylineEditCommand,
    palette_at: Pos2,
    drag: Option<(Project, Point)>,
    chamfer_length: f32,
    fillet_radius: f32,
    multiply_count: usize,
    error: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WallEditCommand {
    MoveNode,
    Move,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct WallSnap {
    element_id: u64,
    point: Point,
}
#[derive(Clone, Debug)]
struct WallEdit {
    element_id: u64,
    /// `0` is the reference-line start, `1` is its end. `None` means the whole
    /// line was grabbed rather than one of its handles.
    node: Option<usize>,
    command: WallEditCommand,
    palette_at: Pos2,
    drag: Option<(Project, Point)>,
    snap: Option<WallSnap>,
    error: String,
}
#[derive(Clone, Debug)]
struct InfoBoxWallState {
    element_id: u64,
    draft: WallSettingsDraft,
    dirty: bool,
    error: String,
}
#[derive(Clone, Debug)]
struct InfoBoxWallToolState {
    draft: WallSettingsDraft,
    dirty: bool,
    error: String,
}
#[derive(Clone, Debug)]
struct InfoBoxOpeningState {
    element_id: u64,
    host_wall: u64,
    kind: OpeningKind,
    width: f32,
    height: f32,
    sill: f32,
    offset: f32,
    dirty: bool,
    error: String,
}
#[derive(Clone, Debug)]
struct InfoBoxOpeningToolState {
    kind: OpeningKind,
    defaults: OpeningDefaults,
    dirty: bool,
    error: String,
}
pub struct AtelierApp {
    pub editor: Editor,
    renderer: Option<SceneRenderer>,
    camera: Camera,
    view: View,
    tab: Tab,
    preview_tool: Option<String>,
    preview_element_id: String,
    preview_layer: String,
    preview_offset: f32,
    navigator_view: NavigatorView,
    navigator_search: String,
    plan_center: Point,
    plan_scale: f32,
    split: f32,
    show_left: bool,
    show_right: bool,
    quick_options_visible: bool,
    quick_options_palette_visible: bool,
    plan_option_dialog: Option<PlanOptionKind>,
    plan_dialog_options: PlanOptions,
    orientation_pick_active: bool,
    orientation_pick_start: Option<Point>,
    design_option_new_name: String,
    saved_zooms: Vec<(String, f32)>,
    guide_ui: GuideUi,
    grid_dialog: Option<crate::grids::GridSettings>,
    grid_dialog_error: String,
    grid_dialog_just_opened: bool,
    grid_preview_zoom: f32,
    rotated_grid_tool: Option<RotatedGridTool>,
    trace: crate::trace::TraceState,
    groups_enabled: bool,
    cutaway_enabled: bool,
    select_sub_elements: bool,
    select_locked_elements: bool,
    all_stories: bool,
    marquee_geometry: MarqueeGeometry,
    marquee_scope: MarqueeScope,
    marquee_outside: bool,
    show_marquee_3d: bool,
    marquee_points: Vec<Point>,
    marquee_preview: Option<(Point, Point)>,
    marquee_dx: f32,
    marquee_dy: f32,
    marquee_height: f32,
    marquee_3d_preview: Option<(Pos2, Pos2)>,
    marquee_3d_points: Vec<Pos2>,
    marquee_3d_cursor: Option<Pos2>,
    tracker_a: f32,
    tracker_b: f32,
    tracker_locked: bool,
    hover_point: Point,
    draft: Option<Element>,
    draft_id: Option<u64>,
    element_settings: Option<Element>,
    element_settings_error: String,
    wall_settings_dialog: Option<WallSettingsDialog>,
    info_box_visible: bool,
    info_box_wall: Option<InfoBoxWallState>,
    info_box_wall_tool: Option<InfoBoxWallToolState>,
    info_box_opening: Option<InfoBoxOpeningState>,
    info_box_opening_tool: Option<InfoBoxOpeningToolState>,
    stair_settings_open: bool,
    stair_settings_draft: crate::stairs::StairSpec,
    stair_settings_error: String,
    command_dialog: Option<String>,
    command_dialog_tab: usize,
    command_dialog_name: String,
    command_dialog_number: f32,
    command_dialog_enabled: bool,
    command_dialog_method: String,
    command_dialog_reference: String,
    command_dialog_representation: String,
    command_dialog_classification: String,
    drag: Option<(Project, Point, Vec<u64>)>,
    polyline_edit: Option<PolylineEdit>,
    wall_edit: Option<WallEdit>,
    wall_edit_palette_rect: Option<Rect>,
    file_action: Option<FileAction>,
    path: String,
    file_error: String,
    library_search: String,
    assistant_input: String,
    messages: Vec<(bool, String)>,
    help: bool,
    gpu_name: String,
    language: UiLanguage,
    close_prompt: bool,
    close_error: String,
    allow_close: bool,
    recovery_path: Option<PathBuf>,
    recovery_prompt: Option<Project>,
    recovery_error: String,
    last_recovery_save: Option<Instant>,
    last_recovery_revision: Option<u64>,
}
impl AtelierApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::style(&cc.egui_ctx);
        let gpu_name = cc
            .wgpu_render_state
            .as_ref()
            .map(|s| {
                let a = s.adapter.get_info();
                format!("{} · {:?}", a.name, a.backend)
            })
            .unwrap_or_else(|| "GPU unavailable".into());
        let renderer = cc.wgpu_render_state.clone().map(SceneRenderer::new);
        let mut app = Self::from_renderer(renderer, gpu_name);
        app.language = UiLanguage::load_preference();
        cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            app.language.window_title().to_owned(),
        ));
        if let Some(path) = app.recovery_path.clone() {
            match crate::recovery::read(&path) {
                Ok(project) => app.recovery_prompt = project,
                Err(error) => app.recovery_error = error,
            }
        }
        app
    }
    fn from_renderer(renderer: Option<SceneRenderer>, gpu_name: String) -> Self {
        let directory = std::env::current_dir()
            .ok()
            .filter(|p| p.parent().is_some())
            .unwrap_or_else(|| crate::platform::documents_dir().unwrap_or_else(std::env::temp_dir));
        Self {
            editor: Editor::new(Project::demo()), renderer, camera: Camera::default(),
            view: View::Plan, tab: Tab::Properties, preview_tool: None,
            preview_element_id: "OA-001".into(), preview_layer: "03 Plans - Preliminary".into(),
            preview_offset: 0.,
            navigator_view: NavigatorView::ProjectMap,
            navigator_search: String::new(), plan_center: Point::new(6.,3.),
            plan_scale: 40., split: 0.53, show_left: true, show_right: true,
            quick_options_visible: true,
            quick_options_palette_visible: false,
            plan_option_dialog: None,
            plan_dialog_options: PlanOptions::default(),
            orientation_pick_active: false,
            orientation_pick_start: None,
            design_option_new_name: String::new(),
            saved_zooms: Vec::new(),
            guide_ui: GuideUi::default(), grid_dialog: None, grid_dialog_error: String::new(),
            grid_dialog_just_opened: false,
            grid_preview_zoom: 25., rotated_grid_tool: None, trace: Default::default(),
            groups_enabled: true, cutaway_enabled: false, select_sub_elements: false,
            select_locked_elements: false, all_stories: true,
            marquee_geometry: MarqueeGeometry::Rectangle, marquee_scope: MarqueeScope::CurrentStory,
            marquee_outside: false, show_marquee_3d: false, marquee_points: Vec::new(), marquee_preview: None,
            marquee_dx: 1., marquee_dy: 0.,
            marquee_height: 3., marquee_3d_preview: None,
            marquee_3d_points: Vec::new(), marquee_3d_cursor: None,
            tracker_a: 4., tracker_b: 0., tracker_locked: false, hover_point: Point::default(),
            draft: None, draft_id: None, element_settings: None, element_settings_error: String::new(),
            wall_settings_dialog: None,
            info_box_visible: true,
            info_box_wall: None,
            info_box_wall_tool: None,
            info_box_opening: None,
            info_box_opening_tool: None,
            stair_settings_open: false, stair_settings_draft: crate::stairs::StairSpec::default(), stair_settings_error: String::new(),
            command_dialog: None, command_dialog_tab: 0, command_dialog_name: String::new(),
            command_dialog_number: 1., command_dialog_enabled: true,
            command_dialog_method: "Default".into(), command_dialog_reference: "Center".into(),
            command_dialog_representation: "By Element".into(), command_dialog_classification: "Not assigned".into(),
            drag: None, polyline_edit: None, wall_edit: None, wall_edit_palette_rect: None, file_action: None,
            path: directory.join("courtyard.openarchi.json").display().to_string(),
            file_error: String::new(), library_search: String::new(), assistant_input: String::new(),
            messages: vec![(false, "Your design companion\n\nAsk for a project summary, quantities, or a model check. This demo runs local rules; no AI service is connected.".into())],
            help: false, gpu_name, language: UiLanguage::English,
            close_prompt: false, close_error: String::new(), allow_close: false,
            recovery_path: crate::recovery::path(), recovery_prompt: None,
            recovery_error: String::new(), last_recovery_save: None,
            last_recovery_revision: None,
        }
    }

    fn style(ctx: &Context) {
        Self::install_cjk_fallback(ctx);
        let mut s = Style {
            visuals: Visuals::dark(),
            ..Default::default()
        };
        s.visuals.panel_fill = PANEL;
        s.visuals.window_fill = PANEL_ALT;
        s.visuals.override_text_color = Some(INK);
        s.visuals.selection.bg_fill = Color32::from_rgb(45, 92, 119);
        s.visuals.selection.stroke = Stroke::new(1., ACCENT);
        s.visuals.widgets.noninteractive.bg_fill = PANEL;
        s.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., LINE);
        s.visuals.widgets.inactive.bg_fill = PANEL_ALT;
        s.visuals.widgets.inactive.weak_bg_fill = PANEL;
        s.visuals.widgets.hovered.bg_fill = Color32::from_rgb(75, 82, 85);
        s.visuals.widgets.active.bg_fill = Color32::from_rgb(50, 90, 112);
        s.visuals.widgets.open.bg_fill = PANEL_ALT;
        s.spacing.item_spacing = vec2(6., 6.);
        s.spacing.button_padding = vec2(8., 5.);
        s.spacing.interact_size.y = 26.;
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(12.5));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(12.));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(11.));
        ctx.set_theme(Theme::Dark);
        ctx.set_global_style(s);
    }
    fn install_cjk_fallback(ctx: &Context) {
        crate::fonts::install_cjk(ctx);
    }
    fn label(ui: &mut Ui, text: &str) {
        ui.label(RichText::new(text).size(10.).color(MUTED).strong());
    }
    fn t(language: UiLanguage, text: &str) -> String {
        language.translate(text).to_owned()
    }
    fn element_count_label(&self, count: usize) -> String {
        match self.language {
            UiLanguage::English => {
                format!("{count} element{}", if count == 1 { "" } else { "s" })
            }
            UiLanguage::SimplifiedChinese => format!("{count} 个构件"),
        }
    }
    fn selected_count_label(&self, count: usize) -> String {
        match self.language {
            UiLanguage::English => format!("{count} selected"),
            UiLanguage::SimplifiedChinese => format!("已选 {count} 个"),
        }
    }
    fn localized_count_status(&self, status: &str) -> Option<String> {
        if let Some(id) = status
            .strip_prefix("Created element ")
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(format!("已创建构件 {id}"));
        }
        for (prefix, suffix, chinese) in [
            ("Copied ", " element(s)", "已复制"),
            ("Cut ", " element(s)", "已剪切"),
            ("Pasted ", " element(s)", "已粘贴"),
            ("Selected ", " element(s) in Marquee", "在选区框中选择"),
        ] {
            if let Some(count) = status
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_suffix(suffix))
                .and_then(|count| count.parse::<usize>().ok())
            {
                return Some(format!("{chinese} {count} 个构件"));
            }
        }
        if let Some(count) = status
            .strip_prefix("Created ")
            .and_then(|rest| rest.strip_suffix(" Polyline copies"))
            .and_then(|count| count.parse::<usize>().ok())
        {
            return Some(format!("已创建 {count} 条多段线副本"));
        }
        for (suffix, chinese) in [
            (" connected wall segments", "段连续墙"),
            (" rectangular wall segments", "段矩形墙"),
            (" polygon wall segments", "段多边形墙"),
        ] {
            if let Some(count) = status
                .strip_prefix("Created ")
                .and_then(|rest| rest.strip_suffix(suffix))
                .and_then(|count| count.parse::<usize>().ok())
            {
                return Some(format!("已创建 {count} {chinese}"));
            }
        }
        None
    }
    fn status_text(&self) -> String {
        let status = self.editor.status.trim();
        if self.language == UiLanguage::English || status.is_empty() {
            return status.to_owned();
        }
        if let Some(message) = self.localized_count_status(status) {
            return message;
        }
        if status.starts_with("Wall · ")
            || status.starts_with("Wall ")
            || status.starts_with("Rectangle ")
            || status.starts_with("Continuous wall ")
            || status.starts_with("Polygon wall ")
        {
            let translated = self.language.translate(status);
            if translated != status {
                return translated.to_owned();
            }
        }
        if let Some(rest) = status.strip_prefix("Element Settings · ")
            && rest.chars().last().is_some_and(|c| c.is_ascii_digit())
            && let Some((kind, id)) = rest.rsplit_once(' ')
            && id.chars().all(|c| c.is_ascii_digit())
        {
            return format!(
                "{} · {} {}",
                self.language.translate("Element Settings"),
                self.language.translate(kind),
                id
            );
        }
        for (suffix, replacement) in [
            (
                " tool selected · interface preview",
                " · 工具已选择 · 界面预览",
            ),
            (
                " tool selected · model interaction is not available in this demo",
                " · 工具已选择 · 此演示中暂不支持模型交互",
            ),
            (
                " preset active · Click in plan to place",
                "预设已启用 · 在平面中点击放置",
            ),
        ] {
            if let Some(command) = status.strip_suffix(suffix) {
                return format!("{}{}", self.language.translate(command), replacement);
            }
        }
        if let Some((command, rest)) = status.split_once(" · ")
            && (rest == "configuration preview" || rest == "preview values applied locally")
        {
            return format!(
                "{} · {}",
                self.language.translate(command),
                self.language.translate(rest)
            );
        }
        let exact = self.language.translate(status);
        if exact != status {
            return exact.to_owned();
        }
        let mut translated_any = false;
        let localized = status
            .split(" · ")
            .map(|segment| {
                let translated = self.language.translate(segment);
                if translated != segment {
                    translated_any = true;
                    translated.to_owned()
                } else {
                    segment.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" · ");
        if translated_any {
            localized
        } else {
            status.to_owned()
        }
    }
    fn preview_menu_items(ui: &mut Ui, language: UiLanguage, labels: &[&str]) -> Option<String> {
        for label in labels {
            if ui.button(language.translate(label)).clicked() {
                ui.close();
                return Some((*label).to_owned());
            }
        }
        None
    }
    fn paint_tool_icon(p: &Painter, rect: Rect, name: &str, color: Color32) {
        let q = |x: f32, y: f32| {
            pos2(
                rect.left() + x * rect.width(),
                rect.top() + y * rect.height(),
            )
        };
        let stroke = Stroke::new(1.35, color);
        let line = |a: (f32, f32), b: (f32, f32)| {
            p.line_segment([q(a.0, a.1), q(b.0, b.1)], stroke);
        };
        let path = |points: &[(f32, f32)]| {
            p.add(egui::Shape::line(
                points.iter().map(|&(x, y)| q(x, y)).collect(),
                stroke,
            ));
        };
        let closed = |points: &[(f32, f32)]| {
            p.add(egui::Shape::closed_line(
                points.iter().map(|&(x, y)| q(x, y)).collect(),
                stroke,
            ));
        };
        let box_outline = |x: f32, y: f32, w: f32, h: f32| {
            p.rect_stroke(
                Rect::from_min_max(q(x, y), q(x + w, y + h)),
                0.,
                stroke,
                StrokeKind::Inside,
            );
        };
        let circle = |x: f32, y: f32, r: f32| {
            p.circle_stroke(q(x, y), r * rect.width().min(rect.height()), stroke);
        };
        match name {
            "Home" | "Project Map" => {
                path(&[(0.12, 0.48), (0.5, 0.16), (0.88, 0.48)]);
                line((0.23, 0.43), (0.23, 0.84));
                line((0.77, 0.43), (0.77, 0.84));
                line((0.23, 0.84), (0.77, 0.84));
                line((0.43, 0.84), (0.43, 0.59));
                line((0.43, 0.59), (0.59, 0.59));
                line((0.59, 0.59), (0.59, 0.84));
            }
            "View Map" | "Layout Book" | "Publisher" | "Document" => {
                box_outline(0.19, 0.12, 0.62, 0.76);
                for y in [0.34, 0.48, 0.62, 0.76] {
                    line((0.31, y), (0.69, y));
                }
            }
            "Save" => {
                box_outline(0.17, 0.14, 0.66, 0.72);
                box_outline(0.31, 0.14, 0.38, 0.28);
                box_outline(0.29, 0.58, 0.42, 0.28);
            }
            "Undo" => {
                path(&[(0.4, 0.28), (0.2, 0.48), (0.4, 0.68)]);
                line((0.22, 0.48), (0.58, 0.48));
                path(&[(0.58, 0.48), (0.78, 0.57), (0.82, 0.75)]);
            }
            "Redo" => {
                path(&[(0.6, 0.28), (0.8, 0.48), (0.6, 0.68)]);
                line((0.78, 0.48), (0.42, 0.48));
                path(&[(0.42, 0.48), (0.22, 0.57), (0.18, 0.75)]);
            }
            "Zoom In" | "Zoom Out" => {
                circle(0.43, 0.42, 0.26);
                line((0.62, 0.62), (0.84, 0.84));
                line((0.3, 0.42), (0.56, 0.42));
                if name == "Zoom In" {
                    line((0.43, 0.29), (0.43, 0.55));
                }
            }
            "Fit" => {
                box_outline(0.2, 0.2, 0.6, 0.6);
                path(&[(0.2, 0.4), (0.2, 0.2), (0.4, 0.2)]);
                path(&[(0.6, 0.2), (0.8, 0.2), (0.8, 0.4)]);
                path(&[(0.8, 0.6), (0.8, 0.8), (0.6, 0.8)]);
                path(&[(0.4, 0.8), (0.2, 0.8), (0.2, 0.6)]);
            }
            "Drag" => {
                line((0.5, 0.15), (0.5, 0.85));
                path(&[(0.34, 0.33), (0.5, 0.16), (0.66, 0.33)]);
                path(&[(0.34, 0.67), (0.5, 0.84), (0.66, 0.67)]);
                line((0.15, 0.5), (0.85, 0.5));
                path(&[(0.33, 0.34), (0.16, 0.5), (0.33, 0.66)]);
                path(&[(0.67, 0.34), (0.84, 0.5), (0.67, 0.66)]);
            }
            "Rotate" => {
                path(&[
                    (0.72, 0.38),
                    (0.67, 0.2),
                    (0.49, 0.13),
                    (0.3, 0.19),
                    (0.19, 0.36),
                    (0.2, 0.57),
                    (0.34, 0.73),
                    (0.55, 0.78),
                ]);
                path(&[(0.64, 0.63), (0.57, 0.8), (0.75, 0.79)]);
            }
            "Mirror" => {
                line((0.5, 0.14), (0.5, 0.86));
                path(&[(0.4, 0.27), (0.2, 0.4), (0.2, 0.69), (0.4, 0.79)]);
                path(&[(0.6, 0.27), (0.8, 0.4), (0.8, 0.69), (0.6, 0.79)]);
            }
            "Multiply" => {
                box_outline(0.16, 0.34, 0.48, 0.48);
                box_outline(0.36, 0.16, 0.48, 0.48);
            }
            "Element" => {
                box_outline(0.21, 0.21, 0.58, 0.58);
                line((0.5, 0.1), (0.5, 0.9));
                line((0.1, 0.5), (0.9, 0.5));
            }
            "Area Selection" => {
                path(&[
                    (0.17, 0.23),
                    (0.79, 0.16),
                    (0.84, 0.71),
                    (0.42, 0.84),
                    (0.17, 0.23),
                ]);
            }
            "Quick Selection" => {
                circle(0.5, 0.5, 0.28);
                line((0.5, 0.08), (0.5, 0.33));
                line((0.5, 0.67), (0.5, 0.92));
                line((0.08, 0.5), (0.33, 0.5));
                line((0.67, 0.5), (0.92, 0.5));
            }
            "Navigator" | "Options" => {
                line((0.18, 0.27), (0.82, 0.27));
                line((0.18, 0.5), (0.82, 0.5));
                line((0.18, 0.73), (0.82, 0.73));
            }
            "Search" => {
                circle(0.43, 0.42, 0.27);
                line((0.62, 0.62), (0.84, 0.84));
            }
            "Split" => {
                box_outline(0.14, 0.2, 0.72, 0.6);
                line((0.5, 0.2), (0.5, 0.8));
            }
            "Floor Plan" => {
                box_outline(0.16, 0.16, 0.68, 0.68);
                line((0.16, 0.48), (0.84, 0.48));
                line((0.48, 0.16), (0.48, 0.48));
                line((0.65, 0.48), (0.65, 0.84));
            }
            "Perspective" => {
                closed(&[(0.16, 0.29), (0.84, 0.29), (0.72, 0.74), (0.28, 0.74)]);
                line((0.16, 0.29), (0.5, 0.53));
                line((0.84, 0.29), (0.5, 0.53));
                line((0.28, 0.74), (0.5, 0.53));
                line((0.72, 0.74), (0.5, 0.53));
            }
            "Elevation View" => {
                path(&[
                    (0.14, 0.76),
                    (0.14, 0.37),
                    (0.5, 0.17),
                    (0.86, 0.37),
                    (0.86, 0.76),
                ]);
                line((0.27, 0.76), (0.27, 0.43));
                line((0.73, 0.76), (0.73, 0.43));
                line((0.27, 0.58), (0.73, 0.58));
            }
            "Help" => {
                circle(0.5, 0.5, 0.36);
                path(&[
                    (0.36, 0.37),
                    (0.41, 0.25),
                    (0.56, 0.22),
                    (0.68, 0.31),
                    (0.66, 0.43),
                    (0.51, 0.54),
                    (0.51, 0.66),
                ]);
                circle(0.51, 0.82, 0.025);
            }
            "Reference" => {
                box_outline(0.18, 0.16, 0.64, 0.68);
                line((0.3, 0.32), (0.7, 0.32));
                line((0.3, 0.46), (0.7, 0.46));
                line((0.3, 0.6), (0.58, 0.6));
            }
            "Arrow" => closed(&[
                (0.2, 0.08),
                (0.2, 0.86),
                (0.43, 0.63),
                (0.57, 0.91),
                (0.7, 0.85),
                (0.55, 0.58),
                (0.87, 0.55),
            ]),
            "Marquee" => {
                box_outline(0.13, 0.22, 0.74, 0.56);
                for (a, b) in [
                    ((0.13, 0.22), (0.36, 0.22)),
                    ((0.13, 0.22), (0.13, 0.44)),
                    ((0.87, 0.78), (0.64, 0.78)),
                    ((0.87, 0.78), (0.87, 0.56)),
                ] {
                    line(a, b);
                }
            }
            "Wall" => {
                box_outline(0.13, 0.29, 0.74, 0.42);
                line((0.35, 0.3), (0.35, 0.7));
            }
            "Slab" => closed(&[(0.08, 0.58), (0.52, 0.24), (0.92, 0.44), (0.48, 0.78)]),
            "Column" => {
                circle(0.5, 0.5, 0.34);
                circle(0.5, 0.5, 0.12);
            }
            "Beam" => {
                line((0.16, 0.28), (0.84, 0.28));
                line((0.16, 0.72), (0.84, 0.72));
                line((0.3, 0.28), (0.3, 0.72));
                line((0.7, 0.28), (0.7, 0.72));
            }
            "Door" => {
                line((0.2, 0.15), (0.2, 0.84));
                line((0.2, 0.84), (0.78, 0.84));
                line((0.2, 0.22), (0.7, 0.22));
                path(&[
                    (0.7, 0.22),
                    (0.69, 0.35),
                    (0.64, 0.49),
                    (0.55, 0.61),
                    (0.42, 0.7),
                    (0.27, 0.75),
                ]);
            }
            "Window" => {
                line((0.18, 0.2), (0.18, 0.8));
                line((0.32, 0.2), (0.32, 0.8));
                line((0.68, 0.2), (0.68, 0.8));
                line((0.82, 0.2), (0.82, 0.8));
                line((0.18, 0.34), (0.32, 0.34));
                line((0.68, 0.66), (0.82, 0.66));
            }
            "Object" | "3D Document" => {
                closed(&[
                    (0.18, 0.34),
                    (0.5, 0.15),
                    (0.82, 0.34),
                    (0.82, 0.69),
                    (0.5, 0.87),
                    (0.18, 0.69),
                ]);
                line((0.18, 0.34), (0.5, 0.53));
                line((0.82, 0.34), (0.5, 0.53));
                line((0.5, 0.53), (0.5, 0.87));
            }
            "Lamp" => {
                circle(0.5, 0.38, 0.22);
                path(&[(0.34, 0.52), (0.41, 0.65), (0.59, 0.65), (0.66, 0.52)]);
                line((0.4, 0.75), (0.6, 0.75));
                line((0.44, 0.84), (0.56, 0.84));
                for (a, b) in [
                    ((0.5, 0.08), (0.5, 0.02)),
                    ((0.17, 0.2), (0.12, 0.14)),
                    ((0.83, 0.2), (0.88, 0.14)),
                ] {
                    line(a, b);
                }
            }
            "Roof" => {
                path(&[(0.1, 0.66), (0.5, 0.22), (0.9, 0.66)]);
                line((0.22, 0.55), (0.22, 0.82));
                line((0.78, 0.55), (0.78, 0.82));
                line((0.22, 0.82), (0.78, 0.82));
                line((0.5, 0.22), (0.5, 0.52));
            }
            "Shell" => {
                path(&[
                    (0.12, 0.72),
                    (0.2, 0.48),
                    (0.36, 0.3),
                    (0.56, 0.22),
                    (0.78, 0.33),
                    (0.9, 0.62),
                ]);
                path(&[
                    (0.12, 0.72),
                    (0.3, 0.7),
                    (0.48, 0.62),
                    (0.66, 0.47),
                    (0.9, 0.62),
                ]);
                line((0.12, 0.72), (0.9, 0.62));
            }
            "Stair" => path(&[
                (0.13, 0.82),
                (0.13, 0.67),
                (0.31, 0.67),
                (0.31, 0.52),
                (0.49, 0.52),
                (0.49, 0.37),
                (0.67, 0.37),
                (0.67, 0.22),
                (0.87, 0.22),
            ]),
            "Railing" => {
                line((0.16, 0.28), (0.84, 0.28));
                line((0.16, 0.42), (0.84, 0.42));
                line((0.2, 0.42), (0.2, 0.82));
                line((0.4, 0.42), (0.4, 0.82));
                line((0.6, 0.42), (0.6, 0.82));
                line((0.8, 0.42), (0.8, 0.82));
            }
            "Curtain Wall" => {
                box_outline(0.14, 0.15, 0.72, 0.7);
                for x in [0.38, 0.62] {
                    line((x, 0.16), (x, 0.84));
                }
                for y in [0.38, 0.62] {
                    line((0.15, y), (0.85, y));
                }
            }
            "Skylight" => {
                closed(&[(0.12, 0.34), (0.65, 0.17), (0.88, 0.32), (0.35, 0.5)]);
                line((0.35, 0.5), (0.35, 0.78));
                line((0.35, 0.78), (0.88, 0.59));
                line((0.88, 0.32), (0.88, 0.59));
            }
            "Corner-Window" => {
                path(&[(0.18, 0.82), (0.18, 0.22), (0.8, 0.22)]);
                path(&[(0.34, 0.82), (0.34, 0.38), (0.8, 0.38)]);
                line((0.18, 0.54), (0.34, 0.54));
                line((0.57, 0.22), (0.57, 0.38));
            }
            "Opening" => {
                line((0.17, 0.18), (0.83, 0.18));
                line((0.17, 0.82), (0.83, 0.82));
                line((0.17, 0.18), (0.17, 0.82));
                line((0.83, 0.18), (0.83, 0.34));
                line((0.83, 0.66), (0.83, 0.82));
            }
            "Wall End" => {
                line((0.2, 0.2), (0.8, 0.2));
                line((0.2, 0.8), (0.8, 0.8));
                line((0.2, 0.2), (0.2, 0.8));
                line((0.8, 0.2), (0.8, 0.8));
                line((0.62, 0.2), (0.62, 0.8));
            }
            "Mesh" => {
                closed(&[
                    (0.15, 0.37),
                    (0.5, 0.17),
                    (0.85, 0.38),
                    (0.75, 0.78),
                    (0.28, 0.82),
                ]);
                for y in [0.42, 0.58] {
                    line((0.22, y), (0.8, y));
                }
                line((0.5, 0.23), (0.5, 0.8));
                line((0.32, 0.3), (0.28, 0.8));
                line((0.69, 0.3), (0.75, 0.78));
            }
            "Morph" => {
                closed(&[
                    (0.16, 0.3),
                    (0.48, 0.16),
                    (0.82, 0.32),
                    (0.76, 0.72),
                    (0.42, 0.86),
                    (0.17, 0.65),
                ]);
                line((0.16, 0.3), (0.42, 0.48));
                line((0.48, 0.16), (0.42, 0.48));
                line((0.82, 0.32), (0.42, 0.48));
                line((0.76, 0.72), (0.42, 0.48));
                line((0.17, 0.65), (0.42, 0.48));
            }
            "Zone" => {
                closed(&[
                    (0.18, 0.24),
                    (0.74, 0.18),
                    (0.84, 0.64),
                    (0.52, 0.82),
                    (0.16, 0.68),
                ]);
                line((0.31, 0.36), (0.69, 0.32));
                line((0.28, 0.48), (0.73, 0.44));
                line((0.26, 0.6), (0.62, 0.56));
            }
            "Section" => {
                line((0.16, 0.78), (0.84, 0.22));
                line((0.42, 0.52), (0.26, 0.36));
                path(&[(0.68, 0.18), (0.86, 0.16), (0.84, 0.34)]);
            }
            "Elevation" | "Interior Elevation" => {
                line((0.18, 0.78), (0.82, 0.78));
                line((0.5, 0.78), (0.5, 0.28));
                path(&[(0.34, 0.43), (0.5, 0.25), (0.66, 0.43)]);
                line((0.38, 0.56), (0.62, 0.56));
            }
            "Detail" => {
                circle(0.5, 0.5, 0.32);
                line((0.5, 0.18), (0.5, 0.82));
                line((0.18, 0.5), (0.82, 0.5));
                circle(0.5, 0.5, 0.1);
            }
            "Worksheet" | "Figure" | "Drawing" => {
                box_outline(0.2, 0.14, 0.6, 0.72);
                line((0.3, 0.3), (0.7, 0.3));
                line((0.3, 0.42), (0.7, 0.42));
                line((0.3, 0.54), (0.58, 0.54));
                line((0.3, 0.72), (0.7, 0.72));
            }
            "Dimension" | "Level Dimension" | "Radial Dimension" | "Angle Dimension" => {
                line((0.18, 0.68), (0.82, 0.32));
                line((0.2, 0.78), (0.17, 0.58));
                line((0.8, 0.42), (0.83, 0.22));
                line((0.31, 0.63), (0.24, 0.48));
                line((0.69, 0.37), (0.76, 0.52));
            }
            "Text" => {
                line((0.2, 0.27), (0.8, 0.27));
                line((0.5, 0.28), (0.5, 0.8));
            }
            "Label" => {
                path(&[
                    (0.18, 0.24),
                    (0.66, 0.24),
                    (0.84, 0.42),
                    (0.66, 0.6),
                    (0.18, 0.6),
                    (0.18, 0.24),
                ]);
                line((0.3, 0.42), (0.58, 0.42));
                line((0.58, 0.42), (0.8, 0.78));
            }
            "Fill" => {
                box_outline(0.18, 0.18, 0.64, 0.64);
                for i in 0..5 {
                    let n = i as f32 * 0.15;
                    line((0.22 + n, 0.76), (0.76, 0.22 + n));
                }
            }
            "Line" | "Polyline" => path(&[(0.16, 0.76), (0.38, 0.34), (0.56, 0.62), (0.84, 0.2)]),
            "Arc / Circle" => {
                circle(0.5, 0.5, 0.32);
                line((0.5, 0.5), (0.78, 0.32));
            }
            "Spline" => path(&[
                (0.14, 0.7),
                (0.28, 0.28),
                (0.48, 0.68),
                (0.67, 0.32),
                (0.86, 0.57),
            ]),
            "Hotspot" => {
                circle(0.5, 0.5, 0.23);
                line((0.5, 0.08), (0.5, 0.3));
                line((0.5, 0.7), (0.5, 0.92));
                line((0.08, 0.5), (0.3, 0.5));
                line((0.7, 0.5), (0.92, 0.5));
            }
            "Grid Element" => {
                box_outline(0.17, 0.17, 0.66, 0.66);
                for n in [0.38, 0.61] {
                    line((n, 0.18), (n, 0.82));
                    line((0.18, n), (0.82, n));
                }
            }
            "Camera" => {
                box_outline(0.16, 0.28, 0.68, 0.52);
                path(&[(0.35, 0.28), (0.42, 0.17), (0.62, 0.17), (0.69, 0.28)]);
                circle(0.5, 0.54, 0.16);
            }
            "Support" => {
                line((0.5, 0.18), (0.5, 0.82));
                line((0.25, 0.3), (0.75, 0.3));
                line((0.32, 0.82), (0.68, 0.82));
                line((0.38, 0.67), (0.62, 0.67));
            }
            "Link" => {
                circle(0.35, 0.5, 0.19);
                circle(0.65, 0.5, 0.19);
                line((0.43, 0.37), (0.57, 0.63));
                line((0.43, 0.63), (0.57, 0.37));
            }
            "Point Load" | "Line Load" | "Surface Load" => {
                line((0.5, 0.16), (0.5, 0.6));
                path(&[(0.32, 0.43), (0.5, 0.62), (0.68, 0.43)]);
                if name == "Line Load" {
                    line((0.22, 0.76), (0.78, 0.76));
                }
                if name == "Surface Load" {
                    closed(&[(0.2, 0.72), (0.5, 0.58), (0.8, 0.72), (0.5, 0.87)]);
                }
            }
            "Duct Route" | "Pipe Route" | "Cable Route" => {
                path(&[
                    (0.15, 0.72),
                    (0.15, 0.35),
                    (0.52, 0.35),
                    (0.52, 0.65),
                    (0.84, 0.65),
                ]);
                line((0.15, 0.28), (0.15, 0.42));
                line((0.84, 0.58), (0.84, 0.72));
            }
            "Branch" => {
                path(&[(0.5, 0.82), (0.5, 0.26), (0.25, 0.26), (0.25, 0.48)]);
                line((0.5, 0.5), (0.75, 0.5));
                line((0.75, 0.42), (0.75, 0.58));
            }
            "Fitting" => {
                closed(&[(0.5, 0.14), (0.84, 0.5), (0.5, 0.86), (0.16, 0.5)]);
                line((0.16, 0.5), (0.84, 0.5));
                line((0.5, 0.14), (0.5, 0.86));
            }
            "Terminal" => {
                box_outline(0.2, 0.2, 0.6, 0.6);
                line((0.32, 0.38), (0.68, 0.38));
                line((0.32, 0.5), (0.68, 0.5));
                line((0.32, 0.62), (0.68, 0.62));
            }
            "Accessory" | "Equipment" => {
                box_outline(0.23, 0.23, 0.54, 0.54);
                circle(0.5, 0.5, 0.13);
                for (a, b) in [
                    ((0.5, 0.08), (0.5, 0.23)),
                    ((0.5, 0.77), (0.5, 0.92)),
                    ((0.08, 0.5), (0.23, 0.5)),
                    ((0.77, 0.5), (0.92, 0.5)),
                ] {
                    line(a, b);
                }
            }
            "Element Snap" => {
                path(&[(0.18, 0.52), (0.18, 0.18), (0.52, 0.18)]);
                path(&[(0.48, 0.82), (0.82, 0.82), (0.82, 0.48)]);
                line((0.18, 0.82), (0.36, 0.64));
                circle(0.5, 0.5, 0.09);
            }
            "Grid Snap" => {
                for value in [0.24, 0.5, 0.76] {
                    line((value, 0.16), (value, 0.84));
                    line((0.16, value), (0.84, value));
                }
                circle(0.5, 0.5, 0.1);
            }
            "Guide Snap" => {
                for (a, b) in [
                    ((0.14, 0.7), (0.24, 0.7)),
                    ((0.32, 0.7), (0.42, 0.7)),
                    ((0.58, 0.7), (0.68, 0.7)),
                    ((0.76, 0.7), (0.86, 0.7)),
                ] {
                    line(a, b);
                }
                path(&[(0.5, 0.22), (0.5, 0.58)]);
                circle(0.5, 0.22, 0.08);
            }
            "Ortho" => {
                path(&[(0.2, 0.18), (0.2, 0.8), (0.82, 0.8)]);
                box_outline(0.14, 0.74, 0.12, 0.12);
            }
            "Construction Grid" => {
                for value in [0.2, 0.4, 0.6, 0.8] {
                    line((value, 0.12), (value, 0.88));
                }
                for value in [0.2, 0.4, 0.6, 0.8] {
                    line((0.12, value), (0.88, value));
                }
            }
            "Guide Lines" => {
                line((0.18, 0.82), (0.82, 0.18));
                line((0.18, 0.55), (0.55, 0.55));
                line((0.45, 0.32), (0.82, 0.32));
                circle(0.55, 0.55, 0.06);
            }
            "Trace" => {
                box_outline(0.16, 0.16, 0.56, 0.56);
                box_outline(0.32, 0.32, 0.52, 0.52);
                line((0.16, 0.72), (0.32, 0.84));
                line((0.72, 0.16), (0.84, 0.32));
            }
            "All Stories" => {
                closed(&[(0.5, 0.14), (0.82, 0.3), (0.5, 0.46), (0.18, 0.3)]);
                closed(&[(0.5, 0.38), (0.82, 0.54), (0.5, 0.7), (0.18, 0.54)]);
                closed(&[(0.5, 0.62), (0.82, 0.78), (0.5, 0.94), (0.18, 0.78)]);
            }
            "Orientation" => {
                path(&[
                    (0.74, 0.34),
                    (0.68, 0.18),
                    (0.52, 0.12),
                    (0.34, 0.18),
                    (0.22, 0.33),
                    (0.21, 0.52),
                    (0.3, 0.68),
                ]);
                path(&[(0.62, 0.62), (0.55, 0.79), (0.73, 0.78)]);
                line((0.5, 0.5), (0.5, 0.14));
                path(&[(0.4, 0.24), (0.5, 0.13), (0.6, 0.24)]);
            }
            "Scale" => {
                box_outline(0.1, 0.42, 0.8, 0.22);
                for x in [0.22, 0.36, 0.5, 0.64, 0.78] {
                    line((x, 0.42), (x, 0.58));
                }
                line((0.36, 0.58), (0.36, 0.68));
                line((0.64, 0.58), (0.64, 0.68));
            }
            "Layer Combination" => {
                closed(&[(0.5, 0.14), (0.86, 0.33), (0.5, 0.52), (0.14, 0.33)]);
                closed(&[(0.5, 0.32), (0.86, 0.51), (0.5, 0.7), (0.14, 0.51)]);
                closed(&[(0.5, 0.5), (0.86, 0.69), (0.5, 0.88), (0.14, 0.69)]);
            }
            "Partial Structure" => {
                box_outline(0.14, 0.2, 0.72, 0.6);
                line((0.3, 0.2), (0.3, 0.8));
                line((0.14, 0.5), (0.86, 0.5));
                for (x, y) in [(0.4, 0.55), (0.56, 0.55), (0.72, 0.55)] {
                    line((x, y), (x - 0.1, y + 0.18));
                }
            }
            "Pen Set" => {
                path(&[(0.5, 0.12), (0.68, 0.34), (0.62, 0.66), (0.5, 0.88)]);
                path(&[(0.5, 0.12), (0.32, 0.34), (0.38, 0.66), (0.5, 0.88)]);
                line((0.32, 0.4), (0.68, 0.4));
                line((0.5, 0.72), (0.5, 0.88));
            }
            "Model View" => {
                box_outline(0.12, 0.16, 0.76, 0.68);
                box_outline(0.3, 0.34, 0.4, 0.32);
                line((0.12, 0.5), (0.3, 0.5));
                line((0.7, 0.5), (0.88, 0.5));
            }
            "Graphic Overrides" => {
                box_outline(0.12, 0.34, 0.5, 0.5);
                box_outline(0.38, 0.16, 0.5, 0.5);
                path(&[(0.62, 0.07), (0.82, 0.07), (0.82, 0.27)]);
                path(&[(0.7, 0.19), (0.82, 0.07), (0.94, 0.19)]);
            }
            "Renovation" => {
                closed(&[
                    (0.12, 0.82),
                    (0.12, 0.4),
                    (0.5, 0.14),
                    (0.88, 0.4),
                    (0.88, 0.82),
                ]);
                line((0.28, 0.82), (0.28, 0.5));
                line((0.28, 0.5), (0.46, 0.5));
                path(&[(0.62, 0.5), (0.62, 0.74), (0.8, 0.62)]);
            }
            "Design Options" => {
                box_outline(0.12, 0.32, 0.52, 0.52);
                box_outline(0.36, 0.14, 0.52, 0.52);
                circle(0.3, 0.58, 0.05);
                circle(0.62, 0.4, 0.05);
            }
            "Dimension Style" => {
                line((0.12, 0.36), (0.12, 0.72));
                line((0.88, 0.36), (0.88, 0.72));
                line((0.12, 0.48), (0.88, 0.48));
                path(&[(0.2, 0.42), (0.12, 0.48), (0.2, 0.54)]);
                path(&[(0.8, 0.42), (0.88, 0.48), (0.8, 0.54)]);
                line((0.12, 0.3), (0.12, 0.14));
                line((0.88, 0.3), (0.88, 0.14));
            }
            _ => {
                closed(&[(0.5, 0.12), (0.84, 0.5), (0.5, 0.88), (0.16, 0.5)]);
                line((0.3, 0.5), (0.7, 0.5));
            }
        }
    }

    fn toolbox_tool_row(
        ui: &mut Ui,
        name: &str,
        display_name: &str,
        selected: bool,
        shortcut: Option<&str>,
    ) -> bool {
        let height = if shortcut.is_some() { 36. } else { 30. };
        let (r, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        response
            .widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, display_name));
        let p = ui.painter();
        if selected || response.hovered() {
            p.rect_filled(
                r,
                3.,
                if selected {
                    Color32::from_rgb(48, 83, 101)
                } else {
                    PANEL_ALT
                },
            );
        }
        let color = if selected { ACCENT } else { INK };
        Self::paint_tool_icon(
            p,
            Rect::from_min_size(r.left_top() + vec2(9., 6.), vec2(23., 23.)),
            name,
            color,
        );
        p.text(
            r.left_center() + vec2(39., 0.),
            Align2::LEFT_CENTER,
            display_name,
            FontId::proportional(13.),
            color,
        );
        if let Some(key) = shortcut {
            p.text(
                r.right_center() - vec2(8., 0.),
                Align2::RIGHT_CENTER,
                key,
                FontId::monospace(10.),
                MUTED,
            );
        }
        response
            .on_hover_text(if let Some(key) = shortcut {
                format!("{display_name} · {key}")
            } else {
                display_name.to_owned()
            })
            .clicked()
    }
    fn vector_icon_button(ui: &mut Ui, icon: &str, tip: &str, selected: bool) -> bool {
        let (r, response) = ui.allocate_exact_size(vec2(30., 28.), Sense::click());
        if selected || response.hovered() {
            ui.painter().rect_filled(
                r,
                3.,
                if selected {
                    Color32::from_rgb(48, 83, 101)
                } else {
                    PANEL_ALT
                },
            );
        }
        Self::paint_tool_icon(
            ui.painter(),
            Rect::from_center_size(r.center(), vec2(18., 18.)),
            icon,
            if selected { ACCENT } else { INK },
        );
        response.on_hover_text(tip).clicked()
    }
    fn vector_labeled_button(
        ui: &mut Ui,
        icon: &str,
        label: &str,
        tip: &str,
        selected: bool,
    ) -> bool {
        let width = (43. + label.chars().count() as f32 * 7.1)
            .min(ui.available_width())
            .max(44.);
        let (r, response) = ui.allocate_exact_size(vec2(width, 28.), Sense::click());
        if selected || response.hovered() {
            ui.painter().rect_filled(
                r,
                3.,
                if selected {
                    Color32::from_rgb(48, 83, 101)
                } else {
                    PANEL_ALT
                },
            );
        }
        let color = if selected { ACCENT } else { INK };
        Self::paint_tool_icon(
            ui.painter(),
            Rect::from_min_size(r.left_top() + vec2(6., 5.), vec2(18., 18.)),
            icon,
            color,
        );
        ui.painter().text(
            r.left_center() + vec2(29., 0.),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.),
            color,
        );
        response.on_hover_text(tip).clicked()
    }
    fn navigator_item(ui: &mut Ui, icon: &str, label: &str) -> bool {
        let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(r, 2., PANEL_ALT);
        }
        Self::paint_tool_icon(
            ui.painter(),
            Rect::from_min_size(r.left_top() + vec2(5., 4.), vec2(18., 18.)),
            icon,
            INK,
        );
        ui.painter().text(
            r.left_center() + vec2(30., 0.),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.),
            INK,
        );
        response.on_hover_text(label).clicked()
    }
    fn inline_vector_icon(ui: &mut Ui, icon: &str, color: Color32) {
        let (r, _) = ui.allocate_exact_size(vec2(20., 20.), Sense::hover());
        Self::paint_tool_icon(
            ui.painter(),
            Rect::from_min_size(r.left_top() + vec2(1., 1.), vec2(18., 18.)),
            icon,
            color,
        );
    }
    fn place_marquee(&mut self, vertices: Vec<Point>) {
        if let Some(area) = MarqueeArea::new(vertices, self.marquee_scope, self.editor.story) {
            self.editor.marquee = Some(area);
            self.editor.set_selection(Vec::new());
            self.editor.status = format!(
                "Marquee placed · {}",
                if self.marquee_scope == MarqueeScope::AllStories {
                    "all stories"
                } else {
                    "current story"
                }
            );
        } else {
            self.editor.status = "Marquee needs three non-collinear points".into();
        }
        self.marquee_points.clear();
        self.marquee_preview = None;
        self.marquee_3d_preview = None;
        self.marquee_3d_points.clear();
        self.marquee_3d_cursor = None;
    }
    fn place_3d_marquee(&mut self, vertices: Vec<Point>, base_z: f32) {
        self.place_marquee(vertices);
        if let Some(area) = &mut self.editor.marquee {
            area.z_range = Some((base_z, base_z + self.marquee_height));
            self.show_marquee_3d = true;
            self.fit_marquee();
        }
    }
    fn open_element_settings(&mut self) {
        if self.wall_settings_dialog.is_some() {
            return;
        }
        if self.info_box_wall.as_ref().is_some_and(|state| state.dirty)
            && !self.commit_info_box_wall()
        {
            return;
        }
        if self
            .info_box_wall_tool
            .as_ref()
            .is_some_and(|state| state.dirty)
            && !self.commit_info_box_wall_tool()
        {
            return;
        }
        self.info_box_wall = None;
        let selected = self.editor.selection_ids();
        if selected.len() == 1 {
            let element = self
                .editor
                .project
                .elements
                .iter()
                .find(|element| element.id == selected[0])
                .cloned();
            match element {
                Some(element) if matches!(element.shape, Shape::Wall { .. }) => {
                    self.wall_settings_dialog = WallSettingsDialog::selected(&element);
                    self.element_settings = None;
                    self.element_settings_error.clear();
                    self.editor.status = format!(
                        "Wall Settings · {} {:03}",
                        self.language.translate("Wall"),
                        element.id
                    );
                    return;
                }
                Some(element) => {
                    self.element_settings = Some(element);
                    self.wall_settings_dialog = None;
                    self.element_settings_error.clear();
                    if let Some(element) = &self.element_settings {
                        self.editor.status =
                            format!("Element Settings · {} {:03}", element.kind(), element.id);
                    }
                    return;
                }
                None => {
                    self.editor.status =
                        "Element Settings · The selected element no longer exists".into();
                }
            }
        }
        if self.editor.tool == Tool::Wall {
            self.element_settings = None;
            self.wall_settings_dialog = Some(WallSettingsDialog::defaults(
                &self.editor.wall_settings,
                self.editor.thickness,
                self.editor.height,
                self.editor.story,
                self.editor.wall_layer,
                self.editor.wall_renovation,
            ));
            self.editor.status = self
                .language
                .translate("Wall Default Settings · New walls use these values")
                .into();
            return;
        }
        self.editor.status = "Element Settings · Select one element first".into();
    }
    fn open_command_preview(&mut self, command: String) {
        self.command_dialog_tab = 0;
        self.command_dialog_name = command.clone();
        self.command_dialog = Some(command.clone());
        self.editor.status = format!("{command} · configuration preview");
    }
    fn activate_stair_tool(&mut self) {
        self.editor.set_tool(Tool::Stair);
        self.preview_tool = None;
        self.tracker_locked = false;
        self.stair_settings_draft = self.editor.stair_spec.clone();
        self.stair_settings_error.clear();
        self.stair_settings_open = true;
    }
    fn activate_polyline_tool(&mut self) {
        self.editor.set_tool(Tool::Polyline);
        self.preview_tool = None;
        self.tracker_locked = false;
    }
    fn apply_element_settings(&mut self) -> Result<(), String> {
        let element = self
            .element_settings
            .clone()
            .ok_or_else(|| "No element settings are open".to_string())?;
        self.editor.replace_element(element)?;
        self.draft_id = None;
        self.element_settings_error.clear();
        self.editor.status = "Element settings applied · Plan and 3D views synchronized".into();
        Ok(())
    }
    fn screen_to_ground(
        camera: &Camera,
        viewport: Rect,
        pos: Pos2,
        elevation: f32,
    ) -> Option<Point> {
        let ndc = [
            (pos.x - viewport.left()) / viewport.width() * 2. - 1.,
            1. - (pos.y - viewport.top()) / viewport.height() * 2.,
        ];
        camera.point_on_horizontal_plane(ndc, viewport.aspect_ratio(), elevation)
    }
    fn shortcuts(&mut self, ctx: &Context) {
        if self.trace.pointer_mode_active() {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.trace.cancel_pointer_modes();
                self.editor.status = "Trace pointer mode cancelled".into();
            }
            return;
        }
        if self.orientation_pick_active && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.cancel_orientation_pick();
            return;
        }
        if self.rotated_grid_tool.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.rotated_grid_tool = None;
            self.editor.status = "Set Rotated Grid cancelled".into();
            return;
        }
        if self.guide_shortcuts(ctx) {
            return;
        }
        if self.wall_settings_dialog.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.wall_settings_dialog = None;
            self.editor.status = self.language.translate("Wall Settings cancelled").into();
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            let operation_in_progress = self.editor.anchor.is_some()
                || !self.editor.polyline_points.is_empty()
                || !self.editor.wall_points.is_empty()
                || !self.marquee_points.is_empty()
                || self.marquee_preview.is_some()
                || self.marquee_3d_preview.is_some()
                || !self.marquee_3d_points.is_empty()
                || self.drag.is_some()
                || self.polyline_edit.is_some()
                || self.wall_edit.is_some()
                || self.stair_settings_open
                || self.element_settings.is_some()
                || self.wall_settings_dialog.is_some()
                || self.command_dialog.is_some()
                || self.orientation_pick_active
                || self.plan_option_dialog.is_some()
                || self.file_action.is_some();
            if operation_in_progress {
                if let Some((before, _, _)) = self.drag.take() {
                    self.editor.project = before;
                }
                if let Some(edit) = self.polyline_edit.take()
                    && let Some((before, _)) = edit.drag
                {
                    self.editor.project = before;
                }
                if let Some(edit) = self.wall_edit.take()
                    && let Some((before, _)) = edit.drag
                {
                    self.editor.project = before;
                }
                self.editor.cancel_wall_draft();
                self.editor.cancel_polyline();
                self.marquee_points.clear();
                self.marquee_preview = None;
                self.marquee_3d_preview = None;
                self.marquee_3d_points.clear();
                self.marquee_3d_cursor = None;
                self.stair_settings_open = false;
                self.element_settings = None;
                self.element_settings_error.clear();
                self.wall_settings_dialog = None;
                self.command_dialog = None;
                self.orientation_pick_active = false;
                self.orientation_pick_start = None;
                self.plan_option_dialog = None;
                self.file_action = None;
                self.file_error.clear();
                self.tracker_locked = false;
            }
            if self.editor.tool != Tool::Select || self.preview_tool.is_some() {
                let restored = self.editor.return_to_previous_tool();
                self.editor.status = format!("Command cancelled · {restored:?} restored");
                self.preview_tool = None;
                self.tracker_locked = false;
                return;
            }
            if operation_in_progress {
                return;
            }
            if !self.editor.selected_ids.is_empty() {
                self.editor.set_selection(Vec::new());
                self.editor.status = "Selection cleared".into();
                return;
            }
            if self.editor.marquee.is_some() {
                self.editor.remove_marquee();
                self.marquee_points.clear();
                self.marquee_preview = None;
                return;
            }
            self.tracker_locked = false;
            self.preview_tool = None;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::S)) {
            self.toggle_grid_snap();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::S)) {
            let settings = &mut self.editor.project.grid_settings;
            settings.snap_mode = match settings.snap_mode {
                crate::grids::GridSnapMode::None => crate::grids::GridSnapMode::SnapGrid,
                crate::grids::GridSnapMode::SnapGrid => {
                    crate::grids::GridSnapMode::ConstructionGrid
                }
                crate::grids::GridSnapMode::ConstructionGrid => crate::grids::GridSnapMode::None,
            };
            settings.grid_snap_enabled =
                !matches!(settings.snap_mode, crate::grids::GridSnapMode::None);
            self.editor.status = self
                .language
                .translate(settings.snap_mode.label_key())
                .into();
        }
        if !self.editor.selected_ids.is_empty()
            && self.editor.anchor.is_none()
            && self.editor.polyline_points.is_empty()
            && self.drag.is_none()
        {
            let large = ctx.input(|i| i.modifiers.shift);
            let settings = self.editor.project.grid_settings.clone();
            let mut nudge = |dx: f32, dy: f32| {
                self.editor.move_selection(Point::new(dx, dy));
                self.editor.status = "Selection nudged".into();
            };
            for (key, dx, dy) in [
                (Key::ArrowLeft, -1.0, 0.0),
                (Key::ArrowRight, 1.0, 0.0),
                (Key::ArrowUp, 0.0, 1.0),
                (Key::ArrowDown, 0.0, -1.0),
            ] {
                let modifiers = if large {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                };
                if ctx.input_mut(|i| i.consume_key(modifiers, key)) {
                    let x = if dx == 0.0 {
                        0.0
                    } else {
                        settings.nudge_distance(true, large) * dx
                    };
                    let y = if dy == 0.0 {
                        0.0
                    } else {
                        settings.nudge_distance(false, large) * dy
                    };
                    nudge(x, y);
                }
            }
        }
        if self.editor.tool == Tool::Wall
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
        {
            match self.editor.wall_settings.geometry.geometry_method {
                WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal => {
                    if self.tracker_locked {
                        if let Some(anchor) = self.editor.anchor {
                            let point = self.tracker_target(anchor);
                            self.editor.click(point, 0.);
                        }
                    } else if let Err(error) = self.editor.finish_wall(None) {
                        self.editor.status = localized_error(self.language, &error);
                    }
                }
                WallGeometryMethod::Single | WallGeometryMethod::Rectangular => {
                    if let Some(anchor) = self.editor.anchor {
                        let point = self.tracker_target(anchor);
                        if let Err(error) = self.editor.finish_wall(Some(point)) {
                            self.editor.status = localized_error(self.language, &error);
                        }
                    }
                }
            }
            self.tracker_locked = false;
            self.draft_id = None;
        }
        if self.editor.tool == Tool::Polyline
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
        {
            if self.tracker_locked {
                if let Some(anchor) = self.editor.polyline_points.last().copied() {
                    let point = self.tracker_target(anchor);
                    self.commit_polyline_point(point, 0.01);
                }
            } else {
                self.finish_polyline();
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::T)) {
            self.open_element_settings();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            self.file_action = Some(FileAction::Save);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.file_action = Some(FileAction::Open);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)) {
            self.editor.redo();
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
            self.editor.undo();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D)) {
            self.editor.duplicate();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::C)) {
            self.editor.copy_selection();
            self.editor.status = format!("Copied {} element(s)", self.editor.clipboard.len());
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::X)) {
            let count = self.editor.cut_selection();
            self.editor.status = format!("Cut {count} element(s)");
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::V)) {
            let count = self.editor.paste(Point::new(0.5, 0.5));
            self.editor.status = format!("Pasted {count} element(s)");
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::A))
            && self.editor.tool == Tool::Marquee
        {
            let count = self.editor.select_all_in_marquee();
            self.editor.status = format!("Selected {count} element(s) in Marquee");
        }
        if ctx.input(|i| i.modifiers.command) {
            return;
        }
        for (key, t) in [
            (Key::V, Tool::Select),
            (Key::M, Tool::Marquee),
            (Key::W, Tool::Wall),
            (Key::L, Tool::Slab),
            (Key::P, Tool::Polyline),
        ] {
            if ctx.input(|i| i.key_pressed(key)) {
                self.editor.set_tool(t);
                self.preview_tool = None;
                self.tracker_locked = false;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::S)) {
            self.activate_stair_tool();
        }
        if self.editor.wall_draft_active()
            && ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace))
        {
            if ctx.input(|i| i.key_pressed(Key::Delete)) {
                self.editor.cancel_wall_draft();
                self.editor.status = "Wall cancelled".into();
            } else {
                self.editor.backspace_wall();
            }
            self.tracker_locked = false;
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.editor.delete();
        }
        if ctx.input(|i| i.key_pressed(Key::F)) {
            self.fit();
        }
        if ctx.input(|i| i.key_pressed(Key::F2)) {
            self.view = View::Plan;
            self.editor.status = "Floor plan · F2".into();
        }
        if ctx.input(|i| i.key_pressed(Key::F3)) {
            self.view = View::Model;
            self.editor.status = "Perspective 3D · F3".into();
        }
        if ctx.input(|i| i.key_pressed(Key::F5)) {
            let ids = self.editor.selection_ids();
            if self.editor.marquee.is_some() {
                self.show_marquee_3d = true;
                self.marquee_outside = false;
                self.fit_marquee();
                self.editor.status = "Showing clipped Marquee volume in perspective 3D · F5".into();
            } else if !ids.is_empty() {
                self.show_marquee_3d = true;
                self.fit_selection();
                self.editor.status = "Showing selected elements in perspective 3D · F5".into();
            } else {
                self.editor.status = "Select elements or define a Marquee before F5".into();
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Num1)) {
            self.view = View::Plan
        }
        if ctx.input(|i| i.key_pressed(Key::Num2)) {
            self.view = View::Split
        }
        if ctx.input(|i| i.key_pressed(Key::Num3)) {
            self.view = View::Model
        }
    }
    fn fit(&mut self) {
        let pts: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == self.editor.story)
            .flat_map(|e| geometry::element_footprint(&self.editor.project, e))
            .collect();
        if pts.is_empty() {
            self.plan_center = Point::new(0., 0.);
            self.camera = Camera::default();
            return;
        }
        let min = pts
            .iter()
            .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, b| {
                Point::new(a.x.min(b.x), a.y.min(b.y))
            });
        let max = pts
            .iter()
            .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| {
                Point::new(a.x.max(b.x), a.y.max(b.y))
            });
        self.plan_center = Point::new((min.x + max.x) / 2., (min.y + max.y) / 2.);
        self.plan_scale = (420. / (max.x - min.x).max(max.y - min.y).max(5.)).clamp(4., 140.);
        self.camera.target = glam::Vec3::new(
            self.plan_center.x,
            self.plan_center.y,
            self.editor.project.elevation(self.editor.story) + 1.,
        );
        self.camera.span = ((max.x - min.x).max(max.y - min.y) * 1.7).max(8.);
    }
    fn fit_marquee(&mut self) {
        let Some(area) = &self.editor.marquee else {
            return;
        };
        if area.vertices.is_empty() {
            return;
        }
        let min = area
            .vertices
            .iter()
            .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| {
                Point::new(a.x.min(p.x), a.y.min(p.y))
            });
        let max = area
            .vertices
            .iter()
            .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| {
                Point::new(a.x.max(p.x), a.y.max(p.y))
            });
        let z = area
            .z_range
            .map_or(self.editor.project.elevation(area.story) + 1., |(a, b)| {
                (a + b) * 0.5
            });
        self.camera.target = glam::Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, z);
        self.camera.span = ((max.x - min.x).max(max.y - min.y) * 1.7).max(6.);
        self.view = View::Model;
    }
    fn fit_selection(&mut self) {
        let ids = self.editor.selection_ids();
        let elements: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .collect();
        if elements.is_empty() {
            return;
        }
        let points: Vec<_> = elements
            .iter()
            .flat_map(|e| geometry::footprint(e))
            .collect();
        let min = points
            .iter()
            .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| {
                Point::new(a.x.min(p.x), a.y.min(p.y))
            });
        let max = points
            .iter()
            .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| {
                Point::new(a.x.max(p.x), a.y.max(p.y))
            });
        let z: Vec<f32> = elements
            .iter()
            .flat_map(|e| geometry::corners(&self.editor.project, e).map(|v| v.z))
            .collect();
        let low = z.iter().copied().fold(f32::INFINITY, f32::min);
        let high = z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        self.camera.target = glam::Vec3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (low + high) * 0.5,
        );
        self.camera.span = ((max.x - min.x).max(max.y - min.y).max(high - low) * 1.7).max(6.);
        self.view = View::Model;
    }
    fn workspace(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.handle_close_request(&ctx);
        self.autosave_recovery(&ctx);
        if self.file_action.is_none()
            && !self.help
            && !self.close_prompt
            && self.recovery_prompt.is_none()
            && self.element_settings.is_none()
            && !self.stair_settings_open
            && self.grid_dialog.is_none()
        {
            self.shortcuts(&ctx);
        }
        self.top(ui);
        egui::Panel::bottom("status")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(10, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.language.translate("READY")).color(ACCENT));
                    ui.label(RichText::new(self.status_text()).size(11.));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dimension_style = self.editor.project.plan_options.dimension_style;
                        ui.label(
                            RichText::new(format!(
                                "X {}   Y {}   |   {}",
                                dimension_style.format_signed_length(self.hover_point.x),
                                dimension_style.format_signed_length(self.hover_point.y),
                                self.element_count_label(self.editor.project.elements.len())
                            ))
                            .monospace()
                            .size(10.)
                            .color(MUTED),
                        );
                    });
                });
            });
        if self.quick_options_visible {
            self.quick_options_bar(ui);
        }
        if self.show_left {
            self.toolbox(ui);
        }
        if self.show_right {
            self.inspector(ui);
        }
        CentralPanel::default().frame(Frame::NONE).show(ui, |ui| {
            self.tracker(ui);
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        Self::vector_icon_button(
                            ui,
                            "Navigator",
                            &Self::t(self.language, "Open Navigator"),
                            false,
                        );
                        ui.separator();
                        let floor_label = format!(
                            "F2 · {:02} · {}",
                            self.editor.story,
                            self.editor
                                .project
                                .stories
                                .iter()
                                .find(|s| s.id == self.editor.story)
                                .map_or_else(
                                    || Self::t(self.language, "Floor plan"),
                                    |s| s.name.clone()
                                )
                        );
                        let _ = Self::vector_labeled_button(
                            ui,
                            "Floor Plan",
                            &floor_label,
                            "Floor Plan · F2",
                            self.view == View::Plan,
                        )
                        .then(|| self.view = View::Plan);
                        let _ = Self::vector_labeled_button(
                            ui,
                            "Split",
                            &Self::t(self.language, "Split"),
                            &Self::t(self.language, "Split view"),
                            self.view == View::Split,
                        )
                        .then(|| self.view = View::Split);
                        let _ = Self::vector_labeled_button(
                            ui,
                            "Perspective",
                            &format!("F3 · {}", Self::t(self.language, "3D Perspective")),
                            &format!("{} · F3", Self::t(self.language, "3D Perspective")),
                            self.view == View::Model,
                        )
                        .then(|| self.view = View::Model);
                        let _ = Self::vector_labeled_button(
                            ui,
                            "Elevation View",
                            &Self::t(self.language, "South Elevation"),
                            &Self::t(self.language, "South Elevation view preview"),
                            false,
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(
                                    self.element_count_label(self.editor.project.elements.len()),
                                )
                                .small()
                                .color(MUTED),
                            );
                        });
                    });
                });
            let r = ui.available_rect_before_wrap();
            match self.view {
                View::Plan => self.plan(ui),
                View::Model => self.model_view(ui),
                View::Split => {
                    let x = r.left() + r.width() * self.split;
                    let handle =
                        Rect::from_min_max(pos2(x - 3., r.top()), pos2(x + 3., r.bottom()));
                    let resp = ui.interact(handle, Id::new("view_split"), Sense::drag());
                    if resp.dragged() {
                        self.split = (self.split + ui.input(|i| i.pointer.delta().x) / r.width())
                            .clamp(0.25, 0.75);
                    }
                    resp.on_hover_cursor(CursorIcon::ResizeHorizontal);
                    let left = Rect::from_min_max(r.min, pos2(x - 3., r.bottom()));
                    let right = Rect::from_min_max(pos2(x + 3., r.top()), r.max);
                    ui.scope_builder(UiBuilder::new().max_rect(left), |ui| self.plan(ui));
                    ui.scope_builder(UiBuilder::new().max_rect(right), |ui| self.model_view(ui));
                    ui.painter().rect_filled(handle, 0., LINE);
                }
            }
        });
        self.file_window(&ctx);
        self.recovery_dialog(&ctx);
        self.close_window(&ctx);
        self.plan_options_dialog(&ctx);
        self.quick_options_palette(&ctx);
        self.element_settings_dialog(&ctx);
        self.wall_settings_dialog(&ctx);
        self.stair_settings_dialog(&ctx);
        self.command_preview_dialog(&ctx);
        self.guides_dialog(&ctx);
        self.grids_dialog(&ctx);
        self.trace_palette(&ctx);
        self.trace_browse_dialog(&ctx);
        Window::new(Self::t(self.language, "About OpenArchi")).open(&mut self.help).default_width(460.).show(&ctx, |ui| {
            if self.language == UiLanguage::SimplifiedChinese {
                ui.heading(self.language.translate("OpenArchi · 建筑设计工作台"));
                ui.label(self.language.translate("版本 0.1 · 原生 Rust 桌面应用"));
                ui.label(self.language.translate("在平面图中绘制墙体、矩形楼板和二维多段线。可在二维与三维视图中选择并编辑构件，尺寸单位为米。"));
                ui.separator();
                for text in ["V 选择 · M 选区框 · W 墙 · L 楼板 · P 多段线 · S 楼梯", "多段线：逐点绘制，按 Enter 完成，Esc 取消并返回上一个工具，再按 Esc 回到选择", "楼梯：设置参数后在平面点击起点和终点", "Esc：取消当前操作并返回上一个工具；再次按 Esc 回到选择", "F2 楼层平面 · F3 三维透视", "选区框支持矩形、旋转矩形和多边形", "F5 在三维中显示选择构件或选区框裁剪体", "⌘A 选择选区框内的构件节点", "⌘Z 撤销 · ⇧⌘Z 重做 · ⌘D 复制", "⌘S 保存 JSON · ⌘O 打开 JSON"] { ui.label(text); }
                ui.separator();
                ui.label(format!("渲染器：{}", self.gpu_name));
                ui.label(self.language.translate("原型限制：暂不支持 PDF/DWG 导出和原生 BIM 文件解析。当前助手为本地规则演示。"));
            } else {
                ui.heading(self.language.translate("OpenArchi · Architectural Design Workbench"));
                ui.label(self.language.translate("Version 0.1 · Native Rust desktop application"));
                ui.label(self.language.translate("Draw walls, rectangular slabs and 2D polylines in plan. Select and edit elements in plan; walls and slabs are also shown in 3D. All dimensions are metres."));
                ui.separator();
                for text in ["V · Select     M · Marquee     W · Wall     L · Slab     P · Polyline     S · Stair", "Polyline: left-click nodes, right-click to finish, click start to close, Esc cancels and returns to previous tool; press Esc again for Select", "Select a Polyline, then click a node or edge for the edit palette; drag to preview, release to commit", "Node palette: move, chamfer, fillet, move, rotate, mirror, multiply; edge palette: scale, move, rotate, mirror, multiply", "Esc · Cancel current command → previous tool → Select", "F2 · Floor plan     F3 · Perspective 3D", "Marquee: rectangle, rotated rectangle or polygon", "F5 · Show selected elements or the clipped Marquee volume in 3D", "⌘A · Select elements with nodes inside Marquee", "Cmd/Ctrl+X/C/V · Cut / Copy / Paste selected elements", "Marquee controls: move area, stretch nodes, mirror, rotate, story", "Click start → click end, or type in Tracker → Create", "Drag selected elements in plan to move them", "Shift / Ortho · constrain walls to X or Y", "Right-drag · pan     Scroll · zoom     F · fit", "3D: Shift+middle-drag orbit, right-drag pan, click select", "⌘Z · Undo     ⇧⌘Z · Redo     ⌘D · Duplicate", "Delete · Remove", "⌘S · Save JSON     ⌘O · Open JSON"] { ui.label(text); }
                ui.separator();
                ui.label(format!("Renderer: {}", self.gpu_name));
                ui.label(self.language.translate("Prototype limits: outside-volume filtering, PDF/DWG export and native BIM file parsing are not implemented. Local assistant is a rule-based demonstration."));
            }
        });
    }
}

impl eframe::App for AtelierApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.workspace(ui);
        self.apply_polyline_edit_cursor(ui.ctx());
    }
}

include!("../guides_ui.rs");
include!("../grids_ui.rs");
include!("../trace_ui.rs");
