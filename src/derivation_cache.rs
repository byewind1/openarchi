//! Derived data only. Every read borrows the authoritative Project, fingerprints
//! its current inputs (including direct legacy edits), and returns immutable data.
//! There is no owned Project, command execution, asynchronous work, or transport.
use crate::{
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity},
    model::{Element, Project, Shape},
    wall_solid::{self, WallContext},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "unit", content = "value", rename_all = "snake_case")]
pub enum NominalMeasure {
    AreaM2(f64),
    LengthM(f64),
}
#[derive(Clone, Debug, PartialEq)]
pub enum GeometrySource {
    ConceptMass(crate::slab_geometry::DerivedSlab),
    Zone(crate::slab_geometry::DerivedSlab),
    Wall(wall_solid::DerivedWall),
    Opening(crate::opening_geometry::DerivedOpening),
    Slab(crate::slab_geometry::DerivedSlab),
    Roof(crate::roof_geometry::DerivedRoof),
    Beam(crate::beam_geometry::DerivedBeam),
    Column(crate::column_geometry::DerivedColumn),
    Stair(crate::stair_geometry::DerivedStair),
    Polyline(crate::polyline_geometry::DerivedPolyline),
}
impl GeometrySource {
    /// Pure derivation identity. Document context belongs to GeometryHandle.
    pub fn identity(&self) -> &GeometryIdentity {
        match self {
            Self::Wall(g) => &g.identity,
            Self::Opening(g) => &g.identity,
            Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => &g.identity,
            Self::Roof(g) => &g.identity,
            Self::Beam(g) => &g.identity,
            Self::Column(g) => &g.identity,
            Self::Stair(g) => &g.identity,
            Self::Polyline(g) => &g.identity,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryHandle {
    /// Actual read revision/session; an old handle is never rebound in place.
    pub identity: GeometryIdentity,
    /// Immutable pure geometry, whose inner identity has no document revision.
    pub source: Arc<GeometrySource>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct CacheStats {
    pub context_rebuilds: usize,
    pub hits: usize,
    pub source_derivations: usize,
    pub plan_hits: usize,
    pub plan_derivations: usize,
    pub retained_entries: usize,
    pub invalidated_ids: Vec<u64>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DependencyIndex {
    pub story_elements: BTreeMap<u32, Vec<u64>>,
    pub host_openings: BTreeMap<u64, Vec<u64>>,
    pub wall_join_candidates: BTreeMap<u64, Vec<u64>>,
}
#[derive(Clone)]
struct PlanEntry {
    key: [u8; 32],
    result: Result<Arc<crate::column_geometry::DerivedColumnPlan>, GeometryDiagnostic>,
}
#[derive(Clone)]
struct Entry {
    key: [u8; 32],
    result: Result<Arc<GeometrySource>, GeometryDiagnostic>,
    plans: Vec<PlanEntry>,
}
#[derive(Clone)]
pub struct DerivationCache {
    enabled: bool,
    session: Option<u64>,
    policy: u32,
    contexts: BTreeMap<u32, Arc<WallContext>>,
    keys: HashMap<u64, [u8; 32]>,
    entries: HashMap<u64, Entry>,
    dependencies: DependencyIndex,
    stats: CacheStats,
}
impl Default for DerivationCache {
    fn default() -> Self {
        Self {
            enabled: true,
            session: None,
            policy: crate::precision::GEOMETRY_POLICY_VERSION,
            contexts: BTreeMap::new(),
            keys: HashMap::new(),
            entries: HashMap::new(),
            dependencies: DependencyIndex::default(),
            stats: CacheStats::default(),
        }
    }
}
impl DerivationCache {
    /// Internal immutable-baseline preparation may share pure payloads with its
    /// disposable shadow Editor. Every read still verifies the actual inputs.
    pub(crate) fn fork_for_session(&self, session: u64) -> Self {
        let mut fork = self.clone();
        fork.session = Some(session);
        fork.stats = Default::default();
        fork
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.clear();
            self.enabled = enabled;
        }
    }
    pub fn clear(&mut self) {
        self.contexts.clear();
        self.keys.clear();
        self.entries.clear();
        self.dependencies = Default::default();
        self.session = None;
        self.stats = Default::default();
    }
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }
    pub fn dependencies(&self) -> &DependencyIndex {
        &self.dependencies
    }
    /// Source preparation is shared once per read scope. Rechecking fingerprints
    /// also protects callers which changed old public fields without a revision.
    pub fn read<'a>(
        &'a mut self,
        project: &'a Project,
        revision: u64,
        session: u64,
    ) -> Result<GeometryRead<'a>, GeometryDiagnostic> {
        if !self.enabled
            || self.session != Some(session)
            || self.policy != crate::precision::GEOMETRY_POLICY_VERSION
        {
            self.clear();
            self.policy = crate::precision::GEOMETRY_POLICY_VERSION;
        }
        self.session = Some(session);
        self.stats = Default::default();
        let mut elements = HashMap::with_capacity(project.elements.len());
        let mut openings = HashMap::<u64, Vec<&Element>>::new();
        let mut walls = BTreeMap::<u32, Vec<&Element>>::new();
        let mut dependencies = DependencyIndex::default();
        if project.elements.len() > 100_000 {
            return Err(failure(
                0,
                "INVALID_PROJECT_SIZE",
                "Project exceeds 100000 elements",
            ));
        }
        for e in &project.elements {
            if elements.insert(e.id, e).is_some() {
                return Err(failure(
                    e.id,
                    "DUPLICATE_ELEMENT_ID",
                    "Duplicate element ID",
                ));
            }
            dependencies
                .story_elements
                .entry(e.story)
                .or_default()
                .push(e.id);
            if matches!(e.shape, Shape::Wall { .. }) {
                walls.entry(e.story).or_default();
            }
            if matches!(e.shape, Shape::Wall { .. }) && e.wall_contour.is_none() {
                walls.entry(e.story).or_default().push(e);
            }
            if let Shape::Opening { host_wall, .. } = e.shape {
                openings.entry(host_wall).or_default().push(e);
                dependencies
                    .host_openings
                    .entry(host_wall)
                    .or_default()
                    .push(e.id);
            }
        }
        let stories: HashSet<_> = walls.keys().copied().collect();
        self.contexts.retain(|story, _| stories.contains(story));
        for (story, list) in walls {
            let groups = project.layers.wall_groups(&list);
            let inputs: Vec<_> = list
                .iter()
                .zip(&groups)
                .map(|(e, g)| {
                    (
                        e.id,
                        &e.shape,
                        e.wall_settings
                            .as_ref()
                            .map(|s| (s.body_offset(e.thickness()), s.vertical_offsets())),
                        g,
                    )
                })
                .collect();
            let fingerprint =
                derived_geometry::input_fingerprint(&inputs, project.elevation(story))
                    .map_err(|m| failure(0, "WALL_CONTEXT_FAILED", m))?;
            if self
                .contexts
                .get(&story)
                .is_none_or(|c| c.input_fingerprint != fingerprint)
            {
                // Existing spatial solver, not a second join algorithm.
                self.contexts.insert(
                    story,
                    Arc::new(
                        wall_solid::context(project, story, None)
                            .map_err(|m| failure(0, "WALL_CONTEXT_FAILED", m))?,
                    ),
                );
                self.stats.context_rebuilds += 1;
            }
        }
        let mut keys = HashMap::with_capacity(elements.len());
        for context in self.contexts.values() {
            dependencies
                .wall_join_candidates
                .extend(context.join_candidates.clone());
        }
        for e in &project.elements {
            if matches!(e.shape, Shape::Opening { .. }) {
                continue;
            }
            let base = project.elevation(e.story);
            let key = match &e.shape {
                Shape::ConceptMass { spec } => derived_geometry::input_fingerprint(spec, base),
                Shape::Zone { spec } => derived_geometry::input_fingerprint(spec, base),
                Shape::Wall { .. } => {
                    let holes: Vec<_> = openings
                        .get(&e.id)
                        .into_iter()
                        .flatten()
                        .map(|o| (o.id, &o.shape))
                        .collect();
                    let outline = self
                        .contexts
                        .get(&e.story)
                        .and_then(|c| c.joined_footprints.get(&e.id));
                    derived_geometry::input_fingerprint(
                        &(
                            &e.shape,
                            &e.wall_contour,
                            e.wall_settings
                                .as_ref()
                                .map(|s| (s.body_offset(e.thickness()), s.vertical_offsets())),
                            e.wall_settings.as_ref().and_then(|s| s.validate().err()),
                            outline,
                            holes,
                            wall_solid::WALL_DERIVATION_VERSION,
                        ),
                        base,
                    )
                }
                Shape::Slab { .. } => derived_geometry::input_fingerprint(
                    &(
                        &e.shape,
                        &e.slab_contour,
                        crate::slab_geometry::SLAB_DERIVATION_VERSION,
                    ),
                    base,
                ),
                Shape::Roof { .. } => derived_geometry::input_fingerprint(
                    &(&e.shape, crate::roof_geometry::ROOF_DERIVATION_VERSION),
                    base,
                ),
                Shape::Beam { .. } => derived_geometry::input_fingerprint(
                    &(&e.shape, crate::beam_geometry::BEAM_DERIVATION_VERSION),
                    base,
                ),
                Shape::Column { .. } => {
                    let identity = crate::column_geometry::input_identity(e, base)?;
                    derived_geometry::input_fingerprint(
                        &(identity.input_fingerprint, identity.algorithm_version),
                        base,
                    )
                }
                Shape::Stair { .. } => derived_geometry::input_fingerprint(
                    &(&e.shape, crate::stair_geometry::STAIR_DERIVATION_VERSION),
                    base,
                ),
                Shape::Polyline { .. } => derived_geometry::input_fingerprint(
                    &(
                        &e.shape,
                        crate::polyline_geometry::POLYLINE_DERIVATION_VERSION,
                    ),
                    base,
                ),
                Shape::Opening { .. } => unreachable!(),
            }
            .map_err(|m| failure(e.id, "GEOMETRY_INPUT_IDENTITY", m))?;
            let key = derived_geometry::input_fingerprint(
                &(
                    e.story,
                    project.stories.iter().any(|s| s.id == e.story),
                    key,
                ),
                0.,
            )
            .map_err(|m| failure(e.id, "GEOMETRY_INPUT_IDENTITY", m))?;
            keys.insert(e.id, key);
        }
        for e in &project.elements {
            if let Shape::Opening { host_wall, .. } = e.shape {
                let key = derived_geometry::input_fingerprint(
                    &(
                        &e.shape,
                        keys.get(&host_wall),
                        e.opening_joinery,
                        crate::opening_geometry::OPENING_DERIVATION_VERSION,
                    ),
                    project.elevation(e.story),
                )
                .map_err(|m| failure(e.id, "GEOMETRY_INPUT_IDENTITY", m))?;
                let key = derived_geometry::input_fingerprint(
                    &(
                        e.story,
                        project.stories.iter().any(|s| s.id == e.story),
                        key,
                    ),
                    0.,
                )
                .map_err(|m| failure(e.id, "GEOMETRY_INPUT_IDENTITY", m))?;
                keys.insert(e.id, key);
            }
        }
        self.stats.invalidated_ids = self
            .keys
            .iter()
            .filter_map(|(id, key)| (keys.get(id) != Some(key)).then_some(*id))
            .collect();
        self.stats.invalidated_ids.sort_unstable();
        self.entries
            .retain(|id, entry| keys.get(id) == Some(&entry.key));
        self.keys = keys;
        self.dependencies = dependencies;
        self.stats.retained_entries = self.entries.len();
        Ok(GeometryRead {
            project,
            revision,
            session,
            cache: self,
            elements,
            openings,
        })
    }
}
fn failure(id: u64, code: &str, message: impl Into<String>) -> GeometryDiagnostic {
    GeometryDiagnostic::error(id, code, "geometry", message)
}
/// A read scope owns indexes of borrowed references, never a copied model.
pub struct GeometryRead<'a> {
    project: &'a Project,
    revision: u64,
    session: u64,
    cache: &'a mut DerivationCache,
    elements: HashMap<u64, &'a Element>,
    openings: HashMap<u64, Vec<&'a Element>>,
}
impl<'a> GeometryRead<'a> {
    pub(crate) fn project(&self) -> &'a Project {
        self.project
    }
    pub(crate) fn input_keys(&self) -> Vec<(u64, [u8; 32])> {
        self.project
            .elements
            .iter()
            .map(|e| (e.id, self.cache.keys[&e.id]))
            .collect()
    }
    pub(crate) fn matches_project(&self, p: &Project) -> bool {
        std::ptr::eq(self.project, p)
    }
    pub(crate) fn wall_context(&self, story: u32) -> Option<&WallContext> {
        self.cache.contexts.get(&story).map(Arc::as_ref)
    }
    pub fn model_includes(
        &mut self,
        id: u64,
        filter: crate::geometry::ModelViewFilter<'_>,
    ) -> Result<bool, GeometryDiagnostic> {
        if let Some(area) = filter.marquee {
            crate::geometry_validation::validate_clip_context(Some(&area.vertices), area.z_range)
                .map_err(|m| failure(id, "INVALID_VIEW_CLIP", m))?;
        }
        self.model_includes_valid_clip(id, filter)
    }
    pub(crate) fn model_includes_valid_clip(
        &mut self,
        id: u64,
        filter: crate::geometry::ModelViewFilter<'_>,
    ) -> Result<bool, GeometryDiagnostic> {
        let e = *self
            .elements
            .get(&id)
            .ok_or_else(|| failure(id, "ELEMENT_NOT_FOUND", "Element does not exist"))?;
        if filter.story.is_some_and(|story| story != e.story)
            || !crate::view_settings::DesignOptionScope::active(
                self.project.plan_options.active_design_option,
            )
            .contains(e.design_option)
            || !self.project.layers.is_visible(e.layer.0)
            || (filter.selection_only && !filter.selected.contains(&id))
        {
            return Ok(false);
        }
        if let Shape::Opening { host_wall, .. } = e.shape {
            if self
                .elements
                .get(&host_wall)
                .is_none_or(|host| !self.project.layers.is_visible(host.layer.0))
            {
                return Ok(false);
            }
        }
        let Some(area) = filter.marquee else {
            return Ok(true);
        };
        if !area.includes_story(e.story) {
            return Ok(false);
        }
        let handle = self.source(id)?;
        let source = handle.source.as_ref();
        if filter.outside_marquee {
            let inside = source.fully_inside_polygon(&area.vertices)?
                && area.z_range.is_none_or(|(a, b)| {
                    source
                        .display_bounds()
                        .is_none_or(|[lo, hi]| lo.z >= a.min(b) && hi.z <= a.max(b))
                });
            Ok(!inside)
        } else {
            Ok(source.intersects_polygon(&area.vertices)?
                && area.z_range.is_none_or(|(a, b)| {
                    source
                        .display_bounds()
                        .is_some_and(|[lo, hi]| hi.z >= a.min(b) && lo.z <= a.max(b))
                }))
        }
    }
    pub fn stats(&self) -> &CacheStats {
        &self.cache.stats
    }
    pub fn batch(&mut self, ids: &[u64], plan_story: Option<u32>) -> GeometryBatch {
        let mut sources = BTreeMap::new();
        let mut columns = BTreeMap::new();
        for &id in ids {
            let source = self.source(id);
            if source
                .as_ref()
                .is_ok_and(|h| matches!(h.source.as_ref(), GeometrySource::Column(_)))
            {
                if let Some(story) = plan_story {
                    columns.insert(id, self.column_plan(id, story));
                }
            }
            sources.insert(id, source);
        }
        GeometryBatch {
            revision: self.revision,
            document_session: self.session,
            sources,
            columns,
            contexts: self.cache.contexts.clone(),
        }
    }
    pub fn column_plan(
        &mut self,
        id: u64,
        story: u32,
    ) -> Result<crate::column_geometry::DerivedColumnPlan, GeometryDiagnostic> {
        let e = *self
            .elements
            .get(&id)
            .ok_or_else(|| failure(id, "ELEMENT_NOT_FOUND", "Column does not exist"))?;
        if !matches!(e.shape, Shape::Column { .. }) {
            return Err(failure(id, "NOT_COLUMN", "Element is not a column"));
        }
        let handle = self.source(id)?;
        let GeometrySource::Column(source) = handle.source.as_ref() else {
            return Err(failure(id, "NOT_COLUMN", "Element is not a column"));
        };
        let context = crate::column_geometry::plan_context(self.project, e, story)?;
        let key = crate::column_geometry::plan_identity(
            self.project,
            e,
            story,
            &source.identity,
            context,
        )?
        .input_fingerprint;
        let found = self.cache.entries[&id]
            .plans
            .iter()
            .position(|p| p.key == key);
        let result = if let Some(index) = found {
            self.cache.stats.plan_hits += 1;
            let entry = self.cache.entries.get_mut(&id).unwrap().plans.remove(index);
            let result = entry.result.clone();
            self.cache.entries.get_mut(&id).unwrap().plans.push(entry);
            result
        } else {
            self.cache.stats.plan_derivations += 1;
            let result =
                crate::column_geometry::derive_plan_with_source(self.project, e, story, source)
                    .map(Arc::new);
            let entries = &mut self.cache.entries.get_mut(&id).unwrap().plans;
            if entries.len() == 2 {
                entries.remove(0);
            }
            entries.push(PlanEntry {
                key,
                result: result.clone(),
            });
            result
        }?;
        let mut result = result.as_ref().clone();
        for identity in [&mut result.identity, &mut result.source_identity] {
            identity.input_revision = Some(self.revision);
            identity.document_session = Some(self.session);
        }
        Ok(result)
    }
    pub fn source(&mut self, id: u64) -> Result<GeometryHandle, GeometryDiagnostic> {
        let e = *self
            .elements
            .get(&id)
            .ok_or_else(|| failure(id, "ELEMENT_NOT_FOUND", "Element does not exist"))?;
        let key = self.cache.keys[&id];
        let result = if let Some(entry) = self.cache.entries.get(&id) {
            self.cache.stats.hits += 1;
            entry.result.clone()
        } else {
            self.cache.stats.source_derivations += 1;
            let result = self.derive(e).map(Arc::new);
            self.cache.entries.insert(
                id,
                Entry {
                    key,
                    result: result.clone(),
                    plans: Vec::new(),
                },
            );
            self.cache.stats.retained_entries = self.cache.entries.len();
            result
        }?;
        let mut identity = result.identity().clone();
        identity.input_revision = Some(self.revision);
        identity.document_session = Some(self.session);
        Ok(GeometryHandle {
            identity,
            source: result,
        })
    }
    fn derive(&mut self, e: &Element) -> Result<GeometrySource, GeometryDiagnostic> {
        if !self.project.stories.iter().any(|s| s.id == e.story) {
            return Err(failure(
                e.id,
                "GEOMETRY_STORY_MISSING",
                "Element references a missing story",
            ));
        }
        let base = self.project.elevation(e.story);
        Ok(match &e.shape {
            Shape::Wall { .. } => GeometrySource::Wall(wall_solid::derive_from_inputs(
                self.project,
                e,
                &self.cache.contexts[&e.story],
                self.openings.get(&e.id).map_or(&[], Vec::as_slice),
            )?),
            Shape::Opening { host_wall, .. } => {
                let host = *self.elements.get(host_wall).ok_or_else(|| {
                    failure(e.id, "OPENING_HOST_MISSING", "Opening host does not exist")
                })?;
                if !matches!(host.shape, Shape::Wall { .. }) {
                    return Err(failure(
                        e.id,
                        "OPENING_HOST_MISSING",
                        "Opening host is not a wall",
                    ));
                }
                let handle = self.source(*host_wall)?;
                let GeometrySource::Wall(wall) = handle.source.as_ref() else {
                    return Err(failure(
                        e.id,
                        "OPENING_HOST_MISSING",
                        "Opening host is not a wall",
                    ));
                };
                GeometrySource::Opening(crate::opening_geometry::derive_with_host(
                    host, e, wall, base,
                )?)
            }
            Shape::ConceptMass { .. } => {
                GeometrySource::ConceptMass(crate::concept_mass::derive(e, base)?)
            }
            Shape::Zone { .. } => GeometrySource::Zone(crate::zones::derive(e, base)?),
            Shape::Slab { .. } => GeometrySource::Slab(crate::slab_geometry::derive(e, base)?),
            Shape::Roof { .. } => GeometrySource::Roof(crate::roof_geometry::derive(e, base)?),
            Shape::Beam { .. } => {
                GeometrySource::Beam(crate::beam_geometry::derive(self.project, e)?)
            }
            Shape::Column { .. } => {
                GeometrySource::Column(crate::column_geometry::derive(self.project, e)?)
            }
            Shape::Stair { .. } => {
                GeometrySource::Stair(crate::stair_geometry::derive(self.project, e)?)
            }
            Shape::Polyline { .. } => {
                GeometrySource::Polyline(crate::polyline_geometry::derive(self.project, e)?)
            }
        })
    }
}

