use crate::{
    cursor::CursorFeedback,
    editor::{Editor, MarqueeArea, MarqueeGeometry, MarqueeScope, Tool},
    geometry::{self, Camera, SnapKind},
    localization::UiLanguage,
    model::{Element, Material, Point, Project, Shape, Story},
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
    file_action: Option<FileAction>,
    path: String,
    file_error: String,
    library_search: String,
    assistant_input: String,
    messages: Vec<(bool, String)>,
    help: bool,
    gpu_name: String,
    language: UiLanguage,
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
        app
    }
    fn from_renderer(renderer: Option<SceneRenderer>, gpu_name: String) -> Self {
        let directory = std::env::current_dir()
            .ok()
            .filter(|p| p.parent().is_some())
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("Documents")
            });
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
            stair_settings_open: false, stair_settings_draft: crate::stairs::StairSpec::default(), stair_settings_error: String::new(),
            command_dialog: None, command_dialog_tab: 0, command_dialog_name: String::new(),
            command_dialog_number: 1., command_dialog_enabled: true,
            command_dialog_method: "Default".into(), command_dialog_reference: "Center".into(),
            command_dialog_representation: "By Element".into(), command_dialog_classification: "Not assigned".into(),
            drag: None, polyline_edit: None, file_action: None,
            path: directory.join("courtyard.openarchi.json").display().to_string(),
            file_error: String::new(), library_search: String::new(), assistant_input: String::new(),
            messages: vec![(false, "Your design companion\n\nAsk for a project summary, quantities, or a model check. This demo runs local rules; no AI service is connected.".into())],
            help: false, gpu_name, language: UiLanguage::English,
        }
    }

    fn style(ctx: &Context) {
        Self::install_cjk_fallback(ctx);
        let mut s = Style::default();
        s.visuals = Visuals::dark();
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
        let candidates = [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/Library/Fonts/NotoSansCJKsc-Regular.otf",
            "/Library/Fonts/NotoSansCHI-Regular.ttf",
            "/Library/Fonts/NotoSansCHI-Bold.ttf",
        ];
        let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
            return;
        };
        let mut fonts = FontDefinitions::default();
        let mut cjk = FontData::from_owned(bytes);
        cjk.index = 0;
        fonts.font_data.insert("OpenArchi-CJK".into(), cjk.into());
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            if let Some(order) = fonts.families.get_mut(&family) {
                order.insert(1.min(order.len()), "OpenArchi-CJK".into());
            }
        }
        ctx.set_fonts(fonts);
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
        if let Some(rest) = status.strip_prefix("Element Settings · ") {
            if rest.chars().last().is_some_and(|c| c.is_ascii_digit()) {
                if let Some((kind, id)) = rest.rsplit_once(' ') {
                    if id.chars().all(|c| c.is_ascii_digit()) {
                        return format!(
                            "{} · {} {}",
                            self.language.translate("Element Settings"),
                            self.language.translate(kind),
                            id
                        );
                    }
                }
            }
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
        if let Some((command, rest)) = status.split_once(" · ") {
            if rest == "configuration preview" || rest == "preview values applied locally" {
                return format!(
                    "{} · {}",
                    self.language.translate(command),
                    self.language.translate(rest)
                );
            }
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
    fn selected_single_wall(&self) -> Option<Element> {
        let selected = self.editor.selection_ids();
        if selected.len() != 1 {
            return None;
        }
        self.editor
            .project
            .elements
            .iter()
            .find(|element| {
                element.id == selected[0] && matches!(element.shape, Shape::Wall { .. })
            })
            .cloned()
    }
    fn sync_info_box_wall(&mut self) {
        let selected_id = self.selected_single_wall().map(|element| element.id);
        if let Some(state) = &self.info_box_wall {
            if state.dirty && Some(state.element_id) != selected_id {
                // Archicad's Info Box applies edits as they are made. If the
                // selection changes while a field still has focus, apply the
                // pending wall before loading the next selection.
                if !self.commit_info_box_wall() {
                    return;
                }
            }
        }
        let Some(element) = self.selected_single_wall() else {
            self.info_box_wall = None;
            return;
        };
        let Some(current) = WallSettingsDraft::from_element(&element) else {
            self.info_box_wall = None;
            return;
        };
        let refresh = match &self.info_box_wall {
            Some(state) => {
                state.element_id != element.id || (!state.dirty && state.draft != current)
            }
            None => true,
        };
        if refresh {
            self.info_box_wall = Some(InfoBoxWallState {
                element_id: element.id,
                draft: current,
                dirty: false,
                error: String::new(),
            });
        }
    }
    fn commit_info_box_wall(&mut self) -> bool {
        let Some(mut state) = self.info_box_wall.take() else {
            return false;
        };
        let Some(mut element) = self
            .editor
            .project
            .elements
            .iter()
            .find(|element| element.id == state.element_id)
            .cloned()
        else {
            state.dirty = true;
            state.error = self
                .language
                .translate("The selected wall no longer exists")
                .into();
            self.info_box_wall = Some(state);
            return false;
        };
        match state
            .draft
            .apply_to_element(&mut element)
            .and_then(|()| self.editor.replace_element(element))
        {
            Ok(()) => {
                if let Some(updated) = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|element| element.id == state.element_id)
                {
                    if let Some(draft) = WallSettingsDraft::from_element(updated) {
                        state.draft = draft;
                    }
                }
                state.dirty = false;
                state.error.clear();
                self.editor.status = self
                    .language
                    .translate("Info Box wall settings applied · Plan and 3D synchronized")
                    .into();
                self.info_box_wall = Some(state);
                true
            }
            Err(error) => {
                state.dirty = true;
                state.error = localized_error(self.language, &error);
                self.info_box_wall = Some(state);
                false
            }
        }
    }
    fn apply_wall_default_draft(&mut self, draft: &WallSettingsDraft) {
        self.editor.wall_settings = draft.settings.clone();
        self.editor.thickness = draft.thickness;
        self.editor.height = draft.height;
        self.editor.wall_layer = draft.layer;
        self.editor.wall_renovation = draft.renovation;
        if self
            .editor
            .project
            .stories
            .iter()
            .any(|story| story.id == draft.story)
        {
            self.editor.story = draft.story;
        }
        self.editor.refresh_wall_status();
    }
    fn sync_info_box_wall_tool(&mut self) {
        let active = self.editor.tool == Tool::Wall && self.editor.selection_ids().is_empty();
        if !active {
            if self
                .info_box_wall_tool
                .as_ref()
                .is_some_and(|state| state.dirty)
            {
                let _ = self.commit_info_box_wall_tool();
            }
            self.info_box_wall_tool = None;
            return;
        }
        let current = WallSettingsDraft::defaults(
            &self.editor.wall_settings,
            self.editor.thickness,
            self.editor.height,
            self.editor.story,
            self.editor.wall_layer,
            self.editor.wall_renovation,
        );
        let refresh = match &self.info_box_wall_tool {
            Some(state) => !state.dirty && state.draft != current,
            None => true,
        };
        if refresh {
            self.info_box_wall_tool = Some(InfoBoxWallToolState {
                draft: current,
                dirty: false,
                error: String::new(),
            });
        }
    }
    fn commit_info_box_wall_tool(&mut self) -> bool {
        let Some(mut state) = self.info_box_wall_tool.take() else {
            return false;
        };
        match state.draft.validate() {
            Ok(()) => {
                self.apply_wall_default_draft(&state.draft);
                state.dirty = false;
                state.error.clear();
                self.editor.status = self
                    .language
                    .translate("Wall default settings applied · New walls use these values")
                    .into();
                self.info_box_wall_tool = Some(state);
                true
            }
            Err(error) => {
                state.dirty = true;
                state.error = localized_error(self.language, &error);
                self.info_box_wall_tool = Some(state);
                false
            }
        }
    }
    fn toggle_info_box_visibility(&mut self) {
        if self.info_box_visible
            && self.info_box_wall.as_ref().is_some_and(|state| state.dirty)
            && !self.commit_info_box_wall()
        {
            return;
        }
        if self.info_box_visible
            && self
                .info_box_wall_tool
                .as_ref()
                .is_some_and(|state| state.dirty)
            && !self.commit_info_box_wall_tool()
        {
            return;
        }
        self.info_box_visible = !self.info_box_visible;
    }
    fn open_info_box_wall_settings(&mut self, floor_plan: bool) {
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
        if self.editor.tool == Tool::Wall && self.selected_single_wall().is_none() {
            let mut dialog = WallSettingsDialog::defaults(
                &self.editor.wall_settings,
                self.editor.thickness,
                self.editor.height,
                self.editor.story,
                self.editor.wall_layer,
                self.editor.wall_renovation,
            );
            if floor_plan {
                dialog.sections.floor_plan = true;
            }
            self.wall_settings_dialog = Some(dialog);
            self.element_settings = None;
            self.element_settings_error.clear();
            self.editor.status = self
                .language
                .translate("Wall Default Settings · New walls use these values")
                .into();
            return;
        }
        let Some(element) = self.selected_single_wall() else {
            return;
        };
        let Some(mut dialog) = WallSettingsDialog::selected(&element) else {
            return;
        };
        if floor_plan {
            dialog.sections.floor_plan = true;
        }
        self.wall_settings_dialog = Some(dialog);
        self.element_settings = None;
        self.element_settings_error.clear();
        self.editor.status = format!(
            "Wall Settings · {} {:03}",
            self.language.translate("Wall"),
            element.id
        );
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
                if let Some(edit) = self.polyline_edit.take() {
                    if let Some((before, _)) = edit.drag {
                        self.editor.project = before;
                    }
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
            .flat_map(geometry::footprint)
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
    fn top(&mut self, ui: &mut Ui) {
        egui::Panel::top("title")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(14, 7)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.language.translate("OpenArchi"))
                            .size(16.)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(24.);
                    ui.label(
                        RichText::new(&self.editor.project.name)
                            .color(Color32::from_rgb(210, 220, 218)),
                    );
                    if self.editor.dirty() {
                        let (dot, _) = ui.allocate_exact_size(vec2(8., 8.), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 2.5, ACCENT);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let previous_language = self.language;
                        ComboBox::from_id_salt("ui_language")
                            .selected_text(self.language.label())
                            .width(112.)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.language,
                                    UiLanguage::English,
                                    "English",
                                );
                                ui.selectable_value(
                                    &mut self.language,
                                    UiLanguage::SimplifiedChinese,
                                    "简体中文",
                                );
                            });
                        if self.language != previous_language {
                            self.language.save_preference();
                            self.editor.status = format!(
                                "{} · {}",
                                self.language.translate("Language"),
                                self.language.label()
                            );
                        }
                        ui.label(
                            RichText::new(if self.language == UiLanguage::SimplifiedChinese {
                                "建筑设计工作台   ·   0.1"
                            } else {
                                "ARCHITECTURAL DESIGN   ·   0.1"
                            })
                            .size(10.)
                            .color(Color32::from_rgb(145, 177, 174)),
                        );
                    });
                });
            });
        egui::Panel::top("commands")
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.menu_button(Self::t(self.language, "OpenArchi"), |ui| {
                        if ui
                            .button(Self::t(self.language, "About OpenArchi"))
                            .clicked()
                        {
                            self.help = true;
                            ui.close();
                        }
                        ui.menu_button(self.language.translate("Project Preferences"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Working Units…",
                                    "Dimensions…",
                                    "Calculation Units…",
                                    "Zones…",
                                    "Reference Levels…",
                                    "Location Settings…",
                                    "Project North…",
                                    "Survey Point…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Work Environment"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Appearance…",
                                    "Keyboard Shortcuts…",
                                    "Dialog Boxes and Palettes…",
                                    "Toolbox…",
                                    "Info Box…",
                                    "Toolbars…",
                                    "Menus…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Services"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Look Up Selection",
                                    "Speech",
                                    "Spelling and Grammar",
                                    "Substitutions",
                                    "Transformations",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.separator();
                        ui.label(
                            RichText::new(
                                self.language
                                    .translate("Native Rust · Apple Silicon / Metal"),
                            )
                            .small()
                            .color(MUTED),
                        );
                    });
                    ui.menu_button(Self::t(self.language, "File"), |ui| {
                        ui.menu_button(self.language.translate("New"), |ui| {
                            if ui
                                .button(self.language.translate("New Empty Project…"))
                                .clicked()
                            {
                                self.file_action = Some(FileAction::New);
                                ui.close();
                            }
                            if ui
                                .button(self.language.translate("New & Reset All…"))
                                .clicked()
                            {
                                self.file_action = Some(FileAction::New);
                                ui.close();
                            }
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &["Share as Teamwork Project…"],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        if ui
                            .button(format!("{}     ⌘O", Self::t(self.language, "Open JSON…")))
                            .clicked()
                        {
                            self.file_action = Some(FileAction::Open);
                            ui.close();
                        }
                        if ui
                            .button(format!("{}      ⌘S", Self::t(self.language, "Save JSON…")))
                            .clicked()
                        {
                            self.file_action = Some(FileAction::Save);
                            ui.close();
                        }
                        for command in [
                            "Save As…",
                            "Save as Template…",
                            "Save Selection as Module…",
                            "Save to BIMcloud…",
                            "Publish…",
                        ] {
                            if ui.button(self.language.translate(command)).clicked() {
                                self.open_command_preview(command.to_owned());
                                ui.close();
                            }
                        }
                        ui.menu_button(self.language.translate("Recent Projects"), |ui| {
                            if ui.button(&self.editor.project.name).clicked() {
                                ui.close();
                            }
                            ui.label(
                                RichText::new(
                                    self.language
                                        .translate("No other recent OpenArchi projects"),
                                )
                                .small()
                                .color(MUTED),
                            );
                        });
                        ui.separator();
                        ui.menu_button(self.language.translate("Interoperability"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Merge…",
                                    "IFC Project Manager…",
                                    "IFC Translators…",
                                    "Import / Export IFC…",
                                    "SAF Translators…",
                                    "DXF-DWG Translation Setup…",
                                    "Classifications & Properties…",
                                    "Import Point Cloud…",
                                    "Place Mesh from Survey Data…",
                                    "Grasshopper Connection…",
                                    "Bimplus Connection…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("External Content"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Hotlink Manager…",
                                    "Place External Drawing…",
                                    "Drawing Manager…",
                                    "Xref Manager…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Libraries and Objects"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Library Manager…",
                                    "BIMcloud Libraries…",
                                    "New Object…",
                                    "Open Object…",
                                    "Save Selection as Object…",
                                    "Import Blocks…",
                                    "Import RFA…",
                                    "Import Rhino / OBJ / FBX…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Info"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Project Info…",
                                    "Action Center…",
                                    "Project Notes…",
                                    "Preview and Credits…",
                                    "Session Report…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Plot and Print"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &["Plot Setup…", "Plot…", "Page Setup…", "Print…"],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.separator();
                        if ui
                            .button(Self::t(self.language, "About OpenArchi"))
                            .clicked()
                        {
                            self.help = true;
                            ui.close();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Edit"), |ui| {
                        if ui
                            .add_enabled(
                                !self.editor.undo.is_empty(),
                                Button::new(format!("{}  ⌘Z", Self::t(self.language, "Undo"))),
                            )
                            .clicked()
                        {
                            self.editor.undo();
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                !self.editor.redo.is_empty(),
                                Button::new(format!("{}  ⇧⌘Z", Self::t(self.language, "Redo"))),
                            )
                            .clicked()
                        {
                            self.editor.redo();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(self.language.translate("Cut  ⌘X")).clicked() {
                            self.editor.cut_selection();
                            ui.close();
                        }
                        if ui.button(self.language.translate("Copy  ⌘C")).clicked() {
                            self.editor.copy_selection();
                            ui.close();
                        }
                        if ui.button(self.language.translate("Paste  ⌘V")).clicked() {
                            self.editor.paste(Point::new(0.5, 0.5));
                            ui.close();
                        }
                        if ui
                            .button(self.language.translate("Duplicate  ⌘D"))
                            .clicked()
                        {
                            self.editor.duplicate();
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .button(self.language.translate("Element Settings  ⌘T"))
                            .clicked()
                        {
                            self.open_element_settings();
                            ui.close();
                        }
                        ui.separator();
                        for (group, commands) in [
                            (
                                "Groups",
                                &["Group", "Ungroup", "Suspend Groups", "Lock", "Unlock"][..],
                            ),
                            (
                                "Display Order",
                                &[
                                    "Bring to Front",
                                    "Send to Back",
                                    "Bring Forward",
                                    "Send Backward",
                                ][..],
                            ),
                            (
                                "Move",
                                &[
                                    "Drag",
                                    "Rotate",
                                    "Mirror",
                                    "Elevate",
                                    "Multiply",
                                    "Move to Story",
                                ][..],
                            ),
                            (
                                "Align & Distribute",
                                &["Align Elements…", "Distribute…"][..],
                            ),
                            (
                                "Reshape",
                                &[
                                    "Trim",
                                    "Stretch",
                                    "Resize",
                                    "Split",
                                    "Fillet / Chamfer",
                                    "Intersect",
                                    "Adjust",
                                    "Offset",
                                    "Explode",
                                    "Unify",
                                ][..],
                            ),
                            (
                                "Reference Line and Plane",
                                &[
                                    "Modify / Invert Wall Reference Line",
                                    "Modify Slab Reference Plane",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "View"), |ui| {
                        ui.selectable_value(
                            &mut self.view,
                            View::Plan,
                            format!("{}  F2", Self::t(self.language, "Floor Plan")),
                        );
                        ui.selectable_value(
                            &mut self.view,
                            View::Split,
                            Self::t(self.language, "Split view"),
                        );
                        ui.selectable_value(
                            &mut self.view,
                            View::Model,
                            format!("{}  F3", Self::t(self.language, "3D Perspective")),
                        );
                        if ui
                            .button(
                                self.language
                                    .translate("Show selection / Marquee in 3D  F5"),
                            )
                            .clicked()
                        {
                            if self.editor.marquee.is_some() {
                                self.show_marquee_3d = true;
                                self.marquee_outside = false;
                                self.fit_marquee();
                            } else if !self.editor.selection_ids().is_empty() {
                                self.show_marquee_3d = true;
                                self.fit_selection();
                            }
                        }
                        ui.separator();
                        if ui
                            .checkbox(
                                &mut self.show_marquee_3d,
                                self.language.translate("Show Marquee in 3D"),
                            )
                            .changed()
                            && self.show_marquee_3d
                        {
                            if self.editor.marquee.is_some() {
                                self.fit_marquee();
                            } else {
                                self.fit_selection();
                            }
                        }
                        if self.show_marquee_3d && self.editor.marquee.is_some() {
                            ui.checkbox(
                                &mut self.marquee_outside,
                                self.language.translate("Show Outside Marquee"),
                            );
                        }
                        ui.separator();
                        if ui
                            .checkbox(&mut self.trace.enabled, self.language.translate("Trace"))
                            .changed()
                        {
                            self.trace_toggled();
                        }
                        ui.menu_button(self.language.translate("Trace Options"), |ui| {
                            self.trace_menu(ui)
                        });
                        ui.separator();
                        let before_guides = self.editor.project.clone();
                        ui.menu_button(self.language.translate("Guides"), |ui| {
                            self.guides_menu(ui)
                        });
                        if ui
                            .checkbox(
                                &mut self.editor.project.guide_settings.guide_lines_enabled,
                                self.language.translate("Guide Lines"),
                            )
                            .changed()
                        {
                            self.editor.commit(before_guides);
                        }
                        ui.checkbox(
                            &mut self.editor.project.grid_settings.show_construction_grid,
                            self.language.translate("Construction Grid Display"),
                        );
                        ui.checkbox(
                            &mut self.editor.snap,
                            self.language.translate("Element Snap"),
                        );
                        ui.menu_button(self.language.translate("Snap Options"), |ui| {
                            ui.checkbox(
                                &mut self.editor.snap,
                                self.language.translate("Element Snap"),
                            );
                            let grid_snap = {
                                let enabled =
                                    &mut self.editor.project.grid_settings.grid_snap_enabled;
                                ui.checkbox(enabled, self.language.translate("Grid Snap"))
                                    .changed()
                                    .then_some(*enabled)
                            };
                            if let Some(enabled) = grid_snap {
                                self.set_grid_snap_enabled(enabled);
                            }
                            let before = self.editor.project.clone();
                            if ui
                                .checkbox(
                                    &mut self.editor.project.guide_settings.snap_enabled,
                                    self.language.translate("Snap Guides and Points"),
                                )
                                .changed()
                            {
                                self.editor.commit(before);
                            }
                        });
                        ui.menu_button(
                            self.language.translate("Grid & Editing Plane Options"),
                            |ui| self.grids_menu(ui),
                        );
                        ui.menu_button(self.language.translate("Elements in 3D View"), |ui| {
                            ui.checkbox(
                                &mut self.all_stories,
                                self.language.translate("Show all stories in 3D"),
                            );
                            ui.separator();
                            ui.checkbox(
                                &mut self.cutaway_enabled,
                                self.language.translate("3D Cutaway"),
                            );
                            ui.checkbox(
                                &mut self.show_marquee_3d,
                                self.language.translate("Filter and Cut Elements in 3D"),
                            );
                            if ui
                                .button(self.language.translate("Show Selection / Marquee"))
                                .clicked()
                            {
                                self.show_marquee_3d = true;
                                if self.editor.marquee.is_some() {
                                    self.fit_marquee();
                                } else {
                                    self.fit_selection();
                                }
                            }
                            if ui
                                .button(self.language.translate("Hide Selection"))
                                .clicked()
                            {
                                self.editor.status =
                                    "3D selection visibility · interface preview".into();
                            }
                            if ui.button(self.language.translate("Show All")).clicked() {
                                self.editor.status = "All elements shown in 3D".into();
                            }
                        });
                        ui.menu_button(self.language.translate("3D View Options"), |ui| {
                            for option in [
                                "Projection Settings…",
                                "Perspective",
                                "Axonometry",
                                "Basic",
                                "Vectorial",
                                "Simple Shading",
                                "Technical",
                                "Detailed",
                                "Shadows",
                                "White Model",
                                "Wireframe",
                                "Orbit",
                                "Explore Model",
                            ] {
                                if ui.button(self.language.translate(option)).clicked() {
                                    self.editor.status =
                                        format!("{option} · viewport interface preview");
                                    ui.close();
                                }
                            }
                        });
                        ui.menu_button(self.language.translate("Zoom and Rebuild"), |ui| {
                            if ui
                                .button(self.language.translate("Fit in Window · F"))
                                .clicked()
                            {
                                self.fit();
                                ui.close();
                            }
                            if ui.button(self.language.translate("Rebuild")).clicked() {
                                self.editor.status = "View refreshed".into();
                                ui.close();
                            }
                            if ui.button(self.language.translate("Refresh")).clicked() {
                                self.editor.status = "View refreshed".into();
                            }
                            if ui
                                .button(self.language.translate("Rebuild & Regenerate"))
                                .clicked()
                            {
                                self.editor.status = "Model refresh requested".into();
                            }
                        });
                        ui.separator();
                        ui.menu_button(Self::t(self.language, "Language"), |ui| {
                            let previous = self.language;
                            ui.selectable_value(&mut self.language, UiLanguage::English, "English");
                            ui.selectable_value(
                                &mut self.language,
                                UiLanguage::SimplifiedChinese,
                                "简体中文",
                            );
                            if self.language != previous {
                                self.language.save_preference();
                                self.editor.status = format!(
                                    "{} · {}",
                                    self.language.translate("Language"),
                                    self.language.label()
                                );
                            }
                        });
                    });
                    ui.menu_button(Self::t(self.language, "Design"), |ui| {
                        ui.menu_button(self.language.translate("Architectural Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Arrow",
                                    "Marquee",
                                    "Wall",
                                    "Wall End",
                                    "Column",
                                    "Beam",
                                    "Slab",
                                    "Roof",
                                    "Shell",
                                    "Stair",
                                    "Railing",
                                    "Curtain Wall",
                                    "Door",
                                    "Window",
                                    "Corner-Window",
                                    "Skylight",
                                    "Opening",
                                    "Zone",
                                    "Mesh",
                                    "Morph",
                                    "Object",
                                    "Lamp",
                                ],
                            ) {
                                match command.as_str() {
                                    "Wall" => {
                                        self.editor.set_tool(Tool::Wall);
                                        self.preview_tool = None;
                                    }
                                    "Slab" => {
                                        self.editor.set_tool(Tool::Slab);
                                        self.preview_tool = None;
                                    }
                                    "Stair" => self.activate_stair_tool(),
                                    "Polyline" => self.activate_polyline_tool(),
                                    "Marquee" => {
                                        self.editor.set_tool(Tool::Marquee);
                                        self.preview_tool = None;
                                    }
                                    "Arrow" => {
                                        self.editor.set_tool(Tool::Select);
                                        self.preview_tool = None;
                                    }
                                    _ => {
                                        self.preview_tool = Some(command.clone());
                                        self.editor.set_tool(Tool::Select);
                                        self.editor.status =
                                            format!("{command} tool selected · interface preview");
                                    }
                                }
                                self.tracker_locked = false;
                            }
                        });
                        ui.menu_button(
                            self.language.translate("Structural Engineering Tools"),
                            |ui| {
                                if let Some(command) = Self::preview_menu_items(
                                    ui,
                                    self.language,
                                    &["Support", "Link", "Point Load", "Line Load", "Surface Load"],
                                ) {
                                    self.open_command_preview(command);
                                }
                            },
                        );
                        ui.menu_button(self.language.translate("MEP Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Duct Route",
                                    "Pipe Route",
                                    "Cable Route",
                                    "Branch",
                                    "Fitting",
                                    "Terminal",
                                    "Accessory",
                                    "Equipment",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        for (group, commands) in [
                            (
                                "Stories and Connections",
                                &[
                                    "Story Settings…",
                                    "Edit Elements by Stories…",
                                    "Trim Elements",
                                    "Merge Elements",
                                    "Join Wall to Curtain Wall",
                                    "Create Opening",
                                    "Clear Connections",
                                ][..],
                            ),
                            (
                                "Model Operations",
                                &[
                                    "Solid Element Operations…",
                                    "Magic Wand",
                                    "Modify Morph",
                                    "Modify Railing",
                                    "Modify Curtain Wall",
                                ][..],
                            ),
                            (
                                "Design Extras",
                                &["RoofMaker…", "TrussMaker…", "Grid System…", "Update Zones"][..],
                            ),
                            (
                                "Model Check",
                                &[
                                    "Collision Detection…",
                                    "Physical Quality…",
                                    "Model Compare…",
                                    "Structural Analytical Model…",
                                    "Energy Evaluation…",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Document"), |ui| {
                        ui.menu_button(self.language.translate("Documenting Tools"), |ui| {
                            for command in [
                                "Section",
                                "Elevation",
                                "Interior Elevation",
                                "Detail",
                                "Worksheet",
                                "Camera",
                                "Dimension",
                                "Level Dimension",
                                "Radial Dimension",
                                "Angle Dimension",
                                "Text",
                                "Label",
                                "Grid Element",
                                "Fill",
                                "Line",
                                "Arc / Circle",
                                "Polyline",
                                "Spline",
                                "Hotspot",
                                "Figure",
                                "Drawing",
                            ] {
                                if ui.button(self.language.translate(command)).clicked() {
                                    if command == "Polyline" {
                                        self.activate_polyline_tool();
                                    } else {
                                        self.open_command_preview(command.to_owned());
                                    }
                                    ui.close();
                                }
                            }
                        });
                        ui.menu_button(self.language.translate("Layers"), |ui| {
                            self.layers_menu_contents(ui);
                        });
                        if ui
                            .button(self.language.translate("Floor Plan Scale…"))
                            .clicked()
                        {
                            self.open_plan_option_dialog(PlanOptionKind::Scale);
                            ui.close();
                        }
                        if ui
                            .button(self.language.translate("Partial Structure Display…"))
                            .clicked()
                        {
                            self.open_plan_option_dialog(PlanOptionKind::PartialStructure);
                            ui.close();
                        }
                        ui.menu_button(self.language.translate("Pen Sets"), |ui| {
                            self.pen_set_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Model View"), |ui| {
                            self.model_view_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Graphic Overrides"), |ui| {
                            self.graphic_overrides_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Design Options"), |ui| {
                            self.design_options_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Renovation"), |ui| {
                            self.renovation_menu_contents(ui);
                        });
                        for (group, commands) in [
                            (
                                "Annotation",
                                &[
                                    "Automatic Dimensioning…",
                                    "Label Selected Elements",
                                    "Create Patch…",
                                    "Spell Checker",
                                ][..],
                            ),
                            (
                                "Schedules and Lists",
                                &[
                                    "Scheme Settings…",
                                    "Wall Schedule",
                                    "Door Schedule",
                                    "Window Schedule",
                                    "Object Inventory",
                                    "Element Lists",
                                    "Component Lists",
                                    "Zone Lists",
                                    "Project Indexes",
                                ][..],
                            ),
                            (
                                "Issues and Changes",
                                &[
                                    "Issue Manager",
                                    "Issue Organizer",
                                    "Change Manager",
                                    "Keynotes",
                                ][..],
                            ),
                            (
                                "Creative Imaging",
                                &[
                                    "PhotoRender Projection",
                                    "Render Marquee Region",
                                    "PhotoRendering Settings…",
                                    "Surface Painter",
                                    "Create Fly-Through…",
                                    "Create Sun Study…",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Options"), |ui| {
                        ui.checkbox(
                            &mut self.editor.snap,
                            self.language.translate("Snap to geometry and grid"),
                        );
                        ui.checkbox(
                            &mut self.editor.orthogonal,
                            self.language.translate("Orthogonal drawing"),
                        );
                        ui.separator();
                        for (group, commands) in [
                            (
                                "Element Attributes",
                                &[
                                    "Layers…",
                                    "Lines…",
                                    "Pens & Color…",
                                    "Fills…",
                                    "Surfaces…",
                                    "Building Materials…",
                                    "Composites…",
                                    "Profile Manager…",
                                    "Zone Categories…",
                                    "Attributes…",
                                    "Attribute Manager…",
                                ][..],
                            ),
                            (
                                "Properties and Classification",
                                &[
                                    "Property Manager…",
                                    "Classification Manager…",
                                    "Operation Profiles…",
                                ][..],
                            ),
                            (
                                "Structural Attributes",
                                &[
                                    "Load Manager…",
                                    "Load Combinations…",
                                    "Load Mapping…",
                                    "Load Categories…",
                                ][..],
                            ),
                            (
                                "Project Preferences",
                                &[
                                    "Working Units…",
                                    "Dimensions…",
                                    "Calculation Units…",
                                    "Calculation Rules…",
                                    "Zones…",
                                    "Reference Levels…",
                                    "Legacy…",
                                    "Stair Rules and Standards…",
                                    "Structural Analytical Model Units…",
                                    "Location Settings…",
                                    "Project North…",
                                    "Survey Point…",
                                ][..],
                            ),
                            (
                                "Work Environment",
                                &[
                                    "Dialog Boxes and Palettes…",
                                    "Selection and Element Information…",
                                    "Tracker and Coordinate Input…",
                                    "Input Constraints and Guides…",
                                    "Model Rebuild Options…",
                                    "Appearance…",
                                    "Data Safety and Integrity…",
                                    "Keyboard Shortcuts…",
                                    "Toolbox…",
                                    "Tool Settings Dialog Boxes…",
                                    "Info Box…",
                                    "Toolbars…",
                                    "Menus…",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    if command == "Layers…" {
                                        self.open_plan_option_dialog(PlanOptionKind::Layers);
                                    } else if command == "Pens & Color…" {
                                        self.open_plan_option_dialog(PlanOptionKind::PenSets);
                                    } else if command == "Dimensions…" {
                                        self.open_plan_option_dialog(PlanOptionKind::Dimensions);
                                    } else {
                                        self.open_command_preview(command);
                                    }
                                }
                            });
                        }
                        ui.separator();
                        if ui
                            .button(self.language.translate("Add-On Manager…"))
                            .clicked()
                        {
                            self.editor.status = "Add-On Manager · interface preview".into();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Teamwork"), |ui| {
                        ui.menu_button(self.language.translate("Project"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Share…",
                                    "Migrate Teamwork Project…",
                                    "Open / Join Teamwork Project…",
                                    "Local Data Manager…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Traveling"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Create Travel Pack",
                                    "Import Travel Pack",
                                    "Leave Teamwork Project",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Send & Receive"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Send & Receive",
                                    "Send Changes",
                                    "Receive Changes",
                                    "Send & Receive, then Compare",
                                    "Reserve Elements…",
                                    "Release All",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Messages and Review"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Assign Selected Elements",
                                    "New Message…",
                                    "Review Selected Elements…",
                                    "Change Tracking",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        if ui
                            .button(self.language.translate("Teamwork Palette"))
                            .clicked()
                        {
                            self.editor.status = "Teamwork Palette · standalone preview".into();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Window"), |ui| {
                        let quick_label = if self.quick_options_visible {
                            "Hide Quick Options Bar"
                        } else {
                            "Show Quick Options Bar"
                        };
                        if ui.button(self.language.translate(quick_label)).clicked() {
                            self.toggle_quick_options_bar();
                            ui.close();
                        }
                        ui.separator();
                        ui.checkbox(&mut self.show_left, self.language.translate("Toolbox"));
                        ui.checkbox(
                            &mut self.show_right,
                            self.language.translate("Navigator & Properties"),
                        );
                        ui.separator();
                        ui.menu_button(self.language.translate("Palettes"), |ui| {
                            for palette in [
                                "Info Box",
                                "Status Bar",
                                "Quick Options",
                                "Organizer",
                                "Navigator Preview",
                                "Drawing Manager",
                                "Model Check Report",
                                "Issue Manager",
                                "Change Manager",
                                "Design Options",
                                "Renovation",
                                "Trace & Reference",
                                "Favorites",
                                "Attributes",
                                "Profile Manager",
                                "Element Information",
                                "Selections",
                                "Coordinates",
                                "Control Box",
                                "Quick Layers",
                                "Teamwork Palette",
                            ] {
                                let selected = match palette {
                                    "Info Box" => self.info_box_visible,
                                    "Quick Options" => self.quick_options_palette_visible,
                                    _ => false,
                                };
                                if ui
                                    .selectable_label(selected, self.language.translate(palette))
                                    .clicked()
                                {
                                    if palette == "Info Box" {
                                        self.toggle_info_box_visibility();
                                        ui.close();
                                    } else if palette == "Trace & Reference" {
                                        self.open_trace_palette();
                                    } else if palette == "Quick Options" {
                                        self.toggle_quick_options_palette();
                                    } else if palette == "Design Options" {
                                        self.open_plan_option_dialog(PlanOptionKind::DesignOptions);
                                    } else if palette == "Renovation" {
                                        self.open_plan_option_dialog(PlanOptionKind::Renovation);
                                    } else {
                                        self.editor.status = format!("Palette · {palette}");
                                    }
                                }
                            }
                        });
                        ui.separator();
                        ui.menu_button(self.language.translate("Toolbars"), |ui| {
                            for toolbar in [
                                "Standard",
                                "Standard for Low-Res Screens",
                                "Tools - Architectural",
                                "Arrange Elements",
                                "Attributes",
                                "Drafting Aids",
                                "Drag Elements",
                                "Edit Elements",
                                "3D Visualization",
                                "Classic 3D Navigation",
                                "MEP Toolbar",
                                "Model Check",
                                "Structural Engineering",
                                "Teamwork",
                            ] {
                                if ui
                                    .selectable_label(false, self.language.translate(toolbar))
                                    .clicked()
                                {
                                    self.editor.status = format!("Toolbar · {toolbar}");
                                }
                            }
                        });
                        ui.separator();
                        if ui
                            .button(self.language.translate("Reset workspace layout"))
                            .clicked()
                        {
                            self.show_left = true;
                            self.show_right = true;
                            self.quick_options_visible = true;
                            self.quick_options_palette_visible = false;
                            self.info_box_visible = true;
                            self.view = View::Plan;
                            self.split = 0.53;
                            ui.close();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Help"), |ui| {
                        if ui
                            .button(self.language.translate("About OpenArchi"))
                            .clicked()
                        {
                            self.help = true;
                            ui.close();
                        }
                        ui.label(
                            RichText::new(self.language.translate("Native Rust · wgpu / Metal"))
                                .small()
                                .color(MUTED),
                        );
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.editor.undo.is_empty(),
                            Button::new(self.language.translate("Undo")),
                        )
                        .on_hover_text(Self::t(self.language, "Undo · ⌘Z"))
                        .clicked()
                    {
                        self.editor.undo();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.redo.is_empty(),
                            Button::new(self.language.translate("Redo")),
                        )
                        .on_hover_text(Self::t(self.language, "Redo · ⇧⌘Z"))
                        .clicked()
                    {
                        self.editor.redo();
                    }
                    ui.separator();
                    if Self::vector_labeled_button(
                        ui,
                        "Fit",
                        &Self::t(self.language, "Fit"),
                        &Self::t(self.language, "Fit model · F"),
                        false,
                    ) {
                        self.fit();
                    }
                    ui.add_space(12.);
                    ui.selectable_value(
                        &mut self.view,
                        View::Plan,
                        format!("{}  F2", Self::t(self.language, "2D Plan")),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        View::Split,
                        Self::t(self.language, "Split view"),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        View::Model,
                        format!("{}  F3", Self::t(self.language, "3D Perspective")),
                    );
                    if self.editor.marquee.is_some() {
                        ui.separator();
                        if ui
                            .checkbox(
                                &mut self.show_marquee_3d,
                                self.language.translate("3D Marquee"),
                            )
                            .changed()
                            && self.show_marquee_3d
                        {
                            if self.editor.marquee.is_some() {
                                self.fit_marquee();
                            } else {
                                self.fit_selection();
                            }
                        }
                        if self.show_marquee_3d {
                            ui.selectable_value(
                                &mut self.marquee_outside,
                                false,
                                self.language.translate("Inside"),
                            );
                            ui.selectable_value(
                                &mut self.marquee_outside,
                                true,
                                self.language.translate("Outside"),
                            );
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Self::vector_labeled_button(
                            ui,
                            "Help",
                            &Self::t(self.language, "Help"),
                            &Self::t(self.language, "Open help and shortcuts"),
                            false,
                        ) {
                            self.help = true;
                        }
                        if ui.button(Self::t(self.language, "Save project")).clicked() {
                            self.file_action = Some(FileAction::Save);
                        }
                    });
                });
            });
        egui::Panel::top("standard_toolbar")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(9, 3)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (icon, tip) in [
                        ("Home", "Home / fit project"),
                        ("Undo", "Undo"),
                        ("Redo", "Redo"),
                        ("Save", "Save project"),
                        ("Zoom In", "Zoom in"),
                        ("Zoom Out", "Zoom out"),
                    ] {
                        if Self::vector_icon_button(ui, icon, &Self::t(self.language, tip), false) {
                            match icon {
                                "Home" => self.fit(),
                                "Undo" => {
                                    self.editor.undo();
                                }
                                "Redo" => {
                                    self.editor.redo();
                                }
                                "Save" => self.file_action = Some(FileAction::Save),
                                "Zoom In" => {
                                    self.plan_scale = (self.plan_scale * 1.15).clamp(4., 400.)
                                }
                                "Zoom Out" => {
                                    self.plan_scale = (self.plan_scale / 1.15).clamp(4., 400.)
                                }
                                _ => {}
                            }
                        }
                    }
                    ui.separator();
                    ui.menu_button(Self::t(self.language, "Guides"), |ui| self.guides_menu(ui));
                    ui.menu_button(Self::t(self.language, "Grids"), |ui| self.grids_menu(ui));
                    if ui
                        .toggle_value(&mut self.trace.enabled, Self::t(self.language, "Trace"))
                        .changed()
                    {
                        self.trace_toggled();
                    }
                    ui.menu_button("▾", |ui| self.trace_menu(ui));
                    ui.small_button(Self::t(self.language, "Measure"))
                        .on_hover_text(if self.language == UiLanguage::SimplifiedChinese {
                            "测量工具预览"
                        } else {
                            "Measure tool preview"
                        });
                    ui.toggle_value(&mut self.groups_enabled, Self::t(self.language, "Groups"));
                    ui.toggle_value(
                        &mut self.cutaway_enabled,
                        Self::t(self.language, "3D Cutaway"),
                    );
                    ui.separator();
                    ui.selectable_value(
                        &mut self.view,
                        View::Plan,
                        Self::t(self.language, "Floor plan · F2"),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        View::Model,
                        Self::t(self.language, "Perspective 3D · F3"),
                    );
                });
            });
        egui::Panel::top("edit_toolbar")
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(9, 2)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.language.translate("EDIT"))
                            .size(10.)
                            .strong()
                            .color(MUTED),
                    );
                    if Self::vector_labeled_button(
                        ui,
                        "Drag",
                        &Self::t(self.language, "Drag"),
                        &Self::t(self.language, "Move selection"),
                        false,
                    ) {
                        self.editor.status =
                            "Drag command · select an element and drag it in the canvas".into();
                    }
                    if Self::vector_labeled_button(
                        ui,
                        "Rotate",
                        &Self::t(self.language, "Rotate"),
                        &Self::t(self.language, "Rotate selection 90°"),
                        false,
                    ) {
                        self.editor.status = "Rotate command · choose a pivot in the canvas".into();
                    }
                    if Self::vector_labeled_button(
                        ui,
                        "Mirror",
                        &Self::t(self.language, "Mirror"),
                        &Self::t(self.language, "Mirror selection"),
                        false,
                    ) {
                        self.editor.status = "Mirror command · choose an axis in the canvas".into();
                    }
                    if Self::vector_labeled_button(
                        ui,
                        "Multiply",
                        &Self::t(self.language, "Multiply"),
                        &Self::t(self.language, "Duplicate selection"),
                        false,
                    ) {
                        self.editor.status = "Multiply command · choose copy settings".into();
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(
                            self.language
                                .translate("Workspace controls · viewport context"),
                        )
                        .small()
                        .color(MUTED),
                    );
                });
            });
        if self.info_box_visible {
            self.sync_info_box_wall();
            self.sync_info_box_wall_tool();
            egui::Panel::top("info_box")
            .frame(
                Frame::new()
                    .fill(PANEL_ALT)
                    .inner_margin(Margin::symmetric(12, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    Self::label(ui, &Self::t(self.language, "INFO BOX"));
                    ui.separator();
                    if let Some(name) = self.preview_tool.as_deref() {
                        ui.strong(Self::t(self.language, name));
                        ui.separator();
                        ComboBox::from_id_salt("preview_favorite").selected_text(self.language.translate("Favorites")).show_ui(ui, |ui| {
                            for favorite in ["Default", "Architectural", "Detailed", "Existing"] { let _ = ui.selectable_label(favorite == "Default", favorite); }
                        });
                        ui.label(self.language.translate("ID"));
                        ui.add_sized([78., 24.], TextEdit::singleline(&mut self.preview_element_id));
                        ComboBox::from_id_salt("preview_home_story").selected_text(Self::t(self.language, "Home Story")).show_ui(ui, |ui| {
                            for story in self.editor.project.stories.clone() { let _ = ui.selectable_label(story.id == self.editor.story, format!("{} · {}", story.id, story.name)); }
                        });
                        ComboBox::from_id_salt("preview_layer").selected_text(&self.preview_layer).width(155.).show_ui(ui, |ui| {
                            for layer in ["01 Site", "02 Drafting", "03 Plans - Preliminary", "04 Plans - Approval", "05 Plans - Detailed", "06 Plans - Mechanical", "07 Plans - Structural", "08 Rendering", "09 Layouting"] {
                                ui.selectable_value(&mut self.preview_layer, layer.to_owned(), layer);
                            }
                        });
                        ui.label(Self::t(self.language, "Offset"));
                        ui.add(DragValue::new(&mut self.preview_offset).speed(0.05).fixed_decimals(2).suffix(" m"));
                        ui.label(Self::t(self.language, "Reference"));
                        Self::inline_vector_icon(ui, "Reference", ACCENT);
                        if ui.button(Self::t(self.language, "Settings…")).clicked() { self.open_command_preview(format!("{name} Settings")); }
                    } else {
                        let tool_name = match self.editor.tool {
                            Tool::Select => "Arrow",
                            Tool::Marquee => "Marquee",
                            Tool::Wall => "Wall",
                            Tool::Slab => "Slab",
                            Tool::Stair => "Stair",
                            Tool::Polyline => "Polyline",
                        };
                        ui.strong(Self::t(self.language, tool_name));
                    }
                    if self.preview_tool.is_some() {
                        // Generic tool metadata fields above are UI-only until the tool backend is implemented.
                    } else if self.info_box_wall.is_some() {
                        self.info_box_wall_properties(ui);
                    } else if self.editor.tool == Tool::Wall {
                        self.info_box_wall_tool_properties(ui);
                        ui.separator();
                        ui.label(Self::t(self.language, "Material"));
                        ComboBox::from_id_salt("wall_tool_material")
                            .selected_text(self.editor.material.name())
                            .width(118.)
                            .show_ui(ui, |ui| {
                                for m in Material::ALL {
                                    ui.selectable_value(&mut self.editor.material, m, m.name());
                                }
                            });
                        ui.checkbox(&mut self.editor.snap, self.language.translate("Snap"))
                            .on_hover_text(Self::t(
                                self.language,
                                "Endpoints / midpoints / 100 mm grid",
                            ));
                        ui.checkbox(
                            &mut self.editor.orthogonal,
                            self.language.translate("Ortho"),
                        )
                        .on_hover_text(Self::t(
                            self.language,
                            "Constrain to X/Y; Shift temporarily constrains",
                        ));
                    } else if self.editor.tool == Tool::Marquee {
                        ComboBox::from_id_salt("marquee_geometry")
                            .selected_text(match self.marquee_geometry {
                                MarqueeGeometry::Rectangle => "Rectangle",
                                MarqueeGeometry::RotatedRectangle => "Rotated rectangle",
                                MarqueeGeometry::Polygon => "Polygon",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::Rectangle, if self.language == UiLanguage::SimplifiedChinese { "矩形 · 拖动" } else { "Rectangle · drag" });
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::RotatedRectangle, if self.language == UiLanguage::SimplifiedChinese { "旋转矩形 · 三点" } else { "Rotated rectangle · 3 points" });
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::Polygon, if self.language == UiLanguage::SimplifiedChinese { "多边形 · 点击顶点，右键闭合" } else { "Polygon · click vertices, right-click to close" });
                            });
                        ui.selectable_value(&mut self.marquee_scope, MarqueeScope::CurrentStory, Self::t(self.language, "Single story"));
                        ui.selectable_value(&mut self.marquee_scope, MarqueeScope::AllStories, Self::t(self.language, "All stories"));
                        ui.label(Self::t(self.language, "3D depth"));
                        ui.add(DragValue::new(&mut self.marquee_height).speed(0.05).range(0.01..=100.).fixed_decimals(2).suffix(" m"));
                        if self.editor.marquee.is_some() {
                            if ui.button(Self::t(self.language, "Select all in Marquee · ⌘A")).clicked() {
                                let count = self.editor.select_all_in_marquee();
                                self.editor.status = format!("Selected {count} element(s) in Marquee");
                            }
                            if ui.button(Self::t(self.language, "Remove Marquee · Esc")).clicked() {
                                self.editor.remove_marquee();
                                self.editor.set_selection(Vec::new());
                            }
                            ui.label(self.language.translate("ΔX"));
                            ui.add(DragValue::new(&mut self.marquee_dx).speed(0.01).fixed_decimals(3).suffix(" m"));
                            ui.label(self.language.translate("ΔY"));
                            ui.add(DragValue::new(&mut self.marquee_dy).speed(0.01).fixed_decimals(3).suffix(" m"));
                            if ui.button(Self::t(self.language, "Move region")).clicked() {
                                self.editor.move_marquee(Point::new(self.marquee_dx, self.marquee_dy));
                            }
                            if ui.button(Self::t(self.language, "Stretch enclosed nodes")).clicked() {
                                match self.editor.stretch_nodes_in_marquee(Point::new(self.marquee_dx, self.marquee_dy)) {
                                    Ok(0) => self.editor.status = "No editable nodes inside Marquee".into(),
                                    Ok(_) => {},
                                    Err(error) => self.editor.status = error,
                                }
                            }
                            if !self.editor.selection_ids().is_empty() {
                                ui.separator();
                                ui.label(Self::t(self.language, "Selection operations"));
                                if ui.button(self.language.translate("Cut · ⌘X")).clicked() { self.editor.cut_selection(); }
                                if ui.button(self.language.translate("Copy · ⌘C")).clicked() { self.editor.copy_selection(); }
                                if ui.button(self.language.translate("Mirror X")).clicked() { self.editor.mirror_selection(true); }
                                if ui.button(self.language.translate("Mirror Y")).clicked() { self.editor.mirror_selection(false); }
                                if ui.button(self.language.translate("Rotate 90°")).clicked() { self.editor.rotate_selection_quarter_turn(false); }
                                if ui.button(self.language.translate("Move to story above")).clicked() { self.editor.move_selection_to_story(1); }
                                if ui.button(self.language.translate("Move to story below")).clicked() { self.editor.move_selection_to_story(-1); }
                                if ui.button(self.language.translate("Move selected elements")).clicked() {
                                    self.editor.move_selection(Point::new(self.marquee_dx, self.marquee_dy));
                                }
                                if ui.button(self.language.translate("Duplicate selected · ⌘D")).clicked() {
                                    self.editor.duplicate();
                                }
                                if ui.button(self.language.translate("Delete selected · Delete")).clicked() {
                                    self.editor.delete();
                                }
                                ui.label(self.selected_count_label(self.editor.selection_ids().len()));
                            }
                        }
                    } else if self.editor.tool == Tool::Select {
                        ui.separator();
                        ui.label(RichText::new(Self::t(self.language, "Selection Settings")).strong());
                        ui.checkbox(&mut self.select_sub_elements, Self::t(self.language, "Sub-element selection"))
                            .on_hover_text(Self::t(self.language, "Toggle element / sub-element selection mode"));
                        ui.checkbox(&mut self.select_locked_elements, Self::t(self.language, "Include locked elements"));
                        ui.separator();
                        ui.label(Self::t(self.language, "Selection methods"));
                        let _ = Self::vector_labeled_button(ui, "Element", &Self::t(self.language, "Element"), &Self::t(self.language, "Select whole elements"), true);
                        let _ = Self::vector_labeled_button(ui, "Area Selection", &Self::t(self.language, "Area"), &Self::t(self.language, "Select by area"), false);
                        let _ = Self::vector_labeled_button(ui, "Quick Selection", &Self::t(self.language, "Quick"), &Self::t(self.language, "Quick selection"), false);
                        ui.label(RichText::new(self.selected_count_label(self.editor.selection_ids().len())).color(MUTED));
                    } else {
                        ui.separator();
                        if self.editor.tool == Tool::Polyline {
                            ui.label(self.language.translate("2D drafting · left-click nodes · right-click to finish · click start to close"));
                        } else if self.editor.tool == Tool::Stair {
                            ui.label(format!("{} risers · {:.2} m wide · {:.2} m rise", self.editor.stair_spec.risers, self.editor.stair_spec.width, self.editor.stair_spec.total_rise));
                            if ui.button(self.language.translate("Stair Settings…")).clicked() {
                                self.stair_settings_draft = self.editor.stair_spec.clone();
                                self.stair_settings_error.clear();
                                self.stair_settings_open = true;
                            }
                        } else {
                            ui.label(Self::t(self.language, "Thickness"));
                            ui.add(
                                DragValue::new(&mut self.editor.thickness)
                                    .speed(0.01)
                                    .range(0.01..=5.)
                                    .fixed_decimals(3)
                                    .suffix(" m"),
                            );
                        }
                        if self.editor.tool != Tool::Slab && self.editor.tool != Tool::Stair && self.editor.tool != Tool::Polyline {
                            ui.label(Self::t(self.language, "Height"));
                            ui.add(
                                DragValue::new(&mut self.editor.height)
                                    .speed(0.05)
                                    .range(0.01..=100.)
                                    .fixed_decimals(2)
                                    .suffix(" m"),
                            );
                        }
                        ComboBox::from_id_salt("default_material")
                            .selected_text(self.editor.material.name())
                            .width(118.)
                            .show_ui(ui, |ui| {
                                for m in Material::ALL {
                                    ui.selectable_value(&mut self.editor.material, m, m.name());
                                }
                            });
                        ui.separator();
                        ui.checkbox(&mut self.editor.snap, self.language.translate("Snap"))
                            .on_hover_text(Self::t(self.language, "Endpoints / midpoints / 100 mm grid"));
                        ui.checkbox(&mut self.editor.orthogonal, self.language.translate("Ortho"))
                            .on_hover_text(Self::t(self.language, "Constrain to X/Y; Shift temporarily constrains"));
                    }
                });
            });
        }
    }
    fn info_box_drag(
        ui: &mut Ui,
        language: UiLanguage,
        label: &str,
        value: &mut f32,
        dirty: &mut bool,
        commit: &mut bool,
    ) {
        ui.label(Self::t(language, label));
        let response = ui.add(
            DragValue::new(value)
                .speed(0.005)
                .fixed_decimals(3)
                .suffix(" m"),
        );
        if response.changed() {
            *dirty = true;
        }
        if response.drag_stopped() || response.lost_focus() {
            *commit = true;
        }
    }
    fn info_box_tool_drag(
        ui: &mut Ui,
        language: UiLanguage,
        label: &str,
        value: &mut f32,
        dirty: &mut bool,
        commit: &mut bool,
    ) {
        ui.label(Self::t(language, label));
        let response = ui.add(
            DragValue::new(value)
                .speed(0.005)
                .fixed_decimals(3)
                .suffix(" m"),
        );
        if response.changed() {
            *dirty = true;
            *commit = true;
        }
    }
    fn info_box_wall_properties(&mut self, ui: &mut Ui) {
        let language = self.language;
        let stories = self.editor.project.stories.clone();
        let selection_label = format!("{}: 1", language.translate("All Selected"));
        let mut commit = false;
        let mut open_settings = false;
        let mut open_floor_plan = false;
        {
            let Some(state) = self.info_box_wall.as_mut() else {
                return;
            };
            let element_id = state.element_id;
            let draft = &mut state.draft;
            let mut dirty = state.dirty;
            ui.separator();
            ui.label(
                RichText::new(format!("{} {:03}", language.translate("Wall"), element_id)).strong(),
            );
            ui.label(RichText::new(selection_label).small().color(MUTED));
            ui.separator();

            ui.label(Self::t(language, "Layer:"));
            let before_layer = draft.layer;
            ComboBox::from_id_salt("info_box_wall_layer")
                .selected_text(layer_label(language, draft.layer))
                .width(145.)
                .show_ui(ui, |ui| {
                    for layer in Layer::ALL {
                        ui.selectable_value(&mut draft.layer, layer, layer_label(language, layer));
                    }
                });
            if draft.layer != before_layer {
                dirty = true;
                commit = true;
            }

            ui.label(Self::t(language, "Structural:"));
            let before_structural = draft.settings.structural.structural_function;
            ComboBox::from_id_salt("info_box_wall_structural")
                .selected_text(language.translate(before_structural.name()))
                .width(115.)
                .show_ui(ui, |ui| {
                    for function in StructuralFunction::ALL {
                        ui.selectable_value(
                            &mut draft.settings.structural.structural_function,
                            *function,
                            language.translate(function.name()),
                        );
                    }
                });
            if draft.settings.structural.structural_function != before_structural {
                dirty = true;
                commit = true;
            }

            ui.label(Self::t(language, "Geometry Method:"));
            let before_method = draft.settings.geometry.geometry_method;
            ui.horizontal(|ui| {
                for choice in crate::wall::WallGeometryMethod::ALL {
                    let selected = draft.settings.geometry.geometry_method == *choice;
                    if ui
                        .add(Button::new(language.translate(choice.name())).selected(selected))
                        .clicked()
                    {
                        draft.settings.geometry.geometry_method = *choice;
                    }
                }
            });
            if draft.settings.geometry.geometry_method != before_method {
                dirty = true;
                commit = true;
            }

            ui.label(Self::t(language, "Reference Line:"));
            let before_line = draft.settings.geometry.reference_line;
            ui.horizontal(|ui| {
                for choice in crate::wall::WallReferenceLine::ALL {
                    let selected = draft.settings.geometry.reference_line == *choice;
                    if ui
                        .add(Button::new(language.translate(choice.name())).selected(selected))
                        .clicked()
                    {
                        draft.settings.geometry.reference_line = *choice;
                    }
                }
            });
            if draft.settings.geometry.reference_line != before_line {
                dirty = true;
                commit = true;
            }
            Self::info_box_drag(
                ui,
                language,
                "Reference Offset",
                &mut draft.settings.geometry.reference_offset,
                &mut dirty,
                &mut commit,
            );

            Self::info_box_drag(
                ui,
                language,
                "Thickness",
                &mut draft.thickness,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Height",
                &mut draft.height,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Top Offset",
                &mut draft.settings.geometry.top_offset,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Bottom Offset",
                &mut draft.settings.geometry.bottom_offset,
                &mut dirty,
                &mut commit,
            );

            ui.label(Self::t(language, "Top Link:"));
            let before_link = draft.settings.geometry.top_link_offset;
            ComboBox::from_id_salt("info_box_wall_top_link")
                .selected_text(top_link_label(before_link, draft.story, &stories, language))
                .width(195.)
                .show_ui(ui, |ui| {
                    for link in -2..=3 {
                        ui.selectable_value(
                            &mut draft.settings.geometry.top_link_offset,
                            link,
                            top_link_label(link, draft.story, &stories, language),
                        );
                    }
                });
            if draft.settings.geometry.top_link_offset != before_link {
                dirty = true;
                commit = true;
            }

            ui.label(Self::t(language, "Structure:"));
            let structure = ui.add_sized(
                [150., 22.],
                TextEdit::singleline(&mut draft.settings.profile.name),
            );
            if structure.changed() {
                dirty = true;
            }
            if structure.lost_focus() {
                commit = true;
            }

            if ui
                .button(Self::t(language, "Floor Plan and Section…"))
                .clicked()
            {
                open_floor_plan = true;
            }
            if ui.button(Self::t(language, "Settings…")).clicked() {
                open_settings = true;
            }
            if ui
                .add_enabled(dirty, Button::new(language.translate("Apply")))
                .clicked()
            {
                commit = true;
            }
            if !state.error.is_empty() {
                ui.colored_label(Color32::from_rgb(224, 125, 105), &state.error);
            }
            state.dirty = dirty;
        }
        if commit && self.info_box_wall.as_ref().is_some_and(|state| state.dirty) {
            let _ = self.commit_info_box_wall();
        }
        if open_floor_plan {
            self.open_info_box_wall_settings(true);
        }
        if open_settings {
            self.open_info_box_wall_settings(false);
        }
    }
    fn info_box_wall_tool_properties(&mut self, ui: &mut Ui) {
        let language = self.language;
        let stories = self.editor.project.stories.clone();
        let mut commit = false;
        let mut open_settings = false;
        let mut open_floor_plan = false;
        let mut method_changed = false;
        {
            let Some(state) = self.info_box_wall_tool.as_mut() else {
                return;
            };
            let draft = &mut state.draft;
            let mut dirty = state.dirty;
            ui.separator();
            ui.label(RichText::new(language.translate("Default Settings")).strong());
            ui.separator();

            ui.label(Self::t(language, "Layer:"));
            let before_layer = draft.layer;
            ComboBox::from_id_salt("info_box_wall_tool_layer")
                .selected_text(layer_label(language, draft.layer))
                .width(145.)
                .show_ui(ui, |ui| {
                    for layer in Layer::ALL {
                        ui.selectable_value(&mut draft.layer, layer, layer_label(language, layer));
                    }
                });
            if draft.layer != before_layer {
                dirty = true;
                commit = true;
            }

            ui.label(Self::t(language, "Geometry Method:"));
            let before_method = draft.settings.geometry.geometry_method;
            ui.horizontal(|ui| {
                for choice in crate::wall::WallGeometryMethod::ALL {
                    let selected = draft.settings.geometry.geometry_method == *choice;
                    if ui
                        .add(Button::new(language.translate(choice.name())).selected(selected))
                        .clicked()
                    {
                        draft.settings.geometry.geometry_method = *choice;
                    }
                }
            });
            if draft.settings.geometry.geometry_method != before_method {
                dirty = true;
                commit = true;
                method_changed = true;
            }

            ui.label(Self::t(language, "Reference Line:"));
            let before_line = draft.settings.geometry.reference_line;
            ui.horizontal(|ui| {
                for choice in crate::wall::WallReferenceLine::ALL {
                    let selected = draft.settings.geometry.reference_line == *choice;
                    if ui
                        .add(Button::new(language.translate(choice.name())).selected(selected))
                        .clicked()
                    {
                        draft.settings.geometry.reference_line = *choice;
                    }
                }
            });
            if draft.settings.geometry.reference_line != before_line {
                dirty = true;
                commit = true;
            }
            Self::info_box_drag(
                ui,
                language,
                "Reference Offset",
                &mut draft.settings.geometry.reference_offset,
                &mut dirty,
                &mut commit,
            );

            ui.label(Self::t(language, "Structure:"));
            let structure = ui.add_sized(
                [150., 22.],
                TextEdit::singleline(&mut draft.settings.profile.name),
            );
            if structure.changed() {
                dirty = true;
            }
            if structure.lost_focus() {
                commit = true;
            }

            Self::info_box_tool_drag(
                ui,
                language,
                "Thickness",
                &mut draft.thickness,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_tool_drag(
                ui,
                language,
                "Height",
                &mut draft.height,
                &mut dirty,
                &mut commit,
            );

            ui.label(Self::t(language, "Top Link:"));
            let before_link = draft.settings.geometry.top_link_offset;
            ComboBox::from_id_salt("info_box_wall_tool_top_link")
                .selected_text(top_link_label(before_link, draft.story, &stories, language))
                .width(195.)
                .show_ui(ui, |ui| {
                    for link in -2..=3 {
                        ui.selectable_value(
                            &mut draft.settings.geometry.top_link_offset,
                            link,
                            top_link_label(link, draft.story, &stories, language),
                        );
                    }
                });
            if draft.settings.geometry.top_link_offset != before_link {
                dirty = true;
                commit = true;
            }
            Self::info_box_tool_drag(
                ui,
                language,
                "Top Offset",
                &mut draft.settings.geometry.top_offset,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_tool_drag(
                ui,
                language,
                "Bottom Offset",
                &mut draft.settings.geometry.bottom_offset,
                &mut dirty,
                &mut commit,
            );

            if ui
                .button(Self::t(language, "Floor Plan and Section…"))
                .clicked()
            {
                open_floor_plan = true;
            }
            if ui.button(Self::t(language, "Settings…")).clicked() {
                open_settings = true;
            }
            if ui
                .add_enabled(dirty, Button::new(language.translate("Apply")))
                .clicked()
            {
                commit = true;
            }
            if !state.error.is_empty() {
                ui.colored_label(Color32::from_rgb(224, 125, 105), &state.error);
            }
            state.dirty = dirty;
        }
        if commit
            && self
                .info_box_wall_tool
                .as_ref()
                .is_some_and(|state| state.dirty)
        {
            let _ = self.commit_info_box_wall_tool();
        }
        if method_changed {
            self.editor.cancel_wall_draft();
            self.editor.refresh_wall_status();
        }
        if open_floor_plan {
            self.open_info_box_wall_settings(true);
        }
        if open_settings {
            self.open_info_box_wall_settings(false);
        }
    }
    fn toolbox(&mut self, ui: &mut Ui) {
        egui::Panel::left("toolbox")
            .default_size(188.)
            .size_range(160.0..=240.)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(8, 8)),
            )
            .show(ui, |ui| {
                ScrollArea::vertical().id_salt("toolbox_scroll").show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::label(ui, &Self::t(self.language, "TOOLBOX"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.small_button("⋯").on_hover_text(Self::t(self.language, "Toolbox options"));
                    });
                });
                ui.add_space(7.);
                Self::label(ui, &Self::t(self.language, "Selection & Region"));
                for (t, label, key) in [
                    (Tool::Select, "Arrow", "V"),
                    (Tool::Marquee, "Marquee", "M"),
                ] {
                    let active = self.preview_tool.is_none() && self.editor.tool == t;
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, Some(key)) {
                        self.editor.set_tool(t);
                        self.preview_tool = None;
                        self.tracker_locked = false;
                    }
                }
                ui.add_space(8.);
                ui.separator();
                ui.add_space(4.);
                CollapsingHeader::new(Self::t(self.language, "Design")).default_open(true).show(ui, |ui| {
                for (t, label, key) in [
                    (Tool::Wall, "Wall", "W"),
                    (Tool::Slab, "Slab", "L"),
                ] {
                    let active = self.preview_tool.is_none() && self.editor.tool == t;
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, Some(key)) {
                        self.editor.set_tool(t);
                        self.preview_tool = None;
                        self.tracker_locked = false;
                    }
                }
                if Self::toolbox_tool_row(ui, "Stair", &self.language.translate("Stair"), self.preview_tool.is_none() && self.editor.tool == Tool::Stair, Some("S")) {
                    self.activate_stair_tool();
                }
                for name in ["Column", "Beam", "Door", "Window", "Object", "Lamp", "Roof", "Shell", "Railing", "Curtain Wall", "Skylight", "Corner-Window", "Opening", "Wall End", "Mesh", "Morph", "Zone"] {
                    if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), self.preview_tool.as_deref() == Some(name), None) {
                        self.preview_tool = Some(name.to_owned());
                        self.editor.set_tool(Tool::Select);
                        self.editor.status = format!("{name} tool selected · model interaction is not available in this demo");
                    }
                }
                });
                ui.add_space(10.);
                ui.separator();
                ui.add_space(4.);
                for (group, names) in [
                    ("Viewpoint", &(["Section", "Elevation", "Interior Elevation", "Detail", "Worksheet", "3D Document"][..])),
                    ("Document", &(["Dimension", "Level Dimension", "Radial Dimension", "Angle Dimension", "Text", "Label", "Fill", "Line", "Arc / Circle", "Polyline", "Spline", "Hotspot", "Figure", "Drawing", "Grid Element", "Camera"][..])),
                    ("Structural Engineering", &(["Support", "Link", "Point Load", "Line Load", "Surface Load"][..])),
                    ("MEP Engineering", &(["Duct Route", "Pipe Route", "Cable Route", "Branch", "Fitting", "Terminal", "Accessory", "Equipment"][..])),
                ] {
                    CollapsingHeader::new(Self::t(self.language, group)).default_open(false).show(ui, |ui| {
                        for name in names {
                            let polyline = *name == "Polyline";
                            let active = if polyline {
                                self.preview_tool.is_none() && self.editor.tool == Tool::Polyline
                            } else {
                                self.preview_tool.as_deref() == Some(*name)
                            };
                            if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), active, polyline.then_some("P")) {
                                if polyline {
                                    self.activate_polyline_tool();
                                    continue;
                                }
                                self.preview_tool = Some((*name).to_owned());
                                self.editor.set_tool(Tool::Select);
                                self.editor.status = format!("{name} tool selected · interface preview");
                            }
                        }
                    });
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "OPENARCHI\n{}\nApple Silicon / Metal",
                            Self::t(self.language, "Metres · Z up")
                        ))
                            .small()
                            .color(MUTED),
                    );
                    ui.separator();
                });
                });
            });
    }
    fn inspector(&mut self, ui: &mut Ui) {
        egui::Panel::right("inspector")
            .default_size(318.)
            .size_range(285.0..=440.)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(9, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::label(ui, &Self::t(self.language, "NAVIGATOR"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        Self::vector_icon_button(
                            ui,
                            "Options",
                            &Self::t(self.language, "Navigator options"),
                            false,
                        );
                    });
                });
                ui.horizontal(|ui| {
                    for (view, icon, tip) in [
                        (NavigatorView::ProjectMap, "Project Map", "Project Map"),
                        (NavigatorView::ViewMap, "View Map", "View Map"),
                        (NavigatorView::LayoutBook, "Layout Book", "Layout Book"),
                        (NavigatorView::Publisher, "Publisher", "Publisher Sets"),
                    ] {
                        if Self::vector_icon_button(
                            ui,
                            icon,
                            &Self::t(self.language, tip),
                            self.navigator_view == view,
                        ) {
                            self.navigator_view = view;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    Self::inline_vector_icon(ui, "Search", MUTED);
                    ui.add_sized(
                        [ui.available_width(), 25.],
                        TextEdit::singleline(&mut self.navigator_search)
                            .hint_text(Self::t(self.language, "Search navigator")),
                    );
                });
                ui.separator();
                match self.navigator_view {
                    NavigatorView::ProjectMap => {
                        ui.horizontal(|ui| {
                            Self::inline_vector_icon(ui, "Home", ACCENT);
                            ui.strong(&self.editor.project.name);
                        });
                        CollapsingHeader::new(Self::t(self.language, "Stories"))
                            .default_open(true)
                            .show(ui, |ui| {
                                for story in self.editor.project.stories.clone() {
                                    let selected = self.editor.story == story.id;
                                    let text = format!(
                                        "{:02}  {}     {:+.2} m",
                                        story.id, story.name, story.elevation
                                    );
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 25.],
                                            Button::new(text).selected(selected),
                                        )
                                        .clicked()
                                    {
                                        self.editor.switch_story(story.id);
                                        self.draft_id = None;
                                    }
                                }
                            });
                        for node in [
                            "Sections",
                            "Elevations",
                            "Interior Elevations",
                            "Worksheets",
                            "Details",
                            "3D Documents",
                            "3D",
                            "Schedules",
                            "Project Indexes",
                            "Lists",
                            "Info",
                            "Help",
                        ] {
                            CollapsingHeader::new(Self::t(self.language, node)).show(ui, |ui| {
                                let entries: &[&str] = match node {
                                    "Elevations" => &[
                                        "East Elevation (Auto-rebuild Model)",
                                        "North Elevation (Auto-rebuild Model)",
                                        "South Elevation (Auto-rebuild Model)",
                                        "West Elevation (Auto-rebuild Model)",
                                    ],
                                    "3D" => &["Generic Perspective", "Generic Axonometry"],
                                    "Schedules" => &[
                                        "Wall Schedule",
                                        "Door Schedule",
                                        "Window Schedule",
                                        "Object Inventory",
                                    ],
                                    "Project Indexes" => {
                                        &["Change List", "Drawing List", "Sheet Index"]
                                    }
                                    _ => &["No saved views"],
                                };
                                for entry in entries {
                                    if Self::navigator_item(
                                        ui,
                                        if node == "3D" {
                                            "Perspective"
                                        } else {
                                            "Elevation View"
                                        },
                                        &Self::t(self.language, entry),
                                    ) {
                                        self.editor.status = format!("Navigator item · {entry}");
                                    }
                                }
                            });
                        }
                    }
                    NavigatorView::ViewMap => {
                        ui.strong(Self::t(self.language, "View Map"));
                        for group in [
                            "Project Views",
                            "Floor Plans",
                            "Ceiling Plans",
                            "Sections & Elevations",
                            "3D Views",
                            "Schedules",
                        ] {
                            CollapsingHeader::new(Self::t(self.language, group))
                                .default_open(group == "Floor Plans")
                                .show(ui, |ui| {
                                    for entry in [
                                        "Ground Floor",
                                        "South Elevation",
                                        "Generic Perspective",
                                        "Generic Axonometry",
                                    ] {
                                        if Self::navigator_item(
                                            ui,
                                            if group == "Floor Plans" {
                                                "Floor Plan"
                                            } else {
                                                "View Map"
                                            },
                                            &Self::t(self.language, entry),
                                        ) {
                                            self.editor.status = format!("View Map · {entry}");
                                        }
                                    }
                                });
                        }
                    }
                    NavigatorView::LayoutBook => {
                        ui.strong(Self::t(self.language, "Layout Book"));
                        for group in [
                            "Master Layouts",
                            "Layout Subsets",
                            "Documentation Set",
                            "Plans",
                            "Elevations",
                        ] {
                            CollapsingHeader::new(Self::t(self.language, group))
                                .default_open(group == "Documentation Set")
                                .show(ui, |ui| {
                                    for entry in
                                        ["A-001 Cover", "A-101 Floor Plan", "A-201 Elevations"]
                                    {
                                        if Self::navigator_item(
                                            ui,
                                            "Layout Book",
                                            &Self::t(self.language, entry),
                                        ) {
                                            self.editor.status = format!("Layout · {entry}");
                                        }
                                    }
                                });
                        }
                    }
                    NavigatorView::Publisher => {
                        ui.strong(Self::t(self.language, "Publisher Sets"));
                        for group in [
                            "Publisher Sets",
                            "PDF Issue",
                            "BIMx Hyper-model",
                            "DWG Exchange",
                        ] {
                            CollapsingHeader::new(Self::t(self.language, group))
                                .default_open(group == "Publisher Sets")
                                .show(ui, |ui| {
                                    for entry in
                                        ["Current Project", "Documentation Set", "3D Model"]
                                    {
                                        if Self::navigator_item(
                                            ui,
                                            "Publisher",
                                            &Self::t(self.language, entry),
                                        ) {
                                            self.editor.status =
                                                format!("Publisher item · {entry}");
                                        }
                                    }
                                });
                        }
                        ui.add_sized(
                            [ui.available_width(), 28.],
                            Button::new(self.language.translate("Publish…")),
                        );
                    }
                }
                ui.checkbox(
                    &mut self.all_stories,
                    Self::t(self.language, "Show all stories in 3D"),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Properties,
                        Self::t(self.language, "Properties"),
                    );
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Library,
                        Self::t(self.language, "Library"),
                    );
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Assistant,
                        Self::t(self.language, "AI Assistant"),
                    );
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt("inspector_scroll")
                    .show(ui, |ui| match self.tab {
                        Tab::Properties => self.properties(ui),
                        Tab::Library => self.library(ui),
                        Tab::Assistant => self.assistant(ui),
                    });
            });
    }
    fn properties(&mut self, ui: &mut Ui) {
        if self.draft_id != self.editor.selected || self.editor.selected.is_none() {
            self.draft_id = self.editor.selected;
            self.draft = self
                .editor
                .project
                .elements
                .iter()
                .find(|e| Some(e.id) == self.editor.selected)
                .cloned();
        }
        if let Some(mut e) = self.draft.clone() {
            Self::label(ui, &Self::t(self.language, "ELEMENT SETTINGS"));
            ui.add_space(8.);
            ui.horizontal(|ui| {
                ui.heading(format!("{} {:03}", e.kind(), e.id));
            });
            let measure = if matches!(e.shape, Shape::Polyline { .. }) {
                format!("{} · {:.2} m", e.material.name(), e.area())
            } else {
                format!("{} · {:.2} m²", e.material.name(), e.area())
            };
            ui.label(RichText::new(measure).color(MUTED));
            ui.add_space(10.);
            let stories = self.editor.project.stories.clone();
            Self::element_fields(ui, &mut e, &stories, "inspector", self.language);
            if let Shape::Wall {
                thickness, height, ..
            } = e.shape
            {
                let settings: WallSettings = e.wall_settings.clone().unwrap_or_default();
                ui.add_space(14.);
                Self::label(ui, self.language.translate("WALL SETTINGS"));
                Grid::new(("inspector_wall_settings", e.id))
                    .num_columns(2)
                    .spacing([14., 6.])
                    .show(ui, |ui| {
                        Self::property_row(
                            ui,
                            self.language,
                            "Thickness",
                            &format!("{thickness:.3} m"),
                        );
                        Self::property_row(ui, self.language, "Height", &format!("{height:.3} m"));
                        Self::property_row(
                            ui,
                            self.language,
                            "Reference Line",
                            self.language
                                .translate(settings.geometry.reference_line.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Reference Offset",
                            &format!("{:.3} m", settings.geometry.reference_offset),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Top / Bottom Offset",
                            &format!(
                                "{:.3} / {:.3} m",
                                settings.geometry.top_offset, settings.geometry.bottom_offset
                            ),
                        );
                        Self::property_row(ui, self.language, "Structure", &settings.profile.name);
                        Self::property_row(
                            ui,
                            self.language,
                            "Floor Plan Display",
                            self.language
                                .translate(settings.floor_plan.floor_plan_display.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Uncut Lines",
                            self.language
                                .translate(settings.floor_plan.uncut_lines.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Left Surface",
                            &settings.model.left_surface,
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Right Surface",
                            &settings.model.right_surface,
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "End Surface",
                            &settings.model.end_surface,
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Structural Function",
                            self.language
                                .translate(settings.structural.structural_function.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Property ID",
                            &settings.classification.property_id,
                        );
                    });
                ui.add_space(8.);
                if ui
                    .add_sized(
                        [ui.available_width(), 32.],
                        Button::new(Self::t(self.language, "Edit Wall Settings  ⌘T")),
                    )
                    .clicked()
                {
                    self.wall_settings_dialog = WallSettingsDialog::selected(&e);
                    self.element_settings = None;
                    self.element_settings_error.clear();
                }
            }
            ui.add_space(14.);
            if ui
                .add_sized(
                    [ui.available_width(), 34.],
                    Button::new(
                        RichText::new(Self::t(self.language, "Apply changes"))
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT),
                )
                .clicked()
            {
                match self.editor.replace_element(e.clone()) {
                    Ok(()) => {
                        self.editor.status = "Properties updated · Both views synchronized".into()
                    }
                    Err(err) => self.editor.status = err,
                }
            }
            ui.horizontal(|ui| {
                if ui.button(Self::t(self.language, "Duplicate")).clicked() {
                    self.editor.duplicate();
                }
                if ui.button(Self::t(self.language, "Delete")).clicked() {
                    self.editor.delete();
                }
            });
            self.draft = Some(e);
        } else {
            ui.add_space(12.);
            ui.heading(self.language.translate("Build with precision."));
            ui.label(
                RichText::new(self.language.translate(
                    "Select an element in either view to inspect and edit its properties.",
                ))
                .color(MUTED),
            );
            ui.add_space(20.);
            Self::label(ui, self.language.translate("PROJECT QUANTITIES"));
            let p = &self.editor.project;
            let walls = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Wall { .. }))
                .count();
            let stairs = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Stair { .. }))
                .count();
            let area: f32 = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Slab { .. }))
                .map(Element::area)
                .sum();
            ui.add_space(8.);
            ui.heading(format!("{}", p.elements.len()));
            ui.label(if self.language == UiLanguage::SimplifiedChinese {
                format!("构件 / {walls} 面墙 / {stairs} 座楼梯")
            } else {
                format!("elements / {walls} walls / {stairs} stairs")
            });
            ui.add_space(12.);
            ui.heading(format!("{area:.1} m²"));
            ui.label(self.language.translate("total slab footprint"));
        }
        ui.add_space(20.);
        ui.separator();
        CollapsingHeader::new(self.language.translate("Elements on current story")).show(
            ui,
            |ui| {
                for e in self
                    .editor
                    .project
                    .elements
                    .clone()
                    .iter()
                    .filter(|e| e.story == self.editor.story)
                {
                    if ui
                        .selectable_label(
                            self.editor.selected == Some(e.id),
                            format!("{}  ·  {}", e.name, e.material.name()),
                        )
                        .clicked()
                    {
                        self.editor.selected = Some(e.id);
                    }
                }
            },
        );
    }
    fn number(ui: &mut Ui, language: UiLanguage, label: &str, n: &mut f32) {
        ui.label(language.translate(label));
        ui.add(DragValue::new(n).speed(0.01).fixed_decimals(3).suffix(" m"));
        ui.end_row();
    }
    fn property_row(ui: &mut Ui, language: UiLanguage, label: &str, value: &str) {
        ui.label(RichText::new(language.translate(label)).color(MUTED));
        ui.label(RichText::new(value).color(INK));
        ui.end_row();
    }
    fn element_fields(
        ui: &mut Ui,
        element: &mut Element,
        stories: &[Story],
        namespace: &str,
        language: UiLanguage,
    ) {
        ui.label(language.translate("Element name"));
        ui.text_edit_singleline(&mut element.name);
        ui.add_space(8.);
        Grid::new(("element_settings_fields", namespace, element.id))
            .num_columns(2)
            .spacing([16., 10.])
            .show(ui, |ui| {
                ui.label(language.translate("Home Story"));
                ComboBox::from_id_salt(("element_story", namespace, element.id))
                    .selected_text(
                        stories
                            .iter()
                            .find(|story| story.id == element.story)
                            .map_or_else(
                                || format!("Level {:02}", element.story),
                                |story| story.name.clone(),
                            ),
                    )
                    .width(180.)
                    .show_ui(ui, |ui| {
                        for story in stories {
                            ui.selectable_value(&mut element.story, story.id, &story.name);
                        }
                    });
                ui.end_row();
                ui.label(language.translate("Surface"));
                ComboBox::from_id_salt(("element_material", namespace, element.id))
                    .selected_text(element.material.name())
                    .width(180.)
                    .show_ui(ui, |ui| {
                        for material in Material::ALL {
                            ui.selectable_value(&mut element.material, material, material.name());
                        }
                    });
                ui.end_row();
                match &mut element.shape {
                    Shape::Wall {
                        start,
                        end,
                        thickness,
                        height,
                    } => {
                        Self::number(ui, language, "Start X", &mut start.x);
                        Self::number(ui, language, "Start Y", &mut start.y);
                        Self::number(ui, language, "End X", &mut end.x);
                        Self::number(ui, language, "End Y", &mut end.y);
                        Self::number(ui, language, "Thickness", thickness);
                        Self::number(ui, language, "Height", height);
                    }
                    Shape::Slab {
                        min,
                        max,
                        thickness,
                    } => {
                        Self::number(ui, language, "Min X", &mut min.x);
                        Self::number(ui, language, "Min Y", &mut min.y);
                        Self::number(ui, language, "Max X", &mut max.x);
                        Self::number(ui, language, "Max Y", &mut max.y);
                        Self::number(ui, language, "Thickness", thickness);
                    }
                    Shape::Stair { spec } => {
                        Self::number(ui, language, "Start X", &mut spec.start.x);
                        Self::number(ui, language, "Start Y", &mut spec.start.y);
                        Self::number(ui, language, "End X", &mut spec.end.x);
                        Self::number(ui, language, "End Y", &mut spec.end.y);
                        Self::number(ui, language, "Width", &mut spec.width);
                        Self::number(ui, language, "Total rise", &mut spec.total_rise);
                        ui.label(language.translate("Risers"));
                        ui.add(DragValue::new(&mut spec.risers).range(2..=100));
                        ui.end_row();
                        Self::number(ui, language, "Slab thickness", &mut spec.slab_thickness);
                        Self::number(ui, language, "Handrail height", &mut spec.handrail_height);
                    }
                    Shape::Polyline { points, .. } => {
                        ui.label(language.translate("Nodes"));
                        ui.label(points.len().to_string());
                        ui.end_row();
                        for (index, point) in points.iter_mut().enumerate() {
                            Self::number(
                                ui,
                                language,
                                &format!("Node {} X", index + 1),
                                &mut point.x,
                            );
                            Self::number(
                                ui,
                                language,
                                &format!("Node {} Y", index + 1),
                                &mut point.y,
                            );
                        }
                    }
                }
            });
    }
    fn element_settings_dialog(&mut self, ctx: &Context) {
        let Some(mut element) = self.element_settings.clone() else {
            return;
        };
        let stories = self.editor.project.stories.clone();
        let mut apply = false;
        let mut accept = false;
        let mut cancel = false;
        let modal =
            Modal::new(Id::new("element_settings_dialog")).show(ctx, |ui| {
                ui.set_min_width(470.);
                ui.heading(format!(
                    "{} {}  ·  {:03}",
                    self.language.translate(element.kind()),
                    self.language.translate("Settings"),
                    element.id
                ));
                ui.label(
                    RichText::new(self.language.translate(
                        "Changes preview after applying and synchronize across all views.",
                    ))
                    .color(MUTED),
                );
                ui.separator();
                Self::element_fields(ui, &mut element, &stories, "dialog", self.language);
                if !self.element_settings_error.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(224, 125, 105),
                        &self.element_settings_error,
                    );
                }
                ui.add_space(12.);
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [94., 32.],
                            Button::new(Self::t(self.language, "OK")).fill(ACCENT),
                        )
                        .clicked()
                    {
                        accept = true;
                    }
                    if ui.button(Self::t(self.language, "Apply")).clicked() {
                        apply = true;
                    }
                    if ui.button(Self::t(self.language, "Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });

        self.element_settings = Some(element);
        if apply || accept {
            match self.apply_element_settings() {
                Ok(()) if accept => self.element_settings = None,
                Ok(()) => {}
                Err(error) => self.element_settings_error = error,
            }
        }
        if cancel || modal.should_close() {
            self.element_settings = None;
            self.element_settings_error.clear();
        }
    }
    fn wall_settings_dialog(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.wall_settings_dialog.take() else {
            return;
        };
        let stories = self.editor.project.stories.clone();
        let action = dialog.show(ctx, self.language, &stories);
        if action == WallDialogAction::None {
            self.wall_settings_dialog = Some(dialog);
            return;
        }
        if action == WallDialogAction::Cancel {
            self.editor.status = self.language.translate("Wall Settings cancelled").into();
            return;
        }
        let result = match dialog.mode {
            crate::wall_ui::WallSettingsMode::Selected(id) => {
                let Some(mut element) = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|element| element.id == id)
                    .cloned()
                else {
                    self.editor.status = self
                        .language
                        .translate("Wall Settings · The selected wall no longer exists")
                        .into();
                    return;
                };
                dialog
                    .draft
                    .apply_to_element(&mut element)
                    .and_then(|()| self.editor.replace_element(element))
            }
            crate::wall_ui::WallSettingsMode::Default => dialog
                .draft
                .validate()
                .map(|()| self.apply_wall_default_draft(&dialog.draft)),
        };
        match result {
            Ok(()) => {
                match dialog.mode {
                    crate::wall_ui::WallSettingsMode::Selected(_) => {
                        self.editor.status =
                            "Wall settings applied · Plan and 3D synchronized".into()
                    }
                    crate::wall_ui::WallSettingsMode::Default => {
                        self.editor.status =
                            "Wall default settings applied · New walls use these values".into()
                    }
                }
                dialog.error.clear();
                if action != WallDialogAction::Ok {
                    self.wall_settings_dialog = Some(dialog);
                }
            }
            Err(error) => {
                dialog.error = error;
                self.wall_settings_dialog = Some(dialog);
            }
        }
    }
    fn stair_settings_dialog(&mut self, ctx: &Context) {
        if !self.stair_settings_open {
            return;
        }
        let mut apply = false;
        let mut close = false;
        let mut cancel = false;
        let modal = Modal::new(Id::new("stair_settings_dialog")).show(ctx, |ui| {
            ui.set_min_width(430.);
            ui.heading(self.language.translate("Stair Settings"));
            ui.label(
                RichText::new(
                    self.language.translate(
                        "Straight flight · click two plan points after applying settings.",
                    ),
                )
                .color(MUTED),
            );
            ui.separator();
            Grid::new("stair_settings_grid")
                .num_columns(2)
                .spacing([18., 11.])
                .show(ui, |ui| {
                    Self::number(
                        ui,
                        self.language,
                        "Width",
                        &mut self.stair_settings_draft.width,
                    );
                    Self::number(
                        ui,
                        self.language,
                        "Total rise",
                        &mut self.stair_settings_draft.total_rise,
                    );
                    ui.label(self.language.translate("Risers"));
                    ui.add(DragValue::new(&mut self.stair_settings_draft.risers).range(2..=100));
                    ui.end_row();
                    Self::number(
                        ui,
                        self.language,
                        "Slab thickness",
                        &mut self.stair_settings_draft.slab_thickness,
                    );
                    Self::number(
                        ui,
                        self.language,
                        "Handrail height",
                        &mut self.stair_settings_draft.handrail_height,
                    );
                });
            ui.label(format!(
                "Rise per step: {:.0} mm",
                self.stair_settings_draft.total_rise * 1000.
                    / f32::from(self.stair_settings_draft.risers.max(1))
            ));
            if !self.stair_settings_error.is_empty() {
                ui.colored_label(Color32::from_rgb(224, 125, 105), &self.stair_settings_error);
            }
            ui.add_space(10.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [112., 32.],
                        Button::new(self.language.translate("Start Drawing")).fill(ACCENT),
                    )
                    .clicked()
                {
                    apply = true;
                    close = true;
                }
                if ui.button(self.language.translate("Apply")).clicked() {
                    apply = true;
                }
                if ui.button(self.language.translate("Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if apply {
            match self.stair_settings_draft.validate() {
                Ok(()) => {
                    self.editor.stair_spec = self.stair_settings_draft.clone();
                    self.stair_settings_error.clear();
                    self.editor.status =
                        "Stair settings applied · click start and end points in plan".into();
                }
                Err(error) => self.stair_settings_error = error.into(),
            }
        }
        let dismissed = cancel || modal.should_close();
        if (close && self.stair_settings_error.is_empty()) || dismissed {
            self.stair_settings_open = false;
            if dismissed {
                self.editor.set_tool(Tool::Select);
            }
        }
    }
    fn command_preview_dialog(&mut self, ctx: &Context) {
        let Some(title) = self.command_dialog.clone() else {
            return;
        };
        let mut close = false;
        let mut applied = false;
        let modal = Modal::new(Id::new("command_preview_dialog")).show(ctx, |ui| {
            ui.set_min_width(610.);
            ui.set_min_height(430.);
            ui.heading(&title);
            ui.label(RichText::new(self.language.translate("Configuration interface preview")).small().color(MUTED));
            ui.separator();
            ui.horizontal(|ui| {
                for (index, tab) in [(0, "General"), (1, "Geometry"), (2, "Representation"), (3, "Classification") ] {
                    ui.selectable_value(&mut self.command_dialog_tab, index, tab);
                }
            });
            ui.separator();
            ScrollArea::vertical().max_height(290.).show(ui, |ui| {
                match self.command_dialog_tab {
                    0 => {
                        Grid::new("command_preview_general").num_columns(2).spacing([14., 9.]).show(ui, |ui| {
                            ui.label(self.language.translate("Name")); ui.add(TextEdit::singleline(&mut self.command_dialog_name).desired_width(360.)); ui.end_row();
                            ui.label(self.language.translate("Element ID")); ui.add(TextEdit::singleline(&mut self.preview_element_id).desired_width(180.)); ui.end_row();
                            ui.label(self.language.translate("Home Story")); ComboBox::from_id_salt("command_preview_story").selected_text(self.language.translate("Current Story")).show_ui(ui, |ui| { for story in self.editor.project.stories.clone() { let _ = ui.selectable_label(story.id == self.editor.story, format!("{} · {}", story.id, story.name)); } }); ui.end_row();
                            ui.label(self.language.translate("Layer")); ComboBox::from_id_salt("command_preview_layer").selected_text(&self.preview_layer).show_ui(ui, |ui| { for layer in ["01 Site", "02 Drafting", "03 Plans - Preliminary", "04 Plans - Approval", "05 Plans - Detailed", "06 Plans - Mechanical", "07 Plans - Structural", "08 Rendering", "09 Layouting"] { ui.selectable_value(&mut self.preview_layer, layer.to_owned(), layer); } }); ui.end_row();
                            ui.label(self.language.translate("State")); ui.checkbox(&mut self.command_dialog_enabled, self.language.translate("Visible and enabled")); ui.end_row();
                        });
                    }
                    1 => {
                        Grid::new("command_preview_geometry").num_columns(2).spacing([14., 9.]).show(ui, |ui| {
                            ui.label(self.language.translate("Length / Width")); ui.add(DragValue::new(&mut self.command_dialog_number).speed(0.05).fixed_decimals(3).suffix(" m")); ui.end_row();
                            ui.label(self.language.translate("Height / Offset")); ui.add(DragValue::new(&mut self.preview_offset).speed(0.05).fixed_decimals(3).suffix(" m")); ui.end_row();
                            ui.label(self.language.translate("Reference")); ComboBox::from_id_salt("command_preview_reference").selected_text(&self.command_dialog_reference).show_ui(ui, |ui| { for value in ["Center", "Inside", "Outside"] { ui.selectable_value(&mut self.command_dialog_reference, value.into(), value); } }); ui.end_row();
                            ui.label(self.language.translate("Method")); ComboBox::from_id_salt("command_preview_method").selected_text(&self.command_dialog_method).show_ui(ui, |ui| { for value in ["Default", "Single", "Continuous", "Polygonal", "Rectangular"] { ui.selectable_value(&mut self.command_dialog_method, value.into(), value); } }); ui.end_row();
                        });
                    }
                    2 => {
                        for value in ["Model View Options", "Pen Set", "Surface Override", "Floor Plan Display", "3D Representation", "Cut Surface", "Line Type"] {
                            ui.horizontal(|ui| { ui.label(value); ComboBox::from_id_salt(value).selected_text(&self.command_dialog_representation).show_ui(ui, |ui| { for option in ["By Element", "By Building Material", "Override", "Hidden", "Visible"] { ui.selectable_value(&mut self.command_dialog_representation, option.into(), option); } }); });
                        }
                    }
                    _ => {
                        for value in ["Classification", "Property Set", "Building Material", "Zone Category", "Renovation Status", "Structural Function"] {
                            ui.horizontal(|ui| { ui.label(value); ComboBox::from_id_salt(value).selected_text(&self.command_dialog_classification).show_ui(ui, |ui| { for option in ["Not assigned", "Architecture", "Structure", "Existing", "New"] { ui.selectable_value(&mut self.command_dialog_classification, option.into(), option); } }); });
                        }
                    }
                }
                ui.separator();
                ui.label(RichText::new(self.language.translate("Preview values are local UI state. This command has no connected model backend.")).small().color(MUTED));
            });
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_sized([90., 30.], Button::new(self.language.translate("OK")).fill(ACCENT)).clicked() { close = true; applied = true; }
                if ui.button(self.language.translate("Apply")).clicked() { applied = true; }
                if ui.button(self.language.translate("Cancel")).clicked() { close = true; }
            });
        });
        if applied {
            self.editor.status = format!("{} · preview values applied locally", title);
        }
        if close || modal.should_close() {
            self.command_dialog = None;
        }
    }
    fn library(&mut self, ui: &mut Ui) {
        Self::label(ui, "PARAMETRIC PRESETS");
        ui.add_space(8.);
        ui.add(
            TextEdit::singleline(&mut self.library_search)
                .hint_text(self.language.translate("Search wall or slab…"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(10.);
        for (name, desc, tool, material, t, h) in [
            (
                "Exterior / plaster",
                "220 mm · 3.00 m high",
                Tool::Wall,
                Material::Plaster,
                0.22,
                3.,
            ),
            (
                "Partition / plaster",
                "120 mm · 2.80 m high",
                Tool::Wall,
                Material::Plaster,
                0.12,
                2.8,
            ),
            (
                "Structure / concrete",
                "300 mm · 3.20 m high",
                Tool::Wall,
                Material::Concrete,
                0.3,
                3.2,
            ),
            (
                "Floor / concrete",
                "280 mm structural slab",
                Tool::Slab,
                Material::Concrete,
                0.28,
                3.,
            ),
            (
                "Terrace / timber",
                "160 mm deck slab",
                Tool::Slab,
                Material::Timber,
                0.16,
                3.,
            ),
        ] {
            if !format!(
                "{name} {desc} {tool:?} {} {}",
                self.language.translate(name),
                self.language.translate(desc)
            )
            .to_lowercase()
            .contains(&self.library_search.to_lowercase())
            {
                continue;
            }
            Frame::new()
                .fill(PANEL_ALT)
                .corner_radius(6)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 48.), Sense::hover());
                    let c = material.color();
                    let col = Color32::from_rgb(
                        (c[0] * 255.) as u8,
                        (c[1] * 255.) as u8,
                        (c[2] * 255.) as u8,
                    );
                    let r = Rect::from_center_size(
                        r.center(),
                        if tool == Tool::Wall {
                            vec2(96., 32.)
                        } else {
                            vec2(140., 15.)
                        },
                    );
                    ui.painter().rect_filled(r, 1., col);
                    ui.painter()
                        .rect_stroke(r, 1., Stroke::new(1., INK), StrokeKind::Inside);
                    ui.strong(self.language.translate(name));
                    ui.label(
                        RichText::new(self.language.translate(desc))
                            .small()
                            .color(MUTED),
                    );
                    if ui.button(self.language.translate("Use preset")).clicked() {
                        self.editor.set_tool(tool);
                        self.preview_tool = None;
                        self.editor.material = material;
                        self.editor.thickness = t;
                        self.editor.height = h;
                        self.editor.status =
                            format!("{name} preset active · Click in plan to place");
                    }
                });
            ui.add_space(8.);
        }
        ui.label(
            RichText::new(self.language.translate(
                "Built-in wall / slab presets. External object catalogs are not connected.",
            ))
            .small()
            .color(MUTED),
        );
    }
    fn assistant(&mut self, ui: &mut Ui) {
        Self::label(
            ui,
            self.language.translate("DESIGN ASSISTANT  /  LOCAL DEMO"),
        );
        ui.add_space(8.);
        let mut prompt = None;
        ui.horizontal_wrapped(|ui| {
            for label in ["Summary", "Quantities", "Check model"] {
                if ui.small_button(self.language.translate(label)).clicked() {
                    prompt = Some(label.to_string());
                }
            }
        });
        ui.add_space(8.);
        for (user, text) in &self.messages {
            Frame::new()
                .fill(if *user {
                    Color32::from_rgb(45, 83, 99)
                } else {
                    PANEL_ALT
                })
                .corner_radius(6)
                .inner_margin(12)
                .show(ui, |ui| {
                    Self::label(
                        ui,
                        if *user {
                            "YOU"
                        } else {
                            self.language.translate("OPENARCHI · LOCAL RULES")
                        },
                    );
                    ui.label(text);
                });
            ui.add_space(8.);
        }
        ui.add(
            TextEdit::multiline(&mut self.assistant_input)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text(self.language.translate("Ask about this model…")),
        );
        if ui.button(self.language.translate("Send")).clicked()
            && !self.assistant_input.trim().is_empty()
        {
            prompt = Some(std::mem::take(&mut self.assistant_input));
        }
        if let Some(q) = prompt {
            let lower = q.to_lowercase();
            let p = &self.editor.project;
            let answer = if lower.contains("check") || lower.contains("检查") {
                match p.validate() {
                    Ok(()) => format!(
                        "Geometry validation passed for {} elements.\n\nChecks: finite coordinates, positive dimensions, unique IDs and story references.\n\nThis is not a building-code or clash check.",
                        p.elements.len()
                    ),
                    Err(e) => e,
                }
            } else if lower.contains("quant") || lower.contains("数量") {
                let area: f32 = p
                    .elements
                    .iter()
                    .filter(|e| matches!(e.shape, crate::model::Shape::Slab { .. }))
                    .map(Element::area)
                    .sum();
                let length: f32 = p
                    .elements
                    .iter()
                    .map(|e| match e.shape {
                        crate::model::Shape::Wall { start, end, .. } => start.distance(end),
                        _ => 0.,
                    })
                    .sum();
                format!(
                    "Wall centerline length: {length:.2} m\nSlab footprint total: {area:.2} m²\n\nGross quantities; intersections and openings are not deducted."
                )
            } else if lower.contains("summary") || lower.contains("总结") {
                format!(
                    "{}\n{} stories · {} model elements\n\nSelected story: Level {:02}\nBoth views share one in-memory model. Save JSON to preserve this design.",
                    p.name,
                    p.stories.len(),
                    p.elements.len(),
                    self.editor.story
                )
            } else {
                "Available local commands: Summary, Quantities, Check model.\n\nFree-form AI design and model generation require a future provider connection.".into()
            };
            self.messages.push((true, q));
            self.messages.push((false, answer));
        }
    }
    fn world(&self, r: Rect, p: Pos2) -> Point {
        let radians = self
            .editor
            .project
            .plan_options
            .normalized_orientation()
            .to_radians();
        let (sin, cos) = radians.sin_cos();
        let view_x = (p.x - r.center().x) / self.plan_scale;
        let view_y = -(p.y - r.center().y) / self.plan_scale;
        Point::new(
            self.plan_center.x + view_x * cos + view_y * sin,
            self.plan_center.y - view_x * sin + view_y * cos,
        )
    }
    fn screen(&self, r: Rect, p: Point) -> Pos2 {
        let radians = self
            .editor
            .project
            .plan_options
            .normalized_orientation()
            .to_radians();
        let (sin, cos) = radians.sin_cos();
        let x = p.x - self.plan_center.x;
        let y = p.y - self.plan_center.y;
        pos2(
            r.center().x + (x * cos - y * sin) * self.plan_scale,
            r.center().y - (x * sin + y * cos) * self.plan_scale,
        )
    }
    fn plan_pen(&self) -> crate::view_settings::PenPalette {
        self.editor.project.plan_options.pen_set.palette()
    }
    fn plan_style(&self, element: &Element) -> Option<crate::view_settings::PlanElementStyle> {
        self.editor
            .project
            .plan_options
            .style_for(element, self.plan_pen())
    }
    fn plan_pen_px(&self, width_mm: f32) -> f32 {
        let world_width = width_mm / 1000. * self.editor.project.plan_options.drawing_scale as f32;
        (world_width * self.plan_scale).clamp(0.45, 10.)
    }
    fn plan_color(rgb: [u8; 3]) -> Color32 {
        Color32::from_rgb(rgb[0], rgb[1], rgb[2])
    }
    fn selected_polyline_hit(
        &self,
        point: Point,
        tolerance: f32,
    ) -> Option<(u64, Option<usize>, Option<usize>)> {
        let selected = self.editor.selection_ids();
        for element in self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| selected.contains(&e.id))
        {
            let Shape::Polyline { points, arcs } = &element.shape else {
                continue;
            };
            if let Some((index, _)) = points
                .iter()
                .enumerate()
                .map(|(index, node)| (index, node.distance(point)))
                .filter(|(_, distance)| *distance <= tolerance)
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                return Some((element.id, Some(index), None));
            }
            if let Some((index, _)) = points
                .windows(2)
                .enumerate()
                .map(|(index, edge)| {
                    let path = arcs
                        .get(index)
                        .and_then(|arc| *arc)
                        .and_then(|arc| geometry::arc_points(edge[0], edge[1], arc))
                        .unwrap_or_else(|| vec![edge[0], edge[1]]);
                    let distance = path
                        .windows(2)
                        .map(|pair| geometry::segment_distance(point, pair[0], pair[1]))
                        .fold(f32::INFINITY, f32::min);
                    (index, distance)
                })
                .filter(|(_, distance)| *distance <= tolerance)
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                return Some((element.id, None, Some(index)));
            }
        }
        None
    }
    fn select_cursor_feedback(&self, point: Point) -> CursorFeedback {
        match geometry::plan_hover_kind(
            &self.editor.project,
            self.editor.story,
            point,
            8. / self.plan_scale,
            6. / self.plan_scale,
        ) {
            Some(geometry::PlanHoverKind::Body) => CursorFeedback::Arrow,
            Some(geometry::PlanHoverKind::Node) => CursorFeedback::ArrowWithCheck,
            Some(geometry::PlanHoverKind::Edge) => CursorFeedback::ArrowWithTripleRay,
            None => CursorFeedback::Empty,
        }
    }
    fn open_polyline_edit(&mut self, id: u64, node: Option<usize>, edge: Option<usize>, at: Pos2) {
        let command = if node.is_some() {
            PolylineEditCommand::MoveNode
        } else {
            PolylineEditCommand::OffsetPolyline
        };
        self.polyline_edit = Some(PolylineEdit {
            element_id: id,
            node,
            edge,
            command,
            palette_at: at + vec2(14., 12.),
            drag: None,
            chamfer_length: 0.25,
            fillet_radius: 0.25,
            multiply_count: 2,
            error: String::new(),
        });
        self.editor.status = if node.is_some() {
            "Polyline node edit · drag the node or choose a palette command".into()
        } else {
            "Polyline edge edit · choose a palette command".into()
        };
    }
    #[cfg(test)]
    fn insert_polyline_node_at(&mut self, id: u64, edge_index: usize, point: Point, at: Pos2) {
        let before = self.editor.project.clone();
        let Some(element) = before.elements.iter().find(|element| element.id == id) else {
            return;
        };
        let Shape::Polyline { points, arcs } = &element.shape else {
            return;
        };
        if arcs.get(edge_index).is_some_and(Option::is_some) {
            self.open_polyline_edit(id, None, Some(edge_index), at);
            self.editor.status =
                "Curved edge selected · choose Curve edge to adjust its radius".into();
            return;
        }
        let Some((&a, &b)) = points.get(edge_index).zip(points.get(edge_index + 1)) else {
            return;
        };
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 1e-10 {
            self.editor.status = "Cannot insert a node into a zero-length edge".into();
            return;
        }
        let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared).clamp(0., 1.);
        let node = Point::new(a.x + t * dx, a.y + t * dy);
        if node.distance(a) < 0.01 || node.distance(b) < 0.01 {
            self.editor.status =
                "A new node must leave at least 0.01 m on both edge segments".into();
            return;
        }
        let node_index = edge_index + 1;
        let Some(slot) = self
            .editor
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == id)
        else {
            return;
        };
        let Shape::Polyline { points, arcs } = &mut slot.shape else {
            return;
        };
        points.insert(node_index, node);
        if !arcs.is_empty() {
            arcs.insert(edge_index + 1, None);
        }
        if self.editor.project.validate().is_err() {
            self.editor.project = before;
            self.editor.status = "Cannot insert a node into invalid Polyline geometry".into();
            return;
        }
        self.editor.commit(before);
        self.editor.set_selection(vec![id]);
        self.open_polyline_edit(id, Some(node_index), Some(edge_index), at);
        self.editor.status = "Polyline node inserted · drag it or choose a palette command".into();
    }
    fn apply_polyline_drag_preview(
        &mut self,
        before: &Project,
        edit: &PolylineEdit,
        start: Point,
        current: Point,
    ) {
        self.editor.project = before.clone();
        let Some(original) = before
            .elements
            .iter()
            .find(|element| element.id == edit.element_id)
        else {
            return;
        };
        let Shape::Polyline {
            points: source,
            arcs: source_arcs,
        } = &original.shape
        else {
            return;
        };
        let mut points = source.clone();
        let mut arcs = if source_arcs.is_empty() {
            vec![None; source.len().saturating_sub(1)]
        } else {
            source_arcs.clone()
        };
        let delta = Point::new(current.x - start.x, current.y - start.y);
        match edit.command {
            PolylineEditCommand::InsertNode => {
                if let Some(index) = edit.edge.filter(|index| *index + 1 < source.len()) {
                    let a = source[index];
                    let b = source[index + 1];
                    let (node, split) = if let Some(arc) = arcs[index] {
                        let Some((center, sweep)) = geometry::arc_center_sweep(a, b, arc) else {
                            return;
                        };
                        let angle = (current.y - center.y).atan2(current.x - center.x);
                        let start_angle = (a.y - center.y).atan2(a.x - center.x);
                        let traveled = if sweep > 0. {
                            (angle - start_angle).rem_euclid(std::f32::consts::TAU)
                        } else {
                            (start_angle - angle).rem_euclid(std::f32::consts::TAU)
                        };
                        if traveled >= sweep.abs() {
                            return;
                        }
                        let node = Point::new(
                            center.x + arc.radius * angle.cos(),
                            center.y + arc.radius * angle.sin(),
                        );
                        let mut first = arc;
                        first.large_arc = traveled > std::f32::consts::PI;
                        let mut second = arc;
                        second.large_arc = sweep.abs() - traveled > std::f32::consts::PI;
                        (node, (Some(first), Some(second)))
                    } else {
                        let dx = b.x - a.x;
                        let dy = b.y - a.y;
                        let t = (((current.x - a.x) * dx + (current.y - a.y) * dy)
                            / (dx * dx + dy * dy))
                            .clamp(0., 1.);
                        (Point::new(a.x + t * dx, a.y + t * dy), (None, None))
                    };
                    if node.distance(a) < 0.01 || node.distance(b) < 0.01 {
                        return;
                    }
                    points.insert(index + 1, node);
                    arcs[index] = split.0;
                    arcs.insert(index + 1, split.1);
                }
            }
            PolylineEditCommand::MoveNode => {
                if let Some(node) = edit.node.filter(|index| *index < points.len()) {
                    points[node] = current;
                    if source.len() > 2
                        && source
                            .first()
                            .zip(source.last())
                            .is_some_and(|(a, b)| a.distance(*b) < 0.01)
                    {
                        if node == 0 {
                            let last = points.len() - 1;
                            points[last] = current;
                        }
                        if node + 1 == points.len() {
                            points[0] = current;
                        }
                    }
                }
            }
            PolylineEditCommand::Move => {
                for point in &mut points {
                    point.x += delta.x;
                    point.y += delta.y;
                }
            }
            PolylineEditCommand::Rotate => {
                let pivot = edit
                    .node
                    .and_then(|index| source.get(index).copied())
                    .or_else(|| {
                        edit.edge.and_then(|index| {
                            source
                                .get(index)
                                .zip(source.get(index + 1))
                                .map(|(a, b)| Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5))
                        })
                    })
                    .unwrap_or(source[0]);
                let a0 = (start.y - pivot.y).atan2(start.x - pivot.x);
                let a1 = (current.y - pivot.y).atan2(current.x - pivot.x);
                let angle = a1 - a0;
                for point in &mut points {
                    let (x, y) = (point.x - pivot.x, point.y - pivot.y);
                    *point = Point::new(
                        pivot.x + x * angle.cos() - y * angle.sin(),
                        pivot.y + x * angle.sin() + y * angle.cos(),
                    );
                }
            }
            PolylineEditCommand::Mirror => {
                let axis = Point::new(current.x - start.x, current.y - start.y);
                let length = axis.x.hypot(axis.y);
                if length > 1e-5 {
                    let ux = axis.x / length;
                    let uy = axis.y / length;
                    for point in &mut points {
                        let x = point.x - start.x;
                        let y = point.y - start.y;
                        let projection = x * ux + y * uy;
                        *point = Point::new(
                            start.x + 2. * projection * ux - x,
                            start.y + 2. * projection * uy - y,
                        );
                    }
                    for arc in arcs.iter_mut().flatten() {
                        arc.clockwise = !arc.clockwise;
                    }
                }
            }
            PolylineEditCommand::OffsetPolyline => {
                let edge = Self::polyline_context_edge(edit, source);
                if let Some(index) = edge.filter(|index| *index + 1 < source.len()) {
                    let a = source[index];
                    let b = source[index + 1];
                    let length = a.distance(b);
                    if length > 1e-5 {
                        let normal = source_arcs
                            .get(index)
                            .and_then(|arc| *arc)
                            .and_then(|arc| {
                                let (center, _) = geometry::arc_center_sweep(a, b, arc)?;
                                let radial = Point::new(start.x - center.x, start.y - center.y);
                                let len = radial.x.hypot(radial.y);
                                (len > 1e-5).then(|| {
                                    let sign = if arc.clockwise { 1. } else { -1. };
                                    Point::new(sign * radial.x / len, sign * radial.y / len)
                                })
                            })
                            .unwrap_or(Point::new(-(b.y - a.y) / length, (b.x - a.x) / length));
                        let distance = delta.x * normal.x + delta.y * normal.y;
                        if let Some((offset, offset_arcs)) =
                            geometry::offset_polyline_with_arcs(source, source_arcs, distance)
                        {
                            points = offset;
                            arcs = offset_arcs;
                        } else if distance.abs() > 1e-6 {
                            if let Some(active) = self.polyline_edit.as_mut() {
                                active.error = "Offset cannot preserve a connected valid path at this distance".into();
                            }
                            return;
                        }
                    }
                }
            }
            PolylineEditCommand::CurveEdge => {
                if let Some(index) = Self::polyline_context_edge(edit, source)
                    .filter(|index| *index + 1 < source.len())
                {
                    if let Some(arc) =
                        geometry::arc_segment_through(source[index], source[index + 1], current)
                    {
                        arcs[index] = Some(arc);
                    }
                }
            }
            PolylineEditCommand::Multiply => {
                for index in 1..=edit.multiply_count.clamp(1, 50) {
                    let mut copy = original.clone();
                    copy.id = self.editor.project.next_id;
                    copy.name = format!("Polyline {:03}", copy.id);
                    self.editor.project.next_id += 1;
                    if let Shape::Polyline { points, .. } = &mut copy.shape {
                        for point in points {
                            point.x += delta.x * index as f32;
                            point.y += delta.y * index as f32;
                        }
                    }
                    self.editor.project.elements.push(copy);
                }
            }
            PolylineEditCommand::Chamfer | PolylineEditCommand::Fillet => return,
        }
        if let Some(slot) = self
            .editor
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == edit.element_id)
        {
            slot.shape = Shape::Polyline { points, arcs };
        }
    }
    fn polyline_context_edge(edit: &PolylineEdit, points: &[Point]) -> Option<usize> {
        edit.edge
            .filter(|index| *index + 1 < points.len())
            .or_else(|| {
                let node = edit.node?;
                if node + 1 < points.len() {
                    Some(node)
                } else {
                    node.checked_sub(1)
                        .filter(|index| *index + 1 < points.len())
                }
            })
    }
    fn apply_polyline_corner_edit(&mut self) {
        let Some(edit) = self.polyline_edit.as_mut() else {
            return;
        };
        let Some(node) = edit.node else {
            edit.error = "Choose a corner node first".into();
            return;
        };
        let before = self.editor.project.clone();
        let Some(element) = self
            .editor
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == edit.element_id)
        else {
            edit.error = "Polyline no longer exists".into();
            return;
        };
        let Shape::Polyline { points, arcs } = &mut element.shape else {
            edit.error = "Selected element is not a Polyline".into();
            return;
        };
        if node == 0 || node + 1 >= points.len() {
            edit.error = "Chamfer and Fillet need an interior node".into();
            return;
        }
        let previous = points[node - 1];
        let corner = points[node];
        let next = points[node + 1];
        let left = corner.distance(previous);
        let right = corner.distance(next);
        let operation = edit.command;
        let (ax, ay) = (previous.x - corner.x, previous.y - corner.y);
        let (bx, by) = (next.x - corner.x, next.y - corner.y);
        let la = ax.hypot(ay);
        let lb = bx.hypot(by);
        let angle = ((ax * bx + ay * by) / (la * lb).max(1e-8))
            .clamp(-1., 1.)
            .acos();
        if angle < 0.01 || (std::f32::consts::PI - angle).abs() < 0.01 {
            edit.error = "Corner edit needs two non-collinear segments".into();
            return;
        }
        let distance = if operation == PolylineEditCommand::Chamfer {
            edit.chamfer_length
        } else {
            edit.fillet_radius * (angle * 0.5).tan()
        };
        if !distance.is_finite()
            || distance <= 0.
            || distance >= left - 0.01
            || distance >= right - 0.01
        {
            edit.error = "Value must be positive and leave 10 mm on both adjacent segments".into();
            return;
        }
        let p1 = Point::new(corner.x + ax / la * distance, corner.y + ay / la * distance);
        let p2 = Point::new(corner.x + bx / lb * distance, corner.y + by / lb * distance);
        let fillet_arc = if operation == PolylineEditCommand::Chamfer {
            None
        } else {
            let center_distance = edit.fillet_radius / (angle * 0.5).sin();
            let bisector = Point::new(ax / la + bx / lb, ay / la + by / lb);
            let bisector_len = bisector.x.hypot(bisector.y);
            let center = Point::new(
                corner.x + bisector.x / bisector_len * center_distance,
                corner.y + bisector.y / bisector_len * center_distance,
            );
            let start_angle = (p1.y - center.y).atan2(p1.x - center.x);
            let end_angle = (p2.y - center.y).atan2(p2.x - center.x);
            let cross =
                (p1.x - center.x) * (p2.y - center.y) - (p1.y - center.y) * (p2.x - center.x);
            let mut sweep = end_angle - start_angle;
            if cross > 0. && sweep < 0. {
                sweep += std::f32::consts::TAU;
            }
            if cross < 0. && sweep > 0. {
                sweep -= std::f32::consts::TAU;
            }
            let middle_angle = start_angle + sweep * 0.5;
            let through = Point::new(
                center.x + edit.fillet_radius * middle_angle.cos(),
                center.y + edit.fillet_radius * middle_angle.sin(),
            );
            geometry::arc_segment_through(p1, p2, through)
        };
        if arcs.is_empty() {
            *arcs = vec![None; points.len() - 1];
        }
        points.splice(node..=node, [p1, p2]);
        let start = node - 1;
        arcs.splice(start..=node, [None, fillet_arc, None]);
        if let Err(error) = self.editor.project.validate() {
            self.editor.project = before;
            if let Some(edit) = self.polyline_edit.as_mut() {
                edit.error = error;
            }
            return;
        }
        self.editor.commit(before);
        self.editor.status = if operation == PolylineEditCommand::Chamfer {
            "Polyline corner chamfered"
        } else {
            "Polyline corner filleted"
        }
        .into();
        self.polyline_edit = None;
    }
    fn base_target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
        if self.editor.tool == Tool::Polyline {
            if let Some(anchor) = self.editor.polyline_points.last().copied() {
                if self.tracker_locked {
                    return (self.tracker_target(anchor), SnapKind::Free);
                }
                if !self.editor.project.guide_settings.snap_enabled {
                    let (base, kind) = geometry::snap(
                        &self.editor.project,
                        self.editor.story,
                        p,
                        9. / self.plan_scale,
                        self.editor.snap,
                    );
                    return self.apply_grid_snap(p, base, kind);
                }
                return geometry::polyline_snap_target(
                    &self.editor.project,
                    self.editor.story,
                    &self.editor.polyline_points,
                    p,
                    9. / self.plan_scale,
                    self.editor.snap,
                );
            }
        }
        let (mut q, kind) = geometry::snap(
            &self.editor.project,
            self.editor.story,
            p,
            9. / self.plan_scale,
            self.editor.snap,
        );
        if let Some(a) = self.editor.anchor {
            if self.tracker_locked {
                return (self.tracker_target(a), SnapKind::Free);
            }
            if self.editor.tool == Tool::Wall
                && self.editor.wall_settings.geometry.geometry_method
                    != WallGeometryMethod::Rectangular
                && (self.editor.orthogonal || shift)
            {
                if (q.x - a.x).abs() > (q.y - a.y).abs() {
                    q.y = a.y
                } else {
                    q.x = a.x
                }
            }
        }
        (q, kind)
    }
    fn tracker_target(&self, a: Point) -> Point {
        if matches!(self.editor.tool, Tool::Wall | Tool::Polyline) {
            if self.editor.tool == Tool::Wall
                && self.editor.wall_settings.geometry.geometry_method
                    == WallGeometryMethod::Rectangular
            {
                return Point::new(a.x + self.tracker_a, a.y + self.tracker_b);
            }
            let angle = self.tracker_b.to_radians();
            Point::new(
                a.x + self.tracker_a * angle.cos(),
                a.y + self.tracker_a * angle.sin(),
            )
        } else {
            Point::new(a.x + self.tracker_a, a.y + self.tracker_b)
        }
    }
    fn wall_preview_segments(&self, cursor: Point, close_tolerance: f32) -> Vec<(Point, Point)> {
        let method = self.editor.wall_settings.geometry.geometry_method;
        match method {
            WallGeometryMethod::Single => self
                .editor
                .anchor
                .map(|start| vec![(start, cursor)])
                .unwrap_or_default(),
            WallGeometryMethod::Rectangular => self
                .editor
                .anchor
                .map(|start| rectangle_wall_segments(start, cursor))
                .unwrap_or_default(),
            WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal => {
                let points = &self.editor.wall_points;
                let mut segments: Vec<_> =
                    points.windows(2).map(|pair| (pair[0], pair[1])).collect();
                let Some(last) = points.last().copied() else {
                    return segments;
                };
                if method == WallGeometryMethod::Polygonal
                    && points.len() >= 3
                    && points[0].distance(cursor) <= close_tolerance
                {
                    segments.push((last, points[0]));
                } else if last.distance(cursor) >= 0.01 {
                    segments.push((last, cursor));
                }
                segments
            }
        }
    }
    fn paint_wall_preview(&self, painter: &Painter, viewport: Rect, start: Point, end: Point) {
        if start.distance(end) < 0.01 {
            return;
        }
        let shape = Shape::Wall {
            start,
            end,
            thickness: self.editor.thickness,
            height: self.editor.height,
        };
        let tmp = Element {
            id: 0,
            name: String::new(),
            story: self.editor.story,
            material: self.editor.material,
            layer: self.editor.wall_layer,
            renovation: self.editor.wall_renovation,
            design_option: None,
            wall_settings: Some(self.editor.wall_settings.clone()),
            shape,
        };
        let points = geometry::footprint(&tmp)
            .into_iter()
            .map(|point| self.screen(viewport, point))
            .collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            Color32::from_rgba_unmultiplied(39, 146, 137, 50),
            Stroke::new(1.5, ACCENT),
        ));
        painter.line_segment(
            [self.screen(viewport, start), self.screen(viewport, end)],
            Stroke::new(1., ACCENT),
        );
    }
    fn commit_polyline_point(&mut self, point: Point, close_tolerance: f32) {
        let close_to_start = self.editor.polyline_points.len() >= 2
            && self
                .editor
                .polyline_points
                .first()
                .is_some_and(|start| start.distance(point) <= close_tolerance);
        if close_to_start {
            self.close_polyline();
        } else {
            self.editor.click(point, 6. / self.plan_scale);
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    fn close_polyline(&mut self) {
        if let Some(start) = self.editor.polyline_points.first().copied() {
            match self.editor.finish_polyline(Some(start)) {
                Ok(Some(_)) => self.preview_tool = None,
                Ok(None) => {}
                Err(error) => self.editor.status = error,
            }
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    fn finish_polyline(&mut self) {
        match self.editor.finish_polyline(None) {
            Ok(Some(_)) => self.preview_tool = None,
            Ok(None) => {}
            Err(error) => self.editor.status = error,
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    fn finish_wall_draft(&mut self) {
        // The click at the polygon start can already close and commit the
        // draft. A following double-click must not report a second, empty
        // completion or start a replacement wall.
        if !self.editor.wall_draft_active() {
            self.tracker_locked = false;
            self.draft_id = None;
            return;
        }
        match self.editor.finish_wall(None) {
            Ok(_) => self.preview_tool = None,
            Err(error) => self.editor.status = localized_error(self.language, &error),
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    fn plan(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        let trace_pointer = self.trace.pointer_mode_active();
        let trace_split = self.trace_split_regions(r);
        let hovered_before = ui.input(|i| i.pointer.hover_pos());
        let over_reference = trace_split
            .is_some_and(|(left, _)| hovered_before.is_some_and(|pos| left.contains(pos)));
        let interactive = !trace_pointer && !over_reference;
        let response = ui.allocate_rect(
            r,
            if interactive {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Floor plan canvas"));
        let background = self.editor.project.grid_settings.background;
        ui.painter_at(r).rect_filled(
            r,
            0.,
            Color32::from_rgba_unmultiplied(
                background[0],
                background[1],
                background[2],
                background[3],
            ),
        );
        let reference_clip = trace_split.map_or(r, |(left, _)| left);
        let active_clip = trace_split.map_or(r, |(_, right)| right);
        if !self.trace.on_top {
            let reference_painter = ui.painter_at(reference_clip);
            self.paint_trace_reference(&reference_painter, r, Some(reference_clip));
        }
        let mut p = ui.painter_at(active_clip);
        if self.trace.enabled {
            p.set_opacity(f32::from(self.trace.active_opacity) / 255.);
        }
        let hovered = response.hovered();
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if trace_pointer {
            self.trace_pointer_in_plan(ui, r);
        }
        if hovered {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0. {
                if let Some(pos) = pointer {
                    let before = self.world(r, pos);
                    self.plan_scale = (self.plan_scale * (scroll * 0.002).exp()).clamp(3., 250.);
                    let after = self.world(r, pos);
                    self.plan_center.x += before.x - after.x;
                    self.plan_center.y += before.y - after.y;
                }
            }
        }
        let pan = response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
            || (response.dragged_by(PointerButton::Primary)
                && ui.input(|i| i.key_down(Key::Space)));
        if pan {
            let d = ui.input(|i| i.pointer.delta());
            let radians = self
                .editor
                .project
                .plan_options
                .normalized_orientation()
                .to_radians();
            let (sin, cos) = radians.sin_cos();
            let view_x = d.x / self.plan_scale;
            let view_y = -d.y / self.plan_scale;
            self.plan_center.x -= view_x * cos + view_y * sin;
            self.plan_center.y -= -view_x * sin + view_y * cos;
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let grid = &self.editor.project.grid_settings;
        if grid.grid_visible_at(self.plan_scale) {
            let local = [
                self.world(r, r.left_top()),
                self.world(r, r.right_top()),
                self.world(r, r.left_bottom()),
                self.world(r, r.right_bottom()),
            ]
            .map(|point| grid.local_point(point));
            let min_u = local
                .iter()
                .map(|point| point.x)
                .fold(f32::INFINITY, f32::min);
            let max_u = local
                .iter()
                .map(|point| point.x)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_v = local
                .iter()
                .map(|point| point.y)
                .fold(f32::INFINITY, f32::min);
            let max_v = local
                .iter()
                .map(|point| point.y)
                .fold(f32::NEG_INFINITY, f32::max);
            for line in grid.lines_in_local(min_u, max_u, min_v, max_v) {
                let color = grid.line_color(line.main);
                p.line_segment(
                    [self.screen(r, line.start), self.screen(r, line.end)],
                    Stroke::new(
                        if line.main { 0.75 } else { 0.45 },
                        Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
                    ),
                );
            }
        }
        // Slabs beneath walls, independently of creation order.
        let mut elements: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.editor.story && self.editor.project.plan_options.element_visible(e)
            })
            .collect();
        elements.sort_by_key(|e| matches!(e.shape, Shape::Wall { .. }));
        let selected_ids = self.editor.selection_ids();
        for e in elements {
            let selected = selected_ids.contains(&e.id);
            if matches!(e.shape, Shape::Wall { .. }) {
                continue;
            }
            let Some(style) = self.plan_style(e) else {
                continue;
            };
            if let Shape::Polyline {
                ref points,
                ref arcs,
            } = e.shape
            {
                let preview_arc = self
                    .polyline_edit
                    .as_ref()
                    .filter(|edit| {
                        edit.element_id == e.id
                            && edit.command == PolylineEditCommand::CurveEdge
                            && edit.drag.is_some()
                    })
                    .and_then(|edit| {
                        let index = Self::polyline_context_edge(edit, points)?;
                        let current = self.world(r, pointer?);
                        Some((
                            index,
                            geometry::arc_segment_through(
                                points[index],
                                points[index + 1],
                                current,
                            ),
                        ))
                    });
                let mut live_arcs = arcs.clone();
                if let Some((index, arc)) = preview_arc {
                    live_arcs.resize(points.len() - 1, None);
                    live_arcs[index] = arc;
                }
                let arcs = &live_arcs;
                let tessellated = geometry::polyline_path(points, arcs);
                let path: Vec<Pos2> = tessellated
                    .iter()
                    .map(|point| self.screen(r, *point))
                    .collect();
                let color = if selected {
                    Color32::from_rgb(0, 220, 70)
                } else {
                    Self::plan_color(style.line)
                };
                p.add(egui::Shape::line(
                    path,
                    Stroke::new(
                        if selected {
                            2.5
                        } else {
                            self.plan_pen_px(style.line_width_mm)
                        },
                        color,
                    ),
                ));
                if let Some(edit) = self.polyline_edit.as_ref().filter(|edit| {
                    selected
                        && edit.element_id == e.id
                        && edit.command == PolylineEditCommand::CurveEdge
                        && edit.drag.is_some()
                }) {
                    let radius = edit
                        .edge
                        .and_then(|index| arcs.get(index).and_then(|arc| *arc))
                        .map(|arc| arc.radius);
                    if let (Some(radius), Some(pointer)) = (radius, pointer) {
                        p.text(
                            pointer + vec2(14., -18.),
                            Align2::LEFT_BOTTOM,
                            format!("R = {:.3} m", radius),
                            FontId::monospace(11.),
                            INK,
                        );
                    }
                }
                if selected {
                    for point in points {
                        let node = self.screen(r, *point);
                        p.circle_filled(node, 3.5, Color32::BLACK);
                    }
                }
                continue;
            }
            let pts: Vec<_> = geometry::footprint(e)
                .into_iter()
                .map(|v| self.screen(r, v))
                .collect();
            let (fill, stroke) = if selected {
                (Color32::from_rgb(195, 225, 218), Stroke::new(2., ACCENT))
            } else {
                let width = self.plan_pen_px(style.line_width_mm);
                (
                    style.fill.map_or(Color32::TRANSPARENT, Self::plan_color),
                    Stroke::new(width, Self::plan_color(style.line)),
                )
            };
            p.add(egui::Shape::convex_polygon(
                pts.clone(),
                self.trace_active_fill(fill),
                stroke,
            ));
            if style.draw_treads {
                if let Shape::Stair { ref spec } = e.shape {
                    if let Ok(stair) = crate::stairs::generate(spec, 0.0) {
                        for [a, b] in stair.tread_lines {
                            p.line_segment(
                                [self.screen(r, a), self.screen(r, b)],
                                Stroke::new(
                                    self.plan_pen_px(style.line_width_mm) * 0.8,
                                    Self::plan_color(style.line),
                                ),
                            );
                        }
                    }
                }
            }
        }
        let walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.editor.story
                    && matches!(e.shape, Shape::Wall { .. })
                    && self.editor.project.plan_options.element_visible(e)
            })
            .collect();
        let mut joined_wall_footprints = Vec::new();
        for material in Material::ALL {
            for renovation in [
                RenovationStatus::Existing,
                RenovationStatus::Demolition,
                RenovationStatus::New,
            ] {
                let same_material: Vec<_> = walls
                    .iter()
                    .copied()
                    .filter(|wall| wall.material == material && wall.renovation == renovation)
                    .collect();
                if same_material.is_empty() {
                    continue;
                }
                let Some(style) = self.plan_style(same_material[0]) else {
                    continue;
                };
                let footprints = geometry::plan_wall_footprints(&same_material);
                for (wall, footprint) in same_material.iter().zip(&footprints) {
                    joined_wall_footprints.push((wall.id, *footprint));
                    if !selected_ids.contains(&wall.id) {
                        let pts: Vec<_> = footprint
                            .iter()
                            .map(|point| self.screen(r, *point))
                            .collect();
                        let wall_fill = self.trace_active_fill(
                            style
                                .fill
                                .map_or(Self::plan_color(style.line), Self::plan_color),
                        );
                        p.add(egui::Shape::convex_polygon(
                            pts,
                            wall_fill,
                            // Slight same-colour bleed seals sub-pixel gaps where
                            // separately editable wall footprints meet.
                            Stroke::new(0.8, wall_fill),
                        ));
                    }
                }
                let wall_stroke = Stroke::new(
                    self.plan_pen_px(style.line_width_mm),
                    Self::plan_color(style.line),
                );
                for mut path in geometry::wall_outline_paths(&footprints) {
                    let closed = path.len() > 2 && path[0].distance(*path.last().unwrap()) <= 1e-3;
                    if closed {
                        path.pop();
                    }
                    let points: Vec<_> = path
                        .into_iter()
                        .map(|point| self.screen(r, point))
                        .collect();
                    if closed {
                        p.add(egui::Shape::closed_line(points, wall_stroke));
                    } else {
                        p.add(egui::Shape::line(points, wall_stroke));
                    }
                }
            }
        }
        // Selected walls retain an individual highlight and editable handles.
        let selected_walls: Vec<_> = walls
            .iter()
            .copied()
            .filter(|wall| selected_ids.contains(&wall.id))
            .collect();
        for wall in &selected_walls {
            let joined = joined_wall_footprints
                .iter()
                .find(|(id, _)| *id == wall.id)
                .map(|(_, footprint)| *footprint)
                .unwrap_or_else(|| geometry::footprint(wall));
            let pts: Vec<_> = joined.into_iter().map(|v| self.screen(r, v)).collect();
            p.add(egui::Shape::convex_polygon(
                pts,
                WALL_SELECTION_FILL,
                Stroke::new(1.5, WALL_SELECTION_OUTLINE),
            ));
        }
        // Paint every baseline after the fills so adjacently selected walls
        // cannot hide each other's reference lines at a junction.
        for wall in selected_walls {
            if let Some((start, end)) = geometry::wall_reference_line(wall) {
                let a = self.screen(r, start);
                let b = self.screen(r, end);
                paint_selected_wall_baseline(&p, a, b);
                let mid = a.lerp(b, 0.5);
                p.text(
                    mid + vec2(0., -14.),
                    Align2::CENTER_CENTER,
                    self.editor
                        .project
                        .plan_options
                        .dimension_style
                        .format_length(start.distance(end)),
                    FontId::monospace(11.),
                    WALL_SELECTION_OUTLINE,
                );
            }
        }
        p.set_opacity(1.);
        if self.trace.on_top {
            let reference_painter = ui.painter_at(reference_clip);
            self.paint_trace_reference(&reference_painter, r, Some(reference_clip));
        }
        if let Some((left, right)) = trace_split {
            let x = left.right();
            ui.painter().line_segment(
                [pos2(x, r.top()), pos2(x, r.bottom())],
                Stroke::new(1.5, LINE),
            );
            let handle = Rect::from_min_max(pos2(x - 3., r.top()), pos2(x + 3., r.bottom()));
            let split_response = ui.interact(handle, Id::new("trace_splitter"), Sense::drag());
            if split_response.dragged() {
                let delta = ui.input(|i| i.pointer.delta().x) / r.width();
                let ratio = (x - r.left()) / r.width() + delta;
                self.trace.splitter = Some(ratio.clamp(0.1, 0.9));
            }
            split_response.on_hover_cursor(CursorIcon::ResizeHorizontal);
            ui.painter().text(
                left.left_top() + vec2(10., 8.),
                Align2::LEFT_TOP,
                self.language.translate("Reference"),
                FontId::proportional(10.),
                MUTED,
            );
            ui.painter().text(
                right.right_top() + vec2(-10., 8.),
                Align2::RIGHT_TOP,
                self.language.translate("Active"),
                FontId::proportional(10.),
                MUTED,
            );
        }
        // Associative overall dimensions, computed from the current story.
        let walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.editor.story
                    && matches!(e.shape, Shape::Wall { .. })
                    && self.editor.project.plan_options.element_visible(e)
            })
            .flat_map(geometry::footprint)
            .collect();
        if !walls.is_empty() {
            let min = walls
                .iter()
                .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, b| {
                    Point::new(a.x.min(b.x), a.y.min(b.y))
                });
            let max = walls
                .iter()
                .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| {
                    Point::new(a.x.max(b.x), a.y.max(b.y))
                });
            let a = self.screen(r, Point::new(min.x, max.y));
            let b = self.screen(r, max);
            let y = a.y - 28.;
            p.line_segment([pos2(a.x, y), pos2(b.x, y)], Stroke::new(0.8, MUTED));
            for x in [a.x, b.x] {
                p.line_segment(
                    [pos2(x, a.y - 6.), pos2(x, y - 7.)],
                    Stroke::new(0.8, MUTED),
                );
                p.line_segment(
                    [pos2(x - 3., y + 3.), pos2(x + 3., y - 3.)],
                    Stroke::new(1., INK),
                );
            }
            p.text(
                pos2((a.x + b.x) / 2., y - 9.),
                Align2::CENTER_BOTTOM,
                self.editor
                    .project
                    .plan_options
                    .dimension_style
                    .format_length(max.x - min.x),
                FontId::monospace(11.),
                INK,
            );
        }
        if let Some(area) = &self.editor.marquee {
            let pts: Vec<Pos2> = area.vertices.iter().map(|v| self.screen(r, *v)).collect();
            for i in 0..pts.len() {
                let a = pts[i];
                let b = pts[(i + 1) % pts.len()];
                let length = a.distance(b).max(1.);
                let direction = (b - a) / length;
                let mut offset = 0.;
                while offset < length {
                    let end = (offset + 7.).min(length);
                    p.line_segment(
                        [a + direction * offset, a + direction * end],
                        Stroke::new(1.5, ACCENT),
                    );
                    offset += 12.;
                }
            }
        }
        if let Some((a, b)) = self.marquee_preview {
            let pts: Vec<_> = geometry::rectangle_polygon(a, b)
                .iter()
                .map(|v| self.screen(r, *v))
                .collect();
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.2, ACCENT)));
        }
        if !self.editor.polyline_points.is_empty() {
            let mut path: Vec<Pos2> = self
                .editor
                .polyline_points
                .iter()
                .map(|point| self.screen(r, *point))
                .collect();
            if let Some(pos) = pointer.filter(|point| r.contains(*point)) {
                let (preview_point, _) =
                    self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                path.push(self.screen(r, preview_point));
            }
            if path.len() >= 2 {
                p.add(egui::Shape::line(path, Stroke::new(1.5, ACCENT)));
            }
            for point in &self.editor.polyline_points {
                let node = self.screen(r, *point);
                p.circle_filled(node, 3.5, Color32::WHITE);
                p.circle_stroke(node, 1.2, Stroke::new(1.2, ACCENT));
            }
        }
        if !self.marquee_points.is_empty() {
            let mut preview = self.marquee_points.clone();
            if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
                let (q, _) = self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                if self.marquee_geometry == MarqueeGeometry::Polygon
                    || self.marquee_geometry == MarqueeGeometry::RotatedRectangle
                {
                    if self.marquee_geometry == MarqueeGeometry::Polygon || preview.len() < 2 {
                        preview.push(q);
                    } else if let Some(poly) =
                        geometry::rotated_rectangle_polygon(preview[0], preview[1], q)
                    {
                        preview = poly;
                    }
                }
            }
            let pts: Vec<_> = preview.iter().map(|v| self.screen(r, *v)).collect();
            if pts.len() >= 2 {
                p.add(egui::Shape::line(pts, Stroke::new(1.3, ACCENT)));
            }
            for v in self.marquee_points.iter() {
                p.circle_filled(self.screen(r, *v), 3.5, Color32::WHITE);
                p.circle_stroke(self.screen(r, *v), 3.5, Stroke::new(1., ACCENT));
            }
        }
        if self.editor.project.grid_settings.origin_display {
            let origin = self.screen(r, Point::default());
            p.line_segment(
                [origin - vec2(8., 0.), origin + vec2(8., 0.)],
                Stroke::new(1., ACCENT),
            );
            p.line_segment(
                [origin - vec2(0., 8.), origin + vec2(0., 8.)],
                Stroke::new(1., ACCENT),
            );
            p.text(
                origin + vec2(10., 10.),
                Align2::LEFT_TOP,
                "0, 0",
                FontId::monospace(9.),
                ACCENT,
            );
        }
        if let Some(tool) = &self.rotated_grid_tool {
            if let Some(start) = tool.start {
                if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
                    let raw = self.world(r, pos);
                    let (end, _) = self.target(raw, ui.input(|i| i.modifiers.shift));
                    let a = self.screen(r, start);
                    let b = self.screen(r, end);
                    p.line_segment([a, b], Stroke::new(1.5, ACCENT));
                    if let Some(angle) = crate::grids::GridSettings::angle_from_vector(start, end) {
                        p.text(
                            b + vec2(12., -24.),
                            Align2::LEFT_BOTTOM,
                            format!("{angle:.2}°"),
                            FontId::monospace(10.),
                            ACCENT,
                        );
                    }
                }
            }
        }
        if self.polyline_edit.is_none() && self.guide_canvas(ui, r, &response) {
            return;
        }
        if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
            let raw = self.world(r, pos);
            let (q, kind) = self.target(raw, ui.input(|i| i.modifiers.shift));
            self.hover_point = q;
            if self.orientation_pick_active {
                if let Some(start) = self.orientation_pick_start {
                    let a = self.screen(r, start);
                    let b = self.screen(r, q);
                    p.line_segment([a, b], Stroke::new(1.6, ACCENT));
                    let angle = (q.y - start.y).atan2(q.x - start.x).to_degrees();
                    p.text(
                        b + vec2(12., -22.),
                        Align2::LEFT_BOTTOM,
                        format!("{angle:.2}°"),
                        FontId::monospace(10.),
                        ACCENT,
                    );
                }
            }
            if !matches!(self.editor.tool, Tool::Select | Tool::Marquee) {
                let s = self.screen(r, q);
                if self.editor.snap {
                    p.rect_stroke(
                        Rect::from_center_size(s, vec2(9., 9.)),
                        0.,
                        Stroke::new(1.5, ACCENT),
                        StrokeKind::Middle,
                    );
                    p.text(
                        s + vec2(12., 12.),
                        Align2::LEFT_TOP,
                        format!("{kind:?}"),
                        FontId::proportional(10.),
                        ACCENT,
                    );
                }
                if self.editor.tool == Tool::Wall {
                    if let Some(a) = self.editor.anchor {
                        let rectangular = self.editor.wall_settings.geometry.geometry_method
                            == WallGeometryMethod::Rectangular;
                        if !self.tracker_locked {
                            if rectangular {
                                self.tracker_a = q.x - a.x;
                                self.tracker_b = q.y - a.y;
                            } else {
                                self.tracker_a = a.distance(q);
                                self.tracker_b = (q.y - a.y).atan2(q.x - a.x).to_degrees();
                            }
                        }
                        for (start, end) in self.wall_preview_segments(q, 9. / self.plan_scale) {
                            self.paint_wall_preview(&p, r, start, end);
                        }
                        let label = if rectangular {
                            format!(
                                "{} × {}",
                                self.editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length((q.x - a.x).abs()),
                                self.editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length((q.y - a.y).abs())
                            )
                        } else {
                            format!(
                                "{}  /  {:.1}°",
                                self.editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length(a.distance(q)),
                                (q.y - a.y).atan2(q.x - a.x).to_degrees()
                            )
                        };
                        let boxr = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                        p.rect_filled(boxr, 4., INK);
                        p.text(
                            boxr.center(),
                            Align2::CENTER_CENTER,
                            label,
                            FontId::monospace(11.),
                            Color32::WHITE,
                        );
                    }
                } else if let Some(a) = self.editor.anchor {
                    if !self.tracker_locked {
                        self.tracker_a = q.x - a.x;
                        self.tracker_b = q.y - a.y;
                    }
                    let shape = match self.editor.tool {
                        Tool::Wall => unreachable!("walls use the wall preview"),
                        Tool::Slab => Shape::Slab {
                            min: Point::new(a.x.min(q.x), a.y.min(q.y)),
                            max: Point::new(a.x.max(q.x), a.y.max(q.y)),
                            thickness: self.editor.thickness,
                        },
                        Tool::Stair => {
                            let mut spec = self.editor.stair_spec.clone();
                            spec.start = a;
                            spec.end = q;
                            Shape::Stair { spec }
                        }
                        Tool::Polyline => Shape::Polyline {
                            points: vec![a, q],
                            arcs: vec![],
                        },
                        _ => Shape::Slab {
                            min: Point::new(a.x.min(q.x), a.y.min(q.y)),
                            max: Point::new(a.x.max(q.x), a.y.max(q.y)),
                            thickness: self.editor.thickness,
                        },
                    };
                    let tmp = Element {
                        id: 0,
                        name: String::new(),
                        story: self.editor.story,
                        material: self.editor.material,
                        layer: match shape {
                            Shape::Slab { .. } => Layer::Structural,
                            Shape::Polyline { .. } => Layer::Drafting,
                            Shape::Wall { .. } => self.editor.wall_layer,
                            _ => Layer::Architectural,
                        },
                        renovation: self.editor.wall_renovation,
                        design_option: None,
                        wall_settings: if matches!(shape, Shape::Wall { .. }) {
                            Some(self.editor.wall_settings.clone())
                        } else {
                            None
                        },
                        shape,
                    };
                    let pts = geometry::footprint(&tmp)
                        .into_iter()
                        .map(|v| self.screen(r, v))
                        .collect();
                    p.add(egui::Shape::convex_polygon(
                        pts,
                        Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                        Stroke::new(1.5, ACCENT),
                    ));
                    if let Shape::Stair { ref spec } = tmp.shape {
                        if let Ok(stair) = crate::stairs::generate(spec, 0.0) {
                            for [ta, tb] in stair.tread_lines {
                                p.line_segment(
                                    [self.screen(r, ta), self.screen(r, tb)],
                                    Stroke::new(0.9, ACCENT),
                                );
                            }
                        }
                    }
                    p.line_segment([self.screen(r, a), s], Stroke::new(1., ACCENT));
                    let label = format!(
                        "{}  /  {:.1}°",
                        self.editor
                            .project
                            .plan_options
                            .dimension_style
                            .format_length(a.distance(q)),
                        (q.y - a.y).atan2(q.x - a.x).to_degrees()
                    );
                    let boxr = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                    p.rect_filled(boxr, 4., INK);
                    p.text(
                        boxr.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::monospace(11.),
                        Color32::WHITE,
                    );
                } else if self.editor.tool == Tool::Polyline {
                    if let Some(a) = self.editor.polyline_points.last().copied() {
                        if !self.tracker_locked {
                            self.tracker_a = a.distance(q);
                            self.tracker_b = (q.y - a.y).atan2(q.x - a.x).to_degrees();
                        }
                        let label = format!(
                            "{}  /  {:.1}°",
                            self.editor
                                .project
                                .plan_options
                                .dimension_style
                                .format_length(a.distance(q)),
                            (q.y - a.y).atan2(q.x - a.x).to_degrees()
                        );
                        let label_rect = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                        p.rect_filled(label_rect, 4., INK);
                        p.text(
                            label_rect.center(),
                            Align2::CENTER_CENTER,
                            label,
                            FontId::monospace(11.),
                            Color32::WHITE,
                        );
                    }
                }
            }
            let wall_double = self.editor.tool == Tool::Wall
                && (response.double_clicked_by(PointerButton::Primary)
                    || response.triple_clicked_by(PointerButton::Primary))
                && matches!(
                    self.editor.wall_settings.geometry.geometry_method,
                    WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal
                );
            if response.clicked() && !wall_double && !ui.input(|i| i.key_down(Key::Space)) {
                if self.orientation_pick_active {
                    if let Some(start) = self.orientation_pick_start.take() {
                        let dx = q.x - start.x;
                        let dy = q.y - start.y;
                        if dx.hypot(dy) > 1e-4 {
                            self.orientation_pick_active = false;
                            self.set_orientation_degrees(dy.atan2(dx).to_degrees());
                        } else {
                            self.orientation_pick_start = Some(start);
                            self.editor.status = "Orientation needs two distinct points".into();
                        }
                    } else {
                        self.orientation_pick_start = Some(q);
                        self.editor.status = "Set Orientation · click the second point".into();
                    }
                } else if self.rotated_grid_tool.is_some() {
                    self.handle_rotated_grid_click(raw, q, 9. / self.plan_scale);
                } else if self.editor.tool == Tool::Marquee {
                    match self.marquee_geometry {
                        MarqueeGeometry::Rectangle => {
                            if let Some(a) = self.marquee_points.first().copied() {
                                self.place_marquee(geometry::rectangle_polygon(a, q));
                            } else {
                                self.marquee_points.push(q);
                            }
                        }
                        MarqueeGeometry::RotatedRectangle => {
                            self.marquee_points.push(q);
                            if self.marquee_points.len() == 3 {
                                if let Some(poly) = geometry::rotated_rectangle_polygon(
                                    self.marquee_points[0],
                                    self.marquee_points[1],
                                    self.marquee_points[2],
                                ) {
                                    self.place_marquee(poly);
                                } else {
                                    self.marquee_points.clear();
                                    self.editor.status =
                                        "Rotated rectangle needs non-zero sides".into();
                                }
                            }
                        }
                        MarqueeGeometry::Polygon => self.marquee_points.push(q),
                    }
                } else if self.editor.tool == Tool::Select {
                    let context = self.selected_polyline_hit(raw, 9. / self.plan_scale);
                    if let Some((id, node, edge)) = context {
                        self.drag = None;
                        self.open_polyline_edit(id, node, edge, pos);
                    } else if ui.input(|i| i.modifiers.shift) {
                        self.polyline_edit = None;
                        let hit = geometry::hit_plan(
                            &self.editor.project,
                            self.editor.story,
                            raw,
                            6. / self.plan_scale,
                        );
                        let mut ids = self.editor.selection_ids();
                        if let Some(id) = hit {
                            if ids.contains(&id) {
                                ids.retain(|v| *v != id);
                            } else {
                                ids.push(id);
                            }
                        }
                        self.editor.set_selection(ids);
                    } else {
                        self.polyline_edit = None;
                        let hit = geometry::hit_plan(
                            &self.editor.project,
                            self.editor.story,
                            raw,
                            6. / self.plan_scale,
                        );
                        self.editor.set_selection(hit.into_iter().collect());
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else if self.editor.tool == Tool::Polyline {
                    let close_start = self.editor.polyline_points.len() >= 2
                        && self
                            .editor
                            .polyline_points
                            .first()
                            .is_some_and(|start| self.screen(r, *start).distance(pos) <= 9.);
                    if close_start {
                        self.close_polyline();
                    } else {
                        self.commit_polyline_point(q, 0.01);
                    }
                } else if self.editor.tool == Tool::Wall {
                    let defaults_ok = !self
                        .info_box_wall_tool
                        .as_ref()
                        .is_some_and(|state| state.dirty)
                        || self.commit_info_box_wall_tool();
                    if defaults_ok {
                        self.editor.click(q, 6. / self.plan_scale);
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else {
                    self.editor.click(q, 6. / self.plan_scale);
                    self.tracker_locked = false;
                    self.draft_id = None;
                }
            }
            if wall_double && !ui.input(|i| i.key_down(Key::Space)) {
                let defaults_ok = !self
                    .info_box_wall_tool
                    .as_ref()
                    .is_some_and(|state| state.dirty)
                    || self.commit_info_box_wall_tool();
                if defaults_ok {
                    self.finish_wall_draft();
                }
                self.tracker_locked = false;
                self.draft_id = None;
            }
        }
        if self.editor.tool == Tool::Polyline && response.clicked_by(PointerButton::Secondary) {
            self.finish_polyline();
        }
        if self.editor.tool == Tool::Marquee
            && self.marquee_geometry == MarqueeGeometry::Polygon
            && response.clicked_by(PointerButton::Secondary)
        {
            if self.marquee_points.len() >= 3 {
                self.place_marquee(self.marquee_points.clone());
            } else {
                self.marquee_points.clear();
                self.editor.status = "Marquee polygon needs at least three points".into();
            }
        }
        if self.editor.tool == Tool::Marquee && self.marquee_geometry == MarqueeGeometry::Rectangle
        {
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                    let a = self.world(r, pos);
                    self.marquee_preview = Some((a, a));
                    self.marquee_points.clear();
                }
            }
            if response.dragged_by(PointerButton::Primary) {
                if let (Some((a, _)), Some(pos)) = (self.marquee_preview, pointer) {
                    self.marquee_preview = Some((a, self.world(r, pos)));
                }
            }
            if response.drag_stopped() {
                if let Some((a, b)) = self.marquee_preview.take() {
                    if a.distance(b) > 2. / self.plan_scale {
                        self.place_marquee(geometry::rectangle_polygon(a, b));
                    }
                }
            }
        }
        if self.editor.tool == Tool::Select && !pan && self.polyline_edit.is_some() {
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                    let start = self.world(r, pos);
                    let before = self.editor.project.clone();
                    if let Some(edit) = self.polyline_edit.as_mut() {
                        edit.drag = Some((before, start));
                    }
                }
            }
            if response.dragged_by(PointerButton::Primary) {
                if let (Some(pos), Some(edit)) = (pointer, self.polyline_edit.clone()) {
                    if let Some((before, start)) = edit.drag.as_ref() {
                        let current = self.world(r, pos);
                        self.apply_polyline_drag_preview(before, &edit, *start, current);
                        ui.ctx().request_repaint();
                    }
                }
            }
            if response.drag_stopped() {
                if let Some(edit) = self.polyline_edit.as_mut() {
                    if let Some((before, _)) = edit.drag.take() {
                        if self.editor.project.validate().is_ok() && self.editor.project != before {
                            if edit.command == PolylineEditCommand::Multiply {
                                let new_ids: Vec<u64> = self
                                    .editor
                                    .project
                                    .elements
                                    .iter()
                                    .filter(|element| element.id >= before.next_id)
                                    .map(|element| element.id)
                                    .collect();
                                self.editor.commit(before);
                                self.editor.set_selection(new_ids.clone());
                                self.editor.status =
                                    format!("Created {} Polyline copies", new_ids.len());
                            } else {
                                self.editor.commit(before);
                                self.editor.status = match edit.command {
                                    PolylineEditCommand::InsertNode => "Polyline node inserted",
                                    PolylineEditCommand::MoveNode => "Polyline node moved",
                                    PolylineEditCommand::OffsetPolyline => "Polyline offset",
                                    PolylineEditCommand::CurveEdge => "Polyline edge curved",
                                    PolylineEditCommand::Move => "Polyline moved",
                                    PolylineEditCommand::Rotate => "Polyline rotated",
                                    PolylineEditCommand::Mirror => "Polyline mirrored",
                                    _ => "Polyline edited",
                                }
                                .into();
                            }
                            self.polyline_edit = None;
                        } else {
                            self.editor.project = before;
                            edit.error = "Edit produced no valid geometry change".into();
                        }
                    }
                }
            }
        } else if self.editor.tool == Tool::Select && !pan {
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                    let w = self.world(r, pos);
                    let id = self
                        .selected_polyline_hit(w, 9. / self.plan_scale)
                        .map(|hit| hit.0)
                        .or_else(|| {
                            geometry::hit_plan(
                                &self.editor.project,
                                self.editor.story,
                                w,
                                6. / self.plan_scale,
                            )
                        });
                    if let Some(id) = id {
                        let mut ids = self.editor.selection_ids();
                        if !ids.contains(&id) {
                            ids = vec![id];
                            self.editor.set_selection(ids.clone());
                        }
                        self.drag = Some((self.editor.project.clone(), w, ids));
                    }
                }
            }
            if response.dragged_by(PointerButton::Primary) {
                if let (Some((before, start, ids)), Some(pos)) = (&self.drag, pointer) {
                    let w = self.world(r, pos);
                    let mut delta = Point::new(w.x - start.x, w.y - start.y);
                    if self.editor.snap {
                        delta.x = (delta.x * 10.).round() / 10.;
                        delta.y = (delta.y * 10.).round() / 10.;
                    }
                    for original in before.elements.iter().filter(|e| ids.contains(&e.id)) {
                        let mut e = original.clone();
                        e.translate(delta);
                        if let Some(slot) = self
                            .editor
                            .project
                            .elements
                            .iter_mut()
                            .find(|slot| slot.id == e.id)
                        {
                            *slot = e;
                        }
                    }
                    self.draft_id = None;
                }
            }
        }
        if response.drag_stopped() {
            if let Some((before, _, _)) = self.drag.take() {
                self.editor.commit(before);
                self.editor.status = "Element moved".into();
            }
        }
        if hovered {
            let feedback = if self.editor.tool == Tool::Polyline {
                if ui.input(|i| i.pointer.button_down(PointerButton::Primary)) {
                    CursorFeedback::SolidPencil
                } else if self.editor.polyline_points.is_empty() {
                    CursorFeedback::Drawing
                } else {
                    let (_, kind) = self.target(
                        self.world(r, pointer.unwrap_or(r.center())),
                        ui.input(|i| i.modifiers.shift),
                    );
                    if matches!(
                        kind,
                        SnapKind::Endpoint
                            | SnapKind::Midpoint
                            | SnapKind::Angle
                            | SnapKind::Perpendicular
                            | SnapKind::Alignment
                    ) {
                        CursorFeedback::HollowPencilWithLine
                    } else {
                        CursorFeedback::HollowPencil
                    }
                }
            } else if pan {
                CursorFeedback::Dragging
            } else if self
                .polyline_edit
                .as_ref()
                .is_some_and(|edit| edit.command == PolylineEditCommand::MoveNode)
            {
                self.select_cursor_feedback(self.world(r, pointer.unwrap_or(r.center())))
            } else if self.drag.as_ref().is_some_and(|(_, _, ids)| {
                ids.iter().any(|id| {
                    self.editor.project.elements.iter().any(|element| {
                        element.id == *id && matches!(element.shape, Shape::Polyline { .. })
                    })
                })
            }) {
                CursorFeedback::Arrow
            } else if self.drag.is_some()
                || self
                    .polyline_edit
                    .as_ref()
                    .is_some_and(|edit| edit.drag.is_some())
            {
                CursorFeedback::Dragging
            } else {
                let pos = pointer.unwrap_or(r.center());
                match self.editor.tool {
                    Tool::Select => {
                        let raw = self.world(r, pos);
                        self.select_cursor_feedback(raw)
                    }
                    Tool::Marquee => CursorFeedback::Drawing,
                    Tool::Wall | Tool::Slab | Tool::Stair | Tool::Polyline => {
                        let (_, snap_kind) =
                            self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                        if matches!(
                            snap_kind,
                            SnapKind::Endpoint
                                | SnapKind::Midpoint
                                | SnapKind::SnapPoint
                                | SnapKind::Guide
                        ) {
                            CursorFeedback::SnapTarget
                        } else {
                            CursorFeedback::Drawing
                        }
                    }
                }
            };
            ui.ctx()
                .set_cursor_icon(if self.rotated_grid_tool.is_some() {
                    CursorIcon::Crosshair
                } else {
                    feedback.icon()
                });
            feedback.paint(&p, pointer.unwrap_or(r.center()));
        }
        p.text(
            r.left_top() + vec2(18., 18.),
            Align2::LEFT_TOP,
            format!(
                "{:02}  /  {}",
                self.editor.story,
                self.editor
                    .project
                    .stories
                    .iter()
                    .find(|s| s.id == self.editor.story)
                    .map_or("Floor plan", |s| s.name.as_str())
            ),
            FontId::proportional(12.),
            MUTED,
        );
        let b = r.left_bottom() + vec2(20., -35.);
        let len = self.plan_scale * 2.;
        p.line_segment([b, b + vec2(len, 0.)], Stroke::new(2., INK));
        for v in [b, b + vec2(len, 0.)] {
            p.line_segment([v - vec2(0., 4.), v + vec2(0., 4.)], Stroke::new(1., INK));
        }
        p.text(
            b + vec2(len / 2., 8.),
            Align2::CENTER_TOP,
            "2 m",
            FontId::monospace(10.),
            MUTED,
        );
        p.text(
            r.right_top() + vec2(-20., 24.),
            Align2::RIGHT_TOP,
            "N",
            FontId::proportional(12.),
            INK,
        );
        if let Some(edit) = self.polyline_edit.as_ref() {
            let active = edit
                .node
                .and_then(|_| {
                    self.editor
                        .project
                        .elements
                        .iter()
                        .find(|element| element.id == edit.element_id)
                })
                .and_then(|element| match &element.shape {
                    Shape::Polyline { points, .. } => points.get(edit.node?).copied(),
                    _ => None,
                });
            if let Some(point) = active {
                p.circle_filled(self.screen(r, point), 5., ACCENT);
            }
        }
        if trace_pointer {
            self.trace_cursor(ui);
        }
        self.polyline_edit_palette(ui.ctx());
    }
    fn polyline_edit_palette(&mut self, ctx: &Context) {
        let language = self.language;
        let Some(edit) = self.polyline_edit.as_mut() else {
            return;
        };
        if edit.drag.is_some() {
            return;
        }
        let at = edit.palette_at;
        let is_node = edit.node.is_some();
        let mut apply_corner = false;
        Area::new(Id::new("polyline_edit_palette"))
            .order(Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(190.);
                    ui.strong(language.translate(if is_node {
                        "Polyline node"
                    } else {
                        "Polyline edge"
                    }));
                    ui.separator();
                    let commands: &[(PolylineEditCommand, &str)] = if is_node {
                        &[
                            (
                                PolylineEditCommand::MoveNode,
                                language.translate("Move node"),
                            ),
                            (PolylineEditCommand::Chamfer, language.translate("Chamfer…")),
                            (PolylineEditCommand::Fillet, language.translate("Fillet…")),
                            (
                                PolylineEditCommand::CurveEdge,
                                language.translate("Curve edge"),
                            ),
                            (
                                PolylineEditCommand::OffsetPolyline,
                                language.translate("Offset Polyline"),
                            ),
                            (PolylineEditCommand::Move, language.translate("Move")),
                            (PolylineEditCommand::Rotate, language.translate("Rotate")),
                            (PolylineEditCommand::Mirror, language.translate("Mirror")),
                            (
                                PolylineEditCommand::Multiply,
                                language.translate("Multiply"),
                            ),
                        ]
                    } else {
                        &[
                            (
                                PolylineEditCommand::InsertNode,
                                language.translate("Insert node"),
                            ),
                            (
                                PolylineEditCommand::OffsetPolyline,
                                language.translate("Offset Polyline"),
                            ),
                            (
                                PolylineEditCommand::CurveEdge,
                                language.translate("Curve edge"),
                            ),
                            (PolylineEditCommand::Move, language.translate("Move")),
                            (PolylineEditCommand::Rotate, language.translate("Rotate")),
                            (PolylineEditCommand::Mirror, language.translate("Mirror")),
                            (
                                PolylineEditCommand::Multiply,
                                language.translate("Multiply"),
                            ),
                        ]
                    };
                    for (command, label) in commands {
                        if ui
                            .selectable_label(edit.command == *command, *label)
                            .clicked()
                        {
                            edit.command = *command;
                            edit.error.clear();
                        }
                    }
                    if matches!(
                        edit.command,
                        PolylineEditCommand::Chamfer | PolylineEditCommand::Fillet
                    ) {
                        ui.separator();
                        if edit.command == PolylineEditCommand::Chamfer {
                            ui.label(language.translate("Cut length (m)"));
                            ui.add(
                                DragValue::new(&mut edit.chamfer_length)
                                    .speed(0.01)
                                    .range(0.001..=1000.),
                            );
                        } else {
                            ui.label(language.translate("Radius (m)"));
                            ui.add(
                                DragValue::new(&mut edit.fillet_radius)
                                    .speed(0.01)
                                    .range(0.001..=1000.),
                            );
                        }
                        if ui.button(language.translate("Apply corner edit")).clicked() {
                            apply_corner = true;
                        }
                    } else if edit.command == PolylineEditCommand::Multiply {
                        ui.separator();
                        ui.label(language.translate("Number of copies"));
                        ui.add(DragValue::new(&mut edit.multiply_count).range(1..=50));
                        ui.label(language.translate("Drag to place repeated copies"));
                    } else {
                        ui.separator();
                        ui.label(match edit.command {
                            PolylineEditCommand::InsertNode => {
                                "Drag on the selected edge to place a node"
                            }
                            PolylineEditCommand::MoveNode => {
                                language.translate("Drag the selected node to edit it")
                            }
                            PolylineEditCommand::OffsetPolyline => language
                                .translate("Drag inward or outward to offset the whole Polyline"),
                            PolylineEditCommand::CurveEdge => {
                                language.translate("Drag the segment to define a circular arc")
                            }
                            PolylineEditCommand::Move => {
                                language.translate("Drag to move the Polyline")
                            }
                            PolylineEditCommand::Rotate => {
                                language.translate("Drag around the pivot to rotate")
                            }
                            PolylineEditCommand::Mirror => {
                                language.translate("Drag to define the mirror axis")
                            }
                            PolylineEditCommand::Multiply => {
                                language.translate("Drag to place repeated copies")
                            }
                            _ => language.translate("Choose a corner command"),
                        });
                    }
                    if !edit.error.is_empty() {
                        ui.colored_label(Color32::from_rgb(240, 125, 110), &edit.error);
                    }
                });
            });
        if apply_corner {
            self.apply_polyline_corner_edit();
        }
    }
    fn apply_polyline_edit_cursor(&self, ctx: &Context) {
        if self
            .polyline_edit
            .as_ref()
            .is_some_and(|edit| edit.command == PolylineEditCommand::MoveNode)
        {
            let cursor = ctx.input(|input| {
                if input.pointer.button_down(PointerButton::Middle)
                    || input.pointer.button_down(PointerButton::Secondary)
                    || (input.pointer.button_down(PointerButton::Primary)
                        && input.key_down(Key::Space))
                {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Default
                }
            });
            ctx.set_cursor_icon(cursor);
        }
    }
    fn model_view(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(r, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "3D model canvas"));
        let placing_3d_marquee = self.editor.tool == Tool::Marquee;
        let base_z = self.editor.project.elevation(self.editor.story);
        let hovered_model_hit = response.interact_pointer_pos().and_then(|pos| {
            self.camera.pick(
                &self.editor.project,
                [
                    (pos.x - r.left()) / r.width() * 2. - 1.,
                    1. - (pos.y - r.top()) / r.height() * 2.,
                ],
                r.aspect_ratio(),
                if self.all_stories {
                    None
                } else {
                    Some(self.editor.story)
                },
            )
        });
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.drag_started_by(PointerButton::Primary)
        {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                self.marquee_3d_preview = Some((pos, pos));
            }
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.dragged_by(PointerButton::Primary)
        {
            if let (Some((start, _)), Some(pos)) =
                (self.marquee_3d_preview, response.interact_pointer_pos())
            {
                self.marquee_3d_preview = Some((start, pos));
            }
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.drag_stopped()
        {
            if let Some((a, b)) = self.marquee_3d_preview.take() {
                if a.distance(b) > 5. {
                    let points: Option<Vec<Point>> = [
                        pos2(a.x, a.y),
                        pos2(b.x, a.y),
                        pos2(b.x, b.y),
                        pos2(a.x, b.y),
                    ]
                    .into_iter()
                    .map(|pos| Self::screen_to_ground(&self.camera, r, pos, base_z))
                    .collect();
                    if let Some(points) = points {
                        self.place_3d_marquee(points, base_z);
                    } else {
                        self.editor.status =
                            "Marquee base must be below the perspective horizon".into();
                    }
                }
            }
        }
        if placing_3d_marquee && self.marquee_geometry != MarqueeGeometry::Rectangle {
            self.marquee_3d_cursor = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|pos| r.contains(*pos));
        }
        if placing_3d_marquee
            && self.marquee_geometry != MarqueeGeometry::Rectangle
            && response.clicked_by(PointerButton::Primary)
        {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = Self::screen_to_ground(&self.camera, r, pos, base_z) {
                    self.marquee_points.push(point);
                    self.marquee_3d_points.push(pos);
                    match self.marquee_geometry {
                        MarqueeGeometry::Polygon => {}
                        MarqueeGeometry::RotatedRectangle if self.marquee_points.len() == 3 => {
                            let vertices = geometry::rotated_rectangle_polygon(
                                self.marquee_points[0],
                                self.marquee_points[1],
                                self.marquee_points[2],
                            );
                            if let Some(vertices) = vertices {
                                self.place_3d_marquee(vertices, base_z);
                            } else {
                                self.marquee_points.clear();
                                self.marquee_3d_points.clear();
                                self.editor.status =
                                    "Rotated rectangle needs non-zero sides".into();
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.editor.status =
                        "Marquee base must be below the perspective horizon".into();
                }
            }
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Polygon
            && response.clicked_by(PointerButton::Secondary)
        {
            if self.marquee_points.len() >= 3 {
                self.place_3d_marquee(self.marquee_points.clone(), base_z);
            } else {
                self.marquee_points.clear();
                self.marquee_3d_points.clear();
                self.marquee_3d_cursor = None;
                self.editor.status = "3D Marquee polygon needs at least three points".into();
            }
        }
        let middle_dragging = ui.input(|i| {
            i.pointer.button_down(PointerButton::Middle)
                && i.pointer.delta().length_sq() > 0.
                && i.pointer.hover_pos().is_some_and(|pos| r.contains(pos))
        });
        let shift_middle_orbit = middle_dragging && ui.input(|i| i.modifiers.shift);
        if (response.dragged_by(PointerButton::Primary)
            && !placing_3d_marquee
            && self.editor.tool != Tool::Marquee)
            || shift_middle_orbit
        {
            let d = ui.input(|i| i.pointer.delta());
            self.camera.yaw -= d.x * 0.009;
            self.camera.pitch = (self.camera.pitch + d.y * 0.007).clamp(0.08, 1.53);
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let model_pan = response.dragged_by(PointerButton::Secondary)
            || (middle_dragging && !ui.input(|i| i.modifiers.shift));
        if model_pan {
            let delta = ui.input(|i| i.pointer.delta());
            let right = glam::Vec3::new(-self.camera.yaw.sin(), self.camera.yaw.cos(), 0.);
            let up = glam::Vec3::new(
                -self.camera.yaw.cos() * self.camera.pitch.sin(),
                -self.camera.yaw.sin() * self.camera.pitch.sin(),
                self.camera.pitch.cos(),
            );
            self.camera.target += (-right * delta.x + up * delta.y)
                * self.camera.vertical_span(r.aspect_ratio())
                / r.height();
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.camera.span = (self.camera.span * (-scroll * 0.002).exp()).clamp(2., 150.);
        }
        if response.clicked() && self.editor.tool != Tool::Marquee {
            if let Some(pos) = response.interact_pointer_pos() {
                let hit = self.camera.pick(
                    &self.editor.project,
                    [
                        (pos.x - r.left()) / r.width() * 2. - 1.,
                        1. - (pos.y - r.top()) / r.height() * 2.,
                    ],
                    r.aspect_ratio(),
                    if self.all_stories {
                        None
                    } else {
                        Some(self.editor.story)
                    },
                );
                self.editor.set_selection(hit.into_iter().collect());
                self.draft_id = None;
            }
        }
        if let Some(renderer) = &mut self.renderer {
            let scale = ui.ctx().pixels_per_point();
            let selected = self.editor.selection_ids();
            let selection_only =
                self.show_marquee_3d && !selected.is_empty() && self.editor.marquee.is_none();
            renderer.render(
                &self.editor.project,
                &selected,
                &self.camera,
                [(r.width() * scale) as u32, (r.height() * scale) as u32],
                if self.all_stories {
                    None
                } else {
                    Some(self.editor.story)
                },
                if self.show_marquee_3d && !selection_only {
                    self.editor.marquee.as_ref()
                } else {
                    None
                },
                self.marquee_outside,
                selection_only,
            );
            ui.painter().image(
                renderer.texture,
                r,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        } else {
            ui.painter().rect_filled(r, 0., LINE);
        }
        let p = ui.painter_at(r);
        if let Some((a, b)) = self.marquee_3d_preview {
            let rect = Rect::from_two_pos(a, b);
            p.rect_filled(rect, 0., Color32::from_rgba_unmultiplied(49, 136, 177, 25));
            p.rect_stroke(rect, 0., Stroke::new(1.5, ACCENT), StrokeKind::Inside);
        }
        if self.show_marquee_3d {
            if let Some(area) = &self.editor.marquee {
                let (bottom, top) = area.z_range.unwrap_or_else(|| {
                    let mut lo = f32::INFINITY;
                    let mut hi = f32::NEG_INFINITY;
                    for element in &self.editor.project.elements {
                        for corner in geometry::corners(&self.editor.project, element) {
                            lo = lo.min(corner.z);
                            hi = hi.max(corner.z);
                        }
                    }
                    if lo.is_finite() && hi.is_finite() {
                        (lo, hi)
                    } else {
                        (0., self.marquee_height)
                    }
                });
                let matrix = self.camera.matrix(r.aspect_ratio());
                let screen_point = |point: Point, z: f32| {
                    let ndc = matrix.project_point3(glam::Vec3::new(point.x, point.y, z));
                    ndc.is_finite().then(|| {
                        pos2(
                            r.left() + (ndc.x + 1.) * 0.5 * r.width(),
                            r.top() + (1. - ndc.y) * 0.5 * r.height(),
                        )
                    })
                };
                let base: Vec<_> = area
                    .vertices
                    .iter()
                    .filter_map(|point| screen_point(*point, bottom))
                    .collect();
                let cap: Vec<_> = area
                    .vertices
                    .iter()
                    .filter_map(|point| screen_point(*point, top))
                    .collect();
                if base.len() == area.vertices.len() && cap.len() == area.vertices.len() {
                    for i in 0..base.len() {
                        let next = (i + 1) % base.len();
                        p.line_segment([base[i], base[next]], Stroke::new(1.7, ACCENT));
                        p.line_segment(
                            [cap[i], cap[next]],
                            Stroke::new(1.2, Color32::from_rgb(114, 193, 224)),
                        );
                        p.line_segment(
                            [base[i], cap[i]],
                            Stroke::new(1.1, Color32::from_rgb(114, 193, 224)),
                        );
                    }
                }
            }
        }
        if !self.marquee_3d_points.is_empty() {
            let mut preview = self.marquee_3d_points.clone();
            if let Some(cursor) = self.marquee_3d_cursor.filter(|pos| r.contains(*pos)) {
                preview.push(cursor);
                if self.marquee_geometry == MarqueeGeometry::RotatedRectangle && preview.len() >= 3
                {
                    preview = vec![
                        self.marquee_3d_points[0],
                        self.marquee_3d_points[1],
                        cursor,
                        self.marquee_3d_points[0],
                    ];
                }
            }
            if self.marquee_geometry == MarqueeGeometry::Polygon && preview.len() >= 3 {
                p.add(egui::Shape::closed_line(preview, Stroke::new(1.5, ACCENT)));
            } else if preview.len() >= 2 {
                p.add(egui::Shape::line(preview, Stroke::new(1.5, ACCENT)));
            }
            for point in &self.marquee_3d_points {
                p.circle_filled(*point, 3.5, Color32::WHITE);
                p.circle_stroke(*point, 3.5, Stroke::new(1., ACCENT));
            }
        }
        p.text(
            r.left_top() + vec2(18., 18.),
            Align2::LEFT_TOP,
            "3D  /  PERSPECTIVE",
            FontId::proportional(12.),
            Color32::from_rgb(91, 109, 106),
        );
        p.text(
            r.left_bottom() + vec2(18., -18.),
            Align2::LEFT_BOTTOM,
            if placing_3d_marquee { "3D Marquee · draw base; right-click closes polygon · depth from Info Box · Shift+middle-drag orbits" } else { "Shift + middle-drag to orbit · Right-drag to pan · Scroll to zoom" },
            FontId::proportional(10.),
            Color32::from_rgb(100, 115, 109),
        );
        let c = r.right_top() + vec2(-37., 45.);
        for (delta, color, label) in [
            (vec2(22., 8.), Color32::from_rgb(166, 90, 70), "X"),
            (vec2(-18., 12.), ACCENT, "Y"),
            (vec2(0., -22.), Color32::from_rgb(86, 115, 159), "Z"),
        ] {
            p.line_segment([c, c + delta], Stroke::new(1.5, color));
            p.text(
                c + delta * 1.3,
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(10.),
                color,
            );
        }
        if response.hovered() {
            let feedback = if shift_middle_orbit
                || model_pan
                || (response.dragged_by(PointerButton::Primary) && !placing_3d_marquee)
            {
                CursorFeedback::Dragging
            } else if placing_3d_marquee {
                CursorFeedback::Drawing
            } else if self.editor.tool == Tool::Select && hovered_model_hit.is_some() {
                CursorFeedback::Selectable
            } else {
                CursorFeedback::Empty
            };
            ui.ctx().set_cursor_icon(feedback.icon());
        }
    }
    fn tracker(&mut self, ui: &mut Ui) {
        let polyline = self.editor.tool == Tool::Polyline;
        let wall = self.editor.tool == Tool::Wall;
        let wall_method = self.editor.wall_settings.geometry.geometry_method;
        let rectangular = wall && wall_method == WallGeometryMethod::Rectangular;
        let chained = wall
            && matches!(
                wall_method,
                WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal
            );
        let tracker_anchor = self.editor.anchor.or_else(|| {
            polyline
                .then(|| self.editor.polyline_points.last().copied())
                .flatten()
        });
        if let Some(a) = tracker_anchor {
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_min_height(40.);
                    ui.horizontal_wrapped(|ui| {
                        Self::label(ui, self.language.translate("TRACKER"));
                        let angular =
                            matches!(self.editor.tool, Tool::Wall | Tool::Polyline) && !rectangular;
                        ui.label(if angular {
                            if wall { "Length" } else { "Distance" }
                        } else {
                            "Δ X"
                        });
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_a)
                                    .speed(0.01)
                                    .fixed_decimals(3)
                                    .suffix(" m"),
                            )
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                        ui.label(if angular { "Angle" } else { "Δ Y" });
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_b)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(if angular { " °" } else { " m" }),
                            )
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                        ui.checkbox(&mut self.tracker_locked, self.language.translate("Lock"));
                        let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                        let action = if polyline || (chained && self.tracker_locked) {
                            "Add point [Enter]"
                        } else if chained {
                            "Finish [Enter]"
                        } else {
                            "Create [Enter]"
                        };
                        if ui.button(self.language.translate(action)).clicked() || enter {
                            let target = self.tracker_target(a);
                            if polyline {
                                self.commit_polyline_point(target, 0.01);
                            } else if chained && !self.tracker_locked {
                                self.finish_wall_draft();
                            } else if chained {
                                self.editor.click(target, 0.);
                                self.tracker_locked = false;
                                self.draft_id = None;
                            } else {
                                if let Err(error) = self.editor.finish_wall(Some(target)) {
                                    self.editor.status = localized_error(self.language, &error);
                                }
                                self.tracker_locked = false;
                                self.draft_id = None;
                            }
                        }
                        if chained
                            && self.tracker_locked
                            && ui.button(self.language.translate("Finish")).clicked()
                        {
                            self.finish_wall_draft();
                        }
                        if !polyline && ui.button(self.language.translate("Cancel")).clicked() {
                            self.editor.cancel_wall_draft();
                            self.tracker_locked = false;
                        }
                    });
                });
        } else if polyline || wall {
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_min_height(40.);
                    ui.vertical_centered(|ui| {
                        ui.label(self.language.translate("Tracker · click a start node"))
                    });
                });
        }
    }
    fn file_window(&mut self, ctx: &Context) {
        if let Some(action) = self.file_action {
            let title = match action {
                FileAction::Save => "Save project",
                FileAction::Open => "Open project",
                FileAction::New => "New project",
            };
            let mut close = false;
            Modal::new(Id::new("project_file_dialog")).show(ctx, |ui| {
                ui.set_min_width(500.);
                ui.heading(title);
                if action != FileAction::Save && self.editor.dirty() {
                    ui.colored_label(
                        Color32::from_rgb(166, 101, 45),
                        "Unsaved changes will be replaced. Save first if you need them.",
                    );
                }
                if action != FileAction::New {
                    ui.label(self.language.translate("Project file · OpenArchi JSON v1"));
                    ui.add(TextEdit::singleline(&mut self.path).desired_width(500.));
                } else {
                    ui.label(
                        self.language
                            .translate("Start an empty project with three stories."),
                    );
                }
                if !self.file_error.is_empty() {
                    ui.colored_label(Color32::from_rgb(177, 60, 40), &self.file_error);
                }
                ui.horizontal(|ui| {
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                    if ui
                        .button(match action {
                            FileAction::Save => "Save",
                            FileAction::Open => "Open / replace",
                            FileAction::New => "Create / replace",
                        })
                        .clicked()
                    {
                        let result = match action {
                            FileAction::Save => storage::save(
                                &self.editor.project,
                                &PathBuf::from(self.path.trim()),
                            )
                            .map(|()| {
                                self.editor.saved = self.editor.project.clone();
                                self.editor.status = "Project saved".into();
                            }),
                            FileAction::Open => storage::load(&PathBuf::from(self.path.trim()))
                                .map(|p| {
                                    self.editor = Editor::new(p);
                                    self.guide_ui = GuideUi::default();
                                    self.draft_id = None;
                                    self.draft = None;
                                    self.fit();
                                }),
                            FileAction::New => {
                                self.editor = Editor::new(Project {
                                    name: "Untitled project".into(),
                                    ..Project::default()
                                });
                                self.guide_ui = GuideUi::default();
                                self.draft_id = None;
                                self.draft = None;
                                self.fit();
                                Ok(())
                            }
                        };
                        match result {
                            Ok(()) => {
                                close = true;
                                self.file_error.clear()
                            }
                            Err(e) => self.file_error = e,
                        }
                    }
                });
            });
            if close {
                self.file_action = None;
                self.file_error.clear();
            }
        }
    }
}
impl AtelierApp {
    fn set_plan_zoom_percent(&mut self, percent: f32) {
        let percent = percent.clamp(1.0, 1000.0);
        self.plan_scale = (40.0 * percent / 100.0).clamp(0.4, 400.0);
        self.editor.status = format!("{} · {percent:.1}%", self.language.translate("Zoom"));
    }

