use atelier_bim::{
    application_host::ApplicationHost,
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    model::{Material, Point, Project, Shape},
    plugins::{
        self, Manifest, Registry,
        jobs::{Bundle, Jobs, Terminal},
    },
    storage,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!("oa-plugin-{}", storage::new_document_identity()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn external() -> Manifest {
    Manifest::parse(include_bytes!("../assets/plugins/example-quantities.json")).unwrap()
}
fn host() -> ApplicationHost {
    let mut p = Project::default();
    p.add(
        0,
        Material::Timber,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(30., 30.),
                    Point::new(38., 30.),
                    Point::new(38., 36.),
                    Point::new(30., 36.),
                ],
                holes: vec![vec![
                    Point::new(32., 32.),
                    Point::new(34., 32.),
                    Point::new(34., 34.),
                    Point::new(32., 34.),
                ]],
                height: 6.,
                bottom_offset: 0.5,
            },
        },
    );
    ApplicationHost::new(p)
}
fn wait(h: &mut ApplicationHost, id: &str) -> Bundle {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let jobs = h.jobs.as_mut().unwrap();
        jobs.poll();
        let b = jobs
            .query(h.document.editor.project_identity(), id)
            .unwrap();
        if b.result.is_some() {
            return b;
        }
        assert!(
            Instant::now() < until,
            "original job not terminal; do not replay"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn reviewed_member_is_exact_readonly_current_and_does_not_restore_authority() {
    let mut h = host();
    let shape = h.document.editor.project.elements[0].shape.clone();
    let other = h.document.editor.project.add(0, Material::Timber, shape);
    let id = h.document.editor.project.elements[0].id;
    let layer = h
        .document
        .editor
        .project
        .layers
        .layers
        .iter()
        .find(|l| l.id != 0 && l.status.visible && !l.status.locked)
        .unwrap()
        .id;
    h.document.editor.project.elements[0].layer = atelier_bim::layers::ElementLayer(layer);
    let r = root();
    h.configure_plugins(&r).unwrap();
    let request = storage::new_document_identity();
    h.start_plugin_job("org.openarchi.quantities.report", &request)
        .unwrap();
    let b = wait(&mut h, &request);
    let before = h.document.editor.project.clone();
    let context = h.context();
    assert_eq!(b.reviewed_member(&h.document.editor, id).unwrap().id, id);
    assert_eq!(h.document.editor.project, before);
    assert!(h.document.editor.selection_ids().is_empty());
    assert_eq!(h.context(), context);
    assert!(b.reviewed_member(&h.document.editor, 999999).is_err());
    let mut pending = b.clone();
    pending.result = None;
    assert!(pending.reviewed_member(&h.document.editor, id).is_err());
    let mut tampered = b.clone();
    if let Terminal::Completed { report } = &mut tampered.result.as_mut().unwrap().terminal {
        report.concept_masses.element_ids.push(999999);
    }
    assert!(tampered.reviewed_member(&h.document.editor, id).is_err());

    // Layer visibility, groups and worksheet are outside the quantity fingerprint.
    // Their current UI meaning must still be checked before changing selection.
    let e = &mut h.document.editor;
    e.project.layers.layer_mut(layer).unwrap().status.visible = false;
    assert!(b.current(e));
    assert!(b.reviewed_member(e, id).is_err());
    e.project.layers.layer_mut(layer).unwrap().status.visible = true;
    e.project.layers.layer_mut(layer).unwrap().status.locked = true;
    assert_eq!(b.reviewed_member(e, id).unwrap().id, id);
    assert!(e.project.layers.is_locked(layer)); // Inspection never unlocks.
    e.project.layers.layer_mut(layer).unwrap().status.locked = false;
    e.project.edit_state.groups.push(vec![id, other]);
    assert!(b.current(e));
    assert!(
        b.reviewed_member(e, id)
            .unwrap_err()
            .contains("Suspend groups")
    );
    e.project.edit_state.suspend_groups = true;
    assert_eq!(b.reviewed_member(e, id).unwrap().id, id);
    e.project.plan_options.active_worksheet = Some(999);
    assert!(b.current(e));
    assert!(b.reviewed_member(e, id).is_err());
    e.project.plan_options.active_worksheet = None;
    let option = e.project.design_options.create("Other candidate".into());
    e.project.plan_options.active_design_option = Some(option);
    assert!(b.reviewed_member(e, id).unwrap_err().contains("stale"));
    e.project.plan_options.active_design_option = None;
    if let Shape::ConceptMass { spec } = &mut e.project.elements[0].shape {
        spec.height += 1.;
    }
    assert!(b.reviewed_member(e, id).unwrap_err().contains("stale"));
    let other_project = atelier_bim::editor::Editor::new(before);
    assert!(
        b.reviewed_member(&other_project, id)
            .unwrap_err()
            .contains("stale")
    );
    assert_eq!(
        h.jobs
            .as_ref()
            .unwrap()
            .query(h.document.editor.project_identity(), &request)
            .unwrap(),
        b
    );
}

#[test]
fn derived_report_roundoff_is_distinct_from_changed_facts_and_changed_quantities() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r).unwrap();
    let id = storage::new_document_identity();
    h.start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let original = wait(&mut h, &id);
    let check = |edit: fn(&mut plugins::jobs::Report)| {
        let mut b = original.clone();
        let Terminal::Completed { report } = &mut b.result.as_mut().unwrap().terminal else {
            panic!("known report")
        };
        edit(report);
        let path = r.join(format!("{}.json", storage::new_document_identity()));
        b.export(&path).map(|()| path)
    };
    // Neighboring representable values arise from independent arithmetic / JSON
    // round trips. This is not a geometric or engineering error allowance.
    let accepted = check(|r| {
        r.concept_masses.volume_m3 = f64::from_bits(r.concept_masses.volume_m3.to_bits() + 1)
    })
    .unwrap();
    let exported = Bundle::read_export(&accepted).unwrap();
    let Terminal::Completed { report } = &exported.result.as_ref().unwrap().terminal else {
        panic!("known report")
    };
    assert_eq!(
        report.concept_masses.volume_m3.to_bits(),
        264f64.to_bits() + 1
    );
    let original_path = r
        .join("plugin-jobs")
        .join(h.document.editor.project_identity())
        .join(&id)
        .join("result.json");
    let exact_original = std::fs::read(&original_path).unwrap();
    assert!(
        h.jobs
            .as_ref()
            .unwrap()
            .import_record(h.document.editor.project_identity(), &accepted)
            .is_err(),
        "arithmetic equivalence cannot replace original evidence"
    );
    assert_eq!(std::fs::read(original_path).unwrap(), exact_original);
    assert!(check(|r|r.concept_masses.area_m2=f64::from_bits(r.concept_masses.area_m2.to_bits()-1)).is_ok());
    assert!(
        check(|r| r.concept_masses.volume_m3 =
            f64::from_bits(r.concept_masses.volume_m3.to_bits() + 9))
        .is_err()
    );
    assert!(check(|r| r.concept_masses.volume_m3 += 1e-8).is_err());
    assert!(check(|r| r.concept_masses.element_ids.clear()).is_err());
    assert!(check(|r| r.definition.push_str(" approved")).is_err());
    assert!(check(|r| r.zones.volume_m3 = f64::from_bits(1)).is_err());
    assert!(check(|r| r.concept_masses.volume_m3 = f64::NAN).is_err());
}

