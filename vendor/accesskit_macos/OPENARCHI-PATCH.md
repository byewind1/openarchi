# Scoped upstream KVO fix

OpenArchi vendors the official0.26.3 archive (SHA in OPENARCHI-PATCH.json) to retain the existing accesskit0.24/eframe0.36 API. Only src/subclass.rs and main-thread integration tests are taken verbatim from official0.27.2, whose crate/VCS hashes are recorded. Normalized manifest adds the upstream integration target/libtest-mimic dev dependency and an isolated workspace for those tests. Licenses retained from the fixed upstream commit.

This backports AccessKit#835/#823: avoid discarding AppKit KVO's runtime subclass on adapter destruction, and correctly forward queries after the association is removed. It applies equally to the actual app and native evaluation harness. No global Cargo cache modification or accessibility removal. Remove this path patch when the normal eframe dependency line accepts an upstream fixed macOS adapter.
