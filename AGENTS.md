# OpenArchi agent instructions

These rules apply to all repository work. OpenArchi is an independent, native Rust BIM application; its workflows may learn from observable authorized software behavior, while its source, model, UI and assets remain original.

## 用户指定的落盘目录

- 从 2026-10-20 起，本项目中用户要求“落盘”“保存研究”“写入方案”“记录进展”等持久化文档时，统一写入 `/Users/ren/Library/Mobile Documents/iCloud~md~obsidian/Documents/库/01-Projects/dev开发/OpenArchi 开发`，除非用户在当次明确指定其他位置。
- 该目录的 `agent.md` 记录落盘规则；写入前读取它并保留已有笔记。研究、方案与进展采用可辨识的主题及日期文件名，修正复制后的相对链接，保留证据、未知项和核准状态。
- 仓库 `docs/` 中的契约、验证记录与源码配套文档仍按本文件要求维护；仅写仓库或 `/tmp` 不算完成用户要求的落盘。需要双处记录时同步本次结论，并注明来源与日期。
- 最终回复提供指定目录内实际交付文件的链接。保存方案不等于用户批准实施。

## Project-specific reusable workflow

For Archicad 29 feature work or continuing an existing reconstruction, read [openarchi-ac29](skills/openarchi-ac29/SKILL.md). Run `python3 scripts/ac29-workflow.py context <feature>` to reuse the installed-target identity and existing evidence before investigating. The skill and tools are versioned in this repository; the global skill entry is only a link to this copy.

`python3 scripts/ac29-workflow.py verify` performs the repository checks and required packaging with source-to-artifact receipts. A `reused` result means an unchanged prior successful run was revalidated, not that tests were rerun. Desktop interaction still needs current runtime evidence. Use `--force` when fresh execution is needed.

## Default: make progress from a short request

- Treat the user's sentence as the requested outcome. Use the established conversation, target version, repository state and prior evidence; do not ask the user to repeat known context.
- Before coding, inspect `git status` and only the relevant source, docs and existing evidence. Preserve unrelated work. Reuse valid REA sessions and Evidence instead of starting over.
- Resolve Archicad behavior questions from existing matching evidence first. If the operation logic remains unclear or evidence conflicts, follow the pre-code gate below and ask the OpenArchi maintainer before changing dependent code. Choose reversible implementation details autonomously only when they do not decide or alter operation semantics; an assumption is not behavior confirmation.
- Do the investigation, implementation, checks and required app build before reporting back. Keep progress messages short and factual. Do not present a plan as completed work.

## Product boundaries

- Keep the desktop app native in Rust with egui/eframe and wgpu; do not replace it with a WebView.
- Keep the in-memory model independent. Treat IFC, PLN, GSM, BSM and other formats as explicit adapter boundaries; claim support only after implementation and verification.
- Recreate observable workflows with original code, controls, icons and assets. Do not copy proprietary source, private resources or branded assets.
- Keep model operations typed, validated, undoable and inspectable. UI and AI features use the same model operations.

## 编辑逻辑复用、鼠标微操作与界面设计（2026-10-29 用户要求）

### 优先查已有功能，再研究差异

- 以后续开发复用统一编辑逻辑为默认方向，尤其是基线/参考线、边线、普通线与节点的选择、移动、偏移、拉伸、插点和数值编辑。优先查找已开发的相同或相近操作、状态转换、模型命令、预览、光标及测试，不为每个构件重新研究或另写一套交互。
- 每项编辑功能先运行 `python3 scripts/ac29-workflow.py context <feature>`，再用 `rg` 查相关功能契约、研究证据、源码符号和回归用例；关键词包括用户用语及已有实现中的同义词。记录可复用的入口、共同行为、对象差异与具体未知点。已有充分证据覆盖的行为直接复用，仅对未覆盖的差异补充调查；有界搜索未命中不等于功能不存在。
- “编辑逻辑统一”是本项目的复用原则，不把某一对象的观测自动推广为所有 Archicad 对象的已证实行为。检查边界、参考方向、约束、连接、视图和选择前提；参数同名或图标相同不足以证明语义一致。存在疑问时按下文改代码前的核对规则向维护者确认。
- 将共同状态转换、命中判定、约束、预览、确认/取消与撤销入口集中在现有编辑层；通过有类型的对象适配处理几何差异。优先扩展已有实现，避免在各面板与工具事件中复制逻辑。复用或修改公共路径时，同时回归原有调用者与新增对象，防止同一操作在不同入口产生不同结果。