#[test]
fn portable_original_record_restores_through_host_without_provider_or_dispatch() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("source-profile")).unwrap();
    h.plugins.enable(external()).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("example.quantities.review", &id)
        .unwrap();
    let original = wait(&mut h, &id);
    let source = r.join("original.job.json");
    original.export(&source).unwrap();
    let source_bytes = std::fs::read(&source).unwrap();
    let file = r.join("project.json");
    storage::save_document(&h.document.editor.project, &file, &identity.project_id).unwrap();
    let mut restored = host();
    restored.document.editor = atelier_bim::editor::Editor::from_stored_document(&file).unwrap();
    restored
        .configure_plugins(&r.join("fresh-profile"))
        .unwrap();
    let before = restored.document.editor.project.clone();
    let context = restored.context();
    assert_eq!(
        restored
            .import_plugin_job(source.clone())
            .unwrap()
            .join()
            .unwrap()
            .unwrap(),
        identity
    );
    assert!(
        restored
            .plugins
            .resolve("example.quantities.review")
            .is_err()
    );
    assert_eq!(restored.plugins.entries().len(), 2);
    let jobs = restored.jobs.as_ref().unwrap();
    assert_eq!(jobs.ids(&identity.project_id).unwrap(), vec![id.clone()]);
    assert_eq!(jobs.query(&identity.project_id, &id).unwrap(), original);
    assert!(
        jobs.query(&identity.project_id, &id)
            .unwrap()
            .current(&restored.document.editor)
    );
    assert_eq!(jobs.progress(&identity.project_id, &id), None);
    assert!(jobs.process_receipt(&identity).unwrap().is_none());
    assert_eq!(
        restored
            .import_plugin_job(source.clone())
            .unwrap()
            .join()
            .unwrap()
            .unwrap(),
        identity
    );
    assert_eq!(restored.context(), context);
    assert_eq!(restored.document.editor.project, before);
    assert!(restored.document.editor.undo.is_empty());
    assert_eq!(std::fs::read(source).unwrap(), source_bytes);
    let reopened = Jobs::new(r.join("fresh-profile/plugin-jobs"));
    assert_eq!(reopened.query(&identity.project_id, &id).unwrap(), original);
}

