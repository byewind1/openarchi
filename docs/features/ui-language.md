# Main-window language switching (English / Simplified Chinese)

## Scope

Goal: switch the OpenArchi main window between English and Simplified Chinese
from either the title-bar language selector or `View > Language`, apply the
change immediately to visible UI, and restore the same language after a restart.
The switch changes presentation only; it must not edit the project, selection or
undo history.

Evidence types:

| Claim | Type | Source / evidence | Limits |
|---|---|---|---|
| Language can be chosen from the title bar and from `View > Language` | runtime observation | Final packaged app, 2026-10-18; title-bar selector switched both ways; `视图 > 语言 > English` and the title-bar selector then restored `简体中文` | Source is authoritative for OpenArchi, not for Archicad |
| The choice is stored as `zh-CN` or `en` under `~/Library/Application Support/OpenArchi/language` | readable source | `src/localization.rs::load_preference`, `save_preference` | File location is a local implementation choice |
| Title bar, top menus, toolbar, Info Box, toolbox, view tabs, counts, status bar and Quick Options bar change together | runtime observation | Final packaged app, 2026-10-18, AX tree and full-window screenshots | Covers the main floor-plan window; every dialog was not visually re-read in both languages |
| The selected language survives an application restart | runtime observation | Final packaged app, 2026-10-18, quit/relaunch after choosing `简体中文` | macOS local preference only |
| The window title follows the language while the macOS application-menu label stays `OpenArchi` | runtime observation | `49906133` bundle, 2026-10-18; AX window title and System Events menu-bar read in both languages | The application-menu label is intentionally language-neutral; it is the bundle/process identity, not a document title |
| Wall Settings dialog labels, choices, group headers and buttons follow the language | runtime observation | Final packaged app, 2026-10-18, `⌘T` on a selected wall; first group read field-by-field, all six group headers read after scrolling | Remaining groups' fields were not edited one-by-one on the final bundle; unit and dictionary tests cover them |
| Info Box wall fields follow the language and stay editable | runtime observation | Final packaged app, 2026-10-18, one wall selected with the Info Box open; Chinese fields read item by item, English render covered by regression | The final bundle's desktop pass read the Chinese Info Box fields individually; the English field list was checked through the render regression |
| Info Box wall-tool defaults follow the language | runtime observation | Final packaged app, 2026-10-18, Wall tool active with nothing selected; Chinese fields read item by item, then English fields read item by item after relaunching with the `en` preference | The English pass used a same-content test copy (`CFBundleIdentifier` changed only) and wrote the preference file directly; it did not exercise the selector gesture |
| Door/Window Info Box labels follow the language | readable source, unit regression and dictionary check | `info_box.rs`, `locales/en.json`, `locales/zh-CN.json`; `door_info_box_shows_and_edits_hosted_opening_fields`, `opening_tool_defaults_are_editable_and_localized`; 462-key localization check | Chinese labels were read on the `A5BCB793` package; English Door/Window Info Box rendering was not re-read on that package |
| Wall reference-line editing and snap text follows the language | unit regression, dictionary check and desktop smoke | `wall_reference_line_palette_renders_in_both_languages`, `wall_endpoint_snap_preview_matches_the_active_language`; `snap` label and join status read on the `638273BF` bundle | Both languages rendered headlessly; on the current desktop bundle the Chinese drag showed `捕捉：墙参考线` and the release status `墙端点已连接到参考线`, but the palette body was not re-read field by field in Chinese |

## Behavior contract

- The title-bar selector shows `English` or `简体中文` and lists both choices.
- `View > Language` exposes the same two choices.
- Choosing a language updates the main-window labels immediately in the current
  frame; no restart or project reopen is required.
- The language change writes the preference file and reports
  `Language · English` or `语言 · 简体中文` in the status bar, replacing any
  status text left in the previous language.
- Switching language does not mutate `Project`, clear or add selection, or add
  an undo/redo entry.
- On the next launch, the saved language is loaded before the first frame.
- The window title is `OpenArchi — Architectural Workbench` or
  `OpenArchi — 建筑设计工作台` and changes in the same frame as the rest of the
  UI. The macOS application-menu label is always `OpenArchi`; eframe derives it
  from the launch-time viewport title, so the launch title stays
  language-neutral and the localized title is applied from the first frame.
