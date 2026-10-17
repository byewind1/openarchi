use eframe::egui::{Color32, CursorIcon, Painter, Pos2, Shape, Stroke, vec2};

/// Semantic cursor feedback for interactions OpenArchi currently supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorFeedback {
    Empty,
    Selectable,
    Draggable,
    Arrow,
    ArrowWithCheck,
    ArrowWithTripleRay,
    Drawing,
    SnapTarget,
    Dragging,
    SolidPencil,
    HollowPencil,
    HollowPencilWithLine,
}

impl CursorFeedback {
    pub fn icon(self) -> CursorIcon {
        match self {
            Self::Empty => CursorIcon::Default,
            Self::Selectable => CursorIcon::Default,
            Self::Draggable => CursorIcon::Grab,
            Self::Arrow | Self::ArrowWithCheck | Self::ArrowWithTripleRay => CursorIcon::Default,
            Self::Drawing => CursorIcon::Crosshair,
            Self::SnapTarget => CursorIcon::PointingHand,
            Self::Dragging => CursorIcon::Grabbing,
            Self::SolidPencil | Self::HollowPencil | Self::HollowPencilWithLine => CursorIcon::None,
        }
    }

    pub fn is_custom_pencil(self) -> bool {
        matches!(
            self,
            Self::SolidPencil | Self::HollowPencil | Self::HollowPencilWithLine
        )
    }

    fn is_custom_arrow(self) -> bool {
        matches!(
            self,
            Self::Arrow | Self::ArrowWithCheck | Self::ArrowWithTripleRay
        )
    }

    /// Paint an original, small pencil cue with its hotspot at the pointer.
    pub fn paint(self, painter: &Painter, pos: Pos2) {
        if self.is_custom_arrow() {
            match self {
                Self::ArrowWithCheck => Self::paint_check(painter, pos + vec2(12., -2.)),
                Self::ArrowWithTripleRay => Self::paint_triple_ray(painter, pos + vec2(14., -2.)),
                _ => {}
            }
            return;
        }
        if !self.is_custom_pencil() {
            return;
        }
        let ink = Color32::from_rgb(38, 43, 46);
        let fill = if self == Self::SolidPencil {
            ink
        } else {
            Color32::WHITE
        };
        // The graphite tip is the hotspot; the long barrel points up-right.
        painter.add(Shape::convex_polygon(
            vec![
                pos + vec2(3., -8.),
                pos + vec2(18., -23.),
                pos + vec2(23., -18.),
                pos + vec2(8., -3.),
            ],
            fill,
            Stroke::new(1.2, ink),
        ));
        painter.add(Shape::convex_polygon(
            vec![pos, pos + vec2(3., -8.), pos + vec2(8., -3.)],
            Color32::from_rgb(231, 205, 159),
            Stroke::new(1., ink),
        ));
        painter.line_segment([pos, pos + vec2(2., -2.)], Stroke::new(2., ink));
        painter.line_segment(
            [pos + vec2(6., -6.), pos + vec2(20., -20.)],
            Stroke::new(0.8, ink),
        );
        painter.line_segment(
            [pos + vec2(16., -21.), pos + vec2(21., -16.)],
            Stroke::new(1.5, ink),
        );
        if self == Self::HollowPencilWithLine {
            painter.line_segment(
                [pos + vec2(12., -2.), pos + vec2(21., -2.)],
                Stroke::new(1.4, Color32::from_rgb(46, 135, 181)),
            );
        }
    }

    fn paint_check(painter: &Painter, at: Pos2) {
        let a = at + vec2(0., 4.);
        let b = at + vec2(3., 7.);
        let c = at + vec2(9., 0.);
        painter.line_segment([a, b], Stroke::new(3.4, Color32::WHITE));
        painter.line_segment([b, c], Stroke::new(3.4, Color32::WHITE));
        painter.line_segment([a, b], Stroke::new(1.6, Color32::from_rgb(20, 25, 29)));
        painter.line_segment([b, c], Stroke::new(1.6, Color32::from_rgb(20, 25, 29)));
    }

    fn paint_triple_ray(painter: &Painter, center: Pos2) {
        let radius = 5.5;
        for angle in [-90_f32, 30., 150.] {
            let end = center + vec2(angle.to_radians().cos(), angle.to_radians().sin()) * radius;
            painter.line_segment([center, end], Stroke::new(3.4, Color32::WHITE));
            painter.line_segment(
                [center, end],
                Stroke::new(1.6, Color32::from_rgb(20, 25, 29)),
            );
        }
    }
}
