use std::path::PathBuf;

pub const RENDERER_LABEL: &str = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    "Apple Silicon / Metal"
} else if cfg!(target_os = "macos") {
    "Intel Mac / Metal"
} else if cfg!(target_os = "windows") {
    "Windows x64 / DirectX 12"
} else {
    "Desktop / Vulkan"
};

pub const RENDERER_BACKEND: &str = if cfg!(target_os = "macos") {
    "Metal"
} else if cfg!(target_os = "windows") {
    "DirectX 12"
} else {
    "Vulkan"
};

pub fn config_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OPENARCHI_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Some(path);
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library/Application Support")
                .join("OpenArchi")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("OpenArchi"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|root| root.join("openarchi"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openarchi"))
    }
}

pub fn documents_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Documents"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Documents"))
    }
}
