//! Versioned mode preference, separate from project storage and model history.
use super::{EnvironmentState, WorkMode};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
const LIMIT: u64 = 16 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preference {
    schema_version: u32,
    mode: WorkMode,
}

fn read(path: &Path) -> Result<Option<Preference>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Work mode preference is too large".into());
    }
    let preference: Preference = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if preference.schema_version != 1 {
        return Err("Unsupported work mode preference version".into());
    }
    Ok(Some(preference))
}

pub fn load(path: &Path) -> Result<EnvironmentState, String> {
    Ok(EnvironmentState {
        mode: read(path)?.map_or(WorkMode::Architecture, |p| p.mode),
    })
}

pub fn save(path: &Path, mode: WorkMode) -> Result<(), String> {
    // Preserve damaged/newer files for explicit recovery instead of silently replacing them.
    read(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("Preference directory required")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(
        ".work-mode-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let bytes = serde_json::to_vec_pretty(&Preference {
            schema_version: 1,
            mode,
        })
        .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
