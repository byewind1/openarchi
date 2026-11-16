//! Typed native joinery, shared plan symbols and editable opening dimensions.
use crate::model::{Element, OpeningKind, Point, Project, Shape};
use serde::{Deserialize, Serialize};
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DoorStyle {
    #[default]
    Single,
    Double,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OpeningJoinery {
    pub door_style: DoorStyle,
    pub hinge_at_start: bool,
    pub opens_positive: bool,
    pub frame_width: f64,
    pub frame_depth: f64,
    pub leaf_thickness: f64,
}
impl Default for OpeningJoinery {
    fn default() -> Self {
        Self {
            door_style: DoorStyle::Single,
            hinge_at_start: true,
            opens_positive: true,
            frame_width: 0.05,
            frame_depth: 0.08,
            leaf_thickness: 0.04,
        }
    }
}
impl OpeningJoinery {
    pub fn default_width(self) -> f64 {
        if self.door_style == DoorStyle::Double {
            1.5
        } else {
            0.9
        }
    }
    pub fn validate(self) -> Result<(), String> {
        if ![self.frame_width, self.frame_depth, self.leaf_thickness]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.005 && *v <= 0.3)
        {
            return Err("Invalid opening frame or leaf dimensions".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartRole {
    Frame,
    Leaf,
}
#[derive(Clone, Debug)]
pub struct JoineryBox {
    pub outline: [Point; 4],
    pub bottom: f64,
    pub top: f64,
    pub role: PartRole,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlanPath {
    pub points: Vec<Point>,
    pub closed: bool,
    pub arc: bool,
}
#[derive(Clone, Debug, Default)]
pub struct JoineryGeometry {
    pub boxes: Vec<JoineryBox>,
    pub plan: Vec<PlanPath>,
}
pub fn generate(
    a: Point,
    b: Point,
    bottom: f64,
    top: f64,
    kind: OpeningKind,
    settings: OpeningJoinery,
) -> JoineryGeometry {
    let width = a.distance(b);
    let h = top - bottom;
    let u = Point::new((b.x - a.x) / width, (b.y - a.y) / width);
    let n = Point::new(-u.y, u.x);
    // Legacy tiny voids remain readable. Fitting dimensions never consume their complete opening.
    let f = settings.frame_width.min(width * 0.49).min(h * 0.49);
    let depth = settings.frame_depth;
    let side = if settings.opens_positive { 1. } else { -1. };
    let p = |x: f64, y: f64| Point::new(a.x + u.x * x + n.x * y, a.y + u.y * x + n.y * y);
    let mut g = JoineryGeometry::default();
    let mut cuboid =
        |x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64, role: PartRole, plan: bool| {
            let outline = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
            g.boxes.push(JoineryBox {
                outline,
                bottom: z0,
                top: z1,
                role,
            });
            if plan {
                g.plan.push(PlanPath {
                    points: outline.to_vec(),
                    closed: true,
                    arc: false,
                });
            }
        };
    cuboid(
        0.,
        f,
        -depth / 2.,
        depth / 2.,
        bottom,
        top,
        PartRole::Frame,
        true,
    );
    cuboid(
        width - f,
        width,
        -depth / 2.,
        depth / 2.,
        bottom,
        top,
        PartRole::Frame,
        true,
    );
    cuboid(
        f,
        width - f,
        -depth / 2.,
        depth / 2.,
        top - f,
        top,
        PartRole::Frame,
        false,
    );
    if kind == OpeningKind::Window {
        cuboid(
            f,
            width - f,
            -depth / 2.,
            depth / 2.,
            bottom,
            bottom + f,
            PartRole::Frame,
            false,
        );
        for y in [-0.008, 0.008] {
            g.plan.push(PlanPath {
                points: vec![p(f, y), p(width - f, y)],
                closed: false,
                arc: false,
            });
        }
        return g;
    }
    let clear = width - 2. * f;
    let leaves = if settings.door_style == DoorStyle::Double {
        2
    } else {
        1
    };
    let leaf_width = clear / f64::from(leaves);
    for i in 0..leaves {
        let start = if leaves == 2 {
            i == 0
        } else {
            settings.hinge_at_start
        };
        let hinge = if start { f } else { width - f };
        let direction = if start { 1. } else { -1. };
        let y0 = side * depth / 2.;
        let thick = settings.leaf_thickness.min(leaf_width * 0.49);
        let outline = [
            p(hinge - thick / 2., y0),
            p(hinge + thick / 2., y0),
            p(hinge + thick / 2., y0 + side * leaf_width),
            p(hinge - thick / 2., y0 + side * leaf_width),
        ];
        g.boxes.push(JoineryBox {
            outline,
            bottom: bottom + 0.01_f64.min(h * 0.05),
            top: top - f,
            role: PartRole::Leaf,
        });
        g.plan.push(PlanPath {
            points: outline.to_vec(),
            closed: true,
            arc: false,
        });
        let arc = (0..=24)
            .map(|j| {
                let angle = f64::from(j) / 24. * std::f64::consts::FRAC_PI_2;
                p(
                    hinge + direction * leaf_width * angle.cos(),
                    y0 + side * leaf_width * angle.sin(),
                )
            })
            .collect();
        g.plan.push(PlanPath {
            points: arc,
            closed: false,
            arc: true,
        });
    }
    g
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
}
/// Fixed opposite edge; width changes also update along-wall centre offset.
pub fn resized(
    project: &Project,
    element: &Element,
    edge: ResizeEdge,
    delta: f64,
) -> Result<Element, String> {
    if !delta.is_finite() {
        return Err("Opening resize must be finite".into());
    }
    let mut e = element.clone();
    let Shape::Opening {
        width,
        height,
        sill,
        offset,
        ..
    } = &mut e.shape
    else {
        return Err("Element is not an opening".into());
    };
    match edge {
        ResizeEdge::Left => {
            *width -= delta;
            *offset += delta / 2.;
        }
        ResizeEdge::Right => {
            *width += delta;
            *offset += delta / 2.;
        }
        ResizeEdge::Top => *height += delta,
        ResizeEdge::Bottom => {
            *height -= delta;
            *sill += delta;
        }
    }
    let mut candidate = project.clone();
    let entry = candidate
        .elements
        .iter_mut()
        .find(|x| x.id == e.id)
        .ok_or("The selected opening no longer exists")?;
    *entry = e.clone();
    candidate.validate()?;
    Ok(e)
}