    fn plan_zoom_percent(&self) -> f32 {
        self.plan_scale / 40.0 * 100.0
    }

    fn toggle_grid_snap(&mut self) {
        let enabled = !self.editor.project.grid_settings.grid_snap_enabled;
        self.set_grid_snap_enabled(enabled);
    }

    fn set_grid_snap_enabled(&mut self, enabled: bool) {
        let settings = &mut self.editor.project.grid_settings;
        settings.set_snap_enabled(enabled);
        self.editor.status = if settings.grid_snap_enabled {
            format!(
                "{} · {}",
                self.language.translate("Grid Snap on"),
                self.language.translate(settings.snap_mode.label_key())
            )
        } else {
            self.language.translate("Grid Snap off").into()
        };
    }

    fn modify_plan_options<F>(&mut self, change: F, status: impl Into<String>) -> bool
    where
        F: FnOnce(&mut PlanOptions),
    {
        let before = self.editor.project.clone();
        change(&mut self.editor.project.plan_options);
        if let Err(error) = self.editor.project.plan_options.validate() {
            self.editor.project.plan_options = before.plan_options;
            self.editor.status = error;
            return false;
        }
        if self.editor.project.plan_options == before.plan_options {
            return false;
        }
        self.editor.commit(before);
        self.editor.status = status.into();
        true
    }

