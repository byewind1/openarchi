//! Project-bound, editable report templates. Geometry stays in the linked figures.
use super::{Diagram, Status};
use crate::{drawings::DrawingDocument, model::Project};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub heading: String,
    pub figure: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub id: u64,
    pub generation: u32,
    pub name: String,
    pub sections: Vec<Section>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Create {
        name: String,
        sections: Vec<Section>,
    },
    Update {
        id: u64,
        expected_sha256: String,
        name: String,
        sections: Vec<Section>,
    },
    Delete {
        id: u64,
        expected_sha256: String,
    },
}
fn label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 160 && !s.chars().any(char::is_control)
}
impl Template {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0
            || self.generation == 0
            || !label(&self.name)
            || self.sections.is_empty()
            || self.sections.len() > 16
        {
            return Err("Invalid report template or section limit reached".into());
        }
        let mut ids = std::collections::HashSet::new();
        if self
            .sections
            .iter()
            .any(|s| !label(&s.heading) || s.figure == 0 || !ids.insert(s.figure))
        {
            return Err("Report sections need a title and distinct figure references".into());
        }
        Ok(())
    }
    pub fn expected(&self) -> String {
        super::hash(self).expect("valid report serializes")
    }
    pub fn figure<'a>(&self, p: &'a Project, index: usize) -> Option<&'a Diagram> {
        self.sections
            .get(index)
            .and_then(|s| p.drawings.diagrams.iter().find(|f| f.id == s.figure))
    }
    pub fn status(&self, p: &Project) -> Status {
        let mut result = Status::Current;
        for i in 0..self.sections.len() {
            match self.figure(p, i).map(|f| f.status(p)) {
                None | Some(Status::Unavailable) => return Status::Unavailable,
                Some(Status::Stale) => result = Status::Stale,
                _ => {}
            }
        }
        result
    }
    /// One ordered SVG long sheet, not a PDF, print layout or compliance report.
    /// All source pages must be current before any export bytes are returned.
    pub fn svg(&self, p: &Project) -> Result<String, String> {
        self.validate()?;
        if self.status(p) != Status::Current {
            return Err("Report contains missing or stale figures; review its sources".into());
        }
        let mut pages = Vec::new();
        for (i, s) in self.sections.iter().enumerate() {
            let f = self.figure(p, i).ok_or("Figure is missing")?;
            pages.push((s, f, f.svg(p)?));
        }
        let height = pages.len() * 950 + 160;
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"{height}\" viewBox=\"0 0 1200 {height}\" data-report-id=\"{}\" data-report-sha256=\"{}\"><title>{}</title><rect width=\"100%\" height=\"100%\" fill=\"white\"/>",
            self.id,
            self.expected(),
            super::xml(&self.name)
        );
        wrapped_title(&mut out, &self.name, 30., 24.);
        out.push_str("<text x=\"30\" y=\"140\" font-size=\"12\">Linked concept figures; authored quantities and declarations, not compliance.</text>");
        for (i, (section, figure, svg)) in pages.into_iter().enumerate() {
            let y = i * 950 + 160;
            write!(out,"<g data-report-page=\"{}\" data-figure-id=\"{}\" data-figure-sha256=\"{}\" transform=\"translate(0 {})\">",i+1,figure.id,super::expected(figure),y).unwrap();
            wrapped_title(
                &mut out,
                &format!("{}. {}", i + 1, section.heading),
                25.,
                20.,
            );
            write!(out,"<svg x=\"0\" y=\"120\" width=\"1200\" height=\"800\" viewBox=\"0 0 1200 800\">{svg}</svg></g>").unwrap();
            if out.len() > 16 * 1024 * 1024 {
                return Err("Report SVG exceeds 16 MiB".into());
            }
        }
        out.push_str("</svg>");
        Ok(out)
    }
}
fn wrapped_title(out: &mut String, text: &str, y: f32, size: f32) {
    let chars: Vec<_> = text.chars().collect();
    for (line, part) in chars.chunks(45).enumerate() {
        write!(
            out,
            "<text x=\"30\" y=\"{}\" font-size=\"{size}\">{}</text>",
            y + line as f32 * (size + 3.),
            super::xml(&part.iter().collect::<String>())
        )
        .unwrap();
    }
}
pub fn apply(p: &Project, edit: &Edit) -> Result<DrawingDocument, String> {
    let mut d = p.drawings.clone();
    match edit {
        Edit::Create { name, sections } => {
            if d.reports.len() >= 8 {
                return Err("Report template limit reached".into());
            }
            let report = Template {
                id: d.next_id()?,
                generation: 1,
                name: name.clone(),
                sections: sections.clone(),
            };
            check_references(&report, p)?;
            d.reports.push(report);
        }
        Edit::Update {
            id,
            expected_sha256,
            name,
            sections,
        } => {
            let index = original(&d, *id, expected_sha256)?;
            let old = &d.reports[index];
            if old.name == *name && old.sections == *sections {
                return Err("Report template is unchanged".into());
            }
            let report = Template {
                id: *id,
                generation: old
                    .generation
                    .checked_add(1)
                    .ok_or("Report generations exhausted")?,
                name: name.clone(),
                sections: sections.clone(),
            };
            check_references(&report, p)?;
            d.reports[index] = report;
        }
        Edit::Delete {
            id,
            expected_sha256,
        } => {
            let index = original(&d, *id, expected_sha256)?;
            d.reports.remove(index);
        }
    }
    Ok(d)
}
fn check_references(report: &Template, p: &Project) -> Result<(), String> {
    report.validate()?;
    if report
        .sections
        .iter()
        .any(|s| !p.drawings.diagrams.iter().any(|f| f.id == s.figure))
    {
        return Err("Report references a missing figure".into());
    }
    Ok(())
}
fn original(d: &DrawingDocument, id: u64, expected: &str) -> Result<usize, String> {
    let index = d
        .reports
        .iter()
        .position(|r| r.id == id)
        .ok_or("Report template is missing")?;
    if d.reports[index].expected() != expected {
        return Err("Report template changed; review the original draft".into());
    }
    Ok(index)
}
pub fn query(p: &Project) -> Vec<serde_json::Value> {
    p.drawings.reports.iter().map(|r| serde_json::json!({"template":r,"expected_sha256":r.expected(),"status":r.status(p).label(),"sections":r.sections.iter().enumerate().map(|(i,s)|serde_json::json!({"figure":s.figure,"status":r.figure(p,i).map(|f|f.status(p).label()).unwrap_or("Figure is missing"),"source_sha256":r.figure(p,i).map(|f|f.source_sha256.as_str())})).collect::<Vec<_>>(),"scope":"ordered project-bound figure references; no automatic refresh or compliance verdict"})).collect()
}
