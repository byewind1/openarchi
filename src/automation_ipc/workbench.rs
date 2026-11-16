#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchInvoke {
    pub target: Target,
    pub expected_revision: u64,
    pub request_id: String,
    pub command_id: String,
    pub parameters: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkbenchOperation {
    target: Target,
    request_id: String,
}
pub struct WorkbenchPending {
    pub params: WorkbenchInvoke,
    pub reply: SyncSender<Value>,
}
struct WorkbenchReceipt {
    request: WorkbenchInvoke,
    response: Option<Value>,
}
impl Endpoint {
    pub(crate) fn validate_workbench(
        &self,
        editor: &Editor,
        p: &WorkbenchInvoke,
    ) -> Result<crate::workbench::WorkbenchCommand, Value> {
        if self.service.target(editor) != p.target {
            return Err(json!({"error":{"code":"TARGET_MISMATCH"}}));
        }
        if editor.revision != p.expected_revision {
            return Err(json!({"error":{"code":"REVISION_CONFLICT"}}));
        }
        self.changes
            .borrow()
            .unrestricted_write_authorization(editor)?;
        if p.request_id.is_empty() || p.request_id.len() > 256 {
            return Err(json!({"error":{"code":"INVALID_REQUEST_ID"}}));
        }
        if !p.parameters.as_object().is_some_and(|o| o.is_empty()) {
            return Err(json!({"error":{"code":"INVALID_PARAMETERS"}}));
        }
        let command = crate::workbench::CommandRegistry::resolve(&p.command_id)
            .ok_or_else(|| json!({"error":{"code":"COMMAND_NOT_FOUND"}}))?;
        if !command.remote_invocation_supported() {
            return Err(
                json!({"error":{"code":"USE_TRANSACTION_ADAPTER","command_id":p.command_id}}),
            );
        }
        Ok(command)
    }
    fn enqueue_workbench(&mut self, editor: &Editor, p: WorkbenchInvoke, reply: SyncSender<Value>) {
        if let Some(record) = self.workbench_receipts.get(&p.request_id) {
            let response = if record.request == p {
                record.response.clone().unwrap_or_else(|| json!({"result":{"status":"pending","request_id":p.request_id,"receipt_scope":"process"}}))
            } else {
                json!({"error":{"code":"REQUEST_ID_CONFLICT"}})
            };
            let _ = reply.try_send(response);
            return;
        }
        if let Err(error) = self.validate_workbench(editor, &p) {
            let _ = reply.try_send(error);
            return;
        }
        if self.workbench_receipts.len() >= 1024 {
            let _ = reply.try_send(json!({"error":{"code":"RECEIPT_CAPACITY"}}));
            return;
        }
        self.workbench_receipts.insert(
            p.request_id.clone(),
            WorkbenchReceipt {
                request: p.clone(),
                response: None,
            },
        );
        self.workbench_pending
            .push(WorkbenchPending { params: p, reply });
    }
    pub(crate) fn finish_workbench(&mut self, pending: WorkbenchPending, response: Value) {
        if let Some(record) = self.workbench_receipts.get_mut(&pending.params.request_id) {
            record.response = Some(response.clone());
        }
        // A disconnected caller does not discard the original operation result.
        let _ = pending.reply.try_send(response);
    }
    fn workbench_operation(&self, p: WorkbenchOperation) -> Value {
        match self.workbench_receipts.get(&p.request_id) {
            Some(record) if record.request.target == p.target => record.response.clone().unwrap_or_else(||json!({"result":{"status":"pending","request_id":p.request_id,"receipt_scope":"process"}})),
            Some(_)=>json!({"error":{"code":"TARGET_MISMATCH"}}),
            None=>json!({"error":{"code":"OPERATION_UNKNOWN","retry_safe":false,"receipt_scope":"process"}}),
        }
    }
}