### 鼠标微操作必须按状态研究与验收

- 把鼠标反馈视为操作契约的一部分。对本次涉及的悬停、预选、选中、节点/边/参考线命中、按下、移动、拖动、释放、继续输入、确认、取消及无效输入，逐项记录触发条件、光标形态、热点位置、几何高亮、吸附标记、辅助线、数值反馈和模型是否变化；不适用项明确标注。
- 区分单击后移动与按住拖动、节点与边、参考线与可见轮廓；研究修饰键、捕捉/约束切换、阈值与命中优先级中本次涉及的差异。尚未测量的像素距离、时延或精确光标热点标为未知，不凭感觉填写。缩放与显示缩放比例是命中和光标验证的环境条件。
- AX 树用于控件身份与可访问状态；光标形态、热点、瞬时反馈和手势时序需通过确实包含这些信息的运行捕获、截图序列或人工步骤补证。普通截图可能不包含系统光标；不能据此推断光标外观或把单张图当作完整手势证据。遵守 Archicad 原生模态框的 AX 操作限制。
- OpenArchi 用共享编辑状态驱动光标与反馈，在悬停目标、工具、修饰键、取消、失焦与离开画布时正确更新/清理，避免光标残留、错误高亮、预览与提交结果不一致。光标与图标保持原创，仅复现已确认的操作含义。
- 验收同时包含事件序列回归与最终打包应用的真实鼠标操作：核对状态切换、反馈时机、命中、光标和热点、确认/取消及撤销；对高密度节点、重叠边等本次相关场景补验。自动事件测试不证明真实系统光标正确，无法检查的细节记录为 `unknown`；操作语义未清楚前不实现依赖它的行为。

### 图标与面板采用一致、极简的原生设计

- 极简以任务清晰、信息层级和操作效率为准：常用操作易发现，次要设置按需展开，相关参数就近分组，避免重复工具条、无效占位按钮、装饰性图形和无意义动效。不能为了视觉简洁隐藏必要反馈、错误解释或关键操作。
- 优先复用项目已有图标、控件和设计约定，统一图标尺寸、线宽、对齐、间距、字体层级及悬停/选中/禁用/焦点状态；新增图标保持原创、含义明确。图标按钮有本地化提示与可访问名称，不使用颜色作为唯一状态提示；适当扩大命中区，不能以缩小点击目标换取紧凑外观。
- 面板布局随窗口和内容合理调整，保证中英文、长标签、显示缩放及键盘导航可用；参数、单位、默认值和确认/取消行为保持清楚。复用 Archicad 操作逻辑时采用 OpenArchi 自身的原生 Rust/egui 视觉体系。
- “最新开发理念”落实为一致的状态驱动组件、可访问性、渐进披露、即时且稳定的反馈、清晰错误恢复与可维护的共享实现；不以追逐新框架或增加依赖代替设计。涉及会变化的平台规范或技术选型时，查当前官方文档，记录来源与兼容性，再在现有原生技术栈内实施。

## Fast feature workflow

### 0. 改代码前的 Archicad 操作逻辑核对（2026-10-29 用户要求）

- 每次修改代码前，先列出本次受影响的命令、手势与状态转换，对照目标 Archicad 29 INT 的对应操作逻辑及已有行为契约。复用匹配版本、初始状态和场景的有效证据；不要求每次重新操作 Archicad，也不为已有充分证据重复启动 REA。
- 核对入口、视图/选择前提、按下/移动/释放及数值输入、预览、确认/取消、模型与平面/3D结果、撤销/重做和保存/重开中本次涉及的部分。每项记录证据来源及适用边界；不能用同名菜单、图标、静态调用路径或 OpenArchi 自身测试代替 Archicad 操作行为证据。
- 涉及操作逻辑的改动，在动代码前必须将核对结论写入或更新到现有专题契约：目标行为、证据、预期结果、待确认项及验收步骤。小修复可补一条对应记录，无需另建文档。纯重构、构建或测试代码也须先检查是否影响操作逻辑；确实不涉及行为变化时，记录理由及现有回归依据，不虚构 Archicad 对应操作。
- 操作逻辑不明、证据不足/冲突、目标版本不匹配，或拟实现行为与 Archicad 不一致时，先整理具体场景、已知证据、未知点与建议，向 OpenArchi 维护者沟通确认；在确认前暂停依赖该结论的代码修改，可继续只读调查和无依赖的工作。不得以“可逆”“先做再说”或默认同意绕过确认。
- 优先使用当前对话中维护者已给出的明确要求或确认，不重复询问已解决的问题。需要新确认时在当前对话提出；维护者身份或联系渠道不明确时，请用户协调/指定，不能自行认定 Git 作者就是维护者，也不能未经授权向外部账号发消息。等待实际回复，不将超时视为确认。
- 维护者明确批准的产品差异须记录确认来源、日期、适用范围和与 Archicad 的差别；它是获准差异，不是 Archicad 等价证据。历史“产品选择”或旧假设未覆盖本次疑问时，不能据此自动放行。实现后按同一场景逐项验证；未验证项保留为 `unknown`，不得宣称全局一致。

