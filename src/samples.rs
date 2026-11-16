//! Original, deterministic synthetic scenes. No real project is an input.
//! Numeric IDs are stable regression handles, not imported project identities.
use crate::model::{Material, Point, Project, Shape};
use serde_json::json;

fn rectangle(x: f64, y: f64, w: f64, d: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + w, y),
        Point::new(x + w, y + d),
        Point::new(x, y + d),
    ]
}
fn add(p: &mut Project, id: u64, story: u32, shape: serde_json::Value) {
    p.next_id = id;
    let shape: Shape = serde_json::from_value(shape).expect("synthetic shape schema");
    p.add(story, Material::Plaster, shape);
    let e = p.elements.last_mut().unwrap();
    e.name = format!("Synthetic object {id}");
    if matches!(e.shape, Shape::Opening { .. }) {
        e.opening_joinery = Some(Default::default());
    }
}

/// Synthetic building grid for geometry, relationship and persistence tests.
/// Every dimension is authored here; no captured building plan is included.
pub fn building() -> Project {
    let mut p = Project::default();
    p.name = "Synthetic workshop".into();
    for id in 13..=110 {
        let story = if (26..=40).contains(&id)
            || (55..=58).contains(&id)
            || (64..=65).contains(&id)
            || (77..=94).contains(&id)
            || id >= 104
        {
            1
        } else if (41..=45).contains(&id) {
            2
        } else {
            0
        };
        let wall = (13..=24).contains(&id)
            || (26..=35).contains(&id)
            || (38..=40).contains(&id)
            || (42..=45).contains(&id);
        if wall {
            // Parallel test bays: 24m long, separated by 3m. This deliberately
            // has no correspondence to an inhabited building or site.
            let y = (id % 12) as f64 * 3.;
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Wall","start":{"x":0.,"y":y},"end":{"x":24.,"y":y},"thickness":0.2,"height":3.2}),
            );
        } else if [25, 36, 46].contains(&id) || (87..=94).contains(&id) {
            let x = if id >= 87 { (id - 87) as f64 * 2.5 } else { 0. };
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Slab","min":{"x":x,"y":0.},"max":{"x":x+2.,"y":2.},"thickness":0.2}),
            );
        } else if id == 37 {
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Stair","spec":{"start":{"x":26.,"y":0.},"end":{"x":26.,"y":5.4},"width":1.2,"total_rise":3.2,"risers":20,"slab_thickness":0.12,"handrail_height":1.}}),
            );
        } else if id == 41 || id == 47 {
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Roof","spec":{"min":{"x":0.,"y":0.},"max":{"x":24.,"y":12.},"pivot_start":{"x":0.,"y":0.},"pivot_end":{"x":24.,"y":0.},"pitch":10.,"thickness":0.2,"pivot_offset":3.2,"rise_left":true}}),
            );
        } else if id == 48 || id == 49 {
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Column","center":{"x":(id-48) as f64*6.,"y":-3.},"width":0.4,"depth":0.4,"height":3.2,"bottom_offset":0.,"axis_rotation":0.}),
            );
        } else if (50..=86).contains(&id) {
            let host = if story == 1 { 26 } else { 13 };
            let door = id < 66;
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Opening","host_wall":host,"opening":if door {"Door"}else{"Window"},"offset":1.+(id-50) as f64*0.5,"width":0.4,"height":if door{2.1}else{1.2},"sill":if door{0.}else{0.8}}),
            );
        } else {
            let i = id - 95;
            add(
                &mut p,
                id,
                story,
                json!({"kind":"Zone","spec":{"outline":rectangle((i%4) as f64*5.,(i/4) as f64*4.,4.,3.),"holes":[],"height":3.,"bottom_offset":0.,"purpose":if i==0 {"living"}else{"workshop"},"label_position":null}}),
            );
        }
    }
    p.next_id = 111;
    let names = [
        "客厅",
        "餐厅",
        "厨房",
        "书房",
        "工作间",
        "展厅",
        "休息室",
        "走廊",
        "门厅",
        "房间A",
        "房间B",
        "房间C",
        "房间D",
        "走道",
        "楼梯厅",
        "服务间",
    ];
    for e in &mut p.elements {
        if (95..=110).contains(&e.id) {
            e.name = names[(e.id - 95) as usize].into();
        }
    }
    p
}

