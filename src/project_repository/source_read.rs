//! Explicit readonly access to an already attached source; never accepts a client path.
use super::*;
impl Repository {
    /// Bounded derived representation of an already admitted source. The raw
    /// source limit and verification are exactly those of existing DXF imports.
    /// Reading a rejected/unreviewed source does not adopt its geometry.
    pub fn read_dxf_outline(
        &self,
        id: &str,
        handle: &str,
        allow_constant_width_centerline: bool,
    ) -> Result<serde_json::Value, String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id && s.format == "dxf")
            .ok_or("Choose a stored DXF source")?;
        let wanted = u64::from_str_radix(handle, 16)
            .ok()
            .filter(|id| *id > 0)
            .ok_or("Invalid DXF entity handle")?;
        let entities = self.boundary_entities(id)?;
        let selected = entities
            .iter()
            .filter(|e| {
                e.handle
                    .as_deref()
                    .and_then(|h| u64::from_str_radix(h, 16).ok())
                    == Some(wanted)
            })
            .collect::<Vec<_>>();
        if selected.len() != 1 || selected[0].block.is_some() {
            return Err("Outline must be one model-space record outside block definitions; block transforms are not inferred".into());
        }
        let bytes = self.read_original(id, MAX_SOURCE)?;
        let (outline, elevation) = crate::dxf::primitives::observe_closed_outline(
            &bytes,
            handle,
            allow_constant_width_centerline,
        )?;
        self.unchanged()?;
        Ok(serde_json::json!({
            "source_sha256":id, "outline":outline, "source_plane_elevation_native":elevation,
            "confirmed_unit":source.confirmed_unit, "declared_units":source.summary.as_ref().map(|s|&s.declared_units),
            "quality":source.quality, "allow_constant_width_centerline":allow_constant_width_centerline,
            "coordinate_space":"source_native XY with separate original plane elevation; no projection, placement, scale or reference transformation applied",
            "interpretation":"Exact selected vertex path only; source declarations are not surveyed dimensions, building/retain identity or approval. Nonzero constant width is a centerline loss when explicitly allowed. No model write or source adoption."
        }))
    }
    pub fn source_context_identity(&self) -> &str {
        &self.baseline
    }
    pub fn read_original(&self, id: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id)
            .ok_or("Source not found")?;
        if source.id != source.blob.sha256 {
            return Err("Source identity differs from original blob".into());
        }
        resource(&self.root, &source.blob, max_bytes.min(MAX_SOURCE))
    }
}
