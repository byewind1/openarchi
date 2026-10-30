# Selected wall junction editing — 2026-10-29

Final app: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`. Release and bundle arm64 UUID **E13465E1-CFD3-37B2-AC26-638D87358165**, strict/deep signature passed. Forced receipt `target/ac29-workflow/runs/20261028T203304Z-0a93b003/report.json`: **413 Rust tests (365 library + 48 integration), 15 tooling tests passed**, one existing performance test ignored. Formatting/includes, all-target check, localization, diff and fresh packaging passed. Final `bundle` check reports source freshness matches that receipt.

## Automated coverage

Nine added regressions cover 2/3/4 shared endpoints, reversed endpoint ordering, rotated plan, private preview, click/Enter, drag release, Escape, one undo/redo, JSON round trip, T/X through-wall splits, unrelated selected wall and unselected incident wall preservation, exact finite-intersection tolerance versus screen hit tolerance, locks/groups/short segments/nonincident input atomic rejection, opening rehosting and opening-crossing rejection. 3D drag leaves camera unchanged; projected hit testing requires a shared edge elevation and checks all participating edge heights. Existing single-wall, palette hover, numeric editing and pointer regressions also pass.

The model operation is `ReshapeCommand::MoveWallJunction`; selection hit collection and private candidate implementation are in `src/reshape.rs`. `start_wall_junction` uses the existing compact shared plan/3D preview session. The first icon is labelled **Move node / 移动节点** using existing translations. Original selected elements remain selected; split pieces are added. The implementation contract and pending experience proposals are in [contract.md](contract.md).

## Native packaged-app verification

Isolated HOME `target/wall-junction-home` was confirmed against process 30076; window 42583 bounds (204,1,1512,988). User process 13982 was left running. Native AX actions operated menus/dialogs; foreground-guarded CoreGraphics operated mouse/keyboard gestures. Disposable [fixture](fixture.json): three 4 m straight walls sharing (0,0), fixed remote ends (-4,0), (4,0), (0,4). Opened through Open JSON, cleared a pending palette with Escape, then used Select All. First fixture-load pending-command provenance was not investigated; all acceptance gestures start from the captured three-wall selection.

| Claim | Result / evidence |
|---|---|
| Three selected walls, shared-node preview | pass; [selection](selected-en.png), [English preview](preview-en.png), [Chinese preview](preview-zh.png), corresponding AX captures show Move node / 移动节点 and 3 selected |
| Plan drag release changes all three, keeps far ends | pass; gesture (895,651)→(820,530), [result](result-zh.png), [saved JSON](result.json). All three starts exactly (-1.4285715,2.3154762), all original ends unchanged; no extra walls |
| One undo and redo | pass; one native Undo gives [JSON exactly equal to fixture](undo.json). One Redo followed by language restoration/save gives parsed JSON exactly equal to `result.json` |
| 3D shared-node preview in both languages | pass; [English](model-preview-en.png), [Chinese](model-preview-zh.png), corresponding AX; all three previews meet at one target |
| 3D direct drag release | pass; Escape pending preview, then direct drag (839,542)→(775,470); [result](model-result-zh.png), [JSON](model-result.json). All starts exactly (-1.7194796,0.9190507), original ends unchanged. An earlier attempted drag began over the floating tracker and correctly did not commit; final acceptance starts on unobstructed canvas |
| Language | View > Language EN→ZH→EN exercised; touched palette/tracker and representative main-window toolbar, Info Box, toolbox, tabs, counts, status, Quick Options bar inspected in both languages. Switching kept visible selection/counts; saved-model comparison after Undo/Redo and EN restoration was exact |
| Chinese restart and saved project reopen | pass; restart process 34298/window 42592 first title 未命名; [restart](chinese-restart.png), [AX](chinese-restart-ax.txt). Explicit Open JSON reopened the saved three-wall result; [screen](reopened-zh.png), [AX](reopened-zh-ax.txt). Isolated preference restored English and test processes closed |

## Limits

Only straight-wall finite junctions are supported; curved/profiled-wall node deformation remains outside this slice. T/X interior splitting, locked/opening rejection and 2/4-wall configurations are verified automatically, not individually captured in the native app. Numeric Enter uses the already-tested shared tracker; this follow-up additionally exercises graphical Enter in the UI regression, not new native typed-number capture. No new AC29 runtime-equivalence claim.

Title-bar language selector remains unavailable under the existing native-title product choice. Floating Quick Options and unrelated dialogs were not rechecked. Existing unlocalized 3D caption/navigation text and project/preset names remain as previously displayed; this is not complete application localization. Native screenshots/model captures establish visible/model outcomes; automated tests establish internal private-preview/history and camera invariants.
