"""Task authorization race/recovery invariants; no model or native UI claims."""
import json
from pathlib import Path
import tempfile
import unittest
from task_guard import TaskGuard


class Host:
    def __init__(self):
        self.target = {"instance_id": "fixture", "project_id": "a" * 64, "document_session_id": 1}
        self.revision = 0
        self.offset = 3.9
        self.commits = 0
        self.receipts = {}
        self.raise_after_commit = False

    def call(self, socket, method, params):
        if method == "get_target":
            return {"result": {"target": self.target.copy()}}
        if method == "get_context":
            return {"result": {"revision": self.revision}}
        if method.startswith("prepare_"):
            return {"result": {"preview_id": "preview", "base_revision": self.revision}}
        if method == "commit_change":
            request = params["request_id"]
            if request not in self.receipts:
                self.receipts[request] = {"revision_before": self.revision, "revision_after": self.revision + 1,
                                          "document_session": self.target["document_session_id"]}
                self.revision += 1
                self.commits += 1
            if self.raise_after_commit:
                self.raise_after_commit = False
                raise OSError("lost commit response")
            return {"result": {"status": "committed", "receipt": self.receipts[request]}}
        if method == "get_operation":
            return {"result": {"status": "committed", "receipt": self.receipts[params["request_id"]]}}
        if method == "undo_operation":
            self.revision += 1
            return {"result": {"status": "undone", "revision": self.revision}}
        if method == "reopen_project":
            self.target = {**self.target, "document_session_id": 2}
            self.revision = 0
            return {"result": {"status": "reopened", "revision": 0, "document_session_id": 2}}
        if method == "cancel_operation":
            return {"result": {"status": "cancelled"}}
        if method == "save_project":
            return {"result": {"status": "saved"}}
        if method == "recover_document":
            self.target = {**self.target, "project_id": params["original_target"]["project_id"], "document_session_id": 4}
            self.revision = 1
            return {"result": {"status": "committed", "recovered_document": True, "target": self.target.copy(), "restoration_receipt": {"revision_after": 1}}}
        raise AssertionError(method)


class TaskGuardTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.path = Path(self.folder.name) / "guard.json"
        self.host = Host()
        self.path.write_text(json.dumps({"target": self.host.target, "revision": 0}))
        self.guard = TaskGuard(self.path)

    def call(self, method, **params):
        return self.guard.dispatch(self.host.call, "fixture.sock", method, {"target": self.host.target.copy(), **params})

    def prepare(self):
        return self.call("prepare_discovered_change", expected_revision=self.host.revision)

    def test_user_edit_cannot_be_renewed_by_reading_new_context(self):
        self.prepare()
        self.host.revision += 1
        self.host.offset = 5.127246093750001
        self.assertEqual(self.call("get_context")["result"]["revision"], 1)
        response = self.prepare()
        self.assertEqual(response["error"]["code"], "TASK_CONTEXT_CHANGED")
        response = self.call("commit_change", preview_id="preview", request_id="new-identity")
        self.assertEqual(response["error"]["code"], "TASK_CONTEXT_CHANGED")
        self.assertEqual(self.host.offset, 5.127246093750001)
        self.assertEqual(self.host.commits, 0)
        self.assertEqual(self.call("cancel_operation", preview_id="preview")["result"]["status"], "cancelled")

    def test_own_commit_save_reopen_and_undo_advance_exact_fence(self):
        self.prepare()
        self.call("commit_change", preview_id="preview", request_id="own")
        self.assertEqual(json.loads(self.path.read_text())["revision"], 1)
        self.call("undo_operation", request_id="own")
        self.assertEqual(json.loads(self.path.read_text())["revision"], 2)
        self.call("save_project", expected_revision=2)
        self.call("reopen_project", expected_revision=2)
        state = json.loads(self.path.read_text())
        self.assertEqual(state["target"]["document_session_id"], 2)
        self.assertEqual(state["revision"], 0)

    def test_lost_response_reconnect_queries_original_identity_without_duplicate(self):
        self.prepare()
        self.host.raise_after_commit = True
        with self.assertRaises(OSError):
            self.call("commit_change", preview_id="preview", request_id="original")
        self.guard = TaskGuard(self.path)
        self.call("get_operation", request_id="original")
        self.assertEqual(json.loads(self.path.read_text())["revision"], 1)
        self.call("commit_change", preview_id="preview", request_id="original")
        self.assertEqual(self.host.commits, 1)

    def test_project_switch_and_foreign_preview_or_history_never_write(self):
        self.assertEqual(self.call("commit_change", preview_id="foreign", request_id="x")["error"]["code"], "TASK_PREVIEW_NOT_AUTHORIZED")
        self.assertEqual(self.call("undo_operation", request_id="foreign")["error"]["code"], "TASK_UNDO_NOT_AUTHORIZED")
        self.host.target["project_id"] = "b" * 64
        self.assertEqual(self.prepare()["error"]["code"], "TASK_CONTEXT_CHANGED")
        self.assertEqual(self.host.commits, 0)

    def test_name_collision_does_not_authorize_earlier_history_undo(self):
        self.host.receipts["older-user-operation"] = {"revision_before": 0, "revision_after": 1, "document_session": 1}
        self.host.revision = 1
        self.path.write_text(json.dumps({"target": self.host.target, "revision": 1}))
        self.prepare()
        response = self.call("commit_change", preview_id="preview", request_id="older-user-operation")
        self.assertEqual(response["error"]["code"], "TASK_OPERATION_NOT_AUTHORIZED")
        self.assertEqual(self.call("undo_operation", request_id="older-user-operation")["error"]["code"], "TASK_UNDO_NOT_AUTHORIZED")
        self.assertEqual(self.host.revision, 1)

    def test_document_recovery_needs_original_identity_and_explicit_resume_authorization(self):
        original = self.host.target.copy()
        state = {"target": original, "revision": 0, "operations": {"resume": {"target": original, "base_revision": 0, "preview_id": "old", "status": "committed"}}}
        self.path.write_text(json.dumps(state))
        response = self.call("recover_document", expected_revision=0, original_target=original, request_id="resume")
        self.assertEqual(response["error"]["code"], "TASK_RECOVERY_NOT_AUTHORIZED")
        state["target"] = {**original, "project_id": "b" * 64, "document_session_id": 3}
        self.host.target = state["target"].copy()
        state["resume_authorization"] = {"original_target": original, "request_id": "resume"}
        self.path.write_text(json.dumps(state))
        response = self.call("recover_document", expected_revision=0, original_target=original, request_id="resume")
        self.assertTrue(response["result"]["recovered_document"])
        after = json.loads(self.path.read_text())
        self.assertEqual(after["target"]["project_id"], original["project_id"])
        self.assertEqual(after["target"]["document_session_id"], 4)
        self.assertEqual(after["revision"], 1)


if __name__ == "__main__":
    unittest.main()
