use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    automation::{ReadService, TerrainRequest},
    automation_registry::{DiscoveredBatch, NativeBatch},
    command::ModelCommand,
    editor::Editor,
    model::Point,
    project_repository::{
        LengthUnit, Repository,
        terrain::{TerrainPlacement, parse_obj},
    },
    render::{self, SceneView},
    site::terrain::{Edit, Geometry, Origin, Placement, Proposal, Terrain},
    storage,
};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "oa-terrain-{}",
            &storage::new_document_identity()[..16]
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn geometry() -> Geometry {
    Geometry {
        vertices: vec![[0., 0., 2.], [8., 0., 2.8], [8., 6., 4.], [0., 6., 3.2]],
        faces: vec![[0, 1, 2], [0, 2, 3]],
    }
}
fn source() -> Vec<u8> {
    b"# synthetic millimetre terrain, not a real survey\nv 0 0 2000\nv 8000 0 2800\nv 8000 6000 4000\nv 0 6000 3200\nf 1 2 3\nf 1 3 4\n".to_vec()
}
fn terrain() -> Terrain {
    Terrain {
        name: "Synthetic terrain".into(),
        origin: Origin {
            source_sha256: "a".repeat(64),
            unit: LengthUnit::Metre,
            placement: Placement::ModelXyz,
        },
        geometry: geometry(),
        imported: geometry(),
    }
}
fn proposal() -> Proposal {
    Proposal {
        terrain: terrain(),
        expected_previous: None,
    }
}
fn cmd(p: Proposal) -> ModelCommand {
    ModelCommand::Terrain {
        edit: Edit::Import { proposal: p },
    }
}
#[test]
fn analytic_plane_edges_outside_and_open_hole_are_not_extrapolated() {
    let g = geometry();
    g.validate().unwrap();
    for xy in [[0., 0.], [3., 2.], [8., 6.], [4., 3.]] {
        let z = g.height_at(xy).unwrap().unwrap().0;
        assert!((z - (2. + 0.1 * xy[0] + 0.2 * xy[1])).abs() < 1e-12);
    }
    assert_eq!(g.height_at([-1e-7, 0.]).unwrap(), None);
    let m = g.metrics().unwrap();
    assert_eq!(m.projected_area_m2, 48.);
    assert!((m.surface_area_m2 - 48. * 1.05_f64.sqrt()).abs() < 1e-12);
    assert_eq!(m.boundary_edges, 4);
    let mut hole = Geometry {
        vertices: vec![
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 4., 0.],
            [0., 4., 0.],
            [1., 1., 0.],
            [3., 1., 0.],
            [3., 3., 0.],
            [1., 3., 0.],
        ],
        faces: vec![
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    };
    hole.validate().unwrap();
    assert_eq!(hole.height_at([2., 2.]).unwrap(), None);
    assert_eq!(hole.metrics().unwrap().projected_area_m2, 12.);
    hole.vertices[0][2] = f64::NAN;
    assert!(hole.validate().is_err());
}
#[test]
fn invalid_duplicate_overlapping_or_nonmanifold_surfaces_refuse() {
    let mut cases = vec![];
    let mut g = geometry();
    g.faces.push(g.faces[0]);
    cases.push(g);
    let mut g = geometry();
    g.faces[1] = [0, 1, 3];
    cases.push(g);
    let mut g = geometry();
    g.vertices.push([0., 0., 10.]);
    g.faces.push([0, 2, 4]);
    cases.push(g);
    let mut g = geometry();
    g.vertices.extend([[2., -2., 0.], [3., -3., 0.]]);
    g.faces.extend([[1, 0, 4], [1, 0, 5]]);
    cases.push(g);
    let mut g = geometry();
    g.faces[0] = [0, 9, 2];
    cases.push(g);
    let mut g = geometry();
    g.faces[0] = [2, 1, 0];
    cases.push(g);
    for g in cases {
        assert!(g.validate().is_err());
    }
}
#[test]
fn obj_is_strict_and_units_source_quality_lock_and_mapping_are_verified() {
    let f = Fixture::new();
    let editor = Editor::new(storage::startup_example());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &editor).unwrap();
    let file = f.0.join("survey.obj");
    fs::write(&file, source()).unwrap();
    let id = repo.add_source(&file).unwrap();
    assert!(
        repo.preview_terrain(
            &editor.project.site,
            &id,
            TerrainPlacement::ModelXyz,
            "Terrain".into()
        )
        .is_err()
    );
    repo.confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    let p = repo
        .preview_terrain(
            &editor.project.site,
            &id,
            TerrainPlacement::ModelXyz,
            "Terrain".into(),
        )
        .unwrap();
    assert_eq!(p.terrain.geometry.faces, geometry().faces);
    for (a, b) in p
        .terrain
        .geometry
        .vertices
        .iter()
        .flatten()
        .zip(geometry().vertices.iter().flatten())
    {
        assert!((a - b).abs() < 1e-12);
    }
    let guard = repo.guard_terrain_commit(&p).unwrap();
    assert!(repo.reject_source(&id, "test").is_err());
    drop(guard);
    let mut forged = p.clone();
    forged.terrain.geometry.vertices[0][2] += 1.;
    forged.terrain.imported = forged.terrain.geometry.clone();
    assert!(repo.guard_terrain_commit(&forged).is_err());
    repo.reject_source(&id, "unverified fixture excluded")
        .unwrap();
    assert!(repo.guard_terrain_commit(&p).is_err());
    for extra in [
        "vt 0 0\n",
        "f 1 2 3 4\n",
        "f -1 -2 -3\n",
        "v 0 0 0 1\n",
        "usemtl grass\n",
    ] {
        let mut bytes = source();
        bytes.extend(extra.as_bytes());
        assert!(parse_obj(&bytes).is_err());
    }
}
#[test]
fn host_edit_undo_native_reopen_and_reimport_preserve_geometry_and_user_edits() {
    let f = Fixture::new();
    let original = storage::startup_example();
    let mut host = ApplicationHost::new(original.clone());
    let receipt = host
        .dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "terrain".into(),
                commands: vec![cmd(proposal())],
            },
        )
        .unwrap()
        .unwrap();
    assert!(receipt.site_context_changed);
    assert!(receipt.changed_ids.is_empty());
    assert_eq!(host.document.editor.project.elements, original.elements);
    let after = host.document.editor.project.clone();
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, original);
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project, after);
    let file = f.0.join("site.json");
    storage::save_document(&after, &file, host.document.editor.project_identity()).unwrap();
    assert_eq!(Editor::from_stored_document(&file).unwrap().project, after);
    let mut context = after.site.clone();
    context.terrain = None;
    assert!(
        host.document
            .editor
            .apply(ModelCommand::SetSiteContext { context })
            .is_err()
    );
    host.document
        .editor
        .apply(ModelCommand::Terrain {
            edit: Edit::UpdateVertex {
                index: 0,
                expected: [0., 0., 2.],
                position: [0., 0., 2.2],
            },
        })
        .unwrap();
    let changed = host.document.editor.project.clone();
    assert!(changed.site.terrain.as_ref().unwrap().user_modified());
    let mut p = proposal();
    p.expected_previous = changed.site.terrain.clone();
    assert!(host.document.editor.apply(cmd(p)).is_err());
    assert_eq!(host.document.editor.project, changed);
    host.document
        .editor
        .apply(ModelCommand::Terrain {
            edit: Edit::Remove {
                expected: changed.site.terrain.unwrap(),
            },
        })
        .unwrap();
    assert!(host.document.editor.project.site.terrain.is_none());
    host.document.editor.undo();
    assert!(host.document.editor.project.site.terrain.is_some());
}
#[test]
fn clipping_and_scene_use_the_same_surface_without_a_selection_proxy() {
    let mut p = atelier_bim::model::Project::default();
    p.grid_settings.show_construction_grid = false;
    p.site.terrain = Some(terrain());
    let view = SceneView {
        selected: &[],
        hovered: None,
        story: None,
        marquee: None,
        outside_marquee: false,
        selection_only: false,
    };
    let scene = render::prepare_scene(&p, view, None);
    assert_eq!(scene.triangle_count(), 2);
    assert!(scene.diagnostics.is_empty());
    let only = render::prepare_scene(
        &p,
        SceneView {
            selection_only: true,
            ..view
        },
        None,
    );
    assert_eq!(only.triangle_count(), 0);
    let clip = vec![
        Point::new(2., 1.),
        Point::new(6., 1.),
        Point::new(6., 5.),
        Point::new(2., 5.),
    ];
    let mesh = geometry().mesh(Some(&clip), Some((2.5, 3.3))).unwrap();
    assert!(!mesh.triangles.is_empty());
    for v in mesh.triangles.into_iter().flatten() {
        assert!(v.x >= 2. - 1e-8 && v.x <= 6. + 1e-8 && v.y >= 1. - 1e-8 && v.y <= 5. + 1e-8);
        assert!(v.z >= 2.5 - 1e-8 && v.z <= 3.3 + 1e-8);
        assert!((v.z - (2. + 0.1 * v.x + 0.2 * v.y)).abs() < 1e-8);
    }
}
#[test]
fn height_read_service_is_revision_bound_and_both_ai_adapters_share_terrain_edits() {
    let mut editor = Editor::new(storage::startup_example());
    let service = ReadService::new("test".into());
    let q = TerrainRequest {
        target: service.target(&editor),
        expected_revision: 0,
        xy_m: [3., 2.],
    };
    assert_eq!(
        service.terrain_height(&editor, &q).unwrap()["sample"]["status"],
        "unknown"
    );
    editor.apply(cmd(proposal())).unwrap();
    assert!(service.terrain_height(&editor, &q).is_err());
    let q = TerrainRequest {
        expected_revision: editor.revision,
        ..q
    };
    assert!(
        (service.terrain_height(&editor, &q).unwrap()["sample"]["elevation_model_m"]
            .as_f64()
            .unwrap()
            - 2.7)
            .abs()
            < 1e-12
    );
    let wire = serde_json::json!({"version":1,"edits":[{"operation":"terrain","edit":{"operation":"import","proposal":proposal()}}]});
    let a: NativeBatch = serde_json::from_value(wire.clone()).unwrap();
    let b: DiscoveredBatch = serde_json::from_value(wire).unwrap();
    assert_eq!(
        a.lower(&editor).unwrap(),
        b.resolve().unwrap().lower(&editor).unwrap()
    );
    assert!(atelier_bim::automation_registry::describe("terrain").is_some());
}