#[test]
fn native_project_job_history_roundtrip_relocation_missing_provider_and_undo() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("source-profile")).unwrap();
    h.plugins.enable(external()).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("example.quantities.review", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let original = h.document.editor.project.clone();
    let context = h.context();
    let snapshot = std::thread::spawn({
        let plan = h.project_snapshot_for_save();
        move || plan.collect()
    })
    .join()
    .unwrap()
    .unwrap();
    assert_eq!(snapshot.job_archive.records().len(), 1);
    assert_eq!(h.document.editor.project, original);
    assert_eq!(h.context(), context);
    let folder = r.join("saved");
    std::fs::create_dir(&folder).unwrap();
    let file = folder.join("project.json");
    storage::save_document(&snapshot, &file, &identity.project_id).unwrap();
    let raw = std::fs::read(&file).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(json["schema_version"], 15);
    let moved = r.join("relocated");
    std::fs::rename(folder, &moved).unwrap();
    let file = moved.join("project.json");
    let mut restored = host();
    restored.document.editor = atelier_bim::editor::Editor::from_stored_document(&file).unwrap();
    restored
        .configure_plugins(&r.join("fresh-profile"))
        .unwrap();
    assert_eq!(restored.plugins.entries().len(), 2);
    assert!(
        restored
            .plugins
            .resolve("example.quantities.review")
            .is_err()
    );
    let e = &mut restored.document.editor;
    let jobs = restored.jobs.as_mut().unwrap();
    assert_eq!(
        jobs.ids_with_archive(e.project_identity(), &e.project.job_archive)
            .unwrap(),
        vec![id.clone()]
    );
    assert_eq!(
        jobs.bundle_with_archive(e.project_identity(), &id, &e.project.job_archive)
            .unwrap(),
        b
    );
    assert!(jobs.progress(e.project_identity(), &id).is_none());
    assert_eq!(
        jobs.start(&h.plugins, e, "example.quantities.review", &id)
            .unwrap(),
        identity
    );
    assert!(
        !r.join("fresh-profile/plugin-jobs")
            .join(&identity.project_id)
            .join(&id)
            .exists()
    );
    let element = e.project.elements[0].id;
    e.apply(ModelCommand::MoveElements {
        ids: vec![element],
        offset: Point::new(0.25, 0.),
    })
    .unwrap();
    assert!(!b.current(e));
    e.undo();
    assert!(b.current(e));
    assert_eq!(e.project.elements, original.elements);
    assert_eq!(std::fs::read(file).unwrap(), raw);
    let no_store = Jobs::new(r.join("absent"));
    assert_eq!(
        no_store
            .capture_archive(e.project_identity(), &e.project.job_archive)
            .unwrap(),
        e.project.job_archive
    );
}

