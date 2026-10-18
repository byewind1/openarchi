use crate::localization::UiLanguage;
use crate::model::{Element, Point, Shape, Story};
use crate::view_settings::{Layer, RenovationStatus};
use crate::wall::*;
use eframe::egui::{
    self, Align, Align2, Button, Color32, ComboBox, Context, DragValue, FontId, Grid, Id, Layout,
    Modal, RichText, ScrollArea, Sense, Stroke, Ui, vec2,
};

const WALL_INK: Color32 = Color32::from_rgb(225, 229, 230);
const WALL_MUTED: Color32 = Color32::from_rgb(155, 164, 167);
const WALL_ACCENT: Color32 = Color32::from_rgb(74, 160, 211);
const WALL_HEADER: Color32 = Color32::from_rgb(39, 42, 44);
const WALL_HEADER_OPEN: Color32 = Color32::from_rgb(48, 52, 54);
const WALL_HEADER_HOVER: Color32 = Color32::from_rgb(58, 63, 65);
const WALL_LINE: Color32 = Color32::from_rgb(76, 81, 83);
const WALL_PANEL: Color32 = Color32::from_rgb(45, 48, 50);
const WALL_ERROR: Color32 = Color32::from_rgb(224, 125, 105);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallSettingsMode {
    Selected(u64),
    Default,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallPosition {
    pub start: Point,
    pub end: Point,
}

/// Mutable editor state for one wall or for the wall tool's default settings.
/// It owns every field shown in the AC29 Wall Settings dialog (plus the
/// element-level story, layer, renovation status, name and position).
#[derive(Clone, Debug, PartialEq)]
pub struct WallSettingsDraft {
    pub settings: WallSettings,
    pub name: String,
    pub story: u32,
    pub layer: Layer,
    pub renovation: RenovationStatus,
    pub thickness: f32,
    pub height: f32,
    pub position: Option<WallPosition>,
}

impl WallSettingsDraft {
    pub fn from_element(element: &Element) -> Option<Self> {
        let Shape::Wall {
            start,
            end,
            thickness,
            height,
        } = element.shape
        else {
            return None;
        };
        Some(Self {
            settings: element.wall_settings.clone().unwrap_or_default(),
            name: element.name.clone(),
            story: element.story,
            layer: element.layer,
            renovation: element.renovation,
            thickness,
            height,
            position: Some(WallPosition { start, end }),
        })
    }

    pub fn defaults(
        settings: &WallSettings,
        thickness: f32,
        height: f32,
        story: u32,
        layer: Layer,
        renovation: RenovationStatus,
    ) -> Self {
        Self {
            settings: settings.clone(),
            name: String::new(),
            story,
            layer,
            renovation,
            thickness,
            height,
            position: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.thickness.is_finite() || !(0.01..=5.).contains(&self.thickness) {
            return Err("Wall thickness must be between 1 mm and 5 m".into());
        }
        if !self.height.is_finite() || !(0.01..=100.).contains(&self.height) {
            return Err("Wall height must be between 10 mm and 100 m".into());
        }
        if self.name.chars().count() > 128 {
            return Err("Wall name is too long".into());
        }
        if let Some(position) = self.position {
            if !position.start.finite() || !position.end.finite() {
                return Err("Wall coordinates must be finite".into());
            }
            if position.start.distance(position.end) < 0.01 {
                return Err("Wall must be at least 10 mm long".into());
            }
            if position.start.x.abs() > 100000.
                || position.start.y.abs() > 100000.
                || position.end.x.abs() > 100000.
                || position.end.y.abs() > 100000.
            {
                return Err("Wall coordinates are outside the local modeling range".into());
            }
        }
        self.settings.validate()
    }

    pub fn apply_to_element(&self, element: &mut Element) -> Result<(), String> {
        self.validate()?;
        let Shape::Wall {
            start,
            end,
            thickness,
            height,
        } = &mut element.shape
        else {
            return Err("The selected element is not a wall".into());
        };
        if let Some(position) = self.position {
            *start = position.start;
            *end = position.end;
        }
        *thickness = self.thickness;
        *height = self.height;
        element.story = self.story;
        element.layer = self.layer;
        element.renovation = self.renovation;
        if !self.name.trim().is_empty() {
            element.name = self.name.trim().to_owned();
        }
        element.wall_settings = Some(self.settings.clone());
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct WallSectionState {
    pub geometry: bool,
    pub profile: bool,
    pub floor_plan: bool,
    pub model: bool,
    pub structural: bool,
    pub classification: bool,
    pub class_list: bool,
    pub class_id: bool,
    pub class_design: bool,
    pub class_renovation: bool,
    pub class_ratings: bool,
    pub class_life: bool,
    pub class_brick: bool,
    pub class_ceiling: bool,
    pub class_cost: bool,
    pub class_definition: bool,
}

impl Default for WallSectionState {
    fn default() -> Self {
        Self {
            geometry: true,
            profile: false,
            floor_plan: false,
            model: false,
            structural: false,
            classification: false,
            class_list: false,
            class_id: false,
            class_design: false,
            class_renovation: false,
            class_ratings: false,
            class_life: false,
            class_brick: false,
            class_ceiling: false,
            class_cost: false,
            class_definition: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WallSettingsDialog {
    pub draft: WallSettingsDraft,
    pub mode: WallSettingsMode,
    pub sections: WallSectionState,
    pub error: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallDialogAction {
    None,
    Apply,
    Ok,
    Cancel,
}

impl WallSettingsDialog {
    pub fn selected(element: &Element) -> Option<Self> {
        let draft = WallSettingsDraft::from_element(element)?;
        Some(Self {
            draft,
            mode: WallSettingsMode::Selected(element.id),
            sections: WallSectionState::default(),
            error: String::new(),
        })
    }

    pub fn defaults(
        settings: &WallSettings,
        thickness: f32,
        height: f32,
        story: u32,
        layer: Layer,
        renovation: RenovationStatus,
    ) -> Self {
        Self {
            draft: WallSettingsDraft::defaults(
                settings, thickness, height, story, layer, renovation,
            ),
            mode: WallSettingsMode::Default,
            sections: WallSectionState::default(),
            error: String::new(),
        }
    }

    pub fn show(
        &mut self,
        context: &Context,
        language: UiLanguage,
        stories: &[Story],
    ) -> WallDialogAction {
        let mut action = WallDialogAction::None;
        let title = match self.mode {
            WallSettingsMode::Selected(_) => language.translate("Wall Settings").to_owned(),
            WallSettingsMode::Default => language.translate("Wall Default Settings").to_owned(),
        };
        let badge = match self.mode {
            WallSettingsMode::Selected(id) => format!("Wall {id:03}"),
            WallSettingsMode::Default => language.translate("Default").to_owned(),
        };
        let draft = &mut self.draft;
        let sections = &mut self.sections;
        let error = &mut self.error;
        let show_position = matches!(self.mode, WallSettingsMode::Selected(_));
        let modal = Modal::new(Id::new("wall_settings_dialog")).show(context, |ui| {
            ui.set_min_width(560.);
            ui.set_max_width(620.);
            ui.visuals_mut().widgets.inactive.weak_bg_fill = WALL_PANEL;
            ui.visuals_mut().widgets.inactive.bg_fill = WALL_PANEL;
            ui.visuals_mut().widgets.hovered.weak_bg_fill = WALL_HEADER_HOVER;
            ui.visuals_mut().widgets.active.weak_bg_fill = WALL_HEADER_HOVER;
            ui.visuals_mut().widgets.noninteractive.bg_stroke = Stroke::new(1., WALL_LINE);
            ui.visuals_mut().selection.bg_fill = Color32::from_rgb(48, 83, 101);

            ui.horizontal(|ui| {
                ui.heading(RichText::new(&title).color(WALL_INK));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(badge).color(WALL_MUTED));
                });
            });
            ui.label(
                RichText::new(language.translate(
                    "Geometry fields update Plan and 3D; every group is stored with the wall.",
                ))
                .small()
                .color(WALL_MUTED),
            );
            ui.add_space(6.);
            ui.separator();

            ScrollArea::vertical()
                .id_salt("wall_settings_scroll")
                .max_height(520.)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    wall_section(
                        ui,
                        language,
                        "GEOMETRY AND POSITIONING",
                        &mut sections.geometry,
                        |ui| geometry_body(ui, language, stories, draft, show_position),
                    );
                    wall_section(
                        ui,
                        language,
                        "PROFILE OFFSET MODIFIERS",
                        &mut sections.profile,
                        |ui| profile_body(ui, language, draft),
                    );
                    wall_section(
                        ui,
                        language,
                        "FLOOR PLAN AND SECTION",
                        &mut sections.floor_plan,
                        |ui| floor_plan_body(ui, language, draft),
                    );
                    wall_section(ui, language, "MODEL", &mut sections.model, |ui| {
                        model_body(ui, language, draft)
                    });
                    wall_section(
                        ui,
                        language,
                        "STRUCTURAL ANALYTICAL PARAMETERS",
                        &mut sections.structural,
                        |ui| structural_body(ui, language, draft),
                    );
                    let class_list = &mut sections.class_list;
                    let class_id = &mut sections.class_id;
                    let class_design = &mut sections.class_design;
                    let class_renovation = &mut sections.class_renovation;
                    let class_ratings = &mut sections.class_ratings;
                    let class_life = &mut sections.class_life;
                    let class_brick = &mut sections.class_brick;
                    let class_ceiling = &mut sections.class_ceiling;
                    let class_cost = &mut sections.class_cost;
                    let class_definition = &mut sections.class_definition;
                    wall_section(
                        ui,
                        language,
                        "CLASSIFICATION AND PROPERTIES",
                        &mut sections.classification,
                        |ui| {
                            classification_body(
                                ui,
                                language,
                                stories,
                                draft,
                                class_list,
                                class_id,
                                class_design,
                                class_renovation,
                                class_ratings,
                                class_life,
                                class_brick,
                                class_ceiling,
                                class_cost,
                                class_definition,
                            )
                        },
                    );
                });

            if !error.is_empty() {
                ui.add_space(6.);
                ui.colored_label(WALL_ERROR, localized_error(language, error));
            }
            ui.add_space(8.);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(language.translate("Layer:"));
                ComboBox::from_id_salt("wall_settings_layer")
                    .selected_text(layer_label(language, draft.layer))
                    .width(190.)
                    .show_ui(ui, |ui| {
                        for layer in Layer::ALL {
                            ui.selectable_value(
                                &mut draft.layer,
                                layer,
                                layer_label(language, layer),
                            );
                        }
                    });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [92., 32.],
                            Button::new(
                                RichText::new(language.translate("OK")).color(Color32::WHITE),
                            )
                            .fill(WALL_ACCENT),
                        )
                        .clicked()
                    {
                        action = WallDialogAction::Ok;
                    }
                    if ui.button(language.translate("Apply")).clicked() {
                        action = WallDialogAction::Apply;
                    }
                    if ui.button(language.translate("Cancel")).clicked() {
                        action = WallDialogAction::Cancel;
                    }
                });
            });
        });
        if action == WallDialogAction::None && modal.should_close() {
            action = WallDialogAction::Cancel;
        }
        action
    }
}

