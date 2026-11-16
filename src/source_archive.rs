//! Shared bounded ZIP preflight. Never extracts a package onto the filesystem.
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
};
pub(crate) fn open(bytes: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>, String> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Source archive exceeds 32 MiB".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if archive.len() > 5000 {
        return Err("Source archive exceeds 5000 entries".into());
    }
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    const MAX: u64 = 64 * 1024 * 1024;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().map_err(|e| e.to_string())?;
        if !names.insert(name.to_string()) {
            return Err("Duplicate source archive entry".into());
        }
        if file.size() > MAX {
            return Err("Expanded source archive exceeds 64 MiB".into());
        }
        total += std::io::copy(&mut file.take(MAX - total + 1), &mut std::io::sink())
            .map_err(|e| e.to_string())?;
        if total > MAX {
            return Err("Expanded source archive exceeds 64 MiB".into());
        }
    }
    Ok(archive)
}