    fn set_layer_combination(&mut self, combination: LayerCombination) {
        let status = self.option_status("Layer Combination", combination.name());
        self.modify_plan_options(
            |options| {
                options.layer_combination = combination;
                options.show_all_layers = false;
            },
            status,
        );
    }

    fn set_show_all_layers(&mut self, show: bool) {
        let status = self.option_status(
            "Show All Layers",
            if show {
                self.language.translate("On")
            } else {
                self.language.translate("Off")
            },
        );
        self.modify_plan_options(|options| options.show_all_layers = show, status);
    }

    fn set_pen_set(&mut self, pen_set: PenSet) {
        let status = self.option_status("Pen Set", pen_set.name());
        self.modify_plan_options(|options| options.pen_set = pen_set, status);
    }

    fn set_partial_structure(&mut self, display: PartialStructureDisplay) {
        let status = self.option_status("Partial Structure", display.name());
        self.modify_plan_options(|options| options.partial_structure = display, status);
    }

    fn set_model_view(&mut self, model_view: ModelViewOptions) {
        let status = self.option_status("Model View Options", model_view.name());
        self.modify_plan_options(|options| options.model_view = model_view, status);
    }

    fn set_graphic_overrides(&mut self, overrides: GraphicOverrides) {
        let status = self.option_status("Graphic Overrides", overrides.name());
        self.modify_plan_options(|options| options.graphic_overrides = overrides, status);
    }