#[derive(Debug)]
pub struct GeometryRef<'a, T> {
    pub identity: &'a GeometryIdentity,
    pub geometry: &'a T,
}
#[derive(Debug)]
pub struct OpeningRef<'a> {
    pub identity: &'a GeometryIdentity,
    pub host_identity: GeometryIdentity,
    pub geometry: &'a crate::opening_geometry::OpeningSnapshot,
}
/// An immutable, finite collection of derived handles, never a Project copy.
pub struct GeometryBatch {
    pub revision: u64,
    pub document_session: u64,
    sources: BTreeMap<u64, Result<GeometryHandle, GeometryDiagnostic>>,
    columns: BTreeMap<u64, Result<crate::column_geometry::DerivedColumnPlan, GeometryDiagnostic>>,
    contexts: BTreeMap<u32, Arc<WallContext>>,
}
impl GeometryBatch {
    pub(crate) fn failed(
        ids: &[u64],
        revision: u64,
        document_session: u64,
        error: GeometryDiagnostic,
    ) -> Self {
        Self {
            revision,
            document_session,
            sources: ids.iter().map(|&id| (id, Err(error.clone()))).collect(),
            columns: BTreeMap::new(),
            contexts: BTreeMap::new(),
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.sources.keys().copied()
    }
    pub fn plan_outline(&self, id: u64) -> Result<Vec<crate::model::Point>, GeometryDiagnostic> {
        if self.columns.contains_key(&id) {
            return Ok(self.column_plan(id)?.view.visible_outline());
        }
        Ok(self.source(id)?.source.plan_outer_outline()?.into_owned())
    }
    pub fn source(&self, id: u64) -> Result<&GeometryHandle, GeometryDiagnostic> {
        self.sources
            .get(&id)
            .ok_or_else(|| {
                failure(
                    id,
                    "ELEMENT_NOT_REQUESTED",
                    "Element was not requested in this read batch",
                )
            })?
            .as_ref()
            .map_err(Clone::clone)
    }
    pub fn wall(
        &self,
        id: u64,
    ) -> Result<GeometryRef<'_, wall_solid::WallSnapshot>, GeometryDiagnostic> {
        let handle = self.source(id)?;
        match handle.source.as_ref() {
            GeometrySource::Wall(g) => Ok(GeometryRef {
                identity: &handle.identity,
                geometry: &g.geometry,
            }),
            _ => Err(failure(id, "NOT_WALL", "Element is not a wall")),
        }
    }
    pub fn opening(&self, id: u64) -> Result<OpeningRef<'_>, GeometryDiagnostic> {
        let handle = self.source(id)?;
        match handle.source.as_ref() {
            GeometrySource::Opening(g) => {
                let mut host_identity = g.host_identity.clone();
                host_identity.input_revision = Some(self.revision);
                host_identity.document_session = Some(self.document_session);
                Ok(OpeningRef {
                    identity: &handle.identity,
                    host_identity,
                    geometry: &g.geometry,
                })
            }
            _ => Err(failure(id, "NOT_OPENING", "Element is not an opening")),
        }
    }
    pub fn context(&self, story: u32) -> Result<&WallContext, String> {
        self.contexts
            .get(&story)
            .map(Arc::as_ref)
            .ok_or_else(|| "Wall story context is unavailable".into())
    }
    pub fn column_plan(
        &self,
        id: u64,
    ) -> Result<&crate::column_geometry::DerivedColumnPlan, GeometryDiagnostic> {
        self.columns
            .get(&id)
            .ok_or_else(|| failure(id, "ELEMENT_NOT_REQUESTED", "Column plan was not requested"))?
            .as_ref()
            .map_err(Clone::clone)
    }
}