#[test]
fn projected_datum_conversion_controls_and_reference_changes_are_explicit() {
    use atelier_bim::site::{
        CoordinateControl, HorizontalReference, SiteContext, SiteEvidence, VerticalReference,
    };
    let f = Fixture::new();
    let mut editor = Editor::new(storage::startup_example());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &editor).unwrap();
    let path = f.0.join("projected.obj");
    fs::write(
        &path,
        b"v 10 10 102\nv 18 10 102.8\nv 18 16 104\nv 10 16 103.2\nf 1 2 3\nf 1 3 4\n",
    )
    .unwrap();
    let id = repo.add_source(&path).unwrap();
    repo.confirm_unit(&id, Some(LengthUnit::Metre)).unwrap();
    assert!(
        repo.preview_terrain(
            &editor.project.site,
            &id,
            TerrainPlacement::SiteReference,
            "Projected".into()
        )
        .is_err()
    );
    let evidence = SiteEvidence {
        source_sha256: id.clone(),
        locator: "Synthetic controls only".into(),
        note: String::new(),
    };
    let mut site = SiteContext {
        horizontal: Some(HorizontalReference {
            crs: "Synthetic EN metres".into(),
            origin_east_north_m: [10., 10.],
            model_to_grid_ccw_deg: 0.,
            evidence: evidence.clone(),
        }),
        vertical: Some(VerticalReference {
            datum: "Synthetic datum".into(),
            model_zero_elevation_m: 100.,
            evidence: evidence.clone(),
        }),
        controls: vec![
            CoordinateControl {
                model_xy_m: [0., 0.],
                reference_east_north_m: [10., 10.],
                evidence: evidence.clone(),
            },
            CoordinateControl {
                model_xy_m: [8., 0.],
                reference_east_north_m: [18., 10.],
                evidence,
            },
        ],
        control_tolerance_m: Some(0.000001),
        ..Default::default()
    };
    editor
        .apply(ModelCommand::SetSiteContext {
            context: site.clone(),
        })
        .unwrap();
    let p = repo
        .preview_terrain(
            &site,
            &id,
            TerrainPlacement::SiteReference,
            "Projected".into(),
        )
        .unwrap();
    assert!((p.terrain.geometry.height_at([3., 2.]).unwrap().unwrap().0 - 2.7).abs() < 1e-12);
    site.controls.clear();
    editor
        .apply(ModelCommand::SetSiteContext {
            context: site.clone(),
        })
        .unwrap();
    assert!(editor.apply(cmd(p.clone())).is_err());
    editor.undo();
    editor.apply(cmd(p)).unwrap();
    let before = editor.project.elements.clone();
    site = editor.project.site.clone();
    site.horizontal.as_mut().unwrap().origin_east_north_m[0] += 1.;
    editor
        .apply(ModelCommand::SetSiteContext { context: site })
        .unwrap();
    assert!(
        !editor
            .project
            .site
            .terrain
            .as_ref()
            .unwrap()
            .reference_current(&editor.project.site)
    );
    assert_eq!(editor.project.elements, before);
}

