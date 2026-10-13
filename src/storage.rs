use crate::model::{ModelAdapter, Project};
use std::path::Path;
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
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
pub fn load(path: &Path) -> Result<Project, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 32 * 1024 * 1024 {
        return Err("Project exceeds 32 MB demo limit".into());
    }
    JsonAdapter.import(&std::fs::read(path).map_err(|e| e.to_string())?)
}
