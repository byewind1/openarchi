#![cfg(unix)]
use atelier_bim::{automation_ipc::Endpoint, editor::Editor, model::Project};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
#[test]
fn ipc_reads_application_editor_and_rejects_stale_document() {
    let directory = std::env::temp_dir().join(format!(
        "oa-ipc-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("app.sock");
    let mut endpoint = Endpoint::bind(path.clone()).unwrap();
    let mut editor = Editor::new(Project::demo());
    let before = serde_json::to_value(&editor.project).unwrap();
    let (tx, rx) = mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = UnixStream::connect(path).unwrap();
        stream.write_all(b"{\"method\":\"get_target\"}\n").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        tx.send(line).unwrap();
    });
    let start = Instant::now();
    let line = loop {
        endpoint.poll(&mut editor);
        if let Ok(line) = rx.try_recv() {
            break line;
        }
        assert!(start.elapsed() < Duration::from_secs(4));
        thread::sleep(Duration::from_millis(10));
    };
    client.join().unwrap();
    let response: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        response["result"]["target"]["document_session_id"],
        editor.document_session()
    );
    assert_eq!(serde_json::to_value(&editor.project).unwrap(), before);
    assert!(Endpoint::bind(directory.join("second.sock")).is_err());
    drop(endpoint);
    assert!(!directory.join("app.sock").exists());
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn description_larger_than_socket_buffer_has_complete_json_frame() {
    let directory = std::env::temp_dir().join(format!(
        "oa-ipc-large-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("app.sock");
    let mut endpoint = Endpoint::bind(path.clone()).unwrap();
    let mut editor = Editor::new(Project::default());
    let (tx, rx) = mpsc::channel();
    let client = thread::spawn(move || {
        fn request(path: &std::path::Path, payload: serde_json::Value) -> serde_json::Value {
            let mut stream = UnixStream::connect(path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            writeln!(stream, "{}", payload).unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            assert!(line.ends_with('\n'));
            serde_json::from_str(&line).unwrap()
        }
        let target =
            request(&path, serde_json::json!({"method":"get_target"}))["result"]["target"].clone();
        let description = request(
            &path,
            serde_json::json!({"method":"describe_command","params":target}),
        );
        tx.send(description).unwrap();
    });
    let start = Instant::now();
    let response = loop {
        endpoint.poll(&mut editor);
        if let Ok(v) = rx.try_recv() {
            break v;
        }
        assert!(start.elapsed() < Duration::from_secs(8));
        thread::sleep(Duration::from_millis(10));
    };
    client.join().unwrap();
    assert_eq!(response["result"]["command"], "batch_change");
    assert_eq!(
        response["result"]["example"]["creates"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert!(serde_json::to_vec(&response).unwrap().len() > 8192);
    drop(endpoint);
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn dropping_endpoint_interrupts_partial_frame_and_preserves_replacement_path() {
    let directory = std::env::temp_dir().join(format!(
        "oa-ipc-drop-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("app.sock");
    let endpoint = Endpoint::bind(path.clone()).unwrap();
    let mut client = UnixStream::connect(&path).unwrap();
    client.write_all(b"{partial").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"replacement must survive").unwrap();
    let start = Instant::now();
    drop(endpoint);
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement must survive");
    drop(client);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn ipc_workbench_catalog_matches_ui_registry_and_rejects_other_document() {
    let directory = std::env::temp_dir().join(format!(
        "oa-ipc-workbench-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("app.sock");
    let mut endpoint = Endpoint::bind(path.clone()).unwrap();
    let mut editor = Editor::new(atelier_bim::storage::startup_example());
    let before = editor.project.clone();
    let expected =
        serde_json::to_value(atelier_bim::workbench::CommandRegistry::catalog(&editor)).unwrap();
    let (tx, rx) = mpsc::channel();
    let client = thread::spawn(move || {
        fn request(path: &std::path::Path, payload: serde_json::Value) -> serde_json::Value {
            let mut stream = UnixStream::connect(path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            writeln!(stream, "{}", payload).unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        }
        let target =
            request(&path, serde_json::json!({"method":"get_target"}))["result"]["target"].clone();
        let good = request(
            &path,
            serde_json::json!({"method":"list_workbench_commands","params":target}),
        );
        let mut foreign = target;
        foreign["document_session_id"] = serde_json::json!(u64::MAX);
        let bad = request(
            &path,
            serde_json::json!({"method":"list_workbench_commands","params":foreign}),
        );
        tx.send((good, bad)).unwrap();
    });
    let start = Instant::now();
    let (good, bad) = loop {
        endpoint.poll(&mut editor);
        if let Ok(result) = rx.try_recv() {
            break result;
        }
        assert!(start.elapsed() < Duration::from_secs(8));
        thread::sleep(Duration::from_millis(5));
    };
    client.join().unwrap();
    assert_eq!(good["result"]["commands"], expected);
    assert_eq!(good["result"]["discovery_only"], true);
    assert_eq!(bad["error"]["code"], "TARGET_MISMATCH");
    assert_eq!(editor.project, before);
    assert!(!editor.dirty());
    assert!(editor.undo.is_empty());
    drop(endpoint);
    std::fs::remove_dir(directory).unwrap();
}