#[test]
fn saved_job_archive_identity_version_bad_data_and_failed_save_preserve_original() {
    use atelier_bim::model::ModelAdapter;
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("profile")).unwrap();
    let id = storage::new_document_identity();
    h.start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let p = h.project_snapshot_for_save().collect().unwrap();
    let pid = h.document.editor.project_identity();
    let file = r.join("saved.json");
    storage::save_document(&p, &file, pid).unwrap();
    let original = std::fs::read(&file).unwrap();
    let base = storage::FileVersion::read(&file).unwrap();
    for field in ["schema_version", "_openarchi_document_id", "result"] {
        let mut raw: serde_json::Value = serde_json::from_slice(&original).unwrap();
        match field {
            "schema_version" => raw[field] = 14.into(),
            "_openarchi_document_id" => raw[field] = storage::new_document_identity().into(),
            _ => {
                raw["job_archive"]["records"][0]["bundle"]["result"]["terminal"]["report"]["concept_masses"]
                    ["area_m2"] = 999.into()
            }
        };
        assert!(
            storage::JsonAdapter
                .import(&serde_json::to_vec(&raw).unwrap())
                .is_err()
        );
    }
    assert!(storage::save_document(&p, &file, &storage::new_document_identity()).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), original);
    let mut oversized = p.clone();
    oversized.name = "x".repeat(storage::MAX_PROJECT_BYTES as usize);
    assert!(
        storage::save_document_if_unchanged(&oversized, &file, &base, pid)
            .unwrap_err()
            .contains("32 MB")
    );
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert_eq!(p.job_archive.records()[0].bundle, b);
    let mut records = p.job_archive.records().to_vec();
    records.push(records[0].clone());
    assert!(plugins::archive::Archive::new(records).is_err());
    let operation = r.join("profile/plugin-jobs").join(pid).join(&id);
    std::fs::write(operation.join("result.json"), b"bad local original").unwrap();
    assert!(h.project_snapshot_for_save().collect().is_err());
    assert_eq!(
        std::fs::read(operation.join("result.json")).unwrap(),
        b"bad local original"
    );
    assert_eq!(std::fs::read(file).unwrap(), original);
}

#[test]
fn archived_unknown_is_not_a_running_owner_and_complete_evidence_cannot_be_replaced() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("source")).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let mut pending = b.clone();
    pending.result = None;
    let pending_archive = plugins::archive::Archive::new(vec![plugins::archive::Record {
        bundle: pending.clone(),
        process: None,
    }])
    .unwrap();
    let no_store = Jobs::new(r.join("absent"));
    assert_eq!(
        no_store
            .bundle_with_archive(&identity.project_id, &id, &pending_archive)
            .unwrap(),
        pending
    );
    h.document.editor.project.job_archive = pending_archive.clone();
    assert_eq!(
        h.project_snapshot_for_save()
            .collect()
            .unwrap()
            .job_archive
            .records()[0]
            .bundle,
        b
    );
    let mut fresh = Jobs::new(r.join("fresh"));
    let other = storage::new_document_identity();
    assert!(
        fresh
            .start(
                &Registry::default(),
                &h.document.editor,
                "org.openarchi.quantities.report",
                &other
            )
            .unwrap_err()
            .contains("Unresolved")
    );
    assert!(fresh.progress(&identity.project_id, &id).is_none());
    let complete = h.project_snapshot_for_save().collect().unwrap().job_archive;
    let mut false_terminal = b.clone();
    false_terminal.result.as_mut().unwrap().terminal = Terminal::Failed {
        message: "different valid but conflicting terminal".into(),
    };
    let record = r.join("conflict.job.json");
    false_terminal.export(&record).unwrap();
    fresh.import_record(&identity.project_id, &record).unwrap();
    assert!(
        fresh
            .bundle_with_archive(&identity.project_id, &id, &complete)
            .unwrap_err()
            .contains("conflict")
    );
    assert_eq!(
        fresh.query(&identity.project_id, &id).unwrap(),
        false_terminal
    );
}