    fn set_renovation_filter(&mut self, filter: RenovationFilter) {
        let status = self.option_status("Renovation Filter", filter.name());
        self.modify_plan_options(|options| options.renovation_filter = filter, status);
    }

    fn set_dimension_style(&mut self, style: DimensionStyle) {
        let status = self.option_status("Dimensions", style.name());
        self.modify_plan_options(|options| options.dimension_style = style, status);
    }

    fn set_drawing_scale(&mut self, denominator: u32) {
        let status = format!(
            "{} · 1:{denominator}",
            self.language.translate("Floor Plan Scale")
        );
        self.modify_plan_options(
            |options| options.drawing_scale = denominator.clamp(1, 10_000),
            status,
        );
    }

    fn set_orientation_degrees(&mut self, degrees: f32) {
        let degrees = degrees.rem_euclid(360.0);
        let status = format!("{} · {degrees:.2}°", self.language.translate("Orientation"));
        self.modify_plan_options(|options| options.orientation_degrees = degrees, status);
    }

    fn set_active_design_option(&mut self, option: Option<u32>) {
        let label = option
            .and_then(|id| self.editor.project.design_options.name(id))
            .map_or_else(
                || self.language.translate("Main Model Only").to_owned(),
                str::to_owned,
            );
        let status = format!("{} · {label}", self.language.translate("Design Options"));
        self.modify_plan_options(|options| options.active_design_option = option, status);
    }

