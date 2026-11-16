//! Hosted void, selectable proxy and display-only glazing share one wall source.
use crate::{
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity, SurfaceMesh},
    model::{Element, OpeningKind, Point, Project, Shape},
    wall_solid::{self, DerivedWall, WallCell},
};
use glam::DVec3;
pub const OPENING_DERIVATION_VERSION: u32 = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneRepresentation {
    DisplayBox,
    OpenDisplaySurface,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayPane {
    pub representation: PaneRepresentation,
    pub mesh: SurfaceMesh,
    lines: Vec<[Point; 2]>,
    bottom: f64,
    top: f64,
    box_outline: Option<[Point; 4]>,
}
impl DisplayPane {
    pub fn clipped(
        &self,
        clip: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<SurfaceMesh, String> {
        crate::geometry_validation::validate_clip_context(clip, range)?;
        if clip.is_some_and(|p| p.len() < 3 || p.iter().any(|p| !p.finite()))
            || range.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite())
        {
            return Err("Invalid glazing clipping context".into());
        }
        if let Some(outline) = self.box_outline {
            let ring = geo::LineString::from(
                outline
                    .iter()
                    .chain(outline.first())
                    .map(|p| (p.x, p.y))
                    .collect::<Vec<_>>(),
            );
            use geo::{Orient, orient::Direction};
            let polygon = geo::Polygon::new(ring, vec![]).orient(Direction::Default);
            return Ok(crate::sloped_prism::mesh(
                geo::MultiPolygon(vec![polygon]),
                crate::slab_geometry::bounds(&outline),
                self.top - self.bottom,
                |_| self.bottom,
                clip,
                range,
            ));
        }
        let (lo, hi) = range.map_or((self.bottom, self.top), |(a, b)| {
            (self.bottom.max(a.min(b)), self.top.min(a.max(b)))
        });
        let mut mesh = SurfaceMesh::default();
        if hi <= lo {
            return Ok(mesh);
        }
        let polygon = clip.map(|p| {
            geo::Polygon::new(
                geo::LineString::from(
                    p.iter()
                        .chain(p.first())
                        .map(|p| (p.x, p.y))
                        .collect::<Vec<_>>(),
                ),
                vec![],
            )
        });
        for [a, b] in &self.lines {
            use geo::BooleanOps;
            let line =
                geo::MultiLineString(vec![geo::LineString::from(vec![(a.x, a.y), (b.x, b.y)])]);
            let lines = polygon
                .as_ref()
                .map_or(line.clone(), |p| p.clip(&line, false));
            for l in lines {
                for p in l.0.windows(2) {
                    let q = [
                        DVec3::new(p[0].x, p[0].y, lo),
                        DVec3::new(p[1].x, p[1].y, lo),
                        DVec3::new(p[1].x, p[1].y, hi),
                        DVec3::new(p[0].x, p[0].y, hi),
                    ];
                    if (q[1] - q[0]).cross(q[2] - q[0]).length_squared() > 0. {
                        mesh.triangles
                            .extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
                    }
                }
            }
        }
        Ok(mesh)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningSnapshot {
    pub host_wall: u64,
    pub kind: OpeningKind,
    pub reference_span: [Point; 2],
    pub nominal_area_m2: f64,
    pub sampled_cut_volume_m3: f64,
    /// A selectable proxy of a VOID, never wall material or a library object.
    pub selectable_proxy: SurfaceMesh,
    pub bounds: Option<[DVec3; 2]>,
    pub pane: Option<DisplayPane>,
    pub joinery: Vec<(crate::opening_joinery::PartRole, DisplayPane)>,
    pub symbol: Vec<crate::opening_joinery::PlanPath>,
    pub plan_paths: Vec<Vec<Point>>,
    pub plan_triangles: Vec<[Point; 3]>,
    pub plan_centerline: Vec<Point>,
    cells: Vec<WallCell>,
    bottom: f64,
    top: f64,
}
impl OpeningSnapshot {
    pub fn clipped_proxy(
        &self,
        clip: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<SurfaceMesh, String> {
        crate::geometry_validation::validate_clip_context(clip, range)?;
        let mut mesh = if clip.is_none() && range.is_none() {
            self.selectable_proxy.clone()
        } else {
            wall_solid::selection_surface(&self.cells, self.bottom, self.top, clip, range)?.mesh
        };
        for (_, part) in &self.joinery {
            let other = part.clipped(clip, range)?;
            mesh.triangles.extend(other.triangles);
            mesh.edges.extend(other.edges);
        }
        Ok(mesh)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedOpening {
    pub identity: GeometryIdentity,
    pub host_identity: GeometryIdentity,
    pub geometry: OpeningSnapshot,
}
pub(crate) fn display_pane(
    host: &Element,
    wall: &wall_solid::WallSnapshot,
    h: &wall_solid::HostedVoid,
) -> Result<Option<DisplayPane>, String> {
    Ok(
        if h.kind == OpeningKind::Window && h.top_m - h.bottom_m > 0.06 {
            let bottom = h.bottom_m + 0.03;
            let top = h.top_m - 0.03;
            let rows: Vec<_> = wall
                .cross_sections
                .iter()
                .filter(|s| s.station_m >= h.from_m - 1e-5 && s.station_m <= h.to_m + 1e-5)
                .collect();
            let center = |s: &crate::wall_solid::WallSection| {
                Point::new(
                    (s.positive.x + s.negative.x) / 2.,
                    (s.positive.y + s.negative.y) / 2.,
                )
            };
            let lines: Vec<_> = rows
                .windows(2)
                .map(|w| [center(w[0]), center(w[1])])
                .collect();
            let box_outline = if host.wall_contour.is_none() {
                let a = center(
                    rows.first()
                        .ok_or_else(|| "Missing glazing start".to_owned())?,
                );
                let b = center(
                    rows.last()
                        .ok_or_else(|| "Missing glazing end".to_owned())?,
                );
                let length = a.distance(b);
                let normal = Point::new(-(b.y - a.y) / length * 0.02, (b.x - a.x) / length * 0.02);
                Some([
                    Point::new(a.x + normal.x, a.y + normal.y),
                    Point::new(b.x + normal.x, b.y + normal.y),
                    Point::new(b.x - normal.x, b.y - normal.y),
                    Point::new(a.x - normal.x, a.y - normal.y),
                ])
            } else {
                None
            };
            let mut pane = DisplayPane {
                representation: if box_outline.is_some() {
                    PaneRepresentation::DisplayBox
                } else {
                    PaneRepresentation::OpenDisplaySurface
                },
                mesh: SurfaceMesh::default(),
                lines,
                bottom,
                top,
                box_outline,
            };
            pane.mesh = pane.clipped(None, None)?;
            Some(pane)
        } else {
            None
        },
    )
}
pub(crate) fn derive_with_host(
    host: &Element,
    opening: &Element,
    wall: &DerivedWall,
    base: f64,
) -> Result<DerivedOpening, GeometryDiagnostic> {
    let fail =
        |m| GeometryDiagnostic::error(opening.id, "INVALID_OPENING_GEOMETRY", "hosted_void", m);
    let Shape::Opening {
        host_wall, kind, ..
    } = opening.shape
    else {
        return Err(fail("Element is not an opening".into()));
    };
    if host.id != host_wall || wall.identity.element_id != host_wall {
        return Err(fail(
            "Opening host identity differs from wall snapshot".into(),
        ));
    }
    let h = wall
        .geometry
        .voids
        .iter()
        .find(|h| h.id == opening.id)
        .ok_or_else(|| fail("Opening is not present in the wall snapshot".into()))?;
    if h.input_fingerprint
        != derived_geometry::input_fingerprint(&opening.shape, base).map_err(fail)?
    {
        return Err(fail("Opening inputs differ from host snapshot".into()));
    }
    let cells: Vec<_> = h
        .cell_indices
        .iter()
        .map(|&i| wall.geometry.cells[i].clone())
        .collect();
    let plan_paths =
        wall_solid::boundary_paths(&cells, &(0..cells.len()).collect::<Vec<_>>()).map_err(fail)?;
    let plan_triangles = cells
        .iter()
        .flat_map(|c| c.triangles.iter().copied())
        .collect();
    let plan_centerline: Vec<Point> = wall
        .geometry
        .cross_sections
        .iter()
        .filter(|s| s.station_m >= h.from_m - 1e-5 && s.station_m <= h.to_m + 1e-5)
        .map(|s| {
            Point::new(
                (s.positive.x + s.negative.x) / 2.,
                (s.positive.y + s.negative.y) / 2.,
            )
        })
        .collect();
    let proxy = wall_solid::selection_surface(&cells, h.bottom_m, h.top_m, None, None)
        .map_err(fail)?
        .mesh;
    let mut bounds = if proxy.triangles.is_empty() {
        None
    } else {
        Some(
            derived_geometry::inspect_closed_surface(&proxy.triangles)
                .map_err(fail)?
                .bounds,
        )
    };
    let settings = opening.opening_joinery.unwrap_or_default();
    settings.validate().map_err(fail)?;
    let fitting_span = [
        *plan_centerline.first().unwrap_or(&h.reference_span[0]),
        *plan_centerline.last().unwrap_or(&h.reference_span[1]),
    ];
    let generated = if host.wall_contour.is_none() {
        crate::opening_joinery::generate(
            fitting_span[0],
            fitting_span[1],
            h.bottom_m,
            h.top_m,
            kind,
            settings,
        )
    } else {
        Default::default()
    };
    let mut joinery = Vec::new();
    for part in generated.boxes {
        let mut box_part = DisplayPane {
            representation: PaneRepresentation::DisplayBox,
            mesh: SurfaceMesh::default(),
            lines: vec![],
            bottom: part.bottom,
            top: part.top,
            box_outline: Some(part.outline),
        };
        box_part.mesh = box_part.clipped(None, None).map_err(fail)?;
        for p in box_part.mesh.triangles.iter().flatten() {
            if let Some([min, max]) = bounds.as_mut() {
                *min = min.min(*p);
                *max = max.max(*p);
            }
        }
        joinery.push((part.role, box_part));
    }
    let mut pane = display_pane(host, &wall.geometry, h).map_err(fail)?;
    if let Some(p) = pane.as_mut() {
        let a = fitting_span[0];
        let b = fitting_span[1];
        let len = a.distance(b);
        let f = settings
            .frame_width
            .min(len * 0.49)
            .min((h.top_m - h.bottom_m) * 0.49);
        let u = Point::new((b.x - a.x) / len, (b.y - a.y) / len);
        let n = Point::new(-u.y * 0.008, u.x * 0.008);
        let a = Point::new(a.x + u.x * f, a.y + u.y * f);
        let b = Point::new(b.x - u.x * f, b.y - u.y * f);
        if host.wall_contour.is_none() {
            p.box_outline = Some([
                Point::new(a.x + n.x, a.y + n.y),
                Point::new(b.x + n.x, b.y + n.y),
                Point::new(b.x - n.x, b.y - n.y),
                Point::new(a.x - n.x, a.y - n.y),
            ]);
            p.bottom = h.bottom_m + f;
            p.top = h.top_m - f;
            p.mesh = p.clipped(None, None).map_err(fail)?;
        }
    }
    let identity = derived_geometry::identity(
        opening.id,
        &(
            &opening.shape,
            opening.opening_joinery,
            wall.identity.input_fingerprint,
        ),
        base,
        OPENING_DERIVATION_VERSION,
    )
    .map_err(fail)?;
    Ok(DerivedOpening {
        identity,
        host_identity: wall.identity.clone(),
        geometry: OpeningSnapshot {
            host_wall,
            kind,
            reference_span: h.reference_span,
            nominal_area_m2: h.nominal_area_m2,
            sampled_cut_volume_m3: h.sampled_cut_volume_m3,
            selectable_proxy: proxy,
            bounds,
            pane,
            joinery,
            symbol: generated.plan,
            plan_paths,
            plan_triangles,
            plan_centerline,
            cells,
            bottom: h.bottom_m,
            top: h.top_m,
        },
    })
}
pub fn derive(project: &Project, opening: &Element) -> Result<DerivedOpening, GeometryDiagnostic> {
    let Shape::Opening { host_wall, .. } = opening.shape else {
        return Err(GeometryDiagnostic::error(
            opening.id,
            "NOT_OPENING",
            "element",
            "Element is not an opening",
        ));
    };
    let host = project
        .elements
        .iter()
        .find(|e| e.id == host_wall && matches!(e.shape, Shape::Wall { .. }))
        .ok_or_else(|| {
            GeometryDiagnostic::error(
                opening.id,
                "OPENING_HOST_MISSING",
                "host",
                "Opening references a missing host wall",
            )
        })?;
    let wall = wall_solid::derive(project, host)?;
    derive_with_host(host, opening, &wall, project.elevation(host.story))
}