pub fn localized_error(language: UiLanguage, error: &str) -> String {
    if language == UiLanguage::English {
        return error.to_owned();
    }
    if error.starts_with("Wall setting ") {
        if error.ends_with(" must be finite") {
            return "墙设置必须是有限数值".into();
        }
        if error.ends_with(" is out of range") {
            return "墙设置超出允许范围".into();
        }
        if error.ends_with(" is too long") {
            return "墙设置文本过长".into();
        }
    }
    if error.ends_with(" would be outside its host wall") {
        return "洞口将超出其宿主墙范围".into();
    }
    let known = match error {
        "Wall thickness must be between 1 mm and 5 m" => "墙厚必须在 1 毫米到 5 米之间",
        "Wall height must be between 10 mm and 100 m" => "墙高必须在 10 毫米到 100 米之间",
        "Wall name is too long" => "墙名称过长",
        "Wall coordinates must be finite" => "墙坐标必须是有限数值",
        "Wall must be at least 10 mm long" => "墙长度至少为 10 毫米",
        "Wall coordinates are outside the local modeling range" => "墙坐标超出本地建模范围",
        "The selected element is not a wall" => "所选构件不是墙",
        "Wall top link is out of range" => "墙顶部链接超出范围",
        "Wall must stay at least 0.01 m long" => "墙参考线长度至少为 0.01 米",
        "Wall reference line must use finite points" => "墙参考线坐标必须是有限数值",
        _ => return error.to_owned(),
    };
    known.to_owned()
}