#[test]
fn record_import_rejects_conflict_foreign_project_unknown_original_and_busy_storage() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("source")).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let source = r.join("source.job.json");
    b.export(&source).unwrap();
    let root = r.join("destination");
    let jobs = Jobs::new(root.clone());
    assert!(
        jobs.import_record(&storage::new_document_identity(), &source)
            .unwrap_err()
            .contains("another project")
    );
    assert!(!root.exists());
    jobs.import_record(&identity.project_id, &source).unwrap();
    let mut conflict = b.clone();
    conflict.result.as_mut().unwrap().terminal = Terminal::Failed {
        message: "controlled incompatible terminal".into(),
    };
    let wrong = r.join("different.job.json");
    conflict.export(&wrong).unwrap();
    assert!(
        jobs.import_record(&identity.project_id, &wrong)
            .unwrap_err()
            .contains("different evidence")
    );
    assert_eq!(jobs.query(&identity.project_id, &id).unwrap(), b);
    let doc = root.join(&identity.project_id);
    std::fs::write(doc.join(".plugin-write.lock"), b"another-owner").unwrap();
    assert!(
        jobs.import_record(&identity.project_id, &source)
            .unwrap_err()
            .contains("busy")
    );
    assert_eq!(
        std::fs::read(doc.join(".plugin-write.lock")).unwrap(),
        b"another-owner"
    );
    let unknown_root = r.join("unknown");
    let path = unknown_root.join(&identity.project_id).join(&id);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("intent.json"), b"unknown original bytes").unwrap();
    assert!(
        Jobs::new(unknown_root)
            .import_record(&identity.project_id, &source)
            .is_err()
    );
    assert_eq!(
        std::fs::read(path.join("intent.json")).unwrap(),
        b"unknown original bytes"
    );
}

#[test]
fn imported_unknown_remains_unowned_blocks_replay_and_preserves_cancel_request() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("source")).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let mut b = wait(&mut h, &id);
    b.result = None;
    b.cancel_requested = true;
    let source = r.join("unknown.job.json");
    b.export(&source).unwrap();
    let mut jobs = Jobs::new(r.join("restored"));
    jobs.import_record(&identity.project_id, &source).unwrap();
    assert_eq!(jobs.query(&identity.project_id, &id).unwrap(), b);
    assert_eq!(b.phase(), "Cancel requested");
    assert!(jobs.progress(&identity.project_id, &id).is_none());
    let new = storage::new_document_identity();
    assert!(
        jobs.start(
            &Registry::default(),
            &h.document.editor,
            "org.openarchi.quantities.report",
            &new
        )
        .unwrap_err()
        .contains("Unresolved job")
    );
    assert_eq!(jobs.ids(&identity.project_id).unwrap(), vec![id.clone()]);
    assert_eq!(jobs.cancel(&identity.project_id, &id).unwrap(), b);
    assert!(jobs.process_receipt(&identity).unwrap().is_none());
}

#[test]
fn portable_record_read_size_limit_matches_write_limit_and_capacity_is_not_eviction() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r.join("source")).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let mut bytes = serde_json::to_vec(&b).unwrap();
    bytes.resize(plugins::MAX_BYTES as usize, b' ');
    let source = r.join("limit.job.json");
    std::fs::write(&source, &bytes).unwrap();
    assert_eq!(Bundle::read_export(&source).unwrap(), b);
    bytes.push(b' ');
    std::fs::write(&source, &bytes).unwrap();
    assert!(Bundle::read_export(&source).unwrap_err().contains("4 MiB"));
    let jobs = Jobs::new(r.join("destination"));
    for _ in 0..64 {
        let mut next = b.clone();
        let id = storage::new_document_identity();
        next.intent.identity.request_id = id.clone();
        next.result.as_mut().unwrap().identity.request_id = id;
        let file = r.join(format!("{}.json", next.intent.identity.request_id));
        next.export(&file).unwrap();
        jobs.import_record(&identity.project_id, &file).unwrap();
    }
    let original_ids = jobs.ids(&identity.project_id).unwrap();
    let file = r.join("one-more.json");
    b.export(&file).unwrap();
    assert!(
        jobs.import_record(&identity.project_id, &file)
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(jobs.ids(&identity.project_id).unwrap(), original_ids);
}

