#!/usr/bin/env python3
"""Local OpenArchi context, feature contract and verified-build receipts. Stdlib only."""
from __future__ import annotations

import argparse
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import signal
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parent.parent
STATE = ROOT / "target/ac29-workflow"
CATALOG = ROOT / "docs/development/ac29-context.json"
POLICY = 1


def now():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def json_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def read_json(path, default=None):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return default


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(path.name + "." + uuid.uuid4().hex + ".tmp")
    try:
        temp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def read_command(command, timeout=20):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed ({result.returncode}): {result.stderr.strip()[:1500]}")
    return result.stdout.strip()


def toolchain():
    rustup = Path.home() / ".cargo/bin/rustup"
    if rustup.is_file():
        compiler = read_command([str(rustup), "which", "--toolchain", "stable", "rustc"])
        formatter = read_command([str(rustup), "which", "--toolchain", "stable", "rustfmt"])
    else:
        compiler, formatter = shutil.which("rustc"), shutil.which("rustfmt")
    if not compiler or not formatter:
        raise RuntimeError("Rust compiler/rustfmt missing; use repository toolchain instructions. Nothing installed automatically.")
    return {
        "cargo": read_command([str(ROOT / "scripts/cargo.sh"), "--version"]),
        "rustc": read_command([compiler, "--version"]),
        "rustfmt": read_command([formatter, "--version"]),
        "rustfmt_path": formatter,
        "platform": list(os.uname()),
        "python": sys.version,
        "python_executable": sys.executable,
    }


def input_snapshot(chain):
    paths = set()
    for name in ("Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain", "rust-toolchain.toml", "rustfmt.toml", ".rustfmt.toml"):
        if (ROOT / name).is_file():
            paths.add(ROOT / name)
    for name in ("src", "tests", "locales", "packaging", "assets", "scripts", ".cargo"):
        folder = ROOT / name
        if folder.exists():
            paths.update(p for p in folder.rglob("*") if p.is_file() and "__pycache__" not in p.parts and p.suffix not in (".pyc", ".log") and p.name != ".DS_Store")
    # Hash relevant environment values privately; never put tokens or credentials in receipts.
    environment = {k: v for k, v in os.environ.items() if k.startswith(("CARGO_", "RUST")) or k in ("PATH", "CC", "CXX", "CFLAGS", "CPPFLAGS", "LDFLAGS", "SDKROOT", "DEVELOPER_DIR", "MACOSX_DEPLOYMENT_TARGET")}
    external = {}
    for name in ("config", "config.toml"):
        candidate = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / name
        if candidate.is_file():
            external[name] = digest(candidate)
    manifest = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
    inputs = {"policy": POLICY, "files": manifest, "toolchain": chain, "environment_sha256": json_hash(environment), "cargo_user_config_sha256": external}
    return {"sha256": json_hash(inputs), "inputs": inputs}


def target_identity(app, refresh=False):
    app = Path(app).expanduser().resolve()
    try:
        with (app / "Contents/Info.plist").open("rb") as stream:
            info = plistlib.load(stream)
        executable = app / "Contents/MacOS" / info["CFBundleExecutable"]
        stat = executable.stat()
        identity = [str(executable), stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns]
        cached = read_json(STATE / "target-identity.json", {})
        reused = not refresh and cached.get("file_identity") == identity
        sha = cached["sha256"] if reused else digest(executable)
        if not reused:
            write_json(STATE / "target-identity.json", {"file_identity": identity, "sha256": sha, "hashed_at": now()})
        historical = read_json(CATALOG)["target"]
        return {"status": "ok", "app": str(app), "executable": str(executable), "bundle_id": info.get("CFBundleIdentifier"), "version": info.get("CFBundleShortVersionString"), "build": info.get("CFBundleVersion"), "sha256": sha, "hash_reused": reused, "matches_recorded_target": sha == historical["sha256"] and info.get("CFBundleIdentifier") == historical["bundle_id"], "limits": "Identity only. No process/window/session was inspected or opened."}
    except (OSError, KeyError, ValueError, plistlib.InvalidFileException) as error:
        return {"status": "unavailable", "app": str(app), "error": str(error), "limits": "No automatic discovery of unrelated installations."}


def context(args):
    catalog = read_json(CATALOG)
    query = args.query.casefold()
    matches = [f for f in catalog["features"] if not query or any(query in t.casefold() or t.casefold() in query for t in f["tags"])]
    notes = {p for f in matches for p in f["notes"]}
    if query:
        for p in (ROOT / "docs/features").rglob("*.md"):
            if query in p.name.casefold() or query in p.read_text().casefold():
                notes.add(str(p.relative_to(ROOT)))
    try:
        chain = toolchain()
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        chain = {"status": "unavailable", "error": str(error)}
    previous = read_json(STATE / "latest.json")
    result = {"repository": str(ROOT), "git_status": read_command(["git", "status", "--short"]), "target": target_identity(args.archicad_app or catalog["target"]["app"], args.refresh_target), "toolchain": chain, "entrypoints": catalog["entrypoints"], "matching_notes": [{"path": p, "exists": (ROOT / p).is_file()} for p in sorted(notes)], "source": sorted({p for f in matches for p in f["source"]}), "previous_verification": {k: previous[k] for k in ("status", "finished_at", "report", "test_count") if k in previous} if previous else None, "native_ui_history": catalog["native_ui_history"], "limits": "Previous verification is historical until verify/bundle rechecks current inputs. Keyword matches are not proof of absence."}
    return result, 0