pub(crate) fn layer_label(language: UiLanguage, layer: Layer) -> String {
    language.translate(layer.name()).to_owned()
}

fn renovation_label(language: UiLanguage, status: RenovationStatus) -> String {
    language.translate(status.name()).to_owned()
}

fn wall_section(
    ui: &mut Ui,
    language: UiLanguage,
    title: &str,
    open: &mut bool,
    body: impl FnOnce(&mut Ui),
) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.), Sense::click());
    let fill = if *open {
        WALL_HEADER_OPEN
    } else if response.hovered() {
        WALL_HEADER_HOVER
    } else {
        WALL_HEADER
    };
    ui.painter().rect_filled(rect, 2., fill);
    let center = rect.left_center() + vec2(15., 0.);
    let arrow = if *open {
        vec![
            center + vec2(-4., -2.),
            center + vec2(4., -2.),
            center + vec2(0., 4.),
        ]
    } else {
        vec![
            center + vec2(-2., -4.),
            center + vec2(-2., 4.),
            center + vec2(4., 0.),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(arrow, WALL_INK, Stroke::NONE));
    ui.painter().text(
        rect.left_center() + vec2(32., 0.),
        Align2::LEFT_CENTER,
        language.translate(title).to_uppercase(),
        FontId::proportional(13.),
        WALL_INK,
    );
    if response.clicked() {
        *open = !*open;
    }
    if *open {
        ui.add_space(6.);
        body(ui);
        ui.add_space(6.);
        ui.separator();
    }
    ui.add_space(4.);
}

fn sub_section(
    ui: &mut Ui,
    language: UiLanguage,
    title: &str,
    open: &mut bool,
    body: impl FnOnce(&mut Ui),
) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 24.), Sense::click());
    let fill = if response.hovered() {
        WALL_HEADER_HOVER
    } else {
        WALL_HEADER
    };
    ui.painter().rect_filled(rect, 1., fill);
    let center = rect.left_center() + vec2(30., 0.);
    let arrow = if *open {
        vec![
            center + vec2(-3., -2.),
            center + vec2(3., -2.),
            center + vec2(0., 3.),
        ]
    } else {
        vec![
            center + vec2(-2., -3.),
            center + vec2(-2., 3.),
            center + vec2(3., 0.),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(arrow, WALL_INK, Stroke::NONE));
    ui.painter().text(
        rect.left_center() + vec2(44., 0.),
        Align2::LEFT_CENTER,
        language.translate(title).to_uppercase(),
        FontId::proportional(12.),
        WALL_INK,
    );
    if response.clicked() {
        *open = !*open;
    }
    if *open {
        ui.add_space(4.);
        ui.indent(("wall_sub_section", title), |ui| body(ui));
        ui.add_space(4.);
    }
    ui.add_space(2.);
}

fn label(ui: &mut Ui, language: UiLanguage, text: &str) {
    ui.label(RichText::new(language.translate(text)).color(WALL_INK));
}

