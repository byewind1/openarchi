use atelier_bim::{
    distance_guides,
    editor::Editor,
    guides::{
        DistanceReference, DoorReference, GuidePrimitive, GuideSettings, PersistentGuide,
        SnapMethod, SnapOrigin, SnapScope,
    },
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::{self, JsonAdapter},
};

fn point(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn project_with_guides() -> Project {
    let mut project = Project::demo();
    project.guide_settings = GuideSettings {
        snap_enabled: false,
        guide_lines_enabled: false,
        distance_guides_enabled: false,
        method: SnapMethod::Percent,
        divisions: 7,
        percent: 37.5,
        distance: 2.25,
        scope: SnapScope::EntireElement,
        origin: SnapOrigin::Midpoint,
        distance_reference: DistanceReference::ReferenceLines,
        door_reference: DoorReference::Side,
    };
    project.guides = vec![
        PersistentGuide {
            story: 0,
            primitive: GuidePrimitive::Line {
                origin: point(2., 3.),
                direction: point(1., 1.),
            },
        },
        PersistentGuide {
            story: 1,
            primitive: GuidePrimitive::Segment {
                start: point(-3., 4.),
                end: point(5., 7.),
            },
        },
        PersistentGuide {
            story: 2,
            primitive: GuidePrimitive::Circle {
                center: point(1., -2.),
                radius: 3.25,
            },
        },
    ];
    project
}

#[test]
fn legacy_projects_default_guides_without_changing_model_data() {
    let original = Project::demo();
    let mut legacy = serde_json::to_value(&original).unwrap();
    let object = legacy.as_object_mut().unwrap();
    object.remove("guide_settings");
    object.remove("guides");
    let loaded = JsonAdapter
        .import(&serde_json::to_vec(&legacy).unwrap())
        .unwrap();
    assert_eq!(loaded, original);
    assert_eq!(loaded.guide_settings, GuideSettings::default());
    assert!(loaded.guides.is_empty());
    assert_eq!(loaded.schema_version, 1);

    legacy["guide_settings"] = serde_json::json!({"snap_enabled": false});
    let partial = JsonAdapter
        .import(&serde_json::to_vec(&legacy).unwrap())
        .unwrap();
    assert_eq!(
        partial.guide_settings,
        GuideSettings {
            snap_enabled: false,
            ..GuideSettings::default()
        }
    );
    assert_eq!(partial.elements, original.elements);
}

#[test]
fn guide_settings_and_all_stories_roundtrip_through_json_and_disk() {
    let project = project_with_guides();
    project.validate().unwrap();
    let bytes = JsonAdapter.export(&project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), project);

    let directory = std::env::temp_dir().join(format!(
        "openarchi-guides-workflows-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("guides.json");
    storage::save(&project, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), project);
    assert!(!path.with_extension("json.tmp").exists());

    let mut invalid = project.clone();
    invalid.guides[0].story = 999;
    assert!(storage::save(&invalid, &path).is_err());
    assert_eq!(storage::load(&path).unwrap(), project);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn guide_creation_and_settings_commit_share_editor_undo_redo_and_dirty_state() {
    let original = Project::demo();
    let mut editor = Editor::new(original.clone());
    let changed = project_with_guides();
    editor.project = changed.clone();
    editor.commit(original.clone());
    assert_eq!(editor.undo.len(), 1);
    assert!(editor.dirty());
    assert_eq!(editor.project.elements, original.elements);
    editor.undo();
    assert_eq!(editor.project, original);
    assert!(!editor.dirty());
    editor.redo();
    assert_eq!(editor.project, changed);
    editor.saved = changed.clone();
    assert!(!editor.dirty());
    editor.undo();
    assert!(editor.dirty());
    editor.redo();
    assert!(!editor.dirty());
}

#[test]
fn remove_all_guides_includes_hidden_stories_and_restores_in_one_undo() {
    let original = project_with_guides();
    let mut editor = Editor::new(original.clone());
    editor.switch_story(1);
    let before = editor.project.clone();
    editor.project.guides.clear();
    editor.commit(before);
    assert!(editor.project.guides.is_empty());
    assert_eq!(editor.project.elements, original.elements);
    assert_eq!(editor.project.guide_settings, original.guide_settings);
    assert_eq!(editor.undo.len(), 1);
    let cleared = editor.project.clone();
    editor.undo();
    assert_eq!(editor.project, original);
    editor.redo();
    assert_eq!(editor.project, cleared);
    let no_change = editor.project.clone();
    editor.project.guides.clear();
    editor.commit(no_change);
    assert_eq!(editor.undo.len(), 1);
}

#[test]
fn invalid_guide_stories_and_geometry_are_rejected_at_import_and_export() {
    let original = project_with_guides();
    let mut invalid_story = original.clone();
    invalid_story.guides[1].story = 999;
    assert!(invalid_story.validate().is_err());
    assert!(JsonAdapter.export(&invalid_story).is_err());
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&invalid_story).unwrap())
            .is_err()
    );

    for primitive in [
        GuidePrimitive::Segment {
            start: point(1., 1.),
            end: point(1., 1.),
        },
        GuidePrimitive::Line {
            origin: point(0., 0.),
            direction: point(0., 0.),
        },
        GuidePrimitive::Circle {
            center: point(0., 0.),
            radius: -1.,
        },
    ] {
        let mut invalid = original.clone();
        invalid.guides[0].primitive = primitive;
        assert!(invalid.validate().is_err());
        assert!(JsonAdapter.export(&invalid).is_err());
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&invalid).unwrap())
                .is_err()
        );
    }
    let mut non_finite = original;
    non_finite.guides[0].primitive = GuidePrimitive::Circle {
        center: point(f32::NAN, 0.),
        radius: 1.,
    };
    assert!(non_finite.validate().is_err());
    assert!(JsonAdapter.export(&non_finite).is_err());
}

