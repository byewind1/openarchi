use atelier_bim::{
    application_host::ApplicationHost,
    editor::Editor,
    model::ModelAdapter,
    plugins::{
        self,
        jobs::{Bundle, Terminal},
        solar::Parameters,
    },
    solar, storage,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize},
    time::{Duration, Instant},
};
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "oa-solar-jobs-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
fn parameters() -> Parameters {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/solar/concept-scenario.json")).unwrap();
    serde_json::from_value(
        serde_json::json!({"request":v["request"],"assumed_north":v["assumed_north"]}),
    )
    .unwrap()
}
fn host(r: &std::path::Path) -> ApplicationHost {
    let mut h = ApplicationHost::new(storage::startup_example());
    h.configure_plugins(r).unwrap();
    h
}
fn wait(h: &mut ApplicationHost, id: &str) -> Bundle {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let j = h.jobs.as_mut().unwrap();
        j.poll();
        let b = j.query(h.document.editor.project_identity(), id).unwrap();
        if b.result.is_some() {
            return b;
        }
        assert!(
            Instant::now() < deadline,
            "query original request; do not replay"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn shared_host_solar_job_has_bound_result_stales_without_replay_and_preserves_model() {
    let r = root();
    let mut h = host(&r);
    let before = h.document.editor.project.clone();
    let rev = h.document.editor.revision;
    let params = parameters();
    let id = storage::new_document_identity();
    let identity = h.start_solar_job(&params, &id).unwrap();
    let b = wait(&mut h, &id);
    assert_eq!(h.document.editor.project, before);
    assert_eq!(h.document.editor.revision, rev);
    assert!(h.document.editor.undo.is_empty());
    assert_eq!(b.intent.input.elements.len(), 4);
    assert_eq!(
        b.intent
            .input
            .solar
            .as_ref()
            .unwrap()
            .excluded_element_ids
            .len(),
        126
    );
    assert!(b.current(&h.document.editor));
    let Terminal::Completed { report } = &b.result.as_ref().unwrap().terminal else {
        panic!("{:?}", b.result)
    };
    let result = report.solar.as_ref().unwrap();
    assert_eq!(result.algorithm, solar::ALGORITHM);
    let point = &result.points[0];
    assert_eq!(point.sampled_sunlit_seconds, 3600);
    assert_eq!(point.uncertain_seconds, 0);
    assert_eq!(point.cells.len(), 6);
    assert!(point.cells.iter().all(|c| c.state == solar::State::Sunlit));
    assert_eq!(
        point.sunlit_segments,
        vec![solar::Segment {
            start_seconds: 0,
            end_seconds: 3600
        }]
    );
    let old_result = std::fs::read(
        r.join("plugin-jobs")
            .join(&identity.project_id)
            .join(&id)
            .join("result.json"),
    )
    .unwrap();
    assert_eq!(h.start_solar_job(&params, &id).unwrap(), identity);
    assert_eq!(
        h.jobs
            .as_ref()
            .unwrap()
            .query(&identity.project_id, &id)
            .unwrap(),
        b
    );
    let mut changed = params.clone();
    changed.request.step_seconds = 300;
    assert!(h.start_solar_job(&changed, &id).is_err());
    if let atelier_bim::model::Shape::ConceptMass { spec } = &mut h
        .document
        .editor
        .project
        .elements
        .iter_mut()
        .find(|e| e.id == 133)
        .unwrap()
        .shape
    {
        spec.height += 0.1;
    }
    assert!(!b.current(&h.document.editor));
    assert!(h.start_solar_job(&params, &id).is_err());
    assert_eq!(
        h.jobs
            .as_ref()
            .unwrap()
            .query(&identity.project_id, &id)
            .unwrap(),
        b
    );
    {
        let bytes = old_result;
        assert_eq!(
            std::fs::read(
                r.join("plugin-jobs")
                    .join(&identity.project_id)
                    .join(&id)
                    .join("result.json")
            )
            .unwrap(),
            bytes
        );
    }
}
#[test]
fn native21_archive_reopens_without_local_jobs_and_rejects_corrupted_result_or_old_marker() {
    let r = root();
    let mut h = host(&r);
    let id = storage::new_document_identity();
    let identity = h.start_solar_job(&parameters(), &id).unwrap();
    let original = wait(&mut h, &id);
    let project = h.project_snapshot_for_save().collect().unwrap();
    let path = r.join("saved.openarchi.json");
    storage::save_document(&project, &path, h.document.editor.project_identity()).unwrap();
    let raw = std::fs::read(&path).unwrap();
    let mut j: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(j["schema_version"], 21);
    j["schema_version"] = 20.into();
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&j).unwrap())
            .unwrap_err()
            .contains("version 21")
    );
    let mut reopened = ApplicationHost::new(atelier_bim::model::Project::default());
    reopened.document.editor = Editor::from_stored_document(&path).unwrap();
    let fresh = r.join("fresh");
    std::fs::create_dir(&fresh).unwrap();
    reopened.configure_plugins(&fresh).unwrap();
    reopened
        .set_plugin_enabled(plugins::solar::ID, false)
        .unwrap();
    let record = reopened
        .jobs
        .as_ref()
        .unwrap()
        .bundle_with_archive(
            &identity.project_id,
            &id,
            &reopened.document.editor.project.job_archive,
        )
        .unwrap();
    assert_eq!(record, original);
    assert!(record.current(&reopened.document.editor));
    assert!(
        reopened
            .start_solar_job(&parameters(), &storage::new_document_identity())
            .is_err()
    );
    let export = r.join("solar.job.json");
    original.export(&export).unwrap();
    let mut v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    v["result"]["terminal"]["report"]["solar"]["points"][0]["sampled_sunlit_seconds"] = 0.into();
    let bad = r.join("corrupt.job.json");
    std::fs::write(&bad, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(Bundle::read_export(&bad).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), raw);
}
#[test]
fn cancellation_keeps_original_identity_and_unknown_seconds_are_not_known_duration() {
    let r = root();
    let mut h = host(&r);
    let params = parameters();
    let input = plugins::jobs::Input::capture_solar(&h.document.editor, &params).unwrap();
    let s = input.solar.as_ref().unwrap();
    let snap = s.snapshot(&input.elements).unwrap();
    let cancelled = solar::run_with_control(
        &snap,
        &s.request,
        &AtomicBool::new(true),
        &AtomicUsize::new(0),
    );
    assert!(cancelled.unwrap_err().contains("cancelled"));
    let id = storage::new_document_identity();
    let identity = h.start_solar_job(&params, &id).unwrap();
    h.jobs
        .as_mut()
        .unwrap()
        .cancel(&identity.project_id, &id)
        .unwrap();
    let b = wait(&mut h, &id);
    assert!(b.cancel_requested);
    assert_eq!(b.intent.identity, identity);
    assert!(matches!(
        b.result.unwrap().terminal,
        Terminal::Cancelled | Terminal::Completed { .. }
    ));
    assert_eq!(h.start_solar_job(&params, &id).unwrap(), identity);
}
#[test]
fn builtin_upgrade_preserves_old_configuration_bytes_flags_dependencies_and_v1_manifest() {
    let r = root();
    let path = r.join("plugins.json");
    let old = serde_json::json!({"schema_version":1,"entries":[{"manifest":plugins::builtin(),"enabled":false,"executable":null}]});
    let bytes = serde_json::to_vec_pretty(&old).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let registry = plugins::Registry::open(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(registry.entries().len(), 2);
    assert!(!registry.entries()[0].enabled);
    assert_eq!(registry.entries()[0].manifest.version, 1);
    assert_eq!(registry.entries()[0].manifest, plugins::builtin());
    assert!(registry.resolve(plugins::solar::COMMAND).is_ok());
    assert!(registry.resolve("org.openarchi.quantities.report").is_err());
}
#[test]
fn uncertain_contact_seconds_are_explicit_and_not_a_zero_complete_sunlight_duration() {
    let params = parameters();
    let s = solar::Snapshot {
        schema_version: 1,
        algorithm: solar::ALGORITHM.into(),
        source_sha256: "a".repeat(64),
        north: params.assumed_north.unwrap(),
        blockers: vec![solar::Blocker {
            element_id: 1,
            triangles: vec![[[-100., -100., 0.], [100., -100., 0.], [0., 100., 0.]]],
        }],
        excluded_element_ids: vec![],
    };
    let mut request = params.request;
    request.receivers[0].point_m = [0., 0., 0.];
    let result = solar::run(&s, &request).unwrap();
    assert_eq!(result.points[0].sampled_sunlit_seconds, 0);
    assert_eq!(result.points[0].uncertain_seconds, 3600);
    assert!(
        result.points[0]
            .cells
            .iter()
            .all(|c| c.state == solar::State::NearSurface)
    );
    assert!(result.points[0].sunlit_segments.is_empty());
}
#[test]
fn measure_actual_bounded_job_query_validation_chain_without_changing_solver_semantics() {
    use atelier_bim::{
        concept_mass::ConceptMassSpec,
        model::{Material, Point, Project, Shape},
    };
    let mut p = Project::default();
    for n in 0..256 {
        let x = f64::from(n) * 3.;
        p.add(
            0,
            Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: vec![
                        Point::new(x, 0.),
                        Point::new(x + 2., 0.),
                        Point::new(x + 2., 2.),
                        Point::new(x, 2.),
                    ],
                    holes: vec![],
                    height: 2.,
                    bottom_offset: 0.,
                },
            },
        );
    }
    let r = root();
    let mut h = ApplicationHost::new(p);
    h.configure_plugins(&r).unwrap();
    let mut params = parameters();
    params.request.step_seconds = 60;
    params.request.end_utc.day = 18;
    params.request.end_utc.hour = 18;
    params.request.receivers.push(atelier_bim::solar::Receiver {
        id: "p2".into(),
        point_m: [0., 0., 100.],
        outward_normal: [0., 0., 1.],
    });
    let id = storage::new_document_identity();
    let start = Instant::now();
    let identity = h.start_solar_job(&params, &id).unwrap();
    let b = wait(&mut h, &id);
    let execute_and_first_query_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut queries = vec![];
    for _ in 0..3 {
        let start = Instant::now();
        assert_eq!(
            h.jobs
                .as_ref()
                .unwrap()
                .query(&identity.project_id, &id)
                .unwrap(),
            b
        );
        queries.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let profile = serde_json::json!({"scope":"256 existing concept solids, two receivers,1440 cells per receiver; full Jobs::query parse/validation/recompute; not GUI/GPU/AI","execute_and_first_query_ms":execute_and_first_query_ms,"repeated_query_ms":queries,"no_optimization_applied":true});
    println!("SOLAR_JOB_PROFILE={profile}");
}