    fn begin_orientation_pick(&mut self) {
        self.orientation_pick_active = true;
        self.orientation_pick_start = None;
        self.editor.status = self
            .language
            .translate("Set Orientation Graphically · click two points")
            .into();
    }

    fn cancel_orientation_pick(&mut self) {
        self.orientation_pick_active = false;
        self.orientation_pick_start = None;
        self.editor.status = self.language.translate("Set Orientation cancelled").into();
    }

    fn reset_renovation_status(&mut self) {
        let before = self.editor.project.clone();
        let selection: Vec<u64> = self.editor.selection_ids();
        let mut changed = 0;
        for element in &mut self.editor.project.elements {
            if selection.is_empty() || selection.contains(&element.id) {
                if element.renovation != RenovationStatus::Existing {
                    element.renovation = RenovationStatus::Existing;
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            self.editor.status = self
                .language
                .translate("Renovation status already Existing")
                .into();
            return;
        }
        self.editor.commit(before);
        self.editor.status = format!(
            "{} · {} {}",
            self.language.translate("Renovation status reset"),
            changed,
            self.language
                .translate(if changed == 1 { "element" } else { "elements" })
        );
    }

    fn open_plan_option_dialog(&mut self, kind: PlanOptionKind) {
        self.plan_dialog_options = self.editor.project.plan_options.clone();
        self.plan_option_dialog = Some(kind);
    }

    fn plan_option_title(language: UiLanguage, kind: PlanOptionKind) -> &'static str {
        language.translate(match kind {
            PlanOptionKind::Layers => "Layers",
            PlanOptionKind::PenSets => "Pen Sets",
            PlanOptionKind::PartialStructure => "Partial Structure Display",
            PlanOptionKind::ModelView => "Model View Options",
            PlanOptionKind::GraphicOverrides => "Graphic Overrides",
            PlanOptionKind::Renovation => "Renovation",
            PlanOptionKind::Dimensions => "Dimensions",
            PlanOptionKind::DesignOptions => "Design Options",
            PlanOptionKind::Scale => "Floor Plan Scale",
        })
    }

    fn option_status(&self, label: &str, value: &str) -> String {
        format!(
            "{} · {}",
            self.language.translate(label),
            self.language.translate(value)
        )
    }

    fn option_menu_list<T: Copy + PartialEq>(
        ui: &mut Ui,
        current: T,
        all: &[T],
        label: impl Fn(T) -> &'static str,
    ) -> Option<T> {
        let mut picked = None;
        for &item in all {
            if ui.selectable_label(current == item, label(item)).clicked() {
                picked = Some(item);
                ui.close();
            }
        }
        picked
    }

    fn elide_text(text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            return text.to_owned();
        }
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    fn quick_option_button(
        ui: &mut Ui,
        icon: &str,
        label: &str,
        min_width: f32,
        max_width: f32,
        tip: &str,
    ) -> Response {
        let font = FontId::proportional(11.0);
        let max_chars = ((max_width - 40.) / 6.0).max(4.) as usize;
        let display = Self::elide_text(label, max_chars);
        let text_width = ui
            .painter()
            .layout_no_wrap(display.clone(), font.clone(), INK)
            .size()
            .x;
        let width = (text_width + 40.).clamp(min_width, max_width);
        let (rect, response) = ui.allocate_exact_size(vec2(width, 26.), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 2., PANEL_ALT);
        }
        Self::paint_tool_icon(
            ui.painter(),
            Rect::from_min_size(rect.left_top() + vec2(4., 5.), vec2(16., 16.)),
            icon,
            if response.hovered() { ACCENT } else { INK },
        );
        ui.painter().text(
            rect.left_center() + vec2(23., 0.),
            Align2::LEFT_CENTER,
            display,
            font,
            INK,
        );
        ui.painter().text(
            rect.right_center() - vec2(5., 0.),
            Align2::RIGHT_CENTER,
            "›",
            FontId::proportional(13.),
            MUTED,
        );
        response.on_hover_text(tip)
    }

