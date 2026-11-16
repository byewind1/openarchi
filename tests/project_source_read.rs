#![cfg(unix)]
use atelier_bim::{
    application_host::ApplicationHost, automation_ipc::Endpoint, model::Project,
    project_repository::Repository, storage,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    thread,
    time::{Duration, Instant},
};
fn request(
    ep: &mut Endpoint,
    host: &mut ApplicationHost,
    repo: Option<&Repository>,
    path: &Path,
    value: Value,
) -> Value {
    let path = path.to_owned();
    let client = thread::spawn(move || {
        let mut stream = UnixStream::connect(path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "{value}").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    });
    let start = Instant::now();
    while !client.is_finished() {
        ep.poll_with_repository(&mut host.document.editor, repo);
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(2));
    }
    client.join().unwrap()
}

#[test]
fn default_read_selects_one_large_dxf_outline_without_relaxing_original_limits() {
    let root = Path::new("/tmp").join(format!(
        "oa-dxf-read-{}",
        &storage::new_document_identity()[..12]
    ));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(Project::default());
    let mut repo =
        Repository::create_for_editor(&root.join("repository"), &host.document.editor).unwrap();
    let header = format!(
        "0\nSECTION\n2\nHEADER\n999\n{}\n0\nENDSEC\n",
        "padding, not selected content ".repeat(80000)
    );
    let record = "0\nLWPOLYLINE\n5\n36CE\n8\nContext\n90\n4\n70\n1\n10\n10\n20\n20\n10\n18\n20\n20\n10\n18\n20\n24\n10\n10\n20\n24\n";
    let bytes =
        format!("{header}0\nSECTION\n2\nENTITIES\n{record}0\nENDSEC\n0\nEOF\n").into_bytes();
    assert!(bytes.len() > 2 * 1024 * 1024);
    let path = root.join("stored-plan.dxf");
    std::fs::write(&path, &bytes).unwrap();
    let id = repo.add_source(&path).unwrap();
    let mut variants = vec![];
    for (name, entity) in [
        ("open", record.replace("70\n1\n", "70\n0\n")),
        ("curve", record.replace("70\n1\n", "70\n1\n42\n0.25\n")),
        ("wide", record.replace("70\n1\n", "70\n1\n43\n0.15\n")),
        (
            "elevated",
            record.replace("70\n1\n", "70\n1\n38\n-945.3216844278852\n"),
        ),
    ] {
        let file = root.join(format!("{name}.dxf"));
        std::fs::write(
            &file,
            format!("0\nSECTION\n2\nENTITIES\n{entity}0\nENDSEC\n0\nEOF\n"),
        )
        .unwrap();
        variants.push((name, repo.add_source(&file).unwrap()));
    }
    let block = root.join("block.dxf");
    std::fs::write(&block,format!("0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n5\nB1\n2\nNested\n{record}0\nENDBLK\n0\nENDSEC\n0\nEOF\n")).unwrap();
    variants.push(("block", repo.add_source(&block).unwrap()));
    let text = root.join("note.md");
    std::fs::write(&text, "A text note is not a DXF outline").unwrap();
    variants.push(("text", repo.add_source(&text).unwrap()));
    let model_before = host.document.editor.project.clone();
    let manifest_before = std::fs::read(root.join("repository/project.json")).unwrap();
    let socket = root.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(socket.clone(), &mut host).unwrap();
    let target = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"get_target"}),
    )["result"]["target"]
        .clone();
    let context = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"get_context","params":target}),
    );
    let base = json!({"target":target,"expected_revision":0,"sources_revision":context["result"]["project_sources"]["sources_revision"],"source_id":format!("repository:{id}"),"representation":"text","offset":0,"limit":32768});
    let raw = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"read_project_source","params":base}),
    );
    assert_eq!(raw["error"]["code"], "SOURCE_READ_FAILED");
    let mut selected = base.clone();
    selected["dxf_outline"] = json!({"handle":"36ce"});
    let read = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"read_project_source","params":selected}),
    );
    assert!(read.get("error").is_none(), "{read}");
    let content = read["result"]["data"]
        .as_str()
        .unwrap()
        .as_bytes()
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        atelier_bim::drawings::digest(&content),
        read["result"]["content_sha256"]
    );
    assert_eq!(read["result"]["original_sha256"], id);
    assert!(read["result"]["next_offset"].is_null());
    let observation: Value = serde_json::from_slice(&content).unwrap();
    assert_eq!(
        observation["outline"]["points"],
        json!([[10., 20.], [18., 20.], [18., 24.], [10., 24.]])
    );
    assert_eq!(
        observation["outline"]["record_sha256"],
        atelier_bim::dxf::primitives::closed_outline(&bytes, "36CE")
            .unwrap()
            .record_sha256
    );
    assert_eq!(observation["confirmed_unit"], Value::Null);
    for (name, id) in &variants {
        let mut args = selected.clone();
        args["source_id"] = json!(format!("repository:{id}"));
        let response = request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"read_project_source","params":args}),
        );
        if *name == "elevated" {
            assert!(
                response.get("error").is_none(),
                "source elevation must be observed, not applied: {response}"
            );
            let raw = response["result"]["data"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks(2)
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                .collect::<Vec<_>>();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(value["source_plane_elevation_native"], -945.3216844278852);
            assert_eq!(value["outline"]["points"], observation["outline"]["points"]);
            assert!(
                atelier_bim::dxf::primitives::closed_outline(
                    &std::fs::read(root.join("elevated.dxf")).unwrap(),
                    "36CE"
                )
                .is_err(),
                "existing model import must still refuse nonzero elevation"
            );
            continue;
        }
        assert!(
            response.get("error").is_some(),
            "{name} must be rejected: {response}"
        );
        if *name == "wide" {
            args["dxf_outline"]["allow_constant_width_centerline"] = json!(true);
            let response = request(
                &mut ep,
                &mut host,
                Some(&repo),
                &socket,
                json!({"method":"read_project_source","params":args}),
            );
            assert!(response.get("error").is_none(), "{response}");
            let raw = response["result"]["data"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks(2)
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                .collect::<Vec<_>>();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(value["outline"]["constant_width_native"], 0.15);
            assert_eq!(value["allow_constant_width_centerline"], true);
        }
    }
    for bad in [
        json!({"handle":"missing"}),
        json!({"handle":"0"}),
        json!({"handle":"../../36CE"}),
        json!({"handle":"AAAA"}),
    ] {
        let mut p = selected.clone();
        p["dxf_outline"] = bad;
        let result = request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"read_project_source","params":p}),
        );
        assert!(result.get("error").is_some(), "{result}");
    }
    let mut original = selected.clone();
    original["representation"] = json!("original");
    assert!(
        request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"read_project_source","params":original})
        )
        .get("error")
        .is_some()
    );
    let mut stale = selected.clone();
    stale["sources_revision"] = json!("old-context");
    assert_eq!(
        request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"read_project_source","params":stale})
        )["error"]["code"],
        "SOURCE_CONTEXT_CONFLICT"
    );
    let blob = root.join("repository").join(
        &repo
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .blob
            .path,
    );
    let mut changed = bytes.clone();
    changed[0] = b'1';
    std::fs::write(&blob, &changed).unwrap();
    let rejected = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"read_project_source","params":selected}),
    );
    assert!(rejected.get("error").is_some());
    assert!(rejected.get("result").is_none());
    assert_eq!(host.document.editor.project, model_before);
    assert!(host.document.editor.undo.is_empty());
    assert_eq!(
        std::fs::read(root.join("repository/project.json")).unwrap(),
        manifest_before
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    drop(ep);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn default_shared_host_reads_originals_with_source_versions_and_no_write_authority() {
    let root =
        std::env::temp_dir().join(format!("oa-sr-{}", &storage::new_document_identity()[..12]));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(Project::default());
    let image_path = root.join("unscaled sketch.png");
    let image = image::RgbaImage::from_pixel(48, 32, image::Rgba([30, 90, 170, 255]));
    image.save(&image_path).unwrap();
    let page = atelier_bim::drawings::import(&image_path, &root.join("assets"), 1).unwrap();
    host.document.editor.project.drawings.pages.push(page);
    let saved = root.join("input.openarchi.json");
    storage::save_document(
        &host.document.editor.project,
        &saved,
        host.document.editor.project_identity(),
    )
    .unwrap();
    host.document.editor = atelier_bim::editor::Editor::from_stored_document(&saved).unwrap();
    let mut repo =
        Repository::create_for_editor(&root.join("repository"), &host.document.editor).unwrap();
    let text_path = root.join("design decision v1.md");
    let text = "保留现有成果；外观意向未给尺寸，尺度须标为设计假设。".repeat(1700);
    std::fs::write(&text_path, &text).unwrap();
    let id = repo.add_source(&text_path).unwrap();
    let before = host.document.editor.project.clone();
    let identity = host.context();
    let path = root.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut host).unwrap();
    let target = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"get_target"}),
    )["result"]["target"]
        .clone();
    let ctx = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"get_context","params":target}),
    );
    let token = ctx["result"]["project_sources"]["sources_revision"].clone();
    assert_eq!(ctx["result"]["permissions"], json!(["read"]));
    let search = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"query_project_sources","params":{"target":target,"expected_revision":0,"query":"","offset":0,"limit":20}}),
    );
    assert_eq!(search["result"]["total"], 2);
    assert_eq!(search["result"]["entries"][0]["content_read"], false);
    let params = json!({"target":target,"expected_revision":0,"sources_revision":token,"source_id":format!("repository:{id}"),"representation":"text","offset":0,"limit":32768});
    let mut bytes = vec![];
    loop {
        let mut args = params.clone();
        args["offset"] = json!(bytes.len());
        let r = request(
            &mut ep,
            &mut host,
            Some(&repo),
            &path,
            json!({"method":"read_project_source","params":args}),
        );
        assert!(r.get("error").is_none(), "{r}");
        let part = &r["result"];
        let hex = part["data"].as_str().unwrap();
        for pair in hex.as_bytes().chunks_exact(2) {
            bytes.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
        }
        if part["next_offset"].is_null() {
            assert_eq!(part["total_bytes"], bytes.len());
            assert_eq!(part["original_sha256"], id);
            break;
        }
    }
    assert_eq!(bytes, text.as_bytes());
    let mut image_args = params.clone();
    image_args["source_id"] = json!("drawing:1");
    image_args["representation"] = json!("original");
    let r = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":image_args}),
    );
    assert_eq!(r["result"]["mime_type"], "image/png");
    assert_eq!(
        r["result"]["original_sha256"],
        atelier_bim::drawings::digest(&std::fs::read(&image_path).unwrap())
    );
    let mut stale_model = params.clone();
    stale_model["expected_revision"] = json!(99);
    let response = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":stale_model}),
    );
    assert_eq!(response["error"]["code"], "REVISION_CONFLICT");
    let mut traversal = params.clone();
    traversal["source_id"] = json!("repository:../../outside");
    let response = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":traversal}),
    );
    assert_eq!(response["error"]["code"], "SOURCE_NOT_FOUND");
    // Distinct versions stay distinct; old context cannot silently read a new repository state.
    std::fs::write(
        &text_path,
        "v2 conflicts with v1; no automatically chosen authority",
    )
    .unwrap();
    repo.add_source(&text_path).unwrap();
    let stale = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":params}),
    );
    assert_eq!(stale["error"]["code"], "SOURCE_CONTEXT_CONFLICT");
    let search = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"query_project_sources","params":{"target":target,"expected_revision":0,"query":"decision","offset":0,"limit":20}}),
    );
    assert_eq!(search["result"]["total"], 2);
    assert_eq!(host.document.editor.project, before);
    assert_eq!(host.context(), identity);
    assert!(host.document.editor.undo.is_empty());
    assert!(!host.document.editor.dirty());
    let after_context = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"get_context","params":target}),
    );
    assert_eq!(
        after_context["result"]["write_authorization"],
        ctx["result"]["write_authorization"]
    );
    // A differently bound repository is unavailable, rather than silently changing document context.
    let other = ApplicationHost::new(Project::default());
    let wrong = Repository::create_for_editor(&root.join("wrong"), &other.document.editor).unwrap();
    let context = request(
        &mut ep,
        &mut host,
        Some(&wrong),
        &path,
        json!({"method":"get_context","params":target}),
    );
    assert_eq!(
        context["result"]["project_sources"]["repository_attached"],
        false
    );
    drop(ep);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn original_read_rechecks_bytes_paths_and_explicit_representation_limits() {
    let root =
        std::env::temp_dir().join(format!("oa-sb-{}", &storage::new_document_identity()[..12]));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(Project::default());
    let mut repo =
        Repository::create_for_editor(&root.join("repository"), &host.document.editor).unwrap();
    let path = root.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut host).unwrap();
    let input = root.join("original.md");
    std::fs::write(&input, [0xff, 0xfe]).unwrap();
    let id = repo.add_source(&input).unwrap();
    let target = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"get_target"}),
    )["result"]["target"]
        .clone();
    let ctx = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"get_context","params":target}),
    );
    let args = json!({"target":target,"expected_revision":0,"sources_revision":ctx["result"]["project_sources"]["sources_revision"],"source_id":format!("repository:{id}"),"representation":"text","offset":0,"limit":32768});
    let result = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":args}),
    );
    assert_eq!(result["error"]["code"], "SOURCE_READ_FAILED");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("UTF-8")
    );
    // Tampering is tested only on this disposable repository copy.
    let blob = repo.root().join(&repo.manifest.sources[0].blob.path);
    std::fs::write(&blob, b"xx").unwrap();
    let mut original = args.clone();
    original["representation"] = json!("original");
    let result = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":original}),
    );
    assert_eq!(result["error"]["code"], "SOURCE_READ_FAILED");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("changed")
    );
    std::fs::remove_file(&blob).unwrap();
    std::os::unix::fs::symlink(&input, &blob).unwrap();
    assert!(
        repo.read_original(&id, 2048)
            .unwrap_err()
            .contains("symlink")
    );
    std::fs::remove_file(&blob).unwrap();
    std::fs::write(&blob, [0xff, 0xfe]).unwrap();
    let large = root.join("large.txt");
    std::fs::write(&large, vec![b'x'; 2 * 1024 * 1024 + 1]).unwrap();
    let large_id = repo.add_source(&large).unwrap();
    // Admission still accepts its existing source limit; this transport has a separate explicit cap.
    let ctx = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"get_context","params":target}),
    );
    let mut oversized = original.clone();
    oversized["sources_revision"] = ctx["result"]["project_sources"]["sources_revision"].clone();
    oversized["source_id"] = json!(format!("repository:{large_id}"));
    let result = request(
        &mut ep,
        &mut host,
        Some(&repo),
        &path,
        json!({"method":"read_project_source","params":oversized}),
    );
    assert_eq!(result["error"]["code"], "SOURCE_READ_FAILED");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("limit")
    );
    assert!(host.document.editor.project.elements.is_empty());
    assert!(host.document.editor.undo.is_empty());
    drop(ep);
    std::fs::remove_dir_all(root).unwrap();
}