### 1. Identify the exact question and evidence

- Name the command/gesture, entry point, selection/view prerequisites and target Archicad version/build from context. Search only what answers that feature.
- Reuse matching evidence and official AC29 documentation first; use focused runtime observation for interaction questions those sources cannot establish, and readable source for source analysis. A label, screenshot, string or static call path alone does not prove the full behavior. Unresolved operation semantics require maintainer confirmation under step 0.
- For REA work, read the current installed `reverse-engineer-anything` skill and relevant guide, and inspect the connected tools' actual schemas. A skill installation does not establish a live connection, available provider, or bound target. Use `open_binary` only for active-target operations that require it; `observe_native_ui` selects an existing PID/window, and target-free tools use their own explicit inputs without a preliminary static session. Reuse matching sessions. Do not run setup/readiness checks without an observed blocker.
- REA supplies evidence; implement OpenArchi with normal coding tools. Distinguish passive native UI observation from scenario actions, debugger launches and instrumentation, which may change target state. Check current schema support for gestures; use an available desktop input tool only within the authorized scenario and the Archicad AX restrictions below. Record its output as desktop-tool evidence, not invented REA Evidence. Do not claim runtime observation unless it was actually captured.
- Do not force REA onto ordinary OpenArchi source analysis. Never bypass licensing, authentication, permissions, encryption, DRM or safety controls.

### 2. Write or update a compact behavior contract before code changes

Record in `docs/features/<feature>/`, `docs/research.md` or an existing focused note:

- user goal and exact interaction sequence, including start state and selection;
- settings, units, defaults, validation, preview, confirmation and cancel;
- relevant geometry/model, plan/3D, selection/snapping, properties, undo/redo and save/restore effects;
- reused editing workflows/code/tests and object-specific differences; affected mouse states, cursor/hotspot and feedback evidence; icon/panel consistency and accessibility acceptance where applicable;
- evidence type for each important claim (`runtime observation`, `public documentation`, `readable source`, `static artifact analysis`, `inference` or `unknown`), with source/Evidence ID and limits;
- confirmed operation semantics, maintainer-approved differences (with confirmation source/date), unresolved items, reversible implementation assumptions that do not decide operation semantics, exclusions and a short reproducible acceptance checklist.

Keep facts, inferences and unknowns distinct. For runtime evidence, record target/build, environment, actions and returned Evidence ID. A screenshot or UI tree supports only what it visibly captures. Static reachability does not prove execution. For source analysis, record file and exact symbol/lines; include a hash when practical. Never invent evidence IDs or treat bounded search as proof of absence.

Trace only the needed path: entry → context/selection → parameters → preview → confirm/cancel → model change → views/properties → undo/redo/persistence. Use a minimal disposable project or authorized copy; protect unsaved work. Reprobe only a concrete unresolved question.

### 3. Implement one complete vertical slice

- Keep geometry/model operations outside the UI event layer where practical; derive plan and 3D from the same model state.
- Include tool entry, settings, interaction, validation/errors, undo/redo and persistence where applicable. Prefer one small complete workflow over disconnected controls or placeholders.
- Add focused regression cases for changed geometry and interaction, including explicit expected outcomes. Mark deferred parts in the contract/UI/docs.

### 4. Verify and package what the user will try

