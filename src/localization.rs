use serde_json::Value;
use std::{fs, path::PathBuf, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiLanguage {
    #[default]
    English,
    SimplifiedChinese,
}

impl UiLanguage {
    /// Presentation only: the original structured reason remains in the report.
    pub fn geometry_diagnostic_message(
        self,
        diagnostic: &crate::derived_geometry::GeometryDiagnostic,
    ) -> String {
        let code = diagnostic.code.as_str();
        let key = if code == "COMPARISON_STALE" {
            "Comparison objects changed; reopen the current comparison"
        } else if code == "COMPARISON_BOUNDS" {
            "Comparison geometry has no bounds"
        } else if code == "COMPARISON_SOURCE_INVALID" {
            match diagnostic.message.as_str() {
                "Candidate 3D preview needs 1–32 unique ascending object IDs" => {
                    "Candidate 3D preview needs 1–32 unique ascending object IDs"
                }
                "Comparison object is missing" => "Comparison object is missing",
                "Candidate 3D preview supports concept masses and Zones only" => {
                    "Candidate 3D preview supports concept masses and Zones only"
                }
                _ => "Cannot generate the geometry. Check the element parameters and story.",
            }
        } else if code.contains("CLIP") {
            "Cannot display the clipping region. Check its outline and height limits."
        } else if code.contains("POLYLINE") {
            "Cannot generate the polyline. Check its nodes and arc radii."
        } else if code.contains("OPENING") {
            "Cannot generate the opening. Check its host wall and dimensions."
        } else if code.contains("WALL") {
            "Cannot generate the wall. Check its dimensions, connections and openings."
        } else if code.contains("SLAB") {
            "Cannot generate the slab. Check its outline, holes and edge slopes."
        } else if code.contains("ROOF") {
            "Cannot generate the roof. Check its outline, pivot and pitch."
        } else if code == "INVALID_COLUMN_PLAN_CONTEXT" {
            "Cannot display the column plan. Check the cut plane and projection range."
        } else if code.contains("COLUMN") {
            "Cannot generate the column. Check its section, height and inclination."
        } else if code.contains("BEAM") {
            "Cannot generate the beam. Check its axis, section and slope."
        } else if code.contains("STAIR") {
            "Cannot generate the stair. Check its run, rise and steps."
        } else {
            "Cannot generate the geometry. Check the element parameters and story."
        };
        let reason = self.translate(key);
        if diagnostic.element_id == 0 {
            reason.to_owned()
        } else {
            self.translate("Element {id}: {reason}")
                .replace("{id}", &diagnostic.element_id.to_string())
                .replace("{reason}", reason)
        }
    }

    pub(crate) fn geometry_error_message(self, message: &str) -> Option<String> {
        crate::geometry_validation::diagnostic_from_message(message)
            .map(|d| self.geometry_diagnostic_message(&d))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
        }
    }

    pub fn translate(self, key: &str) -> &str {
        if self == Self::English {
            let dictionary =
                ENGLISH.get_or_init(|| serde_json::from_str(EN).unwrap_or(Value::Null));
            return dictionary.get(key).and_then(Value::as_str).unwrap_or(key);
        }
        let dictionary = ZH.get_or_init(|| serde_json::from_str(ZH_CN).unwrap_or(Value::Null));
        dictionary.get(key).and_then(Value::as_str).unwrap_or(key)
    }

    pub fn load_preference() -> Self {
        preference_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| value == "zh-CN")
            .map_or(Self::English, |_| Self::SimplifiedChinese)
    }

    pub fn save_preference(self) {
        if let Some(path) = preference_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let value = if self == Self::SimplifiedChinese {
                "zh-CN"
            } else {
                "en"
            };
            let _ = fs::write(path, value);
        }
    }
}

fn preference_path() -> Option<PathBuf> {
    crate::platform::config_dir().map(|directory| directory.join("language"))
}