- Missing or unreadable preference data falls back to English.

## Main-window coverage

The localized surface includes the application title, top menus and toolbar,
Info Box, toolbox groups and tools, plan/split/3D view tabs, elevation tab,
element and selection counts, Quick Options bar and palette (zoom, orientation,
scale, layer combination, partial structure, pen set, model view options,
graphic overrides, renovation filter, design options and dimensions), status
messages, view-settings and navigator groups, and representative dialogs.

## Acceptance checklist

| Check | Result | Evidence |
|---|---|---|
| Simplified Chinese is applied from the title-bar selector | pass — desktop | AX tree changed to `文件 / 编辑 / 视图 / 设计 / 文档 / 选项 / 协作 / 窗 / 帮助`, `分屏视图`, `楼层平面 · F2`, `南立面`, `已选 0 个`, `12 个构件`, `就绪`, and Chinese Quick Options values |
| English is restored from the title-bar selector | pass — desktop | AX tree returned to `File / Edit / View / …`, `Split view`, `Floor plan · F2`, `South Elevation`, `0 selected`, `12 elements`, `READY`, `Language · English` |
| `View > Language` exposes and activates both choices | pass — desktop | On the final bundle the `视图 > 语言` submenu listed `English` and `简体中文`; choosing `English` changed the main window and status to `Language · English`; the title-bar selector then restored `简体中文` |
| Preference is written | pass — desktop | Preference file changed `en` → `zh-CN` → `en` |
| Language survives restart | pass — desktop | After choosing `简体中文` and relaunching the final bundle, the first AX tree was already Chinese |
| Window title follows the language | pass — desktop | Title-bar selector: English `OpenArchi — Architectural Workbench` → 简体中文 `OpenArchi — 建筑设计工作台`; both updated immediately and survived restart |
| Application menu stays language-neutral | pass — desktop | `osascript … get name of every menu bar item of menu bar 1` returned `Apple, OpenArchi` in both languages; it never showed a stale translated title |
| Switching does not edit the project or undo history | pass — desktop and unit regression | `Undo`/`Redo` stayed disabled and the model remained at 12 elements; `simplified_chinese_workspace_renders_without_changing_model_state` compares the project before/after rendering |
| Main labels and counts follow the language | pass — unit regression | `main_window_status_tabs_and_counts_follow_the_selected_language`, `status_text_localizes_dynamic_messages_without_rewriting_editor_state`, `main_window_and_status_labels_are_localized_in_chinese` |
| Quick Options labels follow the language | pass — unit regression | `quick_options_bar_and_palette_render_in_the_selected_language` |
| Wall Settings dialog follows the language | pass — desktop and source | English: `Wall Settings`, `GEOMETRY AND POSITIONING`, `Height`, `Top Link`, `Center / Inside / Outside`, `Cancel / Apply / OK`; Chinese: `墙设置`, `几何与定位`, `高度`, `顶部链接`, `居中 / 内部 / 外部`, `取消 / 应用 / 确定`; all six English and Chinese section headers were read after scrolling |
| Switching language leaves selection and undo history unchanged | pass — desktop | With one wall selected, `1 selected` / `已选 1 个`, `12 elements` / `12 个构件`, and disabled `Undo`/`Redo` stayed identical across the switches |
| Info Box wall fields follow the language | pass — desktop and unit regression | Chinese: `墙 009`, `全部已选: 1`, `图层`, `结构功能`, `几何方法`, `参考线`, `厚度`, `高度`, `顶部链接`, `平面图与剖面…`, `设置…`, `应用`; English field labels covered by `info_box_renders_wall_fields_and_follows_palette_visibility` |
| Info Box wall-tool defaults follow the language | pass — desktop (Chinese and English) | Chinese: `墙`, `默认设置`, `图层：`, `几何方法：`, `参考线：`, `参考线偏移`, `结构：`, `厚度`, `高度`, `顶部链接：`, `顶部偏移`, `底部偏移`, `平面图与剖面…`, `设置…`, `应用`. English: `Wall`, `Default Settings`, `Layer:`, `Geometry Method:` (`Single`/`Continuous`/`Polygonal`/`Rectangular`), `Reference Line:` (`Center`/`Inside`/`Outside`), `Reference Offset`, `Structure:`, `Thickness`, `Height`, `Top Link:`, `Top Offset`, `Bottom Offset`, `Floor Plan and Section…`, `Settings…`, `Apply`. Selecting `Outside` reported `Wall default settings applied · New walls use these values` with 12 elements and disabled `Undo`/`Redo`; `Settings…` opened the English `Wall Default Settings` dialog and `Escape` reported `Wall Settings cancelled` |
| Language switching keeps the Info Box editable and model-stable | pass — desktop | After `简体中文 → English → 简体中文`, the wall stayed selected at `12 elements`, `Undo`/`Redo` availability did not change, and the Info Box still accepted edits |
| Door/Window fields follow the language | pass — unit regression and dictionary | `门 · 洞口`, `宽度`, `高度`, `窗台高度` render in Chinese; `Host Wall`, `Width`, `Sill Height` are present in both dictionaries and covered by the opening UI regressions |