#[cfg(unix)]
#[test]
fn imported_record_rejects_fifo_and_symlink_without_opening_stream() {
    use std::os::unix::ffi::OsStrExt;
    let r = root();
    let fifo = r.join("record.fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(
        Bundle::read_export(&fifo)
            .unwrap_err()
            .contains("regular file")
    );
    let source = r.join("original.json");
    std::fs::write(&source, b"preserve").unwrap();
    let link = r.join("link.json");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    assert!(
        Bundle::read_export(&link)
            .unwrap_err()
            .contains("regular file")
    );
    assert_eq!(std::fs::read(source).unwrap(), b"preserve");
}
#[test]
fn declaration_lifecycle_conflicts_permissions_dependencies_and_corrupt_configuration() {
    let r = root();
    let path = r.join("plugins.json");
    let mut a = Registry::open(&path).unwrap();
    let mut b = Registry::open(&path).unwrap();
    a.enable(external()).unwrap();
    assert!(a.enable(external()).is_err());
    assert!(b.enable(external()).unwrap_err().contains("changed"));
    for _ in 0..3 {
        a.set_enabled("example.quantities", false).unwrap();
        assert!(a.resolve("example.quantities.review").is_err());
        a.set_enabled("example.quantities", true).unwrap();
        assert_eq!(a.entries().len(), 3)
    }
    assert_eq!(Registry::open(&path).unwrap().entries().len(), 3);
    let original = std::fs::read(&path).unwrap();
    for raw in [
        serde_json::json!({"exec":"sh"}),
        serde_json::json!({"schema_version":2}),
    ] {
        assert!(Manifest::parse(&serde_json::to_vec(&raw).unwrap()).is_err())
    }
    let mut m = external();
    m.api_version = 2;
    assert!(a.enable(m).is_err());
    let mut m = external();
    m.id = "../bad".into();
    assert!(a.enable(m).is_err());
    let mut m = external();
    m.id = "other.plugin".into();
    m.commands[0].id = "other.plugin.query".into();
    m.dependencies.push(plugins::Dependency {
        id: "missing.plugin".into(),
        version: 1,
    });
    assert!(a.enable(m).unwrap_err().contains("dependency"));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::write(&path, b"{bad").unwrap();
    assert!(Registry::open(&path).is_err());
    assert!(a.set_enabled("example.quantities", false).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"{bad");
}
#[test]
fn actual_host_external_job_holes_exact_quantities_stale_undo_reopen_and_export() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r).unwrap();
    h.plugins.enable(external()).unwrap();
    let before = h.document.editor.project.clone();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("example.quantities.review", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    assert!(b.current(&h.document.editor));
    assert_eq!(h.document.editor.project, before);
    assert!(h.document.editor.undo.is_empty());
    if let Terminal::Completed { report } = &b.result.as_ref().unwrap().terminal {
        assert_eq!(report.zones.area_m2, 0.);
        assert_eq!(report.concept_masses.area_m2, 44.);
        assert_eq!(report.concept_masses.volume_m3, 264.);
        assert_eq!(
            report.concept_masses.element_ids,
            vec![before.elements[0].id]
        );
    } else {
        panic!("not completed")
    }
    assert_eq!(
        h.start_plugin_job("example.quantities.review", &id)
            .unwrap(),
        identity
    );
    let element = before.elements[0].id;
    h.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![element],
            offset: Point::new(0.25, 0.),
        })
        .unwrap();
    assert!(!b.current(&h.document.editor));
    assert!(
        h.start_plugin_job("example.quantities.review", &id)
            .is_err()
    );
    h.document.editor.undo();
    assert!(b.current(&h.document.editor));
    h.set_plugin_enabled("example.quantities", false).unwrap();
    assert_eq!(
        h.jobs
            .as_ref()
            .unwrap()
            .query(&identity.project_id, &id)
            .unwrap(),
        b
    );
    let exported = r.join("result.job.json");
    b.export(&exported).unwrap();
    assert!(b.export(&exported).is_err());
    assert_eq!(Bundle::read_export(&exported).unwrap(), b);
    let mut bad = serde_json::to_value(&b).unwrap();
    bad["result"]["terminal"]["report"]["concept_masses"]["area_m2"] = 999.into();
    let path = r.join("tampered.job.json");
    std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(Bundle::read_export(&path).is_err());
    let doc = r.join("saved.json");
    storage::save_document(
        &h.document.editor.project,
        &doc,
        h.document.editor.project_identity(),
    )
    .unwrap();
    let mut reopened = host();
    reopened.document.editor = atelier_bim::editor::Editor::from_stored_document(&doc).unwrap();
    reopened.configure_plugins(&r).unwrap();
    assert!(
        reopened
            .plugins
            .resolve("example.quantities.review")
            .is_err()
    );
    assert!(
        reopened
            .jobs
            .as_ref()
            .unwrap()
            .query(&identity.project_id, &id)
            .unwrap()
            .current(&reopened.document.editor)
    );
    assert_eq!(reopened.document.editor.project, before);
}
#[test]
fn unresolved_intent_preserves_original_identity_blocks_new_dispatch_and_can_be_cancelled() {
    let r = root();
    let mut h = host();
    h.configure_plugins(&r).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let path = r.join("plugin-jobs").join(&identity.project_id).join(&id);
    std::fs::rename(
        path.join("result.json"),
        path.join("protected-original-result.json"),
    )
    .unwrap();
    let mut restored = Jobs::new(r.join("plugin-jobs"));
    let unknown = restored.query(&identity.project_id, &id).unwrap();
    assert_eq!(unknown.phase(), "Unknown; inspect original job");
    assert_eq!(
        restored
            .start(&h.plugins, &h.document.editor, &identity.command_id, &id)
            .unwrap(),
        identity
    );
    assert!(restored.progress(&identity.project_id, &id).is_none());
    assert!(
        restored
            .start(
                &h.plugins,
                &h.document.editor,
                &identity.command_id,
                &storage::new_document_identity()
            )
            .unwrap_err()
            .contains(&id)
    );
    assert!(!path.join("result.json").exists());
    let cancelled = restored.cancel(&identity.project_id, &id).unwrap();
    assert!(cancelled.cancel_requested && cancelled.result.is_none());
    assert_eq!(cancelled.intent, b.intent);
    assert_eq!(
        restored.cancel(&identity.project_id, &id).unwrap(),
        cancelled
    );
    assert_eq!(h.document.editor.project, host().document.editor.project);
    assert!(
        restored
            .start(
                &h.plugins,
                &h.document.editor,
                &identity.command_id,
                &storage::new_document_identity()
            )
            .unwrap_err()
            .contains(&id),
        "cancel requested is not proof of worker termination or permission to replay"
    );
}