- Run checks appropriate to the change using repository scripts in `CONTRIBUTING.md` (normally formatting, `cargo check` and relevant tests). Do not claim checks that were not run or passed.
- For changed dialogs, drawing, shortcuts or selection, exercise egui event handling. For geometry changes, cover validation, persistence and undo semantics as applicable.
- After every source change intended for the user to try, build a fresh `dist/OpenArchi.app` with `./scripts/package-macos.sh` after source checks pass. A release binary or temporary bundle is not the deliverable.
- Verify freshness: `dwarfdump --uuid target/release/openarchi` must match `dwarfdump --uuid dist/OpenArchi.app/Contents/MacOS/openarchi`; run `codesign --verify --deep --strict dist/OpenArchi.app` when available. Ad-hoc signing changes bytes, so do not use raw executable comparison as a freshness test.
- Launch the packaged app and perform a native smoke test for substantial UI/GPU changes when the environment permits. If blocked, state the exact blocker and mark desktop UI as unverified. Compilation, RawInput tests and screenshots of an older build are not manual desktop verification.
- For macOS icon changes, follow [the app icon packaging and Dock verification guide](docs/development/macos-app-icon.md). Finder showing the new `.icns` does not prove the running Dock icon is correct; eframe needs the same source image configured through `ViewportBuilder::with_icon`, and Dock must be checked after launching one fresh packaged instance.
- Report the exact app path, UUID/signature result, checks actually run, test counts, manual steps performed and remaining unknowns. If packaging fails, mark the bundle unverified; never imply an older app is current.

#### UI language and localization checks

Any change that adds or edits user-visible text, menus, tooltips, status messages,
counts, dialogs, Quick Options, or the language system is not complete until both
English and Simplified Chinese have been exercised on the final packaged app.

- Treat a language dropdown or a partially translated window as incomplete. Every
  visible main-window surface must follow the active language: title bar, top
  menus, toolbar, Info Box, toolbox, view tabs, counts, status bar, Quick Options
  bar and palette, and every dialog or panel touched by the change.
- Verify both entry points independently: the title-bar selector and
  `View > Language`. Both must apply immediately and save the same preference.
- Switch English → 简体中文 → English on one fresh `dist/OpenArchi.app` instance.
  Check the actual accessibility tree or screenshots for representative labels,
  not just the selector text.
- Restart the app after choosing 简体中文 and confirm the first frame is Chinese.
  Restore the user's previous preference when finished.
- Confirm switching changes presentation only: project elements, selection,
  dirty state, undo/redo history and `Undo`/`Redo` availability must not change.
- Add or update both `locales/en.json` and `locales/zh-CN.json` without duplicate
  keys, and add focused regressions for changed labels, dynamic status messages,
  counts, and Quick Options or dialog text. Run `./scripts/cargo.sh test
  --all-targets` and `python3 scripts/ac29-workflow.py verify --force`.
- Record the entry points, surfaces checked, restart result, test counts, final
  bundle UUID/signature and any unverified dialogs in
  `docs/features/ui-language.md` and `docs/validation.md`. If a surface was not
  checked in both languages, mark it unknown instead of claiming full localization.

### 5. Close the loop

- Update the focused feature note and relevant `docs/validation.md`, README usage/limitations or research entry in the same change. Keep feature claims within the evidence and verification performed.
- Review `git diff` and `git status`; preserve unrelated work. Do not commit, tag, publish, push or release unless explicitly asked.
- For comparisons, define a finite checklist with matching start model, units, settings, actions and tolerances. Record each claim as `pass`, `fail` or `unknown` with both sides' evidence. REA verification passes only the declared claims and supported evidence dimensions; it is not global equivalence. When using reconstruction proof tooling, supply the current schema's implementation/revision bindings and individual claim outcomes; an array of Evidence IDs alone does not establish verified or ready status.
- End every task with an experience-distillation pass (mandated 2026-10-24): while developing, not only at the end, collect candidate reusable lessons — egui/layout pitfalls, tooling and desktop-input behavior, Archicad operation logic — and propose them as ready-to-paste bullets for the developer to confirm. Persist only confirmed items into the reusable sections of this file; never rewrite this file silently.

## Reusable findings: wall joins, grid defaults, attribute dialogs, desktop checks

### Archicad module effort assessment

- Estimate every module in two parts: UI/workflow observation and reconstruction, then model semantics and end-to-end behavior. REA can sharply reduce discovery cost for entry points, visible states, command paths and bounded runtime behavior; it does not supply the geometry/model engine or prove unobserved behavior.
- A module is not complete when its menu, dialog or parameter labels are present. Include validation, preview/commit/cancel, derived plan and 3D output, selection/editing, undo/redo, save/reopen and interactions with related elements as applicable.
- Rate cross-element relationships, consistency across views/edit paths, file compatibility, collaboration and large-project performance as separate complexity drivers. Do not infer implementation cost from menu count or UI coverage.
- Treat PLN/GSM compatibility and Teamwork as independent long-term workstreams. Static REA traces and raw binary loading do not establish document-format semantics or collaboration behavior.
- State the product target before estimating: a useful single-user native BIM subset with its own format is a different scope from full Archicad equivalence across tools, document workflows, libraries and collaboration.