fn muted_note(ui: &mut Ui, language: UiLanguage, text: &str) {
    ui.label(
        RichText::new(language.translate(text))
            .small()
            .color(WALL_MUTED),
    );
}

fn number(ui: &mut Ui, language: UiLanguage, text: &str, value: &mut f32) {
    label(ui, language, text);
    ui.add(
        DragValue::new(value)
            .speed(0.005)
            .fixed_decimals(3)
            .suffix(" m"),
    );
    ui.end_row();
}

fn integer(ui: &mut Ui, language: UiLanguage, text: &str, value: &mut u16, max: u16) {
    label(ui, language, text);
    ui.add(DragValue::new(value).range(0..=max));
    ui.end_row();
}

fn text_row(ui: &mut Ui, language: UiLanguage, text: &str, value: &mut String, width: f32) {
    label(ui, language, text);
    ui.add_sized([width, 20.], egui::TextEdit::singleline(value));
    ui.end_row();
}

fn toggle(ui: &mut Ui, language: UiLanguage, text: &str, value: &mut bool) {
    ui.checkbox(value, language.translate(text));
}

fn choice_combo<T: WallChoice>(ui: &mut Ui, language: UiLanguage, id: &str, value: &mut T) {
    ComboBox::from_id_salt(id)
        .selected_text(language.translate(value.name()))
        .width(190.)
        .show_ui(ui, |ui| {
            for choice in T::ALL {
                ui.selectable_value(value, *choice, language.translate(choice.name()));
            }
        });
}

fn choice_row<T: WallChoice>(
    ui: &mut Ui,
    language: UiLanguage,
    text: &str,
    id: &str,
    value: &mut T,
) {
    label(ui, language, text);
    choice_combo(ui, language, id, value);
    ui.end_row();
}

fn choice_buttons<T: WallChoice>(ui: &mut Ui, language: UiLanguage, value: &mut T) {
    ui.horizontal_wrapped(|ui| {
        for choice in T::ALL {
            let selected = *value == *choice;
            if ui
                .add(Button::new(language.translate(choice.name())).selected(selected))
                .clicked()
            {
                *value = *choice;
            }
        }
    });
}

pub(crate) fn top_link_label(
    link: i32,
    home_story: u32,
    stories: &[Story],
    language: UiLanguage,
) -> String {
    if link == 0 {
        return language.translate("Home Story").into();
    }
    let target = home_story as i32 + link;
    let name = stories
        .iter()
        .find(|story| story.id as i32 == target)
        .map_or_else(|| language.translate("Story"), |story| story.name.as_str());
    let home = language.translate("Home Story");
    let relation = if link > 0 {
        format!("{home} + {link}")
    } else {
        format!("{home} {link}")
    };
    format!("{target}. {name} ({relation})")
}

fn geometry_body(
    ui: &mut Ui,
    language: UiLanguage,
    stories: &[Story],
    draft: &mut WallSettingsDraft,
    show_position: bool,
) {
    Grid::new("wall_geometry_grid")
        .num_columns(2)
        .spacing([16., 8.])
        .show(ui, |ui| {
            label(ui, language, "Top Link:");
            ComboBox::from_id_salt("wall_top_link")
                .selected_text(top_link_label(
                    draft.settings.geometry.top_link_offset,
                    draft.story,
                    stories,
                    language,
                ))
                .width(230.)
                .show_ui(ui, |ui| {
                    for link in -2..=3 {
                        ui.selectable_value(
                            &mut draft.settings.geometry.top_link_offset,
                            link,
                            top_link_label(link, draft.story, stories, language),
                        );
                    }
                });
            ui.end_row();

            number(ui, language, "Height", &mut draft.height);
            number(ui, language, "Thickness", &mut draft.thickness);
            number(
                ui,
                language,
                "Top Offset",
                &mut draft.settings.geometry.top_offset,
            );
            number(
                ui,
                language,
                "Bottom Offset",
                &mut draft.settings.geometry.bottom_offset,
            );

            label(ui, language, "Home Story:");
            ComboBox::from_id_salt("wall_home_story")
                .selected_text(
                    stories
                        .iter()
                        .find(|story| story.id == draft.story)
                        .map_or_else(
                            || format!("Level {:02}", draft.story),
                            |story| format!("{}. {}", story.id, story.name),
                        ),
                )
                .width(230.)
                .show_ui(ui, |ui| {
                    for story in stories {
                        ui.selectable_value(
                            &mut draft.story,
                            story.id,
                            format!("{}. {}", story.id, story.name),
                        );
                    }
                });
            ui.end_row();

            number(
                ui,
                language,
                "to Project Zero",
                &mut draft.settings.geometry.to_project_zero,
            );

            label(ui, language, "Geometry Method:");
            choice_buttons(ui, language, &mut draft.settings.geometry.geometry_method);
            ui.end_row();

            label(ui, language, "Structure:");
            ui.add_sized(
                [230., 20.],
                egui::TextEdit::singleline(&mut draft.settings.profile.name),
            );
            ui.end_row();

            label(ui, language, "Reference Line:");
            choice_buttons(ui, language, &mut draft.settings.geometry.reference_line);
            ui.end_row();

            number(
                ui,
                language,
                "Reference Offset",
                &mut draft.settings.geometry.reference_offset,
            );
        });
    if show_position {
        ui.add_space(6.);
        ui.label(
            RichText::new(language.translate("Wall Position"))
                .strong()
                .color(WALL_INK),
        );
        if let Some(position) = &mut draft.position {
            Grid::new("wall_position_grid")
                .num_columns(2)
                .spacing([16., 8.])
                .show(ui, |ui| {
                    number(ui, language, "Start X", &mut position.start.x);
                    number(ui, language, "Start Y", &mut position.start.y);
                    number(ui, language, "End X", &mut position.end.x);
                    number(ui, language, "End Y", &mut position.end.y);
                });
            let length = position.start.distance(position.end);
            let angle = (position.end.y - position.start.y)
                .atan2(position.end.x - position.start.x)
                .to_degrees();
            ui.label(
                RichText::new(format!(
                    "{} {:.3} m · {} {:.2}°",
                    language.translate("Length"),
                    length,
                    language.translate("Direction"),
                    angle
                ))
                .color(WALL_MUTED),
            );
        }
    }
    muted_note(
        ui,
        language,
        "Height, thickness, reference offset, story and position update Plan and 3D immediately.",
    );
}

