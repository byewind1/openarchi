//! Study settings in the existing jobs window. No model or independent task store.
use super::*;
use crate::{
    plugins::solar::Parameters,
    solar::{self, LocationBasis, North, Utc},
};
#[derive(Default)]
pub(super) struct Draft {
    latitude: String,
    longitude: String,
    north: String,
    note: String,
    start: String,
    end: String,
    point: String,
    step: String,
    previous_location: Option<solar::Location>,
    previous_basis: Option<LocationBasis>,
    error: String,
    focus: bool,
}
impl Draft {
    pub(super) fn from_input(
        input: Option<&crate::plugins::solar::Payload>,
    ) -> Result<Self, String> {
        let Some(s) = input else {
            return Ok(Self {
                step: "60".into(),
                focus: true,
                ..Default::default()
            });
        };
        let r = &s.request;
        if r.receivers.len() != 1 || r.receivers[0].outward_normal != [0., 0., 1.] {
            return Err("This editor supports one horizontal receiver; the original typed request remains available".into());
        }
        Ok(Self {
            latitude: r.location.latitude_deg.to_string(),
            longitude: r.location.longitude_deg.to_string(),
            north: s.north.angle().to_string(),
            note: match &r.location_basis {
                LocationBasis::DesignAssumption { note } => note.clone(),
                _ => String::new(),
            },
            start: format_time(r.start_utc),
            end: format_time(r.end_utc),
            point: r.receivers[0].point_m.map(|v| v.to_string()).join(", "),
            step: r.step_seconds.to_string(),
            previous_location: Some(r.location),
            previous_basis: Some(r.location_basis.clone()),
            error: String::new(),
            focus: true,
        })
    }
    fn parameters(&self, has_north: bool) -> Result<Parameters, String> {
        let numeric = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let location = solar::Location {
            latitude_deg: numeric(&self.latitude).ok_or("Enter valid latitude and longitude")?,
            longitude_deg: numeric(&self.longitude).ok_or("Enter valid latitude and longitude")?,
        };
        location
            .validate()
            .map_err(|_| "Enter valid latitude and longitude")?;
        let start =
            parse_time(&self.start).map_err(|_| "Enter explicit UTC start and end times")?;
        let end = parse_time(&self.end).map_err(|_| "Enter explicit UTC start and end times")?;
        let coords = self
            .point
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| numeric(s).ok_or("Enter a receiver point in model metres"))
            .collect::<Result<Vec<_>, _>>()?;
        let point_m =
            <[f64; 3]>::try_from(coords).map_err(|_| "Enter a receiver point in model metres")?;
        let basis = if self.previous_location == Some(location) {
            self.previous_basis.clone()
        } else {
            None
        }
        .unwrap_or_else(|| LocationBasis::DesignAssumption {
            note: self.note.clone(),
        });
        basis
            .validate()
            .map_err(|_| "Enter an explicit design assumption note")?;
        let assumed_north = if has_north {
            None
        } else {
            let angle = numeric(&self.north).ok_or("Enter the assumed north angle")?;
            let n = North::DesignAssumption {
                clockwise_from_model_y_deg: angle,
                note: self.note.clone(),
            };
            n.validate()
                .map_err(|_| "Enter a bounded north assumption and note")?;
            Some(n)
        };
        let request = solar::Request {
            location,
            location_basis: basis,
            start_utc: start,
            end_utc: end,
            step_seconds: self
                .step
                .parse()
                .map_err(|_| "Enter a valid sampling step in seconds")?,
            ray_offset_m: 0.0001,
            receivers: vec![solar::Receiver {
                id: "point-1".into(),
                point_m,
                outward_normal: [0., 0., 1.],
            }],
        };
        request
            .work_units()
            .map_err(|_| "Solar request exceeds allowed sampling or value bounds")?;
        Ok(Parameters {
            request,
            assumed_north,
        })
    }
}
fn format_time(t: Utc) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}
fn parse_time(s: &str) -> Result<Utc, String> {
    let invalid = || "Expected UTC YYYY-MM-DDTHH:MM:SSZ".to_string();
    let s = s.trim().strip_suffix('Z').ok_or_else(invalid)?;
    let (d, t) = s.split_once('T').ok_or_else(invalid)?;
    let d = d.split('-').collect::<Vec<_>>();
    let t = t.split(':').collect::<Vec<_>>();
    if d.len() != 3 || t.len() != 3 {
        return Err(invalid());
    }
    let v = Utc {
        year: d[0].parse().map_err(|_| invalid())?,
        month: d[1].parse().map_err(|_| invalid())?,
        day: d[2].parse().map_err(|_| invalid())?,
        hour: t[0].parse().map_err(|_| invalid())?,
        minute: t[1].parse().map_err(|_| invalid())?,
        second: t[2].parse().map_err(|_| invalid())?,
    };
    v.julian_day()?;
    Ok(v)
}
fn field(ui: &mut Ui, l: UiLanguage, label: &str, v: &mut String) {
    let r = ui.label(l.translate(label));
    let response = ui
        .add(TextEdit::singleline(v).char_limit(4096).desired_width(440.))
        .labelled_by(r.id);
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            Id::new(("solar_field", label)),
            response.rect.intersect(ui.clip_rect()),
        )
    });
    let _ = response;
}
pub(super) enum Action {
    Run(Parameters),
    Cancel,
}
pub(super) fn contents(ui: &mut Ui, l: UiLanguage, d: &mut Draft, p: &Project) -> Option<Action> {
    ui.group(|ui|{let heading=ui.strong(l.translate("Geometric sunlight study"));if d.focus{heading.scroll_to_me(Some(egui::Align::Min));d.focus=false;}ui.label(l.translate("Concept scenario only. Location and time are not inferred; modelling remains available."));
    field(ui,l,"Latitude (degrees, -90 to 90)",&mut d.latitude);field(ui,l,"Longitude (east-positive degrees)",&mut d.longitude);
    if let Some(n)=&p.site.true_north{ui.label(format!("{}: {}°",l.translate("Declared site north"),n.clockwise_from_model_y_deg));}else{field(ui,l,"Assumed north clockwise from model Y (degrees)",&mut d.north);}
    field(ui,l,"Design assumption note",&mut d.note);if matches!(d.previous_basis,Some(LocationBasis::Declared{..})){ui.label(l.translate("The saved location citation is retained while coordinates remain unchanged."));}
    field(ui,l,"Start UTC (YYYY-MM-DDTHH:MM:SSZ)",&mut d.start);field(ui,l,"End UTC (YYYY-MM-DDTHH:MM:SSZ)",&mut d.end);field(ui,l,"Horizontal receiver point (x, y, z metres)",&mut d.point);field(ui,l,"Sampling step (seconds)",&mut d.step);
    ui.label(l.translate("Only current story/candidate opaque concept masses are included; Zones, other BIM, terrain and glazing are excluded."));
    if !d.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,l.translate(&d.error));}
    let mut action=None;ui.horizontal_wrapped(|ui|{if ui.button(l.translate("Run declared sunlight study")).clicked(){match d.parameters(p.site.true_north.is_some()){Ok(p)=>action=Some(Action::Run(p)),Err(e)=>d.error=e}}if ui.button(l.translate("Cancel study settings")).clicked(){action=Some(Action::Cancel);}});action
    }).inner
}
pub(super) fn result_contents(ui: &mut Ui, l: UiLanguage, r: &solar::ResultSet) {
    ui.label(l.translate("Geometric solar centre and selected opaque envelopes only; not statutory sunlight or approval."));
    ui.label(format!(
        "{} → {} · {}s",
        format_time(r.request.start_utc),
        format_time(r.request.end_utc),
        r.request.step_seconds
    ));
    for p in &r.points {
        ui.label(format!(
            "{} · {}: {} s · {}: {} s",
            p.receiver_id,
            l.translate("Known sampled sunlight"),
            p.sampled_sunlit_seconds,
            l.translate("Uncertain contact time"),
            p.uncertain_seconds
        ));
        ui.label(l.translate("Sampling can miss shorter events. Known sampled seconds are not a complete continuous duration."));
        ui.collapsing(
            format!(
                "{} · {}",
                l.translate("Sunlight sample details"),
                p.receiver_id
            ),
            |ui| {
                for c in p.cells.iter().take(120) {
                    let state = match c.state {
                        solar::State::Sunlit => "Sunlit",
                        solar::State::Occluded => "Occluded",
                        solar::State::BelowHorizon => "Sun below horizon",
                        solar::State::BackFacing => "Receiver faces away",
                        solar::State::NearSurface => "Near-surface contact is unknown",
                    };
                    ui.label(format!(
                        "{}–{} s · {} · {}: {:?}",
                        c.start_seconds,
                        c.end_seconds,
                        l.translate(state),
                        l.translate("Blocker"),
                        c.blocker_id
                    ));
                }
                if p.cells.len() > 120 {
                    ui.label(l.translate("More samples remain in the saved job artifact."));
                }
            },
        );
    }
}