## Implementation

- `src/localization.rs`: `UiLanguage`, preference load/save, and the embedded
  `locales/en.json` / `locales/zh-CN.json` dictionaries.
- `src/app/shell.rs` and `src/app/mod.rs`: language selector, menu entry,
  translated main-window labels, `status_text` and count formatting.
- `src/editor.rs`: canonical default status text, translated only for display.

## Remaining unknowns

- No Archicad 29 runtime comparison was performed for language switching; this
  feature follows OpenArchi's own menu structure and labels.
- Dialogs outside the main floor-plan window were not each re-read in both
  languages after the final build. Wall Settings was re-read in both languages;
  the remaining wall groups' individual fields were not edited one-by-one on
  the final bundle.
- The English Info Box wall-tool defaults were read after writing the `en`
  preference directly to a same-content test copy. This supports label
  localisation and editability, not an independent selector gesture for that
  pass; both selector entry points themselves are covered by the final-bundle
  checks above.
- The stage 5 package `A5BCB793-F9D4-3652-A4C4-076E3CFBDFC5` was only checked
  for Chinese first-frame rendering, the `门` / `窗` toolbox labels and the
  Door Info Box's Chinese fields. Its title-bar menu opened with both choices,
  but English activation was not repeated on that package; the prior
  final-bundle selector evidence predates the opening labels.


## 2026-10-28 Edit-menu localization addendum

- Added EN/zh-CN keys for the full observed Edit catalog, numeric dialog fields, operation errors and preview. Duplicate-key/localization checker passed.
- 324 Rust + 15 tooling tests passed on final fresh forced verification; Drag dialog EN/ZH labels and RawInput OK/Cancel, Escape and language-render stability passed.
- Final bundle: `dist/OpenArchi.app`, arm64 UUID `A93D50AE-DC6E-34FB-8468-FE928B4EBB94`; release UUID matches and deep/strict ad-hoc signature passes.
- Current native desktop acceptance is **unknown**, not a reuse of older pass rows above. New isolated-HOME instance launched but CUA cannot bind its window (`cgWindowNotFound`, -10005). View > Language switching, EN→ZH→EN, selection/dirty/Undo state during actual switching, Chinese-first-frame restart, Quick Options and every other edit dialog in both languages remain unverified.
- Title-bar selector is absent in the pre-existing native-title implementation; independent selector-entry acceptance is not met. User preference preserved by isolated HOME.
- See [Edit coverage](edit-menu/coverage.md) for command-level limitations.


## 2026-10-28 — Edit shortcut labels

Native and egui Edit labels now use the same shortcut table; EN/ZH captions and modifier hints tested headlessly. Removed the obsolete Duplicate ⌘D tooltip and updated redo hints/help. Clipboard and focus event regressions pass. Final fresh verification: 332 Rust + 15 tooling tests, 1 existing ignored performance test; localization and package checks passed. Bundle UUID `A89C6B50-EEF7-350B-8A11-0902CCA10313`, release match and signature pass.