### 共享编辑、小面板与选择反馈（2026-10-30 本轮用户确认）

用户本轮明确要求将可复用规则写入 AGENTS.md，授权沉淀以下已实施、已验证的 OpenArchi 规则。它们不构成 Archicad 29 全局等价证据；对象、视图及原生验收边界仍以 [墙体小面板契约](docs/features/wall-pet-palette/contract.md)、[多段线编辑](docs/features/polyline-editing.md) 和 [墙体选择反馈](docs/features/wall-selection.md) 为准。较早记录中的“悬停后返回画布自动激活指令”已被用户修正，后续不得恢复。

- **共享编辑会话，不另造相似流程。** 墙体与多段线共用小面板、显式指令切换、跟踪器、分步输入、候选预览、确认/取消与模型提交入口；对象差异通过有类型的几何适配处理。复用现有路径前查清全部入口与旁路，迁移后同时回归原调用者和新对象，不保留第二套实际生效的旧面板/拖动逻辑。
- **鼠标切换必须左键点击，悬停只负责外观。** 只有完成图标左键点击才激活另一指令；按下尚未释放、划过其他图标、穿过面板间隙、返回画布或按住拖动经过图标均不得隐式切换。激活后保持当前指令，直到下一次显式选择；已定义的键盘命令切换仍属于显式输入。当前、悬停、普通三态分开绘制，不能以悬停高亮代替指令激活。
- **保持草稿与参数稳定。** 悬停其他图标以及再次点击当前图标，都不能重置已接受的旋转/镜像步骤、数值锁或复制参数。指令切换本身不修改模型、不新增撤销记录；面板/数值字段上的释放不能提交画布编辑。兼容参数如何保留、显式换指令如何清理，以对应操作契约为准，不能从悬停处理器触发重置。
- **同类入口复用同一个面板组件，保留命中上下文。** 当前墙线端点与增加节点入口使用相同的两排十图标面板及本地化提示/可访问名称，不再各写文字菜单。首项按上下文表示：边线为增加节点，端点/接合点为移动节点。切到其他指令再切回时，必须保留原对象、端点/边索引、锚点和适用的高程，回到原有端点移动、验证、吸附和撤销路径；不能把端点当成内部插点位置，或为复用菜单重写几何。不要把墙厚、梯形外侧、墙连接等墙专属语义自动套到二维多段线。
- **节点优先级按整个候选集合处理。** 在已满足视图、楼层、可见性与选择前提的集合中，先找节点，再找边线，分别按已定义的距离/优先级选择。逐对象执行“先节点再边”仍可能让前一个对象的边抢占后一个对象的节点；多段线已为此补过重叠场景回归。应用到其他对象前仍须核对其命中契约，不凭同名控件推广。
- **区分选择注释、真实尺寸与操作数值。** 选中墙线时不显示被动的墙长标签，端点面板也不重复显示静态长度；保留参考线/节点、实际尺寸标注、属性数值和必要的编辑跟踪器。验证移除标签时按所属区域、位置或绘制上下文断言，不能全局禁止某个数值字符串：实际尺寸可能合法地显示完全相同的数值。
- **回归完整事件序列与来回切换。** 至少覆盖默认入口 → 悬停多个图标 → 返回画布（指令不变）→ 左键点击另一图标 → 再悬停/返回（仍锁定）、按下未释放、按住拖动穿过面板、数值编辑、分步操作、无效输入、取消和撤销/重做。统一端点菜单时额外覆盖两端点、切换后回到移动节点、另一端不动及原有吸附；不能只赋值枚举来证明真实 UI 入口正确。涉及的对象和中英文分别验证；未实测视图不宣称已支持。

当前复用入口：`src/app/wall_node_ui.rs::{wall_insert_canvas,wall_pet_buttons,edit_pet_buttons,start_wall_pet}`、`src/app/plan.rs::{wall_edit_palette,selected_polyline_hit,apply_wall_drag_preview}`、`src/polyline_edit.rs` 与 `ReshapeCommand::SelectionEdit/Polyline`。符号随实现演进，使用前先搜索；这是 OpenArchi 源码导航，不是 Archicad 行为证据。

