use crate::model::{ModelAdapter, Project};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
pub struct JsonAdapter;
impl ModelAdapter for JsonAdapter {
    fn format_name(&self) -> &str {
        "OpenArchi JSON v1"
    }
    fn import(&self, bytes: &[u8]) -> Result<Project, String> {
        let p: Project = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }
    fn export(&self, p: &Project) -> Result<Vec<u8>, String> {
        p.validate()?;
        serde_json::to_vec_pretty(p).map_err(|e| e.to_string())
    }
}
pub fn save(p: &Project, path: &Path) -> Result<(), String> {
    let bytes = JsonAdapter.export(p)?;
    // Each writer owns an exclusively created file in the destination directory.
    // Never truncate a leftover file or follow a pre-existing temporary symlink.
    static NEXT_SAVE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let (tmp, mut file) = loop {
        let sequence = NEXT_SAVE.fetch_add(1, Ordering::Relaxed);
        let tmp = parent.join(format!(
            ".openarchi-save-{}-{sequence}.tmp",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => break (tmp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|error| error.to_string())
}
pub fn load(path: &Path) -> Result<Project, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 32 * 1024 * 1024 {
        return Err("Project exceeds 32 MB demo limit".into());
    }
    JsonAdapter.import(&std::fs::read(path).map_err(|e| e.to_string())?)
}
