# Wall icon palette — final validation, 2026-10-29

Final app: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`.
Release and bundled arm64 UUID: **36F797A9-B20D-3043-9BD2-CC877AB1B026**. Strict/deep codesign passed. Final `bundle` check reports `matches_successful_build_receipt`.

Fresh forced receipt: `target/ac29-workflow/runs/20261028T191127Z-5671ac64/report.json`. **399 Rust tests + 15 tooling tests passed**; format (including included Rust files), all-target cargo check, locale completeness/duplicates, diff, packaging and source receipt passed. One performance test remains intentionally ignored by the repository. New coverage includes ten real operation results, actual ten icon clicks, nine graphical operations with idle frames between stages, both languages, nearest-end tangent, exact contour/hosted-opening math, reject/undo/redo/JSON behavior, plan/3D pick/render, old-path corruption guards. Elevation has a model regression plus native numeric field verification.

## Final packaged desktop test

Isolated HOME: `target/wall-pet-smoke-home`. Final test PID **31437**, window **42132**, restart PID **37822**, window **42140**. HOME identity checked on the first smoke launch; subsequent launches used the identical explicit HOME. Existing user PID **85570** remained untouched. Input: foreground-guarded CGEvent canvas clicks and AXPress/AXSetValue controls, per repository instructions. Test processes closed, isolated language restored to `en`.

Fixture: [one rotated wall](fixture.json), (-3,-1)→(3,1), 0.3 m thickness, 3 m height. Select then click middle reference line. [Final EN panel](final-palette-en.png), [AX names](final-palette-en-ax.txt). Original two-row vector icons, hover labels and active highlight visibly present; independent tracker below.

| Command | Final native result |
|---|---|
| Insert node | pass; one wall → two connected walls; original endpoints retained; [JSON](final-insert-result.json), [screen](final-insert-result.png) |
| Curve edge | pass; one exact arc wall, radius 3.3547244 m; [JSON](final-curve-result.json), [plan](final-curve-result-zh.png), [3D](final-model-en.png) |
| Tangent | pass; nearer first endpoint tangent produces persistent clockwise arc, radius 6.112562 m; [JSON](final-tangent-result.json), [screen](final-tangent-result.png) |
| Outer side | pass; two clicks (700,430) and (1050,430) produce horizontal exterior line while retaining original baseline; [first-stage AX](final-outer-stage1-ax.txt), [JSON](final-outer-result.json), [screen](final-outer-result.png) |
| New trapezoid point | pass; two trapezoids with identical shared exterior junction and reference junction; [JSON](final-trapezoid-result.json), [screen](final-trapezoid-result.png) |
| Drag | pass; both wall endpoints translated together; [JSON](final-drag-result.json), [screen](final-drag-result.png) |
| Rotate | pass; three native clicks and intervening idle frames; original pivot retained, new direction ≈90°; [JSON](final-rotate-result.json), [screen](final-rotate-result.png) |
| Mirror | pass; two axis points, reflected wall; [JSON](final-mirror-result.json), [screen](final-mirror-result.png) |
| Elevate | pass; tracker AXSetValue 1.25 m, Chinese Apply; unchanged XY/height, top+bottom offsets both 1.25 m; [JSON](final-elevate-result.json), [EN tracker](final-elevate-en.png), [ZH tracker](final-elevate-zh.png) |
| Multiply | pass; three copies plus original, equal successive offsets; [JSON](final-multiply-result.json), [screen](final-multiply-result.png) |

Model JSON assertions for count, shared trapezoid points, fixed outer-side baseline, equal copy offsets and elevation fields recorded in [native-checks.json](native-checks.json). Each operation was followed by native Undo to restore the source before the next command. Final arc Cancel → Undo → Redo returned byte-equivalent parsed project data. Curved 3D reference-edge click opens the palette, preview/cancel works ([3D palette](final-model-palette-en.png), AX logs alongside). Chinese recovery/restart restores the exact arc project ([reopened](final-reopened-zh.png)); persistence used recovery rather than a native Save As dialog.

Failed input retry: after Multiply Undo removed copied selection, the first click selected the original and did not open its palette. An initial AX value attempt touched an unapplied inspector draft; no model commit occurred. Reopened the actual Elevate tracker, verified its label/value, then applied successfully. Final JSON confirms original wall height stayed 3 m.

## Localization

One final instance switched native `View > Language` EN→ZH→EN repeatedly. Both palette rows have all ten localized AX names. Actual tracker Distance/Angle/Apply/Cancel and signed elevation fields captured in both languages. Title, menus, toolbar, Info Box, toolbox, view tabs, counts, status and Quick Options bar inspected. User/project names (e.g. Ground Floor) are project data and remain as stored. Final arc project JSON, selected count and visible geometry unchanged during language switches; Undo/Redo remained usable. Exact history/dirty invariance has automated coverage; full native stack was not introspected.

Chinese first-frame title **未命名** after restart: [screenshot](final-chinese-restart.png). Recovery reopens the final curve. Existing title-bar language selector is absent under the prior native-title product choice, so that entry point cannot be tested. Floating Quick Options and unrelated dialogs were not checked; not a claim of total application localization. Individual later-stage tracker hints were covered by bilingual tests; not every hint had a native screenshot in both languages.

## Limits and lessons

See [behavior contract](contract.md) for supported objects, numeric/graphical semantics, exact AC29 evidence limits, and the custom-wall junction-cleanup limitation. Pre-fix non-`final-` images are diagnostic/intermediate evidence, not final build acceptance.

Candidate lesson for developer confirmation: a numeric tracker must accept its neutral intermediate value (zero distance) without auto-clamping and generating an edit event; validate nonzero radius/axis at the operation boundary. Otherwise an idle frame between graphical points can silently switch the gesture to numeric mode. Not persisted into reusable AGENTS rules without confirmation.

## Default-node / cursor follow-up

Later user correction implemented automatic Insert-node entry, original-line solid/hollow pencil and hover-only command activation. New final UUID **0A21A3A6-9135-3AD1-99D0-8AD06964E908**, 402 Rust + 15 tooling tests; [final follow-up behavior and native evidence](cursor-followup/validation.md). This supersedes the earlier click-icon-to-start step.
