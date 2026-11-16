//! Strict ASCII DXF adapter: planar LINE and straight LWPOLYLINE in model space.
//! Unsupported entities/OCS/widths are rejected, never silently dropped.
pub mod inventory;
pub mod primitives;
pub mod reference;
use crate::model::{Material, Point, Project, Shape};
pub fn parse(bytes: &[u8]) -> Result<Project, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Only ASCII/UTF-8 DXF is supported")?;
    let lines: Vec<_> = text.lines().collect();
    if lines.len() % 2 != 0 {
        return Err("Invalid DXF group pairs".into());
    }
    let pairs: Vec<(i32, &str)> = lines
        .chunks_exact(2)
        .map(|p| {
            Ok((
                p[0].trim().parse().map_err(|_| "Invalid DXF group code")?,
                p[1].trim(),
            ))
        })
        .collect::<Result<_, String>>()?;
    let unit = pairs
        .windows(2)
        .find(|v| v[0] == (9, "$INSUNITS"))
        .and_then(|v| v[1].1.parse::<i32>().ok())
        .ok_or("DXF needs explicit INSUNITS (mm, cm, m, inch or foot)")?;
    let scale = match unit {
        1 => 0.0254,
        2 => 0.3048,
        4 => 0.001,
        5 => 0.01,
        6 => 1.,
        _ => return Err("DXF needs explicit INSUNITS (mm, cm, m, inch or foot)".into()),
    };
    let mut p = Project::default();
    p.elements.clear();
    p.next_id = 1;
    let mut in_entities = false;
    let mut found = false;
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i] == (0, "SECTION") && pairs.get(i + 1) == Some(&(2, "ENTITIES")) {
            in_entities = true;
            found = true;
            i += 2;
            continue;
        }
        if pairs[i] == (0, "ENDSEC") {
            in_entities = false;
        }
        if !in_entities || pairs[i].0 != 0 {
            i += 1;
            continue;
        }
        let kind = pairs[i].1;
        i += 1;
        let start = i;
        while i < pairs.len() && pairs[i].0 != 0 {
            i += 1;
        }
        let a = &pairs[start..i];
        let number = |code: i32, default: Option<f64>| -> Result<f64, String> {
            let v = a
                .iter()
                .find(|v| v.0 == code)
                .map(|v| {
                    v.1.parse::<f64>()
                        .map_err(|_| "Invalid DXF number".to_owned())
                })
                .transpose()?
                .or(default)
                .ok_or("Missing DXF coordinate")?;
            if !v.is_finite() {
                return Err("Invalid DXF number".into());
            }
            Ok(v)
        };
        for (code, value) in a
            .iter()
            .filter(|(code, _)| [30, 31, 38, 39, 40, 41, 42, 43, 67, 210, 220, 230].contains(code))
        {
            let v = value.parse::<f64>().map_err(|_| "Invalid DXF number")?;
            if !v.is_finite() || v != if *code == 230 { 1. } else { 0. } {
                return Err(
                    "DXF 3D, paper-space, widths, bulges and custom OCS are not supported".into(),
                );
            }
        }
        let mut points = match kind {
            "LINE" => vec![
                Point::new(number(10, None)? * scale, number(20, None)? * scale),
                Point::new(number(11, None)? * scale, number(21, None)? * scale),
            ],
            "LWPOLYLINE" => {
                let mut points = vec![];
                let mut x = None;
                for (code, value) in a {
                    match code {
                        10 => {
                            if x.is_some() {
                                return Err("Invalid DXF vertex".into());
                            }
                            x = Some(value.parse::<f64>().map_err(|_| "Invalid DXF vertex")?);
                        }
                        20 => {
                            points.push(Point::new(
                                x.take().ok_or("Invalid DXF vertex")? * scale,
                                value.parse::<f64>().map_err(|_| "Invalid DXF vertex")? * scale,
                            ));
                        }
                        42 if value.parse::<f64>().map_err(|_| "Invalid DXF bulge")? != 0. => {
                            return Err("DXF bulges are not supported".into());
                        }
                        _ => {}
                    }
                }
                if x.is_some() || number(90, None)? as usize != points.len() {
                    return Err("Invalid DXF vertex count".into());
                }
                let flag = number(70, Some(0.))? as u32;
                if flag & !129 != 0 {
                    return Err("Unsupported DXF polyline flags".into());
                }
                if flag & 1 != 0 && points.len() > 1 {
                    points.push(points[0]);
                }
                points
            }
            _ => return Err(format!("Unsupported DXF entity: {kind}")),
        };
        if points.len() < 2 || points.iter().any(|p| !p.finite()) {
            return Err("Invalid DXF geometry".into());
        }
        let layer_name = a.iter().find(|v| v.0 == 8).map_or("0", |v| v.1);
        let layer = p
            .layers
            .layers
            .iter()
            .find(|v| v.name == layer_name)
            .map(|v| v.id)
            .unwrap_or_else(|| p.layers.new_layer(layer_name, None));
        let id = p.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: std::mem::take(&mut points),
                arcs: vec![],
            },
        );
        let e = p.elements.iter_mut().find(|v| v.id == id).unwrap();
        e.layer = crate::layers::ElementLayer::new(layer);
    }
    if !found || p.elements.is_empty() {
        return Err("DXF has no supported model-space entities".into());
    }
    p.validate()?;
    Ok(p)
}