### Active-command construction-method transitions

Confirmed for reuse by the user's follow-up on 2026-10-29; evidence and limits: [wall method switching](docs/features/wall-method-switch.md).

- Treat changing a construction method during an active command as a draft-state transition. Do not call the command's cancel/reset path merely because a method button changed. Preserve the anchor and accepted nodes; define how the retained prefix participates in the new method before implementing the switch.
- Keep switching separate from committing: switching changes pending input/preview without creating model elements or an undo entry. For the current wall workflow, completion commits the whole result once and Escape discards the whole draft. Do not silently drop accepted nodes when moving from a multi-node method to a two-point method.
- Centralize the transition in the editor and route Info Box/settings defaults through it. Reset incompatible numeric tracker locks when parameter meaning changes (for example length/angle versus rectangle ΔX/ΔY). Update both preview and final geometry for retained nodes.
- Regress the real UI entry path as well as geometry: start input, click the actual method button through full-workspace egui events, continue and finish; assert retained nodes, exact preview/committed endpoints, no premature model/undo changes, cancel, and undo/redo. Directly assigning a method enum cannot catch a UI handler that still cancels the draft.
- These are OpenArchi workflow lessons, not proof of Archicad equivalence. The retained-prefix + rectangle rule is an explicit product choice; exact AC29 mixed-method behavior remains unverified. Apply the state-transition checklist to other tools only after defining their own semantics.

### Wall join geometry

- Derive plan and 3D from the same reference-line/footprint join pipeline. Do not store trim polygons or per-view join results.
- Resolve the exposed plan outline once for the whole visible story footprint set, then bucket fragments by pen color and width. Do not resolve outlines per material or renovation fill batch: that reintroduces a buried end stroke when same-material walls differ only by renovation status.
- Same-material union acceptance: the through wall's fill is continuous across the junction; its top edge is interrupted only across the branch body; the branch side outlines stop at the through wall face. Keep regressions `same_material_wall_join_has_no_internal_plan_outline_across_renovation_groups` and the geometry join tests together.
- Fills remain presentation batches. The slight same-color bleed used to seal sub-pixel seams is not an outline and must not be replaced with a contrasting pen.

### Grid defaults

- `GridSettings::default()` keeps the construction grid hidden, grid snapping disabled and `GridSnapMode::None`. Any test or demo that needs snapping must enable it explicitly.
- When changing a default, search tests and docs for implicit dependence (`src/grids_ui.rs`, `src/app/ui_tests.rs`, display-density checks) before rerunning the full suite.

### AC29 attribute dialogs and layer semantics

- AC29 attribute dialogs share one anatomy — reproduce it for future Pen Sets, Line Types, Building Materials, Surfaces: left combinations list (clicking a combination previews its snapshot, `Update` writes the current state back, `New…` creates, a check mark marks the effective combination) + right search / discipline tree / status table + OK/Cancel footer. A combination is a named snapshot of the full attribute state; OK commits the working copy as-is after any preview clicks, without re-applying.
- Layer semantics quick reference (full evidence in `docs/features/layer-settings/contract.md`): the Archicad Layer is the protected catch-all (cannot be deleted, hidden or locked; elements on deleted layers move to it); locked layers forbid editing, deleting and placing elements; hidden layers are not rendered but still participate in wall intersection cleaning; walls clean up only within the same non-zero intersection group (`0` never joins).
- For attribute-behavior questions, consult the AC29 INT help `025_Attributes-*` pages first (Layers entry points `-4`, layer status `-5`, combinations `-6`, intersection groups `-8`). Prefer official help over runtime probing for interaction semantics; probe the target only for what help cannot answer.

### egui modal dialog layout

- A `Modal` content ui has no intrinsic height and a screen-wide max_rect: clamp it with `ui.set_min_width`/`set_max_width` at the root, and give right-aligned footers an explicit `ui.set_width` before `with_layout(right_to_left)`, or buttons land outside the frame.
- `ui.horizontal` clamps its row to the current line height (about 26 pt), so ScrollAreas inside it collapse to a few rows. Reserve the dialog body with `allocate_ui(fixed size)` and build each column via `ui.scope_builder(UiBuilder::new().max_rect(side_rect))` with pinned width/height. Reference implementation: `src/app/layers_ui.rs` (layer settings dialog).
- To diagnose suspected layout collapse, add temporary `#[cfg(test)] eprintln!` rect probes and run a headless ui_test printing actual viewport rects; do not guess from egui sources. Remove the probes before delivery.
- For dialog UI tests, follow the `layers_frame_output`/`layers_click` helper pattern in `src/app/ui_tests.rs`; never call the dialog render function inside the test helper — `workspace()` already renders it and double-rendering clashes widget IDs.