#[test]
fn invalid_snap_settings_are_rejected_even_when_snap_is_disabled() {
    let original = project_with_guides();
    let mut settings_cases = vec![];
    for divisions in [0, 2, 101] {
        settings_cases.push(GuideSettings {
            divisions,
            ..original.guide_settings.clone()
        });
    }
    for percent in [0., -1., 100., f32::NAN, f32::INFINITY] {
        settings_cases.push(GuideSettings {
            percent,
            ..original.guide_settings.clone()
        });
    }
    for distance in [0., -1., f32::NAN, f32::INFINITY] {
        settings_cases.push(GuideSettings {
            distance,
            ..original.guide_settings.clone()
        });
    }
    for settings in settings_cases {
        let mut invalid = original.clone();
        invalid.guide_settings = settings;
        assert!(!invalid.guide_settings.snap_enabled);
        assert!(invalid.validate().is_err());
        assert!(JsonAdapter.export(&invalid).is_err());
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&invalid).unwrap())
                .is_err()
        );
    }
}

#[test]
fn distance_edit_has_one_undo_step_preserves_guides_and_rejects_invalid_input() {
    let mut project = project_with_guides();
    project.elements.clear();
    project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: point(0., 0.),
            end: point(8., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let selected = project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: point(2., 2.),
            end: point(6., 2.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let mut editor = Editor::new(project.clone());
    editor.set_selection(vec![selected]);
    let measure =
        distance_guides::measurements(&project, 0, point(4., 2.), Some(selected), false)[0];
    assert_eq!(measure.distance, 2.);
    for value in [f32::NAN, -1., 0.] {
        assert!(
            distance_guides::apply_distance(&editor.project, &measure, selected, value).is_err()
        );
        assert_eq!(editor.project, project);
        assert!(editor.undo.is_empty());
    }
    editor.project =
        distance_guides::apply_distance(&editor.project, &measure, selected, 3.).unwrap();
    editor.commit(project.clone());
    let changed = editor.project.clone();
    assert_eq!(editor.undo.len(), 1);
    assert_eq!(changed.guides, project.guides);
    assert_eq!(changed.guide_settings, project.guide_settings);
    assert!(matches!(changed.elements[1].shape,
        Shape::Wall { start, end, .. } if start == point(2., 3.) && end == point(6., 3.)));
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&changed).unwrap())
            .unwrap(),
        changed
    );
    editor.undo();
    assert_eq!(editor.project, project);
    assert!(!editor.dirty());
    editor.redo();
    assert_eq!(editor.project, changed);
    assert!(editor.dirty());
}
