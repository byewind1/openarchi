use serde_json::Value;
use std::{fs, path::PathBuf, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiLanguage {
    #[default]
    English,
    SimplifiedChinese,
}

impl UiLanguage {
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
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library/Application Support/OpenArchi")
            .join("language")
    })
}

const EN: &str = include_str!("../locales/en.json");
const ZH_CN: &str = include_str!("../locales/zh-CN.json");
static ZH: OnceLock<Value> = OnceLock::new();
static ENGLISH: OnceLock<Value> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::{EN, UiLanguage, ZH_CN};

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