Final isolated-English launch performed. CUA still reports `-10005 cgWindowNotFound`; actual EN→ZH→EN, View > Language, first Chinese frame after restart, all dialog surfaces and native shortcut behavior are **unverified**. Title-bar language selector remains absent from earlier work. User preference preserved. Details: [shortcuts](edit-menu/shortcuts.md).


### 2026-10-28 Cmd+D 鼠标移动修正

没有新增/修改语言词条；移除了 Drag 数值模态入口，反馈为位移数值和 +/++，沿用状态翻译。英文/中文 egui 事件测试确认 Cmd+D 无模态且预览不改变模型/历史。完整验证：336 Rust / 15 tooling 通过，1 performance ignored；最终包 UUID `20722E96-CE86-3AD6-9C90-30AEB71F7628`、签名通过。CUA 无法区分原有用户实例与新启动隔离实例，故未操作原有用户窗口；最终包标题栏语言入口、View > Language 入口、EN→中文→EN、中文重启首帧、全表面翻译与切换不影响模型/选择/历史均为桌面 unknown，不声称完整本地化验收。


## 2026-10-28 REA移动实测修正最终验收


`python3 scripts/ac29-workflow.py verify --force`：340 Rust tests passed、1 performance test ignored；15 tooling tests passed；格式、cargo check --locked --all-targets、本地化检查和diff检查通过。

最终包：/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app。release与bundle arm64 UUID 均 `E25DBA40-EC95-3424-AEA1-D96269C31FD8`；codesign deep/strict通过；bundle检查与成功源码receipt一致。
最终运行PID45500，隔离HOME `target/rea-drag-smoke-home`，用户实际HOME偏好没有改动。新包完成英文移动/复制/多副本/取消，中文基点/单副本/多副本/取消，以及菜单撤销。

语言：真实新实例首帧为中文；同一最终实例经 View > Language 完成 English→简体中文→English；标题、顶层菜单、工具栏、Info Box、工具箱、视图标签、选择/构件数量、状态栏、Quick Options条已查看。切换前后恢复项目JSON完全相同；选择保持，Undo/Redo菜单状态及撤销结果正常。Quick Options浮动面板、无关对话框和原有部分3D子菜单译名未逐项验收，不能声称全应用本地化完成。标题栏语言选择器已在此前用户要求的原生标题栏改造中移除（native-menu-title/contract.md已有记录），本轮使用现有唯一语言入口，没有恢复它。


详见 [本轮验收](</Users/ren/MAC工作/工作/code/开源项目/openarchi/docs/features/edit-menu/drag-runtime-acceptance.md>)。AC29 REA10份，最终包REA1份，English/中文阶段AX记录在 edit-menu/evidence/。

### 2026-10-28 直接按压拖动补丁验证边界

此次未新增/修改语言词条，复用已有移动/单副本/多副本状态提示。全量342 Rust及本地化检查通过；最终包UUID `E856826B-CF89-38E1-99ED-1F24094199EF`、签名passed。隔离HOME英文启动到恢复模态有窗口截图，但系统前台loginwindow锁屏，无法完成中英实际交互、切换后状态不变或中文重启检查；上述均标unknown，待解锁。唯一语言入口仍为原生View > Language，标题栏旧入口已按此前用户要求移除。详见[直接拖动验收](edit-menu/direct-drag-acceptance.md)。

### 2026-10-28 Cmd+D取点修复

本轮不改词条；344 Rust与本地化检查通过，最终包UUID `6D9C20DF-E324-3692-A1EE-1AB1C1CF6D9A`、签名passed。最终包启动时系统loginwindow锁屏，未完成新包中英实际操作/语言切换/重启检查，均unknown。唯一View > Language入口及既有标题栏移除例外不变。详见[取点修复](edit-menu/cmd-d-object-click-fix.md)。

### 2026-10-29 三维导航图形验证

本轮无新文字/词条。最终包UUID `6C1ABA5B-EE6F-3647-AEC0-98D09BD2991A`，345 Rust/15 tooling通过、签名通过。隔离HOME同实例通过View > Language（AXPress）English→简体中文→English，三维底面与旋转光标在两种语言下桌面验证；原有3D/PERSPECTIVE和三维手势英文提示仍存在，不声明全量本地化。语言重启/无关面板未重复验收。标题栏旧语言入口已按此前用户要求移除，不恢复。证据见[三维旋转](3d-orbit/contract.md)。

