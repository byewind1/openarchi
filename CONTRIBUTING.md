# Contributing to OpenArchi

Thanks for helping build a lightweight native BIM workbench. The project is an early prototype: focused, reviewable changes are more useful than broad rewrites.

## Before opening an issue or pull request

- Check the README and [roadmap](docs/roadmap.md) for current scope and known limitations.
- For behavior reports, include macOS version, Mac architecture, Rust version, steps to reproduce, expected behavior, and actual behavior. Do not attach private project files without removing sensitive information.
- For UI or workflow changes, describe the user task and include a screenshot or short recording when it clarifies the result.
- For geometry changes, describe the expected plan and 3D result and include a small reproducible model or a focused test where practical.

## Local development

Requirements: macOS on Apple Silicon and Rust 1.95 or newer.

```sh
./scripts/cargo.sh fmt --all -- --check
./scripts/cargo.sh check --locked --all-targets
./scripts/cargo.sh test --locked
./scripts/run.sh
```

For Archicad 29 feature work, the repository skill [openarchi-ac29](skills/openarchi-ac29/SKILL.md) records local target identity, evidence reuse and known native-UI limits. These Python 3.11+ stdlib-only helpers work from the repository root:

```sh
python3 scripts/ac29-workflow.py context guides
python3 scripts/ac29-workflow.py verify
python3 scripts/ac29-workflow.py bundle
python3 -m unittest discover -s tests/tooling -v
```

`verify` runs format checks (including included Rust files), compilation, Rust and tooling tests, diff checks, then the existing macOS packaging script and UUID/signature checks. Logs and source/artifact receipts are under ignored `target/ac29-workflow/`. A matching successful receipt can be reused; `verify --force` reruns all steps. `bundle` returns exit 2 for unknown source freshness, even if UUID and signature match. No helper launches GUI apps or installs dependencies. See the skill for feature-contract creation and desktop evidence requirements.

The native app uses egui/eframe and wgpu. Keep geometry and model behavior outside the UI layer where practical. User-visible model changes should be undoable and should update both plan and 3D views from the same project model.

## Contribution principles

- Keep the application native; do not introduce a WebView UI.
- Preserve the independent OpenArchi model. External formats belong behind adapters and must not become the in-memory model's identity.
- Treat AI as a proposed, inspectable actor over explicit model operations. Avoid hidden, irreversible model edits.
- Use original icons, interface assets, code, and sample content. Archicad is a workflow reference, not a source of assets or implementation.
- Keep limitations and evidence current in the README and validation notes.
- Include tests for nontrivial model, geometry, persistence, or interaction changes. Avoid tests that merely restate implementation details.

By submitting a contribution, you agree that it may be distributed under the project's Apache-2.0 license. This is not a Contributor License Agreement and does not transfer copyright.
