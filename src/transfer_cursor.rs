//! In-canvas cursor glyphs for parameter transfer and drawing tools.
//!
//! These follow the app's proven pencil-cursor pattern: hide the OS cursor
//! (`CursorIcon::None`) and paint the tool glyph at the pointer every frame.
//! Unlike an OS cursor bitmap this is visible on every platform, tracks the
//! snap position, and shows up in screenshots.

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferCursor {
    /// Eyedropper: pick up parameters.
    PickUp,
    /// Syringe: inject parameters.
    Inject,
}

const INK: Color32 = Color32::from_rgb(28, 32, 38);
const BODY: Color32 = Color32::from_rgb(240, 242, 246);
const LIQUID: Color32 = Color32::from_rgb(64, 190, 255);

fn halo_line(painter: &Painter, a: Pos2, b: Pos2, width: f32) {
    painter.line_segment([a, b], Stroke::new(width + 2.4, INK));
    painter.line_segment([a, b], Stroke::new(width, BODY));
}

/// Compact pipette with a tapered glass tube, collar and rounded rubber grip.
/// Its actual tip remains the pointer hotspot; blue liquid indicates a valid source.
pub fn paint_pick_up(painter: &Painter, pos: Pos2, primed: bool) {
    let axis = vec2(1., -1.).normalized();
    let across = vec2(1., 1.).normalized();
    let at = |length: f32, width: f32| pos + axis * length + across * width;
    let ink = Color32::from_rgb(45, 53, 60);
    let keyline = Color32::from_rgba_unmultiplied(255, 255, 255, 235);
    let tube = vec![
        at(0., 0.),
        at(4., -1.6),
        at(13., -1.6),
        at(13., 1.6),
        at(4., 1.6),
    ];
    painter.add(Shape::convex_polygon(
        tube.clone(),
        BODY,
        Stroke::new(2.4, keyline),
    ));
    painter.add(Shape::convex_polygon(tube, BODY, Stroke::new(0.9, ink)));
    // Capsule grip: a restrained solid silhouette instead of an oversized circle.
    let mut grip = Vec::with_capacity(26);
    for i in 0..=12 {
        let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.;
        grip.push(at(18.5 + 2.8 * angle.cos(), 2.8 * angle.sin()));
    }
    for i in 0..=12 {
        let angle = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.;
        grip.push(at(15. + 2.8 * angle.cos(), 2.8 * angle.sin()));
    }
    painter.add(Shape::convex_polygon(grip, ink, Stroke::new(0.8, keyline)));
    painter.line_segment([at(13., -3.4), at(13., 3.4)], Stroke::new(2.8, keyline));
    painter.line_segment([at(13., -3.4), at(13., 3.4)], Stroke::new(1.2, ink));
    if primed {
        // Keep liquid above the glass fill, inside the fine tube outline.
        painter.line_segment([at(4., 0.), at(10., 0.)], Stroke::new(1.4, LIQUID));
    }
}

/// Syringe with its needle tip at `pos`. `primed` fills the tip half of the
/// barrel with liquid.
pub fn paint_inject(painter: &Painter, pos: Pos2, primed: bool) {
    halo_line(painter, pos, pos + vec2(7., -7.), 1.3);
    // Hollow barrel: dark halo, white wall, dark interior.
    painter.line_segment(
        [pos + vec2(8., -8.), pos + vec2(19., -19.)],
        Stroke::new(8.6, INK),
    );
    painter.line_segment(
        [pos + vec2(8., -8.), pos + vec2(19., -19.)],
        Stroke::new(6., BODY),
    );
    painter.line_segment(
        [pos + vec2(9., -9.), pos + vec2(18., -18.)],
        Stroke::new(3.6, INK),
    );
    // Flange at the barrel base, plunger rod and head.
    halo_line(painter, pos + vec2(15.5, -4.5), pos + vec2(4.5, -15.5), 1.2);
    halo_line(painter, pos + vec2(20., -20.), pos + vec2(25., -25.), 1.6);
    halo_line(
        painter,
        pos + vec2(22.5, -27.5),
        pos + vec2(27.5, -22.5),
        2.,
    );
    // Keep the fill above the barrel walls and dark interior outline.
    if primed {
        let a = pos + vec2(10., -10.);
        let b = pos + vec2(14.5, -14.5);
        painter.line_segment([a, b], Stroke::new(4.2, INK));
        painter.line_segment([a, b], Stroke::new(2.8, LIQUID));
    }
}