fn profile_body(ui: &mut Ui, language: UiLanguage, draft: &mut WallSettingsDraft) {
    Grid::new("wall_profile_grid")
        .num_columns(2)
        .spacing([16., 8.])
        .show(ui, |ui| {
            text_row(
                ui,
                language,
                "Profile Name",
                &mut draft.settings.profile.name,
                230.,
            );
            choice_row(
                ui,
                language,
                "Origin",
                "wall_profile_origin",
                &mut draft.settings.profile.origin,
            );
            number(
                ui,
                language,
                "Horizontal Offset",
                &mut draft.settings.profile.horizontal_offset,
            );
            label(ui, language, "Enable Modifiers");
            toggle(
                ui,
                language,
                "Profile Offset Modifiers",
                &mut draft.settings.profile.modifier_enabled,
            );
            ui.end_row();
            number(
                ui,
                language,
                "Top Modifier",
                &mut draft.settings.profile.modifier_top,
            );
            number(
                ui,
                language,
                "Bottom Modifier",
                &mut draft.settings.profile.modifier_bottom,
            );
            number(
                ui,
                language,
                "Left Modifier",
                &mut draft.settings.profile.modifier_left,
            );
            number(
                ui,
                language,
                "Right Modifier",
                &mut draft.settings.profile.modifier_right,
            );
        });
    muted_note(
        ui,
        language,
        "Profile offsets are stored with the wall; profiled wall geometry is not generated yet.",
    );
}

fn floor_plan_body(ui: &mut Ui, language: UiLanguage, draft: &mut WallSettingsDraft) {
    Grid::new("wall_floor_plan_grid")
        .num_columns(2)
        .spacing([16., 8.])
        .show(ui, |ui| {
            choice_row(
                ui,
                language,
                "Show on Stories",
                "wall_show_stories",
                &mut draft.settings.floor_plan.show_on_stories,
            );
            choice_row(
                ui,
                language,
                "Floor Plan Display",
                "wall_floor_display",
                &mut draft.settings.floor_plan.floor_plan_display,
            );
            choice_row(
                ui,
                language,
                "Show Projection",
                "wall_projection",
                &mut draft.settings.floor_plan.show_projection,
            );
            label(ui, language, "Override Cut Fill Pens");
            toggle(
                ui,
                language,
                "Override Cut Fill Pens",
                &mut draft.settings.floor_plan.override_cut_fill_pens,
            );
            ui.end_row();
            text_row(
                ui,
                language,
                "Cut Fill Pen",
                &mut draft.settings.floor_plan.cut_fill_pen,
                150.,
            );
            choice_row(
                ui,
                language,
                "Uncut Lines",
                "wall_uncut_lines",
                &mut draft.settings.floor_plan.uncut_lines,
            );
            number(
                ui,
                language,
                "Uncut Line Weight",
                &mut draft.settings.floor_plan.uncut_line_weight_mm,
            );
            label(ui, language, "Uncut Line Pen");
            ui.add(
                DragValue::new(&mut draft.settings.floor_plan.uncut_line_pen_index).range(0..=255),
            );
            ui.end_row();
            choice_row(
                ui,
                language,
                "Wall End Lines",
                "wall_end_lines",
                &mut draft.settings.floor_plan.wall_end_lines,
            );
        });
    muted_note(
        ui,
        language,
        "Floor-plan display settings are stored; plan line and fill overrides are not rendered yet.",
    );
}