const EN: &str = include_str!("../locales/en.json");
const ZH_CN: &str = include_str!("../locales/zh-CN.json");
static ZH: OnceLock<Value> = OnceLock::new();
static ENGLISH: OnceLock<Value> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::{EN, UiLanguage, ZH_CN};

    #[test]
    fn view_option_labels_have_bilingual_translations() {
        for (english, chinese) in [
            ("Axonometry", "轴测图"),
            ("Basic", "基本"),
            ("Vectorial", "矢量"),
            ("Simple Shading", "简单着色"),
            ("Technical", "技术"),
            ("Detailed", "详细"),
            ("Shadows", "阴影"),
            ("White Model", "白模"),
            ("Wireframe", "线框"),
            ("Orbit", "轨道旋转"),
            ("Explore Model", "漫游模型"),
        ] {
            assert_eq!(UiLanguage::English.translate(english), english);
            assert_eq!(UiLanguage::SimplifiedChinese.translate(english), chinese);
        }
    }

    #[test]
    fn all_model_material_display_names_have_bilingual_entries() {
        let english: serde_json::Value = serde_json::from_str(EN).unwrap();
        let chinese: serde_json::Value = serde_json::from_str(ZH_CN).unwrap();
        for material in crate::model::Material::ALL {
            let key = material.name();
            assert_eq!(english.get(key).and_then(|v| v.as_str()), Some(key));
            assert!(chinese.get(key).and_then(|v| v.as_str()).is_some());
            assert_ne!(UiLanguage::SimplifiedChinese.translate(key), key);
            assert_eq!(UiLanguage::English.translate(key), key);
        }
    }

    #[test]
    fn both_language_packs_resolve_core_labels_and_fallback_unknown_keys() {
        assert_eq!(UiLanguage::SimplifiedChinese.translate("Wall"), "墙");
        assert_eq!(UiLanguage::English.translate("Wall"), "Wall");
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Untranslated key"),
            "Untranslated key"
        );
    }

    #[test]
    fn untitled_document_name_is_localized_for_the_window_title() {
        assert_eq!(UiLanguage::English.translate("Untitled"), "Untitled");
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Untitled"),
            "未命名"
        );
    }

    #[test]
    fn wall_drawing_hints_name_right_click_finishing() {
        use crate::wall::WallGeometryMethod;
        for method in [
            WallGeometryMethod::Continuous,
            WallGeometryMethod::Polygonal,
        ] {
            let key = method.status_key();
            assert!(key.contains("right-click"), "{key}");
            let chinese = UiLanguage::SimplifiedChinese.translate(key);
            assert!(chinese.contains("右键"), "{chinese}");
            assert_ne!(chinese, key);
        }
    }

    #[test]
    fn wall_reference_line_editing_labels_are_localized() {
        for key in [
            "Wall reference line",
            "Start endpoint",
            "End endpoint",
            "Reference line",
            "Drag the handle to reshape the wall",
            "Drag the line to move the wall",
            "Done",
            "Wall reference-line endpoint edit · drag the handle to reshape the wall",
            "Wall reference-line edit · drag the line to move the wall",
            "Wall reference-line endpoint moved",
            "Wall moved",
        ] {
            let chinese = UiLanguage::SimplifiedChinese.translate(key);
            assert_ne!(chinese, key, "missing Simplified Chinese label for {key}");
        }
    }

    #[test]
    fn quick_options_labels_are_localized_in_both_language_packs() {
        let labels = [
            "Zoom",
            "Orientation",
            "Scale",
            "Layer Combination",
            "Partial Structure",
            "Pen Set",
            "Model View Options",
            "Graphic Overrides",
            "Renovation Filter",
            "Design Options",
            "Dimensions",
            "All Layers",
            "Main Model Only",
            "Save Current Zoom…",
            "Set Orientation Graphically…",
            "Custom Scale…",
            "Layers (Model Views)…",
            "Show All Layers",
            "Partial Structure Display…",
            "Pens & Color (Model Views)…",
            "Model View Options…",
            "Graphic Override Combinations…",
            "Renovation Filter Options…",
            "Reset Renovation Status…",
            "Design Option Manager",
            "Entire Model",
            "Without Finishes",
            "Core Only",
            "03 Architectural 100",
            "No Overrides",
            "00 Show All Elements",
            "Plain Meter",
            "Quick Options bar shown",
            "Quick Options palette hidden",
        ];
        let english: serde_json::Value = serde_json::from_str(EN).unwrap();
        let chinese: serde_json::Value = serde_json::from_str(ZH_CN).unwrap();
        for label in labels {
            assert!(
                english.get(label).is_some(),
                "missing English dictionary entry for {label}"
            );
            assert!(
                chinese.get(label).is_some(),
                "missing Simplified Chinese dictionary entry for {label}"
            );
            assert_ne!(
                UiLanguage::SimplifiedChinese.translate(label),
                label,
                "Simplified Chinese dictionary still returns the English label for {label}"
            );
        }
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Layer Combination"),
            "图层组合"
        );
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Graphic Overrides"),
            "图形覆盖"
        );
        assert_eq!(
            UiLanguage::English.translate("Layer Combination"),
            "Layer Combination"
        );
    }

    #[test]
    fn file_dialog_labels_are_localized_in_both_language_packs() {
        let labels = [
            "Open project",
            "New project",
            "Unsaved changes will be replaced. Save first if you need them.",
            "Open / replace",
            "Create / replace",
            "OpenArchi project",
        ];
        let english: serde_json::Value = serde_json::from_str(EN).unwrap();
        let chinese: serde_json::Value = serde_json::from_str(ZH_CN).unwrap();
        for label in labels {
            assert!(
                english.get(label).is_some(),
                "missing English entry for {label}"
            );
            assert!(
                chinese.get(label).is_some(),
                "missing Chinese entry for {label}"
            );
            assert_ne!(UiLanguage::SimplifiedChinese.translate(label), label);
        }
    }
    #[test]
    fn wall_settings_labels_and_choices_are_localized_in_both_language_packs() {
        let labels = [
            "Wall Settings",
            "Wall Default Settings",
            "GEOMETRY AND POSITIONING",
            "PROFILE OFFSET MODIFIERS",
            "FLOOR PLAN AND SECTION",
            "MODEL",
            "STRUCTURAL ANALYTICAL PARAMETERS",
            "CLASSIFICATION AND PROPERTIES",
            "Top Link:",
            "Home Story",
            "Reference Line:",
            "Reference Offset",
            "Wall Position",
            "Profile Name",
            "Horizontal Offset",
            "Show on Stories",
            "Floor Plan Display",
            "Show Projection",
            "Uncut Lines",
            "Wall End Lines",
            "Override Surfaces",
            "Left Surface",
            "Right Surface",
            "End Surface",
            "Junction Order:",
            "Structural Function",
            "Structural Member",
            "Model Type",
            "Allow Rule-based modifications",
            "Classifications",
            "Classification Name",
            "Option Status",
            "Renovation Status",
            "Fire Resistance Rating",
            "Thermal Transmittance",
            "Construction Type",
            "Technology",
            "Service Life",
            "Brick type",
            "Brick H size",
            "Sub Ceiling thickness",
            "Cost Per Volume",
            "Dynamic ID value",
            "Strength Grade",
            "IFC Type",
            "Center",
            "Inside",
            "Outside",
            "Single",
            "Continuous",
            "Polygonal",
            "Rectangular",
            "Top",
            "Bottom",
            "All Relevant Stories",
            "Home Story Only",
            "Custom",
            "Projected",
            "Cut Only",
            "Symbolic",
            "To Floor Plan Range",
            "Entire Model",
            "Lower Story",
            "Solid",
            "Dashed",
            "None",
            "Both",
            "Start Only",
            "End Only",
            "Zone Boundary",
            "No Relation",
            "Manual",
            "Undefined",
            "Bearing",
            "Non-Bearing",
            "Shear",
            "Non-Structural",
            "Shell",
            "Beam",
            "Column",
            "Plate",
            "Rigid",
            "Free",
            "Hinged",
            "Fixed",
            "Interior",
            "Exterior",
            "Above",
            "Below",
            "Main Model",
            "Design Option",
            "Not Set",
            "All Relevant Filters",
            "Home Filter Only",
            "Hidden",
            "Non-Combustible",
            "Brick",
            "Concrete",
            "Timber",
            "Steel",
            "Composite",
            "Cast in Place",
            "Precast",
            "Masonry",
            "Framed",
            "Existing",
            "Demolition",
            "New",
            "Wall Settings cancelled",
            "Wall settings applied · Plan and 3D synchronized",
            "Wall default settings applied · New walls use these values",
        ];
        let english: serde_json::Value = serde_json::from_str(EN).unwrap();
        let chinese: serde_json::Value = serde_json::from_str(ZH_CN).unwrap();
        for label in labels {
            assert!(
                english.get(label).is_some(),
                "missing English dictionary entry for {label}"
            );
            assert!(
                chinese.get(label).is_some(),
                "missing Simplified Chinese dictionary entry for {label}"
            );
            assert_ne!(
                UiLanguage::SimplifiedChinese.translate(label),
                label,
                "Simplified Chinese dictionary still returns the English label for {label}"
            );
        }
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Wall Settings"),
            "墙设置"
        );
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Floor Plan Display"),
            "平面显示"
        );
    }
    #[test]
    fn main_window_and_status_labels_are_localized_in_chinese() {
        for (key, expected) in [
            ("Select an element or start drawing", "选择构件或开始绘制"),
            ("Viewpoint", "视点"),
            ("South Elevation view preview", "南立面视图预览"),
            ("Metres · Z up", "米制 · Z 轴向上"),
            ("Project saved", "项目已保存"),
            ("Navigator item", "导航器项目"),
            ("interface preview", "界面预览"),
            ("Grid Snap off", "网格捕捉关"),
        ] {
            assert_eq!(
                UiLanguage::SimplifiedChinese.translate(key),
                expected,
                "missing or stale Simplified Chinese label for {key}"
            );
        }
    }
}

