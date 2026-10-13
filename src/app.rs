use crate::{
    editor::{Editor, MarqueeArea, MarqueeGeometry, MarqueeScope, Tool},
    geometry::{self, Camera, SnapKind},
    localization::UiLanguage,
    model::{Element, Material, Point, Project, Shape, Story},
    render::SceneRenderer,
    storage,
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
    guides_enabled: bool,
    grids_enabled: bool,
    trace_enabled: bool,
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
            guides_enabled: true, grids_enabled: true, trace_enabled: false,
            groups_enabled: true, cutaway_enabled: false, select_sub_elements: false,
            select_locked_elements: false, all_stories: true,
            marquee_geometry: MarqueeGeometry::Rectangle, marquee_scope: MarqueeScope::CurrentStory,
            marquee_outside: false, show_marquee_3d: false, marquee_points: Vec::new(), marquee_preview: None,
            marquee_dx: 1., marquee_dy: 0.,
            marquee_height: 3., marquee_3d_preview: None,
            marquee_3d_points: Vec::new(), marquee_3d_cursor: None,
            tracker_a: 4., tracker_b: 0., tracker_locked: false, hover_point: Point::default(),
            draft: None, draft_id: None, element_settings: None, element_settings_error: String::new(),
            command_dialog: None, command_dialog_tab: 0, command_dialog_name: String::new(),
            command_dialog_number: 1., command_dialog_enabled: true,
            command_dialog_method: "Default".into(), command_dialog_reference: "Center".into(),
            command_dialog_representation: "By Element".into(), command_dialog_classification: "Not assigned".into(),
            drag: None, file_action: None,
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
        let q = |x: f32, y: f32| pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height());
        let stroke = Stroke::new(1.35, color);
        let line = |a: (f32, f32), b: (f32, f32)| { p.line_segment([q(a.0, a.1), q(b.0, b.1)], stroke); };
        let path = |points: &[(f32, f32)]| { p.add(egui::Shape::line(points.iter().map(|&(x, y)| q(x, y)).collect(), stroke)); };
        let closed = |points: &[(f32, f32)]| { p.add(egui::Shape::closed_line(points.iter().map(|&(x, y)| q(x, y)).collect(), stroke)); };
        let box_outline = |x: f32, y: f32, w: f32, h: f32| {
            p.rect_stroke(Rect::from_min_max(q(x, y), q(x + w, y + h)), 0., stroke, StrokeKind::Inside);
        };
        let circle = |x: f32, y: f32, r: f32| { p.circle_stroke(q(x, y), r * rect.width().min(rect.height()), stroke); };
        match name {
            "Home" | "Project Map" => { path(&[(0.12,0.48),(0.5,0.16),(0.88,0.48)]); line((0.23,0.43),(0.23,0.84)); line((0.77,0.43),(0.77,0.84)); line((0.23,0.84),(0.77,0.84)); line((0.43,0.84),(0.43,0.59)); line((0.43,0.59),(0.59,0.59)); line((0.59,0.59),(0.59,0.84)); },
            "View Map" | "Layout Book" | "Publisher" | "Document" => { box_outline(0.19,0.12,0.62,0.76); for y in [0.34,0.48,0.62,0.76] { line((0.31,y),(0.69,y)); } },
            "Save" => { box_outline(0.17,0.14,0.66,0.72); box_outline(0.31,0.14,0.38,0.28); box_outline(0.29,0.58,0.42,0.28); },
            "Undo" => { path(&[(0.4,0.28),(0.2,0.48),(0.4,0.68)]); line((0.22,0.48),(0.58,0.48)); path(&[(0.58,0.48),(0.78,0.57),(0.82,0.75)]); },
            "Redo" => { path(&[(0.6,0.28),(0.8,0.48),(0.6,0.68)]); line((0.78,0.48),(0.42,0.48)); path(&[(0.42,0.48),(0.22,0.57),(0.18,0.75)]); },
            "Zoom In" | "Zoom Out" => { circle(0.43,0.42,0.26); line((0.62,0.62),(0.84,0.84)); line((0.3,0.42),(0.56,0.42)); if name == "Zoom In" { line((0.43,0.29),(0.43,0.55)); } },
            "Fit" => { box_outline(0.2,0.2,0.6,0.6); path(&[(0.2,0.4),(0.2,0.2),(0.4,0.2)]); path(&[(0.6,0.2),(0.8,0.2),(0.8,0.4)]); path(&[(0.8,0.6),(0.8,0.8),(0.6,0.8)]); path(&[(0.4,0.8),(0.2,0.8),(0.2,0.6)]); },
            "Drag" => { line((0.5,0.15),(0.5,0.85)); path(&[(0.34,0.33),(0.5,0.16),(0.66,0.33)]); path(&[(0.34,0.67),(0.5,0.84),(0.66,0.67)]); line((0.15,0.5),(0.85,0.5)); path(&[(0.33,0.34),(0.16,0.5),(0.33,0.66)]); path(&[(0.67,0.34),(0.84,0.5),(0.67,0.66)]); },
            "Rotate" => { path(&[(0.72,0.38),(0.67,0.2),(0.49,0.13),(0.3,0.19),(0.19,0.36),(0.2,0.57),(0.34,0.73),(0.55,0.78)]); path(&[(0.64,0.63),(0.57,0.8),(0.75,0.79)]); },
            "Mirror" => { line((0.5,0.14),(0.5,0.86)); path(&[(0.4,0.27),(0.2,0.4),(0.2,0.69),(0.4,0.79)]); path(&[(0.6,0.27),(0.8,0.4),(0.8,0.69),(0.6,0.79)]); },
            "Multiply" => { box_outline(0.16,0.34,0.48,0.48); box_outline(0.36,0.16,0.48,0.48); },
            "Element" => { box_outline(0.21,0.21,0.58,0.58); line((0.5,0.1),(0.5,0.9)); line((0.1,0.5),(0.9,0.5)); },
            "Area Selection" => { path(&[(0.17,0.23),(0.79,0.16),(0.84,0.71),(0.42,0.84),(0.17,0.23)]); },
            "Quick Selection" => { circle(0.5,0.5,0.28); line((0.5,0.08),(0.5,0.33)); line((0.5,0.67),(0.5,0.92)); line((0.08,0.5),(0.33,0.5)); line((0.67,0.5),(0.92,0.5)); },
            "Navigator" | "Options" => { line((0.18,0.27),(0.82,0.27)); line((0.18,0.5),(0.82,0.5)); line((0.18,0.73),(0.82,0.73)); },
            "Search" => { circle(0.43,0.42,0.27); line((0.62,0.62),(0.84,0.84)); },
            "Split" => { box_outline(0.14,0.2,0.72,0.6); line((0.5,0.2),(0.5,0.8)); },
            "Floor Plan" => { box_outline(0.16,0.16,0.68,0.68); line((0.16,0.48),(0.84,0.48)); line((0.48,0.16),(0.48,0.48)); line((0.65,0.48),(0.65,0.84)); },
            "Perspective" => { closed(&[(0.16,0.29),(0.84,0.29),(0.72,0.74),(0.28,0.74)]); line((0.16,0.29),(0.5,0.53)); line((0.84,0.29),(0.5,0.53)); line((0.28,0.74),(0.5,0.53)); line((0.72,0.74),(0.5,0.53)); },
            "Elevation View" => { path(&[(0.14,0.76),(0.14,0.37),(0.5,0.17),(0.86,0.37),(0.86,0.76)]); line((0.27,0.76),(0.27,0.43)); line((0.73,0.76),(0.73,0.43)); line((0.27,0.58),(0.73,0.58)); },
            "Help" => { circle(0.5,0.5,0.36); path(&[(0.36,0.37),(0.41,0.25),(0.56,0.22),(0.68,0.31),(0.66,0.43),(0.51,0.54),(0.51,0.66)]); circle(0.51,0.82,0.025); },
            "Reference" => { box_outline(0.18,0.16,0.64,0.68); line((0.3,0.32),(0.7,0.32)); line((0.3,0.46),(0.7,0.46)); line((0.3,0.6),(0.58,0.6)); },
            "Arrow" => closed(&[(0.2,0.08),(0.2,0.86),(0.43,0.63),(0.57,0.91),(0.7,0.85),(0.55,0.58),(0.87,0.55)]),
            "Marquee" => { box_outline(0.13,0.22,0.74,0.56); for (a,b) in [((0.13,0.22),(0.36,0.22)),((0.13,0.22),(0.13,0.44)),((0.87,0.78),(0.64,0.78)),((0.87,0.78),(0.87,0.56))] { line(a,b); } },
            "Wall" => { box_outline(0.13,0.29,0.74,0.42); line((0.35,0.3),(0.35,0.7)); },
            "Slab" => closed(&[(0.08,0.58),(0.52,0.24),(0.92,0.44),(0.48,0.78)]),
            "Column" => { circle(0.5,0.5,0.34); circle(0.5,0.5,0.12); },
            "Beam" => { line((0.16,0.28),(0.84,0.28)); line((0.16,0.72),(0.84,0.72)); line((0.3,0.28),(0.3,0.72)); line((0.7,0.28),(0.7,0.72)); },
            "Door" => { line((0.2,0.15),(0.2,0.84)); line((0.2,0.84),(0.78,0.84)); line((0.2,0.22),(0.7,0.22)); path(&[(0.7,0.22),(0.69,0.35),(0.64,0.49),(0.55,0.61),(0.42,0.7),(0.27,0.75)]); },
            "Window" => { line((0.18,0.2),(0.18,0.8)); line((0.32,0.2),(0.32,0.8)); line((0.68,0.2),(0.68,0.8)); line((0.82,0.2),(0.82,0.8)); line((0.18,0.34),(0.32,0.34)); line((0.68,0.66),(0.82,0.66)); },
            "Object" | "3D Document" => { closed(&[(0.18,0.34),(0.5,0.15),(0.82,0.34),(0.82,0.69),(0.5,0.87),(0.18,0.69)]); line((0.18,0.34),(0.5,0.53)); line((0.82,0.34),(0.5,0.53)); line((0.5,0.53),(0.5,0.87)); },
            "Lamp" => { circle(0.5,0.38,0.22); path(&[(0.34,0.52),(0.41,0.65),(0.59,0.65),(0.66,0.52)]); line((0.4,0.75),(0.6,0.75)); line((0.44,0.84),(0.56,0.84)); for (a,b) in [((0.5,0.08),(0.5,0.02)),((0.17,0.2),(0.12,0.14)),((0.83,0.2),(0.88,0.14))] { line(a,b); } },
            "Roof" => { path(&[(0.1,0.66),(0.5,0.22),(0.9,0.66)]); line((0.22,0.55),(0.22,0.82)); line((0.78,0.55),(0.78,0.82)); line((0.22,0.82),(0.78,0.82)); line((0.5,0.22),(0.5,0.52)); },
            "Shell" => { path(&[(0.12,0.72),(0.2,0.48),(0.36,0.3),(0.56,0.22),(0.78,0.33),(0.9,0.62)]); path(&[(0.12,0.72),(0.3,0.7),(0.48,0.62),(0.66,0.47),(0.9,0.62)]); line((0.12,0.72),(0.9,0.62)); },
            "Stair" => path(&[(0.13,0.82),(0.13,0.67),(0.31,0.67),(0.31,0.52),(0.49,0.52),(0.49,0.37),(0.67,0.37),(0.67,0.22),(0.87,0.22)]),
            "Railing" => { line((0.16,0.28),(0.84,0.28)); line((0.16,0.42),(0.84,0.42)); line((0.2,0.42),(0.2,0.82)); line((0.4,0.42),(0.4,0.82)); line((0.6,0.42),(0.6,0.82)); line((0.8,0.42),(0.8,0.82)); },
            "Curtain Wall" => { box_outline(0.14,0.15,0.72,0.7); for x in [0.38,0.62] { line((x,0.16),(x,0.84)); } for y in [0.38,0.62] { line((0.15,y),(0.85,y)); } },
            "Skylight" => { closed(&[(0.12,0.34),(0.65,0.17),(0.88,0.32),(0.35,0.5)]); line((0.35,0.5),(0.35,0.78)); line((0.35,0.78),(0.88,0.59)); line((0.88,0.32),(0.88,0.59)); },
            "Corner-Window" => { path(&[(0.18,0.82),(0.18,0.22),(0.8,0.22)]); path(&[(0.34,0.82),(0.34,0.38),(0.8,0.38)]); line((0.18,0.54),(0.34,0.54)); line((0.57,0.22),(0.57,0.38)); },
            "Opening" => { line((0.17,0.18),(0.83,0.18)); line((0.17,0.82),(0.83,0.82)); line((0.17,0.18),(0.17,0.82)); line((0.83,0.18),(0.83,0.34)); line((0.83,0.66),(0.83,0.82)); },
            "Wall End" => { line((0.2,0.2),(0.8,0.2)); line((0.2,0.8),(0.8,0.8)); line((0.2,0.2),(0.2,0.8)); line((0.8,0.2),(0.8,0.8)); line((0.62,0.2),(0.62,0.8)); },
            "Mesh" => { closed(&[(0.15,0.37),(0.5,0.17),(0.85,0.38),(0.75,0.78),(0.28,0.82)]); for y in [0.42,0.58] { line((0.22,y),(0.8,y)); } line((0.5,0.23),(0.5,0.8)); line((0.32,0.3),(0.28,0.8)); line((0.69,0.3),(0.75,0.78)); },
            "Morph" => { closed(&[(0.16,0.3),(0.48,0.16),(0.82,0.32),(0.76,0.72),(0.42,0.86),(0.17,0.65)]); line((0.16,0.3),(0.42,0.48)); line((0.48,0.16),(0.42,0.48)); line((0.82,0.32),(0.42,0.48)); line((0.76,0.72),(0.42,0.48)); line((0.17,0.65),(0.42,0.48)); },
            "Zone" => { closed(&[(0.18,0.24),(0.74,0.18),(0.84,0.64),(0.52,0.82),(0.16,0.68)]); line((0.31,0.36),(0.69,0.32)); line((0.28,0.48),(0.73,0.44)); line((0.26,0.6),(0.62,0.56)); },
            "Section" => { line((0.16,0.78),(0.84,0.22)); line((0.42,0.52),(0.26,0.36)); path(&[(0.68,0.18),(0.86,0.16),(0.84,0.34)]); },
            "Elevation" | "Interior Elevation" => { line((0.18,0.78),(0.82,0.78)); line((0.5,0.78),(0.5,0.28)); path(&[(0.34,0.43),(0.5,0.25),(0.66,0.43)]); line((0.38,0.56),(0.62,0.56)); },
            "Detail" => { circle(0.5,0.5,0.32); line((0.5,0.18),(0.5,0.82)); line((0.18,0.5),(0.82,0.5)); circle(0.5,0.5,0.1); },
            "Worksheet" | "Figure" | "Drawing" => { box_outline(0.2,0.14,0.6,0.72); line((0.3,0.3),(0.7,0.3)); line((0.3,0.42),(0.7,0.42)); line((0.3,0.54),(0.58,0.54)); line((0.3,0.72),(0.7,0.72)); },
            "Dimension" | "Level Dimension" | "Radial Dimension" | "Angle Dimension" => { line((0.18,0.68),(0.82,0.32)); line((0.2,0.78),(0.17,0.58)); line((0.8,0.42),(0.83,0.22)); line((0.31,0.63),(0.24,0.48)); line((0.69,0.37),(0.76,0.52)); },
            "Text" => { line((0.2,0.27),(0.8,0.27)); line((0.5,0.28),(0.5,0.8)); },
            "Label" => { path(&[(0.18,0.24),(0.66,0.24),(0.84,0.42),(0.66,0.6),(0.18,0.6),(0.18,0.24)]); line((0.3,0.42),(0.58,0.42)); line((0.58,0.42),(0.8,0.78)); },
            "Fill" => { box_outline(0.18,0.18,0.64,0.64); for i in 0..5 { let n=i as f32*0.15; line((0.22+n,0.76),(0.76,0.22+n)); } },
            "Line" | "Polyline" => path(&[(0.16,0.76),(0.38,0.34),(0.56,0.62),(0.84,0.2)]),
            "Arc / Circle" => { circle(0.5,0.5,0.32); line((0.5,0.5),(0.78,0.32)); },
            "Spline" => path(&[(0.14,0.7),(0.28,0.28),(0.48,0.68),(0.67,0.32),(0.86,0.57)]),
            "Hotspot" => { circle(0.5,0.5,0.23); line((0.5,0.08),(0.5,0.3)); line((0.5,0.7),(0.5,0.92)); line((0.08,0.5),(0.3,0.5)); line((0.7,0.5),(0.92,0.5)); },
            "Grid Element" => { box_outline(0.17,0.17,0.66,0.66); for n in [0.38,0.61] { line((n,0.18),(n,0.82)); line((0.18,n),(0.82,n)); } },
            "Camera" => { box_outline(0.16,0.28,0.68,0.52); path(&[(0.35,0.28),(0.42,0.17),(0.62,0.17),(0.69,0.28)]); circle(0.5,0.54,0.16); },
            "Support" => { line((0.5,0.18),(0.5,0.82)); line((0.25,0.3),(0.75,0.3)); line((0.32,0.82),(0.68,0.82)); line((0.38,0.67),(0.62,0.67)); },
            "Link" => { circle(0.35,0.5,0.19); circle(0.65,0.5,0.19); line((0.43,0.37),(0.57,0.63)); line((0.43,0.63),(0.57,0.37)); },
            "Point Load" | "Line Load" | "Surface Load" => { line((0.5,0.16),(0.5,0.6)); path(&[(0.32,0.43),(0.5,0.62),(0.68,0.43)]); if name == "Line Load" { line((0.22,0.76),(0.78,0.76)); } if name == "Surface Load" { closed(&[(0.2,0.72),(0.5,0.58),(0.8,0.72),(0.5,0.87)]); } },
            "Duct Route" | "Pipe Route" | "Cable Route" => { path(&[(0.15,0.72),(0.15,0.35),(0.52,0.35),(0.52,0.65),(0.84,0.65)]); line((0.15,0.28),(0.15,0.42)); line((0.84,0.58),(0.84,0.72)); },
            "Branch" => { path(&[(0.5,0.82),(0.5,0.26),(0.25,0.26),(0.25,0.48)]); line((0.5,0.5),(0.75,0.5)); line((0.75,0.42),(0.75,0.58)); },
            "Fitting" => { closed(&[(0.5,0.14),(0.84,0.5),(0.5,0.86),(0.16,0.5)]); line((0.16,0.5),(0.84,0.5)); line((0.5,0.14),(0.5,0.86)); },
            "Terminal" => { box_outline(0.2,0.2,0.6,0.6); line((0.32,0.38),(0.68,0.38)); line((0.32,0.5),(0.68,0.5)); line((0.32,0.62),(0.68,0.62)); },
            "Accessory" | "Equipment" => { box_outline(0.23,0.23,0.54,0.54); circle(0.5,0.5,0.13); for (a,b) in [((0.5,0.08),(0.5,0.23)),((0.5,0.77),(0.5,0.92)),((0.08,0.5),(0.23,0.5)),((0.77,0.5),(0.92,0.5))] { line(a,b); } },
            _ => { closed(&[(0.5,0.12),(0.84,0.5),(0.5,0.88),(0.16,0.5)]); line((0.3,0.5),(0.7,0.5)); },
        }
    }

    fn toolbox_tool_row(ui: &mut Ui, name: &str, display_name: &str, selected: bool, shortcut: Option<&str>) -> bool {
        let height = if shortcut.is_some() { 36. } else { 30. };
        let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, display_name));
        let p = ui.painter();
        if selected || response.hovered() {
            p.rect_filled(r, 3., if selected { Color32::from_rgb(48,83,101) } else { PANEL_ALT });
        }
        let color = if selected { ACCENT } else { INK };
        Self::paint_tool_icon(p, Rect::from_min_size(r.left_top()+vec2(9.,6.), vec2(23.,23.)), name, color);
        p.text(r.left_center()+vec2(39.,0.), Align2::LEFT_CENTER, display_name, FontId::proportional(13.), color);
        if let Some(key) = shortcut {
            p.text(r.right_center()-vec2(8.,0.), Align2::RIGHT_CENTER, key, FontId::monospace(10.), MUTED);
        }
        response.on_hover_text(if let Some(key) = shortcut { format!("{display_name} · {key}") } else { display_name.to_owned() }).clicked()
    }
    fn vector_icon_button(ui: &mut Ui, icon: &str, tip: &str, selected: bool) -> bool {
        let (r, response) = ui.allocate_exact_size(vec2(30., 28.), Sense::click());
        if selected || response.hovered() {
            ui.painter().rect_filled(r, 3., if selected { Color32::from_rgb(48,83,101) } else { PANEL_ALT });
        }
        Self::paint_tool_icon(ui.painter(), Rect::from_center_size(r.center(), vec2(18.,18.)), icon, if selected { ACCENT } else { INK });
        response.on_hover_text(tip).clicked()
    }
    fn vector_labeled_button(ui: &mut Ui, icon: &str, label: &str, tip: &str, selected: bool) -> bool {
        let width = (43. + label.chars().count() as f32 * 7.1).min(ui.available_width()).max(44.);
        let (r, response) = ui.allocate_exact_size(vec2(width, 28.), Sense::click());
        if selected || response.hovered() {
            ui.painter().rect_filled(r, 3., if selected { Color32::from_rgb(48,83,101) } else { PANEL_ALT });
        }
        let color = if selected { ACCENT } else { INK };
        Self::paint_tool_icon(ui.painter(), Rect::from_min_size(r.left_top()+vec2(6.,5.), vec2(18.,18.)), icon, color);
        ui.painter().text(r.left_center()+vec2(29.,0.), Align2::LEFT_CENTER, label, FontId::proportional(12.), color);
        response.on_hover_text(tip).clicked()
    }
    fn navigator_item(ui: &mut Ui, icon: &str, label: &str) -> bool {
        let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.), Sense::click());
        if response.hovered() { ui.painter().rect_filled(r, 2., PANEL_ALT); }
        Self::paint_tool_icon(ui.painter(), Rect::from_min_size(r.left_top()+vec2(5.,4.), vec2(18.,18.)), icon, INK);
        ui.painter().text(r.left_center()+vec2(30.,0.), Align2::LEFT_CENTER, label, FontId::proportional(12.), INK);
        response.on_hover_text(label).clicked()
    }
    fn inline_vector_icon(ui: &mut Ui, icon: &str, color: Color32) {
        let (r, _) = ui.allocate_exact_size(vec2(20., 20.), Sense::hover());
        Self::paint_tool_icon(ui.painter(), Rect::from_min_size(r.left_top()+vec2(1.,1.), vec2(18.,18.)), icon, color);
    }
    fn place_marquee(&mut self, vertices: Vec<Point>) {
        if let Some(area) = MarqueeArea::new(vertices, self.marquee_scope, self.editor.story) {
            self.editor.marquee = Some(area);
            self.editor.set_selection(Vec::new());
            self.editor.status = format!(
                "Marquee placed · {}",
                if self.marquee_scope == MarqueeScope::AllStories { "all stories" } else { "current story" }
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
        let selected = self.editor.selection_ids();
        if selected.len() != 1 {
            self.editor.status = "Element Settings · Select one wall or slab first".into();
            return;
        }
        self.element_settings = self
            .editor
            .project
            .elements
            .iter()
            .find(|element| element.id == selected[0])
            .cloned();
        self.element_settings_error.clear();
        if let Some(element) = &self.element_settings {
            self.editor.status = format!("Element Settings · {} {:03}", element.kind(), element.id);
        } else {
            self.editor.status = "Element Settings · The selected element no longer exists".into();
        }
    }
    fn open_command_preview(&mut self, command: String) {
        self.command_dialog_tab = 0;
        self.command_dialog_name = command.clone();
        self.command_dialog = Some(command.clone());
        self.editor.status = format!("{command} · configuration preview");
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
    fn screen_to_ground(camera: &Camera, viewport: Rect, pos: Pos2, elevation: f32) -> Option<Point> {
        let ndc = [
            (pos.x - viewport.left()) / viewport.width() * 2. - 1.,
            1. - (pos.y - viewport.top()) / viewport.height() * 2.,
        ];
        camera.point_on_horizontal_plane(ndc, viewport.aspect_ratio(), elevation)
    }
    fn shortcuts(&mut self, ctx: &Context) {
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.editor.anchor.is_some() || !self.marquee_points.is_empty() || self.marquee_preview.is_some()
                || self.marquee_3d_preview.is_some() || !self.marquee_3d_points.is_empty() {
                self.editor.anchor = None;
                self.marquee_points.clear();
                self.marquee_preview = None;
                self.marquee_3d_preview = None;
                self.marquee_3d_points.clear();
                self.marquee_3d_cursor = None;
                self.tracker_locked = false;
                self.editor.set_tool(Tool::Select);
                self.preview_tool = None;
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
            self.editor.set_tool(Tool::Select);
            self.preview_tool = None;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
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
        ] {
            if ctx.input(|i| i.key_pressed(key)) {
                self.editor.set_tool(t);
                self.preview_tool = None;
                self.tracker_locked = false;
            }
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
        let Some(area) = &self.editor.marquee else { return };
        if area.vertices.is_empty() { return; }
        let min = area.vertices.iter().fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| Point::new(a.x.min(p.x), a.y.min(p.y)));
        let max = area.vertices.iter().fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| Point::new(a.x.max(p.x), a.y.max(p.y)));
        let z = area.z_range.map_or(self.editor.project.elevation(area.story) + 1., |(a, b)| (a + b) * 0.5);
        self.camera.target = glam::Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, z);
        self.camera.span = ((max.x - min.x).max(max.y - min.y) * 1.7).max(6.);
        self.view = View::Model;
    }
    fn fit_selection(&mut self) {
        let ids = self.editor.selection_ids();
        let elements: Vec<_> = self.editor.project.elements.iter().filter(|e| ids.contains(&e.id)).collect();
        if elements.is_empty() { return; }
        let points: Vec<_> = elements.iter().flat_map(|e| geometry::footprint(e)).collect();
        let min = points.iter().fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| Point::new(a.x.min(p.x), a.y.min(p.y)));
        let max = points.iter().fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| Point::new(a.x.max(p.x), a.y.max(p.y)));
        let z: Vec<f32> = elements.iter().flat_map(|e| geometry::corners(&self.editor.project, e).map(|v| v.z)).collect();
        let low = z.iter().copied().fold(f32::INFINITY, f32::min);
        let high = z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        self.camera.target = glam::Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, (low + high) * 0.5);
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
                    ui.label(RichText::new(&self.editor.project.name).color(Color32::from_rgb(210, 220, 218)));
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
                                ui.selectable_value(&mut self.language, UiLanguage::English, "English");
                                ui.selectable_value(&mut self.language, UiLanguage::SimplifiedChinese, "简体中文");
                            });
                        if self.language != previous_language {
                            self.language.save_preference();
                        }
                        ui.label(
                        RichText::new(if self.language == UiLanguage::SimplifiedChinese { "建筑设计工作台   ·   0.1" } else { "ARCHITECTURAL DESIGN   ·   0.1" })
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
                        if ui.button(Self::t(self.language, "About OpenArchi")).clicked() {
                            self.help = true;
                            ui.close();
                        }
                        ui.menu_button(self.language.translate("Project Preferences"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Working Units…", "Dimensions…", "Calculation Units…", "Zones…", "Reference Levels…", "Location Settings…", "Project North…", "Survey Point…"]) { self.open_command_preview(command); }
                        });
                        ui.menu_button(self.language.translate("Work Environment"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Appearance…", "Keyboard Shortcuts…", "Dialog Boxes and Palettes…", "Toolbox…", "Info Box…", "Toolbars…", "Menus…"]) { self.open_command_preview(command); }
                        });
                        ui.menu_button(self.language.translate("Services"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Look Up Selection", "Speech", "Spelling and Grammar", "Substitutions", "Transformations"]) { self.open_command_preview(command); }
                        });
                        ui.separator();
                        ui.label(RichText::new(self.language.translate("Native Rust · Apple Silicon / Metal")).small().color(MUTED));
                    });
                    ui.menu_button(Self::t(self.language, "File"), |ui| {
                        ui.menu_button(self.language.translate("New"), |ui| {
                            if ui.button(self.language.translate("New Empty Project…")).clicked() { self.file_action = Some(FileAction::New); ui.close(); }
                            if ui.button(self.language.translate("New & Reset All…")).clicked() { self.file_action = Some(FileAction::New); ui.close(); }
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Share as Teamwork Project…"]) { self.open_command_preview(command); }
                        });
                        if ui.button(format!("{}     ⌘O", Self::t(self.language, "Open JSON…"))).clicked() {
                            self.file_action = Some(FileAction::Open);
                            ui.close();
                        }
                        if ui.button(format!("{}      ⌘S", Self::t(self.language, "Save JSON…"))).clicked() {
                            self.file_action = Some(FileAction::Save);
                            ui.close();
                        }
                        for command in ["Save As…", "Save as Template…", "Save Selection as Module…", "Save to BIMcloud…", "Publish…"] {
                            if ui.button(self.language.translate(command)).clicked() { self.open_command_preview(command.to_owned()); ui.close(); }
                        }
                        ui.menu_button(self.language.translate("Recent Projects"), |ui| {
                            if ui.button(&self.editor.project.name).clicked() { ui.close(); }
                            ui.label(RichText::new(self.language.translate("No other recent OpenArchi projects")).small().color(MUTED));
                        });
                        ui.separator();
                        ui.menu_button(self.language.translate("Interoperability"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Merge…", "IFC Project Manager…", "IFC Translators…", "Import / Export IFC…", "SAF Translators…", "DXF-DWG Translation Setup…", "Classifications & Properties…", "Import Point Cloud…", "Place Mesh from Survey Data…", "Grasshopper Connection…", "Bimplus Connection…"]) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("External Content"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Hotlink Manager…", "Place External Drawing…", "Drawing Manager…", "Xref Manager…"]) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Libraries and Objects"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Library Manager…", "BIMcloud Libraries…", "New Object…", "Open Object…", "Save Selection as Object…", "Import Blocks…", "Import RFA…", "Import Rhino / OBJ / FBX…"]) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Info"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Project Info…", "Action Center…", "Project Notes…", "Preview and Credits…", "Session Report…"]) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Plot and Print"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Plot Setup…", "Plot…", "Page Setup…", "Print…"]) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.separator();
                        if ui.button(Self::t(self.language, "About OpenArchi")).clicked() {
                            self.help = true;
                            ui.close();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Edit"), |ui| {
                        if ui.add_enabled(!self.editor.undo.is_empty(), Button::new(format!("{}  ⌘Z", Self::t(self.language, "Undo")))).clicked() {
                            self.editor.undo();
                            ui.close();
                        }
                        if ui.add_enabled(!self.editor.redo.is_empty(), Button::new(format!("{}  ⇧⌘Z", Self::t(self.language, "Redo")))).clicked() {
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
                        if ui.button(self.language.translate("Duplicate  ⌘D")).clicked() {
                            self.editor.duplicate();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(self.language.translate("Element Settings  ⌘T")).clicked() {
                            self.open_element_settings();
                            ui.close();
                        }
                        ui.separator();
                        for (group, commands) in [
                            ("Groups", &["Group", "Ungroup", "Suspend Groups", "Lock", "Unlock"][..]),
                            ("Display Order", &["Bring to Front", "Send to Back", "Bring Forward", "Send Backward"][..]),
                            ("Move", &["Drag", "Rotate", "Mirror", "Elevate", "Multiply", "Move to Story"][..]),
                            ("Align & Distribute", &["Align Elements…", "Distribute…"][..]),
                            ("Reshape", &["Trim", "Stretch", "Resize", "Split", "Fillet / Chamfer", "Intersect", "Adjust", "Offset", "Explode", "Unify"][..]),
                            ("Reference Line and Plane", &["Modify / Invert Wall Reference Line", "Modify Slab Reference Plane"][..]),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) = Self::preview_menu_items(ui, self.language, commands) {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "View"), |ui| {
                    ui.selectable_value(&mut self.view, View::Plan, format!("{}  F2", Self::t(self.language, "Floor Plan")));
                    ui.selectable_value(&mut self.view, View::Split, Self::t(self.language, "Split view"));
                    ui.selectable_value(&mut self.view, View::Model, format!("{}  F3", Self::t(self.language, "3D Perspective")));
                        if ui.button(self.language.translate("Show selection / Marquee in 3D  F5")).clicked() {
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
                        if ui.checkbox(&mut self.show_marquee_3d, self.language.translate("Show Marquee in 3D")).changed() && self.show_marquee_3d {
                            if self.editor.marquee.is_some() { self.fit_marquee(); } else { self.fit_selection(); }
                        }
                        if self.show_marquee_3d && self.editor.marquee.is_some() {
                            ui.checkbox(&mut self.marquee_outside, self.language.translate("Show Outside Marquee"));
                        }
                        ui.separator();
                        ui.checkbox(&mut self.guides_enabled, self.language.translate("Guide Lines"));
                        ui.checkbox(&mut self.grids_enabled, self.language.translate("Construction Grid / Editing Plane"));
                        ui.checkbox(&mut self.editor.snap, self.language.translate("Element Snap"));
                        ui.menu_button(self.language.translate("Snap Options"), |ui| {
                            ui.checkbox(&mut self.editor.snap, self.language.translate("Element Snap"));
                            ui.checkbox(&mut self.grids_enabled, self.language.translate("Grid Snap"));
                            ui.checkbox(&mut self.guides_enabled, self.language.translate("Snap Guides and Points"));
                        });
                        ui.menu_button(self.language.translate("Elements in 3D View"), |ui| {
                            ui.checkbox(&mut self.cutaway_enabled, self.language.translate("3D Cutaway"));
                            ui.checkbox(&mut self.show_marquee_3d, self.language.translate("Filter and Cut Elements in 3D"));
                            if ui.button(self.language.translate("Show Selection / Marquee")).clicked() {
                                self.show_marquee_3d = true;
                                if self.editor.marquee.is_some() { self.fit_marquee(); } else { self.fit_selection(); }
                            }
                            if ui.button(self.language.translate("Hide Selection")).clicked() { self.editor.status = "3D selection visibility · interface preview".into(); }
                            if ui.button(self.language.translate("Show All")).clicked() { self.editor.status = "All elements shown in 3D".into(); }
                        });
                        ui.menu_button(self.language.translate("3D View Options"), |ui| {
                            for option in ["Projection Settings…", "Perspective", "Axonometry", "Basic", "Vectorial", "Simple Shading", "Technical", "Detailed", "Shadows", "White Model", "Wireframe", "Orbit", "Explore Model"] {
                                if ui.button(self.language.translate(option)).clicked() { self.editor.status = format!("{option} · viewport interface preview"); ui.close(); }
                            }
                        });
                        ui.menu_button(self.language.translate("Zoom and Rebuild"), |ui| {
                            if ui.button(self.language.translate("Fit in Window · F")).clicked() { self.fit(); ui.close(); }
                            if ui.button(self.language.translate("Rebuild")).clicked() { self.editor.status = "View refreshed".into(); ui.close(); }
                            if ui.button(self.language.translate("Refresh")).clicked() { self.editor.status = "View refreshed".into(); }
                            if ui.button(self.language.translate("Rebuild & Regenerate")).clicked() { self.editor.status = "Model refresh requested".into(); }
                        });
                    });
                    ui.menu_button(Self::t(self.language, "Design"), |ui| {
                        ui.menu_button(self.language.translate("Architectural Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Arrow", "Marquee", "Wall", "Wall End", "Column", "Beam", "Slab", "Roof", "Shell", "Stair", "Railing", "Curtain Wall", "Door", "Window", "Corner-Window", "Skylight", "Opening", "Zone", "Mesh", "Morph", "Object", "Lamp"]) {
                                match command.as_str() {
                                    "Wall" => { self.editor.set_tool(Tool::Wall); self.preview_tool = None; },
                                    "Slab" => { self.editor.set_tool(Tool::Slab); self.preview_tool = None; },
                                    "Marquee" => { self.editor.set_tool(Tool::Marquee); self.preview_tool = None; },
                                    "Arrow" => { self.editor.set_tool(Tool::Select); self.preview_tool = None; },
                                    _ => { self.preview_tool = Some(command.clone()); self.editor.set_tool(Tool::Select); self.editor.status = format!("{command} tool selected · interface preview"); },
                                }
                                self.tracker_locked = false;
                            }
                        });
                        ui.menu_button(self.language.translate("Structural Engineering Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Support", "Link", "Point Load", "Line Load", "Surface Load"]) { self.open_command_preview(command); }
                        });
                        ui.menu_button(self.language.translate("MEP Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Duct Route", "Pipe Route", "Cable Route", "Branch", "Fitting", "Terminal", "Accessory", "Equipment"]) { self.open_command_preview(command); }
                        });
                        for (group, commands) in [
                            ("Stories and Connections", &["Story Settings…", "Edit Elements by Stories…", "Trim Elements", "Merge Elements", "Join Wall to Curtain Wall", "Create Opening", "Clear Connections"][..]),
                            ("Model Operations", &["Solid Element Operations…", "Magic Wand", "Modify Morph", "Modify Railing", "Modify Curtain Wall"][..]),
                            ("Design Extras", &["RoofMaker…", "TrussMaker…", "Grid System…", "Update Zones"][..]),
                            ("Model Check", &["Collision Detection…", "Physical Quality…", "Model Compare…", "Structural Analytical Model…", "Energy Evaluation…"][..]),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) = Self::preview_menu_items(ui, self.language, commands) { self.open_command_preview(command); }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Document"), |ui| {
                        for (group, commands) in [
                            ("Documenting Tools", &["Section", "Elevation", "Interior Elevation", "Detail", "Worksheet", "Camera", "Dimension", "Level Dimension", "Radial Dimension", "Angle Dimension", "Text", "Label", "Grid Element", "Fill", "Line", "Arc / Circle", "Polyline", "Spline", "Hotspot", "Figure", "Drawing"][..]),
                            ("Layers", &["Layers (Model Views)…", "Show All Layers", "Hide / Show Layers", "Lock / Unlock Layers"][..]),
                            ("Pen Sets", &["Pens & Color…", "Conceptual", "Architectural", "Structural", "Mechanical", "Grayscale"][..]),
                            ("Model View", &["Model View Options…", "Floor Plan", "Ceiling Plan", "Sections and Elevations"][..]),
                            ("Graphic Overrides", &["Graphic Override Combinations…", "No Overrides", "Ceiling Plan", "Fire Plan", "Highlight Problems", "Structural Plan", "Graphic Override Rules…"][..]),
                            ("Renovation", &["Renovation Filter Options…", "Existing Plan", "Demolition Plan", "New Construction", "Reset Renovation Status…"][..]),
                            ("Annotation", &["Automatic Dimensioning…", "Label Selected Elements", "Create Patch…", "Spell Checker"][..]),
                            ("Schedules and Lists", &["Scheme Settings…", "Wall Schedule", "Door Schedule", "Window Schedule", "Object Inventory", "Element Lists", "Component Lists", "Zone Lists", "Project Indexes"][..]),
                            ("Issues and Changes", &["Issue Manager", "Issue Organizer", "Change Manager", "Keynotes"][..]),
                            ("Creative Imaging", &["PhotoRender Projection", "Render Marquee Region", "PhotoRendering Settings…", "Surface Painter", "Create Fly-Through…", "Create Sun Study…"][..]),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) = Self::preview_menu_items(ui, self.language, commands) { self.open_command_preview(command); }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Options"), |ui| {
                        ui.checkbox(&mut self.editor.snap, self.language.translate("Snap to geometry and grid"));
                        ui.checkbox(&mut self.editor.orthogonal, self.language.translate("Orthogonal drawing"));
                        ui.separator();
                        for (group, commands) in [
                            ("Element Attributes", &["Layers…", "Lines…", "Pens & Color…", "Fills…", "Surfaces…", "Building Materials…", "Composites…", "Profile Manager…", "Zone Categories…", "Attributes…", "Attribute Manager…"][..]),
                            ("Properties and Classification", &["Property Manager…", "Classification Manager…", "Operation Profiles…"][..]),
                            ("Structural Attributes", &["Load Manager…", "Load Combinations…", "Load Mapping…", "Load Categories…"][..]),
                            ("Project Preferences", &["Working Units…", "Dimensions…", "Calculation Units…", "Calculation Rules…", "Zones…", "Reference Levels…", "Legacy…", "Stair Rules and Standards…", "Structural Analytical Model Units…", "Location Settings…", "Project North…", "Survey Point…"][..]),
                            ("Work Environment", &["Dialog Boxes and Palettes…", "Selection and Element Information…", "Tracker and Coordinate Input…", "Input Constraints and Guides…", "Model Rebuild Options…", "Appearance…", "Data Safety and Integrity…", "Keyboard Shortcuts…", "Toolbox…", "Tool Settings Dialog Boxes…", "Info Box…", "Toolbars…", "Menus…"][..]),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) = Self::preview_menu_items(ui, self.language, commands) { self.open_command_preview(command); }
                            });
                        }
                        ui.separator();
                        if ui.button(self.language.translate("Add-On Manager…")).clicked() { self.editor.status = "Add-On Manager · interface preview".into(); }
                    });
                    ui.menu_button(Self::t(self.language, "Teamwork"), |ui| {
                        ui.menu_button(self.language.translate("Project"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Share…", "Migrate Teamwork Project…", "Open / Join Teamwork Project…", "Local Data Manager…"]) { self.open_command_preview(command); }
                        });
                        ui.menu_button(self.language.translate("Traveling"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Create Travel Pack", "Import Travel Pack", "Leave Teamwork Project"]) { self.open_command_preview(command); }
                        });
                        ui.menu_button(self.language.translate("Send & Receive"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Send & Receive", "Send Changes", "Receive Changes", "Send & Receive, then Compare", "Reserve Elements…", "Release All"]) { self.open_command_preview(command); }
                        });
                        ui.menu_button(self.language.translate("Messages and Review"), |ui| {
                            if let Some(command) = Self::preview_menu_items(ui, self.language, &["Assign Selected Elements", "New Message…", "Review Selected Elements…", "Change Tracking"]) { self.open_command_preview(command); }
                        });
                        if ui.button(self.language.translate("Teamwork Palette")).clicked() { self.editor.status = "Teamwork Palette · standalone preview".into(); }
                    });
                    ui.menu_button(Self::t(self.language, "Window"), |ui| {
                        ui.checkbox(&mut self.show_left, self.language.translate("Toolbox"));
                        ui.checkbox(&mut self.show_right, self.language.translate("Navigator & Properties"));
                        ui.separator();
                        for palette in ["Info Box", "Status Bar", "Quick Options", "Organizer", "Navigator Preview", "Drawing Manager", "Model Check Report", "Issue Manager", "Change Manager", "Design Options", "Renovation", "Trace & Reference", "Favorites", "Attributes", "Profile Manager", "Element Information", "Selections", "Coordinates", "Control Box", "Quick Layers", "Teamwork Palette"] {
                            if ui.selectable_label(false, self.language.translate(palette)).clicked() { self.editor.status = format!("Palette · {palette}"); }
                        }
                        ui.separator();
                        ui.menu_button(self.language.translate("Toolbars"), |ui| {
                            for toolbar in ["Standard", "Standard for Low-Res Screens", "Tools - Architectural", "Arrange Elements", "Attributes", "Drafting Aids", "Drag Elements", "Edit Elements", "3D Visualization", "Classic 3D Navigation", "MEP Toolbar", "Model Check", "Structural Engineering", "Teamwork"] {
                                if ui.selectable_label(false, self.language.translate(toolbar)).clicked() { self.editor.status = format!("Toolbar · {toolbar}"); }
                            }
                        });
                        ui.separator();
                        if ui.button(self.language.translate("Reset workspace layout")).clicked() {
                            self.show_left = true;
                            self.show_right = true;
                            self.view = View::Plan;
                            self.split = 0.53;
                            ui.close();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Help"), |ui| {
                        if ui.button(self.language.translate("About OpenArchi")).clicked() {
                            self.help = true;
                            ui.close();
                        }
                        ui.label(RichText::new(self.language.translate("Native Rust · wgpu / Metal")).small().color(MUTED));
                    });
                    ui.separator();
                    if ui
                        .add_enabled(!self.editor.undo.is_empty(), Button::new(self.language.translate("Undo")))
                        .on_hover_text("Undo · ⌘Z")
                        .clicked()
                    {
                        self.editor.undo();
                    }
                    if ui
                        .add_enabled(!self.editor.redo.is_empty(), Button::new(self.language.translate("Redo")))
                        .on_hover_text("Redo · ⇧⌘Z")
                        .clicked()
                    {
                        self.editor.redo();
                    }
                    ui.separator();
                    if Self::vector_labeled_button(ui, "Fit", &Self::t(self.language, "Fit"), "Fit model · F", false) {
                        self.fit();
                    }
                    ui.add_space(12.);
                    ui.selectable_value(&mut self.view, View::Plan, format!("{}  F2", Self::t(self.language, "2D Plan")));
                    ui.selectable_value(&mut self.view, View::Split, Self::t(self.language, "Split view"));
                    ui.selectable_value(&mut self.view, View::Model, format!("{}  F3", Self::t(self.language, "3D Perspective")));
                    if self.editor.marquee.is_some() {
                        ui.separator();
                        if ui.checkbox(&mut self.show_marquee_3d, self.language.translate("3D Marquee")).changed() && self.show_marquee_3d {
                            if self.editor.marquee.is_some() { self.fit_marquee(); } else { self.fit_selection(); }
                        }
                        if self.show_marquee_3d {
                            ui.selectable_value(&mut self.marquee_outside, false, self.language.translate("Inside"));
                            ui.selectable_value(&mut self.marquee_outside, true, self.language.translate("Outside"));
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Self::vector_labeled_button(ui, "Help", &Self::t(self.language, "Help"), "Open help and shortcuts", false) {
                            self.help = true;
                        }
                        if ui.button(Self::t(self.language, "Save project")).clicked() {
                            self.file_action = Some(FileAction::Save);
                        }
                    });
                });
            });
        egui::Panel::top("standard_toolbar")
            .frame(Frame::new().fill(CHROME).inner_margin(Margin::symmetric(9, 3)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (icon, tip) in [("Home", "Home / fit project"), ("Undo", "Undo"), ("Redo", "Redo"), ("Save", "Save project"), ("Zoom In", "Zoom in"), ("Zoom Out", "Zoom out")] {
                        if Self::vector_icon_button(ui, icon, tip, false) {
                            match icon {
                                "Home" => self.fit(),
                                "Undo" => { self.editor.undo(); },
                                "Redo" => { self.editor.redo(); },
                                "Save" => self.file_action = Some(FileAction::Save),
                                "Zoom In" => self.plan_scale = (self.plan_scale * 1.15).clamp(4., 400.),
                                "Zoom Out" => self.plan_scale = (self.plan_scale / 1.15).clamp(4., 400.),
                                _ => {}
                            }
                        }
                    }
                    ui.separator();
                    ui.toggle_value(&mut self.guides_enabled, Self::t(self.language, "Guides"));
                    ui.toggle_value(&mut self.grids_enabled, Self::t(self.language, "Grids"));
                    ui.toggle_value(&mut self.trace_enabled, Self::t(self.language, "Trace"));
                    ui.small_button(Self::t(self.language, "Measure")).on_hover_text(if self.language == UiLanguage::SimplifiedChinese { "测量工具预览" } else { "Measure tool preview" });
                    ui.toggle_value(&mut self.groups_enabled, Self::t(self.language, "Groups"));
                    ui.toggle_value(&mut self.cutaway_enabled, Self::t(self.language, "3D Cutaway"));
                    ui.separator();
                    ui.selectable_value(&mut self.view, View::Plan, Self::t(self.language, "Floor plan · F2"));
                    ui.selectable_value(&mut self.view, View::Model, Self::t(self.language, "Perspective 3D · F3"));
                });
            });
        egui::Panel::top("edit_toolbar")
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(9, 2)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.language.translate("EDIT")).size(10.).strong().color(MUTED));
                    if Self::vector_labeled_button(ui, "Drag", &Self::t(self.language, "Drag"), "Move selection", false) {
                        self.editor.status = "Drag command · select an element and drag it in the canvas".into();
                    }
                    if Self::vector_labeled_button(ui, "Rotate", &Self::t(self.language, "Rotate"), "Rotate selection 90°", false) {
                        self.editor.status = "Rotate command · choose a pivot in the canvas".into();
                    }
                    if Self::vector_labeled_button(ui, "Mirror", &Self::t(self.language, "Mirror"), "Mirror selection", false) {
                        self.editor.status = "Mirror command · choose an axis in the canvas".into();
                    }
                    if Self::vector_labeled_button(ui, "Multiply", &Self::t(self.language, "Multiply"), "Duplicate selection", false) {
                        self.editor.status = "Multiply command · choose copy settings".into();
                    }
                    ui.separator();
                    ui.label(RichText::new(self.language.translate("Workspace controls · viewport context")).small().color(MUTED));
                });
            });
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
                        };
                        ui.strong(Self::t(self.language, tool_name));
                    }
                    if self.preview_tool.is_some() {
                        // Generic tool metadata fields above are UI-only until the tool backend is implemented.
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
                                ui.label(format!("{} elements selected", self.editor.selection_ids().len()));
                            }
                        }
                    } else if self.editor.tool == Tool::Select {
                        ui.separator();
                        ui.label(RichText::new(Self::t(self.language, "Selection Settings")).strong());
                        ui.checkbox(&mut self.select_sub_elements, Self::t(self.language, "Sub-element selection"))
                            .on_hover_text("Toggle element / sub-element selection mode");
                        ui.checkbox(&mut self.select_locked_elements, Self::t(self.language, "Include locked elements"));
                        ui.separator();
                        ui.label(Self::t(self.language, "Selection methods"));
                        let _ = Self::vector_labeled_button(ui, "Element", &Self::t(self.language, "Element"), "Select whole elements", true);
                        let _ = Self::vector_labeled_button(ui, "Area Selection", &Self::t(self.language, "Area"), "Select by area", false);
                        let _ = Self::vector_labeled_button(ui, "Quick Selection", &Self::t(self.language, "Quick"), "Quick selection", false);
                        ui.label(RichText::new(format!("{} selected", self.editor.selection_ids().len())).color(MUTED));
                    } else {
                        ui.separator();
                        ui.label(Self::t(self.language, "Thickness"));
                        ui.add(
                            DragValue::new(&mut self.editor.thickness)
                                .speed(0.01)
                                .range(0.01..=5.)
                                .fixed_decimals(3)
                                .suffix(" m"),
                        );
                        if self.editor.tool != Tool::Slab {
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
                            .on_hover_text("Endpoints / midpoints / 100 mm grid");
                        ui.checkbox(&mut self.editor.orthogonal, self.language.translate("Ortho"))
                            .on_hover_text("Constrain to X/Y; Shift temporarily constrains");
                    }
                });
            });
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
                        ui.small_button("⋯").on_hover_text("Toolbox options");
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
                for name in ["Column", "Beam", "Door", "Window", "Object", "Lamp", "Roof", "Shell", "Stair", "Railing", "Curtain Wall", "Skylight", "Corner-Window", "Opening", "Wall End", "Mesh", "Morph", "Zone"] {
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
                            if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), self.preview_tool.as_deref() == Some(*name), None) {
                                self.preview_tool = Some((*name).to_owned());
                                self.editor.set_tool(Tool::Select);
                                self.editor.status = format!("{name} tool selected · interface preview");
                            }
                        }
                    });
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("OPENARCHI\nMetres · Z up\nApple Silicon / Metal")
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
                        Self::vector_icon_button(ui, "Options", "Navigator options", false);
                    });
                });
                ui.horizontal(|ui| {
                    for (view, icon, tip) in [
                        (NavigatorView::ProjectMap, "Project Map", "Project Map"),
                        (NavigatorView::ViewMap, "View Map", "View Map"),
                        (NavigatorView::LayoutBook, "Layout Book", "Layout Book"),
                        (NavigatorView::Publisher, "Publisher", "Publisher Sets"),
                    ] {
                        if Self::vector_icon_button(ui, icon, &Self::t(self.language, tip), self.navigator_view == view) { self.navigator_view = view; }
                    }
                });
                ui.horizontal(|ui| {
                    Self::inline_vector_icon(ui, "Search", MUTED);
                    ui.add_sized([ui.available_width(), 25.], TextEdit::singleline(&mut self.navigator_search).hint_text(Self::t(self.language, "Search navigator")));
                });
                ui.separator();
                match self.navigator_view {
                    NavigatorView::ProjectMap => {
                        ui.horizontal(|ui| {
                            Self::inline_vector_icon(ui, "Home", ACCENT);
                            ui.strong(&self.editor.project.name);
                        });
                        CollapsingHeader::new(Self::t(self.language, "Stories")).default_open(true).show(ui, |ui| {
                            for story in self.editor.project.stories.clone() {
                                let selected = self.editor.story == story.id;
                                let text = format!("{:02}  {}     {:+.2} m", story.id, story.name, story.elevation);
                                if ui.add_sized([ui.available_width(), 25.], Button::new(text).selected(selected)).clicked() {
                                    self.editor.switch_story(story.id);
                                    self.draft_id = None;
                                }
                            }
                        });
                        for node in ["Sections", "Elevations", "Interior Elevations", "Worksheets", "Details", "3D Documents", "3D", "Schedules", "Project Indexes", "Lists", "Info", "Help"] {
                            CollapsingHeader::new(Self::t(self.language, node)).show(ui, |ui| {
                                let entries: &[&str] = match node {
                                    "Elevations" => &["East Elevation (Auto-rebuild Model)", "North Elevation (Auto-rebuild Model)", "South Elevation (Auto-rebuild Model)", "West Elevation (Auto-rebuild Model)"],
                                    "3D" => &["Generic Perspective", "Generic Axonometry"],
                                    "Schedules" => &["Wall Schedule", "Door Schedule", "Window Schedule", "Object Inventory"],
                                    "Project Indexes" => &["Change List", "Drawing List", "Sheet Index"],
                                    _ => &["No saved views"],
                                };
                                for entry in entries {
                                    if Self::navigator_item(ui, if node == "3D" { "Perspective" } else { "Elevation View" }, &Self::t(self.language, entry)) {
                                        self.editor.status = format!("Navigator item · {entry}");
                                    }
                                }
                            });
                        }
                    }
                    NavigatorView::ViewMap => {
                        ui.strong(Self::t(self.language, "View Map"));
                        for group in ["Project Views", "Floor Plans", "Ceiling Plans", "Sections & Elevations", "3D Views", "Schedules"] {
                            CollapsingHeader::new(Self::t(self.language, group)).default_open(group == "Floor Plans").show(ui, |ui| {
                                for entry in ["Ground Floor", "South Elevation", "Generic Perspective", "Generic Axonometry"] {
                                    if Self::navigator_item(ui, if group == "Floor Plans" { "Floor Plan" } else { "View Map" }, &Self::t(self.language, entry)) {
                                        self.editor.status = format!("View Map · {entry}");
                                    }
                                }
                            });
                        }
                    }
                    NavigatorView::LayoutBook => {
                        ui.strong(Self::t(self.language, "Layout Book"));
                        for group in ["Master Layouts", "Layout Subsets", "Documentation Set", "Plans", "Elevations"] {
                            CollapsingHeader::new(Self::t(self.language, group)).default_open(group == "Documentation Set").show(ui, |ui| {
                                for entry in ["A-001 Cover", "A-101 Floor Plan", "A-201 Elevations"] {
                                    if Self::navigator_item(ui, "Layout Book", &Self::t(self.language, entry)) { self.editor.status = format!("Layout · {entry}"); }
                                }
                            });
                        }
                    }
                    NavigatorView::Publisher => {
                        ui.strong(Self::t(self.language, "Publisher Sets"));
                        for group in ["Publisher Sets", "PDF Issue", "BIMx Hyper-model", "DWG Exchange"] {
                            CollapsingHeader::new(Self::t(self.language, group)).default_open(group == "Publisher Sets").show(ui, |ui| {
                                for entry in ["Current Project", "Documentation Set", "3D Model" ] {
                                    if Self::navigator_item(ui, "Publisher", &Self::t(self.language, entry)) { self.editor.status = format!("Publisher item · {entry}"); }
                                }
                            });
                        }
                        ui.add_sized([ui.available_width(), 28.], Button::new(self.language.translate("Publish…")));
                    }
                }
                ui.checkbox(&mut self.all_stories, Self::t(self.language, "Show all stories in 3D"));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Properties, Self::t(self.language, "Properties"));
                    ui.selectable_value(&mut self.tab, Tab::Library, Self::t(self.language, "Library"));
                    ui.selectable_value(&mut self.tab, Tab::Assistant, Self::t(self.language, "AI Assistant"));
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
            ui.label(
                RichText::new(format!("{} · {:.2} m²", e.material.name(), e.area())).color(MUTED),
            );
            ui.add_space(10.);
            let stories = self.editor.project.stories.clone();
            Self::element_fields(ui, &mut e, &stories, "inspector", self.language);
            ui.add_space(14.);
            if ui
                .add_sized(
                    [ui.available_width(), 34.],
                    Button::new(RichText::new(Self::t(self.language, "Apply changes")).color(Color32::WHITE)).fill(ACCENT),
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
                RichText::new(self.language.translate("Select a wall or slab in either view to inspect and edit its properties."))
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
            let area: f32 = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Slab { .. }))
                .map(Element::area)
                .sum();
            ui.add_space(8.);
            ui.heading(format!("{}", p.elements.len()));
            ui.label(if self.language == UiLanguage::SimplifiedChinese { format!("构件 / {walls} 面墙") } else { format!("elements / {walls} walls") });
            ui.add_space(12.);
            ui.heading(format!("{area:.1} m²"));
            ui.label(self.language.translate("total slab footprint"));
        }
        ui.add_space(20.);
        ui.separator();
        CollapsingHeader::new(self.language.translate("Elements on current story")).show(ui, |ui| {
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
        });
    }
    fn number(ui: &mut Ui, language: UiLanguage, label: &str, n: &mut f32) {
        ui.label(language.translate(label));
        ui.add(DragValue::new(n).speed(0.01).fixed_decimals(3).suffix(" m"));
        ui.end_row();
    }
    fn element_fields(ui: &mut Ui, element: &mut Element, stories: &[Story], namespace: &str, language: UiLanguage) {
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
                            .map_or_else(|| format!("Level {:02}", element.story), |story| story.name.clone()),
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
        let modal = Modal::new(Id::new("element_settings_dialog")).show(ctx, |ui| {
            ui.set_min_width(470.);
            ui.heading(format!("{} {}  ·  {:03}", self.language.translate(element.kind()), self.language.translate("Settings"), element.id));
            ui.label(
                RichText::new(self.language.translate("Changes preview after applying and synchronize across all views."))
                    .color(MUTED),
            );
            ui.separator();
            Self::element_fields(ui, &mut element, &stories, "dialog", self.language);
            if !self.element_settings_error.is_empty() {
                ui.colored_label(Color32::from_rgb(224, 125, 105), &self.element_settings_error);
            }
            ui.add_space(12.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized([94., 32.], Button::new(Self::t(self.language, "OK")).fill(ACCENT))
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
    fn command_preview_dialog(&mut self, ctx: &Context) {
        let Some(title) = self.command_dialog.clone() else { return; };
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
        if applied { self.editor.status = format!("{} · preview values applied locally", title); }
        if close || modal.should_close() { self.command_dialog = None; }
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
            if !format!("{name} {desc} {tool:?} {} {}", self.language.translate(name), self.language.translate(desc))
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
                    ui.label(RichText::new(self.language.translate(desc)).small().color(MUTED));
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
            RichText::new(
                self.language.translate("Built-in wall / slab presets. External object catalogs are not connected."),
            )
            .small()
            .color(MUTED),
        );
    }
    fn assistant(&mut self, ui: &mut Ui) {
        Self::label(ui, self.language.translate("DESIGN ASSISTANT  /  LOCAL DEMO"));
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
        if ui.button(self.language.translate("Send")).clicked() && !self.assistant_input.trim().is_empty() {
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
        Point::new(
            self.plan_center.x + (p.x - r.center().x) / self.plan_scale,
            self.plan_center.y - (p.y - r.center().y) / self.plan_scale,
        )
    }
    fn screen(&self, r: Rect, p: Point) -> Pos2 {
        pos2(
            r.center().x + (p.x - self.plan_center.x) * self.plan_scale,
            r.center().y - (p.y - self.plan_center.y) * self.plan_scale,
        )
    }
    fn target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
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
            if self.editor.tool == Tool::Wall && (self.editor.orthogonal || shift) {
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
        if self.editor.tool == Tool::Wall {
            let angle = self.tracker_b.to_radians();
            Point::new(
                a.x + self.tracker_a * angle.cos(),
                a.y + self.tracker_a * angle.sin(),
            )
        } else {
            Point::new(a.x + self.tracker_a, a.y + self.tracker_b)
        }
    }
    fn plan(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(r, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Floor plan canvas"));
        let p = ui.painter_at(r);
        p.rect_filled(r, 0., PAPER);
        let hovered = response.hovered();
        let pointer = ui.input(|i| i.pointer.hover_pos());
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
            self.plan_center.x -= d.x / self.plan_scale;
            self.plan_center.y += d.y / self.plan_scale;
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let low = self.world(r, r.left_bottom());
        let high = self.world(r, r.right_top());
        let step = if self.plan_scale < 12. { 5. } else { 1. };
        for x in (low.x / step).floor() as i32..=(high.x / step).ceil() as i32 {
            let a = self.screen(r, Point::new(x as f32 * step, low.y));
            let b = self.screen(r, Point::new(x as f32 * step, high.y));
            p.line_segment([a, b], Stroke::new(0.5, Color32::from_rgb(231, 234, 227)));
        }
        for y in (low.y / step).floor() as i32..=(high.y / step).ceil() as i32 {
            let a = self.screen(r, Point::new(low.x, y as f32 * step));
            let b = self.screen(r, Point::new(high.x, y as f32 * step));
            p.line_segment([a, b], Stroke::new(0.5, Color32::from_rgb(231, 234, 227)));
        }
        // Slabs beneath walls, independently of creation order.
        let mut elements: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == self.editor.story)
            .collect();
        elements.sort_by_key(|e| matches!(e.shape, Shape::Wall { .. }));
        let selected_ids = self.editor.selection_ids();
        for e in elements {
            let selected = selected_ids.contains(&e.id);
            if matches!(e.shape, Shape::Wall { .. }) {
                continue;
            }
            let pts: Vec<_> = geometry::footprint(e)
                .into_iter()
                .map(|v| self.screen(r, v))
                .collect();
            let (fill, stroke) = if selected {
                (Color32::from_rgb(195, 225, 218), Stroke::new(2., ACCENT))
            } else {
                match e.shape {
                    Shape::Wall { .. } => unreachable!("walls are drawn in the join-aware pass"),
                    Shape::Slab { .. } => (
                        if e.material == Material::Timber {
                            Color32::from_rgb(233, 219, 193)
                        } else {
                            Color32::from_rgb(237, 238, 230)
                        },
                        Stroke::new(1., Color32::from_rgb(160, 173, 164)),
                    ),
                }
            };
            p.add(egui::Shape::convex_polygon(pts.clone(), fill, stroke));
        }
        let walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == self.editor.story && matches!(e.shape, Shape::Wall { .. }))
            .collect();
        let mut joined_wall_footprints = Vec::new();
        for material in Material::ALL {
            let same_material: Vec<_> = walls
                .iter()
                .copied()
                .filter(|wall| wall.material == material)
                .collect();
            let footprints = geometry::plan_wall_footprints(&same_material);
            for (wall, footprint) in same_material.iter().zip(&footprints) {
                joined_wall_footprints.push((wall.id, *footprint));
                if !selected_ids.contains(&wall.id) {
                    let pts: Vec<_> = footprint
                        .iter()
                        .map(|point| self.screen(r, *point))
                        .collect();
                    let wall_fill = Color32::from_rgb(82, 94, 92);
                    p.add(egui::Shape::convex_polygon(
                        pts,
                        wall_fill,
                        // Slight same-colour bleed seals sub-pixel gaps where
                        // separately editable wall footprints meet.
                        Stroke::new(0.8, wall_fill),
                    ));
                }
            }
            for mut path in geometry::wall_outline_paths(&footprints) {
                let closed = path.len() > 2 && path[0].distance(*path.last().unwrap()) <= 1e-3;
                if closed {
                    path.pop();
                }
                let points: Vec<_> = path.into_iter().map(|point| self.screen(r, point)).collect();
                if closed {
                    p.add(egui::Shape::closed_line(points, Stroke::new(1.1, INK)));
                } else {
                    p.add(egui::Shape::line(points, Stroke::new(1.1, INK)));
                }
            }
        }
        // Selected walls retain an individual highlight and editable handles.
        for wall in walls.iter().filter(|wall| selected_ids.contains(&wall.id)) {
            let joined = joined_wall_footprints
                .iter()
                .find(|(id, _)| *id == wall.id)
                .map(|(_, footprint)| *footprint)
                .unwrap_or_else(|| geometry::footprint(wall));
            let pts: Vec<_> = joined
                .into_iter()
                .map(|v| self.screen(r, v))
                .collect();
            p.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgb(195, 225, 218),
                Stroke::new(2., ACCENT),
            ));
            if let Shape::Wall { start, end, .. } = wall.shape {
                let a = self.screen(r, start);
                let b = self.screen(r, end);
                for v in [a, b] {
                    p.circle_filled(v, 4., Color32::WHITE);
                    p.circle_stroke(v, 4., Stroke::new(1.5, ACCENT));
                }
                let mid = a.lerp(b, 0.5);
                p.text(
                    mid + vec2(0., -14.),
                    Align2::CENTER_CENTER,
                    format!("{:.3} m", start.distance(end)),
                    FontId::monospace(11.),
                    ACCENT,
                );
            }
        }
        // Associative overall dimensions, computed from the current story.
        let walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == self.editor.story && matches!(e.shape, Shape::Wall { .. }))
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
                format!("{:.3} m", max.x - min.x),
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
                    p.line_segment([a + direction * offset, a + direction * end], Stroke::new(1.5, ACCENT));
                    offset += 12.;
                }
            }
        }
        if let Some((a, b)) = self.marquee_preview {
            let pts: Vec<_> = geometry::rectangle_polygon(a, b).iter().map(|v| self.screen(r, *v)).collect();
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.2, ACCENT)));
        }
        if !self.marquee_points.is_empty() {
            let mut preview = self.marquee_points.clone();
            if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
                let (q, _) = self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                if self.marquee_geometry == MarqueeGeometry::Polygon || self.marquee_geometry == MarqueeGeometry::RotatedRectangle {
                    if self.marquee_geometry == MarqueeGeometry::Polygon || preview.len() < 2 {
                        preview.push(q);
                    } else if let Some(poly) = geometry::rotated_rectangle_polygon(preview[0], preview[1], q) {
                        preview = poly;
                    }
                }
            }
            let pts: Vec<_> = preview.iter().map(|v| self.screen(r, *v)).collect();
            if pts.len() >= 2 { p.add(egui::Shape::line(pts, Stroke::new(1.3, ACCENT))); }
            for v in self.marquee_points.iter() {
                p.circle_filled(self.screen(r, *v), 3.5, Color32::WHITE);
                p.circle_stroke(self.screen(r, *v), 3.5, Stroke::new(1., ACCENT));
            }
        }
        let origin = self.screen(r, Point::default());
        p.line_segment(
            [origin - vec2(8., 0.), origin + vec2(8., 0.)],
            Stroke::new(1., ACCENT),
        );
        p.line_segment(
            [origin - vec2(0., 8.), origin + vec2(0., 8.)],
            Stroke::new(1., ACCENT),
        );
        if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
            let raw = self.world(r, pos);
            let (q, kind) = self.target(raw, ui.input(|i| i.modifiers.shift));
            self.hover_point = q;
            if !matches!(self.editor.tool, Tool::Select | Tool::Marquee) {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
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
                if let Some(a) = self.editor.anchor {
                    if !self.tracker_locked {
                        if self.editor.tool == Tool::Wall {
                            self.tracker_a = a.distance(q);
                            self.tracker_b = (q.y - a.y).atan2(q.x - a.x).to_degrees();
                        } else {
                            self.tracker_a = q.x - a.x;
                            self.tracker_b = q.y - a.y;
                        }
                    }
                    let shape = if self.editor.tool == Tool::Wall {
                        Shape::Wall {
                            start: a,
                            end: q,
                            thickness: self.editor.thickness,
                            height: self.editor.height,
                        }
                    } else {
                        Shape::Slab {
                            min: Point::new(a.x.min(q.x), a.y.min(q.y)),
                            max: Point::new(a.x.max(q.x), a.y.max(q.y)),
                            thickness: self.editor.thickness,
                        }
                    };
                    let tmp = Element {
                        id: 0,
                        name: String::new(),
                        story: self.editor.story,
                        material: self.editor.material,
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
                    p.line_segment([self.screen(r, a), s], Stroke::new(1., ACCENT));
                    let label = format!(
                        "{:.3} m  /  {:.1}°",
                        a.distance(q),
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
                }
            }
            if response.clicked() && !ui.input(|i| i.key_down(Key::Space)) {
                if self.editor.tool == Tool::Marquee {
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
                                if let Some(poly) = geometry::rotated_rectangle_polygon(self.marquee_points[0], self.marquee_points[1], self.marquee_points[2]) {
                                    self.place_marquee(poly);
                                } else {
                                    self.marquee_points.clear();
                                    self.editor.status = "Rotated rectangle needs non-zero sides".into();
                                }
                            }
                        }
                        MarqueeGeometry::Polygon => self.marquee_points.push(q),
                    }
                } else if self.editor.tool == Tool::Select {
                    let hit = geometry::hit_plan(&self.editor.project, self.editor.story, raw, 6. / self.plan_scale);
                    if ui.input(|i| i.modifiers.shift) {
                        let mut ids = self.editor.selection_ids();
                        if let Some(id) = hit {
                            if ids.contains(&id) { ids.retain(|v| *v != id); } else { ids.push(id); }
                        }
                        self.editor.set_selection(ids);
                    } else {
                        self.editor.set_selection(hit.into_iter().collect());
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else {
                    self.editor.click(q, 6. / self.plan_scale);
                    self.tracker_locked = false;
                    self.draft_id = None;
                }
            }
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
        if self.editor.tool == Tool::Marquee && self.marquee_geometry == MarqueeGeometry::Rectangle {
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
        if self.editor.tool == Tool::Select && !pan {
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                    let w = self.world(r, pos);
                    if let Some(id) = geometry::hit_plan(
                        &self.editor.project,
                        self.editor.story,
                        w,
                        6. / self.plan_scale,
                    ) {
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
    }
    fn model_view(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(r, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "3D model canvas"));
        let placing_3d_marquee = self.editor.tool == Tool::Marquee;
        let base_z = self.editor.project.elevation(self.editor.story);
        if placing_3d_marquee && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.drag_started_by(PointerButton::Primary) {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                self.marquee_3d_preview = Some((pos, pos));
            }
        }
        if placing_3d_marquee && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.dragged_by(PointerButton::Primary) {
            if let (Some((start, _)), Some(pos)) = (self.marquee_3d_preview, response.interact_pointer_pos()) {
                self.marquee_3d_preview = Some((start, pos));
            }
        }
        if placing_3d_marquee && self.marquee_geometry == MarqueeGeometry::Rectangle && response.drag_stopped() {
            if let Some((a, b)) = self.marquee_3d_preview.take() {
                if a.distance(b) > 5. {
                    let points: Option<Vec<Point>> = [pos2(a.x, a.y), pos2(b.x, a.y), pos2(b.x, b.y), pos2(a.x, b.y)]
                        .into_iter()
                        .map(|pos| Self::screen_to_ground(&self.camera, r, pos, base_z))
                        .collect();
                    if let Some(points) = points {
                        self.place_3d_marquee(points, base_z);
                    } else {
                        self.editor.status = "Marquee base must be below the perspective horizon".into();
                    }
                }
            }
        }
        if placing_3d_marquee && self.marquee_geometry != MarqueeGeometry::Rectangle {
            self.marquee_3d_cursor = ui.input(|i| i.pointer.hover_pos()).filter(|pos| r.contains(*pos));
        }
        if placing_3d_marquee && self.marquee_geometry != MarqueeGeometry::Rectangle
            && response.clicked_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = Self::screen_to_ground(&self.camera, r, pos, base_z) {
                    self.marquee_points.push(point);
                    self.marquee_3d_points.push(pos);
                    match self.marquee_geometry {
                        MarqueeGeometry::Polygon => {},
                        MarqueeGeometry::RotatedRectangle if self.marquee_points.len() == 3 => {
                            let vertices = geometry::rotated_rectangle_polygon(
                                self.marquee_points[0], self.marquee_points[1], self.marquee_points[2],
                            );
                            if let Some(vertices) = vertices {
                                self.place_3d_marquee(vertices, base_z);
                            } else {
                                self.marquee_points.clear();
                                self.marquee_3d_points.clear();
                                self.editor.status = "Rotated rectangle needs non-zero sides".into();
                            }
                        }
                        _ => {},
                    }
                } else {
                    self.editor.status = "Marquee base must be below the perspective horizon".into();
                }
            }
        }
        if placing_3d_marquee && self.marquee_geometry == MarqueeGeometry::Polygon
            && response.clicked_by(PointerButton::Secondary) {
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
        if (response.dragged_by(PointerButton::Primary) && !placing_3d_marquee && self.editor.tool != Tool::Marquee)
            || shift_middle_orbit
        {
            let d = ui.input(|i| i.pointer.delta());
            self.camera.yaw -= d.x * 0.009;
            self.camera.pitch = (self.camera.pitch + d.y * 0.007).clamp(0.08, 1.53);
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        if response.dragged_by(PointerButton::Secondary)
            || (middle_dragging && !ui.input(|i| i.modifiers.shift))
        {
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
            let selection_only = self.show_marquee_3d && !selected.is_empty() && self.editor.marquee.is_none();
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
                if self.show_marquee_3d && !selection_only { self.editor.marquee.as_ref() } else { None },
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
                    if lo.is_finite() && hi.is_finite() { (lo, hi) } else { (0., self.marquee_height) }
                });
                let matrix = self.camera.matrix(r.aspect_ratio());
                let screen_point = |point: Point, z: f32| {
                    let ndc = matrix.project_point3(glam::Vec3::new(point.x, point.y, z));
                    ndc.is_finite().then(|| pos2(
                        r.left() + (ndc.x + 1.) * 0.5 * r.width(),
                        r.top() + (1. - ndc.y) * 0.5 * r.height(),
                    ))
                };
                let base: Vec<_> = area.vertices.iter().filter_map(|point| screen_point(*point, bottom)).collect();
                let cap: Vec<_> = area.vertices.iter().filter_map(|point| screen_point(*point, top)).collect();
                if base.len() == area.vertices.len() && cap.len() == area.vertices.len() {
                    for i in 0..base.len() {
                        let next = (i + 1) % base.len();
                        p.line_segment([base[i], base[next]], Stroke::new(1.7, ACCENT));
                        p.line_segment([cap[i], cap[next]], Stroke::new(1.2, Color32::from_rgb(114, 193, 224)));
                        p.line_segment([base[i], cap[i]], Stroke::new(1.1, Color32::from_rgb(114, 193, 224)));
                    }
                }
            }
        }
        if !self.marquee_3d_points.is_empty() {
            let mut preview = self.marquee_3d_points.clone();
            if let Some(cursor) = self.marquee_3d_cursor.filter(|pos| r.contains(*pos)) {
                preview.push(cursor);
                if self.marquee_geometry == MarqueeGeometry::RotatedRectangle && preview.len() >= 3 {
                    preview = vec![self.marquee_3d_points[0], self.marquee_3d_points[1], cursor, self.marquee_3d_points[0]];
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
    }
    fn tracker(&mut self, ui: &mut Ui) {
        if let Some(a) = self.editor.anchor {
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        Self::label(ui, self.language.translate("TRACKER"));
                        let wall = self.editor.tool == Tool::Wall;
                        ui.label(if wall { "Length" } else { "Δ X" });
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
                        ui.label(if wall { "Angle" } else { "Δ Y" });
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_b)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(if wall { " °" } else { " m" }),
                            )
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                        ui.checkbox(&mut self.tracker_locked, self.language.translate("Lock"));
                        let enter = ui.input(|i| i.key_pressed(Key::Enter));
                        if ui.button(self.language.translate("Create [Enter]")).clicked() || enter {
                            let target = self.tracker_target(a);
                            self.editor.click(target, 0.);
                            self.tracker_locked = false;
                            self.draft_id = None;
                        }
                        if ui.button(self.language.translate("Cancel")).clicked() {
                            self.editor.anchor = None;
                            self.tracker_locked = false;
                        }
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
                    ui.label(self.language.translate("Start an empty project with three stories."));
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
                                    self.draft_id = None;
                                    self.draft = None;
                                    self.fit();
                                }),
                            FileAction::New => {
                                self.editor = Editor::new(Project {
                                    name: "Untitled project".into(),
                                    ..Project::default()
                                });
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
    fn workspace(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.file_action.is_none() && !self.help && self.element_settings.is_none() {
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
                    ui.label(RichText::new(&self.editor.status).size(11.));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "X {:.3}   Y {:.3}   |   {} elements",
                                self.hover_point.x,
                                self.hover_point.y,
                                self.editor.project.elements.len()
                            ))
                            .monospace()
                            .size(10.)
                            .color(MUTED),
                        );
                    });
                });
            });
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
                        Self::vector_icon_button(ui, "Navigator", "Open Navigator", false);
                        ui.separator();
                        let floor_label = format!("F2 · {:02} · {}", self.editor.story, self.editor.project.stories.iter().find(|s| s.id == self.editor.story).map_or("Floor plan", |s| s.name.as_str()));
                        let _ = Self::vector_labeled_button(ui, "Floor Plan", &floor_label, "Floor Plan · F2", self.view == View::Plan).then(|| self.view = View::Plan);
                        let _ = Self::vector_labeled_button(ui, "Split", "Split", "Split view", self.view == View::Split).then(|| self.view = View::Split);
                        let _ = Self::vector_labeled_button(ui, "Perspective", "F3 · 3D Perspective", "3D Perspective · F3", self.view == View::Model).then(|| self.view = View::Model);
                        let _ = Self::vector_labeled_button(ui, "Elevation View", "South Elevation", "South Elevation view preview", false);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("{} elements", self.editor.project.elements.len())).small().color(MUTED));
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
        self.element_settings_dialog(&ctx);
        self.command_preview_dialog(&ctx);
        Window::new(Self::t(self.language, "About OpenArchi")).open(&mut self.help).default_width(460.).show(&ctx, |ui| {
            if self.language == UiLanguage::SimplifiedChinese {
                ui.heading(self.language.translate("OpenArchi · 建筑设计工作台"));
                ui.label(self.language.translate("版本 0.1.0 · 原生 Rust 桌面应用"));
                ui.label(self.language.translate("在平面图中绘制墙体和矩形楼板。可在二维与三维视图中选择并编辑同一构件，尺寸单位为米。"));
                ui.separator();
                for text in ["V 选择 · M 选区框 · W 墙 · L 楼板", "F2 楼层平面 · F3 三维透视", "选区框支持矩形、旋转矩形和多边形", "F5 在三维中显示选择构件或选区框裁剪体", "⌘A 选择选区框内的构件节点", "⌘Z 撤销 · ⇧⌘Z 重做 · ⌘D 复制", "⌘S 保存 JSON · ⌘O 打开 JSON"] { ui.label(text); }
                ui.separator();
                ui.label(format!("渲染器：{}", self.gpu_name));
                ui.label(self.language.translate("原型限制：暂不支持 PDF/DWG 导出和原生 BIM 文件解析。当前助手为本地规则演示。"));
            } else {
                ui.heading(self.language.translate("OpenArchi · Architectural Design Workbench"));
                ui.label(self.language.translate("Version 0.1.0 · Native Rust desktop application"));
                ui.label(self.language.translate("Draw walls and rectangular slabs in plan. Select and edit the same elements in either view. All dimensions are metres."));
                ui.separator();
                for text in ["V · Select     M · Marquee     W · Wall     L · Slab", "F2 · Floor plan     F3 · Perspective 3D", "Marquee: rectangle, rotated rectangle or polygon", "F5 · Show selected elements or the clipped Marquee volume in 3D", "⌘A · Select elements with nodes inside Marquee", "Cmd/Ctrl+X/C/V · Cut / Copy / Paste selected elements", "Marquee controls: move area, stretch nodes, mirror, rotate, story", "Click start → click end, or type in Tracker → Create", "Drag selected elements in plan to move them", "Shift / Ortho · constrain walls to X or Y", "Right-drag · pan     Scroll · zoom     F · fit", "3D: Shift+middle-drag orbit, right-drag pan, click select", "⌘Z · Undo     ⇧⌘Z · Redo     ⌘D · Duplicate", "Delete · Remove     Esc · Cancel / Select", "⌘S · Save JSON     ⌘O · Open JSON"] { ui.label(text); }
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
    fn plan_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
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
            },
        );
        output.textures_delta.clear();
    }
    fn model_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
        let mut output=ctx.run_ui(RawInput{
            screen_rect:Some(Rect::from_min_size(Pos2::ZERO,vec2(800.,600.))),
            events,..Default::default()
        },|ui|{CentralPanel::default().frame(Frame::NONE).show(ui,|ui|app.model_view(ui));});
        output.textures_delta.clear();
    }
    fn click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) {
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
        );
    }
    fn click_model(ctx: &Context, app: &mut AtelierApp, pos: Pos2, button: PointerButton) {
        model_frame(ctx, app, vec![Event::PointerMoved(pos)]);
        model_frame(ctx, app, vec![Event::PointerMoved(pos), Event::PointerButton {
            pos, button, pressed: true, modifiers: Modifiers::NONE,
        }]);
        model_frame(ctx, app, vec![Event::PointerButton {
            pos, button, pressed: false, modifiers: Modifiers::NONE,
        }]);
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
    fn marquee_rectangle_is_created_by_plan_canvas_drag() {
        let (ctx, mut app) = setup();
        plan_frame(&ctx, &mut app, vec![key(Key::M, Modifiers::NONE)]);
        let a = pos2(340., 260.);
        let b = pos2(460., 340.);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(a), Event::PointerButton {
            pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE,
        }]);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        plan_frame(&ctx, &mut app, vec![Event::PointerButton {
            pos: b, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE,
        }]);
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
        assert_eq!(app.marquee_points.len(), 3, "points before Enter: {:?}", app.marquee_points);
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.editor.marquee.is_none(), "Enter is reserved for numeric tracker input");
        let pos = pos2(440., 370.);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos), Event::PointerButton {
            pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE,
        }]);
        plan_frame(&ctx, &mut app, vec![Event::PointerButton {
            pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE,
        }]);
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
        assert_eq!(app.marquee_3d_points.len(), 1, "first screen point not registered: {:?}", app.editor.status);
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
        assert!(app.editor.marquee.is_none(), "three vertices only define the base until right-click");
        click_model(&ctx, &mut app, points[2], PointerButton::Secondary);
        let area = app.editor.marquee.as_ref().expect("right click closes the 3D polygon");
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
                app.marquee_points,
                app.editor.status
            )
        });
        assert_eq!(area.vertices.len(), 4);
        assert_eq!(area.z_range, Some((0., app.marquee_height)));
        assert!(app.show_marquee_3d);
    }
    #[test]
    fn default_is_single_plan_window_and_f_keys_switch_3d_workflow() {
        let (ctx,mut app)=setup();
        assert_eq!(app.view,View::Plan);
        let selected_id=app.editor.project.add(0,Material::Concrete,Shape::Wall{
            start:Point::new(0.,0.),end:Point::new(3.,0.),thickness:0.2,height:3.,
        });
        app.editor.set_selection(vec![selected_id]);
        app.editor.marquee=MarqueeArea::new(
            vec![Point::new(0.,0.),Point::new(2.,0.),Point::new(2.,1.),Point::new(0.,1.)],
            MarqueeScope::CurrentStory,0,
        );
        plan_frame(&ctx,&mut app,vec![key(Key::F5,Modifiers::NONE)]);
        assert_eq!(app.view,View::Model);
        assert!(app.show_marquee_3d);
        assert!(app.editor.status.contains("clipped Marquee"));
        assert_eq!(app.editor.selection_ids(),vec![selected_id]);
        plan_frame(&ctx,&mut app,vec![key(Key::F2,Modifiers::NONE)]);
        assert_eq!(app.view,View::Plan);
        app.editor.marquee=None;
        plan_frame(&ctx,&mut app,vec![key(Key::F5,Modifiers::NONE)]);
        assert!(app.editor.status.contains("selected elements"));
        plan_frame(&ctx,&mut app,vec![key(Key::F3,Modifiers::NONE)]);
        assert_eq!(app.view,View::Model);
    }
    #[test]
    fn command_t_opens_independent_element_settings_and_applies_edits() {
        let (ctx, mut app) = setup();
        let id = app.editor.project.add(0, Material::Concrete, Shape::Wall {
            start: Point::new(0., 0.), end: Point::new(4., 0.), thickness: 0.2, height: 3.,
        });
        app.editor.set_selection(vec![id]);
        app.tab = Tab::Library;
        app.show_right = false;
        plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
        assert!(matches!(app.tab, Tab::Library));
        assert!(!app.show_right, "the settings dialog is independent of the inspector");
        assert_eq!(app.element_settings.as_ref().map(|element| element.id), Some(id));

        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            app.element_settings_dialog(ui.ctx());
        });
        output.textures_delta.clear();
        if let Some(Shape::Wall { height, .. }) = app.element_settings.as_mut().map(|element| &mut element.shape) {
            *height = 4.25;
        }
        app.apply_element_settings().unwrap();
        assert!(matches!(app.editor.project.elements[0].shape, Shape::Wall { height, .. } if height == 4.25));
        app.editor.undo();
        assert!(matches!(app.editor.project.elements[0].shape, Shape::Wall { height, .. } if height == 3.));
    }
    #[test]
    fn shift_middle_drag_orbits_3d_while_plain_middle_drag_pans() {
        let (ctx,mut app)=setup();
        app.editor.set_tool(Tool::Marquee);
        let yaw=app.camera.yaw;
        let pitch=app.camera.pitch;
        let target=app.camera.target;
        let start=pos2(400.,300.);
        let end=pos2(450.,325.);
        let shift=Modifiers{shift:true,..Modifiers::NONE};
        model_frame(&ctx,&mut app,vec![Event::PointerMoved(start)]);
        model_frame(&ctx,&mut app,vec![Event::ModifiersChanged(shift),Event::PointerButton{
            pos:start,button:PointerButton::Middle,pressed:true,modifiers:shift,
        }]);
        model_frame(&ctx,&mut app,vec![Event::ModifiersChanged(shift),Event::PointerMoved(end)]);
        assert!(ctx.input(|i| i.pointer.button_down(PointerButton::Middle)));
        assert_ne!(ctx.input(|i| i.pointer.delta()),Vec2::ZERO);
        model_frame(&ctx,&mut app,vec![Event::ModifiersChanged(shift),Event::PointerButton{
            pos:end,button:PointerButton::Middle,pressed:false,modifiers:shift,
        },Event::ModifiersChanged(Modifiers::NONE)]);
        assert_ne!(app.camera.yaw,yaw);
        assert_ne!(app.camera.pitch,pitch);
        assert_eq!(app.camera.target,target);

        let yaw=app.camera.yaw;
        let target=app.camera.target;
        let pan_start=pos2(300.,200.);
        let pan_end=pos2(360.,230.);
        model_frame(&ctx,&mut app,vec![Event::PointerMoved(pan_start)]);
        model_frame(&ctx,&mut app,vec![Event::ModifiersChanged(Modifiers::NONE),Event::PointerButton{
            pos:pan_start,button:PointerButton::Middle,pressed:true,modifiers:Modifiers::NONE,
        }]);
        model_frame(&ctx,&mut app,vec![Event::ModifiersChanged(Modifiers::NONE),Event::PointerMoved(pan_end)]);
        model_frame(&ctx,&mut app,vec![Event::PointerButton{
            pos:pan_end,button:PointerButton::Middle,pressed:false,modifiers:Modifiers::NONE,
        }]);
        assert_eq!(app.camera.yaw,yaw);
        assert_ne!(app.camera.target,target);
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
                for navigator_view in [NavigatorView::ProjectMap, NavigatorView::ViewMap, NavigatorView::LayoutBook, NavigatorView::Publisher] {
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
        assert!(chinese_glyphs_present, "the UI font fallback must contain Simplified Chinese glyphs");
        assert_eq!(app.editor.project.elements, before);
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
}