/// AC29's simple crosshair for drawing tools, centred on the snap point.
pub fn paint_crosshair(painter: &Painter, pos: Pos2) {
    let inner = 1.5;
    let outer = 8.;
    for (a, b) in [
        (pos + vec2(0., -outer), pos + vec2(0., inner)),
        (pos + vec2(0., -inner), pos + vec2(0., outer)),
        (pos + vec2(-outer, 0.), pos + vec2(inner, 0.)),
        (pos + vec2(-inner, 0.), pos + vec2(outer, 0.)),
    ] {
        painter.line_segment([a, b], Stroke::new(2.2, INK));
        painter.line_segment([a, b], Stroke::new(1., BODY));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, RawInput};

    fn painted_shapes(paint: impl FnOnce(&Painter)) -> Vec<eframe::egui::epaint::ClippedShape> {
        let ctx = Context::default();
        let mut paint = Some(paint);
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            paint.take().expect("paint exactly once")(ui.painter());
        });
        output.textures_delta.clear();
        output.shapes
    }

    fn line_color(shape: &eframe::egui::Shape) -> Option<Color32> {
        match shape {
            Shape::LineSegment { stroke, .. } => Some(stroke.color),
            _ => None,
        }
    }

    #[test]
    fn primed_pickup_liquid_is_painted_over_the_eyedropper_body() {
        let shapes = painted_shapes(|painter| paint_pick_up(painter, Pos2::ZERO, true));
        let body = shapes
            .iter()
            .position(|clipped| matches!(&clipped.shape, Shape::Path(path) if path.fill == BODY))
            .expect("eyedropper body");
        let liquid = shapes
            .iter()
            .position(|clipped| line_color(&clipped.shape) == Some(LIQUID))
            .expect("visible liquid fill");
        assert!(liquid > body, "liquid must be layered over the body");
    }

    #[test]
    fn drawing_crosshair_is_simple_and_has_no_square_frame() {
        let shapes = painted_shapes(|painter| paint_crosshair(painter, Pos2::ZERO));
        assert_eq!(
            shapes.len(),
            8,
            "four fine crosshair arms with contrast halos"
        );
        assert!(
            shapes
                .iter()
                .all(|clipped| !matches!(clipped.shape, Shape::Rect(_)))
        );
    }
}

/// Compact original orbit cursor: fine dark arcs and solid tangent arrowheads.
/// A restrained pale keyline keeps the silhouette legible over dark geometry.
pub fn paint_orbit(painter: &Painter, pos: Pos2) {
    let radius = 8.5;
    let ink = Color32::from_rgb(45, 53, 60);
    let keyline = Color32::from_rgba_unmultiplied(255, 255, 255, 235);
    for start in [0.2_f32, std::f32::consts::PI + 0.2] {
        let sweep = 2.15;
        let points: Vec<_> = (0..=32)
            .map(|i| {
                let angle = start + sweep * i as f32 / 32.;
                pos + vec2(angle.cos(), angle.sin()) * radius
            })
            .collect();
        painter.add(Shape::line(points.clone(), Stroke::new(3., keyline)));
        painter.add(Shape::line(points, Stroke::new(1.4, ink)));
        let angle = start + sweep;
        let radial = vec2(angle.cos(), angle.sin());
        let tangent = vec2(-angle.sin(), angle.cos());
        let end = pos + radial * radius;
        painter.add(Shape::convex_polygon(
            vec![
                end + tangent * 2.5,
                end - tangent * 2.4 + radial * 2.6,
                end - tangent * 2.4 - radial * 2.6,
            ],
            ink,
            Stroke::new(0.8, keyline),
        ));
    }
}
