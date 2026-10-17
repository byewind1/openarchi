"""Behavioral checks of cache invalidation/failure semantics, without GUI or cargo."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest
from unittest.mock import patch

MODULE_PATH = Path(__file__).resolve().parents[2] / "scripts/ac29-workflow.py"
spec = importlib.util.spec_from_file_location("ac29_workflow", MODULE_PATH)
workflow = importlib.util.module_from_spec(spec)
spec.loader.exec_module(workflow)


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="openarchi-tool-test-")
        self.root = Path(self.temp.name)
        self.state = self.root / "target/ac29-workflow"
        self.patches = [patch.object(workflow, "read_command", return_value=""), patch.object(workflow, "ROOT", self.root), patch.object(workflow, "STATE", self.state), patch.object(workflow, "CATALOG", self.root / "catalog.json")]
        for p in self.patches:
            p.start()
        self.addCleanup(self.temp.cleanup)
        for p in self.patches:
            self.addCleanup(p.stop)
        for folder in ("src", "tests", "scripts", "docs/features", "packaging", "locales"):
            (self.root / folder).mkdir(parents=True, exist_ok=True)
        (self.root / "Cargo.toml").write_text('[package]\nname="test"\n')
        (self.root / "src/lib.rs").write_text('include!("extra.rs");\n')
        (self.root / "src/extra.rs").write_text("pub fn extra() {}\n")
        self.chain = {"cargo": "cargo test-version", "rustc": "rustc test-version", "rustfmt": "fmt test-version", "rustfmt_path": "/fake/rustfmt"}
        self.artifact = {"uuids": {"arm64": "AAA"}, "signature": "passed", "bundle_tree_sha256": "signed-content", "release_sha256": "unsigned-content"}
        self.args = argparse.Namespace(force=False, step_timeout=2)

    def snapshot(self):
        return workflow.input_snapshot(self.chain)

    def step(self, name, command, run_dir, timeout):
        log = run_dir / (name + ".log")
        log.write_text("test result: ok. 5 passed; 0 failed; 0 ignored\ntest result: ok. 2 passed; 0 failed; 0 ignored\n" if name == "tests" else "ok\n")
        return {"name": name, "status": "passed", "command": command, "log": str(log)}

    def test_inputs_include_untracked_tests_locales_and_scripts_not_docs(self):
        before = self.snapshot()["sha256"]
        (self.root / "docs/new.md").write_text("documentation")
        self.assertEqual(self.snapshot()["sha256"], before)
        for name in ("tests/untracked.rs", "locales/en.json", "scripts/new.py", "packaging/Info.plist"):
            (self.root / name).write_text("new input")
            after = self.snapshot()["sha256"]
            self.assertNotEqual(before, after, name)
            before = after
        with patch.dict(os.environ, {"RUSTFLAGS": "--cfg=changed"}):
            self.assertNotEqual(before, self.snapshot()["sha256"])

    def test_environment_secrets_are_never_written_as_values(self):
        with patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "secret-test-value"}):
            value = self.snapshot()
        self.assertNotIn("secret-test-value", json.dumps(value))

    def test_success_receipt_reuses_only_same_inputs_and_signed_artifacts(self):
        with patch.object(workflow, "toolchain", return_value=self.chain), patch.object(workflow, "inspect_bundle", return_value=self.artifact), patch.object(workflow, "run_step", side_effect=self.step) as run:
            result, code = workflow.verify_locked(self.args)
            self.assertEqual((result["status"], code, result["test_count"]), ("passed", 0, 7))
            self.assertIn("format-include-0", [s["name"] for s in result["steps"]])
            calls = run.call_count
            reused, code = workflow.verify_locked(self.args)
            self.assertEqual((reused["status"], code), ("reused", 0))
            self.assertEqual(run.call_count, calls)
            self.assertEqual(reused["verified_at"], result["finished_at"])
            self.artifact["bundle_tree_sha256"] = "resigned-or-changed"
            rebuilt, _ = workflow.verify_locked(self.args)
            self.assertEqual(rebuilt["status"], "passed")
            self.assertGreater(run.call_count, calls)
            self.assertEqual(rebuilt["desktop_ui"], "unverified")

    def test_reuse_still_checks_whitespace_in_changed_documents(self):
        with patch.object(workflow, "toolchain", return_value=self.chain), patch.object(workflow, "inspect_bundle", return_value=self.artifact), patch.object(workflow, "run_step", side_effect=self.step) as run:
            workflow.verify_locked(self.args)
            calls = run.call_count
            with patch.object(workflow, "read_command", side_effect=RuntimeError("whitespace in docs")):
                result, code = workflow.verify_locked(self.args)
            self.assertEqual((result["status"], code), ("failed", 1))
            self.assertEqual(run.call_count, calls)
            self.assertIn("whitespace", result["error"])

    def test_failed_test_stops_package_and_invalidates_old_success(self):
        def fail_tests(name, command, folder, timeout):
            value = self.step(name, command, folder, timeout)
            if name == "tests":
                value["status"] = "failed"
            return value
        with patch.object(workflow, "toolchain", return_value=self.chain), patch.object(workflow, "inspect_bundle", return_value=self.artifact), patch.object(workflow, "run_step", side_effect=self.step):
            workflow.verify_locked(self.args)
        self.args.force = True
        with patch.object(workflow, "toolchain", return_value=self.chain), patch.object(workflow, "inspect_bundle", return_value=self.artifact), patch.object(workflow, "run_step", side_effect=fail_tests):
            result, code = workflow.verify_locked(self.args)
        self.assertEqual(code, 1)
        self.assertNotIn("package", [s["name"] for s in result["steps"]])
        self.assertFalse(workflow.reusable(workflow.read_json(self.state / "latest.json"), self.snapshot(), self.artifact))

    def test_concurrent_source_change_cannot_certify_package(self):
        def changed(name, command, folder, timeout):
            value = self.step(name, command, folder, timeout)
            if name == "package":
                (self.root / "src/lib.rs").write_text("changed while building")
            return value
        with patch.object(workflow, "toolchain", return_value=self.chain), patch.object(workflow, "inspect_bundle", return_value=self.artifact), patch.object(workflow, "run_step", side_effect=changed):
            result, code = workflow.verify_locked(self.args)
        self.assertEqual((result["status"], code), ("failed", 1))
        self.assertIn("Inputs changed", result["error"])

    def test_matching_uuid_without_receipt_is_unknown(self):
        with patch.object(workflow, "toolchain", return_value=self.chain), patch.object(workflow, "inspect_bundle", return_value=self.artifact):
            result, code = workflow.bundle(None)
        self.assertEqual((result["status"], code), ("unknown", 2))

    def test_bundle_rejects_uuid_mismatch_before_signature(self):
        with patch.object(workflow, "uuid_map", side_effect=[{"arm64": "A"}, {"arm64": "B"}]), patch.object(workflow, "read_command") as command:
            with self.assertRaisesRegex(RuntimeError, "UUID mismatch"):
                workflow.inspect_bundle()
            command.assert_not_called()

    def test_contract_creation_does_not_overwrite_or_escape_root(self):
        args = argparse.Namespace(slug="new-feature", title="Test feature")
        result, code = workflow.new_feature(args)
        self.assertEqual(code, 0)
        contract = Path(result["contract"])
        contract.write_text("user-edited contract")
        with self.assertRaises(ValueError):
            workflow.new_feature(args)
        self.assertEqual(contract.read_text(), "user-edited contract")
        with self.assertRaises(ValueError):
            workflow.new_feature(argparse.Namespace(slug="../outside", title="bad"))
        (self.root / "docs/features/guides.md").write_text("existing")
        with self.assertRaises(ValueError):
            workflow.new_feature(argparse.Namespace(slug="guides", title="Guides"))

    def test_target_hash_reused_then_invalidated_by_actual_executable_change(self):
        app = self.root / "Archicad.app"
        binary = app / "Contents/MacOS/Archicad"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"one-version")
        with (app / "Contents/Info.plist").open("wb") as stream:
            plistlib.dump({"CFBundleExecutable": "Archicad", "CFBundleIdentifier": "com.graphisoft.archicad29", "CFBundleShortVersionString": "29"}, stream)
        workflow.write_json(workflow.CATALOG, {"target": {"bundle_id": "com.graphisoft.archicad29", "sha256": workflow.digest(binary)}})
        first = workflow.target_identity(app)
        self.assertFalse(first["hash_reused"])
        self.assertTrue(workflow.target_identity(app)["hash_reused"])
        binary.write_bytes(b"another-version")
        changed = workflow.target_identity(app)
        self.assertFalse(changed["hash_reused"])
        self.assertFalse(changed["matches_recorded_target"])
        self.assertFalse(workflow.target_identity(app, refresh=True)["hash_reused"])

    def test_timed_out_owned_process_is_reported_not_passed(self):
        run_dir = self.root / "run"
        run_dir.mkdir()
        step = workflow.run_step("timeout", [sys.executable, "-c", "import time; time.sleep(5)"], run_dir, 0.05)
        self.assertEqual(step["status"], "timeout")
        self.assertNotEqual(step["exit_code"], 0)


if __name__ == "__main__":
    unittest.main()
