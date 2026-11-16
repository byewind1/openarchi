//! Original standalone public-protocol example. No application/Rust ABI dependency.
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
fn area(points: &Value) -> f64 {
    let p = points.as_array().unwrap();
    let Some(origin) = p.first() else {
        return 0.;
    };
    let x = origin["x"].as_f64().unwrap();
    let y = origin["y"].as_f64().unwrap();
    (0..p.len())
        .map(|i| {
            let a = &p[i];
            let b = &p[(i + 1) % p.len()];
            (a["x"].as_f64().unwrap() - x) * (b["y"].as_f64().unwrap() - y)
                - (b["x"].as_f64().unwrap() - x) * (a["y"].as_f64().unwrap() - y)
        })
        .sum::<f64>()
        .abs()
        * 0.5
}
fn quantity(e: &Value) -> (f64, f64) {
    let s = &e["shape"]["spec"];
    let a = area(&s["outline"]) - s["holes"].as_array().unwrap().iter().map(area).sum::<f64>();
    (a, a * s["height"].as_f64().unwrap())
}
fn emit(v: Value) {
    println!("{v}");
    std::io::stdout().flush().unwrap();
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .take(4 * 1024 * 1024 + 1)
        .read_line(&mut line)?;
    if line.len() > 4 * 1024 * 1024 || !line.ends_with('\n') {
        return Err("invalid input frame size".into());
    }
    let request: Value = serde_json::from_str(&line)?;
    if request["protocol_version"] != 1 {
        return Err("unsupported protocol".into());
    }
    let intent = &request["intent"];
    let identity = &intent["identity"];
    let input = &intent["input"];
    let elements = input["elements"].as_array().ok_or("no immutable input")?;
    let mut zones = vec![];
    let mut masses = vec![];
    let (mut za, mut zv, mut ma, mut mv) = (0., 0., 0., 0.);
    for (i, e) in elements.iter().enumerate() {
        let (a, v) = quantity(e);
        match e["shape"]["kind"].as_str() {
            Some("Zone") => {
                zones.push(e["id"].clone());
                za += a;
                zv += v;
            }
            Some("ConceptMass") => {
                masses.push(e["id"].clone());
                ma += a;
                mv += v;
            }
            _ => return Err("unsupported geometry".into()),
        }
        if i % 64 == 0 || i + 1 == elements.len() {
            emit(json!({"type":"progress","identity":identity,"completed":i+1}));
        }
    }
    let mut report = json!({"definition":"Authored boundary minus holes; overlaps counted separately within each domain. Zone area and mass footprint are separate; not net/legal floor area or compliance.",
        "zones":{"element_ids":zones,"area_m2":za,"volume_m3":zv},"concept_masses":{"element_ids":masses,"area_m2":ma,"volume_m3":mv}});
    if let Some(program) = input.get("program") {
        let sheets = program["sheets"].as_array().ok_or("invalid program")?;
        let mut rows = vec![];
        for sheet in sheets {
            for row in sheet["rows"].as_array().ok_or("invalid requirement rows")? {
                let v = &row["values"];
                let label = &v["label"];
                let candidates: Vec<_> = elements
                    .iter()
                    .filter(|e| {
                        e["shape"]["kind"] == "Zone" && e["shape"]["spec"]["purpose"] == *label
                    })
                    .collect();
                let ids: Vec<_> = candidates.iter().map(|e| e["id"].clone()).collect();
                let a: f64 = candidates.iter().map(|e| quantity(e).0).sum();
                let duplicates = sheets
                    .iter()
                    .flat_map(|s| s["rows"].as_array().unwrap())
                    .filter(|r| r["values"]["label"] == *label)
                    .count()
                    > 1;
                let delta = if v["area"]["status"] == "known" {
                    json!(a - v["area"]["m2"].as_f64().ok_or("invalid declared area")?)
                } else {
                    Value::Null
                };
                rows.push(json!({"sheet_id":sheet["id"],"source_sha256":sheet["source_sha256"],"key":row["key"],"label":label,"scope":v["scope"],"declared_quantity":v["quantity"],"required_area":v["area"],"candidate_zone_ids":ids,"candidate_area_m2":a,"delta_m2":delta,"mapping_status":if candidates.is_empty(){"no_label_match"}else if duplicates{"ambiguous_label"}else{"unverified_label_match"}}));
            }
        }
        report["program_review"] = json!({"method":"Exact purpose-label candidates only; no approved requirement-to-Zone mapping. Areas are declared row totals, not multiplied by quantity. Empty requirements remain unknown; no satisfaction/compliance verdict.","rows":rows,"unassigned_zone_ids":zones});
    }
    emit(
        json!({"type":"result","artifact":{"schema_version":1,"identity":identity,"terminal":{"phase":"completed","report":report}}}),
    );
    Ok(())
}