#[test]
fn reserved_command_cycles_corrupt_result_and_symlink_refuse_without_overwriting_evidence() {
    let r = root();
    let mut reg = Registry::open(&r.join("plugins.json")).unwrap();
    let mut m = external();
    m.id = "tool".into();
    m.commands[0].id = "tool.zone".into();
    assert!(reg.enable(m).is_err());
    let mut a = external();
    a.dependencies.push(plugins::Dependency {
        id: "second.plugin".into(),
        version: 1,
    });
    let mut b = external();
    b.id = "second.plugin".into();
    b.commands[0].id = "second.plugin.query".into();
    b.dependencies.push(plugins::Dependency {
        id: a.id.clone(),
        version: 1,
    });
    let path = r.join("cyclic.json");
    let bytes=serde_json::to_vec(&serde_json::json!({"schema_version":1,"entries":[{"manifest":a,"enabled":true},{"manifest":b,"enabled":true}]})).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(Registry::open(&path).err().unwrap().contains("Cyclic"));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    let mut h = host();
    h.configure_plugins(&r).unwrap();
    let id = storage::new_document_identity();
    let identity = h
        .start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    wait(&mut h, &id);
    let p = r
        .join("plugin-jobs")
        .join(&identity.project_id)
        .join(&id)
        .join("result.json");
    std::fs::rename(&p, p.with_file_name("protected-original-result.json")).unwrap();
    std::fs::write(&p, b"{partial").unwrap();
    let jobs = h.jobs.as_mut().unwrap();
    assert!(jobs.query(&identity.project_id, &id).is_err());
    assert!(
        jobs.start(
            &h.plugins,
            &h.document.editor,
            &identity.command_id,
            &storage::new_document_identity()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&p).unwrap(), b"{partial");
    #[cfg(unix)]
    {
        let linked = r.join("linked");
        std::os::unix::fs::symlink(r.join("plugin-jobs"), &linked).unwrap();
        assert!(Jobs::new(linked).ids(&identity.project_id).is_err());
    }
}
