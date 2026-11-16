//! Portable declared quality/loss reports. Storing a report never approves a source.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    #[default]
    Unverified,
    Rejected,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub expected_unique_handles: u64,
    pub observed_unique_handles: u64,
    pub missing_handles: u64,
    pub extra_handles: u64,
    pub duplicate_handles: u64,
}
impl Counts {
    fn validate(&self) -> Result<(), String> {
        if self.expected_unique_handles.checked_add(self.extra_handles)
            != self
                .observed_unique_handles
                .checked_add(self.missing_handles)
            || self
                .expected_unique_handles
                .checked_add(self.extra_handles)
                .is_none()
            || self.missing_handles > self.expected_unique_handles
            || self.extra_handles > self.observed_unique_handles
        {
            return Err("Source comparison counts are inconsistent or overflow".into());
        }
        Ok(())
    }
    pub fn mismatch(&self) -> bool {
        self.missing_handles > 0 || self.extra_handles > 0 || self.duplicate_handles > 0
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub subject_id: String,
    pub reference_source_id: Option<String>,
    pub method: String,
    pub scope: String,
    pub summary: String,
    pub limits: String,
    pub verdict: Verdict,
    pub comparison: Option<Counts>,
    pub evidence_source_ids: Vec<String>,
}
impl Draft {
    pub fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        let exists = |id: &str| manifest.sources.iter().any(|s| s.id == id);
        if !exists(&self.subject_id)
            || self.method.trim().is_empty()
            || self.method.len() > 256
            || self.scope.trim().is_empty()
            || self.scope.len() > 1024
            || self.summary.trim().is_empty()
            || self.summary.len() > 4096
            || self.limits.trim().is_empty()
            || self.limits.len() > 4096
            || self.evidence_source_ids.len() > 100
        {
            return Err("Source review needs an existing subject, method, scope, summary and explicit limits".into());
        }
        if let Some(id) = &self.reference_source_id
            && (id == &self.subject_id || !exists(id))
        {
            return Err("Source review reference must be a different stored source".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for id in &self.evidence_source_ids {
            if !exists(id) || !ids.insert(id) || id == &self.subject_id {
                return Err(
                    "Source review evidence is missing, duplicated or is its subject".into(),
                );
            }
        }
        if let Some(counts) = &self.comparison {
            counts.validate()?;
            if self.reference_source_id.is_none() {
                return Err("Handle comparison needs an explicit reference source".into());
            }
            if manifest
                .sources
                .iter()
                .find(|s| s.id == self.subject_id)
                .is_none_or(|s| s.format != "dxf")
                || manifest
                    .sources
                    .iter()
                    .find(|s| Some(&s.id) == self.reference_source_id.as_ref())
                    .is_none_or(|s| !matches!(s.format.as_str(), "dwg" | "dxf"))
            {
                return Err("Handle counts require a DXF subject and DWG or DXF reference".into());
            }
            if counts.mismatch() && self.verdict != Verdict::Rejected {
                return Err("Handle mismatch requires a rejected verdict".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema_version: u32,
    pub operation_id: String,
    pub draft: Draft,
}
impl Review {
    pub fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        if self.schema_version != 1 || !storage::valid_document_identity(&self.operation_id) {
            return Err("Invalid source review version or operation identity".into());
        }
        self.draft.validate(manifest)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReviewOperation {
    NotFound,
    Published { review: Review },
    Unpublished { review: Review },
}
impl Repository {
    pub(crate) fn read_review(&self, item: &Resource) -> Result<Review, String> {
        let bytes = resource(&self.root, item, 64 * 1024)?;
        let review: Review = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        review.validate(&self.manifest)?;
        Ok(review)
    }
    pub fn source_reviews(&self, subject: Option<&str>) -> Result<Vec<Review>, String> {
        self.unchanged()?;
        self.validate()?;
        self.manifest
            .reviews
            .iter()
            .map(|item| self.read_review(item))
            .filter(|r| {
                r.as_ref()
                    .map_or(true, |r| subject.is_none_or(|s| s == r.draft.subject_id))
            })
            .collect()
    }
    /// Read the original operation, including an artifact not reached by the manifest.
    pub fn review_operation(&self, id: &str) -> Result<ReviewOperation, String> {
        if !storage::valid_document_identity(id) {
            return Err("Invalid source review operation identity".into());
        }
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        let relative = format!("reviews/{id}.json");
        let path = safe_path(&self.root, &relative)?;
        if !path.try_exists().map_err(|e| e.to_string())? {
            if self.manifest.reviews.iter().any(|r| r.path == relative) {
                return Err("Published source review resource is missing".into());
            }
            return Ok(ReviewOperation::NotFound);
        }
        let bytes = read_bounded(&path, 64 * 1024)?;
        let item = Resource {
            path: relative,
            sha256: hash(&bytes),
            bytes: bytes.len() as u64,
        };
        let review = self.read_review(&item)?;
        if review.operation_id != id {
            return Err("Source review operation identity mismatch".into());
        }
        if let Some(stored) = self.manifest.reviews.iter().find(|r| r.path == item.path) {
            self.read_review(stored)?;
            Ok(ReviewOperation::Published { review })
        } else {
            Ok(ReviewOperation::Unpublished { review })
        }
    }
    pub fn record_source_review(
        &mut self,
        operation_id: &str,
        draft: Draft,
    ) -> Result<Review, String> {
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        self.validate()?;
        let review = Review {
            schema_version: 1,
            operation_id: operation_id.into(),
            draft,
        };
        review.validate(&self.manifest)?;
        let relative = format!("reviews/{operation_id}.json");
        if let Some(item) = self.manifest.reviews.iter().find(|r| r.path == relative) {
            let original = self.read_review(item)?;
            return if original == review {
                Ok(original)
            } else {
                Err("Source review operation identity already has different input".into())
            };
        }
        if self.manifest.reviews.len() >= 200 {
            return Err("Project exceeds 200 source reviews".into());
        }
        if fs::symlink_metadata(safe_path(&self.root, &relative)?).is_ok() {
            return Err(
                "Unpublished review operation exists; inspect its original result before recovery"
                    .into(),
            );
        }
        let bytes = serde_json::to_vec_pretty(&review).map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("Source review exceeds 64 KiB".into());
        }
        directory(&self.root, "reviews")?;
        let item = publish_blob(&self.root, relative, &bytes)?;
        let mut next = self.clone();
        next.manifest.reviews.push(item);
        next.manifest.schema_version = 2;
        if review.draft.verdict == Verdict::Rejected {
            let source = next
                .manifest
                .sources
                .iter_mut()
                .find(|s| s.id == review.draft.subject_id)
                .ok_or("Missing reviewed source")?;
            if matches!(source.quality, SourceQuality::Unreviewed) {
                source.quality = SourceQuality::Rejected {
                    reason: review.draft.summary.clone(),
                };
            }
        }
        next.manifest.revision = next
            .manifest
            .revision
            .checked_add(1)
            .ok_or("Repository revision overflow")?;
        next.validate()?;
        next.publish()?;
        *self = next;
        Ok(review)
    }
}
