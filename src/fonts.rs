//! Cross-platform Simplified Chinese font discovery.
//!
//! The UI prefers a font installed by the operating system so that Chinese text
//! follows the native look. A bundled Noto Sans SC copy guarantees readable
//! Chinese text when no suitable system font exists, notably on a minimal
//! Windows or Linux installation.

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

/// Noto Sans SC Regular, licensed under the SIL Open Font License 1.1.
/// See `assets/fonts/README.md` and `assets/fonts/OFL.txt`.
pub const BUNDLED_CJK_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansSC-Regular.ttf");

pub const WINDOWS_CJK_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyhbd.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
    r"C:\Windows\Fonts\Deng.ttf",
];

pub const MACOS_CJK_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "/Library/Fonts/NotoSansCJKsc-Regular.otf",
    "/Library/Fonts/NotoSansCHI-Regular.ttf",
    "/Library/Fonts/NotoSansCHI-Bold.ttf",
];

pub const LINUX_CJK_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/wenquanyi/wqy-zenhei/wqy-zenhei.ttc",
];

/// Candidate paths for the host platform, in preference order.
pub fn platform_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        WINDOWS_CJK_CANDIDATES
    }
    #[cfg(target_os = "macos")]
    {
        MACOS_CJK_CANDIDATES
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        LINUX_CJK_CANDIDATES
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        &[]
    }
}

/// Read the first candidate that exists and is readable.
pub fn first_readable(candidates: &[&str]) -> Option<(String, Vec<u8>)> {
    candidates.iter().find_map(|path| {
        std::fs::read(path)
            .ok()
            .map(|bytes| ((*path).to_owned(), bytes))
    })
}

/// Read the preferred installed CJK font, or return `None` when the bundled
/// fallback should be used.
pub fn installed_cjk_font() -> Option<Vec<u8>> {
    first_readable(platform_candidates()).map(|(_, bytes)| bytes)
}

/// Install the CJK font into egui's proportional and monospace families.
///
/// The font is inserted after the default family entry so Latin text keeps the
/// original typography and missing CJK glyphs fall through to this font.
pub fn install_cjk(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let font = match installed_cjk_font() {
        Some(bytes) => FontData::from_owned(bytes),
        None => FontData::from_static(BUNDLED_CJK_FONT),
    };
    fonts.font_data.insert("OpenArchi-CJK".into(), font.into());
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(order) = fonts.families.get_mut(&family) {
            order.insert(1.min(order.len()), "OpenArchi-CJK".into());
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openarchi-fonts-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn windows_candidates_include_common_simplified_chinese_fonts() {
        for name in ["msyh.ttc", "simhei.ttf", "simsun.ttc", "Deng.ttf"] {
            assert!(
                WINDOWS_CJK_CANDIDATES
                    .iter()
                    .any(|path| path.ends_with(name)),
                "missing Windows CJK candidate {name}"
            );
        }
        assert_eq!(WINDOWS_CJK_CANDIDATES[0], r"C:\Windows\Fonts\msyh.ttc");
    }

    #[test]
    fn platform_candidate_list_is_nonempty_and_unique() {
        assert!(!platform_candidates().is_empty());
        let mut seen = std::collections::HashSet::new();
        for path in platform_candidates() {
            assert!(seen.insert(*path), "duplicate font candidate {path}");
        }
    }

    #[test]
    fn first_readable_skips_missing_files_and_returns_first_present() {
        let dir = temp_dir("first-readable");
        let missing = dir.join("missing.ttf");
        let present = dir.join("present.ttf");
        std::fs::write(&present, b"font-bytes").unwrap();
        let candidates = [
            missing.to_str().unwrap(),
            present.to_str().unwrap(),
            present.to_str().unwrap(),
        ];
        let (path, bytes) = first_readable(&candidates).expect("readable candidate");
        assert_eq!(path, present.to_str().unwrap());
        assert_eq!(bytes, b"font-bytes");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bundled_font_is_a_nonempty_truetype_font() {
        assert!(BUNDLED_CJK_FONT.len() > 1_000_000);
        let magic = &BUNDLED_CJK_FONT[..4];
        assert!(
            matches!(magic, b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"ttcf"),
            "unexpected bundled font signature: {magic:?}"
        );
    }
}
