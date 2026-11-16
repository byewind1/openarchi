"""Optional task authorization fence within the existing MCP adapter.

Stores only target/revision and owned preview/operation identities, never a
second model. Reads cannot silently renew authorization after a user edit.
"""
import fcntl
import json
import os
from pathlib import Path
import tempfile
import threading

PREPARE = {"prepare_change", "prepare_native_change", "prepare_discovered_change", "prepare_spatial_intent", "prepare_mass_intent", "prepare_parametric_proposal", "prepare_task_update"}
WRITE = PREPARE | {"commit_change", "undo_operation", "save_project", "reopen_project", "recover_operation", "recover_document"}


class TaskGuard:
    def __init__(self, path):
        self.path = Path(path)
        if not self.path.is_absolute() or self.path.is_symlink() or not self.path.is_file():
            raise ValueError("existing absolute private task authorization file required")
        self.lock = threading.Lock()

    def store(self, state):
        fd, name = tempfile.mkstemp(prefix=".task-guard-", dir=self.path.parent)
        with os.fdopen(fd, "w") as file:
            json.dump(state, file, ensure_ascii=False, indent=2)
            file.flush()
            os.fsync(file.fileno())
        os.replace(name, self.path)

    def dispatch(self, transport, socket, method, arguments):
        with self.lock:
            fd = os.open(str(self.path) + ".lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, "w") as file:
                fcntl.flock(file, fcntl.LOCK_EX)
                state = json.loads(self.path.read_text())
                state.setdefault("previews", {})
                state.setdefault("operations", {})
                if method in WRITE:
                    current = transport(socket, "get_target", {})
                    if "error" in current:
                        return current
                    target = current["result"]["target"]
                    context = transport(socket, "get_context", target)
                    if "error" in context:
                        return context
                    revision = context["result"]["revision"]
                    if state.get("blocked") or target != state["target"] or revision != state["revision"] or arguments.get("target") != state["target"]:
                        state["blocked"] = {"code": "TASK_CONTEXT_CHANGED", "expected_target": state["target"],
                            "expected_revision": state["revision"], "actual_target": target, "actual_revision": revision,
                            "message": "Task authorization changed. Read and report the user edit; do not reprepare, overwrite, undo or renew authorization without user input."}
                        self.store(state)
                        return {"error": state["blocked"]}
                    if "expected_revision" in arguments and arguments["expected_revision"] != state["revision"]:
                        return {"error": {"code": "TASK_REVISION_NOT_AUTHORIZED"}}
                    if method == "commit_change":
                        preview = arguments.get("preview_id")
                        request = arguments.get("request_id")
                        if preview not in state["previews"] or (request in state["operations"] and state["operations"][request]["preview_id"] != preview):
                            return {"error": {"code": "TASK_PREVIEW_NOT_AUTHORIZED"}}
                        state["operations"].setdefault(request, {"preview_id": preview, "status": "pending",
                            "base_revision": state["previews"][preview]["revision"], "target": state["previews"][preview]["target"]})
                        # Persist identity BEFORE dispatch; loss recovery can
                        # query this exact pending operation after reconnect.
                        self.store(state)
                    if method == "undo_operation":
                        owned = state["operations"].get(arguments.get("request_id"), {})
                        if owned.get("status") not in {"committed", "undone"}:
                            return {"error": {"code": "TASK_UNDO_NOT_AUTHORIZED"}}
                    if method in {"recover_operation", "recover_document"}:
                        operation = state["operations"].get(arguments.get("request_id"), {})
                        if not operation or arguments.get("original_target") != operation.get("target"):
                            return {"error": {"code": "TASK_RECOVERY_NOT_AUTHORIZED"}}
                        if method == "recover_document" and state.get("resume_authorization") != {"original_target": operation["target"], "request_id": arguments["request_id"]}:
                            return {"error": {"code": "TASK_RECOVERY_NOT_AUTHORIZED"}}
                if method == "cancel_operation" and (arguments.get("target") != state["target"] or arguments.get("preview_id") not in state["previews"]):
                    return {"error": {"code": "TASK_PREVIEW_NOT_AUTHORIZED"}}
                response = transport(socket, method, arguments)
                result = response.get("result", {})
                if "error" not in response:
                    if method in PREPARE and result.get("preview_id"):
                        state["previews"][result["preview_id"]] = {"target": state["target"], "revision": state["revision"]}
                    if method in {"commit_change", "get_operation"} and result.get("status") == "committed" and result.get("receipt"):
                        request = arguments.get("request_id")
                        operation = state["operations"].get(request)
                        if operation:
                            receipt = result["receipt"]
                            if receipt["revision_before"] != operation["base_revision"] or receipt["document_session"] != operation["target"]["document_session_id"]:
                                return {"error": {"code": "TASK_OPERATION_NOT_AUTHORIZED"}}
                            if operation["status"] == "pending" and receipt["revision_before"] == state["revision"] and receipt["document_session"] == state["target"]["document_session_id"]:
                                state["revision"] = receipt["revision_after"]
                            operation["status"] = "committed"
                            operation["receipt"] = receipt
                    if method == "undo_operation" and result.get("status") == "undone":
                        state["revision"] = result["revision"]
                        state["operations"][arguments["request_id"]]["status"] = "undone"
                    if method == "reopen_project" and result.get("status") == "reopened":
                        state["target"] = {**state["target"], "document_session_id": result["document_session_id"]}
                        state["revision"] = result["revision"]
                    if method == "recover_operation" and result.get("reconciled"):
                        state["revision"] = result["current_revision"]
                    if method == "recover_document" and result.get("recovered_document"):
                        authorization = state["resume_authorization"]
                        restored = result["target"]
                        if restored["project_id"] != authorization["original_target"]["project_id"] or restored["instance_id"] != state["target"]["instance_id"]:
                            state["blocked"] = {"code": "TASK_RECOVERY_TARGET_MISMATCH"}
                            self.store(state)
                            return {"error": state["blocked"]}
                        state["target"] = restored
                        state["revision"] = result["restoration_receipt"]["revision_after"]
                        state["operations"][arguments["request_id"]]["status"] = result["status"]
                    self.store(state)
                return response