#[test]
fn scene_cache_rebuilds_for_terrain_only_changes_and_nonconforming_edges_refuse() {
    use atelier_bim::{
        derivation_cache::DerivationCache,
        editor::{MarqueeArea, MarqueeScope},
        render::CpuSceneCache,
    };
    let mut p = atelier_bim::model::Project::default();
    p.grid_settings.show_construction_grid = false;
    p.site.terrain = Some(terrain());
    let mut cache = DerivationCache::default();
    let mut scenes = CpuSceneCache::default();
    let view = SceneView {
        selected: &[],
        hovered: None,
        story: None,
        marquee: None,
        outside_marquee: false,
        selection_only: false,
    };
    let first = {
        let mut read = cache.read(&p, 0, 1).unwrap();
        scenes.prepare(&p, view, &mut read).clone()
    };
    p.site.terrain.as_mut().unwrap().geometry.vertices[0][2] += 0.2;
    let second = {
        let mut read = cache.read(&p, 1, 1).unwrap();
        scenes.prepare(&p, view, &mut read).clone()
    };
    assert_ne!(first.content_fingerprint(), second.content_fingerprint());
    assert_eq!(second, render::prepare_scene(&p, view, None));
    assert_eq!(scenes.stats().builds, 2);
    let area = MarqueeArea::new(
        vec![
            Point::new(-1., -1.),
            Point::new(9., -1.),
            Point::new(9., 7.),
            Point::new(-1., 7.),
        ],
        MarqueeScope::AllStories,
        0,
    )
    .unwrap();
    let outside = render::prepare_scene(
        &p,
        SceneView {
            marquee: Some(&area),
            outside_marquee: true,
            ..view
        },
        None,
    );
    assert_eq!(outside.triangle_count(), 0);
    let bad = Geometry {
        vertices: vec![
            [0., 0., 0.],
            [8., 0., 0.],
            [0., 4., 0.],
            [4., 0., 2.],
            [4., -4., 2.],
            [8., -4., 2.],
        ],
        faces: vec![[0, 1, 2], [3, 4, 5]],
    };
    assert!(bad.validate().is_err());
    assert!(bad.height_at([4., 0.]).is_err());
}

