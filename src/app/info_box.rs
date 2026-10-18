use super::*;

impl AtelierApp {
    pub(super) fn selected_single_wall(&self) -> Option<Element> {
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
    pub(super) fn sync_info_box_wall(&mut self) {
        let selected_id = self.selected_single_wall().map(|element| element.id);
        if let Some(state) = &self.info_box_wall
            && state.dirty
            && Some(state.element_id) != selected_id
        {
            // Archicad's Info Box applies edits as they are made. If the
            // selection changes while a field still has focus, apply the
            // pending wall before loading the next selection.
            if !self.commit_info_box_wall() {
                return;
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
    pub(super) fn commit_info_box_wall(&mut self) -> bool {
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
                    && let Some(draft) = WallSettingsDraft::from_element(updated)
                {
                    state.draft = draft;
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
    pub(super) fn apply_wall_default_draft(&mut self, draft: &WallSettingsDraft) {
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
    pub(super) fn sync_info_box_wall_tool(&mut self) {
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
    pub(super) fn commit_info_box_wall_tool(&mut self) -> bool {
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
    pub(super) fn toggle_info_box_visibility(&mut self) {
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
    pub(super) fn open_info_box_wall_settings(&mut self, floor_plan: bool) {
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
    pub(super) fn info_box_drag(
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
    pub(super) fn info_box_tool_drag(
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
    pub(super) fn info_box_wall_properties(&mut self, ui: &mut Ui) {
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
    pub(super) fn info_box_wall_tool_properties(&mut self, ui: &mut Ui) {
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
}

impl AtelierApp {
    pub(super) fn selected_single_opening(&self) -> Option<Element> {
        let selected = self.editor.selection_ids();
        if selected.len() != 1 {
            return None;
        }
        self.editor
            .project
            .elements
            .iter()
            .find(|element| {
                element.id == selected[0] && matches!(element.shape, Shape::Opening { .. })
            })
            .cloned()
    }

    pub(super) fn sync_info_box_opening(&mut self) {
        let selected_id = self.selected_single_opening().map(|element| element.id);
        if let Some(state) = &self.info_box_opening
            && state.dirty
            && Some(state.element_id) != selected_id
            && !self.commit_info_box_opening()
        {
            return;
        }
        let Some(element) = self.selected_single_opening() else {
            self.info_box_opening = None;
            return;
        };
        let Shape::Opening {
            host_wall,
            kind,
            width,
            height,
            sill,
            offset,
        } = element.shape
        else {
            self.info_box_opening = None;
            return;
        };
        let refresh = match &self.info_box_opening {
            Some(state) => {
                state.element_id != element.id
                    || (!state.dirty
                        && (state.width != width
                            || state.height != height
                            || state.sill != sill
                            || state.offset != offset))
            }
            None => true,
        };
        if refresh {
            self.info_box_opening = Some(InfoBoxOpeningState {
                element_id: element.id,
                host_wall,
                kind,
                width,
                height,
                sill,
                offset,
                dirty: false,
                error: String::new(),
            });
        }
    }

    pub(super) fn commit_info_box_opening(&mut self) -> bool {
        let Some(mut state) = self.info_box_opening.take() else {
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
                .translate("The selected opening no longer exists")
                .into();
            self.info_box_opening = Some(state);
            return false;
        };
        if let Shape::Opening {
            width,
            height,
            sill,
            offset,
            ..
        } = &mut element.shape
        {
            *width = state.width;
            *height = state.height;
            *sill = state.sill;
            *offset = state.offset;
        }
        match self.editor.replace_element(element) {
            Ok(()) => {
                if let Some(updated) = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|element| element.id == state.element_id)
                    && let Shape::Opening {
                        width,
                        height,
                        sill,
                        offset,
                        ..
                    } = updated.shape
                {
                    state.width = width;
                    state.height = height;
                    state.sill = sill;
                    state.offset = offset;
                }
                state.dirty = false;
                state.error.clear();
                self.editor.status = self
                    .language
                    .translate("Info Box opening settings applied · Plan and 3D synchronized")
                    .into();
                self.info_box_opening = Some(state);
                true
            }
            Err(error) => {
                state.dirty = true;
                state.error = localized_error(self.language, &error);
                self.info_box_opening = Some(state);
                false
            }
        }
    }

    pub(super) fn sync_info_box_opening_tool(&mut self) {
        let kind = match self.editor.tool {
            Tool::Door => Some(OpeningKind::Door),
            Tool::Window => Some(OpeningKind::Window),
            _ => None,
        };
        let active = kind.is_some() && self.editor.selection_ids().is_empty();
        if !active {
            if self
                .info_box_opening_tool
                .as_ref()
                .is_some_and(|state| state.dirty)
            {
                let _ = self.commit_info_box_opening_tool();
            }
            self.info_box_opening_tool = None;
            return;
        }
        let Some(kind) = kind else { return };
        let defaults = self.editor.opening_defaults(kind);
        let refresh = match &self.info_box_opening_tool {
            Some(state) => state.kind != kind || (!state.dirty && state.defaults != defaults),
            None => true,
        };
        if refresh {
            self.info_box_opening_tool = Some(InfoBoxOpeningToolState {
                kind,
                defaults,
                dirty: false,
                error: String::new(),
            });
        }
    }

    pub(super) fn commit_info_box_opening_tool(&mut self) -> bool {
        let Some(mut state) = self.info_box_opening_tool.take() else {
            return false;
        };
        match state.defaults.validate() {
            Ok(()) => {
                self.editor.set_opening_defaults(state.kind, state.defaults);
                state.dirty = false;
                state.error.clear();
                self.editor.status = self
                    .language
                    .translate("Opening defaults applied · New openings use these values")
                    .into();
                self.info_box_opening_tool = Some(state);
                true
            }
            Err(error) => {
                state.dirty = true;
                state.error = localized_error(self.language, &error);
                self.info_box_opening_tool = Some(state);
                false
            }
        }
    }

    pub(super) fn info_box_opening_properties(&mut self, ui: &mut Ui) {
        let language = self.language;
        let mut commit = false;
        {
            let Some(state) = self.info_box_opening.as_mut() else {
                return;
            };
            let mut dirty = state.dirty;
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{} {:03}",
                    language.translate(state.kind.name()),
                    state.element_id
                ))
                .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "{}: {:03}",
                    language.translate("Host Wall"),
                    state.host_wall
                ))
                .small()
                .color(MUTED),
            );
            ui.separator();
            Self::info_box_drag(
                ui,
                language,
                "Width",
                &mut state.width,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Height",
                &mut state.height,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Sill Height",
                &mut state.sill,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Offset",
                &mut state.offset,
                &mut dirty,
                &mut commit,
            );
            if !state.error.is_empty() {
                ui.label(
                    RichText::new(&state.error)
                        .small()
                        .color(Color32::LIGHT_RED),
                );
            }
            state.dirty = dirty;
        }
        if commit {
            let _ = self.commit_info_box_opening();
        }
    }

    pub(super) fn info_box_opening_tool_properties(&mut self, ui: &mut Ui) {
        let language = self.language;
        let mut commit = false;
        {
            let Some(state) = self.info_box_opening_tool.as_mut() else {
                return;
            };
            let mut dirty = state.dirty;
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    language.translate(state.kind.name()),
                    language.translate("Opening")
                ))
                .strong(),
            );
            Self::info_box_drag(
                ui,
                language,
                "Width",
                &mut state.defaults.width,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Height",
                &mut state.defaults.height,
                &mut dirty,
                &mut commit,
            );
            Self::info_box_drag(
                ui,
                language,
                "Sill Height",
                &mut state.defaults.sill,
                &mut dirty,
                &mut commit,
            );
            if !state.error.is_empty() {
                ui.label(
                    RichText::new(&state.error)
                        .small()
                        .color(Color32::LIGHT_RED),
                );
            }
            state.dirty = dirty;
        }
        if commit {
            let _ = self.commit_info_box_opening_tool();
        }
    }
}