### 2026-10-29 移动Tracker

Distance/Angle复用现有词条，新错误键Enter a valid number已同步en/zh-CN；字段热键D/A两语言一致、单位显式显示。新增中英文字段/错误信息回归及本地化校验通过，348 Rust/15 tooling通过。最终包UUID `7908B563-1CC2-3B50-82B0-FACA46863462`、签名passed。

最终独立包启动时系统锁屏（loginwindow），未完成真实English→简体中文→English切换、中文重启、Tracker视觉/原生键盘输入；以上均unknown，待解锁。原用户偏好未改，标题栏旧语言入口按既有用户要求不恢复。详见[Tracker记录](edit-menu/drag-tracker.md)。

## 2026-10-29 — Reshape toolbar subset

Final bundle UUID `BD0A3CB4-A003-3F81-AA7C-CB1D4EC3A237`, signature passed; forced verification: 364 Rust + 15 tooling tests. Both packs updated and duplicate/literal-key checks passed. Shared toolbar tooltips, native/egui menu aliases, new pointer statuses and Resize/Fillet strings localized. Actual final native instance: View > Language English → 简体中文 → English; Chinese persisted across restart (`未命名` first-frame title). Main window and Resize dialog captured in both languages; selection remained 11, visible geometry/title dirty marker unchanged. Exact model/history invariance automated, not desktop-process measured.

