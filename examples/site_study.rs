//! Generate the original public site study; no external project is read.
use atelier_bim::{
    cad_reference as cad,
    concept_mass::ConceptMassSpec,
    model::{Material, Point, Project, Shape},
    storage,
};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("new output directory required")?,
    );
    std::fs::create_dir(&output).map_err(|e| e.to_string())?;
    let mut text =
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n"
            .to_owned();
    let mut line = |a: [f64; 2], b: [f64; 2]| {
        text.push_str(&format!(
            "0\nLINE\n8\nSITE\n10\n{}\n20\n{}\n11\n{}\n21\n{}\n",
            a[0], a[1], b[0], b[1]
        ))
    };
    for (a, b) in [
        ([0., 0.], [60., 0.]),
        ([60., 0.], [60., 40.]),
        ([60., 40.], [0., 40.]),
        ([0., 40.], [0., 0.]),
        ([-4., -6.], [64., -6.]),
        ([-4., -10.], [64., -10.]),
        ([-6., -12.], [-6., 44.]),
        ([-10., -12.], [-10., 44.]),
        ([0., 20.], [60., 20.]),
        ([30., 0.], [30., 40.]),
    ] {
        line(a, b);
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    let drawing = output.join("synthetic-site.dxf");
    std::fs::write(&drawing, text).map_err(|e| e.to_string())?;
    let mut source = cad::prepare(&drawing, None, &output.join("cache"))?;
    source.linked_path = "synthetic-site.dxf".into();
    let (mut document, _, _) = cad::attach(
        &Default::default(),
        source,
        None,
        cad::Transform {
            metres_per_unit: 1.,
            ..Default::default()
        },
        true,
    )?;
    document.cad_underlays[0].show_3d = true;
    document.cad_underlays[0].show_plan = true;
    let mut project = Project::default();
    project.name = "合成场地 · 方案推敲".into();
    project.drawings = document;
    for (x, y, w, d, h) in [
        (8., 6., 18., 8., 6.),
        (34., 6., 16., 8., 9.),
        (8., 26., 12., 8., 4.5),
        (34., 26., 16., 8., 6.),
    ] {
        let id = project.add(
            0,
            Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    outline: vec![
                        Point::new(x, y),
                        Point::new(x + w, y),
                        Point::new(x + w, y + d),
                        Point::new(x, y + d),
                    ],
                    height: h,
                    ..Default::default()
                },
            },
        );
        project.elements.last_mut().unwrap().name = format!("合成体量 {id}");
    }
    project.validate()?;
    storage::save(&project, &output.join("site-study.openarchi.json"))?;
    let reopened = storage::load(&output.join("site-study.openarchi.json"))?;
    assert_eq!(project.elements, reopened.elements);
    assert_eq!(reopened.drawings.cad_underlays.len(), 1);
    Ok(())
}