### Desktop verification on macOS

- Use an isolated HOME and launch the packaged app with `open -n --env HOME=<dir> -a dist/OpenArchi.app`; confirm the running process actually received it with `ps eww -p <pid>`.
- 多实例验收须用本次实际发现的 PID/窗口绑定输入和截图，并核对隔离 HOME 与当前包的构建凭据；按 bundle ID 查找可能选中用户仍在运行的旧实例。不得复用历史 PID、关闭用户实例或在其未保存项目上试验；只清理自己创建的测试实例。
- 每次核对窗口逻辑点尺寸、截图像素尺寸和显示比例，不默认一定是 2×；本轮先后出现过 1× 与 2×。键盘/修饰键输入需核对前台与释放状态；事件已发出或 AXPress 返回成功不等于应用已执行，必须读取后续状态。选择/构件计数不变只证明这些可见字段，不证明整个模型、脏状态或历史均未变化。
- Recovery is offered only when the recovery file parses as a complete project. An invalid file can be cleared without a visible error because the dialog returns early when `recovery_prompt` is `None`; base test projects on a known-good save and confirm the file still exists after launch.
- CUA coordinate clicks can move the cursor but fail to activate custom egui modals or menus. For reliable native clicks, read the real window bounds with `CGWindowListCopyWindowInfo` and post CoreGraphics mouse down/up at screen points. Screenshot pixels are device pixels (often 2x), while CGEvent coordinates are points.
- CUA hotkeys need `delivery_mode: foreground` for custom egui UIs, and may still require one `bring_to_front` + retry. Confirm a dialog opened with an AX query on its (localized) labels rather than a screenshot.
- Prefer the AX tree over image analysis for anything the tree can answer: screenshot analyzers misread selection highlights and floating scrollbars as occlusion defects and misplace check marks (observed twice in one task). Use image analysis only as visual backup.
- The language preference is stored at `$HOME/Library/Application Support/OpenArchi/language` (no extension, contents `en` or `zh-CN`). Pre-writing it inside an isolated HOME controls the first-frame language; the localized window title is the objective first-frame proof.
- For a T-junction plan check, measure pixels: the through-wall fill row stays one color across the junction, the top-edge dark run stops at the branch body, and both branch side outlines stop at the through-wall face. Record the screenshot path and the sampled rows/columns.
- `python3 scripts/ac29-workflow.py verify` reuses a successful run when only docs changed. `--force` rebuilds, and ad-hoc signing changes the UUID and hashes, so update `docs/validation.md` with the new values after any forced run.

## REA investigation notes

- Reuse an existing target/session and matching Evidence; bind or switch targets explicitly only when the selected operation requires it. Record target identity/hash, provider when applicable, Evidence IDs, coverage and limitations. Export Evidence needed after session close. Close sessions opened for this investigation using their matching lifecycle tools; preserve unrelated user sessions and report unresolved cleanup rather than assuming cancellation released resources.
- Start from one concrete seed (tool/menu command, control, gesture, error, symbol or string). Use default summaries and retained Evidence when advertised. In REA 6.8, MCP `analyze_function` defaults to a summary: use its `parent_evidence_id` for focused `inspect_analysis_view` facets or a `retained-evidence` comparison reference; request `detail: "complete"` only when a consumer requires the inline dossier. A summary or truncated client preview is not complete Evidence. Read selected pages or export instead of repeating the producer; page exhaustion does not establish source completeness. Follow actual connected schemas rather than assuming the CLI and MCP defaults match.
- Follow the installed connection/recovery guide only for an observed blocker. Distinguish client approval rejection, host timeout, server/provider timeout, connection loss and incomplete cleanup; a host timeout does not prove the operation stopped. Do not repeat an unchanged failed action or bypass client policy through another route. Inspect retained results or lifecycle state where useful, apply only the guide's applicable safe retry/remediation, and resolve cleanup before starting dependent work. Scope setup/configuration changes to existing authorization with a dry-run plan and backups; do not reinstall providers to solve a client-policy rejection.
- Static traces, decompiled pseudocode and inferred call edges are analysis evidence, not recovered original source or proof of runtime causality. Keep unresolved/indirect paths explicit. Record unknowns with the authority and probe needed to resolve them.
- For bounded runtime captures, declare the exact target, minimal scenario, steps, duration/stop condition, expected file effects and cleanup. Record before/after state; probe only relevant positive, invalid, cancel, undo/redo and save/reopen cases. Distinguish UI captures, input actions, process events, logs and measured model results.
- Use native static/artifact analysis only if it answers a remaining feature question. Do not inspect unrelated plugins or broaden target access by analogy.
- Never send synthetic pixel clicks to Archicad's native modals or dialogs. During the 2026-10 layer-settings investigation a background pixel click on the dialog's eye icon was followed by the target's FATAL exit (causality unproven; app was relaunched immediately). Drive native UI through AX actions (`press`/AXPress); if a modal/dialog has no usable AX action, record the blocker and request manual operation rather than pixel-clicking it. Pixel gestures are a last resort only on non-modal surfaces with no AX action, within the declared scenario. The CoreGraphics fallback described for OpenArchi's custom egui UI is not permission to use it on Archicad modals.