    fn zoom_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Save Current Zoom…"))
            .clicked()
        {
            let name = format!(
                "{} {}",
                language.translate("Zoom"),
                self.saved_zooms.len() + 1
            );
            self.saved_zooms.push((name.clone(), self.plan_scale));
            self.editor.status = format!("{} · {name}", language.translate("Saved zoom"));
            ui.close();
        }
        ui.add_enabled(false, Button::new(language.translate("Rename …")));
        ui.add_enabled(false, Button::new(language.translate("Delete")));
        ui.separator();
        for percent in ZOOM_PRESETS {
            let selected = (self.plan_zoom_percent() - percent as f32).abs() < 0.05;
            if ui
                .selectable_label(selected, format!("{percent}%"))
                .clicked()
            {
                self.set_plan_zoom_percent(percent as f32);
                ui.close();
            }
        }
        if !self.saved_zooms.is_empty() {
            ui.separator();
            let saved = self.saved_zooms.clone();
            for (name, scale) in saved {
                let percent = scale / 40.0 * 100.0;
                if ui
                    .selectable_label(false, format!("{name} · {percent:.1}%"))
                    .clicked()
                {
                    self.plan_scale = scale;
                    self.editor.status = format!("{} · {name}", language.translate("Zoom"));
                    ui.close();
                }
            }
        }
    }

    fn orientation_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Set Orientation Graphically…"))
            .clicked()
        {
            self.begin_orientation_pick();
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.normalized_orientation();
        for degrees in ORIENTATION_PRESETS {
            if ui
                .selectable_label((current - degrees).abs() < 0.005, format!("{degrees:.2}°"))
                .clicked()
            {
                self.set_orientation_degrees(degrees);
                ui.close();
            }
        }
    }

    fn scale_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui.button(language.translate("Custom Scale…")).clicked() {
            self.open_plan_option_dialog(PlanOptionKind::Scale);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.drawing_scale;
        for preset in SCALE_PRESETS {
            if ui
                .selectable_label(current == preset, format!("1:{preset}"))
                .clicked()
            {
                self.set_drawing_scale(preset);
                ui.close();
            }
        }
    }

    fn layers_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Layers (Model Views)…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::Layers);
            ui.close();
        }
        let mut show_all = self.editor.project.plan_options.show_all_layers;
        if ui
            .checkbox(&mut show_all, language.translate("Show All Layers"))
            .changed()
        {
            self.set_show_all_layers(show_all);
        }
        ui.add_enabled(false, Button::new(language.translate("Layer Extras")));
        ui.separator();
        let current = self.editor.project.plan_options.layer_combination;
        if let Some(combination) =
            Self::option_menu_list(ui, current, &LayerCombination::ALL, |value| {
                language.translate(value.name())
            })
        {
            self.set_layer_combination(combination);
        }
    }

    fn partial_structure_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Partial Structure Display…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::PartialStructure);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.partial_structure;
        for option in PartialStructureDisplay::ALL {
            if ui
                .radio(current == option, language.translate(option.name()))
                .clicked()
            {
                self.set_partial_structure(option);
                ui.close();
            }
        }
    }

    fn pen_set_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Pens & Color (Model Views)…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::PenSets);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.pen_set;
        if let Some(pen_set) = Self::option_menu_list(ui, current, &PenSet::ALL, |value| {
            language.translate(value.name())
        }) {
            self.set_pen_set(pen_set);
        }
    }

    fn model_view_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Model View Options…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::ModelView);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.model_view;
        if let Some(model_view) =
            Self::option_menu_list(ui, current, &ModelViewOptions::ALL, |value| {
                language.translate(value.name())
            })
        {
            self.set_model_view(model_view);
        }
    }

    fn graphic_overrides_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Graphic Override Combinations…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::GraphicOverrides);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.graphic_overrides;
        if let Some(overrides) =
            Self::option_menu_list(ui, current, &GraphicOverrides::ALL, |value| {
                language.translate(value.name())
            })
        {
            self.set_graphic_overrides(overrides);
        }
        ui.separator();
        if ui
            .button(language.translate("Graphic Override Rules…"))
            .clicked()
        {
            self.editor.status = language
                .translate("Graphic Override Rules · interface preview")
                .into();
            ui.close();
        }
    }

    fn renovation_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Renovation Filter Options…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::Renovation);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.renovation_filter;
        if let Some(filter) = Self::option_menu_list(ui, current, &RenovationFilter::ALL, |value| {
            language.translate(value.name())
        }) {
            self.set_renovation_filter(filter);
        }
        ui.separator();
        if ui
            .button(language.translate("Reset Renovation Status…"))
            .clicked()
        {
            self.reset_renovation_status();
            ui.close();
        }
    }

    fn design_options_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Design Option Manager"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::DesignOptions);
            ui.close();
        }
        ui.separator();
        let active = self.editor.project.plan_options.active_design_option;
        if ui
            .radio(active.is_none(), language.translate("Main Model Only"))
            .clicked()
        {
            self.set_active_design_option(None);
            ui.close();
        }
        let options: Vec<(u32, String)> = self
            .editor
            .project
            .design_options
            .options
            .iter()
            .map(|option| (option.id, option.name.clone()))
            .collect();
        if !options.is_empty() {
            ui.separator();
            for (id, name) in options {
                if ui.radio(active == Some(id), name).clicked() {
                    self.set_active_design_option(Some(id));
                    ui.close();
                }
            }
        }
    }

    fn dimensions_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui.button(language.translate("Dimensions…")).clicked() {
            self.open_plan_option_dialog(PlanOptionKind::Dimensions);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.dimension_style;
        if let Some(style) = Self::option_menu_list(ui, current, &DimensionStyle::ALL, |value| {
            language.translate(value.name())
        }) {
            self.set_dimension_style(style);
        }
    }

    fn toggle_quick_options_bar(&mut self) {
        self.quick_options_visible = !self.quick_options_visible;
        self.editor.status = if self.quick_options_visible {
            self.language.translate("Quick Options bar shown").into()
        } else {
            self.language.translate("Quick Options bar hidden").into()
        };
    }

    fn toggle_quick_options_palette(&mut self) {
        self.quick_options_palette_visible = !self.quick_options_palette_visible;
        self.editor.status = if self.quick_options_palette_visible {
            self.language
                .translate("Quick Options palette shown")
                .into()
        } else {
            self.language
                .translate("Quick Options palette hidden")
                .into()
        };
    }

    fn quick_options_bar(&mut self, ui: &mut Ui) {
        let language = self.language;
        egui::Panel::bottom("quick_options_bar")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(2, 1)),
            )
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .id_salt("quick_options_scroll")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            let zoom = format!("{:.1}%", self.plan_zoom_percent());
                            let response = Self::quick_option_button(
                                ui,
                                "Zoom In",
                                &zoom,
                                64.,
                                84.,
                                &format!("{} · {zoom}", language.translate("Zoom")),
                            );
                            Popup::menu(&response).show(|ui| self.zoom_menu_contents(ui));
                            ui.separator();
                            let orientation = format!(
                                "{:.2}°",
                                self.editor.project.plan_options.normalized_orientation()
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Orientation",
                                &orientation,
                                78.,
                                96.,
                                &format!("{} · {orientation}", language.translate("Orientation")),
                            );
                            Popup::menu(&response).show(|ui| self.orientation_menu_contents(ui));
                            ui.separator();
                            let scale =
                                format!("1:{}", self.editor.project.plan_options.drawing_scale);
                            let response = Self::quick_option_button(
                                ui,
                                "Scale",
                                &scale,
                                62.,
                                80.,
                                &format!("{} · {scale}", language.translate("Scale")),
                            );
                            Popup::menu(&response).show(|ui| self.scale_menu_contents(ui));
                            ui.separator();
                            let layer_label = if self.editor.project.plan_options.show_all_layers {
                                language.translate("All Layers").to_owned()
                            } else {
                                let name =
                                    self.editor.project.plan_options.layer_combination.name();
                                language.translate(name).to_owned()
                            };
                            let response = Self::quick_option_button(
                                ui,
                                "Layer Combination",
                                &layer_label,
                                120.,
                                158.,
                                &format!(
                                    "{} · {layer_label}",
                                    language.translate("Layer Combination")
                                ),
                            );
                            Popup::menu(&response).show(|ui| self.layers_menu_contents(ui));
                            ui.separator();
                            let partial = language.translate(
                                self.editor.project.plan_options.partial_structure.name(),
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Partial Structure",
                                partial,
                                104.,
                                132.,
                                &format!("{} · {partial}", language.translate("Partial Structure")),
                            );
                            Popup::menu(&response)
                                .show(|ui| self.partial_structure_menu_contents(ui));
                            ui.separator();
                            let pen =
                                language.translate(self.editor.project.plan_options.pen_set.name());
                            let response = Self::quick_option_button(
                                ui,
                                "Pen Set",
                                pen,
                                116.,
                                148.,
                                &format!("{} · {pen}", language.translate("Pen Set")),
                            );
                            Popup::menu(&response).show(|ui| self.pen_set_menu_contents(ui));
                            ui.separator();
                            let model_view = language
                                .translate(self.editor.project.plan_options.model_view.name());
                            let response = Self::quick_option_button(
                                ui,
                                "Model View",
                                model_view,
                                112.,
                                142.,
                                &format!(
                                    "{} · {model_view}",
                                    language.translate("Model View Options")
                                ),
                            );
                            Popup::menu(&response).show(|ui| self.model_view_menu_contents(ui));
                            ui.separator();
                            let overrides = language.translate(
                                self.editor.project.plan_options.graphic_overrides.name(),
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Graphic Overrides",
                                overrides,
                                108.,
                                142.,
                                &format!(
                                    "{} · {overrides}",
                                    language.translate("Graphic Overrides")
                                ),
                            );
                            Popup::menu(&response)
                                .show(|ui| self.graphic_overrides_menu_contents(ui));
                            ui.separator();
                            let renovation = language.translate(
                                self.editor.project.plan_options.renovation_filter.name(),
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Renovation",
                                renovation,
                                112.,
                                146.,
                                &format!(
                                    "{} · {renovation}",
                                    language.translate("Renovation Filter")
                                ),
                            );
                            Popup::menu(&response).show(|ui| self.renovation_menu_contents(ui));
                            ui.separator();
                            let design = self
                                .editor
                                .project
                                .plan_options
                                .active_design_option
                                .and_then(|id| self.editor.project.design_options.name(id))
                                .unwrap_or(language.translate("Main Model Only"));
                            let response = Self::quick_option_button(
                                ui,
                                "Design Options",
                                design,
                                108.,
                                142.,
                                &format!("{} · {design}", language.translate("Design Options")),
                            );
                            Popup::menu(&response).show(|ui| self.design_options_menu_contents(ui));
                            ui.separator();
                            let dimensions = language
                                .translate(self.editor.project.plan_options.dimension_style.name());
                            let response = Self::quick_option_button(
                                ui,
                                "Dimension Style",
                                dimensions,
                                86.,
                                116.,
                                &format!("{} · {dimensions}", language.translate("Dimensions")),
                            );
                            Popup::menu(&response).show(|ui| self.dimensions_menu_contents(ui));
                        });
                    });
            });
    }

    fn palette_row(ui: &mut Ui, label: &str, value: String, contents: impl FnOnce(&mut Ui)) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(10.).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button(value, contents);
            });
        });
    }

    fn quick_options_palette(&mut self, ctx: &Context) {
        if !self.quick_options_palette_visible {
            return;
        }
        let mut open = self.quick_options_palette_visible;
        let language = self.language;
        egui::Window::new(Self::t(language, "Quick Options"))
            .id(Id::new("quick_options_palette_window"))
            .default_pos(pos2(1180., 520.))
            .default_width(268.)
            .resizable(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(language.translate("Floor Plan"))
                        .size(10.)
                        .strong()
                        .color(MUTED),
                );
                ui.separator();
                let zoom = format!("{:.1}%", self.plan_zoom_percent());
                Self::palette_row(ui, language.translate("Zoom"), zoom, |ui| {
                    self.zoom_menu_contents(ui)
                });
                let orientation = format!(
                    "{:.2}°",
                    self.editor.project.plan_options.normalized_orientation()
                );
                Self::palette_row(ui, language.translate("Orientation"), orientation, |ui| {
                    self.orientation_menu_contents(ui)
                });
                let scale = format!("1:{}", self.editor.project.plan_options.drawing_scale);
                Self::palette_row(ui, language.translate("Scale"), scale, |ui| {
                    self.scale_menu_contents(ui)
                });
                let layer = if self.editor.project.plan_options.show_all_layers {
                    language.translate("All Layers").to_owned()
                } else {
                    let name = self.editor.project.plan_options.layer_combination.name();
                    language.translate(name).to_owned()
                };
                Self::palette_row(ui, language.translate("Layer Combination"), layer, |ui| {
                    self.layers_menu_contents(ui)
                });
                let partial = language
                    .translate(self.editor.project.plan_options.partial_structure.name())
                    .to_owned();
                Self::palette_row(ui, language.translate("Partial Structure"), partial, |ui| {
                    self.partial_structure_menu_contents(ui)
                });
                let pen = language
                    .translate(self.editor.project.plan_options.pen_set.name())
                    .to_owned();
                Self::palette_row(ui, language.translate("Pen Set"), pen, |ui| {
                    self.pen_set_menu_contents(ui)
                });
                let model_view = language
                    .translate(self.editor.project.plan_options.model_view.name())
                    .to_owned();
                Self::palette_row(
                    ui,
                    language.translate("Model View Options"),
                    model_view,
                    |ui| self.model_view_menu_contents(ui),
                );
                let overrides = language
                    .translate(self.editor.project.plan_options.graphic_overrides.name())
                    .to_owned();
                Self::palette_row(
                    ui,
                    language.translate("Graphic Overrides"),
                    overrides,
                    |ui| self.graphic_overrides_menu_contents(ui),
                );
                let renovation = language
                    .translate(self.editor.project.plan_options.renovation_filter.name())
                    .to_owned();
                Self::palette_row(
                    ui,
                    language.translate("Renovation Filter"),
                    renovation,
                    |ui| self.renovation_menu_contents(ui),
                );
                let design = self
                    .editor
                    .project
                    .plan_options
                    .active_design_option
                    .and_then(|id| self.editor.project.design_options.name(id))
                    .unwrap_or(language.translate("Main Model Only"))
                    .to_owned();
                Self::palette_row(ui, language.translate("Design Options"), design, |ui| {
                    self.design_options_menu_contents(ui)
                });
                let dimensions = language
                    .translate(self.editor.project.plan_options.dimension_style.name())
                    .to_owned();
                Self::palette_row(ui, language.translate("Dimensions"), dimensions, |ui| {
                    self.dimensions_menu_contents(ui)
                });
                ui.separator();
                if ui
                    .button(language.translate("Hide Quick Options Bar"))
                    .clicked()
                {
                    self.quick_options_visible = false;
                    self.editor.status = language.translate("Quick Options bar hidden").into();
                }
            });
        self.quick_options_palette_visible = open;
    }

    fn plan_options_dialog(&mut self, ctx: &Context) {
        let Some(kind) = self.plan_option_dialog else {
            return;
        };
        if kind == PlanOptionKind::DesignOptions {
            self.design_options_dialog(ctx);
            return;
        }
        let language = self.language;
        let mut options = self.plan_dialog_options.clone();
        let mut apply = false;
        let mut cancel = false;
        let modal = Modal::new(Id::new(("plan_options_dialog", kind as u8))).show(ctx, |ui| {
            ui.set_min_width(430.);
            ui.heading(Self::plan_option_title(language, kind));
            ui.separator();
            match kind {
                PlanOptionKind::Scale => {
                    ui.horizontal(|ui| {
                        ui.label("1 :");
                        ui.add(
                            DragValue::new(&mut options.drawing_scale)
                                .range(1..=10_000)
                                .speed(1.0),
                        );
                    });
                    ui.label(
                        RichText::new(
                            language
                                .translate("Plan scale controls pen widths and annotation size."),
                        )
                        .small()
                        .color(MUTED),
                    );
                    ui.add_space(6.);
                    ui.horizontal_wrapped(|ui| {
                        for preset in SCALE_PRESETS {
                            if ui
                                .selectable_label(
                                    options.drawing_scale == preset,
                                    format!("1:{preset}"),
                                )
                                .clicked()
                            {
                                options.drawing_scale = preset;
                            }
                        }
                    });
                }
                PlanOptionKind::Layers => {
                    ui.checkbox(
                        &mut options.show_all_layers,
                        language.translate("Show All Layers"),
                    );
                    ui.add_space(4.);
                    for combination in LayerCombination::ALL {
                        ui.radio_value(
                            &mut options.layer_combination,
                            combination,
                            language.translate(combination.name()),
                        );
                    }
                }
                PlanOptionKind::PenSets => {
                    for pen_set in PenSet::ALL {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(vec2(18., 18.), Sense::hover());
                            ui.painter().rect_filled(
                                rect,
                                3.,
                                Self::plan_color(pen_set.palette().line),
                            );
                            ui.radio_value(
                                &mut options.pen_set,
                                pen_set,
                                language.translate(pen_set.name()),
                            );
                        });
                    }
                }
                PlanOptionKind::PartialStructure => {
                    for display in PartialStructureDisplay::ALL {
                        ui.radio_value(
                            &mut options.partial_structure,
                            display,
                            language.translate(display.name()),
                        );
                    }
                }
                PlanOptionKind::ModelView => {
                    for model_view in ModelViewOptions::ALL {
                        ui.radio_value(
                            &mut options.model_view,
                            model_view,
                            language.translate(model_view.name()),
                        );
                    }
                }
                PlanOptionKind::GraphicOverrides => {
                    for overrides in GraphicOverrides::ALL {
                        ui.radio_value(
                            &mut options.graphic_overrides,
                            overrides,
                            language.translate(overrides.name()),
                        );
                    }
                }
                PlanOptionKind::Renovation => {
                    for filter in RenovationFilter::ALL {
                        ui.radio_value(
                            &mut options.renovation_filter,
                            filter,
                            language.translate(filter.name()),
                        );
                    }
                }
                PlanOptionKind::Dimensions => {
                    for style in DimensionStyle::ALL {
                        ui.radio_value(
                            &mut options.dimension_style,
                            style,
                            language.translate(style.name()),
                        );
                    }
                    ui.separator();
                    ui.label(format!(
                        "{} · 3.200 m → {}",
                        language.translate("Preview"),
                        options.dimension_style.format_length(3.2)
                    ));
                }
                PlanOptionKind::DesignOptions => {}
            }
            ui.add_space(12.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [94., 32.],
                        Button::new(Self::t(self.language, "OK")).fill(ACCENT),
                    )
                    .clicked()
                {
                    apply = true;
                }
                if ui.button(Self::t(self.language, "Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        self.plan_dialog_options = options;
        if apply {
            let before = self.editor.project.clone();
            self.editor.project.plan_options = self.plan_dialog_options.clone();
            match self.editor.project.plan_options.validate() {
                Ok(()) => {
                    if self.editor.project.plan_options != before.plan_options {
                        self.editor.commit(before);
                    }
                    self.editor.status = format!(
                        "{} {}",
                        Self::plan_option_title(self.language, kind),
                        self.language.translate("updated")
                    );
                    self.plan_option_dialog = None;
                }
                Err(error) => {
                    self.editor.project.plan_options = before.plan_options;
                    self.editor.status = error;
                }
            }
        }
        if cancel || modal.should_close() {
            self.plan_option_dialog = None;
        }
    }

    fn design_options_dialog(&mut self, ctx: &Context) {
        #[derive(Clone, Copy)]
        enum DesignAction {
            SetActive(Option<u32>),
            Delete(u32),
            Create,
            AssignSelected,
            MoveSelectedToMain,
        }
        let mut actions = Vec::new();
        let mut create_name = self.design_option_new_name.clone();
        let mut close = false;
        let language = self.language;
        let modal = Modal::new(Id::new("design_options_manager")).show(ctx, |ui| {
            ui.set_min_width(500.);
            ui.heading(language.translate("Design Options"));
            ui.label(
                RichText::new(language.translate(
                    "Main Model Only shows elements that are not assigned to a design option.",
                ))
                .small()
                .color(MUTED),
            );
            ui.separator();
            let active = self.editor.project.plan_options.active_design_option;
            if ui
                .radio(active.is_none(), language.translate("Main Model Only"))
                .clicked()
            {
                actions.push(DesignAction::SetActive(None));
            }
            ui.add_space(4.);
            let options: Vec<(u32, String)> = self
                .editor
                .project
                .design_options
                .options
                .iter()
                .map(|option| (option.id, option.name.clone()))
                .collect();
            for (id, name) in options {
                let count = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|element| element.design_option == Some(id))
                    .count();
                ui.horizontal(|ui| {
                    if ui.radio(active == Some(id), "").clicked() {
                        actions.push(DesignAction::SetActive(Some(id)));
                    }
                    ui.label(name);
                    ui.label(
                        RichText::new(format!(
                            "{count} {}",
                            language.translate(if count == 1 { "element" } else { "elements" })
                        ))
                        .small()
                        .color(MUTED),
                    );
                    if ui.small_button(language.translate("Delete")).clicked() {
                        actions.push(DesignAction::Delete(id));
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut create_name)
                        .hint_text(language.translate("New design option name"))
                        .desired_width(250.),
                );
                if ui.button(language.translate("New Design Option")).clicked() {
                    actions.push(DesignAction::Create);
                }
            });
            ui.add_space(6.);
            ui.horizontal(|ui| {
                let assign_enabled = active.is_some();
                if ui
                    .add_enabled(
                        assign_enabled,
                        Button::new(language.translate("Assign Selected to Active Option")),
                    )
                    .clicked()
                {
                    actions.push(DesignAction::AssignSelected);
                }
                if ui
                    .button(language.translate("Move Selected to Main Model"))
                    .clicked()
                {
                    actions.push(DesignAction::MoveSelectedToMain);
                }
            });
            ui.add_space(12.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [94., 32.],
                        Button::new(Self::t(self.language, "Close")).fill(ACCENT),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        });
        self.design_option_new_name = create_name;
        for action in actions {
            match action {
                DesignAction::SetActive(option) => self.set_active_design_option(option),
                DesignAction::Delete(id) => {
                    let before = self.editor.project.clone();
                    self.editor.project.design_options.remove(id);
                    if self.editor.project.plan_options.active_design_option == Some(id) {
                        self.editor.project.plan_options.active_design_option = None;
                    }
                    for element in &mut self.editor.project.elements {
                        if element.design_option == Some(id) {
                            element.design_option = None;
                        }
                    }
                    self.editor.commit(before);
                    self.editor.status = language.translate("Design option deleted").into();
                }
                DesignAction::Create => {
                    let before = self.editor.project.clone();
                    let next = self.editor.project.design_options.next_id.max(1);
                    let name = if self.design_option_new_name.trim().is_empty() {
                        format!("{} {next}", language.translate("Design Option"))
                    } else {
                        self.design_option_new_name.trim().to_owned()
                    };
                    let id = self.editor.project.design_options.create(name.clone());
                    self.editor.project.plan_options.active_design_option = Some(id);
                    self.editor.commit(before);
                    self.design_option_new_name.clear();
                    self.editor.status =
                        format!("{} · {name}", language.translate("Design option created"));
                }
                DesignAction::AssignSelected => {
                    let Some(active) = self.editor.project.plan_options.active_design_option else {
                        self.editor.status =
                            language.translate("Select a design option first").into();
                        continue;
                    };
                    let selection: Vec<u64> = self.editor.selection_ids();
                    if selection.is_empty() {
                        self.editor.status = language.translate("Select elements to assign").into();
                        continue;
                    }
                    let before = self.editor.project.clone();
                    for element in &mut self.editor.project.elements {
                        if selection.contains(&element.id) {
                            element.design_option = Some(active);
                        }
                    }
                    self.editor.commit(before);
                    self.editor.status = language
                        .translate("Selected elements assigned to design option")
                        .into();
                }
                DesignAction::MoveSelectedToMain => {
                    let selection: Vec<u64> = self.editor.selection_ids();
                    if selection.is_empty() {
                        self.editor.status = language.translate("Select elements to move").into();
                        continue;
                    }
                    let before = self.editor.project.clone();
                    for element in &mut self.editor.project.elements {
                        if selection.contains(&element.id) {
                            element.design_option = None;
                        }
                    }
                    self.editor.commit(before);
                    self.editor.status = language
                        .translate("Selected elements moved to Main Model")
                        .into();
                }
            }
        }
        if close || modal.should_close() {
            self.plan_option_dialog = None;
        }
    }

    fn workspace(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.file_action.is_none()
            && !self.help
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
                ui.label(self.language.translate("版本 0.1.0 · 原生 Rust 桌面应用"));
                ui.label(self.language.translate("在平面图中绘制墙体、矩形楼板和二维多段线。可在二维与三维视图中选择并编辑构件，尺寸单位为米。"));
                ui.separator();
                for text in ["V 选择 · M 选区框 · W 墙 · L 楼板 · P 多段线 · S 楼梯", "多段线：逐点绘制，按 Enter 完成，Esc 取消并返回上一个工具，再按 Esc 回到选择", "楼梯：设置参数后在平面点击起点和终点", "Esc：取消当前操作并返回上一个工具；再次按 Esc 回到选择", "F2 楼层平面 · F3 三维透视", "选区框支持矩形、旋转矩形和多边形", "F5 在三维中显示选择构件或选区框裁剪体", "⌘A 选择选区框内的构件节点", "⌘Z 撤销 · ⇧⌘Z 重做 · ⌘D 复制", "⌘S 保存 JSON · ⌘O 打开 JSON"] { ui.label(text); }
                ui.separator();
                ui.label(format!("渲染器：{}", self.gpu_name));
                ui.label(self.language.translate("原型限制：暂不支持 PDF/DWG 导出和原生 BIM 文件解析。当前助手为本地规则演示。"));
            } else {
                ui.heading(self.language.translate("OpenArchi · Architectural Design Workbench"));
                ui.label(self.language.translate("Version 0.1.0 · Native Rust desktop application"));
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

#[cfg(test)]
mod ui_tests {
    use super::*;
    fn setup() -> (Context, AtelierApp) {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = AtelierApp::from_renderer(None, "Headless test".into());
        app.editor = Editor::new(Project::default());
        app.plan_center = Point::default();
        app.plan_scale = 50.;
        (ctx, app)
    }
    fn plan_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> CursorIcon {
        let output = plan_frame_output(ctx, app, events);
        output.platform_output.cursor_icon
    }
    fn plan_frame_output(
        ctx: &Context,
        app: &mut AtelierApp,
        events: Vec<Event>,
    ) -> eframe::egui::FullOutput {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                CentralPanel::default()
                    .frame(Frame::NONE)
                    .show(ui, |ui| app.plan(ui));
                app.apply_polyline_edit_cursor(ui.ctx());
            },
        );
        output.textures_delta.clear();
        output
    }
    fn model_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                CentralPanel::default()
                    .frame(Frame::NONE)
                    .show(ui, |ui| app.model_view(ui));
            },
        );
        output.textures_delta.clear();
    }
    fn click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) -> CursorIcon {
        plan_frame(ctx, app, vec![Event::PointerMoved(pos)]);
        plan_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(
            ctx,
            app,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        )
    }
    #[test]
    fn selected_polyline_node_opens_palette_and_drag_edits_both_segments_once() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.editor.set_selection(vec![id]);
        let node = pos2(500., 300.);
        click(&ctx, &mut app, node);
        assert!(
            app.polyline_edit
                .as_ref()
                .is_some_and(|edit| edit.element_id == id && edit.node == Some(1))
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(node),
                Event::PointerButton {
                    pos: node,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let moved = pos2(550., 250.);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(moved)]);
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[1]==Point::new(3.,1.)),
            "incident segments update during the drag preview"
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: moved,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let Shape::Polyline { points, .. } = &app.editor.project.elements[0].shape else {
            panic!("shape changed");
        };
        assert_eq!(points[1], Point::new(3., 1.));
        assert_eq!(points[0], Point::new(0., 0.));
        assert_eq!(points[2], Point::new(2., 2.));
        assert_eq!(app.editor.undo.len(), 1, "node edit is one transaction");
        app.editor.undo();
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[1]==Point::new(2.,0.))
        );
    }
    #[test]
    fn selected_polyline_edge_click_requires_explicit_insert_command() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.editor.set_selection(vec![id]);
        click(&ctx, &mut app, pos2(450., 300.));
        assert!(
            matches!(app.editor.project.elements[0].shape, Shape::Polyline { ref points, .. } if points.len()==3)
        );
        assert!(app.editor.undo.is_empty());
        assert_eq!(app.polyline_edit.as_ref().unwrap().edge, Some(0));
        app.insert_polyline_node_at(id, 0, Point::new(1., 0.), pos2(450., 300.));
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==4 && points[1]==Point::new(1.,0.))
        );
        assert!(
            app.polyline_edit
                .as_ref()
                .is_some_and(|edit| edit.element_id == id
                    && edit.node == Some(1)
                    && edit.edge == Some(0)
                    && edit.command == PolylineEditCommand::MoveNode)
        );
        assert_eq!(
            app.editor.undo.len(),
            1,
            "node insertion is one undoable operation"
        );
        app.editor.undo();
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==3 && points[1]==Point::new(2.,0.))
        );
    }
    #[test]
    fn explicit_arc_node_insertion_preserves_circle_and_undo() {
        let (_ctx, mut app) = setup();
        let a = Point::new(0., 0.);
        let b = Point::new(2., 0.);
        let through = Point::new(1., 1.);
        let arc = geometry::arc_segment_through(a, b, through).unwrap();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![a, b],
                arcs: vec![Some(arc)],
            },
        );
        let before = app.editor.project.clone();
        app.open_polyline_edit(id, None, Some(0), Pos2::ZERO);
        let mut edit = app.polyline_edit.clone().unwrap();
        edit.command = PolylineEditCommand::InsertNode;
        app.apply_polyline_drag_preview(&before, &edit, through, through);
        let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
            panic!()
        };
        assert_eq!(points.len(), 3);
        assert!(points[1].distance(through) < 1e-4);
        for piece in arcs.iter().flatten() {
            assert!((piece.radius - arc.radius).abs() < 1e-5);
            assert_eq!(piece.clockwise, arc.clockwise);
        }
        assert!(app.editor.project.validate().is_ok());
        let loaded =
            serde_json::from_slice::<Project>(&serde_json::to_vec(&app.editor.project).unwrap())
                .unwrap();
        assert_eq!(loaded, app.editor.project);
        app.editor.commit(before.clone());
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        app.editor.redo();
        assert_eq!(app.editor.project, loaded);
    }
    #[test]
    fn selecting_curved_polyline_edge_opens_edit_without_inserting_a_node() {
        let (_ctx, mut app) = setup();
        let a = Point::new(0., 0.);
        let b = Point::new(2., 0.);
        let arc = geometry::arc_segment_through(a, b, Point::new(1., 0.5)).unwrap();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![a, b],
                arcs: vec![Some(arc)],
            },
        );
        app.editor.set_selection(vec![id]);
        app.insert_polyline_node_at(id, 0, Point::new(1., 0.5), Pos2::ZERO);
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points,ref arcs} if points.len()==2 && arcs[0].is_some())
        );
        assert!(
            app.polyline_edit
                .as_ref()
                .is_some_and(|edit| edit.element_id == id
                    && edit.edge == Some(0)
                    && edit.node.is_none())
        );
        assert!(app.editor.undo.is_empty());
    }
    #[test]
    fn dragging_polyline_node_in_select_moves_the_whole_object() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.editor.set_selection(vec![id]);
        let start = pos2(500., 300.);
        let end = pos2(550., 250.);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
        assert!(
            app.drag.is_some(),
            "dragging a node target in Select must start a whole-element drag"
        );
        assert!(
            app.polyline_edit.is_none(),
            "a drag from a node target must not enter node-edit mode"
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(1.,1.) && points[1]==Point::new(3.,1.) && points[2]==Point::new(3.,3.))
        );
        assert!(app.polyline_edit.is_none());
        assert_eq!(
            app.editor.undo.len(),
            1,
            "whole-element movement is one undoable action"
        );
    }
    #[test]
    fn confirmed_curve_then_pointer_offset_uses_current_shape_and_undo() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.)],
                arcs: vec![],
            },
        );
        app.editor.set_selection(vec![id]);
        app.open_polyline_edit(id, None, Some(0), pos2(450., 300.));
        let before = app.editor.project.clone();
        let mut edit = app.polyline_edit.clone().unwrap();
        edit.command = PolylineEditCommand::CurveEdge;
        app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 1.));
        app.editor.commit(before);
        let curved = app.editor.project.clone();
        for step in 0..2 {
            let y = 250. - step as f32 * 12.5;
            app.open_polyline_edit(id, None, Some(0), pos2(450., y));
            app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::OffsetPolyline;
            let start = pos2(450., y);
            let end = pos2(450., y - 12.5);
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
            plan_frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(start),
                    Event::PointerButton {
                        pos: start,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
            let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
                panic!()
            };
            assert_eq!(points.len(), 2);
            assert!(
                (arcs[0].unwrap().radius - (1.25 + step as f32 * 0.25)).abs() < 1e-4,
                "step={step} arc={:?} edit={:?}",
                arcs[0],
                app.polyline_edit
            );
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: end,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            assert!(app.polyline_edit.is_none());
        }
        let offset = app.editor.project.clone();
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&offset).unwrap()).unwrap();
        assert_eq!(loaded, offset);
        app.editor.undo();
        app.editor.undo();
        assert_eq!(app.editor.project, curved);
        app.editor.redo();
        app.editor.redo();
        assert_eq!(app.editor.project, offset);
    }

    #[test]
    fn polyline_offset_and_curve_commands_follow_pointer_preview_and_undo() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.editor.set_selection(vec![id]);
        app.open_polyline_edit(id, Some(1), Some(0), pos2(450., 300.));
        app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::OffsetPolyline;
        let start = pos2(450., 300.);
        let offset = pos2(450., 287.5);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(offset)]);
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if (points[0].y-0.25).abs()<1e-4 && (points[1].x-1.75).abs()<1e-4),
            "shape={:?} edit={:?} status={}",
            app.editor.project.elements[0].shape,
            app.polyline_edit
                .as_ref()
                .map(|edit| (edit.drag.is_some(), edit.error.clone())),
            app.editor.status
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: offset,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.editor.undo.len(),
            1,
            "whole-path offset commits as one operation"
        );
        app.editor.undo();
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(0.,0.) && points[1]==Point::new(2.,0.))
        );

        app.open_polyline_edit(id, Some(1), Some(0), start);
        app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::CurveEdge;
        let arc = pos2(450., 250.);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(arc)]);
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, ref arcs} if points.len()==3 && arcs[0].is_some_and(|arc| (arc.radius-1.).abs()<1e-3))
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: arc,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.editor.undo.len(),
            1,
            "curve conversion commits as one operation"
        );
        app.editor.undo();
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==3)
        );
    }
    #[test]
    fn dragging_selected_polyline_body_moves_whole_object_with_arrow_without_palette() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        let start = pos2(450., 300.);
        let end = pos2(500., 250.);
        click(&ctx, &mut app, start);
        assert_eq!(app.editor.selection_ids(), vec![id]);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        let moving_cursor = plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
        assert!(app.drag.is_some());
        assert!(app.polyline_edit.is_none());
        assert_eq!(
            moving_cursor,
            CursorIcon::Default,
            "moving a selected Polyline keeps the arrow cursor"
        );
        assert!(
            app.polyline_edit.is_none(),
            "a move gesture must not open the edit palette"
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(1.,1.) && points[1]==Point::new(3.,1.) && points[2]==Point::new(3.,3.))
        );
        assert!(app.polyline_edit.is_none());
        assert_eq!(app.editor.undo.len(), 1);
    }
    #[test]
    fn moving_polyline_node_keeps_arrow_cursor_while_middle_pan_uses_hand() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.editor.set_selection(vec![id]);
        let node = pos2(500., 300.);
        let click_cursor = click(&ctx, &mut app, node);
        assert_eq!(
            click_cursor,
            CursorIcon::Default,
            "opening the node palette should not let its controls replace the arrow cursor"
        );

        let away_from_geometry = pos2(650., 450.);
        assert_eq!(
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(away_from_geometry)]
            ),
            CursorIcon::Default,
            "Move node mode should keep the arrow while the pointer moves off the node"
        );
        assert_eq!(
            plan_frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(node),
                    Event::PointerButton {
                        pos: node,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            ),
            CursorIcon::Default,
            "node editing should keep the arrow while the primary button is held"
        );
        let moved_node = node + vec2(30., -30.);
        assert_eq!(
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(moved_node)]),
            CursorIcon::Default,
            "dragging the node should keep the arrow cursor"
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: moved_node,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: node,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(node + vec2(20., 20.))]
            ),
            CursorIcon::Grabbing,
            "middle-button view pan should keep the hand cursor"
        );
    }
    #[test]
    fn select_cursor_uses_arrow_on_body_check_at_nodes_and_triple_ray_on_edges() {
        let (ctx, mut app) = setup();
        app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(4., 0.),
                max: Point::new(6., 2.),
                thickness: 0.2,
            },
        );

        assert_eq!(
            app.select_cursor_feedback(Point::new(5., 1.)),
            CursorFeedback::Arrow
        );
        assert_eq!(
            app.select_cursor_feedback(Point::new(0., 0.)),
            CursorFeedback::ArrowWithCheck
        );
        assert_eq!(
            app.select_cursor_feedback(Point::new(1., 0.)),
            CursorFeedback::ArrowWithTripleRay
        );
        assert_eq!(
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]),
            CursorIcon::Default,
            "the node cursor must use the arrow platform icon"
        );
    }
    #[test]
    fn chamfer_and_fillet_edit_polyline_corners_and_undo() {
        let (_ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        app.open_polyline_edit(id, Some(1), None, pos2(500., 300.));
        app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::Chamfer;
        app.apply_polyline_corner_edit();
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==4)
        );
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        app.open_polyline_edit(id, Some(1), None, pos2(500., 300.));
        app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::Fillet;
        app.apply_polyline_corner_edit();
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, ref arcs} if points.len()==4 && arcs[1].is_some())
        );
        assert!(app.editor.project.validate().is_ok());
    }
    #[test]
    fn polyline_edge_offset_move_rotate_mirror_and_multiply_preview_geometry() {
        let (_ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        let before = app.editor.project.clone();
        let mut edit = PolylineEdit {
            element_id: id,
            node: None,
            edge: Some(0),
            command: PolylineEditCommand::OffsetPolyline,
            palette_at: Pos2::ZERO,
            drag: None,
            chamfer_length: 0.25,
            fillet_radius: 0.25,
            multiply_count: 3,
            error: String::new(),
        };
        app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 0.25));
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(0.,0.25) && points[1]==Point::new(1.75,0.25) && points[2]==Point::new(1.75,2.))
        );
        edit.command = PolylineEditCommand::CurveEdge;
        app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 1.));
        let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
            panic!("curve edit changed element type");
        };
        assert_eq!(
            points,
            &[Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
            "curve stays a single segment with endpoint nodes only"
        );
        assert!(arcs[0].is_some());
        assert!(
            geometry::polyline_path(points, arcs).len() > 18,
            "display tessellation is generated without changing model vertices"
        );
        let first_radius = arcs[0].unwrap().radius;
        app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 0.5));
        let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
            panic!("curve preview changed element type");
        };
        assert_eq!(points.len(), 3, "drag preview never creates arc nodes");
        assert!(
            arcs[0].unwrap().radius > first_radius,
            "moving the pointer updates the circle radius live"
        );
        let closed_id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![
                    Point::new(0., 0.),
                    Point::new(2., 0.),
                    Point::new(2., 2.),
                    Point::new(0., 2.),
                    Point::new(0., 0.),
                ],
                arcs: vec![],
            },
        );
        let closed_before = app.editor.project.clone();
        edit.command = PolylineEditCommand::OffsetPolyline;
        edit.element_id = closed_id;
        app.apply_polyline_drag_preview(
            &closed_before,
            &edit,
            Point::new(1., 0.),
            Point::new(1., 0.25),
        );
        assert!(
            matches!(app.editor.project.elements.iter().find(|element|element.id==closed_id).unwrap().shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(0.25,0.25) && points[1]==Point::new(1.75,0.25) && points[4]==points[0])
        );
        edit.element_id = id;
        edit.command = PolylineEditCommand::Move;
        app.apply_polyline_drag_preview(&before, &edit, Point::default(), Point::new(1., 1.));
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(1.,1.))
        );
        edit.command = PolylineEditCommand::Rotate;
        edit.node = Some(0);
        app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(0., 1.));
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if (points[1].x).abs()<1e-5 && (points[1].y-2.).abs()<1e-5)
        );
        edit.command = PolylineEditCommand::Mirror;
        app.apply_polyline_drag_preview(&before, &edit, Point::default(), Point::new(0., 2.));
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[1]==Point::new(-2.,0.))
        );
        edit.command = PolylineEditCommand::Multiply;
        edit.node = None;
        app.apply_polyline_drag_preview(&before, &edit, Point::default(), Point::new(1., 1.));
        assert_eq!(app.editor.project.elements.len(), 4);
        assert!(
            app.editor
                .project
                .elements
                .windows(2)
                .all(|pair| pair[0].id != pair[1].id)
        );
        assert!(
            matches!(app.editor.project.elements[3].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(3.,3.))
        );
        assert!(app.editor.project.validate().is_ok());
    }
    fn click_model(ctx: &Context, app: &mut AtelierApp, pos: Pos2, button: PointerButton) {
        model_frame(ctx, app, vec![Event::PointerMoved(pos)]);
        model_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        model_frame(
            ctx,
            app,
            vec![Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn collect_text(shapes: &[egui::epaint::ClippedShape], output: &mut Vec<String>) {
        fn walk(shape: &egui::Shape, output: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => output.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, output);
                    }
                }
                _ => {}
            }
        }
        for clipped in shapes {
            walk(&clipped.shape, output);
        }
    }
    fn collect_shapes<'a>(
        shapes: &'a [egui::epaint::ClippedShape],
        output: &mut Vec<&'a egui::Shape>,
    ) {
        fn walk<'a>(shape: &'a egui::Shape, output: &mut Vec<&'a egui::Shape>) {
            output.push(shape);
            if let egui::Shape::Vec(shapes) = shape {
                for shape in shapes {
                    walk(shape, output);
                }
            }
        }
        for clipped in shapes {
            walk(&clipped.shape, output);
        }
    }
    #[test]
    fn selected_wall_draws_the_archicad_style_reference_line_and_handles() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.3,
                height: 3.,
            },
        );
        app.editor.project.elements[0].wall_settings = Some(WallSettings::default());
        app.editor.project.elements[0]
            .wall_settings
            .as_mut()
            .unwrap()
            .geometry
            .reference_offset = 0.25;

        let output = plan_frame_output(&ctx, &mut app, vec![]);
        let mut shapes = Vec::new();
        collect_shapes(&output.shapes, &mut shapes);
        assert!(
            !shapes.iter().any(|shape| matches!(
                shape,
                egui::Shape::LineSegment { stroke, .. } if stroke.color == WALL_REFERENCE_LINE
            )),
            "an unselected wall must not show its reference line"
        );

        app.editor.set_selection(vec![id]);
        let output = plan_frame_output(&ctx, &mut app, vec![]);
        let mut shapes = Vec::new();
        collect_shapes(&output.shapes, &mut shapes);
        assert!(shapes.iter().any(|shape| matches!(
            shape,
            egui::Shape::Path(path) if path.fill == WALL_SELECTION_FILL
        )));
        let (line, stroke) = shapes
            .iter()
            .find_map(|shape| match shape {
                egui::Shape::LineSegment { points, stroke }
                    if stroke.color == WALL_REFERENCE_LINE =>
                {
                    Some((*points, *stroke))
                }
                _ => None,
            })
            .expect("selected wall reference line");
        assert_eq!(stroke.width, WALL_REFERENCE_LINE_WIDTH);
        let handles: Vec<Rect> = shapes
            .iter()
            .filter_map(|shape| match shape {
                egui::Shape::Rect(rect)
                    if rect.fill == Color32::BLACK
                        && (rect.rect.width() - WALL_REFERENCE_HANDLE_SIZE).abs() < 1e-4
                        && (rect.rect.height() - WALL_REFERENCE_HANDLE_SIZE).abs() < 1e-4 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(handles.len(), 2);
        for endpoint in line {
            assert!(
                handles
                    .iter()
                    .any(|handle| handle.center().distance(endpoint) < 1e-4),
                "the blue line must end in the black square handles"
            );
        }
    }
    #[test]
    fn pointer_wall_creation_selection_drag_undo_redo() {
        let (ctx, mut app) = setup();
        plan_frame(&ctx, &mut app, vec![key(Key::W, Modifiers::NONE)]);
        click(&ctx, &mut app, pos2(400., 300.));
        assert_eq!(app.editor.anchor, Some(Point::new(0., 0.)));
        click(&ctx, &mut app, pos2(600., 300.));
        assert_eq!(app.editor.project.elements.len(), 1);
        assert_eq!(app.editor.project.elements[0].area(), 12.);
        plan_frame(&ctx, &mut app, vec![key(Key::V, Modifiers::NONE)]);
        click(&ctx, &mut app, pos2(500., 300.));
        assert_eq!(app.editor.selected, Some(1));
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(500., 300.)),
                Event::PointerButton {
                    pos: pos2(500., 300.),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(550., 250.))]);
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: pos2(550., 250.),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let wall = &app.editor.project.elements[0];
        assert!(matches!(wall.shape,Shape::Wall{start,..} if start==Point::new(1.,1.)));
        assert_eq!(app.editor.undo.len(), 2, "one drag is one transaction");
        plan_frame(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Wall{start,..} if start==Point::default())
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
        );
        assert!(
            matches!(app.editor.project.elements[0].shape,Shape::Wall{start,..} if start==Point::new(1.,1.))
        );
    }
    #[test]
    fn pointer_slab_creation_and_cancel() {
        let (ctx, mut app) = setup();
        plan_frame(&ctx, &mut app, vec![key(Key::L, Modifiers::NONE)]);
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(600., 150.));
        assert_eq!(app.editor.project.elements[0].area(), 12.);
        click(&ctx, &mut app, pos2(350., 350.));
        assert!(app.editor.anchor.is_some());
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.editor.anchor.is_none());
        assert_eq!(app.editor.project.elements.len(), 1);
    }
    #[test]
    fn pointer_polyline_draw_commit_json_undo_and_redo() {
        let (ctx, mut app) = setup();
        plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
        assert_eq!(app.editor.tool, Tool::Polyline);
        click(&ctx, &mut app, pos2(360., 280.));
        click(&ctx, &mut app, pos2(470., 250.));
        click(&ctx, &mut app, pos2(580., 320.));
        assert_eq!(
            app.editor.project.elements.len(),
            0,
            "nodes stay a preview until commit"
        );
        assert_eq!(app.editor.polyline_points.len(), 3);
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert_eq!(app.editor.project.elements.len(), 1);
        assert_eq!(app.editor.selected, Some(1));
        let Shape::Polyline { points, .. } = &app.editor.project.elements[0].shape else {
            panic!("Polyline tool created a different shape");
        };
        assert_eq!(points.len(), 3);
        assert!(
            points
                .windows(2)
                .all(|pair| pair[0].distance(pair[1]) >= 0.01)
        );
        assert!(app.editor.project.validate().is_ok());
        let json = serde_json::to_string(&app.editor.project).unwrap();
        assert_eq!(
            serde_json::from_str::<Project>(&json).unwrap(),
            app.editor.project
        );
        app.editor.undo();
        assert!(app.editor.project.elements.is_empty());
        app.editor.redo();
        assert!(
            matches!(app.editor.project.elements[0].shape, Shape::Polyline { ref points , .. } if points.len() == 3)
        );
    }
    #[test]
    fn pointer_polyline_escape_discards_draft_without_model_change() {
        let (ctx, mut app) = setup();
        let before = app.editor.project.clone();
        plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
        click(&ctx, &mut app, pos2(360., 280.));
        assert_eq!(app.editor.polyline_points.len(), 1);
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert_eq!(app.editor.tool, Tool::Select);
        assert!(app.editor.polyline_points.is_empty());
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn escape_cancels_current_command_returns_to_previous_then_select() {
        let (ctx, mut app) = setup();
        plan_frame(&ctx, &mut app, vec![key(Key::W, Modifiers::NONE)]);
        plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
        app.editor.polyline_points = vec![Point::new(1., 1.), Point::new(2., 1.)];

        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert_eq!(
            app.editor.tool,
            Tool::Wall,
            "first Escape restores the previous command"
        );
        assert!(
            app.editor.polyline_points.is_empty(),
            "first Escape cancels the active drawing"
        );

        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert_eq!(
            app.editor.tool,
            Tool::Select,
            "second Escape returns to Select"
        );
    }
    #[test]
    fn pointer_stair_creation_persists_parametric_settings_and_undoes() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Stair);
        app.editor.stair_spec.width = 1.25;
        app.editor.stair_spec.total_rise = 3.4;
        app.editor.stair_spec.risers = 20;
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(640., 300.));
        assert_eq!(app.editor.project.elements.len(), 1);
        let element = &app.editor.project.elements[0];
        let Shape::Stair { spec } = &element.shape else {
            panic!("stair tool created another kind")
        };
        assert_eq!(spec.width, 1.25);
        assert_eq!(spec.risers, 20);
        let geometry =
            crate::stairs::generate(spec, app.editor.project.elevation(element.story)).unwrap();
        assert_eq!(geometry.tread_lines.len(), 20);
        assert!(app.editor.project.validate().is_ok());
        let json = serde_json::to_string(&app.editor.project).unwrap();
        let restored: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, app.editor.project);
        app.editor.undo();
        assert!(app.editor.project.elements.is_empty());
        app.editor.redo();
        assert_eq!(app.editor.project.elements.len(), 1);
        let id = app.editor.project.elements[0].id;
        app.editor.set_selection(vec![id]);
        app.open_element_settings();
        if let Some(Element {
            shape: Shape::Stair { spec },
            ..
        }) = app.element_settings.as_mut()
        {
            spec.width = 1.4;
        }
        app.apply_element_settings().unwrap();
        assert!(
            matches!(app.editor.project.elements[0].shape, Shape::Stair { ref spec } if spec.width == 1.4)
        );
        app.editor.undo();
        assert!(
            matches!(app.editor.project.elements[0].shape, Shape::Stair { ref spec } if spec.width == 1.25)
        );
    }
    #[test]
    fn command_t_opens_selected_wall_settings_or_wall_defaults() {
        let (_ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.click(Point::new(0., 0.), 0.);
        app.editor.click(Point::new(3., 0.), 0.);
        let id = app.editor.project.elements[0].id;
        app.editor.set_selection(vec![id]);
        app.open_element_settings();
        assert!(app.element_settings.is_none());
        let selected = app.wall_settings_dialog.as_ref().unwrap();
        assert_eq!(
            selected.mode,
            crate::wall_ui::WallSettingsMode::Selected(id)
        );
        app.wall_settings_dialog = None;
        app.editor.set_selection(Vec::new());
        app.editor.set_tool(Tool::Wall);
        app.open_element_settings();
        assert_eq!(
            app.wall_settings_dialog.as_ref().unwrap().mode,
            crate::wall_ui::WallSettingsMode::Default
        );
    }
    #[test]
    fn wall_settings_dialog_renders_without_mutating_the_project() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.click(Point::new(0., 0.), 0.);
        app.editor.click(Point::new(3., 0.), 0.);
        let id = app.editor.project.elements[0].id;
        app.editor.set_selection(vec![id]);
        app.open_element_settings();
        let before = app.editor.project.clone();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900., 700.))),
                ..Default::default()
            },
            |ui| app.wall_settings_dialog(ui.ctx()),
        );
        output.textures_delta.clear();
        assert!(app.wall_settings_dialog.is_some());
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn stair_settings_dialog_renders_without_mutating_the_project() {
        let (ctx, mut app) = setup();
        app.stair_settings_open = true;
        let before = app.editor.project.clone();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                ..Default::default()
            },
            |ui| app.stair_settings_dialog(ui.ctx()),
        );
        output.textures_delta.clear();
        assert!(app.stair_settings_open);
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn stair_shortcut_opens_settings_and_activates_the_real_tool() {
        let (ctx, mut app) = setup();
        let before = app.editor.project.clone();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events: vec![key(Key::S, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.editor.tool, Tool::Stair);
        assert!(app.stair_settings_open);
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn marquee_rectangle_is_created_by_plan_canvas_drag() {
        let (ctx, mut app) = setup();
        plan_frame(&ctx, &mut app, vec![key(Key::M, Modifiers::NONE)]);
        let a = pos2(340., 260.);
        let b = pos2(460., 340.);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(a),
                Event::PointerButton {
                    pos: a,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: b,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let area = app.editor.marquee.as_ref().expect("drag places a marquee");
        assert_eq!(area.vertices.len(), 4);
        assert!(area.z_range.is_none());
    }
    #[test]
    fn polygon_marquee_uses_right_click_to_close_and_enter_does_not_close() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Marquee);
        app.marquee_geometry = MarqueeGeometry::Polygon;
        click(&ctx, &mut app, pos2(330., 270.));
        click(&ctx, &mut app, pos2(470., 270.));
        click(&ctx, &mut app, pos2(440., 370.));
        assert_eq!(
            app.marquee_points.len(),
            3,
            "points before Enter: {:?}",
            app.marquee_points
        );
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(
            app.editor.marquee.is_none(),
            "Enter is reserved for numeric tracker input"
        );
        let pos = pos2(440., 370.);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Secondary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.editor.marquee.as_ref().map(|area| area.vertices.len()),
            Some(3),
            "right-click close failed: points={:?}, status={:?}",
            app.marquee_points,
            app.editor.status
        );
    }
    #[test]
    fn three_dimensional_polygon_marquee_previews_and_closes_on_right_click() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Marquee);
        app.marquee_geometry = MarqueeGeometry::Polygon;
        let points = [pos2(250., 450.), pos2(430., 450.), pos2(410., 500.)];
        click_model(&ctx, &mut app, points[0], PointerButton::Primary);
        assert_eq!(
            app.marquee_3d_points.len(),
            1,
            "first screen point not registered: {:?}",
            app.editor.status
        );
        click_model(&ctx, &mut app, points[1], PointerButton::Primary);
        model_frame(&ctx, &mut app, vec![Event::PointerMoved(points[2])]);
        assert_eq!(
            app.marquee_3d_points.len(),
            2,
            "ground points={:?}, status={:?}",
            app.marquee_points,
            app.editor.status
        );
        assert!(
            app.marquee_3d_cursor.is_some(),
            "the open polygon has a live cursor segment"
        );
        click_model(&ctx, &mut app, points[2], PointerButton::Primary);
        assert!(
            app.editor.marquee.is_none(),
            "three vertices only define the base until right-click"
        );
        click_model(&ctx, &mut app, points[2], PointerButton::Secondary);
        let area = app
            .editor
            .marquee
            .as_ref()
            .expect("right click closes the 3D polygon");
        assert_eq!(area.vertices.len(), 3);
        assert_eq!(area.z_range, Some((0., app.marquee_height)));
        assert!(app.show_marquee_3d);
        assert!(app.marquee_3d_points.is_empty());
    }
    #[test]
    fn three_dimensional_rotated_rectangle_uses_three_ground_plane_points() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Marquee);
        app.marquee_geometry = MarqueeGeometry::RotatedRectangle;
        for pos in [pos2(250., 440.), pos2(430., 430.), pos2(420., 500.)] {
            click_model(&ctx, &mut app, pos, PointerButton::Primary);
        }
        let area = app.editor.marquee.as_ref().unwrap_or_else(|| {
            panic!(
                "third point failed: points={:?}, status={:?}",
                app.marquee_points, app.editor.status
            )
        });
        assert_eq!(area.vertices.len(), 4);
        assert_eq!(area.z_range, Some((0., app.marquee_height)));
        assert!(app.show_marquee_3d);
    }
    #[test]
    fn default_is_single_plan_window_and_f_keys_switch_3d_workflow() {
        let (ctx, mut app) = setup();
        assert_eq!(app.view, View::Plan);
        let selected_id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(3., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.editor.set_selection(vec![selected_id]);
        app.editor.marquee = MarqueeArea::new(
            vec![
                Point::new(0., 0.),
                Point::new(2., 0.),
                Point::new(2., 1.),
                Point::new(0., 1.),
            ],
            MarqueeScope::CurrentStory,
            0,
        );
        plan_frame(&ctx, &mut app, vec![key(Key::F5, Modifiers::NONE)]);
        assert_eq!(app.view, View::Model);
        assert!(app.show_marquee_3d);
        assert!(app.editor.status.contains("clipped Marquee"));
        assert_eq!(app.editor.selection_ids(), vec![selected_id]);
        plan_frame(&ctx, &mut app, vec![key(Key::F2, Modifiers::NONE)]);
        assert_eq!(app.view, View::Plan);
        app.editor.marquee = None;
        plan_frame(&ctx, &mut app, vec![key(Key::F5, Modifiers::NONE)]);
        assert!(app.editor.status.contains("selected elements"));
        plan_frame(&ctx, &mut app, vec![key(Key::F3, Modifiers::NONE)]);
        assert_eq!(app.view, View::Model);
    }
    #[test]
    fn command_t_opens_independent_element_settings_and_applies_edits() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(4., 3.),
                thickness: 0.2,
            },
        );
        app.editor.set_selection(vec![id]);
        app.tab = Tab::Library;
        app.show_right = false;
        plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
        assert!(matches!(app.tab, Tab::Library));
        assert!(
            !app.show_right,
            "the settings dialog is independent of the inspector"
        );
        assert_eq!(
            app.element_settings.as_ref().map(|element| element.id),
            Some(id)
        );

        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            app.element_settings_dialog(ui.ctx());
        });
        output.textures_delta.clear();
        if let Some(Shape::Slab { thickness, .. }) = app
            .element_settings
            .as_mut()
            .map(|element| &mut element.shape)
        {
            *thickness = 0.35;
        }
        app.apply_element_settings().unwrap();
        assert!(
            matches!(app.editor.project.elements[0].shape, Shape::Slab { thickness, .. } if thickness == 0.35)
        );
        app.editor.undo();
        assert!(
            matches!(app.editor.project.elements[0].shape, Shape::Slab { thickness, .. } if thickness == 0.2)
        );
    }
    #[test]
    fn shift_middle_drag_orbits_3d_while_plain_middle_drag_pans() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Marquee);
        let yaw = app.camera.yaw;
        let pitch = app.camera.pitch;
        let target = app.camera.target;
        let start = pos2(400., 300.);
        let end = pos2(450., 325.);
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        model_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        model_frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(shift),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Middle,
                    pressed: true,
                    modifiers: shift,
                },
            ],
        );
        model_frame(
            &ctx,
            &mut app,
            vec![Event::ModifiersChanged(shift), Event::PointerMoved(end)],
        );
        assert!(ctx.input(|i| i.pointer.button_down(PointerButton::Middle)));
        assert_ne!(ctx.input(|i| i.pointer.delta()), Vec2::ZERO);
        model_frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(shift),
                Event::PointerButton {
                    pos: end,
                    button: PointerButton::Middle,
                    pressed: false,
                    modifiers: shift,
                },
                Event::ModifiersChanged(Modifiers::NONE),
            ],
        );
        assert_ne!(app.camera.yaw, yaw);
        assert_ne!(app.camera.pitch, pitch);
        assert_eq!(app.camera.target, target);

        let yaw = app.camera.yaw;
        let target = app.camera.target;
        let pan_start = pos2(300., 200.);
        let pan_end = pos2(360., 230.);
        model_frame(&ctx, &mut app, vec![Event::PointerMoved(pan_start)]);
        model_frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(Modifiers::NONE),
                Event::PointerButton {
                    pos: pan_start,
                    button: PointerButton::Middle,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        model_frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(Modifiers::NONE),
                Event::PointerMoved(pan_end),
            ],
        );
        model_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: pan_end,
                button: PointerButton::Middle,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.camera.yaw, yaw);
        assert_ne!(app.camera.target, target);
    }
    #[test]
    fn tracker_enter_creates_exact_wall() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.anchor = Some(Point::new(1., 2.));
        app.tracker_a = 5.25;
        app.tracker_b = 90.;
        app.tracker_locked = true;
        let mut output = ctx.run_ui(
            RawInput {
                events: vec![key(Key::Enter, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.tracker(ui),
        );
        output.textures_delta.clear();
        assert_eq!(app.editor.project.elements.len(), 1);
        if let Shape::Wall { start, end, .. } = app.editor.project.elements[0].shape {
            assert!((start.distance(end) - 5.25).abs() < 1e-5);
            assert!((end.y - 7.25).abs() < 1e-5);
        } else {
            panic!("Expected wall")
        }
    }
    #[test]
    fn workspace_all_views_and_tabs_render_at_minimum_window() {
        let (ctx, mut app) = setup();
        for view in [View::Plan, View::Split, View::Model] {
            for tab in [Tab::Properties, Tab::Library, Tab::Assistant] {
                for navigator_view in [
                    NavigatorView::ProjectMap,
                    NavigatorView::ViewMap,
                    NavigatorView::LayoutBook,
                    NavigatorView::Publisher,
                ] {
                    app.view = view;
                    app.tab = tab;
                    app.navigator_view = navigator_view;
                    let mut out = ctx.run_ui(
                        RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1050., 700.))),
                            ..Default::default()
                        },
                        |ui| app.workspace(ui),
                    );
                    out.textures_delta.clear();
                    assert!(!out.shapes.is_empty());
                }
            }
        }
    }
    #[test]
    fn simplified_chinese_workspace_renders_without_changing_model_state() {
        let (ctx, mut app) = setup();
        app.language = UiLanguage::SimplifiedChinese;
        let before = app.editor.project.elements.clone();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1050., 700.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
        let chinese_glyphs_present = ctx.fonts_mut(|fonts| {
            let font = FontId::proportional(14.);
            fonts.has_glyphs(&font, "墙体楼层项目")
        });
        assert!(
            chinese_glyphs_present,
            "the UI font fallback must contain Simplified Chinese glyphs"
        );
        assert_eq!(app.editor.project.elements, before);
    }
    #[test]
    fn quick_options_bar_and_palette_render_in_the_selected_language() {
        let (ctx, mut app) = setup();
        app.quick_options_palette_visible = true;
        let before = app.editor.project.clone();

        app.language = UiLanguage::English;
        let mut first = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        first.textures_delta.clear();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut english = Vec::new();
        collect_text(&output.shapes, &mut english);
        output.textures_delta.clear();
        assert!(
            english
                .iter()
                .any(|text| text.contains("Layer Combination")),
            "English palette labels missing: {english:?}"
        );
        assert!(english.iter().any(|text| text.contains("02 Drafting")));

        app.language = UiLanguage::SimplifiedChinese;
        let mut first = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        first.textures_delta.clear();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut chinese = Vec::new();
        collect_text(&output.shapes, &mut chinese);
        output.textures_delta.clear();
        assert!(
            chinese.iter().any(|text| text.contains("图层组合")),
            "Simplified Chinese palette labels missing: {chinese:?}"
        );
        assert!(chinese.iter().any(|text| text.contains("02 绘图")));
        assert!(
            !chinese
                .iter()
                .any(|text| text.contains("Layer Combination")),
            "English Quick Options labels remain after switching to Chinese: {chinese:?}"
        );
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn main_window_status_tabs_and_counts_follow_the_selected_language() {
        let (ctx, mut app) = setup();
        app.editor.status = "Selected 2 element(s) in Marquee".into();
        let before = app.editor.project.clone();

        app.language = UiLanguage::SimplifiedChinese;
        for pass in 0..2 {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                    ..Default::default()
                },
                |ui| app.workspace(ui),
            );
            output.textures_delta.clear();
            if pass == 1 {
                let mut chinese = Vec::new();
                collect_text(&output.shapes, &mut chinese);
                for expected in [
                    "已选 0 个",
                    "0 个构件",
                    "分屏",
                    "南立面",
                    "在选区框中选择 2 个构件",
                ] {
                    assert!(
                        chinese.iter().any(|text| text.contains(expected)),
                        "missing Chinese main-window label {expected:?}: {chinese:?}"
                    );
                }
                for english in [
                    "0 selected",
                    "Split",
                    "South Elevation",
                    "Metres · Z up",
                    "Selected 2 element(s) in Marquee",
                ] {
                    assert!(
                        !chinese.iter().any(|text| text.contains(english)),
                        "English main-window text {english:?} remains in Chinese mode: {chinese:?}"
                    );
                }
            }
        }

        app.language = UiLanguage::English;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut english = Vec::new();
        collect_text(&output.shapes, &mut english);
        output.textures_delta.clear();
        for expected in [
            "0 selected",
            "0 elements",
            "Split",
            "South Elevation",
            "Selected 2 element(s) in Marquee",
        ] {
            assert!(
                english.iter().any(|text| text.contains(expected)),
                "missing English main-window label {expected:?}: {english:?}"
            );
        }
        assert_eq!(
            app.editor.status, "Selected 2 element(s) in Marquee",
            "switching language must not rewrite model/editor state"
        );
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn status_text_localizes_dynamic_messages_without_rewriting_editor_state() {
        let (_ctx, mut app) = setup();
        app.language = UiLanguage::SimplifiedChinese;
        for (status, expected) in [
            (
                "Selected 2 element(s) in Marquee",
                "在选区框中选择 2 个构件",
            ),
            ("Copied 3 element(s)", "已复制 3 个构件"),
            ("Cut 1 element(s)", "已剪切 1 个构件"),
            ("Pasted 4 element(s)", "已粘贴 4 个构件"),
            ("Created element 013", "已创建构件 013"),
            ("Element Settings · Wall 007", "构件设置 · 墙 007"),
            ("Project saved", "项目已保存"),
            ("Navigator item · Ground Floor", "导航器项目 · 首层"),
            ("View Map · South Elevation", "视图地图 · 南立面"),
            (
                "Wall tool selected · interface preview",
                "墙 · 工具已选择 · 界面预览",
            ),
        ] {
            app.editor.status = status.into();
            assert_eq!(app.status_text(), expected, "status {status:?}");
            assert_eq!(
                app.editor.status, status,
                "editor state must stay canonical"
            );
        }
    }
    #[test]
    fn command_preview_dialog_renders_without_mutating_the_project() {
        let (ctx, mut app) = setup();
        let initial = app.editor.project.clone();
        app.command_dialog = Some("Wall Settings".into());
        app.command_dialog_name = "Wall Settings".into();
        for tab in 0..4 {
            app.command_dialog_tab = tab;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100., 760.))),
                    ..Default::default()
                },
                |ui| app.workspace(ui),
            );
            output.textures_delta.clear();
            assert!(!output.shapes.is_empty());
        }
        assert_eq!(app.editor.project, initial);
    }
    #[test]
    fn trace_toggle_opens_plan_and_resolves_an_available_story() {
        let (_ctx, mut app) = setup();
        app.view = View::Model;
        app.trace.enabled = true;
        app.trace_toggled();
        assert_eq!(app.view, View::Plan);
        assert_eq!(app.trace.reference, crate::trace::TraceReference::Story(1));
        assert_eq!(
            app.trace.selection(
                &app.editor.project,
                app.editor.story,
                app.editor.previous_story
            ),
            Some(crate::trace::TraceSelection::Story(1))
        );
    }
    #[test]
    fn trace_switch_reference_preserves_model_and_undo_history() {
        let (_ctx, mut app) = setup();
        app.editor.switch_story(1);
        app.trace.enabled = true;
        app.trace.reference = crate::trace::TraceReference::AboveCurrentStory;
        let initial = app.editor.project.clone();
        let undo_len = app.editor.undo.len();
        app.switch_trace_reference();
        assert_eq!(app.editor.story, 2);
        assert_eq!(app.trace.reference, crate::trace::TraceReference::Story(1));
        assert_eq!(app.editor.project, initial);
        assert_eq!(app.editor.undo.len(), undo_len);
        assert!(!app.editor.dirty());
    }
    #[test]
    fn trace_drag_moves_only_the_reference() {
        let (ctx, mut app) = setup();
        app.editor.switch_story(1);
        app.trace.enabled = true;
        app.trace.reference = crate::trace::TraceReference::BelowCurrentStory;
        let before = app.editor.project.clone();
        let offset = app.trace.offset;
        app.trace.drag_mode = true;
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(400., 300.)),
                Event::PointerButton {
                    pos: pos2(400., 300.),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 320.))]);
        assert_ne!(app.trace.offset, offset);
        assert!((app.trace.offset.x - 1.).abs() < 1e-5);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        assert!(!app.editor.dirty());
    }
    #[test]
    fn escape_cancels_trace_pointer_mode_and_restores_temporary_displacement() {
        let (ctx, mut app) = setup();
        app.trace.enabled = true;
        app.trace.displace_mode = true;
        app.trace.displace_origin = Some(Point::new(1., 2.));
        app.trace.offset = Point::new(5., 6.);
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(!app.trace.pointer_mode_active());
        assert_eq!(app.trace.offset, Point::new(1., 2.));
    }
    #[test]
    fn trace_overlay_palette_and_splitter_render_without_mutating_the_model() {
        let (ctx, mut app) = setup();
        app.editor.switch_story(1);
        app.trace.enabled = true;
        app.trace.palette_open = true;
        app.trace.browse_open = true;
        app.trace.splitter = Some(0.5);
        app.trace.on_top = true;
        let initial = app.editor.project.clone();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100., 760.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        output.textures_delta.clear();
        assert!(!output.shapes.is_empty());
        assert_eq!(app.editor.project, initial);
    }
    fn quick_option_element(layer: Layer, design_option: Option<u32>) -> Element {
        Element {
            id: 1,
            name: "Wall 001".into(),
            story: 0,
            material: Material::Concrete,
            layer,
            renovation: RenovationStatus::Existing,
            design_option,
            wall_settings: None,
            shape: Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        }
    }

    #[test]
    fn quick_options_edit_plan_options_through_undoable_commands() {
        let (_ctx, mut app) = setup();
        assert_eq!(
            app.editor.project.plan_options.layer_combination,
            LayerCombination::Drafting
        );
        assert_eq!(
            app.editor.project.plan_options.pen_set,
            PenSet::Architectural100
        );
        assert_eq!(
            app.editor.project.plan_options.partial_structure,
            PartialStructureDisplay::EntireModel
        );

        app.set_layer_combination(LayerCombination::PlansDetailed);
        app.set_pen_set(PenSet::ColorConceptual);
        app.set_partial_structure(PartialStructureDisplay::WithoutFinishes);
        app.set_model_view(ModelViewOptions::CeilingPlans);
        app.set_graphic_overrides(GraphicOverrides::FirePlan);
        app.set_renovation_filter(RenovationFilter::NewConstruction);
        app.set_dimension_style(DimensionStyle::PlainMillimeter);
        assert_eq!(
            app.editor.project.plan_options.layer_combination,
            LayerCombination::PlansDetailed
        );
        assert_eq!(
            app.editor.project.plan_options.pen_set,
            PenSet::ColorConceptual
        );
        assert_eq!(
            app.editor.project.plan_options.partial_structure,
            PartialStructureDisplay::WithoutFinishes
        );
        assert_eq!(
            app.editor.project.plan_options.model_view,
            ModelViewOptions::CeilingPlans
        );
        assert_eq!(
            app.editor.project.plan_options.graphic_overrides,
            GraphicOverrides::FirePlan
        );
        assert_eq!(
            app.editor.project.plan_options.renovation_filter,
            RenovationFilter::NewConstruction
        );
        assert_eq!(
            app.editor.project.plan_options.dimension_style,
            DimensionStyle::PlainMillimeter
        );
        assert_eq!(app.editor.undo.len(), 7);

        for _ in 0..7 {
            app.editor.undo();
        }
        assert_eq!(app.editor.project.plan_options, PlanOptions::default());
    }

    #[test]
    fn quick_option_visibility_follows_layers_and_design_options() {
        let (_ctx, mut app) = setup();
        let main = quick_option_element(Layer::Structural, None);
        let mut option_element = main.clone();
        option_element.id = 2;
        let option_id = app.editor.project.design_options.create("Option A".into());
        option_element.design_option = Some(option_id);
        app.editor.project.elements = vec![main.clone(), option_element.clone()];

        app.set_layer_combination(LayerCombination::Layouting);
        assert!(!app.editor.project.plan_options.element_visible(&main));
        assert!(
            !app.editor
                .project
                .plan_options
                .element_visible(&option_element)
        );
        app.set_show_all_layers(true);
        assert!(app.editor.project.plan_options.element_visible(&main));

        app.set_active_design_option(None);
        assert!(app.editor.project.plan_options.element_visible(&main));
        assert!(
            !app.editor
                .project
                .plan_options
                .element_visible(&option_element)
        );
        app.set_active_design_option(Some(option_id));
        assert!(
            app.editor
                .project
                .plan_options
                .element_visible(&option_element)
        );
    }

    #[test]
    fn quick_options_bar_and_palette_visibility_are_independent() {
        let (_ctx, mut app) = setup();
        assert!(app.quick_options_visible);
        assert!(!app.quick_options_palette_visible);
        let before = app.editor.project.clone();
        let undo_len = app.editor.undo.len();

        app.toggle_quick_options_palette();
        assert!(app.quick_options_visible);
        assert!(app.quick_options_palette_visible);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo.len(), undo_len);

        app.toggle_quick_options_bar();
        assert!(!app.quick_options_visible);
        assert!(app.quick_options_palette_visible);
        assert_eq!(app.editor.project, before);

        app.toggle_quick_options_palette();
        assert!(!app.quick_options_palette_visible);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo.len(), undo_len);
    }

    #[test]
    fn plan_orientation_round_trips_between_world_and_screen() {
        let (_ctx, mut app) = setup();
        app.set_orientation_degrees(37.5);
        assert!((app.editor.project.plan_options.normalized_orientation() - 37.5).abs() < 1e-5);
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        for point in [
            Point::new(0., 0.),
            Point::new(3., -2.),
            Point::new(-1.5, 4.),
        ] {
            let screen = app.screen(rect, point);
            let back = app.world(rect, screen);
            assert!((back.x - point.x).abs() < 1e-4, "{back:?} != {point:?}");
            assert!((back.y - point.y).abs() < 1e-4, "{back:?} != {point:?}");
        }
    }
    #[test]
    fn quick_options_zoom_and_bar_visibility_round_trip() {
        let (_ctx, mut app) = setup();
        assert!(app.quick_options_visible);
        app.set_plan_zoom_percent(150.);
        assert!((app.plan_scale - 60.).abs() < 1e-5);
        assert_eq!(app.editor.status, "Zoom · 150.0%");

        app.toggle_quick_options_bar();
        assert!(!app.quick_options_visible);
        assert_eq!(app.editor.status, "Quick Options bar hidden");
        app.toggle_quick_options_bar();
        assert!(app.quick_options_visible);
        assert_eq!(app.editor.status, "Quick Options bar shown");
    }

    fn add_info_box_wall(app: &mut AtelierApp, id_offset: f32) -> u64 {
        app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(id_offset, 0.),
                end: Point::new(id_offset + 4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        )
    }

    fn info_box_wall(app: &AtelierApp, id: u64) -> &Element {
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == id)
            .expect("wall exists")
    }

    #[test]
    fn info_box_tracks_single_wall_selection() {
        let (_ctx, mut app) = setup();
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();
        let state = app.info_box_wall.as_ref().expect("wall state");
        assert_eq!(state.element_id, id);
        assert!(!state.dirty);
        assert_eq!(state.draft.thickness, 0.2);
        assert_eq!(state.draft.height, 3.);

        app.editor.set_selection(Vec::new());
        app.sync_info_box_wall();
        assert!(app.info_box_wall.is_none());
    }

    #[test]
    fn info_box_renders_wall_fields_and_follows_palette_visibility() {
        let (ctx, mut app) = setup();
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        output.textures_delta.clear();
        for expected in [
            "INFO BOX",
            "All Selected: 1",
            "Structural:",
            "Reference Line:",
            "Thickness",
            "Height",
            "Floor Plan and Section…",
            "Apply",
        ] {
            assert!(
                texts.iter().any(|text| text == expected),
                "missing Info Box text {expected:?}: {texts:?}"
            );
        }

        app.info_box_visible = false;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        output.textures_delta.clear();
        assert!(!texts.iter().any(|text| text == "All Selected: 1"));
        assert!(!texts.iter().any(|text| text == "INFO BOX"));
    }

    #[test]
    fn info_box_wall_fields_follow_simplified_chinese() {
        let (ctx, mut app) = setup();
        app.language = UiLanguage::SimplifiedChinese;
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        output.textures_delta.clear();
        for expected in ["信息框", "结构功能：", "厚度", "平面图与剖面…"] {
            assert!(
                texts.iter().any(|text| text == expected),
                "missing localized Info Box text {expected:?}: {texts:?}"
            );
        }
        assert!(
            texts.iter().any(|text| text.starts_with("全部已选: 1")),
            "missing localized selection count: {texts:?}"
        );
    }

    #[test]
    fn info_box_commit_updates_plan_model_and_single_undo_restores() {
        let (_ctx, mut app) = setup();
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();
        {
            let state = app.info_box_wall.as_mut().expect("wall state");
            state.draft.thickness = 0.35;
            state.draft.height = 4.25;
            state.draft.settings.geometry.reference_line = crate::wall::WallReferenceLine::Outside;
            state.draft.settings.geometry.reference_offset = 0.12;
            state.dirty = true;
        }
        assert!(app.commit_info_box_wall());
        let wall = info_box_wall(&app, id);
        assert!(matches!(
            wall.shape,
            Shape::Wall {
                thickness: 0.35,
                height: 4.25,
                ..
            }
        ));
        let settings = wall.wall_settings.as_ref().expect("wall settings");
        assert_eq!(
            settings.geometry.reference_line,
            crate::wall::WallReferenceLine::Outside
        );
        assert_eq!(settings.geometry.reference_offset, 0.12);
        assert_eq!(app.editor.undo.len(), 1);

        app.editor.undo();
        let wall = info_box_wall(&app, id);
        assert!(matches!(
            wall.shape,
            Shape::Wall {
                thickness: 0.2,
                height: 3.,
                ..
            }
        ));
        assert_eq!(
            wall.wall_settings
                .as_ref()
                .expect("wall settings")
                .geometry
                .reference_line,
            crate::wall::WallReferenceLine::Center
        );
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();
        assert_eq!(
            app.info_box_wall.as_ref().expect("wall state").draft.height,
            3.
        );
    }

    #[test]
    fn info_box_invalid_thickness_preserves_model_and_reports_error() {
        let (_ctx, mut app) = setup();
        app.language = UiLanguage::SimplifiedChinese;
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();
        {
            let state = app.info_box_wall.as_mut().expect("wall state");
            state.draft.thickness = 0.001;
            state.dirty = true;
        }
        assert!(!app.commit_info_box_wall());
        assert!(matches!(
            info_box_wall(&app, id).shape,
            Shape::Wall { thickness: 0.2, .. }
        ));
        let state = app.info_box_wall.as_ref().expect("wall state");
        assert!(state.dirty);
        assert!(state.error.contains("墙厚"));
        assert!(app.editor.undo.is_empty());
    }

    #[test]
    fn info_box_selection_change_commits_pending_wall() {
        let (_ctx, mut app) = setup();
        let first = add_info_box_wall(&mut app, 0.);
        let second = add_info_box_wall(&mut app, 6.);
        app.editor.set_selection(vec![first]);
        app.sync_info_box_wall();
        {
            let state = app.info_box_wall.as_mut().expect("wall state");
            state.draft.thickness = 0.31;
            state.dirty = true;
        }
        app.editor.set_selection(vec![second]);
        app.sync_info_box_wall();
        assert!(matches!(
            info_box_wall(&app, first).shape,
            Shape::Wall {
                thickness: 0.31,
                ..
            }
        ));
        let state = app.info_box_wall.as_ref().expect("wall state");
        assert_eq!(state.element_id, second);
        assert!(!state.dirty);
        assert_eq!(state.draft.thickness, 0.2);
    }

    #[test]
    fn info_box_hide_commits_pending_value() {
        let (_ctx, mut app) = setup();
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();
        {
            let state = app.info_box_wall.as_mut().expect("wall state");
            state.draft.height = 3.6;
            state.dirty = true;
        }
        app.toggle_info_box_visibility();
        assert!(!app.info_box_visible);
        assert!(matches!(
            info_box_wall(&app, id).shape,
            Shape::Wall { height: 3.6, .. }
        ));
    }

    #[test]
    fn command_t_commits_pending_info_box_edit_before_wall_settings() {
        let (ctx, mut app) = setup();
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();
        {
            let state = app.info_box_wall.as_mut().expect("wall state");
            state.draft.thickness = 0.42;
            state.dirty = true;
        }
        plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
        assert!(app.wall_settings_dialog.is_some());
        assert!(app.info_box_wall.is_none());
        assert!(matches!(
            info_box_wall(&app, id).shape,
            Shape::Wall {
                thickness: 0.42,
                ..
            }
        ));
    }

    #[test]
    fn info_box_floor_plan_and_settings_buttons_open_wall_settings() {
        let (_ctx, mut app) = setup();
        let id = add_info_box_wall(&mut app, 0.);
        app.editor.set_selection(vec![id]);
        app.sync_info_box_wall();

        app.open_info_box_wall_settings(true);
        let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
        assert!(dialog.sections.floor_plan);
        assert_eq!(dialog.draft.name, "Wall 001");

        app.wall_settings_dialog = None;
        app.open_info_box_wall_settings(false);
        let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
        assert!(!dialog.sections.floor_plan);
        assert!(dialog.sections.geometry);
    }

    #[test]
    fn wall_tool_info_box_shows_creation_defaults_in_chinese() {
        let (ctx, mut app) = setup();
        app.language = UiLanguage::SimplifiedChinese;
        app.editor.set_tool(Tool::Wall);
        app.sync_info_box_wall_tool();
        let state = app.info_box_wall_tool.as_ref().expect("wall tool defaults");
        assert_eq!(state.draft.thickness, app.editor.thickness);
        assert_eq!(state.draft.layer, app.editor.wall_layer);
        assert!(!state.dirty);
        assert!(app.info_box_wall.is_none());

        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1500., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        output.textures_delta.clear();
        for expected in [
            "默认设置",
            "图层：",
            "几何方法：",
            "参考线：",
            "参考线偏移",
            "结构：",
            "厚度",
            "高度",
            "顶部链接：",
            "顶部偏移",
            "底部偏移",
            "平面图与剖面…",
            "设置…",
        ] {
            assert!(
                texts.iter().any(|text| text == expected),
                "missing wall-tool Info Box text {expected:?}: {texts:?}"
            );
        }
        assert!(!texts.iter().any(|text| text == "全部已选: 1"));
    }

    #[test]
    fn wall_tool_info_box_defaults_apply_to_the_next_wall() {
        let (_ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.sync_info_box_wall_tool();
        {
            let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
            state.draft.thickness = 0.36;
            state.draft.height = 4.2;
            state.draft.layer = Layer::Structural;
            state.draft.settings.geometry.geometry_method = crate::wall::WallGeometryMethod::Single;
            state.draft.settings.geometry.reference_line = crate::wall::WallReferenceLine::Outside;
            state.draft.settings.geometry.reference_offset = 0.15;
            state.draft.settings.geometry.top_link_offset = -1;
            state.draft.settings.geometry.top_offset = 0.25;
            state.draft.settings.geometry.bottom_offset = -0.1;
            state.draft.settings.profile.name = "Brick 240".into();
            state.dirty = true;
        }
        assert!(app.commit_info_box_wall_tool());
        assert!(app.editor.undo.is_empty());
        assert_eq!(app.editor.thickness, 0.36);
        assert_eq!(app.editor.height, 4.2);
        assert_eq!(app.editor.wall_layer, Layer::Structural);

        app.editor.click(Point::new(0., 0.), 0.);
        app.editor.click(Point::new(4., 0.), 0.);
        let wall = app.editor.project.elements.last().expect("new wall");
        assert!(matches!(
            wall.shape,
            Shape::Wall {
                thickness: 0.36,
                height: 4.2,
                ..
            }
        ));
        assert_eq!(wall.layer, Layer::Structural);
        let settings = wall.wall_settings.as_ref().expect("wall settings");
        assert_eq!(
            settings.geometry.geometry_method,
            crate::wall::WallGeometryMethod::Single
        );
        assert_eq!(
            settings.geometry.reference_line,
            crate::wall::WallReferenceLine::Outside
        );
        assert_eq!(settings.geometry.reference_offset, 0.15);
        assert_eq!(settings.geometry.top_link_offset, -1);
        assert_eq!(settings.geometry.top_offset, 0.25);
        assert_eq!(settings.geometry.bottom_offset, -0.1);
        assert_eq!(settings.profile.name, "Brick 240");
        assert_eq!(app.editor.undo.len(), 1);
    }

    #[test]
    fn wall_tool_info_box_geometry_method_drives_real_drawing_behavior() {
        let (_ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.sync_info_box_wall_tool();
        {
            let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
            state.draft.settings.geometry.geometry_method = WallGeometryMethod::Continuous;
            state.dirty = true;
        }
        assert!(app.commit_info_box_wall_tool());
        for point in [Point::new(0., 0.), Point::new(4., 0.), Point::new(4., 3.)] {
            app.editor.click(point, 0.);
        }
        assert_eq!(app.editor.wall_points.len(), 3);
        assert!(app.editor.project.elements.is_empty());
        app.editor.finish_wall(None).unwrap();
        assert_eq!(app.editor.project.elements.len(), 2);
        assert_eq!(app.editor.selection_ids().len(), 1);
        app.sync_info_box_wall();
        assert!(
            app.info_box_wall.is_some(),
            "the last created wall stays selected so the Info Box remains usable"
        );
    }

    #[test]
    fn continuous_wall_double_click_finishes_the_chain() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        click(&ctx, &mut app, pos2(600., 300.));
        click(&ctx, &mut app, pos2(600., 300.));
        assert_eq!(app.editor.project.elements.len(), 2);
        assert!(app.editor.wall_points.is_empty());
        assert_eq!(app.editor.undo.len(), 1);
    }

    #[test]
    fn polygonal_wall_double_click_on_start_closes_once() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Polygonal;
        click(&ctx, &mut app, pos2(300., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        click(&ctx, &mut app, pos2(500., 200.));
        click(&ctx, &mut app, pos2(300., 300.));
        click(&ctx, &mut app, pos2(300., 300.));
        assert_eq!(app.editor.project.elements.len(), 3);
        assert!(app.editor.wall_points.is_empty());
        assert!(app.editor.anchor.is_none());
        assert_eq!(app.editor.undo.len(), 1);
        assert!(
            !app.editor.status.contains("needs"),
            "{}",
            app.editor.status
        );
    }

    #[test]
    fn wall_tool_info_box_rejects_invalid_thickness_without_changing_defaults() {
        let (_ctx, mut app) = setup();
        app.language = UiLanguage::SimplifiedChinese;
        app.editor.set_tool(Tool::Wall);
        app.sync_info_box_wall_tool();
        {
            let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
            state.draft.thickness = 0.001;
            state.dirty = true;
        }
        assert!(!app.commit_info_box_wall_tool());
        assert_eq!(app.editor.thickness, 0.22);
        let state = app.info_box_wall_tool.as_ref().expect("wall tool defaults");
        assert!(state.dirty);
        assert!(state.error.contains("墙厚"));
        assert!(app.editor.undo.is_empty());
    }

    #[test]
    fn command_t_commits_pending_wall_tool_defaults_before_settings() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.sync_info_box_wall_tool();
        {
            let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
            state.draft.height = 3.75;
            state.dirty = true;
        }
        plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
        let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
        assert_eq!(dialog.mode, crate::wall_ui::WallSettingsMode::Default);
        assert_eq!(dialog.draft.height, 3.75);
        assert_eq!(app.editor.height, 3.75);
        assert!(app.editor.undo.is_empty());
    }

    #[test]
    fn wall_tool_info_box_settings_buttons_open_default_dialog() {
        let (_ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.sync_info_box_wall_tool();

        app.open_info_box_wall_settings(true);
        let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
        assert_eq!(dialog.mode, crate::wall_ui::WallSettingsMode::Default);
        assert!(dialog.sections.floor_plan);
        assert_eq!(dialog.draft.thickness, app.editor.thickness);

        app.wall_settings_dialog = None;
        app.open_info_box_wall_settings(false);
        let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
        assert_eq!(dialog.mode, crate::wall_ui::WallSettingsMode::Default);
        assert!(!dialog.sections.floor_plan);
        assert!(dialog.sections.geometry);
    }
}

include!("guides_ui.rs");
include!("grids_ui.rs");
include!("trace_ui.rs");
