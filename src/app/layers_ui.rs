//! AC29-style Layer Settings dialog (Cmd+L): combinations panel on the
//! left, discipline tree plus status table on the right, original vector
//! icons for eye / lock / 3D display. Layout and semantics follow
//! `docs/features/layer-settings/contract.md`.

use super::*;
use crate::layers::{
    ARCHICAD_LAYER, CATEGORY_DESIGN, CategoryId, CombinationId, LayerDocument, LayerId,
};
use eframe::egui::Shape;
use std::collections::{BTreeMap, BTreeSet};

const LAYERS_ACCENT: Color32 = Color32::from_rgb(74, 160, 211);
const LAYERS_ROW_HOVER: Color32 = Color32::from_rgb(55, 60, 62);
const LAYERS_ROW_SELECTED: Color32 = Color32::from_rgb(52, 84, 106);
const LAYERS_ERROR: Color32 = Color32::from_rgb(224, 125, 105);
const LAYERS_INK: Color32 = Color32::from_rgb(226, 230, 232);
const LAYERS_DIM: Color32 = Color32::from_rgb(140, 147, 151);
const LAYERS_ICON: Color32 = Color32::from_rgb(190, 198, 202);
const LAYERS_ICON_OFF: Color32 = Color32::from_rgb(110, 116, 120);
const LAYERS_FOLDER: Color32 = Color32::from_rgb(126, 152, 172);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerSort {
    NameAsc,
    NameDesc,
    GroupAsc,
    GroupDesc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayerSettingsOutcome {
    None,
    Ok,
    Cancel,
}

/// Working copy of the layer document plus dialog-only UI state. OK applies
/// the selected combination and commits through one reversible command;
/// Cancel drops everything.
#[derive(Clone, Debug)]
pub struct LayerSettingsState {
    pub draft: LayerDocument,
    pub selected_category: Option<CategoryId>,
    pub expanded: BTreeSet<CategoryId>,
    pub search: String,
    pub sort: LayerSort,
    pub selected_combination: Option<CombinationId>,
    pub selected_layer: Option<LayerId>,
    pub renaming: Option<LayerId>,
    pub rename_buffer: String,
    pub delete_warning: Option<LayerId>,
    pub error: String,
}

impl LayerSettingsState {
    pub fn new(document: &LayerDocument) -> Self {
        let mut expanded = BTreeSet::new();
        expanded.insert(CATEGORY_DESIGN);
        Self {
            draft: document.clone(),
            selected_category: None,
            expanded,
            search: String::new(),
            sort: LayerSort::NameAsc,
            selected_combination: document.active_combination,
            selected_layer: Some(ARCHICAD_LAYER),
            renaming: None,
            rename_buffer: String::new(),
            delete_warning: None,
            error: String::new(),
        }
    }

    /// Table rows for the current tree selection and search filter.
    fn table_layers(&self) -> Vec<LayerId> {
        let mut ids = match self.selected_category {
            Some(category) => self.draft.subtree_layer_ids(category),
            None => self.draft.root_layer_ids(),
        };
        let needle = self.search.trim().to_lowercase();
        if !needle.is_empty() {
            ids.retain(|id| {
                self.draft
                    .layer(*id)
                    .is_some_and(|layer| layer.name.to_lowercase().contains(&needle))
            });
        }
        let key = |id: LayerId| -> (String, u8) {
            self.draft
                .layer(id)
                .map(|layer| (layer.name.to_lowercase(), layer.group))
                .unwrap_or_default()
        };
        match self.sort {
            LayerSort::NameAsc => ids.sort_by_key(|id| key(*id).0),
            LayerSort::NameDesc => ids.sort_by_key(|id| std::cmp::Reverse(key(*id).0)),
            LayerSort::GroupAsc => ids.sort_by_key(|id| key(*id).1),
            LayerSort::GroupDesc => ids.sort_by_key(|id| std::cmp::Reverse(key(*id).1)),
        }
        ids
    }
}

// --- Original vector icons (eye, lock, 3D display, folder, triangle) ---

fn eye_icon(painter: &Painter, rect: Rect, color: Color32, open: bool) {
    let center = rect.center();
    let radius = rect.size().min_elem() * 0.4;
    if open {
        let left = center + vec2(-radius * 1.35, 0.);
        let right = center + vec2(radius * 1.35, 0.);
        painter.add(Shape::line(
            vec![
                left,
                pos2(center.x - radius * 0.6, center.y - radius),
                pos2(center.x + radius * 0.6, center.y - radius),
                right,
                pos2(center.x + radius * 0.6, center.y + radius),
                pos2(center.x - radius * 0.6, center.y + radius),
                left,
            ],
            Stroke::new(1.4, color),
        ));
        painter.circle_filled(center, radius * 0.42, color);
    } else {
        let left = center + vec2(-radius * 1.3, 0.);
        let right = center + vec2(radius * 1.3, 0.);
        painter.add(Shape::line(
            vec![
                left,
                pos2(center.x - radius * 0.5, center.y - radius * 0.55),
                pos2(center.x + radius * 0.5, center.y - radius * 0.55),
                right,
                pos2(center.x + radius * 0.5, center.y + radius * 0.55),
                pos2(center.x - radius * 0.5, center.y + radius * 0.55),
                left,
            ],
            Stroke::new(1.4, color),
        ));
        for offset in [-0.8f32, 0., 0.8] {
            painter.line_segment(
                [
                    center + vec2(offset * radius, radius * 0.3),
                    center + vec2(offset * radius, radius * 0.85),
                ],
                Stroke::new(1.2, color),
            );
        }
    }
}

fn lock_icon(painter: &Painter, rect: Rect, color: Color32, locked: bool) {
    let center = rect.center();
    let s = rect.size().min_elem() * 0.3;
    let body = Rect::from_center_size(center + vec2(0., s * 0.45), vec2(s * 1.5, s * 1.1));
    painter.rect_filled(body, 2., color);
    let shackle_radius = s * 0.62;
    let shackle_center = pos2(body.center().x, body.top() + shackle_radius * 0.15);
    if locked {
        painter.circle_stroke(shackle_center, shackle_radius, Stroke::new(1.4, color));
    } else {
        // Open shackle: arc plus a lifted free end.
        painter.add(Shape::line(
            vec![
                pos2(
                    shackle_center.x - shackle_radius,
                    shackle_center.y + s * 0.35,
                ),
                pos2(
                    shackle_center.x - shackle_radius,
                    shackle_center.y - shackle_radius * 0.2,
                ),
                pos2(shackle_center.x, shackle_center.y - shackle_radius),
                pos2(shackle_center.x + shackle_radius, shackle_center.y),
                pos2(
                    shackle_center.x + shackle_radius,
                    shackle_center.y + s * 0.7,
                ),
            ],
            Stroke::new(1.4, color),
        ));
    }
}

fn model_display_icon(painter: &Painter, rect: Rect, color: Color32, solid: bool) {
    let size = rect.size().min_elem() * 0.62;
    let cube = Rect::from_center_size(rect.center(), vec2(size, size));
    if solid {
        painter.rect_filled(cube, 1.5, color);
    } else {
        painter.rect_stroke(cube, 1.5, Stroke::new(1.3, color), StrokeKind::Inside);
        painter.line_segment(
            [cube.left_top(), cube.right_bottom()],
            Stroke::new(1.1, color),
        );
    }
}

fn folder_icon(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.size().min_elem() * 0.42;
    let center = rect.center();
    let tab = Rect::from_min_max(
        pos2(center.x - s, center.y - s * 0.72),
        pos2(center.x - s * 0.2, center.y - s * 0.28),
    );
    painter.rect_filled(tab, 1.5, color);
    let body = Rect::from_min_max(
        pos2(center.x - s, center.y - s * 0.28),
        pos2(center.x + s, center.y + s * 0.72),
    );
    painter.rect_filled(body, 2., color);
}

fn triangle_icon(painter: &Painter, rect: Rect, color: Color32, expanded: bool) {
    let center = rect.center();
    let s = rect.size().min_elem() * 0.28;
    let points = if expanded {
        vec![
            pos2(center.x - s, center.y - s * 0.7),
            pos2(center.x + s, center.y - s * 0.7),
            pos2(center.x, center.y + s * 0.7),
        ]
    } else {
        vec![
            pos2(center.x - s * 0.7, center.y - s),
            pos2(center.x - s * 0.7, center.y + s),
            pos2(center.x + s * 0.8, center.y),
        ]
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

fn check_mark(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.size().min_elem() * 0.28;
    painter.add(Shape::line(
        vec![
            pos2(c.x - s, c.y),
            pos2(c.x - s * 0.25, c.y + s * 0.75),
            pos2(c.x + s, c.y - s * 0.75),
        ],
        Stroke::new(1.8, color),
    ));
}

/// One small icon cell in the layer table. Returns true when toggled.
fn icon_toggle(ui: &mut Ui, enabled: bool, draw: impl FnOnce(&Painter, Rect, Color32)) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(22., 18.), Sense::click());
    let (bg, color) = if !enabled {
        (Color32::TRANSPARENT, LAYERS_ICON_OFF)
    } else if response.hovered() {
        (LAYERS_ROW_HOVER, LAYERS_INK)
    } else {
        (Color32::TRANSPARENT, LAYERS_ICON)
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 3., bg);
    }
    draw(ui.painter(), rect, color);
    enabled && response.clicked()
}

