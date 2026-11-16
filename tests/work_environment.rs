use atelier_bim::workbench::{WorkMode, environment::*};
use std::{fs, path::PathBuf};
fn directory() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "oa-env-{}",
        atelier_bim::storage::new_document_identity()
    ));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn reports_drawings_builtin_resolves_real_commands_and_preserves_personal_layers() {
    use atelier_bim::workbench::{CommandEffect, CommandRegistry};
    let config = Configuration::new(WorkMode::Architecture);
    let builtin = config
        .resolve("openarchi.reports-drawings")
        .expect("the existing local drawing/report capabilities need a usable work mode");
    assert!(builtin.commands.iter().any(|id| id == "project.drawings"));
    assert!(
        builtin
            .commands
            .iter()
            .any(|id| id == "project.report_templates")
    );
    let editor = atelier_bim::editor::Editor::new(atelier_bim::model::Project::default());
    for profile in config.profiles() {
        for id in profile.commands {
            let command = CommandRegistry::resolve(&id)
                .expect("every shipped mode entry resolves a real capability");
            let descriptor = CommandRegistry::describe(command, &editor);
            if id == "project.drawings" {
                assert_eq!(descriptor.effect, CommandEffect::View);
            }
        }
    }
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let mut initial = service.configuration.clone();
    personal(&mut initial);
    service.save(initial).unwrap();
    let previous_personal = service.configuration.resolve("user.personal").unwrap();
    let before = fs::read(&path).unwrap();
    let mut report = builtin;
    report.id = "user.report-review".into();
    report.name = "图组审阅".into();
    report.builtin_name = false;
    report.commands.swap(0, 1);
    report.panels.toolbox = false;
    let mut staged = service.configuration.clone();
    staged.put(report.layer());
    staged.active = report.id.clone();
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "a draft never writes or activates itself"
    );
    service.save(staged).unwrap();
    let reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert_eq!(
        reopened.configuration.resolve(&report.id).unwrap().commands,
        report.commands
    );
    assert_eq!(
        reopened.configuration.resolve(&report.id).unwrap().panels,
        report.panels
    );
    assert_eq!(
        reopened.configuration.resolve("user.personal").unwrap(),
        previous_personal
    );
    let exported = root.join("review.json");
    reopened.export(&exported).unwrap();
    assert_eq!(
        Configuration::parse(&fs::read(exported).unwrap()).unwrap(),
        reopened.configuration
    );
    service.restore_previous().unwrap();
    let mut restored = Configuration::parse(&before).unwrap();
    // Restore is a new configuration write, not a rewind of its CAS revision.
    restored.revision += 2;
    assert_eq!(
        Configuration::parse(&fs::read(&path).unwrap()).unwrap(),
        restored
    );
    fs::remove_dir_all(root).unwrap();
}
fn personal(config: &mut Configuration) {
    let mut profile = Profile::builtin(WorkMode::SiteConcept);
    profile.id = "user.personal".into();
    profile.name = "个人前期模式".into();
    profile.builtin_name = false;
    profile.commands = vec![
        "tool.zone".into(),
        "project.program".into(),
        "view.model".into(),
    ];
    profile.panels.navigator = false;
    config.put(profile.layer());
    config.active = profile.id;
}
#[test]
fn named_layer_roundtrip_export_import_and_previous_recovery() {
    let root = directory();
    let path = root.join("environment.json");
    let mut store = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert!(!path.exists());
    let mut draft = store.configuration.clone();
    personal(&mut draft);
    let applied = store.save(draft).unwrap();
    assert_eq!(applied.revision, 1);
    assert!(applied.warning.is_none());
    let p = store
        .configuration
        .resolve(&store.configuration.active)
        .unwrap();
    assert_eq!(p.commands, ["tool.zone", "project.program", "view.model"]);
    assert!(!p.panels.navigator);
    let mut second = store.configuration.clone();
    second.profiles[0].commands = Some(vec!["project.sources".into()]);
    store.save(second).unwrap();
    assert_eq!(store.configuration.revision, 2);
    store.restore_previous().unwrap();
    assert_eq!(
        store
            .configuration
            .resolve("user.personal")
            .unwrap()
            .commands,
        p.commands
    );
    let moved = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert_eq!(moved.configuration, store.configuration);
    let export = root.join("personal.json");
    moved.export(&export).unwrap();
    assert!(moved.export(&export).is_err());
    let mut other =
        EnvironmentService::open(&root.join("other.json"), WorkMode::Architecture).unwrap();
    other.import(&export).unwrap();
    assert_eq!(other.configuration.resolve("user.personal").unwrap(), p);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn stale_writers_newer_and_corrupt_files_preserve_primary_until_explicit_recovery() {
    let root = directory();
    let path = root.join("environment.json");
    let mut a = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let mut b = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let mut cfg = a.configuration.clone();
    personal(&mut cfg);
    a.save(cfg).unwrap();
    let winner = fs::read(&path).unwrap();
    assert!(
        b.save(b.configuration.clone())
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(fs::read(&path).unwrap(), winner);
    for invalid in [
        b"broken".as_slice(),
        br#"{"schema_version":999,"future":"keep"}"#.as_slice(),
    ] {
        fs::write(&path, invalid).unwrap();
        let mut damaged = EnvironmentService::open(&path, WorkMode::SiteConcept).unwrap();
        assert!(damaged.recovery.is_some());
        assert_eq!(damaged.configuration.active, "user.personal");
        assert!(damaged.save(damaged.configuration.clone()).is_err());
        assert_eq!(fs::read(&path).unwrap(), invalid);
        let result = damaged.restore_valid().unwrap();
        assert_eq!(fs::read(result.preserved_file.unwrap()).unwrap(), invalid);
        assert!(damaged.recovery.is_none());
        assert_eq!(
            damaged
                .configuration
                .resolve("user.personal")
                .unwrap()
                .commands,
            ["tool.zone", "project.program", "view.model"]
        );
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn default_upgrades_merge_only_unmodified_fields_and_keep_unknown_command_ids() {
    let mut cfg = Configuration::new(WorkMode::Architecture);
    cfg.put(ProfileLayer {
        id: "user.inherited".into(),
        base_mode: WorkMode::Architecture,
        name: Some("Inherited".into()),
        commands: None,
        project_menu: None,
        panels: None,
        workspace: None,
        shortcuts: None,
        creation_presets: None,
    });
    let mut custom = Profile::builtin(WorkMode::Architecture);
    custom.id = "user.custom".into();
    custom.name = "Custom".into();
    custom.builtin_name = false;
    custom.commands = vec!["tool.zone".into(), "missing.plugin:action".into()];
    custom.panels.toolbox = false;
    cfg.put(custom.layer());
    cfg.validate().unwrap();
    let mut upgraded = defaults();
    upgraded[0].commands.push("query.spaces".into());
    upgraded[0].panels.info_box = false;
    let inherited = cfg
        .resolve_with_defaults("user.inherited", &upgraded)
        .unwrap();
    assert!(inherited.commands.contains(&"query.spaces".into()));
    assert!(!inherited.panels.info_box);
    let retained = cfg.resolve_with_defaults("user.custom", &upgraded).unwrap();
    assert_eq!(retained.commands, custom.commands);
    assert!(!retained.panels.toolbox);
    assert!(retained.command("missing.plugin:action").is_none());
}
#[test]
fn configuration_edits_have_no_document_or_history_dependency() {
    use atelier_bim::{
        application_host::{ApplicationHost, HostCommand},
        command::ModelCommand,
        model::Point,
        storage,
    };
    let root = directory();
    let mut host = ApplicationHost::new(storage::startup_example());
    let id = host.document.editor.project.elements[0].id;
    host.dispatch(
        host.context(),
        HostCommand::Apply {
            request_id: "existing-edit".into(),
            commands: vec![ModelCommand::MoveElements {
                ids: vec![id],
                offset: Point::new(0.25, 0.),
            }],
        },
    )
    .unwrap();
    host.document.editor.set_selection(vec![id]);
    let project = host.document.editor.project.clone();
    let state = (
        host.context(),
        host.document.editor.undo.len(),
        host.document.editor.dirty(),
        host.document.editor.selection_ids(),
    );
    let mut store =
        EnvironmentService::open(&root.join("env.json"), WorkMode::Architecture).unwrap();
    let mut cfg = store.configuration.clone();
    personal(&mut cfg);
    store.save(cfg).unwrap();
    store.export(&root.join("export.json")).unwrap();
    assert_eq!(host.document.editor.project, project);
    assert_eq!(
        state,
        (
            host.context(),
            host.document.editor.undo.len(),
            host.document.editor.dirty(),
            host.document.editor.selection_ids()
        )
    );
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn lock_conflicts_and_symlink_destination_never_replace_user_files() {
    use std::os::unix::fs::symlink;
    let root = directory();
    let path = root.join("environment.json");
    let mut store = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("environment.json.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(
        store
            .save(store.configuration.clone())
            .unwrap_err()
            .contains("another process")
    );
    assert!(!path.exists());
    drop(lock);
    let target = root.join("private.json");
    fs::write(&target, "preserve").unwrap();
    symlink(&target, &path).unwrap();
    assert!(store.save(store.configuration.clone()).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "preserve");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn first_corrupt_configuration_can_recover_defaults_without_discarding_original() {
    let root = directory();
    let path = root.join("environment.json");
    fs::write(&path, b"first invalid configuration").unwrap();
    let mut store = EnvironmentService::open(&path, WorkMode::SiteConcept).unwrap();
    assert!(store.restore_valid().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"first invalid configuration");
    let outcome = store.restore_defaults(WorkMode::SiteConcept).unwrap();
    assert_eq!(
        fs::read(outcome.preserved_file.unwrap()).unwrap(),
        b"first invalid configuration"
    );
    assert!(store.recovery.is_none());
    assert_eq!(
        EnvironmentService::open(&path, WorkMode::Architecture)
            .unwrap()
            .configuration
            .active,
        "openarchi.site-concept"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_workspace_groups_unknown_slots_and_invalid_edits_are_recoverable() {
    use atelier_bim::workbench::workspace::*;
    let mut w = NativeWorkspace::default();
    w.group_with(TOOLBOX, NAVIGATOR).unwrap();
    assert_eq!(w.groups.len(), 1);
    assert_eq!(w.groups[0].active, TOOLBOX);
    w.groups[0].panels.push("extension.missing.panel".into());
    w.validate().unwrap();
    w.ungroup(TOOLBOX, "workspace.new").unwrap();
    assert_eq!(w.groups.len(), 2);
    assert_eq!(w.groups[1].placement, Placement::Floating);
    let before = w.clone();
    assert!(w.group_with(TOOLBOX, "absent").is_err());
    assert_eq!(w, before);
    assert!(w.ungroup(NAVIGATOR, "workspace.new").is_err());
    assert_eq!(w, before);
    let mut invalid = w.clone();
    invalid.groups[0].panels.push(TOOLBOX.into());
    assert!(invalid.validate().is_err());
    let mut invalid = w.clone();
    invalid.groups[0].floating.x = i32::MIN;
    assert!(invalid.validate().is_err());
    let mut invalid = w;
    invalid.groups[0].floating.width = 0;
    assert!(invalid.validate().is_err());
}
#[test]
fn native_workspace_recovery_uses_logical_viewport_and_handles_negative_origins() {
    use atelier_bim::workbench::workspace::*;
    for viewport in [
        [0, 0, 800, 600],
        [-1200, -100, 1200, 800],
        [10, 30, 320, 240],
        [0, 0, 80, 100],
    ] {
        let rect = FloatRect {
            x: 12000,
            y: -12000,
            width: 4096,
            height: 4096,
        }
        .recovered(viewport);
        let [x, y, w, h] = viewport;
        assert!(rect.x >= x && rect.y >= y);
        assert!(rect.x + rect.width as i32 <= x + w);
        assert!(rect.y + rect.height as i32 <= y + h);
        assert_eq!(rect.recovered(viewport), rect);
    }
    let mut w = NativeWorkspace::default();
    w.groups[0].placement = Placement::Floating;
    w.groups[0].floating.x = 4000;
    assert!(w.recover([0, 0, 800, 600]));
    let recovered = w.clone();
    assert!(!w.recover([0, 0, 800, 600]));
    assert_eq!(w, recovered);
}
#[test]
fn workspace_v1_migration_and_personal_layout_roundtrip_preserve_previous_file() {
    use atelier_bim::workbench::workspace::*;
    let root = directory();
    let path = root.join("environment.json");
    let legacy =
        br#"{"schema_version":1,"revision":5,"active":"openarchi.architecture","profiles":[]}"#;
    fs::write(&path, legacy).unwrap();
    let mut service = EnvironmentService::open(&path, WorkMode::SiteConcept).unwrap();
    assert_eq!(service.configuration.schema_version, 2);
    assert_eq!(fs::read(&path).unwrap(), legacy);
    let mut config = service.configuration.clone();
    personal(&mut config);
    let mut p = config.resolve("user.personal").unwrap();
    p.workspace.group_with(TOOLBOX, NAVIGATOR).unwrap();
    p.workspace.groups[0].placement = Placement::Floating;
    config.put(p.layer());
    service.save(config).unwrap();
    assert_eq!(
        fs::read(path.with_file_name("environment.json.previous.json")).unwrap(),
        legacy
    );
    assert_eq!(
        EnvironmentService::open(&path, WorkMode::Architecture)
            .unwrap()
            .configuration
            .resolve("user.personal")
            .unwrap()
            .workspace,
        p.workspace
    );
    service.restore_previous().unwrap();
    assert_eq!(service.configuration.active, "openarchi.architecture");
    assert_eq!(
        service
            .configuration
            .resolve("openarchi.architecture")
            .unwrap()
            .workspace,
        NativeWorkspace::default()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn imported_review_keeps_snapshot_and_file_authority_until_explicit_save() {
    let root = directory();
    let path = root.join("environment.json");
    let proposal = root.join("proposal.json");
    let mut store = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    store.save(store.configuration.clone()).unwrap();
    let original = fs::read(&path).unwrap();
    let mut external = Configuration::new(WorkMode::SiteConcept);
    personal(&mut external);
    external.profiles[0].workspace = Some(Default::default());
    external.revision = 999;
    fs::write(&proposal, serde_json::to_vec(&external).unwrap()).unwrap();
    let mut draft = store.preview_import(&proposal).unwrap();
    let mut profile = draft.resolve(&draft.active).unwrap();
    draft.apply_profile_draft(&profile);
    assert!(draft.profiles[0].workspace.is_some());
    profile.commands.push("view.plan".into());
    draft.apply_profile_draft(&profile);
    assert!(
        draft.profiles[0].workspace.is_some(),
        "editing another field must preserve explicit layout defaults"
    );
    profile.commands.pop();
    draft.apply_profile_draft(&profile);
    assert_eq!(draft.revision, store.configuration.revision);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(store.configuration.active, "openarchi.architecture");
    let difference = draft.differences_from(&store.configuration).unwrap();
    assert_eq!(difference.active_after, "user.personal");
    assert_eq!(difference.profiles.len(), 1);
    assert!(difference.profiles[0].before.is_none());
    assert_eq!(
        difference.profiles[0].after.as_ref().unwrap().commands,
        ["tool.zone", "project.program", "view.model"]
    );
    // Replacing the incoming file is not permission to silently re-read it at Save.
    fs::write(&proposal, b"changed after preview").unwrap();
    assert!(store.preview_import(&proposal).is_err());
    store.save(draft.clone()).unwrap();
    let reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert_eq!(
        reopened
            .configuration
            .resolve("user.personal")
            .unwrap()
            .name,
        "个人前期模式"
    );
    assert!(store.save(draft).unwrap_err().contains("revision changed"));
    // A new proposal cannot reset the baseline of a concurrently edited saved file.
    fs::write(&proposal, serde_json::to_vec(&external).unwrap()).unwrap();
    let mut other = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let mut changed = other.configuration.clone();
    changed.profiles[0].name = Some("User intervened".into());
    other.save(changed).unwrap();
    let winner = fs::read(&path).unwrap();
    let stale = store.preview_import(&proposal).unwrap();
    assert!(store.save(stale).unwrap_err().contains("file changed"));
    assert_eq!(fs::read(&path).unwrap(), winner);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configuration_difference_includes_inactive_removals_unknown_ids_and_inheritance() {
    let mut before = Configuration::new(WorkMode::Architecture);
    personal(&mut before);
    let mut after = before.clone();
    after.active = "openarchi.architecture".into();
    after.profiles.clear();
    let mut p = Profile::builtin(WorkMode::Architecture);
    p.commands.push("missing.plugin:task".into());
    after.put(p.layer());
    let diff = after.differences_from(&before).unwrap();
    assert_eq!(
        diff.profiles
            .iter()
            .map(|d| d.id.as_str())
            .collect::<Vec<_>>(),
        ["openarchi.architecture", "user.personal"]
    );
    assert!(diff.profiles[1].after.is_none());
    assert!(
        diff.profiles[0]
            .after
            .as_ref()
            .unwrap()
            .commands
            .contains(&"missing.plugin:task".into())
    );
    // Equal resolved values must still disclose that future default inheritance changed.
    let mut layer = before.profiles[0].clone();
    layer.workspace = Some(Default::default());
    let mut inherited_change = before.clone();
    inherited_change.put(layer);
    let diff = inherited_change.differences_from(&before).unwrap();
    assert_eq!(diff.profiles.len(), 1);
    assert_eq!(diff.profiles[0].before, diff.profiles[0].after);
    assert_ne!(diff.profiles[0].before_layer, diff.profiles[0].after_layer);
    assert!(
        before
            .differences_from(&before)
            .unwrap()
            .profiles
            .is_empty()
    );
    let original = Configuration::new(WorkMode::Architecture);
    let mut empty_layer = original.clone();
    empty_layer.put(Profile::builtin(WorkMode::Architecture).layer());
    assert!(
        empty_layer
            .differences_from(&original)
            .unwrap()
            .profiles
            .is_empty()
    );
    assert_eq!(
        empty_layer.profiles.len(),
        1,
        "view comparison must not rewrite data"
    );
}

#[test]
fn compact_valid_but_pretty_oversize_configuration_rejects_before_touching_saved_history() {
    use std::collections::BTreeMap;
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    service.save(service.configuration.clone()).unwrap();
    service.save(service.configuration.clone()).unwrap();
    let files = |root: &std::path::Path| -> BTreeMap<_, _> {
        fs::read_dir(root)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (e.file_name(), fs::read(e.path()).unwrap())
            })
            .collect()
    };
    let before = files(&root);
    let mut draft = service.configuration.clone();
    for n in 0..32 {
        let mut p = Profile::builtin(WorkMode::Architecture);
        p.id = format!("user.large-{n}");
        p.name = format!("Large fixture {n}");
        p.builtin_name = false;
        p.commands = (0..128)
            .map(|c| {
                let prefix = format!("unavailable.{n}.{c}.");
                format!("{prefix}{}", "x".repeat(248 - prefix.len()))
            })
            .collect();
        draft.put(p.layer());
    }
    draft.validate().unwrap();
    let compact = serde_json::to_vec(&draft).unwrap();
    assert!(compact.len() <= 1024 * 1024);
    Configuration::parse(&compact).unwrap();
    assert!(serde_json::to_vec_pretty(&draft).unwrap().len() > 1024 * 1024);
    assert!(service.save(draft).unwrap_err().contains("exceeds1 MiB"));
    assert_eq!(
        files(&root),
        before,
        "known size rejection must not replace previous settings or create recovery archives"
    );
    assert_eq!(service.configuration.revision, 2);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn personal_shortcuts_effective_conflicts_unbind_and_future_ids() {
    use atelier_bim::workbench::shortcuts::{self, Chord};
    let mut bindings = vec![];
    let f2 = Chord::new("F2", false, false, false);
    assert!(
        shortcuts::assign(&mut bindings, "tool.zone", Some(vec![f2.clone()]))
            .unwrap_err()
            .contains("view.plan")
    );
    assert!(bindings.is_empty());
    shortcuts::assign(&mut bindings, "view.plan", Some(vec![])).unwrap();
    shortcuts::assign(&mut bindings, "tool.zone", Some(vec![f2])).unwrap();
    let before = bindings.clone();
    assert!(shortcuts::assign(&mut bindings, "view.plan", None).is_err());
    assert_eq!(bindings, before);
    for (key, p, a, s) in [
        ("C", true, false, false),
        ("T", true, true, false),
        ("S", false, true, false),
        ("M", false, false, true),
        ("Q", true, false, false),
        ("Q", true, false, true),
        ("Q", true, true, true),
        ("W", true, true, false),
        ("3", true, false, true),
        ("F1", true, true, false),
        ("H", true, true, false),
        ("Escape", false, false, false),
    ] {
        assert!(
            shortcuts::assign(
                &mut bindings,
                "tool.zone",
                Some(vec![Chord::new(key, p, a, s)])
            )
            .is_err(),
            "{key}"
        );
        assert_eq!(bindings, before);
    }
    shortcuts::assign(
        &mut bindings,
        "missing.plugin:action",
        Some(vec![Chord::new("J", true, false, true)]),
    )
    .unwrap();
    assert!(
        shortcuts::assign(
            &mut bindings,
            "tool.wall",
            Some(vec![Chord::new("J", true, false, true)])
        )
        .is_err()
    );
    shortcuts::assign(&mut bindings, "model.undo", Some(vec![])).unwrap();
    shortcuts::assign(
        &mut bindings,
        "tool.wall",
        Some(vec![Chord::new("Z", true, false, false)]),
    )
    .unwrap();
    shortcuts::validate(&bindings).unwrap();
}

#[test]
fn personal_shortcuts_native_version_roundtrip_export_previous_and_cas() {
    use atelier_bim::workbench::shortcuts::{self, Chord};
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    service.save(service.configuration.clone()).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap()["schema_version"],
        2
    );
    let mut draft = service.configuration.clone();
    let mut profile = draft.resolve(&draft.active).unwrap();
    shortcuts::assign(
        &mut profile.shortcuts,
        "tool.zone",
        Some(vec![Chord::new("J", true, false, true)]),
    )
    .unwrap();
    shortcuts::assign(&mut profile.shortcuts, "model.undo", Some(vec![])).unwrap();
    draft.apply_profile_draft(&profile);
    service.save(draft.clone()).unwrap();
    let bytes = fs::read(&path).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 3);
    assert_eq!(Configuration::parse(&bytes).unwrap(), service.configuration);
    value["schema_version"] = serde_json::json!(2);
    assert!(Configuration::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    let export = root.join("export.json");
    service.export(&export).unwrap();
    assert_eq!(fs::read(export).unwrap(), bytes);
    let mut reopened = EnvironmentService::open(&path, WorkMode::SiteConcept).unwrap();
    let imported = reopened.configuration.resolve(&profile.id).unwrap();
    assert_eq!(imported.shortcuts, profile.shortcuts);
    // Editing panels preserves explicit shortcut overrides and leaves the original draft stale.
    let mut next = reopened.configuration.clone();
    let mut changed = imported;
    changed.panels.toolbox = false;
    next.apply_profile_draft(&changed);
    reopened.save(next).unwrap();
    assert!(
        service
            .save(service.configuration.clone())
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(
        reopened
            .configuration
            .resolve(&profile.id)
            .unwrap()
            .shortcuts,
        profile.shortcuts
    );
    reopened.restore_previous().unwrap();
    assert_eq!(
        reopened
            .configuration
            .resolve(&profile.id)
            .unwrap()
            .shortcuts,
        profile.shortcuts
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn personal_shortcuts_original_proposal_identity_survives_native_v3_reopen_and_restore() {
    use atelier_bim::workbench::{
        environment::proposal::Request,
        shortcuts::{self, Chord},
    };
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    service.save(service.configuration.clone()).unwrap();
    let original = service.configuration.clone();
    let inspection = service.inspect().unwrap();
    let mut configuration = inspection.configuration;
    let mut profile = configuration.resolve(&configuration.active).unwrap();
    shortcuts::assign(
        &mut profile.shortcuts,
        "tool.zone",
        Some(vec![Chord::new("J", true, false, true)]),
    )
    .unwrap();
    configuration.apply_profile_draft(&profile);
    let request = Request {
        schema_version: 1,
        request_id: "shortcut-owned-original".into(),
        project_id: "a".repeat(64),
        expected_snapshot: inspection.snapshot,
        configuration,
    };
    service.propose(request.clone()).unwrap();
    let draft = service
        .proposal_draft(&request.request_id, &request.project_id)
        .unwrap();
    service
        .confirm_proposal(&request.request_id, &request.project_id, draft)
        .unwrap();
    let committed = fs::read(&path).unwrap();
    let mut reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let view = reopened
        .read_proposal(&request.request_id, &request.project_id)
        .unwrap();
    assert_eq!(view.status, "applied");
    assert!(view.current_matches_applied);
    assert!(view.expected_configuration_observed);
    assert_eq!(view.request, request);
    assert_eq!(fs::read(&path).unwrap(), committed);
    reopened
        .restore_proposal(&request.request_id, &request.project_id)
        .unwrap();
    assert_eq!(
        reopened
            .configuration
            .resolve(&original.active)
            .unwrap()
            .shortcuts,
        original.resolve(&original.active).unwrap().shortcuts
    );
    assert_eq!(
        reopened
            .read_proposal(&request.request_id, &request.project_id)
            .unwrap()
            .status,
        "reverted"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn concept_presets_native_v4_validate_roundtrip_and_original_proposal_restore() {
    use atelier_bim::workbench::{
        creation_presets::CreationPresets,
        environment::proposal::Request,
        shortcuts::{self, Chord},
    };
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    service.save(service.configuration.clone()).unwrap();
    let before = fs::read(&path).unwrap();
    let inspection = service.inspect().unwrap();
    let mut configuration = inspection.configuration;
    let mut profile = configuration.resolve(&configuration.active).unwrap();
    let presets = CreationPresets {
        mass_height_mm: 6850,
        zone_height_mm: 3250,
        zone_purpose: "概念教学（未核准）".into(),
    };
    profile.creation_presets = Some(presets.clone());
    shortcuts::assign(
        &mut profile.shortcuts,
        "tool.zone",
        Some(vec![Chord::new("J", true, false, true)]),
    )
    .unwrap();
    configuration.apply_profile_draft(&profile);
    assert_eq!(
        configuration
            .differences_from(&service.configuration)
            .unwrap()
            .profiles[0]
            .after
            .as_ref()
            .unwrap()
            .creation_presets,
        Some(presets.clone())
    );
    let encoded = configuration.to_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["schema_version"], 4);
    for version in [1, 2, 3] {
        let mut disguised = value.clone();
        disguised["schema_version"] = version.into();
        assert!(
            Configuration::parse(&serde_json::to_vec(&disguised).unwrap())
                .unwrap_err()
                .contains("version4")
        );
    }
    assert_eq!(Configuration::parse(&encoded).unwrap(), configuration);
    let request = Request {
        schema_version: 1,
        request_id: "preset-original-operation".into(),
        project_id: "a".repeat(64),
        expected_snapshot: inspection.snapshot,
        configuration,
    };
    service.propose(request.clone()).unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "proposal is not application"
    );
    let draft = service
        .proposal_draft(&request.request_id, &request.project_id)
        .unwrap();
    service
        .confirm_proposal(&request.request_id, &request.project_id, draft)
        .unwrap();
    let mut reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let view = reopened
        .read_proposal(&request.request_id, &request.project_id)
        .unwrap();
    assert_eq!(view.status, "applied");
    assert_eq!(view.request, request);
    assert!(view.current_matches_applied);
    assert_eq!(
        reopened
            .configuration
            .resolve(&reopened.configuration.active)
            .unwrap()
            .creation_presets,
        Some(presets.clone())
    );
    let export = root.join("export.json");
    reopened.export(&export).unwrap();
    assert_eq!(
        Configuration::parse(&fs::read(export).unwrap()).unwrap(),
        reopened.configuration
    );
    let saved = fs::read(&path).unwrap();
    let mut invalid = reopened.configuration.clone();
    invalid.profiles[0]
        .creation_presets
        .as_mut()
        .unwrap()
        .mass_height_mm = 0;
    assert!(reopened.save(invalid).is_err());
    assert_eq!(fs::read(&path).unwrap(), saved);
    reopened
        .restore_proposal(&request.request_id, &request.project_id)
        .unwrap();
    assert!(
        reopened
            .configuration
            .resolve(&reopened.configuration.active)
            .unwrap()
            .creation_presets
            .is_none()
    );
    assert_eq!(
        reopened
            .read_proposal(&request.request_id, &request.project_id)
            .unwrap()
            .status,
        "reverted"
    );
    for legacy in [1, 2, 3] {
        let mut old: serde_json::Value = serde_json::from_slice(&before).unwrap();
        old["schema_version"] = legacy.into();
        assert!(
            Configuration::parse(&serde_json::to_vec(&old).unwrap())
                .unwrap()
                .resolve("openarchi.architecture")
                .unwrap()
                .creation_presets
                .is_none()
        );
    }
    assert!(
        CreationPresets::from_metres(3.0001, 3., "concept").is_err(),
        "never silently round a captured live default"
    );
    assert!(CreationPresets::from_metres(f64::NAN, 3., "concept").is_err());
    assert!(CreationPresets::from_metres(3., 3., "").is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_menu_and_real_toolbar_layers_roundtrip_with_version_and_anchor_protection() {
    use atelier_bim::workbench::{CommandRegistry, WorkbenchCommand};
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let old_panels = serde_json::json!({"toolbox":true,"navigator":true,"info_box":true,"quick_options":true,"quick_palette":false});
    assert_eq!(
        serde_json::to_value(Panels::default()).unwrap(),
        old_panels,
        "default toolbar flags must not change historical bytes"
    );
    let mut config = service.configuration.clone();
    let mut profile = Profile::builtin(WorkMode::SiteConcept);
    profile.id = "user.canvas".into();
    profile.name = "Canvas menu".into();
    profile.builtin_name = false;
    profile.project_menu = vec![
        "analysis.constraints".into(),
        "project.conditions".into(),
        "project.space_options".into(),
        "missing.plugin:command".into(),
    ];
    profile.panels.standard_toolbar = false;
    profile.panels.edit_toolbar = false;
    config.put(profile.layer());
    config.active = profile.id.clone();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&config.to_bytes().unwrap()).unwrap()["schema_version"],
        5
    );
    service.save(config.clone()).unwrap();
    let saved = fs::read(&path).unwrap();
    let reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert_eq!(
        reopened.configuration.resolve(&profile.id).unwrap(),
        config.resolve(&profile.id).unwrap()
    );
    assert!(
        CommandRegistry::resolve(&profile.project_menu[3]).is_none(),
        "unavailable provider identity stays data, never a callback"
    );
    for c in WorkbenchCommand::PROJECT_MENU {
        assert!(profile.project_menu.iter().any(|id| id == c.id()));
    }
    for version in 1..=4 {
        let mut disguised: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        disguised["schema_version"] = version.into();
        assert!(Configuration::parse(&serde_json::to_vec(&disguised).unwrap()).is_err());
        disguised["profiles"][0]
            .as_object_mut()
            .unwrap()
            .remove("project_menu");
        assert!(
            Configuration::parse(&serde_json::to_vec(&disguised).unwrap()).is_err(),
            "toolbar flags also require format5"
        );
    }
    for invalid in [
        vec!["analysis.constraints".into()],
        vec![
            "project.conditions".into(),
            "project.space_options".into(),
            "project.conditions".into(),
        ],
        vec![
            "project.conditions".into(),
            "project.space_options".into(),
            "bad identity".into(),
        ],
    ] {
        let mut bad = config.clone();
        bad.profiles[0].project_menu = Some(invalid);
        assert!(service.save(bad).is_err());
        assert_eq!(fs::read(&path).unwrap(), saved);
    }
    let mut untouched = config.clone();
    let mut draft = untouched.resolve(&profile.id).unwrap();
    draft.name = "Renamed".into();
    untouched.apply_profile_draft(&draft);
    assert_eq!(
        untouched.profiles[0].project_menu,
        config.profiles[0].project_menu
    );
    assert_eq!(untouched.profiles[0].panels, config.profiles[0].panels);
    let defaults = Profile::builtin(WorkMode::SiteConcept);
    draft.project_menu = defaults.project_menu;
    draft.panels = defaults.panels;
    untouched.put(draft.layer());
    untouched.validate().unwrap();
    assert!(untouched.profiles[0].project_menu.is_none() && untouched.profiles[0].panels.is_none());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&untouched.to_bytes().unwrap()).unwrap()["schema_version"],
        2
    );
    // Old files use the same absent-field defaults, without being written on Open.
    fs::write(&path, untouched.to_bytes().unwrap()).unwrap();
    let legacy = fs::read(&path).unwrap();
    let legacy_open = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert!(
        legacy_open
            .configuration
            .resolve(&profile.id)
            .unwrap()
            .panels
            .standard_toolbar
    );
    assert_eq!(fs::read(&path).unwrap(), legacy);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn editing_bindings_share_defaults_release_ownership_and_preserve_config() {
    use atelier_bim::workbench::{
        CommandRegistry, WorkbenchCommand,
        shortcuts::{self, Chord},
    };
    let actions = [
        WorkbenchCommand::Drag,
        WorkbenchCommand::Rotate,
        WorkbenchCommand::Mirror,
        WorkbenchCommand::Elevate,
        WorkbenchCommand::Resize,
        WorkbenchCommand::Multiply,
        WorkbenchCommand::ToolSettings,
    ];
    let mut bindings = vec![];
    shortcuts::validate(&bindings).unwrap();
    for (index, command) in actions.into_iter().enumerate() {
        assert!(!command.remote_invocation_supported());
        assert_eq!(CommandRegistry::resolve(command.id()), Some(command));
        let defaults = shortcuts::default_keys(command.id());
        assert_eq!(defaults.len(), 1);
        let before = bindings.clone();
        assert!(shortcuts::assign(&mut bindings, "tool.zone", Some(defaults.clone())).is_err());
        assert_eq!(bindings, before);
        let replacement = Chord::new(&format!("F{}", index + 6), false, false, false);
        // F12 is valid; none of these unmodified chords belong to another command.
        shortcuts::assign(&mut bindings, command.id(), Some(vec![replacement])).unwrap();
        shortcuts::assign(&mut bindings, "tool.zone", Some(defaults)).unwrap();
        let before = bindings.clone();
        assert!(shortcuts::assign(&mut bindings, command.id(), None).is_err());
        assert_eq!(
            bindings, before,
            "restoring a now-occupied default must be atomic"
        );
        shortcuts::assign(&mut bindings, "tool.zone", None).unwrap();
        shortcuts::assign(&mut bindings, command.id(), Some(vec![])).unwrap();
    }
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let mut config = service.configuration.clone();
    let mut profile = config.resolve(&config.active).unwrap();
    profile.shortcuts = bindings.clone();
    config.put(profile.layer());
    service.save(config).unwrap();
    let bytes = fs::read(&path).unwrap();
    let reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    assert_eq!(
        reopened
            .configuration
            .resolve(&reopened.configuration.active)
            .unwrap()
            .shortcuts,
        bindings
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        3
    );
    // Existing clipboard, unsupported edit and OS reservations remain protected.
    for chord in [
        Chord::new("C", true, false, false),
        Chord::new("T", true, true, false),
        Chord::new("Q", true, false, false),
    ] {
        assert!(shortcuts::assign(&mut bindings, "tool.zone", Some(vec![chord])).is_err());
    }
}

#[test]
fn precision_coordinate_commands_use_shared_registry_and_session_context() {
    use atelier_bim::workbench::{
        CommandEffect, CommandRegistry, WorkbenchCommand,
        shortcuts::{self, Chord},
    };
    let editor = atelier_bim::editor::Editor::new(atelier_bim::model::Project::default());
    let project = editor.project.clone();
    for (command, id, letters) in [
        (WorkbenchCommand::InputX, "input.coordinate.x", vec!["X"]),
        (WorkbenchCommand::InputY, "input.coordinate.y", vec!["Y"]),
        (
            WorkbenchCommand::InputDistance,
            "input.distance",
            vec!["D", "R"],
        ),
        (WorkbenchCommand::InputAngle, "input.angle", vec!["A"]),
    ] {
        assert_eq!(command.id(), id);
        assert_eq!(CommandRegistry::resolve(id), Some(command));
        assert!(command.is_coordinate_input());
        assert_eq!(
            shortcuts::keys(&[], id),
            letters
                .iter()
                .map(|k| Chord::new(k, false, false, false))
                .collect::<Vec<_>>()
        );
        let descriptor = CommandRegistry::describe(command, &editor);
        assert_eq!(descriptor.effect, CommandEffect::ToolSession);
        assert!(
            descriptor
                .context_requirements
                .contains(&"precision_input_session")
        );
        assert!(!descriptor.remote_invocation);
        assert!(command.edit_label().is_none());
    }
    assert!(!WorkbenchCommand::Wall.is_coordinate_input());
    assert_eq!(editor.project, project);
    assert!(editor.undo.is_empty());
    assert!(editor.redo.is_empty());
    shortcuts::validate(&[]).unwrap();
}

#[test]
fn precision_coordinate_bindings_can_rebind_unbind_restore_and_reject_conflicts_atomically() {
    use atelier_bim::workbench::shortcuts::{self, Chord};
    let mut bindings = vec![];
    // Explicit D/R is accepted by the same profile writer as its implicit default.
    let distance_defaults = shortcuts::default_keys("input.distance");
    shortcuts::assign(
        &mut bindings,
        "input.distance",
        Some(distance_defaults.clone()),
    )
    .unwrap();
    shortcuts::assign(&mut bindings, "input.distance", Some(vec![])).unwrap();
    shortcuts::assign(
        &mut bindings,
        "input.coordinate.x",
        Some(vec![Chord::new("D", false, false, false)]),
    )
    .unwrap();
    let before = bindings.clone();
    assert!(shortcuts::assign(&mut bindings, "input.distance", None).is_err());
    assert_eq!(bindings, before);
    shortcuts::assign(&mut bindings, "input.coordinate.x", None).unwrap();
    shortcuts::assign(&mut bindings, "input.distance", None).unwrap();
    assert_eq!(
        shortcuts::keys(&bindings, "input.distance"),
        distance_defaults
    );
    let f8 = Chord::new("F8", false, false, false);
    shortcuts::assign(&mut bindings, "input.coordinate.x", Some(vec![f8.clone()])).unwrap();
    let before = bindings.clone();
    assert!(shortcuts::assign(&mut bindings, "input.coordinate.y", Some(vec![f8])).is_err());
    assert_eq!(bindings, before);
    assert!(
        !shortcuts::keys(&bindings, "input.coordinate.x")
            .contains(&Chord::new("X", false, false, false))
    );
    shortcuts::assign(&mut bindings, "input.coordinate.x", None).unwrap();
    assert_eq!(
        shortcuts::keys(&bindings, "input.coordinate.x"),
        vec![Chord::new("X", false, false, false)]
    );
    // A non-registered canvas key is reusable only by an active-input command.
    shortcuts::assign(
        &mut bindings,
        "input.angle",
        Some(vec![Chord::new("P", false, false, false)]),
    )
    .unwrap();
    assert!(
        shortcuts::assign(
            &mut bindings,
            "tool.zone",
            Some(vec![Chord::new("P", false, false, false)])
        )
        .is_err()
    );
    for chord in [
        Chord::new("Q", true, false, false),
        Chord::new("W", true, true, false),
        Chord::new("C", true, false, false),
        Chord::new("3", true, false, true),
        Chord::new("Escape", false, false, false),
        Chord::new("Tab", false, false, false),
        Chord::new("Enter", false, false, false),
        Chord::new("Backspace", false, false, false),
        Chord::new("5", false, false, false),
        Chord::new("8", false, false, true),
    ] {
        let before = bindings.clone();
        assert!(shortcuts::assign(&mut bindings, "input.coordinate.x", Some(vec![chord])).is_err());
        assert_eq!(bindings, before);
    }
}

#[test]
fn precision_coordinate_bindings_roundtrip_in_existing_profile_schema() {
    use atelier_bim::workbench::shortcuts::{self, Chord};
    let root = directory();
    let path = root.join("environment.json");
    let mut service = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let mut config = service.configuration.clone();
    let mut profile = config.resolve(&config.active).unwrap();
    let commands = profile.commands.clone();
    // A personal binding that predates the field defaults keeps its ordinary
    // canvas context; the new input context cannot invalidate the saved mode.
    shortcuts::assign(
        &mut profile.shortcuts,
        "tool.zone",
        Some(vec![Chord::new("X", false, false, false)]),
    )
    .unwrap();
    shortcuts::assign(
        &mut profile.shortcuts,
        "input.coordinate.x",
        Some(vec![Chord::new("F8", false, false, false)]),
    )
    .unwrap();
    shortcuts::assign(&mut profile.shortcuts, "input.distance", Some(vec![])).unwrap();
    shortcuts::assign(
        &mut profile.shortcuts,
        "input.angle",
        Some(vec![Chord::new("F9", false, false, false)]),
    )
    .unwrap();
    let expected = profile.shortcuts.clone();
    config.put(profile.layer());
    service.save(config).unwrap();
    let bytes = fs::read(&path).unwrap();
    let reopened = EnvironmentService::open(&path, WorkMode::Architecture).unwrap();
    let profile = reopened
        .configuration
        .resolve(&reopened.configuration.active)
        .unwrap();
    assert_eq!(profile.shortcuts, expected);
    assert_eq!(profile.commands, commands);
    assert_eq!(
        shortcuts::keys(&profile.shortcuts, "input.coordinate.y"),
        shortcuts::default_keys("input.coordinate.y")
    );
    assert!(shortcuts::keys(&profile.shortcuts, "input.distance").is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        3
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}