fn model_body(ui: &mut Ui, language: UiLanguage, draft: &mut WallSettingsDraft) {
    Grid::new("wall_model_grid")
        .num_columns(2)
        .spacing([16., 8.])
        .show(ui, |ui| {
            label(ui, language, "Override Surfaces:");
            toggle(
                ui,
                language,
                "Override Surfaces",
                &mut draft.settings.model.override_surfaces,
            );
            ui.end_row();
            text_row(
                ui,
                language,
                "Left Surface",
                &mut draft.settings.model.left_surface,
                210.,
            );
            text_row(
                ui,
                language,
                "Right Surface",
                &mut draft.settings.model.right_surface,
                210.,
            );
            text_row(
                ui,
                language,
                "End Surface",
                &mut draft.settings.model.end_surface,
                210.,
            );
            integer(
                ui,
                language,
                "Junction Order:",
                &mut draft.settings.model.junction_order,
                999,
            );
            label(ui, language, "End Surfaces");
            toggle(
                ui,
                language,
                "End Surfaces: Override using Adjoining Walls",
                &mut draft
                    .settings
                    .model
                    .end_surfaces_override_using_adjoining_walls,
            );
            ui.end_row();
            label(ui, language, "Texture");
            toggle(
                ui,
                language,
                "Align Texture to Wall Edges",
                &mut draft.settings.model.align_texture_to_wall_edges,
            );
            ui.end_row();
            number(
                ui,
                language,
                "Texture Alignment X",
                &mut draft.settings.model.texture_alignment_x,
            );
            number(
                ui,
                language,
                "Texture Alignment Y",
                &mut draft.settings.model.texture_alignment_y,
            );
            number(
                ui,
                language,
                "Texture Alignment Rotation",
                &mut draft.settings.model.texture_alignment_rotation,
            );
            choice_row(
                ui,
                language,
                "Relation To Zones:",
                "wall_zone_relation",
                &mut draft.settings.model.relation_to_zones,
            );
        });
    muted_note(
        ui,
        language,
        "Surface and texture settings are stored with the wall; rendering uses the element material.",
    );
}

fn structural_body(ui: &mut Ui, language: UiLanguage, draft: &mut WallSettingsDraft) {
    Grid::new("wall_structural_grid")
        .num_columns(2)
        .spacing([16., 8.])
        .show(ui, |ui| {
            choice_row(
                ui,
                language,
                "Structural Function:",
                "wall_structural_function",
                &mut draft.settings.structural.structural_function,
            );
            label(ui, language, "Structural Member");
            toggle(
                ui,
                language,
                "Structural Member",
                &mut draft.settings.structural.structural_member,
            );
            ui.end_row();
            label(ui, language, "Openings");
            toggle(
                ui,
                language,
                "Ignore small openings",
                &mut draft.settings.structural.ignore_small_openings,
            );
            ui.end_row();
            label(ui, language, "Connections");
            toggle(
                ui,
                language,
                "Stretch & Cutback",
                &mut draft.settings.structural.stretch_and_cutback,
            );
            ui.end_row();
            label(ui, language, "Analytical Offset");
            toggle(
                ui,
                language,
                "Offset",
                &mut draft.settings.structural.offset_to_structural_model,
            );
            ui.end_row();
            number(
                ui,
                language,
                "ez:",
                &mut draft.settings.structural.structural_offset_ez,
            );
            choice_row(
                ui,
                language,
                "Model Type",
                "wall_structural_model_type",
                &mut draft.settings.structural.model_type,
            );
            label(ui, language, "Allow Rule-based modifications");
            toggle(
                ui,
                language,
                "Allow Rule-based modifications",
                &mut draft.settings.structural.allow_rule_based_modifications,
            );
            ui.end_row();
        });
    ui.add_space(6.);
    ui.label(
        RichText::new(language.translate("Support Conditions"))
            .strong()
            .color(WALL_INK),
    );
    Grid::new("wall_support_grid")
        .num_columns(2)
        .spacing([18., 6.])
        .show(ui, |ui| {
            for (label, support) in ["Ux", "Uy", "Uz", "Fix", "Fiy", "Fiz"]
                .into_iter()
                .zip(draft.settings.structural.supports.iter_mut())
            {
                ui.label(RichText::new(label).color(WALL_INK));
                choice_combo(ui, language, &format!("wall_support_{label}"), support);
                ui.end_row();
            }
        });
    muted_note(
        ui,
        language,
        "Structural analytical settings are stored; no analytical model or support solver is generated.",
    );
}