#[cfg(test)]
mod save_menu_translation_tests {
    #[test]
    fn save_as_label_resolves_in_both_languages() {
        use super::UiLanguage;
        assert_eq!(UiLanguage::English.translate("Save As…"), "Save As…");
        assert_eq!(
            UiLanguage::SimplifiedChinese.translate("Save As…"),
            "另存为…"
        );
    }
}

#[cfg(test)]
mod geometry_diagnostic_translation_tests {
    use super::*;
    #[test]
    fn every_supported_geometry_family_and_clip_has_actionable_bilingual_errors() {
        for family in [
            "POLYLINE",
            "WALL",
            "SLAB",
            "ROOF",
            "COLUMN",
            "COLUMN_PLAN_CONTEXT",
            "BEAM",
            "STAIR",
            "OPENING",
            "VIEW_CLIP",
            "UNKNOWN",
        ] {
            let d = crate::derived_geometry::GeometryDiagnostic::error(
                17,
                format!("INVALID_{family}"),
                "body",
                "raw backend reason",
            );
            let en = UiLanguage::English.geometry_diagnostic_message(&d);
            let zh = UiLanguage::SimplifiedChinese.geometry_diagnostic_message(&d);
            assert_ne!(en, zh);
            assert!(en.starts_with("Element 17:"));
            assert!(zh.starts_with("构件 17："));
            assert!(!en.contains("backend"));
            assert!(!zh.contains("backend"));
            assert!(en.contains("Check"));
            assert!(zh.contains("请检查"));
        }
    }
}
