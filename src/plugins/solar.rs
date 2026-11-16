//! Solar payload inside the existing immutable Job envelope, never a second model.
use crate::{
    editor::Editor,
    model::{Element, Shape},
    solar::{self, North, Request, Snapshot},
};
use serde::{Deserialize, Serialize};
pub const COMMAND: &str = "org.openarchi.solar.study";
pub const ID: &str = "org.openarchi.solar";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub request: Request,
    pub assumed_north: Option<North>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub request: Request,
    pub north: North,
    pub story_elevation_m: f64,
    pub excluded_element_ids: Vec<u64>,
}
/// Bounded source context for the existing background read projection.
#[derive(Clone)]
pub struct Context {
    north: Option<crate::site::TrueNorth>,
    elevation: Option<f64>,
    all_ids: Vec<u64>,
}
impl Context {
    pub fn capture(e: &Editor) -> Self {
        let mut all_ids = e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>();
        all_ids.sort_unstable();
        Self {
            north: e.project.site.true_north.clone(),
            elevation: e
                .project
                .stories
                .iter()
                .find(|s| s.id == e.story)
                .map(|s| s.elevation),
            all_ids,
        }
    }
    pub fn apply(
        &self,
        mut input: super::jobs::Input,
        params: &Parameters,
    ) -> Result<super::jobs::Input, String> {
        let north=match &self.north {Some(n)=>North::Declared(n.clone()),None=>match &params.assumed_north {Some(n@North::DesignAssumption{..})=>n.clone(),_=>return Err("True north is unknown; supply an explicit design assumption or declared site north".into())}};
        input
            .elements
            .retain(|e| matches!(e.shape, Shape::ConceptMass { .. }));
        let excluded = self
            .all_ids
            .iter()
            .filter(|id| input.elements.binary_search_by_key(*id, |e| e.id).is_err())
            .copied()
            .collect();
        input.solar = Some(Payload {
            request: params.request.clone(),
            north,
            story_elevation_m: self.elevation.ok_or("Job source story is unavailable")?,
            excluded_element_ids: excluded,
        });
        input.validate()?;
        Ok(input)
    }
}
impl Payload {
    pub fn validate(&self, elements: &[Element]) -> Result<(), String> {
        self.north.validate()?;
        let work = self.request.work_units()?;
        if !self.story_elevation_m.is_finite()
            || self.story_elevation_m.abs() > 100000.
            || elements.is_empty()
            || elements.len() > 256
            || elements
                .iter()
                .any(|e| !matches!(e.shape, Shape::ConceptMass { .. }))
            || self.excluded_element_ids.len() > 100000
            || self.excluded_element_ids.windows(2).any(|p| p[0] >= p[1])
            || self
                .excluded_element_ids
                .iter()
                .any(|id| *id == 0 || elements.iter().any(|e| e.id == *id))
        {
            return Err("Invalid solar job physical scope or source frame".into());
        }
        // Retain the existing4MiB read/write envelope; no implicit resampling.
        let request_bytes = serde_json::to_vec_pretty(&self.request)
            .map_err(|e| e.to_string())?
            .len();
        if work
            .checked_mul(512)
            .and_then(|b| b.checked_add(request_bytes * 3 + 16384))
            .is_none_or(|b| b as u64 > super::MAX_BYTES)
        {
            return Err(
                "Solar result exceeds the 4 MiB job budget; explicitly reduce points or sampling"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn snapshot(&self, elements: &[Element]) -> Result<Snapshot, String> {
        self.validate(elements)?;
        solar::capture_objects(
            &elements
                .iter()
                .map(|e| (e, self.story_elevation_m))
                .collect::<Vec<_>>(),
            self.north.clone(),
            self.excluded_element_ids.clone(),
        )
    }
    pub fn parameters(&self) -> Parameters {
        Parameters {
            request: self.request.clone(),
            assumed_north: match &self.north {
                n @ North::DesignAssumption { .. } => Some(n.clone()),
                _ => None,
            },
        }
    }
}
pub fn builtin() -> super::Manifest {
    super::Manifest::parse(include_bytes!("../../assets/plugins/openarchi-solar.json"))
        .expect("valid bundled solar declaration")
}