def new_feature(args):
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", args.slug):
        raise ValueError("Feature slug must contain lowercase letters/digits separated by hyphens")
    standalone = ROOT / f"docs/features/{args.slug}.md"
    folder = ROOT / "docs/features" / args.slug
    if standalone.exists() or folder.exists():
        raise ValueError(f"Feature already exists: {standalone if standalone.exists() else folder}; continue it instead")
    folder.mkdir(parents=True)
    contract = folder / "contract.md"
    title = args.title or args.slug
    text = f"""# {title}

## Goal and bounded scope

Archicad 29 command/gesture and entry point:
Initial view, selection, model, units and settings:
Requested outcome and explicit exclusions:

## Existing evidence / next unknown

Run `python3 scripts/ac29-workflow.py context {args.slug}` and reuse matching notes.
Target/build/hash: unknown until verified. Do not reuse historical PID/window/AX indices.

| Claim | Type (runtime/documentation/source/inference/unknown) | Source or actual Evidence ID | Scope/limits |
|---|---|---|---|
| Exact interaction sequence | unknown | none yet | not observed |

## Behavior contract

Entry → parameters/defaults/units → preview → confirm/cancel → model and views.
Validation/errors, snapping/selection, undo/redo, save/reopen:
Original OpenArchi choices and deferred capabilities:

## Finite acceptance checklist

| Scenario, matching initial conditions and expected result/tolerance | OpenArchi result | Archicad evidence/comparison |
|---|---|---|
| Positive case | unknown | unknown |
| Invalid input and cancel preserve prior state | unknown | unknown |
| Undo/redo and save/reopen where applicable | unknown | unknown |

## Delivery evidence

Source symbols/hash, checks actually run, test counts:
Exact packaged app path, matched UUID, signature:
Desktop steps actually performed / exact blocker:
Remaining unknowns:
"""
    with contract.open("x") as stream:
        stream.write(text)
    return {"status": "created", "contract": str(contract), "limits": "Template is not evidence of implementation or verification."}, 0


def uuid_map(path):
    output = read_command(["dwarfdump", "--uuid", str(path)])
    parsed = {arch: value.upper() for value, arch in re.findall(r"UUID:\s*([0-9A-Fa-f-]+)\s*\(([^)]+)\)", output)}
    if not parsed:
        raise RuntimeError(f"No Mach-O UUID found: {path}")
    return parsed


def inspect_bundle():
    release = ROOT / "target/release/openarchi"
    app = ROOT / "dist/OpenArchi.app"
    executable = app / "Contents/MacOS/openarchi"
    release_uuid, bundle_uuid = uuid_map(release), uuid_map(executable)
    if release_uuid != bundle_uuid:
        raise RuntimeError(f"Release/bundle UUID mismatch: {release_uuid} != {bundle_uuid}")
    read_command(["codesign", "--verify", "--deep", "--strict", str(app)])
    tree = {str(p.relative_to(app)): digest(p) for p in sorted(app.rglob("*")) if p.is_file()}
    return {"app": str(app), "uuids": bundle_uuid, "signature": "passed", "release_sha256": digest(release), "bundle_tree_sha256": json_hash(tree), "limits": "Matching UUID/signature alone does not establish freshness against current source."}


def reusable(receipt, snapshot, artifact):
    return bool(receipt and receipt.get("status") == "passed" and receipt.get("input_sha256") == snapshot["sha256"] and receipt.get("artifact") == artifact)


def bundle(_args):
    snapshot = input_snapshot(toolchain())
    artifact = inspect_bundle()
    receipt = read_json(STATE / "latest.json")
    fresh = reusable(receipt, snapshot, artifact)
    return {"status": "passed" if fresh else "unknown", "artifact": artifact, "source_freshness": "matches_successful_build_receipt" if fresh else "no_matching_successful_receipt", "receipt": receipt.get("report") if receipt else None, "desktop_ui": "unverified"}, 0 if fresh else 2