#[cfg(unix)]
#[test]
fn terrain_height_is_available_over_readonly_ipc_without_mutation() {
    use atelier_bim::automation_ipc::Endpoint;
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixStream,
        sync::mpsc,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    let socket = f.0.join("ipc/app.sock");
    let mut endpoint = Endpoint::bind(socket.clone()).unwrap();
    let mut editor = Editor::new(storage::startup_example());
    editor.apply(cmd(proposal())).unwrap();
    let before = editor.project.clone();
    let revision = editor.revision;
    let (tx, rx) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let request = |v: serde_json::Value| {
            let mut stream = UnixStream::connect(&socket).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            writeln!(stream, "{v}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str::<serde_json::Value>(&line).unwrap()
        };
        let target =
            request(serde_json::json!({"method":"get_target"}))["result"]["target"].clone();
        let height = request(
            serde_json::json!({"method":"query_terrain_height","params":{"target":target,"expected_revision":revision,"xy_m":[3.,2.]}}),
        );
        tx.send(height).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        endpoint.poll(&mut editor);
        if let Ok(result) = rx.try_recv() {
            break result;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    client.join().unwrap();
    assert!(
        (result["result"]["sample"]["elevation_model_m"]
            .as_f64()
            .unwrap()
            - 2.7)
            .abs()
            < 1e-12
    );
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    drop(endpoint);
}
