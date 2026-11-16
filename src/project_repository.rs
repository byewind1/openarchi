//! Portable project envelope. Source admission never edits the parameter model.
use crate::{
    dxf::inventory::{self, InventorySummary},
    editor::Editor,
    model::Project,
    storage,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub mod boundary;
pub mod conditions;
pub mod connection;
pub mod constraints;
pub mod conversion;
pub mod evidence;
pub mod program;
pub mod quality;
pub mod rules;
pub mod source_read;
pub mod terrain;

const MAX_SOURCE: usize = 256 * 1024 * 1024;
const MAX_MANIFEST: usize = 4 * 1024 * 1024;
pub use crate::site::LengthUnit;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceQuality {
    #[default]
    Unreviewed,
    Rejected {
        reason: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub names: Vec<String>,
    pub format: String,
    pub blob: Resource,
    pub inventory: Option<Resource>,
    pub summary: Option<InventorySummary>,
    pub inventory_error: Option<String>,
    pub confirmed_unit: Option<LengthUnit>,
    #[serde(default)]
    pub quality: SourceQuality,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub document_id: String,
    pub revision: u64,
    pub model_revision: u64,
    pub model: Resource,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub conversions: Vec<Resource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviews: Vec<Resource>,
}
#[derive(Clone, Debug)]
pub struct Repository {
    root: PathBuf,
    pub manifest: Manifest,
    baseline: String,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Expected a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("File exceeds {limit} byte limit"));
    }
    Ok(bytes)
}
fn safe_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty() {
        return Err("Empty repository resource path".into());
    }
    let mut path = root.to_owned();
    for part in relative.components() {
        match part {
            Component::Normal(name) => path.push(name),
            _ => return Err("Unsafe repository resource path".into()),
        }
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && metadata.file_type().is_symlink()
        {
            return Err("Repository resource symlink rejected".into());
        }
    }
    Ok(path)
}
fn resource(root: &Path, item: &Resource, limit: usize) -> Result<Vec<u8>, String> {
    if item.bytes > limit as u64 {
        return Err("Repository resource exceeds limit".into());
    }
    let bytes = read_bounded(&safe_path(root, &item.path)?, limit)?;
    if bytes.len() as u64 != item.bytes || hash(&bytes) != item.sha256 {
        return Err(format!("Repository resource changed: {}", item.path));
    }
    Ok(bytes)
}
fn directory(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = safe_path(root, name)?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn publish_blob(root: &Path, relative: String, bytes: &[u8]) -> Result<Resource, String> {
    let item = Resource {
        path: relative,
        sha256: hash(bytes),
        bytes: bytes.len() as u64,
    };
    let path = safe_path(root, &item.path)?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            let result = file.write_all(bytes).and_then(|()| file.sync_all());
            if let Err(error) = result {
                drop(file);
                let _ = fs::remove_file(path);
                return Err(error.to_string());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            resource(root, &item, MAX_SOURCE)?;
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(item)
}
struct Lock {
    path: PathBuf,
    token: String,
}
impl Lock {
    fn acquire(root: &Path) -> Result<Self, String> {
        let path = safe_path(root, ".openarchi-repository.lock")?;
        let token = storage::new_document_identity();
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|_| {
                "Repository is locked; inspect the original writer before recovery".to_owned()
            })?;
        if let Err(error) = file
            .write_all(token.as_bytes())
            .and_then(|()| file.sync_all())
        {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(error.to_string());
        }
        Ok(Self { path, token })
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        if fs::read(&self.path).ok().as_deref() == Some(self.token.as_bytes()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
impl Repository {
    /// Portable stored snapshot; never follows unreferenced files or replaces the live Editor.
    pub fn copy_to(&self, destination: &Path) -> Result<Self, String> {
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        self.validate()?;
        fs::create_dir(destination)
            .map_err(|e| format!("Choose a new project copy folder: {e}"))?;
        let root = fs::canonicalize(destination).map_err(|e| e.to_string())?;
        let mut resources = vec![self.manifest.model.clone()];
        for source in &self.manifest.sources {
            resources.push(source.blob.clone());
            if let Some(item) = &source.inventory {
                resources.push(item.clone());
            }
        }
        resources.extend(self.manifest.conversions.clone());
        resources.extend(self.manifest.reviews.clone());
        let raw: Project = serde_json::from_slice(&resource(
            &self.root,
            &self.manifest.model,
            32 * 1024 * 1024,
        )?)
        .map_err(|e| e.to_string())?;
        let parent = Path::new(&self.manifest.model.path)
            .parent()
            .ok_or("Invalid model path")?;
        for page in raw.drawings.pages {
            let path = parent.join(page.path);
            let relative = path.to_str().ok_or("Invalid drawing path")?.to_owned();
            let bytes = read_bounded(&safe_path(&self.root, &relative)?, 32 * 1024 * 1024)?;
            if hash(&bytes) != page.sha256 {
                return Err("Drawing source changed during copy".into());
            }
            resources.push(Resource {
                path: relative,
                sha256: page.sha256,
                bytes: bytes.len() as u64,
            });
        }
        for item in resources {
            let bytes = resource(&self.root, &item, MAX_SOURCE)?;
            let parent = Path::new(&item.path)
                .parent()
                .ok_or("Invalid resource path")?
                .to_str()
                .ok_or("Invalid resource path")?;
            if !parent.is_empty() {
                directory(&root, parent)?;
            }
            publish_blob(&root, item.path, &bytes)?;
        }
        self.unchanged()?;
        let mut copy = Self {
            root,
            manifest: self.manifest.clone(),
            baseline: String::new(),
        };
        copy.validate()?;
        copy.publish()?;
        Self::open(copy.root())
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn model_path(&self) -> Result<PathBuf, String> {
        safe_path(&self.root, &self.manifest.model.path)
    }
    pub fn create_for_editor(root: &Path, editor: &Editor) -> Result<Self, String> {
        Self::create(
            root,
            &editor.project,
            editor.project_identity(),
            editor.revision,
        )
    }
    pub fn create(
        root: &Path,
        project: &Project,
        document_id: &str,
        model_revision: u64,
    ) -> Result<Self, String> {
        if !storage::valid_document_identity(document_id) {
            return Err("Invalid document identity".into());
        }
        if fs::symlink_metadata(root).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Repository root symlink rejected".into());
        }
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let _lock = Lock::acquire(&root)?;
        if fs::symlink_metadata(root.join("project.json")).is_ok() {
            return Err("Project repository already exists".into());
        }
        let model_dir = directory(&root, "model")?;
        let model_name = format!("{}.json", storage::new_document_identity());
        let model_path = model_dir.join(&model_name);
        storage::save_document(project, &model_path, document_id)?;
        let model_bytes = read_bounded(&model_path, 32 * 1024 * 1024)?;
        let manifest = Manifest {
            schema_version: 1,
            document_id: document_id.into(),
            revision: 0,
            model_revision,
            model: Resource {
                path: format!("model/{model_name}"),
                sha256: hash(&model_bytes),
                bytes: model_bytes.len() as u64,
            },
            sources: Vec::new(),
            conversions: Vec::new(),
            reviews: Vec::new(),
        };
        let mut repository = Self {
            root,
            manifest,
            baseline: String::new(),
        };
        repository.publish()?;
        repository.validate()?;
        Ok(repository)
    }
    pub fn open(root: &Path) -> Result<Self, String> {
        if fs::symlink_metadata(root)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Repository root symlink rejected".into());
        }
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let bytes = read_bounded(&safe_path(&root, "project.json")?, MAX_MANIFEST)?;
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let repository = Self {
            root,
            manifest,
            baseline: hash(&bytes),
        };
        repository.validate()?;
        Ok(repository)
    }
    pub fn load_model(&self) -> Result<Editor, String> {
        self.validate()?;
        Editor::from_stored_document(&self.model_path()?)
    }
    /// Publish an immutable snapshot of the matching document. Editor state is not owned here.
    pub fn checkpoint(
        &mut self,
        project: &Project,
        document_id: &str,
        model_revision: u64,
    ) -> Result<(), String> {
        if document_id != self.manifest.document_id {
            return Err("Repository document identity mismatch".into());
        }
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        self.validate()?;
        let model_dir = directory(&self.root, "model")?;
        let name = format!("{}.json", storage::new_document_identity());
        let path = model_dir.join(&name);
        storage::save_document(project, &path, document_id)?;
        let bytes = read_bounded(&path, 32 * 1024 * 1024)?;
        let mut next = self.clone();
        next.manifest.model = Resource {
            path: format!("model/{name}"),
            sha256: hash(&bytes),
            bytes: bytes.len() as u64,
        };
        next.manifest.model_revision = model_revision;
        next.manifest.revision = next
            .manifest
            .revision
            .checked_add(1)
            .ok_or("Repository revision overflow")?;
        next.publish()?;
        *self = next;
        Ok(())
    }
    /// Open an editable copy through the normal replacement dialog. Never bind saves
    /// to the immutable manifest snapshot. Unreferenced working copies are preserved.
    pub fn working_model_copy(&self) -> Result<PathBuf, String> {
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        let editor = self.load_model()?;
        let directory = directory(&self.root, "working")?;
        let path = directory.join(format!("{}.json", storage::new_document_identity()));
        storage::save_document(&editor.project, &path, &self.manifest.document_id)?;
        Ok(path)
    }
    fn validate(&self) -> Result<(), String> {
        if !matches!(
            (
                self.manifest.schema_version,
                self.manifest.reviews.is_empty()
            ),
            (1, true) | (2, false)
        ) || !storage::valid_document_identity(&self.manifest.document_id)
        {
            return Err("Unsupported or invalid project manifest".into());
        }
        let bytes = resource(&self.root, &self.manifest.model, 32 * 1024 * 1024)?;
        // Validate all sidecar paths before storage resolves them or reads their bytes.
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if value.get("_openarchi_document_id").and_then(|v| v.as_str())
            != Some(&self.manifest.document_id)
        {
            return Err("Repository model identity mismatch".into());
        }
        let raw: Project = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let model_parent = self
            .model_path()?
            .parent()
            .ok_or("Invalid model path")?
            .to_owned();
        for page in &raw.drawings.pages {
            let relative = page.path.to_str().ok_or("Invalid drawing path")?;
            let path = safe_path(&model_parent, relative)?;
            if !path.starts_with(&self.root) {
                return Err("Drawing escaped repository".into());
            }
        }
        let (project, _, _) = storage::load_document(&self.model_path()?)?;
        for page in &project.drawings.pages {
            crate::drawings::verified_bytes(page)?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for source in &self.manifest.sources {
            if let SourceQuality::Rejected { reason } = &source.quality
                && (reason.trim().is_empty() || reason.len() > 4096)
            {
                return Err("Invalid source rejection reason".into());
            }
            if source.id != source.blob.sha256 || !ids.insert(&source.id) {
                return Err("Invalid or duplicate source identity".into());
            }
            resource(&self.root, &source.blob, MAX_SOURCE)?;
            if let Some(item) = &source.inventory {
                let bytes = resource(&self.root, item, MAX_SOURCE)?;
                let inventory: inventory::Inventory =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if inventory.schema_version != 1
                    || source.summary.as_ref() != Some(&inventory.summary)
                {
                    return Err("Inventory summary mismatch".into());
                }
            } else if source.summary.is_some() {
                return Err("Missing source inventory".into());
            }
        }
        for item in &self.manifest.conversions {
            let bytes = resource(&self.root, item, 4 * 1024 * 1024)?;
            let receipt: conversion::ConversionReceipt =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if receipt.schema_version != 1
                || !self
                    .manifest
                    .sources
                    .iter()
                    .any(|s| s.id == receipt.source_id)
            {
                return Err("Invalid conversion receipt source".into());
            }
            if let Some(id) = &receipt.derived_source_id
                && !self.manifest.sources.iter().any(|s| &s.id == id)
            {
                return Err("Missing converted source".into());
            }
        }
        if self.manifest.reviews.len() > 200 {
            return Err("Project exceeds 200 source reviews".into());
        }
        let mut operations = std::collections::BTreeSet::new();
        for item in &self.manifest.reviews {
            let review = self.read_review(item)?;
            if !operations.insert(review.operation_id.clone())
                || item.path != format!("reviews/{}.json", review.operation_id)
            {
                return Err("Invalid or duplicate source review operation".into());
            }
            if review.draft.verdict == quality::Verdict::Rejected
                && self
                    .manifest
                    .sources
                    .iter()
                    .find(|s| s.id == review.draft.subject_id)
                    .is_none_or(|s| !matches!(s.quality, SourceQuality::Rejected { .. }))
            {
                return Err("Rejected source review cannot have a usable subject".into());
            }
        }
        Ok(())
    }
    fn unchanged(&self) -> Result<(), String> {
        if self.baseline.is_empty() {
            if fs::symlink_metadata(self.root.join("project.json")).is_ok() {
                return Err("Repository appeared during creation".into());
            }
        } else {
            let bytes = read_bounded(&safe_path(&self.root, "project.json")?, MAX_MANIFEST)?;
            if hash(&bytes) != self.baseline {
                return Err("Repository changed; reopen before updating".into());
            }
        }
        Ok(())
    }
    fn publish(&mut self) -> Result<(), String> {
        self.unchanged()?;
        let bytes = serde_json::to_vec_pretty(&self.manifest).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_MANIFEST {
            return Err("Project manifest exceeds limit".into());
        }
        let temporary = self
            .root
            .join(format!(".project-{}.tmp", storage::new_document_identity()));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let result = result.and_then(|()| fs::rename(&temporary, self.root.join("project.json")));
        if let Err(error) = result {
            let _ = fs::remove_file(temporary);
            return Err(error.to_string());
        }
        self.baseline = hash(&bytes);
        Ok(())
    }
    /// Idempotent raw-copy admission by content hash; originals are never modified.
    pub fn add_source(&mut self, path: &Path) -> Result<String, String> {
        let bytes = read_bounded(path, MAX_SOURCE)?;
        let id = hash(&bytes);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Source filename is not Unicode")?
            .to_owned();
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        self.validate()?;
        if self
            .manifest
            .sources
            .iter()
            .any(|s| s.id == id && s.names.contains(&name))
        {
            return Ok(id);
        }
        let mut next = self.clone();
        let format = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        next.admit_bytes(name, format, &bytes)?;
        next.manifest.revision = next
            .manifest
            .revision
            .checked_add(1)
            .ok_or("Repository revision overflow")?;
        next.publish()?;
        *self = next;
        Ok(id)
    }
    // Caller owns the cooperative lock; resources precede the manifest commit.
    fn admit_bytes(
        &mut self,
        name: String,
        format: String,
        bytes: &[u8],
    ) -> Result<String, String> {
        let id = hash(bytes);
        if let Some(source) = self.manifest.sources.iter_mut().find(|s| s.id == id) {
            if source.names.contains(&name) {
                return Ok(id);
            }
            source.names.push(name);
        } else {
            let extension = if matches!(
                format.as_str(),
                "dxf"
                    | "dwg"
                    | "pdf"
                    | "png"
                    | "jpg"
                    | "json"
                    | "txt"
                    | "md"
                    | "docx"
                    | "html"
                    | "htm"
            ) {
                format.as_str()
            } else {
                "bin"
            };
            directory(&self.root, "sources")?;
            let blob = publish_blob(&self.root, format!("sources/{id}.{extension}"), bytes)?;
            let mut source = Source {
                id: id.clone(),
                names: vec![name],
                format,
                blob,
                inventory: None,
                summary: None,
                inventory_error: None,
                confirmed_unit: None,
                quality: SourceQuality::Unreviewed,
            };
            if source.format == "dxf" {
                match inventory::inspect(bytes) {
                    Ok(inventory) => {
                        let bytes = serde_json::to_vec(&inventory).map_err(|e| e.to_string())?;
                        if bytes.len() > MAX_SOURCE {
                            return Err("DXF inventory exceeds storage limit".into());
                        }
                        directory(&self.root, "inventories")?;
                        source.inventory = Some(publish_blob(
                            &self.root,
                            format!("inventories/{}.json", hash(&bytes)),
                            &bytes,
                        )?);
                        source.summary = Some(inventory.summary);
                    }
                    Err(error) => source.inventory_error = Some(error),
                }
            } else if source.format == "dwg" {
                source.inventory_error =
                    Some("Raw DWG stored; conversion and entity inventory not performed".into());
            }
            self.manifest.sources.push(source);
        }
        Ok(id)
    }
    /// Quality rejection preserves bytes/receipts and cannot approve an input.
    pub fn reject_source(&mut self, id: &str, reason: &str) -> Result<(), String> {
        if reason.trim().is_empty() || reason.len() > 4096 {
            return Err("Source rejection needs a reason within 4096 bytes".into());
        }
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        self.validate()?;
        let mut next = self.clone();
        let source = next
            .manifest
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("Source not found")?;
        source.quality = SourceQuality::Rejected {
            reason: reason.trim().into(),
        };
        next.manifest.revision = next
            .manifest
            .revision
            .checked_add(1)
            .ok_or("Repository revision overflow")?;
        next.publish()?;
        *self = next;
        Ok(())
    }
    /// An explicit user declaration, independent from any file-header unit value.
    pub fn confirm_unit(&mut self, id: &str, unit: Option<LengthUnit>) -> Result<(), String> {
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        self.validate()?;
        let mut next = self.clone();
        let source = next
            .manifest
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("Source not found")?;
        source.confirmed_unit = unit;
        next.manifest.revision = next
            .manifest
            .revision
            .checked_add(1)
            .ok_or("Repository revision overflow")?;
        next.publish()?;
        *self = next;
        Ok(())
    }
}
