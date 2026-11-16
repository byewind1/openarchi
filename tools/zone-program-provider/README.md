# Original Zone/program provider example

Apache-2.0. Standalone Rust + pinned serde_json, no application library/Rust ABI,
network, private files or provider account. Build outside the main checkout:

```sh
cp -R tools/zone-program-provider /tmp/openarchi-zone-provider
cargo build --offline --locked --manifest-path /tmp/openarchi-zone-provider/Cargo.toml
OPENARCHI_TEST_PROVIDER=/tmp/openarchi-zone-provider/target/debug/openarchi-zone-program-provider ./scripts/cargo.sh test --locked --test plugin_provider standalone_provider_declared_requirements_unknowns_stale_undo_and_missing_provider_reopen -- --ignored
```

Use a fresh destination; do not overwrite an existing build. Offline compilation
requires the normal Rust dependency cache; otherwise Cargo can fetch the locked
open-source dependencies as part of ordinary building. No installer or global
configuration change is required. The explicit ignored regression is separate
from normal in-process/provider-fault tests, which use protected original Rust
fixtures. Ignored is not passed unless actually run.

Native entry: load assets/plugins/example-zone-program.json in Plugins and jobs,
choose/review/authorize the exact executable SHA, then run its current-candidate
command. It receives one newline JSON frame:
`{protocol_version:1,intent:<the complete saved jobs::Intent>}`. It emits only
`{type:progress,identity:<original Identity>,completed:<geometry items>}` and one
`{type:result,artifact:<jobs::ResultArtifact>}` frame. Definitions are the public
serde types in src/plugins/jobs.rs and src/plugins/provider.rs. Stdout is protocol;
stderr is diagnostic. Exit0 plus a validated original result is required.

Progress is monotonic and bounded by input element count. Host caps/deadline and
owned process-group cancellation apply. No model-write command or live Editor is
passed. The executable still runs under the OS user account; this is NOT a sandbox.

Exact purpose/label matching only proposes candidates. It does not infer approved
assignments, requirement applicability, approval, net/legal area or compliance.
Declared row area is already an SI row total and is not multiplied by quantity.
Unknown areas stay unknown; empty program rows do not become satisfied.