/// Original comparison scene with three grid-space and three mass alternatives.
pub fn concept_study() -> Project {
    let mut p = building();
    p.name = "Synthetic courtyard study".into();
    p.design_options = serde_json::from_value(json!({"next_id":7,"options":[
        {"id":1,"name":"Grid baseline"},{"id":2,"name":"Grid A"},{"id":3,"name":"Grid B"},
        {"id":4,"name":"Mass baseline"},{"id":5,"name":"Mass A"},{"id":6,"name":"Mass B"}]}))
    .unwrap();
    for e in &mut p.elements {
        if (95..=103).contains(&e.id) {
            e.design_option = Some(1);
        }
    }
    for id in 111..=128 {
        let i = (id - 111) % 9;
        add(
            &mut p,
            id,
            0,
            json!({"kind":"Zone","spec":{"outline":rectangle((i%3) as f64*5.,(i/3) as f64*4.,4.,3.),"holes":[],"height":3.,"bottom_offset":0.,"purpose":"workshop","label_position":null}}),
        );
        p.elements.last_mut().unwrap().design_option = Some(if id < 120 { 2 } else { 3 });
    }
    for id in 129..=140 {
        let i = (id - 129) % 4;
        let (x, y, w, d, h, b) = match i {
            0 => (30., 30., 8., 5., 5.05, 0.),
            1 => (38., 30., 6., 4., 6.5, 1.25),
            2 => (30., 29.8, 8., 0.1, 0.85, 3.),
            _ => (38., 29.8, 6., 0.1, 0.85, 5.5),
        };
        add(
            &mut p,
            id,
            0,
            json!({"kind":"ConceptMass","spec":{"outline":rectangle(x,y,w,d),"holes":[],"height":h,"bottom_offset":b}}),
        );
        p.elements.last_mut().unwrap().design_option = Some((4 + (id - 129) / 4) as u32);
    }
    for id in 141..=142 {
        add(
            &mut p,
            id,
            0,
            json!({"kind":"Zone","spec":{"outline":rectangle(30.+(id-141) as f64*8.,30.,6.,3.),"holes":[],"height":2.8,"bottom_offset":0.,"purpose":"Unknown concept use","label_position":null}}),
        );
        p.elements.last_mut().unwrap().design_option = Some(5);
    }
    p.next_id = 143;
    p.plan_options.active_design_option = Some(5);
    let mut editor = crate::editor::Editor::new(p);
    use crate::constraints::{self, Basis, Definition, Domain, Edit, Metric, Relation, Scope};
    for (index, (domain, metric, scope, limit, relation)) in [
        (
            Domain::ConceptMasses,
            Metric::Height,
            Scope::Candidate {
                story: 0,
                option: Some(5),
            },
            5.5,
            Relation::AtMost,
        ),
        (
            Domain::ConceptMasses,
            Metric::ReferenceBoundaryClearance,
            Scope::Candidate {
                story: 0,
                option: Some(5),
            },
            3.,
            Relation::AtLeast,
        ),
        (
            Domain::ConceptMasses,
            Metric::FootprintAreaSum,
            Scope::Candidate {
                story: 0,
                option: Some(5),
            },
            70.,
            Relation::AtMost,
        ),
        (
            Domain::ConceptMasses,
            Metric::HorizontalSpacing,
            Scope::Objects {
                ids: vec![133, 134],
            },
            2.,
            Relation::AtLeast,
        ),
        (
            Domain::Zones,
            Metric::Height,
            Scope::Candidate {
                story: 0,
                option: Some(5),
            },
            3.1,
            Relation::AtMost,
        ),
        (
            Domain::Zones,
            Metric::FootprintAreaSum,
            Scope::Candidate {
                story: 0,
                option: Some(5),
            },
            70.,
            Relation::AtMost,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("synthetic-check-{}", index + 1);
        let definition = Definition {
            id: id.clone(),
            title: format!("Synthetic design assumption {}", index + 1),
            domain,
            metric,
            scope,
            limit: Some(limit),
            relation,
            basis: Basis::DesignAssumption {
                note: "Software example only; no survey, client requirement or statutory approval"
                    .into(),
            },
        };
        editor
            .apply(crate::command::ModelCommand::Constraint {
                edit: Edit::Put {
                    definition,
                    expected_sha256: None,
                },
            })
            .unwrap();
        let hash = constraints::expected(editor.project.rules.checks.last().unwrap());
        editor
            .apply(crate::command::ModelCommand::Constraint {
                edit: Edit::Run {
                    id,
                    expected_sha256: hash,
                },
            })
            .unwrap();
    }
    editor
        .apply(crate::command::ModelCommand::Diagram {
            edit: crate::diagrams::Edit::Create {
                name: "Synthetic concept checks".into(),
                definition: crate::diagrams::Definition {
                    story: 0,
                    option: Some(5),
                    domain: crate::diagrams::Domain::Site,
                    presentation: crate::diagrams::Presentation::ConstraintOverlay,
                },
            },
        })
        .unwrap();
    editor.project
}
