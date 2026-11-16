#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentInspect {
    target: Target,
    expected_revision: u64,
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct EnvironmentPropose {
    target: Target,
    expected_revision: u64,
    proposal: crate::workbench::environment::proposal::Request,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentOperation {
    target: Target,
    request_id: String,
}
impl Endpoint {
    fn environment_request(
        &self,
        editor: &Editor,
        environment: Option<&crate::workbench::environment::EnvironmentService>,
        request: Request,
    ) -> Value {
        let (target, revision) = match &request {
            Request::EnvironmentInspect(p) => (&p.target, Some(p.expected_revision)),
            Request::EnvironmentPropose(p) => (&p.target, Some(p.expected_revision)),
            Request::EnvironmentProposal(p) | Request::EnvironmentCancel(p) => (&p.target, None),
            _ => unreachable!(),
        };
        if self.service.target(editor) != *target {
            return json!({"error":{"code":"TARGET_MISMATCH"}});
        }
        if revision.is_some_and(|r| r != editor.revision) {
            return json!({"error":{"code":"REVISION_CONFLICT"}});
        }
        // Staging is a persistent local side effect. It uses the default shared
        // task grant; it never gives permission to apply settings or execute code.
        if matches!(request, Request::EnvironmentPropose(_))
            || (matches!(request, Request::EnvironmentCancel(_))
                && self.changes.borrow().requires_object_task_control(editor))
        {
            if let Err(error) = self
                .changes
                .borrow()
                .unrestricted_write_authorization(editor)
            {
                return error;
            }
        }
        let Some(environment) = environment else {
            return json!({"error":{"code":"ENVIRONMENT_HOST_UNAVAILABLE"}});
        };
        let original_read = matches!(request, Request::EnvironmentProposal(_));
        let result=match request {
            Request::EnvironmentInspect(_)=>environment.inspect().and_then(|inspection|environment.project_proposals(editor.project_identity()).map(|project_proposals|json!({"inspection":inspection,"project_proposals":project_proposals,"model_changed":false,"remote_apply":false}))),
            Request::EnvironmentPropose(p)=>{
                if p.proposal.project_id!=editor.project_identity(){return json!({"error":{"code":"TARGET_MISMATCH"}});}
                environment.propose(p.proposal).map(|proposal|json!({"proposal":proposal,"model_changed":false,"remote_apply":false}))
            },
            Request::EnvironmentProposal(p)=>environment.read_proposal(&p.request_id,editor.project_identity()).map(|proposal|json!({"proposal":proposal})),
            Request::EnvironmentCancel(p)=>environment.cancel_proposal(&p.request_id,editor.project_identity()).map(|proposal|json!({"proposal":proposal})),
            _=>unreachable!(),
        };
        match result {
            Ok(result) => json!({"result":result}),
            Err(message) => {
                json!({"error":{"code":if original_read {"ENVIRONMENT_PROPOSAL_UNKNOWN"}else{"ENVIRONMENT_PROPOSAL_REFUSED"},"message":message,"retry_safe":false}})
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_host::ApplicationHost;
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixStream,
        path::Path,
        thread,
        time::{Duration, Instant},
    };
    fn call(
        endpoint: &mut Endpoint,
        host: &mut ApplicationHost,
        path: &Path,
        request: Value,
    ) -> Value {
        let path = path.to_owned();
        let client = thread::spawn(move || {
            let mut stream = UnixStream::connect(path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            writeln!(stream, "{request}").unwrap();
            let mut response = String::new();
            BufReader::new(stream).read_line(&mut response).unwrap();
            serde_json::from_str(&response).unwrap()
        });
        let start = Instant::now();
        while !client.is_finished() {
            endpoint.poll_with_host(host, None);
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(1));
        }
        client.join().unwrap()
    }
    #[test]
    fn schema_advertises_configuration_only_for_the_actual_shared_environment_host() {
        let root = PathBuf::from("/tmp").join(format!(
            "oa-env-schema-{}",
            &crate::storage::new_document_identity()[..12]
        ));
        fs::create_dir(&root).unwrap();
        let mut host = ApplicationHost::new(crate::model::Project::demo());
        let socket = root.join("ipc/app.sock");
        let mut endpoint =
            Endpoint::bind_for_host(socket.clone(), false, Some(root.join("journal")), &mut host)
                .unwrap();
        let missing = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_native_schema"}),
        );
        assert_eq!(missing["result"]["environment_proposal_version"], 0);
        host.configure_environment(
            &root.join("profile"),
            crate::workbench::WorkMode::Architecture,
        )
        .unwrap();
        let ready = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_native_schema"}),
        );
        assert_eq!(ready["result"]["environment_proposal_version"], 1);
        assert!(
            ready["result"]["environment_proposal_schema"]["$defs"]["Configuration"].is_object()
        );
        let target = host.operation_target();
        let unknown = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_environment_proposal","params":{"target":target,"request_id":"missing-original"}}),
        );
        assert_eq!(unknown["error"]["code"], "ENVIRONMENT_PROPOSAL_UNKNOWN");
        assert_eq!(unknown["error"]["retry_safe"], false);
        drop(endpoint);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn default_endpoint_configuration_proposals_keep_identity_and_do_not_apply_or_renew() {
        let root = PathBuf::from("/tmp").join(format!(
            "oa-ep-env-{}",
            &crate::storage::new_document_identity()[..12]
        ));
        fs::create_dir(&root).unwrap();
        let mut host = ApplicationHost::new(crate::model::Project::demo());
        host.configure_environment(
            &root.join("profile"),
            crate::workbench::WorkMode::Architecture,
        )
        .unwrap();
        let socket = root.join("ipc/app.sock");
        let mut endpoint =
            Endpoint::bind_for_host(socket.clone(), true, Some(root.join("journal")), &mut host)
                .unwrap();
        let target = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_target"}),
        )["result"]["target"]
            .clone();
        let context = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_context","params":target}),
        );
        assert!(
            context["result"]["permissions"]
                .as_array()
                .unwrap()
                .contains(&json!("write"))
        );
        let inspected = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"inspect_work_environment","params":{"target":target,"expected_revision":0}}),
        );
        assert!(inspected.get("error").is_none(), "{inspected}");
        let original = host.document.editor.project.clone();
        let inspection = &inspected["result"]["inspection"];
        let mut config = inspection["configuration"].clone();
        config["active"] = json!("openarchi.site-concept");
        let proposal = json!({"schema_version":1,"request_id":"wire-original","project_id":target["project_id"],"expected_snapshot":inspection["snapshot"],"configuration":config});
        let request = json!({"method":"propose_work_environment","params":{"target":target,"expected_revision":0,"proposal":proposal}});
        let result = call(&mut endpoint, &mut host, &socket, request.clone());
        assert_eq!(
            result["result"]["proposal"]["status"], "pending",
            "{result}"
        );
        assert_eq!(call(&mut endpoint, &mut host, &socket, request), result);
        assert!(!root.join("profile/work-environment.json").exists());
        assert_eq!(host.document.editor.project, original);
        // Send once and discard the connection. The next action is an original-ID read.
        let mut lost = proposal.clone();
        lost["request_id"] = json!("lost-reply");
        let request = json!({"method":"propose_work_environment","params":{"target":target,"expected_revision":0,"proposal":lost}});
        {
            let mut stream = UnixStream::connect(&socket).unwrap();
            writeln!(stream, "{request}").unwrap();
        }
        let original_query = json!({"method":"get_environment_proposal","params":{"target":target,"request_id":"lost-reply"}});
        let result = call(&mut endpoint, &mut host, &socket, original_query.clone());
        assert_eq!(result["result"]["proposal"]["sequence"], 2, "{result}");
        assert_eq!(result["result"]["proposal"]["status"], "pending");
        let id = host.document.editor.project.elements[0].id;
        host.document
            .editor
            .apply(crate::command::ModelCommand::MoveElements {
                ids: vec![id],
                offset: crate::model::Point::new(0.1, 0.),
            })
            .unwrap();
        let current = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_context","params":target}),
        );
        assert_eq!(
            current["result"]["write_authorization"]["status"],
            "suspended"
        );
        let revision = host.document.editor.revision;
        let refused = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"propose_work_environment","params":{"target":target,"expected_revision":revision,"proposal":proposal}}),
        );
        assert_eq!(refused["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
        assert_eq!(
            call(&mut endpoint, &mut host, &socket, original_query)["result"]["proposal"]["status"],
            "pending"
        );
        let cancelled = call(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"cancel_environment_proposal","params":{"target":target,"request_id":"lost-reply"}}),
        );
        assert_eq!(cancelled["result"]["proposal"]["status"], "cancelled");
        drop(endpoint);
        fs::remove_dir_all(root).unwrap();
    }
}