Desktop limitations: title-bar selector absent in current native-title configuration; Quick Options palette and Fillet dialog not checked in both languages. CUA `cgWindowNotFound`; native AX menu actions work, egui AX controls unavailable and bounded pointer/key injection did not activate canvas/dialog controls. Thus no claim of complete localization or native reshape completion. See [contract and final screenshots](reshape-toolbar/contract.md#final-packaged-desktop-evidence). Real user's preference untouched (isolated HOME); test preference restored to en.

## 2026-10-29 — Ctrl Trim tooltip follow-up

EN/ZH tooltip now documents holding Ctrl and releasing to exit; both translations covered by `trim_ctrl` egui tests. Forced verification **369 Rust + 15 tooling tests**, locale checks passed. Final package UUID **E8A6B374-EEA4-3641-889F-03610EBD897D**, strict/deep signature passed.

Isolated final native PID 11406: native View > Language English→简体中文→English, screenshots checked title/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar; 12 elements, selection 0, clean title remained visible. Chinese restart PID 36492 first-frame title `未命名`; restored isolated preference English, closed only test instances. Actual Ctrl tooltip hover/operation unknown: CUA bound older user instance, which was not operated. Title-bar selector remains absent under prior product choice; floating Quick Options and other dialogs not rechecked. Exact history invariance automated only. [Evidence and limits](reshape-toolbar/contract.md#ctrl-follow-up-validation--final-package).

## 2026-10-29 — Cmd correction, final desktop evidence

Tooltip now uses Cmd; both JSON packs updated, old Ctrl tooltip replaced. **373 Rust + 15 tooling tests passed**, locale/format/check/package passed. Final release/bundle UUID **967D7AFF-53BC-36FE-B4CF-A75C0B0FDF99**, strict/deep signature passed. See [Cmd acceptance](reshape-toolbar/contract.md#cmd-final-verification-and-native-acceptance).

Final isolated native PID 32074: View > Language EN→简体中文→EN; actual revised tooltip AX text and screenshot in both languages. Checked title, menus, toolbar, Info Box, toolbox, view tabs, counts, status and Quick Options bar; Chinese continuous Trim also operated successfully. Switching retained selected count 2, visible geometry, and byte-identical recovery JSON; Undo/Redo after switching worked. Complete history/dirty invariance automated only. Chinese preference survived restart PID 51563 (`未命名` first frame), recovery reopened final three-line geometry. Restored test preference English; closed test processes, user's preference and project untouched. Title-bar selector remains absent by earlier product choice; floating Quick Options and unrelated reshape dialogs not rechecked, no full localization claim.

## 2026-10-29 — Wall node palette

Eight new strings added to both packs for wall-node placement, validation, cancellation and numeric pointer mode; existing Distance/Angle/Apply/Cancel and transform names reused. **377 Rust + 15 tooling tests**, locale/fmt/check/package passed. Final bundle UUID **1BA174C1-BFB7-3218-8144-D1F78A68D3F2**, strict/deep signature passed.

Final native isolated instance: View > Language EN→ZH→EN; title/menu/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar inspected, wall-edge palette and active node/numeric palette captured in both languages. Chinese preview/cancel and actual commit successful, numeric field AX readbacks verified. Chinese restart first-frame title `未命名`, recovery reopened two-wall result. Restored isolated English preference and closed test instances; real user's process/preferences untouched. Existing title-bar selector absent by prior product choice; floating Quick Options, unrelated dialogs and full undo-stack/dirty invariance during native switch not directly measured. See [wall-node evidence](wall-editing.md#2026-10-29-final-verification).

## 2026-10-29 — Ten-command wall icon palette

Final **399 Rust + 15 tooling tests**, locale/fmt/check and fresh package passed; arm64 UUID **36F797A9-B20D-3043-9BD2-CC877AB1B026**, strict/deep signature passed. Both packs include all ten labels, stage hints, tracker values and contour validation errors; bilingual label/stage/error and actual icon-switch tests passed.

Final isolated PID31437: native View > Language EN→ZH→EN; two-row icon palette and separate tracker inspected, Distance/Angle and Elevation offset captured in both languages. Title/menu/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar inspected; project names intentionally remain data. Switch retained selected count, visible geometry and exact recovery JSON; native Undo/Redo worked. Restart PID37822 first-frame `未命名`, restored exact arc wall; isolated preference restored English and test processes closed. Existing user PID85570 untouched. Title-bar selector remains absent by prior product choice; floating Quick Options and unrelated dialogs unverified. Not all individual later-stage instructions have native captures in both languages (automated coverage passed). [Final evidence and boundary](wall-pet-palette/validation.md).

## 2026-10-29 — Default wall node mode and hover activation

Reuses existing translated command/stage/cancel strings; status now follows the active node/hover command. Final **402 Rust + 15 tooling tests**, locale/check/format/package passed; UUID **0A21A3A6-9135-3AD1-99D0-8AD06964E908**, strict/deep signature passed. Both-language event tests exercise all ten hover transitions and cursor rendering.

Final isolated native PID67172: View > Language EN→ZH→EN, palette/default-node/rotation tracker and solid/hollow pencil checked in both languages; title/menu/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar inspected. Model JSON identical after language switch and Undo/Redo. Restart PID74683 first observed title 未命名; isolated preference restored English, all test processes closed, user PID46331 untargeted. Existing title-bar selector absent by prior choice; floating Quick Options/unrelated dialogs and both-language 3D-specific screenshots unverified. [Evidence and exact limits](wall-pet-palette/cursor-followup/validation.md).

## 2026-10-29 — Compact wall palette and mouse-release confirmation

Fresh `verify --force`: **404 Rust + 15 tooling passed**, formatting/includes, all-target check, localization, diff/package. Receipt `target/ac29-workflow/runs/20261028T201755Z-a93fb169/report.json`. App `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **85D62F1A-641C-3977-B41A-7F73F94C586C**, strict/deep signature passed; source freshness matches receipt.

Compact 148×64-point ten-icon palette; command instructions and Apply/Cancel block removed. Pointer-adjacent numeric tracker, real text+Enter confirmation, plan/3D selected-baseline drag-release, held-drag hover switching and UI-release exclusion verified. Native final app passed EN→ZH→EN, Chinese restart, undo/redo and exact saved-model comparison. Title-bar language selector unavailable by prior product choice; floating Quick Options, unrelated dialogs, other command-specific tracker fields not native rechecked. Full evidence and explicit localization limits: [compact follow-up](wall-pet-palette/compact-followup/validation.md).

## 2026-10-29 — Selected multi-wall junction movement

Fresh `verify --force`: **413 Rust + 15 tooling passed**, formatting/includes, all-target check, locale validation, diff/package. Receipt `target/ac29-workflow/runs/20261028T203304Z-0a93b003/report.json`. App `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **E13465E1-CFD3-37B2-AC26-638D87358165**, strict/deep signature passed; source freshness matches receipt.

Selected straight-wall junction drag now moves every participating node atomically; fixed far ends, private preview, click/Enter/release, cancel, one undo/redo and JSON round trip. T/X through-walls split with existing metadata/opening handling. Nine new regressions cover geometry, selection, rejection and plan/3D event handling. Final native package verified three-wall shared plan/3D preview and release, exact saved coordinates/remote endpoints, Undo/Redo exact JSON, EN→ZH→EN, Chinese restart and reopen. Move node uses existing EN/ZH translation keys. Title-bar selector unavailable; unrelated dialogs/floating Quick Options and curved/profiled junctions unverified/unsupported. [Full evidence and scope](wall-junction-edit/validation.md); [experience candidates](wall-junction-edit/contract.md#implementation-evidence-and-candidate-lessons) await developer confirmation.

## 2026-10-29 three-state wall palette / multi-way junction cleanup

No user-visible strings were added or changed. Three-state palette/event tests exercise English and 简体中文 (active/candidate/normal, command promotion, staged/numeric preservation); localization validation and 428 Rust + 15 tooling tests pass on the fresh package. Receipt `target/ac29-workflow/runs/20261029T025513Z-71e440bb/report.json`; arm64 UUID **ECA2334E-2A55-32E6-A959-B0D0A70DD16A**, deep/strict signature passed.

Final packaged-app attempt used isolated HOME `target/wall-junction-cleanup-home`, PID 81807 / window 43250. macOS reported `CGSSessionScreenIsLocked=Yes`, foreground loginwindow PID 420; foreground-guarded input refused delivery. The window remained blank. **Current native palette/tracker, all representative main-window surfaces, View > Language EN→ZH→EN, model/history invariance on language switching and Chinese restart are therefore unknown/unverified** for this bundle. The title-bar selector remains unavailable per existing native-title product choice. No final native bilingual acceptance is inferred from menu-only AX or earlier-build screenshots. All dialogs and specialized 3D paths not checked remain unknown. [Blocker and verification record](wall-junction-cleanup/validation.md).

## 2026-10-29 — shared polyline/wall editing session

The independent polyline text palette was replaced with the wall icon palette/session. English and Chinese packs cover shared completion/cancel and polyline geometry validation messages; localization/duplicate-key checks passed. RawInput regressions exercise both languages for icon activation, cancellation and numeric distance/angle + Enter. Fresh final verification: 431 Rust tests, 15 tooling tests, `target/ac29-workflow/runs/20261029T045457Z-c7623878/report.json`; bundle UUID `1E951660-57EF-36A1-82DB-F6D93FBE7E45`, strict deep ad-hoc signature passed.

On final packaged instances, native AX and screenshots checked node and edge icon labels plus Distance/Angle in both languages, the main-window title, toolbar, Info Box, view tabs, selection/element counts, status and Quick Options strip. View → Language switches were exercised; Chinese restart first title was `未命名`. The title-bar selector is absent under the existing [native title contract](native-menu-title/contract.md); a second selector cannot be exercised. Project names such as Ground floor are model data. Offscreen/untouched dialogs, exact switch timing and a native assertion of all dirty/history fields remain unknown; automated history-preservation regressions pass. Original user instance/configuration preserved; only isolated demo instances were used. See [polyline validation and current screenshots](polyline-editing.md#2026-10-29--implementation-and-final-validation).

## 2026-10-29 — palette click-lock regression

No UI strings or locale-pack keys changed. The shared wall/polyline hover behavior now changes appearance only; completed left click changes the command. Both-language regression sequences pass along with localization validation: 432 Rust tests, 15 tooling tests; final bundle UUID `4095BCED-962A-36A1-8AEC-BD27D78FB11F`, strict deep signature passed. Native English wall/polyline click-lock behavior was exercised; Chinese native replay and unchanged unrelated dialogs are not newly certified. See [native evidence](wall-pet-palette/click-lock-20261029/native.md).

## 2026-10-29 — selected wall length removed

Removed the passive canvas length and redundant endpoint-popup Length row; existing shared translations remain for properties and actual editing. Bilingual paint/event regressions and final-package English → Chinese → English selection inspection passed. No changes to language entry points; the title-bar selector remains absent under the existing native-title contract. Chinese restart first title `未命名` passed (isolated PID 87819 with stored zh-CN preference); test preference restored to en. Untouched dialogs were not rechecked. Final UUID `5858F0D1-77C2-36F5-9485-0F512AA2329B`, strict deep signature passed, 432 Rust + 15 tooling tests. Evidence and remaining bounds: [wall selection](wall-selection.md#removal-delivered-and-checked).

## 2026-10-30 — shared wall endpoint icon menu

The old endpoint text panel is replaced with the existing localized ten-icon pet palette. Move node and every command accessible name/tooltip reuse existing English/Chinese keys; no new locale keys or language behavior introduced. Automated checks cover both ends and both languages, with no old heading/Done/length labels. Final native package showed the start-endpoint palette in English → Chinese → English; main-window selection/count stayed constant. Chinese restart with stored zh-CN showed first title `未命名`. Title-bar language selector remains absent under the existing native-title contract; View → Language was used. End endpoint/every command and untouched dialogs are not all newly verified natively. UUID `8A99BFD7-6D77-3566-B8E0-5F2404D5D802`, strict deep signature passed; 433 Rust + 15 tooling tests passed. [Native evidence](wall-pet-palette/endpoint-menu-20261030/native.md).


## 2026-10-31 Slab edit status

Added bilingual Slab edited / 楼板已编辑. Forced verify: 439 Rust and 15 tooling tests passed, UUID 500A51F6-6954-3181-892E-8B7D15A2280B with matching release/bundle and valid strict/deep signature. Final packaged app exercised View > Language EN→zh-CN, Chinese real edge insert/commit and undo, Chinese restart first frame, then View > Language→English. Checked representative main menus/toolbox/Info Box/tabs/count/status/Quick Options bar. Title-bar selector remains absent; floating Quick Options and untouched dialogs unknown. Language-invariance of full history/dirty state remains automated rather than extracted from native process. Isolated HOME only; original user preference unaffected. See [slab contract](slab-edge-editing/contract.md).


## 2026-10-31 完整边线编辑包验证结果

`python3 scripts/ac29-workflow.py verify --force` PASS：461项Rust测试（413 lib + 7/10/31 integration，1 ignored）、15项工具测试、格式/include格式、all-targets check、本地化、diff检查和fresh打包。凭据：`target/ac29-workflow/runs/20261030T191701Z-0793e2d5/report.json`。新增完整小面板13项实际egui事件回归全部通过，含双语角度45/135、表面/三作用域、孔洞输入、曲边保弧插点显式释放立即提交、变换/切线/倒圆角、候选分裂ID、undo/redo/JSON；另有7项完整kernel与2项geometry专项回归。

交付包：`dist/OpenArchi.app`；arm64 release/bundle UUID均 `78EE8804-D188-39AD-9986-35B2635C18B5`，deep/strict codesign PASS。本轮没有提交或推送。

最终包桌面限制：隔离HOME `/tmp/oa-slab-full-home-20261031`，当次新进程73397、CGwindow52526（逻辑1512×988），ps核对HOME正确；用户旧进程89567保留。CUA getApp路径返回用户旧实例（中文13个构件），未对其发输入；精确PID的REA启动观察`ev_592c3e705aec3e489c5426956fa1845fb7cb264e804998ff495aa599fc2b5ba4`确认新包英文启动、Recover对话框，完整记录[evidence/20261031-openarchi-final-native.json](evidence/20261031-openarchi-final-native.json)。它只证明启动/界面，不证明恢复已点击或完整鼠标编辑。

当前CUA对Archicad已知窗口的点击持续返回 `-10005 noWindowsAvailable`（重读AX/AXRaise及重新绑定后仍失败）；重置CUA会话没有解决多实例精确绑定。未绕过用户实例保护。故本轮新包完整真鼠标手势、EN→中文→EN即时菜单切换、中文重启、精确系统光标热点为unknown，不能套用上一版原生验证冒充通过；双语新增命令的自动事件回归通过。圆弧半径显示已实现，独立数值半径锁定待维护者确认当前分支保持规则，异步问题已提出，未将超时视为同意。
