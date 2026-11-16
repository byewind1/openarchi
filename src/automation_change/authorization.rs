//! Session write grants owned by the existing host service, independent of read revisions.
use super::*;
#[derive(Clone)]
pub(super) struct WriteGrant {
    target: Target,
    revision: u64,
    suspended: bool,
}
impl ChangeService {
    /// Native user takeover. Independent of transient pointer/modal blocking and grants.
    pub(crate) fn set_remote_paused(&mut self, paused: bool) {
        self.remote_paused = paused;
    }

    pub(crate) fn set_remote_interaction_blocked(&mut self, blocked: bool) {
        self.remote_interaction_blocked = blocked;
    }
    /// Called by the host before dispatch. Only an explicit native grant may reset a suspension.
    /// Environment-enabled endpoints bind once to their first observed application document.
    pub fn observe_document(&mut self, editor: &Editor) {
        // Native Undo/open may remove the task. Revocation never creates authority.
        if self.object_task_scope.as_ref().is_some_and(|g| {
            g.project_id == editor.project_identity()
                && crate::evidence::topic_tasks::find(&editor.project, &g.request_id).is_none()
        }) {
            self.object_task_scope = None;
            self.enabled = false;
            self.write_grant = None;
        }
        if !self.enabled {
            return;
        }
        let target = self.read.target(editor);
        let unowned =
            self.write_grant.as_ref().is_some_and(|g| {
                g.suspended || g.target != target || g.revision != editor.revision
            }) || !self.object_scope_binding(editor);
        if unowned {
            if let Some(g) = &mut self.object_task_scope {
                g.suspended = true;
            }
        }
        match &mut self.write_grant {
            None => {
                self.write_grant = Some(WriteGrant {
                    target,
                    revision: editor.revision,
                    suspended: false,
                })
            }
            Some(grant) if grant.target != target || grant.revision != editor.revision => {
                grant.suspended = true
            }
            _ => {}
        }
    }
    /// Trusted native/library grant; not exposed as an automation method.
    pub fn authorize_current(&mut self, editor: &Editor) {
        if self
            .object_task_scope
            .as_ref()
            .is_some_and(|g| g.project_id != editor.project_identity())
        {
            self.object_task_scope = None;
        }
        self.enabled = true;
        self.write_grant = Some(WriteGrant {
            target: self.read.target(editor),
            revision: editor.revision,
            suspended: false,
        });
    }
    pub fn write_authorization(&self, editor: &Editor) -> Result<(), Value> {
        if self.remote_paused {
            return Err(error("REMOTE_WRITES_PAUSED"));
        }
        if !self.enabled {
            return Err(error("WRITE_NOT_AUTHORIZED"));
        }
        let Some(grant) = &self.write_grant else {
            return Err(error("WRITE_CONTEXT_NOT_BOUND"));
        };
        if grant.suspended
            || grant.target != self.read.target(editor)
            || grant.revision != editor.revision
        {
            return Err(error("WRITE_AUTHORIZATION_SUSPENDED"));
        }
        self.object_scope_authorization(editor)?;
        if self.remote_interaction_blocked || editor.model_input_active() {
            return Err(error("USER_INTERACTION_ACTIVE"));
        }
        Ok(())
    }
    pub(super) fn check_write_grant(&mut self, editor: &Editor) -> Result<(), Value> {
        self.observe_document(editor);
        self.write_authorization(editor)
    }
    pub fn authorization_status(&self, editor: &Editor) -> Value {
        let blocked = self.remote_interaction_blocked || editor.model_input_active();
        let status = if self.remote_paused {
            "paused_by_user"
        } else if !self.enabled {
            "read_only"
        } else if self.write_authorization(editor).is_ok() {
            "authorized"
        } else if self.write_grant.is_none() {
            "awaiting_document"
        } else if self.write_grant.as_ref().is_some_and(|g| {
            g.suspended || g.target != self.read.target(editor) || g.revision != editor.revision
        }) {
            "suspended"
        } else if self.object_scope_authorization(editor).is_err() {
            "object_task_paused_or_finished"
        } else {
            "interaction_blocked"
        };
        json!({"scope":"local_endpoint_session","enabled":self.enabled,"status":status,"interaction_blocked":blocked,"authorized_revision":self.write_grant.as_ref().map(|g|g.revision),"current_revision":editor.revision,"reauthorization":"explicit native action only; reading context never renews","agent_identity_isolation":false,"persistent_task_authorization":false,"object_task":self.object_task_authorization(editor)})
    }
    // Called only after this service's successful commit/Undo/restoration. A failed/unknown
    // outcome deliberately leaves the old grant: inspect/reconcile the original operation first.
    pub(super) fn advance_owned_write(&mut self, editor: &Editor) {
        if let Some(grant) = &mut self.write_grant {
            if !grant.suspended {
                grant.target = self.read.target(editor);
                grant.revision = editor.revision;
            }
        }
    }
    /// File reopen is an owned document transition only when its pre-dispatch grant still matches.
    pub(crate) fn finish_owned_file(
        &mut self,
        before: &Target,
        revision: u64,
        editor: &Editor,
        result: &Value,
    ) {
        if result.get("result").is_some()
            && self
                .write_grant
                .as_ref()
                .is_some_and(|g| !g.suspended && &g.target == before && g.revision == revision)
        {
            self.advance_owned_write(editor);
        }
    }
}