#[allow(clippy::too_many_arguments)]
fn classification_body(
    ui: &mut Ui,
    language: UiLanguage,
    stories: &[Story],
    draft: &mut WallSettingsDraft,
    class_list: &mut bool,
    class_id: &mut bool,
    class_design: &mut bool,
    class_renovation: &mut bool,
    class_ratings: &mut bool,
    class_life: &mut bool,
    class_brick: &mut bool,
    class_ceiling: &mut bool,
    class_cost: &mut bool,
    class_definition: &mut bool,
) {
    let home_story = stories
        .iter()
        .find(|story| story.id == draft.story)
        .map_or_else(
            || format!("Level {:02}", draft.story),
            |story| story.name.clone(),
        );
    sub_section(ui, language, "Classifications", class_list, |ui| {
        Grid::new("wall_classifications_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                label(ui, language, "Classification System");
                ui.label(
                    RichText::new(language.translate("Project Classification - Wall"))
                        .color(WALL_MUTED),
                );
                ui.end_row();
                text_row(
                    ui,
                    language,
                    "Classification Name",
                    &mut draft.settings.classification.classification_name,
                    200.,
                );
                text_row(
                    ui,
                    language,
                    "Classification ID",
                    &mut draft.settings.classification.classification_id,
                    200.,
                );
                text_row(
                    ui,
                    language,
                    "IFC Type",
                    &mut draft.settings.classification.ifc_type,
                    200.,
                );
            });
    });
    sub_section(ui, language, "ID AND CATEGORIES", class_id, |ui| {
        Grid::new("wall_class_id_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                text_row(
                    ui,
                    language,
                    "ID",
                    &mut draft.settings.classification.property_id,
                    200.,
                );
                choice_row(
                    ui,
                    language,
                    "Structural Function",
                    "wall_class_structural_function",
                    &mut draft.settings.structural.structural_function,
                );
                choice_row(
                    ui,
                    language,
                    "Position",
                    "wall_class_position",
                    &mut draft.settings.classification.position,
                );
            });
    });
    sub_section(ui, language, "DESIGN OPTIONS", class_design, |ui| {
        Grid::new("wall_class_design_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                choice_row(
                    ui,
                    language,
                    "Option Status",
                    "wall_option_status",
                    &mut draft.settings.classification.option_status,
                );
                label(ui, language, "Placement");
                ui.label(RichText::new(home_story).color(WALL_MUTED));
                ui.end_row();
            });
    });
    sub_section(ui, language, "RENOVATION", class_renovation, |ui| {
        Grid::new("wall_class_renovation_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                label(ui, language, "Renovation Status");
                ComboBox::from_id_salt("wall_renovation_status")
                    .selected_text(renovation_label(language, draft.renovation))
                    .width(190.)
                    .show_ui(ui, |ui| {
                        for status in [
                            RenovationStatus::Existing,
                            RenovationStatus::Demolition,
                            RenovationStatus::New,
                        ] {
                            ui.selectable_value(
                                &mut draft.renovation,
                                status,
                                renovation_label(language, status),
                            );
                        }
                    });
                ui.end_row();
                choice_row(
                    ui,
                    language,
                    "Show On Renovation Filter",
                    "wall_renovation_filter",
                    &mut draft.settings.classification.show_on_renovation_filter,
                );
            });
    });
    sub_section(ui, language, "GENERAL RATINGS", class_ratings, |ui| {
        Grid::new("wall_class_ratings_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                text_row(
                    ui,
                    language,
                    "Fire Resistance Rating",
                    &mut draft.settings.classification.fire_resistance_rating,
                    200.,
                );
                choice_row(
                    ui,
                    language,
                    "Combustible",
                    "wall_combustible",
                    &mut draft.settings.classification.combustible,
                );
                number(
                    ui,
                    language,
                    "Thermal Transmittance",
                    &mut draft.settings.classification.thermal_transmittance,
                );
                text_row(
                    ui,
                    language,
                    "Sound Transmission Class",
                    &mut draft.settings.classification.sound_transmission_class,
                    200.,
                );
                choice_row(
                    ui,
                    language,
                    "Construction Type",
                    "wall_construction_type",
                    &mut draft.settings.classification.construction_type,
                );
                choice_row(
                    ui,
                    language,
                    "Technology",
                    "wall_construction_technology",
                    &mut draft.settings.classification.technology,
                );
                number(
                    ui,
                    language,
                    "Concrete Cover at Main Bars",
                    &mut draft.settings.classification.concrete_cover_at_main_bars,
                );
                number(
                    ui,
                    language,
                    "Lifting Weight",
                    &mut draft.settings.classification.lifting_weight,
                );
            });
    });
    sub_section(ui, language, "LIFE CYCLE ENVIRONMENTAL", class_life, |ui| {
        Grid::new("wall_class_life_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                text_row(
                    ui,
                    language,
                    "Environmental Class",
                    &mut draft.settings.classification.environmental_class,
                    200.,
                );
                label(ui, language, "Service Life");
                ui.add(
                    DragValue::new(&mut draft.settings.classification.service_life_years)
                        .range(0..=500)
                        .suffix(" yr"),
                );
                ui.end_row();
                number(
                    ui,
                    language,
                    "Stored Energy",
                    &mut draft.settings.classification.stored_energy_kwh_m3,
                );
            });
    });
    sub_section(ui, language, "BRICK TYPE AND MEASURES", class_brick, |ui| {
        Grid::new("wall_class_brick_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                text_row(
                    ui,
                    language,
                    "Brick type",
                    &mut draft.settings.classification.brick_type,
                    200.,
                );
                number(
                    ui,
                    language,
                    "Brick H size",
                    &mut draft.settings.classification.brick_h_size,
                );
                number(
                    ui,
                    language,
                    "Brick W size",
                    &mut draft.settings.classification.brick_w_size,
                );
                number(
                    ui,
                    language,
                    "Brick L size",
                    &mut draft.settings.classification.brick_l_size,
                );
                number(
                    ui,
                    language,
                    "Mortar thickness",
                    &mut draft.settings.classification.mortar_thickness,
                );
                number(
                    ui,
                    language,
                    "Brick volume",
                    &mut draft.settings.classification.brick_volume,
                );
                label(ui, language, "No. of bricks to purchase");
                ui.add(
                    DragValue::new(&mut draft.settings.classification.bricks_to_purchase)
                        .range(0..=10_000_000),
                );
                ui.end_row();
            });
    });
    sub_section(ui, language, "CEILING AND FLOOR", class_ceiling, |ui| {
        Grid::new("wall_class_ceiling_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                number(
                    ui,
                    language,
                    "Sub Ceiling thickness",
                    &mut draft.settings.classification.sub_ceiling_thickness,
                );
                number(
                    ui,
                    language,
                    "Sub Floor Thickness",
                    &mut draft.settings.classification.sub_floor_thickness,
                );
                number(
                    ui,
                    language,
                    "Suspended Ceiling level",
                    &mut draft.settings.classification.suspended_ceiling_level,
                );
                number(
                    ui,
                    language,
                    "Raised Floor Level",
                    &mut draft.settings.classification.raised_floor_level,
                );
                number(
                    ui,
                    language,
                    "Interior height",
                    &mut draft.settings.classification.interior_height,
                );
                number(
                    ui,
                    language,
                    "Interior height detailed",
                    &mut draft.settings.classification.interior_height_detailed,
                );
            });
    });
    sub_section(ui, language, "COST", class_cost, |ui| {
        Grid::new("wall_class_cost_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                number(
                    ui,
                    language,
                    "Cost Per Volume",
                    &mut draft.settings.classification.cost_per_volume,
                );
                number(
                    ui,
                    language,
                    "Total Cost of Volume",
                    &mut draft.settings.classification.total_cost_of_volume,
                );
                number(
                    ui,
                    language,
                    "Total Cost of Volume (USD)",
                    &mut draft.settings.classification.total_cost_usd,
                );
                number(
                    ui,
                    language,
                    "Total Cost of Volume ($)",
                    &mut draft.settings.classification.total_cost_symbol,
                );
            });
    });
    sub_section(ui, language, "CLASSIFICATION", class_definition, |ui| {
        Grid::new("wall_class_definition_grid")
            .num_columns(2)
            .spacing([16., 7.])
            .show(ui, |ui| {
                label(ui, language, "Dynamic ID");
                toggle(
                    ui,
                    language,
                    "Dynamic ID by Classification",
                    &mut draft.settings.classification.dynamic_id_by_classification,
                );
                ui.end_row();
                text_row(
                    ui,
                    language,
                    "Dynamic ID value",
                    &mut draft.settings.classification.dynamic_id_value,
                    200.,
                );
                text_row(
                    ui,
                    language,
                    "Strength Grade",
                    &mut draft.settings.classification.strength_grade,
                    200.,
                );
                text_row(
                    ui,
                    language,
                    "GlobalId (Attribute)",
                    &mut draft.settings.classification.global_id_attribute,
                    200.,
                );
                text_row(
                    ui,
                    language,
                    "Name (Attribute)",
                    &mut draft.settings.classification.name_attribute,
                    200.,
                );
                text_row(
                    ui,
                    language,
                    "PredefinedType (Attribute)",
                    &mut draft.settings.classification.predefined_type_attribute,
                    200.,
                );
            });
    });
    muted_note(
        ui,
        language,
        "Classification and property values are stored with the wall and survive save/reopen.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Material;

    fn wall() -> Element {
        let mut project = crate::model::Project::default();
        let id = project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.22,
                height: 3.,
            },
        );
        project
            .elements
            .into_iter()
            .find(|element| element.id == id)
            .unwrap()
    }

    #[test]
    fn draft_round_trip_updates_geometry_and_settings() {
        let mut element = wall();
        let mut draft = WallSettingsDraft::from_element(&element).unwrap();
        draft.height = 4.25;
        draft.thickness = 0.3;
        draft.settings.geometry.reference_offset = 0.1;
        draft.settings.geometry.top_offset = 0.5;
        draft.settings.classification.property_id = "SW - 099".into();
        draft.position = Some(WallPosition {
            start: Point::new(1., 2.),
            end: Point::new(7., 2.),
        });
        draft.apply_to_element(&mut element).unwrap();
        let Shape::Wall {
            start,
            end,
            thickness,
            height,
        } = element.shape
        else {
            panic!("wall shape expected");
        };
        assert_eq!(start, Point::new(1., 2.));
        assert_eq!(end, Point::new(7., 2.));
        assert_eq!(thickness, 0.3);
        assert_eq!(height, 4.25);
        assert_eq!(
            element
                .wall_settings
                .as_ref()
                .unwrap()
                .classification
                .property_id,
            "SW - 099"
        );
    }

    #[test]
    fn invalid_draft_is_rejected_without_mutating_the_element() {
        let element = wall();
        let mut draft = WallSettingsDraft::from_element(&element).unwrap();
        draft.height = f32::NAN;
        let mut target = element.clone();
        assert!(draft.apply_to_element(&mut target).is_err());
        assert_eq!(target, element);
    }

    #[test]
    fn generic_wall_setting_errors_are_localized() {
        assert_eq!(
            localized_error(
                UiLanguage::SimplifiedChinese,
                "Wall setting top offset is out of range"
            ),
            "墙设置超出允许范围"
        );
        assert_eq!(
            localized_error(
                UiLanguage::SimplifiedChinese,
                "Wall setting profile name is too long"
            ),
            "墙设置文本过长"
        );
        assert_eq!(
            localized_error(
                UiLanguage::English,
                "Wall setting top offset is out of range"
            ),
            "Wall setting top offset is out of range"
        );
    }
}
