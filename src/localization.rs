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
            let dictionary = ENGLISH.get_or_init(|| serde_json::from_str(EN).unwrap_or(Value::Null));
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
            let value = if self == Self::SimplifiedChinese { "zh-CN" } else { "en" };
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
    use super::UiLanguage;

    #[test]
    fn both_language_packs_resolve_core_labels_and_fallback_unknown_keys() {
        assert_eq!(UiLanguage::SimplifiedChinese.translate("Wall"), "墙");
        assert_eq!(UiLanguage::English.translate("Wall"), "Wall");
        assert_eq!(UiLanguage::SimplifiedChinese.translate("Untranslated key"), "Untranslated key");
    }
}