## Security and network work

- OpenArchi is a native desktop prototype. Do not invoke PentAGI for offline-only work.
- For a new network service, public endpoint or security-sensitive form, define the authorized target/scope and run suitable checks only on that target. Use PentAGI when available and appropriate; otherwise report its absence and the local checks performed without implying a penetration test.

## Reusable one-request brief

The user may give only the first line; fill known context from the conversation and workspace. Resolve unknowns from matching evidence first; ask the maintainer about any unresolved operation semantics before changing dependent code, as required by step 0.

```text
Goal / exact Archicad command or gesture:
Target version/build and authorized scope:
Initial view, selection, model, units and settings:
Expected pointer/keyboard sequence and visible feedback:
Reused operations / implementation / tests / object-specific differences:
Mouse micro-interactions / cursor and hotspot / capture limitations:
Icon and panel design / accessibility / localization acceptance:
Confirm/cancel, undo/redo and persistence expectations:
Existing evidence and unknowns:
Pre-code Archicad behavior check / non-behavioral scope and reason:
Maintainer confirmation / approved differences / pending questions:
Acceptance checklist / tolerances:
OpenArchi implementation and reversible implementation assumptions:
Checks / packaged app path and freshness / remaining gaps:
UI language: both entry points, main-window coverage, restart persistence, no model/undo change
```


### 操作研究与墙编辑复盘（2026-10-29）

本节由用户本轮明确要求“沉淀开发技能、真实判断、教训与应记住规则”授权加入；具体案例和证据边界见 [操作逻辑研究](skills/openarchi-ac29/references/operation-research.md)，实现技巧见 [实现参考](skills/openarchi-ac29/references/implementation.md)。这不把此前未测的 Archicad 假设转为已确认事实。

- 每个行为主张分别标记用户要求、对应版本文档、运行观测、静态分析或推断。OpenArchi 测试只能证明自身；旧版说明与新版相同图标不能代替新版行为实测。
- 将入口、选择/对象、按下/移动/释放、数值确认、取消及模型结果作为状态转换研究。图标身份、参数语义与完整操作步骤分开验证；最新用户修正覆盖旧产品要求，并明确标记旧记录失效。
- 几何研究先固定最小场景、单位、参考线、材料和连接设置，保留API请求/结果与截图；多显示图元先比较union覆盖和暴露边。面积相等不足以证明无尖片/缺口；用边界坐标及正反覆盖点补验。
- 复刻目标可能带有顺序依赖或不理想结果。观测和需求冲突时，明确说明证据、产品选择及差异，不将改进算法宣称为恢复原算法或全局等价。
- 共享几何表示改变时审计所有消费者及专用旁路，尤其选中高亮、私有预览、Trace和选中3D；以新路径回归证明接入。锁屏/前台输入受阻时，保留自动验证结果但将原生与双语操作记为未验证。

### 跨平台与 AI Agent 建模评估

- 评估跨平台支持时，分别检查架构可移植性、窗口后端配置、系统集成与目标机器实测；平台标签或条件编译分支不能代替编译和运行证据。
- 设计 Agent 建模能力时，让人工与 Agent 共用有类型、可验证、可检查且可撤销的模型命令；区分架构潜力与已实现、已验证的能力。