def run_step(name, command, run_dir, timeout):
    log = run_dir / (name + ".log")
    print(f"[{name}] {' '.join(map(str, command))}", file=sys.stderr, flush=True)
    started = time.monotonic()
    with log.open("w") as stream:
        process = subprocess.Popen(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            code = process.wait(timeout=timeout)
            status = "passed" if code == 0 else "failed"
        except subprocess.TimeoutExpired:
            # Terminate only the process group created for this verification step.
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            code, status = process.returncode, "timeout"
    return {"name": name, "command": list(map(str, command)), "status": status, "exit_code": code, "seconds": round(time.monotonic() - started, 3), "log": str(log)}


def verify(args):
    if os.environ.get("CARGO_TARGET_DIR"):
        raise ValueError("Unset CARGO_TARGET_DIR: repository packaging expects target/release/openarchi")
    STATE.mkdir(parents=True, exist_ok=True)
    with (STATE / "verify.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("Another workflow verification is running; do not race its build outputs")
        return verify_locked(args)


def verify_locked(args):
    run_dir = STATE / "runs" / (dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8])
    run_dir.mkdir(parents=True)
    report = {"status": "failed", "started_at": now(), "report": str(run_dir / "report.json"), "steps": [], "desktop_ui": "unverified"}
    try:
        chain = toolchain()
        snapshot = input_snapshot(chain)
        report["input_sha256"] = snapshot["sha256"]
        write_json(run_dir / "inputs.json", snapshot["inputs"])
        previous = read_json(STATE / "latest.json")
        if not args.force and previous and previous.get("input_sha256") == snapshot["sha256"]:
            try:
                artifact = inspect_bundle()
            except (OSError, RuntimeError, subprocess.TimeoutExpired):
                artifact = None  # Missing/stale output must be rebuilt, never certified.
            if artifact is not None and reusable(previous, snapshot, artifact):
                # Docs are deliberately outside build inputs, but diff checks include them.
                read_command(["git", "diff", "--check"])
                report.update(status="reused", reused_report=previous["report"], verified_at=previous["finished_at"], test_count=previous.get("test_count"), tool_test_count=previous.get("tool_test_count"), artifact=artifact, finished_at=now(), rechecked=["artifact_hashes", "uuids", "signature", "git_diff_check"])
                write_json(report["report"], report)
                return report, 0
        commands = [("format", [str(ROOT / "scripts/cargo.sh"), "fmt", "--all", "--", "--check"])]
        included = set()
        for source in (ROOT / "src").rglob("*.rs"):
            for name in re.findall(r'include!\s*\(\s*"([^"\n]+\.rs)"\s*\)', source.read_text()):
                included.add((source.parent / name).resolve())
        for index, source in enumerate(sorted(included)):
            commands.append((f"format-include-{index}", [chain["rustfmt_path"], "--edition", "2024", "--check", str(source)]))
        commands.extend([
            ("check", [str(ROOT / "scripts/cargo.sh"), "check", "--locked", "--all-targets"]),
            ("localization", [sys.executable, "scripts/check-localization.py"]),
            ("tests", [str(ROOT / "scripts/cargo.sh"), "test", "--locked", "--all-targets"]),
            ("tool-tests", [sys.executable, "-m", "unittest", "discover", "-s", "tests/tooling", "-v"]),
            ("diff", ["git", "diff", "--check"]),
            ("package", [str(ROOT / "scripts/package-macos.sh")]),
        ])
        for name, command in commands:
            step = run_step(name, command, run_dir, args.step_timeout)
            report["steps"].append(step)
            if step["status"] != "passed":
                raise RuntimeError(f"{name} {step['status']}; see {step['log']}. Later steps were not run; bundle unverified.")
            if name == "tests":
                text = Path(step["log"]).read_text()
                report["test_count"] = sum(map(int, re.findall(r"test result: ok\. (\d+) passed", text)))
            if name == "tool-tests":
                count = re.search(r"Ran (\d+) tests?", Path(step["log"]).read_text())
                report["tool_test_count"] = int(count[1]) if count else None
        report["artifact"] = inspect_bundle()
        if input_snapshot(toolchain())["sha256"] != snapshot["sha256"]:
            raise RuntimeError("Inputs changed during verification; results cannot certify current source. Rerun after changes settle.")
        report["status"] = "passed"
    except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
        report["error"] = str(error)
    report["finished_at"] = now()
    write_json(report["report"], report)
    write_json(STATE / "latest.json", report)
    return report, 0 if report["status"] == "passed" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("context", help="Read local target identity and matching repository knowledge")
    p.add_argument("query", nargs="?", default="")
    p.add_argument("--archicad-app")
    p.add_argument("--refresh-target", action="store_true")
    p.set_defaults(func=context)
    p = commands.add_parser("feature", help="Create a non-overwriting behavior contract")
    p.add_argument("slug")
    p.add_argument("--title")
    p.set_defaults(func=new_feature)
    p = commands.add_parser("verify", help="Check, test, package, verify and record; reuse unchanged successful inputs")
    p.add_argument("--force", action="store_true")
    p.add_argument("--step-timeout", type=int, default=900)
    p.set_defaults(func=verify)
    p = commands.add_parser("bundle", help="Read-only UUID/signature and source-receipt verification")
    p.set_defaults(func=bundle)
    args = parser.parse_args()
    try:
        if getattr(args, "step_timeout", 1) <= 0:
            raise ValueError("Step timeout must be positive")
        result, code = args.func(args)
    except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
        result, code = {"status": "failed", "error": str(error)}, 1
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return code


if __name__ == "__main__":
    sys.exit(main())