impl AtelierApp {
    pub(crate) fn open_layer_settings(&mut self) {
        self.layer_settings = Some(LayerSettingsState::new(&self.editor.project.layers));
        self.editor.status = self.language.translate("Layer Settings").into();
    }

    pub(super) fn layer_settings_dialog(&mut self, ctx: &Context) {
        let Some(mut state) = self.layer_settings.take() else {
            return;
        };
        let language = self.language;
        let element_layers: BTreeMap<LayerId, usize> =
            self.editor
                .project
                .elements
                .iter()
                .fold(BTreeMap::new(), |mut counts, element| {
                    *counts.entry(element.layer.0).or_default() += 1;
                    counts
                });
        let mut outcome = LayerSettingsOutcome::None;
        let modal = Modal::new(Id::new("layer_settings_dialog")).show(ctx, |ui| {
            state.error.clear();
            ui.set_min_width(730.);
            ui.set_max_width(730.);
            ui.heading(format!(
                "{} {}",
                language.translate("Layers"),
                language.translate("(Model Views)")
            ));
            ui.separator();
            // The modal's content ui has no intrinsic height, and `ui.horizontal`
            // caps its row at the current line height, so scroll areas inside
            // collapse to a couple of rows. Reserve the AC29-style fixed body and
            // give each panel a full-height side rect explicitly.
            ui.allocate_ui(vec2(718., 420.), |ui| {
                let full = ui.available_rect_before_wrap();
                let left = Rect::from_min_size(full.min, vec2(252., full.height()));
                let right = Rect::from_min_size(
                    full.min + vec2(262., 0.),
                    vec2(full.width() - 262., full.height()),
                );
                ui.painter().vline(
                    left.right() + 5.,
                    full.y_range(),
                    Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(left)
                        .layout(Layout::top_down(Align::LEFT)),
                    |ui| Self::layer_combinations_panel(ui, &mut state, language),
                );
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(right)
                        .layout(Layout::top_down(Align::LEFT)),
                    |ui| Self::layer_tree_and_table(ui, &mut state, language, &element_layers),
                );
            });
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                outcome = LayerSettingsOutcome::Cancel;
            }
            Self::layer_settings_footer(ui, &mut state, language, &mut outcome);
        });
        match outcome {
            LayerSettingsOutcome::Ok => {
                // Selecting a combination already applied its snapshot to the
                // working copy; OK commits that state as-is.
                // Elements on deleted layers move to the Archicad Layer (the
                // AC catch-all) as part of the same reversible change.
                let remap: Vec<(u64, crate::layers::ElementLayer)> = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|element| state.draft.layer(element.layer.0).is_none())
                    .map(|element| (element.id, crate::layers::ElementLayer(ARCHICAD_LAYER)))
                    .collect();
                let moved = remap.len();
                self.editor.set_layer_document(state.draft, &remap);
                self.editor.status = if moved > 0 {
                    format!(
                        "{} · {moved} {} {}",
                        self.language.translate("Layer Settings"),
                        self.language.translate("element(s) moved to"),
                        self.language.translate("Archicad Layer")
                    )
                } else {
                    format!(
                        "{} {}",
                        self.language.translate("Layer Settings"),
                        self.language.translate("updated")
                    )
                };
            }
            LayerSettingsOutcome::Cancel => {
                self.editor.status = format!(
                    "{} {}",
                    language.translate("Layer Settings"),
                    language.translate("cancelled")
                );
            }
            LayerSettingsOutcome::None => {
                self.layer_settings = Some(state);
            }
        }
        if modal.should_close() && outcome == LayerSettingsOutcome::None {
            self.layer_settings = None;
        }
    }

    fn layer_combinations_panel(ui: &mut Ui, state: &mut LayerSettingsState, language: UiLanguage) {
        ui.vertical(|ui| {
            ui.set_width(252.);
            ui.label(
                RichText::new(language.translate("Layer Combinations"))
                    .strong()
                    .color(LAYERS_ACCENT),
            );
            ui.add_space(4.);
            let effective = state.draft.effective_combination();
            let selected = state.selected_combination;
            ScrollArea::vertical()
                .id_salt("layer_combinations_list")
                .auto_shrink([false, false])
                .max_height(326.)
                .show_rows(ui, 22., state.draft.combinations.len(), |ui, range| {
                    for row in range {
                        let combination = &state.draft.combinations[row];
                        let is_selected = selected == Some(combination.id);
                        let is_effective = effective == Some(combination.id);
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(ui.available_width(), 22.), Sense::click());
                        if is_selected {
                            ui.painter().rect_filled(rect, 3., LAYERS_ROW_SELECTED);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 3., LAYERS_ROW_HOVER);
                        }
                        if is_effective {
                            check_mark(
                                ui.painter(),
                                Rect::from_min_size(rect.left_top() + vec2(6., 4.), vec2(14., 14.)),
                                LAYERS_ACCENT,
                            );
                        }
                        ui.painter().text(
                            rect.left_top() + vec2(28., 11.),
                            Align2::LEFT_CENTER,
                            language.translate(&combination.name),
                            FontId::default(),
                            LAYERS_INK,
                        );
                        if response.clicked() {
                            let id = combination.id;
                            state.draft.apply_combination(id);
                            state.selected_combination = Some(id);
                        }
                    }
                });
            ui.add_space(4.);
            ui.horizontal(|ui| {
                if ui.button(language.translate("New…")).clicked() {
                    let id = state
                        .draft
                        .new_combination(language.translate("New Combination"));
                    state.selected_combination = Some(id);
                }
                let update_enabled = state
                    .selected_combination
                    .is_some_and(|id| state.draft.combinations.iter().any(|c| c.id == id));
                ui.add_enabled_ui(update_enabled, |ui| {
                    if ui.button(language.translate("Update")).clicked() {
                        if let Some(id) = state.selected_combination {
                            state.draft.update_combination(id);
                        }
                    }
                });
            });
        });
    }

    fn layer_tree_and_table(
        ui: &mut Ui,
        state: &mut LayerSettingsState,
        language: UiLanguage,
        element_layers: &BTreeMap<LayerId, usize>,
    ) {
        ui.vertical(|ui| {
            ui.set_min_width(440.);
            // Search field with a magnifier glyph.
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut state.search)
                        .hint_text(language.translate("Search Layers"))
                        .desired_width(192.),
                );
                let (rect, _) = ui.allocate_exact_size(vec2(14., 14.), Sense::hover());
                let painter = ui.painter_at(rect);
                painter.circle_stroke(
                    pos2(
                        rect.left() + rect.width() * 0.4,
                        rect.top() + rect.height() * 0.4,
                    ),
                    rect.width() * 0.32,
                    Stroke::new(1.3, LAYERS_DIM),
                );
                painter.line_segment(
                    [
                        pos2(
                            rect.left() + rect.width() * 0.62,
                            rect.top() + rect.height() * 0.62,
                        ),
                        rect.right_bottom() - vec2(1., 1.),
                    ],
                    Stroke::new(1.3, LAYERS_DIM),
                );
            });
            ui.add_space(3.);

            // Discipline tree (folder view).
            let mut tree_selection = state.selected_category;
            let mut expanded = state.expanded.clone();
            ScrollArea::vertical()
                .id_salt("layer_category_tree")
                .auto_shrink([false, false])
                .max_height(100.)
                .show(ui, |ui| {
                    Self::tree_row(
                        ui,
                        language,
                        None,
                        0,
                        tree_selection.is_none(),
                        language.translate("Layers").to_owned(),
                        true,
                        &mut expanded,
                        &mut tree_selection,
                    );
                    Self::tree_children(
                        ui,
                        language,
                        &state.draft,
                        None,
                        1,
                        &mut expanded,
                        &mut tree_selection,
                    );
                });
            state.expanded = expanded;
            if tree_selection != state.selected_category {
                state.selected_category = tree_selection;
                state.selected_layer = None;
            }

            ui.add_space(3.);
            ui.separator();

            // Status table.
            let ids = state.table_layers();
            let shown_count = ids.len();
            let mut toggles: Vec<(LayerId, u8)> = Vec::new();
            let mut group_edits: Vec<(LayerId, u8)> = Vec::new();
            let sort = state.sort;
            ScrollArea::vertical()
                .id_salt("layer_table")
                .auto_shrink([false, false])
                .max_height(252.)
                .show(ui, |ui| {
                    ui.set_min_width(430.);
                    Self::layer_table_header(ui, language, sort, &mut state.sort);
                    for id in ids {
                        let Some(layer) = state.draft.layer(id).cloned() else {
                            continue;
                        };
                        let is_selected = state.selected_layer == Some(id);
                        let renaming = state.renaming == Some(id);
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(ui.available_width(), 22.), Sense::click());
                        if is_selected {
                            ui.painter().rect_filled(rect, 2., LAYERS_ROW_SELECTED);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 2., LAYERS_ROW_HOVER);
                        }
                        if response.clicked() {
                            state.selected_layer = Some(id);
                            state.renaming = None;
                        }
                        if response.double_clicked() {
                            state.renaming = Some(id);
                            state.rename_buffer = layer.name.clone();
                        }

                        let row = rect.left_top() + vec2(2., 2.);
                        if icon_toggle(ui, !layer.protected, |painter, rect, color| {
                            eye_icon(painter, rect, color, layer.status.visible)
                        }) {
                            toggles.push((id, 0));
                        }
                        if icon_toggle(ui, !layer.protected, |painter, rect, color| {
                            lock_icon(painter, rect, color, layer.status.locked)
                        }) {
                            toggles.push((id, 1));
                        }
                        if icon_toggle(ui, true, |painter, rect, color| {
                            model_display_icon(painter, rect, color, layer.status.solid_3d)
                        }) {
                            toggles.push((id, 2));
                        }

                        let name_rect = Rect::from_min_size(
                            row + vec2(72., 0.),
                            vec2((rect.width() - 140.).max(120.), 18.),
                        );
                        if renaming {
                            let edit = TextEdit::singleline(&mut state.rename_buffer)
                                .desired_width(name_rect.width().min(200.));
                            let mut response = None;
                            ui.scope_builder(UiBuilder::new().max_rect(name_rect), |ui| {
                                response = Some(ui.add(edit))
                            });
                            let response = response.unwrap();
                            if response.lost_focus() {
                                match state.draft.rename_layer(id, &state.rename_buffer) {
                                    Ok(()) => state.renaming = None,
                                    Err(error) => state.error = error,
                                }
                            }
                        } else {
                            let text_color = if layer.status.visible {
                                LAYERS_INK
                            } else {
                                LAYERS_DIM
                            };
                            let mut label = layer.name.clone();
                            if layer.protected {
                                label.push_str("  ⌂");
                            }
                            if layer.status.locked {
                                label.push_str("  🔒");
                            }
                            ui.painter().text(
                                name_rect.left_center(),
                                Align2::LEFT_CENTER,
                                label,
                                FontId::default(),
                                text_color,
                            );
                            if let Some(count) = element_layers.get(&id).filter(|count| **count > 0)
                            {
                                ui.painter().text(
                                    name_rect.right_center() + vec2(-2., 0.),
                                    Align2::RIGHT_CENTER,
                                    format!("({count})"),
                                    FontId::proportional(10.),
                                    LAYERS_DIM,
                                );
                            }
                        }

                        let group_rect =
                            Rect::from_min_size(rect.right_top() + vec2(-58., 1.), vec2(54., 20.));
                        let mut group = layer.group;
                        let answer = DragValue::new(&mut group).range(0..=99).speed(0.25);
                        let mut changed = false;
                        ui.scope_builder(
                            UiBuilder::new()
                                .max_rect(group_rect)
                                .layout(Layout::right_to_left(Align::Center)),
                            |ui| changed = ui.add_sized([52., 20.], answer).changed(),
                        );
                        if changed {
                            group_edits.push((id, group));
                        }
                    }
                });
            for (id, which) in toggles {
                match which {
                    0 => state.toggle_visible(id),
                    1 => state.toggle_locked(id),
                    _ => state.toggle_solid(id),
                }
            }
            for (id, group) in group_edits {
                if let Some(layer) = state.draft.layer_mut(id) {
                    layer.group = group;
                }
            }

            // Footer: Show: X of Y, New (with menu), Delete.
            ui.add_space(4.);
            ui.horizontal(|ui| {
                let total = match state.selected_category {
                    Some(category) => state.draft.subtree_layer_ids(category).len(),
                    None => state.draft.layers.len(),
                };
                ui.label(
                    RichText::new(format!(
                        "{}: {} / {}",
                        language.translate("Show"),
                        shown_count,
                        total
                    ))
                    .small()
                    .color(LAYERS_DIM),
                );
                let new_layer = ui.button(language.translate("New")).clicked();
                let delete_enabled = state.selected_layer.is_some_and(|id| id != ARCHICAD_LAYER);
                ui.add_enabled_ui(delete_enabled, |ui| {
                    if ui.button(language.translate("Delete")).clicked() {
                        state.delete_warning = state.selected_layer;
                        state.error.clear();
                    }
                });
                ui.menu_button("▾", |ui| {
                    if ui.button(language.translate("New Layer")).clicked() {
                        let category =
                            state
                                .selected_category
                                .unwrap_or_else(|| match state.selected_layer {
                                    Some(id) => state
                                        .draft
                                        .layer(id)
                                        .and_then(|layer| layer.category)
                                        .unwrap_or(CATEGORY_DESIGN),
                                    None => CATEGORY_DESIGN,
                                });
                        let id = state
                            .draft
                            .new_layer(language.translate("New Layer"), Some(category));
                        state.selected_layer = Some(id);
                        state.renaming = Some(id);
                        state.rename_buffer = state
                            .draft
                            .layer(id)
                            .map(|layer| layer.name.clone())
                            .unwrap_or_default();
                        ui.close();
                    }
                    if ui.button(language.translate("New Folder")).clicked() {
                        let parent = match state.selected_category {
                            Some(CATEGORY_DESIGN) => Some(CATEGORY_DESIGN),
                            Some(other) => state
                                .draft
                                .category(other)
                                .and_then(|category| category.parent)
                                .or(Some(other)),
                            None => None,
                        };
                        let id = state
                            .draft
                            .new_category(language.translate("New Folder"), parent);
                        state.selected_category = Some(id);
                        state.expanded.insert(id);
                        ui.close();
                    }
                });
                if new_layer {
                    let category = state.selected_category.or_else(|| {
                        state
                            .selected_layer
                            .and_then(|id| state.draft.layer(id).and_then(|layer| layer.category))
                    });
                    let id = state
                        .draft
                        .new_layer(language.translate("New Layer"), category);
                    state.selected_layer = Some(id);
                }
            });
            if let Some(id) = state.delete_warning {
                let count = element_layers.get(&id).copied().unwrap_or(0);
                let name = state
                    .draft
                    .layer(id)
                    .map(|layer| layer.name.clone())
                    .unwrap_or_default();
                ui.add_space(4.);
                ui.colored_label(
                    LAYERS_ERROR,
                    format!(
                        "{} \"{}\" · {} {} {}",
                        language.translate("Delete layer"),
                        name,
                        count,
                        language.translate("element(s) move to"),
                        language.translate("Archicad Layer")
                    ),
                );
                ui.horizontal(|ui| {
                    if ui.button(language.translate("Delete")).clicked() {
                        match state.draft.delete_layer(id) {
                            Ok(()) => {
                                state.delete_warning = None;
                                state.selected_layer = Some(ARCHICAD_LAYER);
                            }
                            Err(error) => state.error = error,
                        }
                    }
                    if ui.button(language.translate("Cancel")).clicked() {
                        state.delete_warning = None;
                    }
                });
            }
            if !state.error.is_empty() {
                ui.colored_label(LAYERS_ERROR, &state.error);
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn tree_children(
        ui: &mut Ui,
        language: UiLanguage,
        document: &LayerDocument,
        parent: Option<CategoryId>,
        depth: usize,
        expanded: &mut BTreeSet<CategoryId>,
        selection: &mut Option<CategoryId>,
    ) {
        for category_id in document.child_categories(parent) {
            let Some(category) = document.category(category_id) else {
                continue;
            };
            let has_children = !document.child_categories(Some(category_id)).is_empty();
            let show =
                has_children || document.subtree_layer_ids(category_id).len() > 0 || depth <= 1;
            if !show {
                continue;
            }
            Self::tree_row(
                ui,
                language,
                Some(category_id),
                depth,
                *selection == Some(category_id),
                language.translate(&category.name).to_owned(),
                has_children,
                expanded,
                selection,
            );
            if has_children && expanded.contains(&category_id) {
                Self::tree_children(
                    ui,
                    language,
                    document,
                    Some(category_id),
                    depth + 1,
                    expanded,
                    selection,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tree_row(
        ui: &mut Ui,
        language: UiLanguage,
        category: Option<CategoryId>,
        depth: usize,
        selected: bool,
        label: String,
        has_children: bool,
        expanded: &mut BTreeSet<CategoryId>,
        selection: &mut Option<CategoryId>,
    ) {
        let indent = 4. + depth as f32 * 16.;
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 20.), Sense::click());
        if selected {
            ui.painter().rect_filled(rect, 3., LAYERS_ROW_SELECTED);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 3., LAYERS_ROW_HOVER);
        }
        let mut triangle_clicked = false;
        if has_children {
            let triangle_rect =
                Rect::from_min_size(rect.left_top() + vec2(indent, 4.), vec2(12., 12.));
            triangle_icon(
                ui.painter(),
                triangle_rect,
                LAYERS_ICON,
                category.is_some_and(|id| expanded.contains(&id)),
            );
            triangle_clicked = ui
                .interact(
                    triangle_rect,
                    Id::new(("layers_tree_triangle", category)),
                    Sense::click(),
                )
                .clicked();
        }
        let icon_rect =
            Rect::from_min_size(rect.left_top() + vec2(indent + 16., 3.), vec2(14., 14.));
        folder_icon(ui.painter(), icon_rect, LAYERS_FOLDER);
        ui.painter().text(
            rect.left_top() + vec2(indent + 36., 10.),
            Align2::LEFT_CENTER,
            label,
            FontId::default(),
            LAYERS_INK,
        );
        if triangle_clicked {
            match category {
                Some(id) => {
                    if expanded.contains(&id) {
                        expanded.remove(&id);
                    } else {
                        expanded.insert(id);
                    }
                }
                None => {
                    // The root toggles the top-level categories.
                    for id in [CATEGORY_DESIGN] {
                        if expanded.contains(&id) {
                            expanded.remove(&id);
                        } else {
                            expanded.insert(id);
                        }
                    }
                }
            }
        } else if response.clicked() {
            *selection = category;
        }
        let _ = language;
    }

    fn layer_table_header(
        ui: &mut Ui,
        language: UiLanguage,
        current: LayerSort,
        sort: &mut LayerSort,
    ) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(22., 18.), Sense::hover());
            eye_icon(ui.painter(), rect, LAYERS_DIM, true);
            let (rect, _) = ui.allocate_exact_size(vec2(22., 18.), Sense::hover());
            lock_icon(ui.painter(), rect, LAYERS_DIM, true);
            let (rect, _) = ui.allocate_exact_size(vec2(22., 18.), Sense::hover());
            model_display_icon(ui.painter(), rect, LAYERS_DIM, true);
            let name_label = match current {
                LayerSort::NameAsc => format!("{} ▲", language.translate("Name")),
                LayerSort::NameDesc => format!("{} ▼", language.translate("Name")),
                _ => language.translate("Name").to_owned(),
            };
            if ui
                .selectable_label(
                    matches!(current, LayerSort::NameAsc | LayerSort::NameDesc),
                    name_label,
                )
                .clicked()
            {
                *sort = match current {
                    LayerSort::NameAsc => LayerSort::NameDesc,
                    _ => LayerSort::NameAsc,
                };
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let group_label = match current {
                    LayerSort::GroupAsc => format!("{} ▲", language.translate("Extension")),
                    LayerSort::GroupDesc => format!("{} ▼", language.translate("Extension")),
                    _ => language.translate("Extension").to_owned(),
                };
                if ui
                    .selectable_label(
                        matches!(current, LayerSort::GroupAsc | LayerSort::GroupDesc),
                        group_label,
                    )
                    .clicked()
                {
                    *sort = match current {
                        LayerSort::GroupAsc => LayerSort::GroupDesc,
                        _ => LayerSort::GroupAsc,
                    };
                }
            });
        });
        ui.separator();
    }

    fn layer_settings_footer(
        ui: &mut Ui,
        state: &mut LayerSettingsState,
        language: UiLanguage,
        outcome: &mut LayerSettingsOutcome,
    ) {
        ui.add_space(8.);
        ui.separator();
        if let Some(id) = state.selected_combination {
            if let Some(combination) = state.draft.combinations.iter().find(|c| c.id == id) {
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        language.translate("Combination"),
                        language.translate(&combination.name)
                    ))
                    .small()
                    .color(LAYERS_DIM),
                );
            }
        }
        ui.horizontal(|ui| {
            ui.set_width(726.);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [94., 30.],
                        Button::new(language.translate("OK")).fill(LAYERS_ACCENT),
                    )
                    .clicked()
                {
                    *outcome = LayerSettingsOutcome::Ok;
                }
                if ui.button(language.translate("Cancel")).clicked() {
                    *outcome = LayerSettingsOutcome::Cancel;
                }
            });
        });
    }
}

impl LayerSettingsState {
    fn toggle_visible(&mut self, id: LayerId) {
        if let Some(layer) = self.draft.layer(id) {
            let mut status = layer.status;
            status.visible = !status.visible;
            self.draft.set_status(id, status);
        }
    }

    fn toggle_locked(&mut self, id: LayerId) {
        if let Some(layer) = self.draft.layer(id) {
            let mut status = layer.status;
            status.locked = !status.locked;
            self.draft.set_status(id, status);
        }
    }

    fn toggle_solid(&mut self, id: LayerId) {
        if let Some(layer) = self.draft.layer(id) {
            let mut status = layer.status;
            status.solid_3d = !status.solid_3d;
            self.draft.set_status(id, status);
        }
    }
}
