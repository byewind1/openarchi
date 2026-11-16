//! Derived drawing feedback only; accepted input and commit geometry stay owned
//! by the editor. Filled candidates must be painted separately without a stroke.
use super::*;

pub(super) enum DrawingPreview<'a> {
    /// Confirmed edges followed by the single current input segment. Even a
    /// contour tool has no closing edge until the pointer actually returns.
    Chain {
        confirmed: &'a [Point],
        cursor: Option<Point>,
    },
    /// An already fully determined boundary, e.g. two-corner rectangle.
    Boundary(&'a [Point]),
}

impl DrawingPreview<'_> {
    pub(super) fn paint(self, painter: &Painter, screen: impl Fn(Point) -> Pos2, stroke: Stroke) {
        let (points, closed) = match self {
            Self::Chain { confirmed, cursor } => {
                if confirmed.is_empty() {
                    return;
                }
                (
                    confirmed
                        .iter()
                        .copied()
                        .chain(cursor)
                        .map(screen)
                        .collect(),
                    false,
                )
            }
            Self::Boundary(points) => (points.iter().copied().map(screen).collect(), true),
        };
        let points: Vec<Pos2> = points;
        if points.len() >= 2 {
            painter.add(if closed {
                egui::Shape::closed_line(points, stroke)
            } else {
                egui::Shape::line(points, stroke)
            });
        }
    }
}
