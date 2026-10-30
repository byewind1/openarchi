# 验证记录

## 2026-10-26 AI 原生工作台架构研究（仅文档与只读探测）

- 报告：[架构评估](features/ai-native-workbench/assessment-2026-10-26.md)；证据：[源码基线](features/ai-native-workbench/2026-10-26-source-baseline.json)、[API 探测](features/ai-native-workbench/2026-10-26-api-probe.json)。状态为提案，未实施、未核准实施。
- `python3 scripts/ac29-workflow.py context architecture` 复用安装身份与历史研究；本轮检查工作树中的模型、命令、编辑器、几何、渲染、存储与本地助手源码，并记录 SHA-256。
- Archicad 连接器 `list_instances` 返回 AC29 build 5101、Tapir 1.7.1；只读 `API.IsAddOnCommandAvailable` 确认 14 个准确命令名已注册。`Publish` 返回 false 是名称不匹配，正确名 `PublishPublisherSet` 返回 true。不代表参数覆盖、建模、撤销或保存重开已验证。
- 未执行 Archicad 写命令，未读出项目构件或上传工程数据；未改应用源码、未运行 Rust 测试、未打包、未做桌面交互测试。现有 `dist/OpenArchi.app` 未在本轮重新核对 UUID/签名，不作为本轮交付物。
- 研究交付物同步至用户指定 Obsidian 目录；检查研究 JSON 可解析、所记录源码哈希仍匹配、报告相对链接存在、两份报告正文在链接替换后对应。仅检查本次研究文档差异，不将已有工作区改动归入本任务。
- 经验提炼候选保存在报告，待开发者确认；没有修改 AGENTS.md。

## 2026-10-25 去除重复的应用内标题行

按用户反馈，移除窗口内容区重复显示文件名、版本号和语言下拉框的整行。系统窗口标题栏继续显示文件名；语言切换保留在 `View > Language`。变更影响 `src/app/shell.rs`、`src/app/mod.rs` 和对应 UI 回归，不改项目数据或撤销历史。

### 自动验证

- 新增 `workspace_does_not_repeat_the_native_window_title_or_language_picker`，断言窗口内容不渲染 `Untitled`、语言选择项或版本横幅；专项测试通过。
- `python3 scripts/ac29-workflow.py verify --force`：通过格式、`cargo check --locked --all-targets`、本地化、**307 项 Rust 测试**、**15 项工具测试**、`git diff --check` 和 macOS 打包。
- 报告：`target/ac29-workflow/runs/20261024T192956Z-5155078e/report.json`；输入 SHA-256 `e390e7bb596a40c3a37f132d8e41183df83d17c8f24827a39250f6e70434eb1f`。
- 最终包 `dist/OpenArchi.app`：arm64 UUID `611C0AFD-9095-3764-9351-34408CD8C373`，与 release 二进制一致；签名验证通过，release SHA-256 `f19aba2e339d34288d9aafdc55342988bf1e45da9354302e06c3d13a97e5b793`。

### 桌面实测

用 `open -n --env HOME=/tmp/oa-header-acceptance/home -a dist/OpenArchi.app` 启动最终包；`ps eww` 确认隔离 HOME。CUA 辅助功能树与截图均来自唯一剩余的最终包实例，显示标题栏为 `Untitled`，其下直接是标准工具栏；内容区没有重复文件名、版本横幅或语言下拉框。窗口内 Edit/Info Box/Toolbox、视图标签、Quick Options 栏和状态行均保留。

- 英文首帧：系统菜单、工具栏、Info Box、Toolbox、视图标签、12 个构件计数、状态和 Quick Options 栏为英文；选中数为 0。
- 通过 `View > Language > 简体中文` 即时切换后，窗口标题为“未命名”，系统菜单及上述可见主窗口文字切为中文；截图确认 Info Box、Toolbox、视图标签、底部 Quick Options 栏和状态行。构件数仍为 12、选中数仍为 0；Edit 菜单中 Undo/Redo 均禁用，语言切换未创建撤销记录。
- 再切回 English 后界面恢复。随后选择简体中文、退出并重启同一隔离 HOME，首帧显示中文菜单、工具箱、状态和“12 个构件”；最后恢复隔离 HOME 原先的英文偏好并退出。未修改真实用户偏好或项目模型。Quick Options 浮动调色板本次未打开；项目文件名在真实打开操作后的标题效果仍按前一节记为 unknown。
- 依用户要求，应用内标题行的语言选择器已移除；因此本次只验证保留的 View 菜单入口，没有可单独测试的标题行入口。该移除是有意的产品调整。

未提交、未推送。

## 2026-10-25 原生菜单栏与文档标题验收

目标是将 macOS 菜单从窗口内移到系统菜单栏，并让窗口标题显示文档文件名。行为契约与逐项结论见[原生菜单栏与文档标题](features/native-menu-title/contract.md)。本次源码更改还包含多个既有功能；验证覆盖当前整个工作区，不将通过等同为每项功能单独的桌面验收。

### 自动验证

- `python3 scripts/ac29-workflow.py verify --force`：通过格式检查（含 4 个 `include!` UI 文件）、`cargo check --locked --all-targets`、本地化、**306 项 Rust 测试**、**15 项工具测试**、`git diff --check` 和 macOS 打包。
- 验证报告：`target/ac29-workflow/runs/20261024T191606Z-eeca77fa/report.json`；输入 SHA-256 `e4cbfb672f379998be7b9546fb60653e46f8a285409b24d8318fccc129eacb98`。
- 最终包：`dist/OpenArchi.app`；arm64 UUID `13A38A8A-AD0F-3BE8-9192-134F3E7CB206`，与 `target/release/openarchi` 一致；`codesign --verify --deep --strict` 通过。release SHA-256 `e85179570052f6fbcd87e825a602faaf44bd808aab94a1bed583c537f895ff35`。

### 桌面实测

通过 `open -n --env HOME=/tmp/oa-menu-acceptance/home -a dist/OpenArchi.app` 启动隔离实例；`ps eww` 字段筛选确认 PID 52110 的 `HOME=/tmp/oa-menu-acceptance/home`。CUA 按精确包路径绑定 OpenArchi。窗口标题为“未命名”，主窗口中仍有 Courtyard House 演示模型（12 个构件）。测试未保存或改写该模型。

- **pass：系统菜单位置与菜单组。** 辅助功能树明确显示 macOS `menu bar`，其中依次有 `OpenArchi、文件、编辑、视图、设计、文档、选项、协作、窗、帮助`。窗口内不再出现菜单/命令条；标准工具栏、视图栏、Quick Options、Info Box 仍在窗口内。
- **pass：菜单随语言即时变化。** `视图 > 语言 > English` 后，窗口标题显示 `Untitled`，主界面状态及系统菜单变为英文；再选择 `简体中文` 后恢复 `未命名`、中文主界面与中文系统菜单。测试后已恢复中文。没有重启应用验证偏好持久化。
- **pass：原生菜单命令到达应用。** `文件 > 打开 JSON…` 显示与原窗口相同的应用内“打开项目”对话框。对话框默认路径 `/tmp/oa-menu-test-home/Documents/courtyard.openarchi.json` 不存在，点击“打开 / 替换”显示 `No such file or directory (os error 2)`；随后取消对话框。故打开文件后的标题名、重启持久化、Undo/Redo、退出守卫和文本框快捷键均未验证，见契约中的 unknown 项。
- 测试没有打开或操作 Archicad，也没有执行写入操作。

桌面结果只覆盖上述可见项；原生菜单栏已实测通过，文档标题完整生命周期仍需有效文件路径后复验。未提交、未推送。

## 2026-10-24 参数传输（吸管/注射器）实现与验证

实现 AC29 风格参数传输：`⌥+点击` 吸取、`⌘⌥+点击` 注入（平面与 3D）、`⌥C`/`⌘⌥C` 命令、无选中时一次性光标模式（Esc 取消）、Element Transfer Settings 对话框（传输集增删改、按类型分组复选框、随工程保存并产生一条撤销）。行为契约、证据与边界见[参数传输](features/parameter-transfer.md)。

### 自动验证

- `./scripts/cargo.sh fmt --all -- --check`：通过；`cargo check --all-targets`：0 警告（清理了 `transfer.rs` 测试中一处被遮蔽的未用绑定）。
- `./scripts/cargo.sh test --all-targets`：**274 项通过**（226 库、7 Guides、10 门窗、31 工作流）。参数传输专项 **14 项**：`src/transfer.rs` 9 项单元测试（同类型几何/材质传输且位置不变、跨类型拒绝、组取消勾选保留字段、门窗保宿主与 offset、楼梯保起终点、传输集切换、按类型缓冲、吸取默认值跳过绘制方法、墙选择值随组传输）+ `src/app/ui_tests.rs` 5 项 egui 回归（`⌥C` 选中吸取并更新工具默认值、无选中 `⌘⌥C` 进入一次性模式、平面 `⌥+点击` 手势、跨类型与非法几何注入整体失败、传输设置对话框 OK 为一条撤销记录）。
- `python3 scripts/check-localization.py`：472 键，en/zh 缺失 0、重复 0、zh 未翻译 0。
- `python3 scripts/ac29-workflow.py verify --force`：通过格式、检查、localization、274 项 Rust 测试、15 项工具测试、`git diff --check` 和 macOS 打包，报告 `target/ac29-workflow/runs/20261023T200806Z-309aae34/report.json`。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `AD4B9F6E-6016-3CDA-8BCD-D0C3F07625E7`，release SHA-256 `41092a55ed85aabf0693ec81cd049859fc1b034c2de45fdc3f7c44e42fae6852`，`codesign --verify --deep --strict` 通过，`dwarfdump --uuid` 两处一致。

### 桌面实测（用户授予屏幕录制权限后完成）

最终包 `dist/OpenArchi.app`，隔离 HOME `/tmp/oa-transfer-smoke/home`（`ps eww` 确认注入），演示工程 Courtyard House 12 构件，cua-driver（用户已授权 Accessibility+Screen Recording 的 0.21.0 daemon）窗口截图 + 像素点击/键盘，后台 shell `screencapture -l <window>` 辅助取样：

- 英文界面：工具栏出现吸管/注射器图标按钮；选中构件后 `⌥C`（前台 CGEvent 和弦）状态栏显示 `Picked up Slab parameters from Slab 001`（吸取进入每类型缓冲并写入工具默认值，无撤销记录、无 dirty）；选中阳台楼板（Timber、厚 0.16）后点工具栏注射按钮，状态 `Injected parameters into 1 element(s)`，注入后未选中阳台的平面填充 (237,238,230) 与主楼板完全一致（Concrete、0.28 生效）；工具栏撤销恢复木色 (233,219,193)、重做再次一致——整次注入为一条可逆记录；Esc 清选正常，全程构件数 12。
- Edit 菜单出现 `Pick Up Parameters ⌥C`、`Inject Parameters ⌘⌥C`、`Element Transfer Settings…`；对话框呈现传输集列表（All Parameters 高亮）、New/Duplicate/Delete（单集时 Delete 禁用）、名称框与按类型分组复选框（Wall 五组、Slab/Stair/Door/Window 各两组、Polyline 一组，默认全选），Cancel 关闭无变更。期间 Info Box 输入非法厚度顺带触发 `Wall thickness must be between 1 mm and 5 m` 校验拒绝（既有信息框路径）。
- 中文：标题栏选择器切到简体中文后全界面即时切换（标题 `建筑设计工作台`、菜单/工具箱/状态栏中文）；编辑菜单显示 `吸取参数 ⌥C`、`注入参数 ⌘⌥C`、`元素传输设置…`；对话框为 `传输集/新建/复制/删除/名称/传输的参数/几何/材质与图层/表面/平面显示/数据/确定/取消`（传输集名 `All Parameters` 为用户数据按原样显示）；中文保存守卫 `未保存的更改/放弃并关闭` 出现并可用。选简体中文退出重启后**首帧即中文**（标题、菜单、工具箱、状态 `就绪 选择构件或开始绘制`、`12 个构件`），隔离 HOME 偏好文件持久化为 `zh-CN`；随后切回 English（即时生效）并退出，不影响用户真实偏好。语言切换前后构件数与撤销栈不变。

### 桌面未验证项与限制

- 含 Cmd 的组合键（`⌘⌥C`）与带修饰键的指针点击（`⌥+点击`/`⌘⌥+点击` 手势）未能经桌面自动化送达：本机系统级输入拦截吞掉 Cmd 和弦（前台与后台 CGEvent、CLI 全局投递均失败，和弦曾被前台应用接收），修饰键不随投递的鼠标事件生效（应用收到普通点击）。`⌥C` 与单键可达。这两条路径由无头 RawInput 回归覆盖（`cmd_option_c_without_selection_arms_one_shot_inject`、`option_click_gesture_picks_up_in_plan`）；桌面键鼠实测以工具栏按钮、菜单项与 `⌥C` 为准。
- 3D 视图手势未在桌面重复（与平面共用 `execute_transfer` 代码路径）；传输集 XML 导入/导出未实现（契约记录为延后）。
- 观察项（非本特性缺陷）：Info Box 的墙厚度校验错误文本在 Esc 清选后仍残留显示，待后续信息框状态清理。

测试后已清理：OpenArchi 测试实例退出、Terminal 转发进程停止、FIFO 移除，未触碰用户 Archicad 与既有 REA MCP 进程；未提交、未推送。

### 2026-10-24 补充：手势语义核对与 `⌘⌥+点击` 正向回归

用户核对快捷方式后确认语义为：`Alt(⌥)+左键点击` 吸取、`Cmd+Alt(⌘⌥)+点击` 注入目标构件。复核源码（`src/app/plan.rs`、`src/app/model_view.rs` 拦截块，`src/app/mod.rs:1674-1678` 命令键）与既有实现一致，无需行为改动；`⌥C`/`⌘⌥C` 是工具栏与 Edit 菜单命令的附加快捷键（依据用户提供的 AC29 工具栏提示 "Pick Up Parameters (Option+C)"），不是对手势的替代。为此新增正向回归 `cmd_option_click_gesture_injects_into_target_element`（吸取后 `⌘⌥+点击` 目标墙 → 厚/高/材质注入、参考线不动、一条撤销），全量测试重新通过。该改动仅涉及 `#[cfg(test)]` 代码，当时的发布包（UUID `AD4B9F6E-6016-3CDA-8BCD-D0C3F07625E7`）不受影响。

### 2026-10-24 补充：悬停光标与构件亮显（AC29 悬停反馈链）

用户描述 AC29 的完整交互语义后补齐了悬停反馈：按住 `⌥`（吸取）或 `⌘⌥`（注入）且指针在平面/3D 画布上时，指针切换为自绘位图光标（吸管/注射器，`src/transfer_cursor.rs` 程序化绘制，经 `PlatformOutput::cursor_image` → egui-winit → NSCursor 送达；不支持的平台回退 Crosshair）；悬停到点击会生效的构件上时光标出现液体且构件亮显（平面：白色浅罩+亮青描边；3D：`SceneRenderer::render` 新增 `hovered` 参数改用亮蓝基色）。注入仅在吸取缓冲已有该构件类型时 primed/亮显；指针离开画布、松开修饰键、平移/拖拽或 Space 时恢复常规光标；工具栏命令的一次性模式复用同一套光标。egui 命中测试使用上一轮 widget 矩形（一帧延迟），测试与实现均按此处理。

- `./scripts/cargo.sh fmt --all -- --check`：通过（rustfmt 已运行）。`./scripts/cargo.sh test --all-targets`：**279 项通过**（231 库、7 Guides、10 门窗、31 工作流）。新增 4 项回归：`transfer_cursor::cursor_bitmaps_are_distinct_and_well_formed`（四种位图稳定且互异）、`alt_hover_shows_eyedropper_cursor_and_highlights_element`（⌥ 悬停 primed/亮显、空白处空管、松键清除）、`cmd_alt_hover_primes_syringe_only_when_buffer_holds_type`（空缓冲不 primed、吸取后 primed+亮显）、`alt_hover_in_3d_shows_transfer_cursor_and_clears_without_modifier`（3D 位图与清除）。
- `python3 scripts/ac29-workflow.py verify`：通过格式、检查、localization、279 项 Rust 测试、15 项工具测试与 macOS 打包。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `99147C64-610C-3634-90DF-00089795F1E1`，release SHA-256 `ed0c668a9767e21b4ff1759f51d3fc0f86aa354e08e363584619572803ccd80d`，`codesign --verify --deep --strict` 通过（上一包 `AD4B9F6E-…` 已被本次替换）。
- 桌面烟雾（隔离 HOME `/tmp/oa-cursor-smoke/home`，`open -n` 启动 pid 18008）：2D 平面截图渲染完整；F3 切 3D 透视后 wgpu 渲染正常（墙/板/地面/轴标无回归）。测试实例已退出，临时文件已清理。
- **unknown**：修饰键按住期间的光标样式、液体状态与悬停亮显无法经本机自动化送达（系统输入拦截吞掉修饰键+指针组合，`screencapture` 也不含光标），真机视觉需用户手持验证；位图为 32×32，Retina 下观感（是否偏小）未知。此前记录的 `⌘⌥C` 和弦与 `⌥+点击` 桌面拦截限制不变。

### 2026-10-24 补充：吸取后激活对应工具

按用户描述补齐 AC29 吸取语义的最后一段：点击吸取构件属性后，自动 `set_tool` 激活对应构件的工具（墙→Wall、板→Slab、楼梯→Stair、多段线→Polyline、门→Door、窗→Window），带吸取到的默认值直接进入可绘制状态；工具属视图状态，不产生模型撤销记录，状态栏仍显示吸取消息。注入（`⌘⌥`）不切换工具。实现位于 `Editor::pick_up_from`（`src/editor.rs`），命令（`⌥C`/工具栏/菜单）与手势（`⌥+点击`）共用同一入口。

- `./scripts/cargo.sh test --all-targets`：**280 项通过**（232 库、7 Guides、10 门窗、31 工作流）。新增 `pick_up_activates_the_matching_tool`（吸取板→Slab 工具+板厚默认、吸取墙→Wall 工具+厚高默认、无撤销无 dirty），并加固两处既有断言：吸取后 `tool == Wall`；注入前切到 Select 后注入不改变工具。
- `python3 scripts/ac29-workflow.py verify`：通过格式、检查、localization、280 项 Rust 测试、15 项工具测试与 macOS 打包。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `2266DEFC-C31C-36C0-8015-845FE15E10D4`，release SHA-256 `03b8c93e69f2138ae545fee6308704ed1c501c3f72ee1d235fdfe82b09912a8d`，`codesign --verify --deep --strict` 通过（替换上一包 `99147C64-…`）。
- 桌面尝试（隔离 HOME，pid 98411）：应用启动并完整渲染（截图确认）；随后尝试经 cua-driver 前景点击与原始 CGEvent 点击画布以"选中墙 → `⌥C` 吸取 → 观察工具激活"，三次点击均未在画布生效（Info Box 保持 0 selected，状态栏不变）——与此前记录的 egui 画布输入投递限制一致；期间确认另一运行中的 OpenArchi 实例（用户开启，pid 96389，主屏）不受影响并保持未触碰。工具激活的桌面验证由无头测试支持，真机确认待用户以新包手持执行。测试实例已退出、临时文件已清理；未提交、未推送。

### 2026-10-24 补充：按住修饰键时独占指针反馈（隐藏捕捉标记）

按用户澄清：按住 `⌥`/`⌘⌥` 期间指针只允许吸管/注射器两种状态（空管 / 半管液体），原有的捕捉点/悬停示意标记（`CursorFeedback` 的箭头勾、三射线、捕捉目标样式，`src/app/plan.rs` 悬停反馈块）必须隐藏；松开修饰键后恢复捕捉交点/线中点的常规样式。实现为在 `plan()` 中以 `transfer_cursor_active` 门控整个悬停反馈块（含 `set_cursor_icon` 与 `paint`），传输光标激活时不运行；一次性武装模式同样独占。3D 视图无 painted 捕捉标记，无需处理。

- 新增回归 `alt_hover_suppresses_snap_feedback_until_released`（悬停墙端点：无修饰键时 `cursor_icon == Default` 即反馈块运行；按住 `⌥` 后为 `Crosshair` 回退 + 位图光标，反馈被抑制；松开后恢复 `Default`），`./scripts/cargo.sh test --all-targets` **281 项通过**（233 库、7 Guides、10 门窗、31 工作流）。
- `python3 scripts/ac29-workflow.py verify`：通过格式、检查、localization、281 项 Rust 测试、15 项工具测试与 macOS 打包。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `0FBA737F-507F-39ED-BC66-82B463B6883C`，release SHA-256 `8055bd45e805bce4ef09e783d579d1d26201e33433e6908ecc260b7c9c1d642b`，`codesign --verify --deep --strict` 通过（替换上一包 `2266DEFC-…`）。
- 桌面烟雾：新包（隔离 HOME，pid 26907）启动并完整渲染（截图确认）；修饰键按住期间的指针视觉仍无法经自动化送达（记录在案的限制），需用户手持验证。测试实例已退出，另一运行中的 OpenArchi 实例（pid 17479）未触碰；临时文件已清理；未提交、未推送。

### 2026-10-24 补充：光标改画布内绘制 + 绘制状态方块十字

用户真机验证失败。根因判断：首版的吸管/注射器经 `PlatformOutput::cursor_image` → egui-winit → NSCursor 位图送达，本机真机不可见（无头回归只覆盖 egui 输出层，NSCursor 上传链路无证据）。按本应用已验证的铅笔光标模式重做：**隐藏系统指针（`CursorIcon::None`）+ 每帧在指针位置绘制字形**（`src/transfer_cursor.rs` 重写为 `paint_pick_up`/`paint_inject`/`paint_square_crosshair`）。同时按用户澄清实现绘制状态光标：六个绘图工具（墙/板/楼梯/多段线/门/窗）激活时指针统一为吸附点上的**方块十字**（小方块+四向刻线，随端点/中点/自由位置移动），替代箭头/铅笔/手型组合；Select 箭头与 Marquee 十字不变。平面与 3D 同步；`apply_polyline_edit_cursor` 与 3D 悬停反馈块在传输模式激活时不再覆盖指针。

- `./scripts/cargo.sh test --all-targets`：**281 项通过**（233 库、7 Guides、10 门窗、31 工作流）。光标回归改为断言 `CursorIcon::None` + `transfer_cursor`（kind, primed）+ `transfer_hover` 状态；新增 `drawing_tool_shows_square_crosshair_cursor`（Wall 工具 None、Select 恢复 Default）；删除 NSCursor 位图单元测试。
- `python3 scripts/ac29-workflow.py verify`：通过格式、检查、localization、281 项 Rust 测试、15 项工具测试与 macOS 打包。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `759C40BE-D727-3E5F-8D98-8FA4DC3FAFD6`，release SHA-256 `031ec7e6bcb4f645a4611e7817b671dd762f21bfd7d20411b7eed7ef5efd7a13`，`codesign --verify --deep --strict` 通过（替换 `0FBA737F-…`）。
- 桌面视觉验证尝试（隔离 HOME）：启动后窗口仅背景色、内容不合成，点击激活后仍不渲染；用独立 bundle id（com.openarchi.smoke）的临时副本复现同样现象——**扩展了已记录的"第二实例窗口不合成内容"未知项：与 bundle id 无关，本环境同会话非首个实例均不渲染，首实例（如用户手开的）正常**。绘制字形真机视觉留待用户以新包手持验证；测试实例与临时副本已清理，用户实例（pid 27461）未触碰；未提交、未推送。

### 2026-10-24 补充：液体可见性与绘图十字简化

依据[Archicad 29 Parameter Transfer 官方帮助](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-98.htm)和[The Intelligent Cursor 官方帮助](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-39.htm)：吸管/注射器在有效悬停时有半满样式；绘图工具空白处使用简单十字，智能吸附位置再显示相应节点/边提示。修正画布字形绘制顺序：此前液体先画、随后被白色管身和深色内腔盖住；现改为管体完成后再绘制带暗边的青色液体。绘图光标移除方框与粗刻线，改为细十字，仍定位在吸附点。此项为 OpenArchi 原创绘制，不复制帮助图片资源。

- `python3 scripts/ac29-workflow.py verify --force`：格式、`cargo check --all-targets`、localization、**283 项 Rust 测试**（含新增液体绘制层级和简洁十字两项回归）、15 项工具测试、`git diff --check` 与 macOS 打包均通过。
- 新包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `975757F5-4C46-359E-906F-A621CA6AAA12`，`codesign --verify --deep --strict` 通过；release SHA-256 `782b03ac75bacc2099522cbff90d6e1581f29193c0b3d988c2156fe885ff592a`。GUI 未做本轮手持验证。
- 真机按住 Option 悬停构件后的半满显示、绘图十字观感仍需以最终包手持确认；代码/无头回归不能替代该视觉验收。

### 2026-10-24 补充：绘制中按住 Shift 锁定 X/Y 轴向

用户要求绘制时按住 Shift 后只能沿 X 或 Y 方向移动（AC29 正交手势）。实现于统一捕捉入口 `AtelierApp::target`（`src/guides_ui.rs`）：Shift 生效且下一个点未吸附到端点时，以活动绘制锚点（`editor.anchor`，即墙起点；无 anchor 时取多段线最后一个节点）为基准，按指针相对锚点的主方向把目标锁定到过锚点的 X 或 Y 轴上（主导轴取绝对偏移较大者）；锚点/端点吸附不受影响，避免 Shift 阻断闭合或吸附起点。墙工具原有的 Ortho/Shift 逻辑保留（`base_target` 内在捕捉前先做正交收缩），新增约束作用于最终目标，因此对墙、板、楼梯、多段线、门、窗及参考线绘制一致生效，且不会被后续捕捉带离轴。

- `./scripts/cargo.sh test --all-targets`：**284 项通过**（235 库、7 Guides、10 门窗、31 工作流）。新增回归 `shift_locks_drawing_to_dominant_axis_through_anchor`：主导横向锁定 Y、主导纵向锁定 X、无 Shift 时不约束、多段线末节点作锚点、精确端点吸附优先于轴锁。
- `python3 scripts/ac29-workflow.py verify`：通过格式、检查、localization、284 项 Rust 测试、15 项工具测试与 macOS 打包，报告 `target/ac29-workflow/runs/20261024T143827Z-74207094/report.json`。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `3C4F39A0-564B-3768-80B4-45763F232CD5`，release SHA-256 `8a88a9410d2743c53696ba3bfaa3fe2879e8dafb87e601e4810356f8846708ec`，签名校验通过。
- 桌面烟雾（隔离 HOME `/tmp/oa-shift-smoke/home`，`open -n` 启动 pid 83170，`ps eww` 确认注入）：新包启动并完整渲染 Courtyard House（截图确认，12 elements）。按住 Shift 的绘制手势属于修饰键+指针组合，无法经本机自动化送达（既有记录的限制）；轴锁语义由无头 `target()` 回归覆盖，真机确认待用户手持。测试实例已退出（`kill 83170` 仅杀本实例）；用户手开的实例（pid 62178）确认未被触碰后自行退出。临时目录已清理；未提交、未推送。
- **unknown**：Shift 轴锁与轨迹/捕捉提示（tracker）组合下的视觉观感、Shift 中途按下/松开时锁定轴的切换手感，需用户真机验证。

## 2026-10-20 PLN 与图库互操作方案（文档调查，待核准）

新增[架构及实施方案](features/pln-interop/plan.md)和[证据台账](features/pln-interop/evidence.md)。已只读核对当前源码/进展、`context pln` 目标身份及固定 AC29 SDK 官方文档。REA 绑定目标成功；Mach-O 完整结果超传输预算，导出及 plist 检查被客户端 `approval policy is never` 拒绝；Hopper 符号查询超时且清理未确认，详细资源/限制见台账。没有取得 PLN 解码或回存运行证据。

仅文档变更；未改应用源码、未运行应用测试/打包/桌面验证，不产生新的应用 UUID 或签名结论。历史测试与包记录不代表本轮重新验证。实施需用户核准。

### 2026-10-20 REA MCP 重新调查

`context pln` 复用本机 AC29 29.0.0 v1 INT (5101) / arm64 / SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。发现两份候选样本：

- `/Users/ren/MAC工作/工作/code/开源项目/su2bim/tests/test1.pln`：26,119,696 bytes；SHA-256 `73ec87bd2fc839d2a6b8502d3b28fc22720eb445b624ee432ce721f33a7dd57b`。
- `/Users/ren/MAC工作/工作/code/开源项目/_legacy_backup/obr存档/gdl 案例/bak/chair-gdl.pln`：18,212,624 bytes；SHA-256 `ab2b9d1b4f02dd692ed9a03fc497709f8a4d06a677ea20c4fbd2de4e9ed29ef1`。

当前 REA provider 为 Ghidra 12.1.4（Hopper launcher 缺失，IDA 未配置）。`open_binary(test1.pln, provider_id=ghidra)` 返回 `unsupported binary format`；打开 AC29 Mach-O 成功，但 `binary_overview`、2 个 strings 搜索、1 个 procedures regex 搜索长时间未返回，停止等待时没有 Evidence。`close_binary` 调用未及时返回；CLI 搜索结束后的最终 `binary_session` 显示 `open=false`、无 provider binding、无活动请求，确认本次 MCP Ghidra 会话已关闭。不得据此解释任何 PLN 字节布局。细节见 [REA 证据台账](features/pln-interop/evidence.md#2026-10-20-按当前-rea-mcp-链路重新调查-pln)。

本次未在 Archicad 打开或保存样本，未修改样本/应用源码，未运行测试或打包。结论：按当前 REA `open_binary` 路由不能直接分析 PLN；PLN 解析仍未完成。

后续沿既有 Ghidra CLI 配置尝试：REA 6.4.0 的 `inspect-artifact test1.pln` 也返回 `unsupported binary format`；`inspect-binary-layout` 命令 schema 仅接受 Linux ELF。当前 shell 不继承 Codex MCP 的 Ghidra 环境变量，显式传入已记录的 Ghidra/JDK/REA provider 路径后，AC29 静态搜索运行约 210 秒仍未返回，Ctrl+C 后 CLI 报 `cancelled` / `cleanup=complete`。无搜索结果、无 Evidence。详细边界见 [证据台账](features/pln-interop/evidence.md)。

## 2026-10-18 墙体参考线编辑、接头几何、连续墙结束与窗口标题（当前）

本节记录当前工作树（未提交）在 AC29 墙体参考线编辑、接头几何、连续墙结束手势和窗口标题本地化方面的结论，取代上方各节的测试数字作为当前结论；历史记录保留。

### AC29 静态依据

- 目标 `29.0.0 v1 INT (5101)`，arm64；主程序 SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`（`python3 scripts/ac29-workflow.py context walls`）。
- `VBElem::Wall::RectPoly` `0x84e1b4`、`GetCorePolygon` `0x84db0c`、`GetWOffsetBeg` `0x848b9c`；`VBD::WallVRDData::CalculateWPolygon` `0x34f06c`；`VBCalculations::MakeWall2PatStruct` `0x625bc`、`SetWall2PointAndInd` `0x369538`、`GetTind` `0x369a90`、`GetCrossInd` `0x369ef0`。
- Framework SHA-256：`VBElemOperations` `32c9d2c0014aca680d425aa31e17cdba84983fbaf287a9b15f7966f989ec304f`，`VBElements` `f86d144f5f04ce1c7a88a2df831c4b54459f3ad703be53768999a24aab70aae3`。
- 限制：均为静态反汇编阅读，无 REA Evidence ID；`GetTind`/`GetCrossInd` 的索引选择分支未逐行还原；本轮未捕获 AC29 运行时交互。

### 实现结论

- `src/geometry.rs` 的 `WallBody` 从参考线、厚度、参考线位置和数值偏移得到 `[lo, hi]` 本体区间；`plan_wall_footprints` 在显示时按相邻墙计算 L 角同侧斜接、偏移角远端面裁剪、T 接头近端面裁剪和四墙交汇，平面、3D、Trace 共用同一轮廓。模型中的 `start`/`end` 不被改写。
- `src/editor.rs` 的 `wall_close_target`/`click_wall`：Continuous 两节点点击起点只生成一段墙，三节点以上生成闭合段；Polygonal 至少三节点闭合。
- `src/app/plan.rs`：起点 12 px 磁性闭合（屏幕空间兜底）；右键 `Response::clicked_by(Secondary)` 完成 Continuous（≥2 节点）/Polygonal（≥3 节点），不足则取消；右拖仍平移。
- `src/model.rs` 的 `validate_wall_reference_line` 与 `src/editor.rs` 的 `set_wall_reference_line`：墙体编辑只替换参考线 `start`/`end`，墙身、接头、洞口和 3D 全部由模型派生；拖动预览从手势开始快照重算，松开只提交一个撤销事务。端点手柄优先于基线命中，拖基线平移整面墙。
- `src/app/plan.rs` 的 `wall_endpoint_drag_target`：端点捕获先读取原始指针世界坐标，再计算目标墙基线；网格或对象吸附只能约束沿被编辑墙方向的位置，不能把端点先拉离目标基线。候选墙以墙身包含/邻近优先，其次才按基线距离判定。
- `src/app/plan.rs` 的墙体编辑面板记录自身矩形，画布在其下方不再触发选择或拖动；`Done` 关闭未改动的面板并恢复 Select 提示，`Escape` 取消实时拖动且不写撤销记录。
- `src/localization.rs`、`src/main.rs`、`src/app/mod.rs`、`src/app/shell.rs`：窗口标题随语言即时更新；macOS 应用菜单标签固定为 `OpenArchi`（eframe 从启动时的 viewport title 派生，因此启动标题保持中性，首帧设置本地化窗口标题）。
- `src/view_settings.rs`：未选中墙的平面切面填充改为固定浅灰 `RGB 211, 211, 211`（`WALL_CUT_FILL`），不再按材质压暗；默认 `03 建筑 100` 笔集不叠加色调，选择高亮、覆盖样式和改建状态色仍按原规则覆盖。

### 自动检查

- `./scripts/cargo.sh fmt --all -- --check`：通过。
- `./scripts/cargo.sh check --locked --all-targets`：通过。
- `./scripts/cargo.sh test --locked --all-targets`：**258 项通过**（210 库、7 Guides、10 门窗洞口、31 工作流），1 项手动性能基线 ignored。
- `python3 scripts/check-localization.py`：**462 个 literal key**，en/zh 缺失 0、重复 0、zh 未翻译 0。
- `python3 scripts/ac29-workflow.py verify --force`：通过 format、check、localization、tests、tool-tests（15 项）、diff 和 package；报告 `target/ac29-workflow/runs/20261018T100735Z-066ccded/report.json`；`desktop_ui` 为 `unverified`（该脚本不执行桌面交互，手动结果见下节）。
- 最终包 `dist/OpenArchi.app`：release 与包内 arm64 UUID 均为 `E6E06421-C90A-3A80-B072-DFBD824F28CF`；release SHA-256 `f57bf785c6b3cdc0399dd408b4b302223d8f5c0347ba7cedefffd3942817246d`；bundle tree SHA-256 `963e11229f4b8a5e5f004c157e22f79220b2a51715057b1487107e1305b078fe`；`codesign --verify --deep --strict` 通过；macOS zip SHA-256 `f00994404159b3c30c2addd8cb8f4bbb1393f962eb27d9820064f797e187a1cd`。

### 桌面实测

隔离 HOME（`/tmp/openarchi-ac29-final/home`）启动最终包；真实 HOME 与真实恢复项目未触碰。

- 右键：Wall / Continuous，两点后右键，构件 12 → 13，状态 `Created 1 connected wall segments`，Undo 可用，tracker 与起点标记清除。此前同一手势在旧包的 Single 草稿上取消成功（`墙绘制已取消`）；CUA 对两点链的右键投递曾多次失败，最终包重试成功，因此不把旧失败记作功能缺陷。
- 回车结束：两点后回车，12 → 13，状态 `已创建 1 段连续墙`（`D030826E` 包，几何源码与最终包一致）。
- 点击起点：两点链点击起点，12 → 14，状态 `已创建 1 段连续墙`，未追加零长度段（同一 `D030826E` 包）。
- 语言：最终包标题栏选择器 English → 简体中文；窗口标题即时切换；英文提示 `right-click, double-click or Enter to finish`，中文提示 `右键、双击或回车完成`；macOS 菜单栏应用项两种语言均为 `OpenArchi`。
- 重启：选择简体中文后退出并重新启动，首帧为中文（窗口标题 `OpenArchi — 建筑设计工作台`，菜单 `文件 / 编辑 / 视图`，`13 个构件`）。
- 切换语言前后模型保持 13 构件，`Undo` 可用、`Redo` 禁用，无模型或撤销历史变化。
- 墙填充：唯一 Bundle ID 的测试副本（内容同最终包，隔离 HOME，默认 `Courtyard House`）启动后截图采样，外墙切面内部像素为精确 `RGB 211, 211, 211`，与页面底色 `250, 250, 248` 和深色轮廓 `57, 58, 62` 区分清楚；未选中墙全部使用该浅灰，楼板与房间底色不变。该检查未改动真实 HOME 或用户工程。
- 墙体编辑面板：在最终包上选中 `Wall 004` 后单击参考线端点，面板显示 `Wall reference line`、`End endpoint`、`Length: 12.000 m` 与 `Done`；点击 `Done` 后面板关闭，墙体仍保持选中，构件数 12 不变，`Undo`/`Redo` 保持禁用。
- 桌面端点拖动：**pass**。使用最终包与隔离 HOME `/tmp/oa-drag-test/home`，恢复项目 `Wall Join Final` 的交叉状态：`Wall 001=(0,0)→(4,0)`、厚 `0.3 m`，`Wall 002=(2,2)→(2,-2)`、厚 `1.0 m`。选中 `Wall 001`，将起点手柄从 `(0,0)` 拖到 `Wall 002` 基线 `x=2`。CUA 的 `drag()` 仍只产生悬停，因此本轮改用 CGEvent 真实鼠标事件序列：按下、50 帧 `leftMouseDragged`（约 20 ms 间隔）、松开。拖动中显示蓝色目标基线、圆形吸附点和中文 `捕捉：墙参考线`（`/tmp/oa-drag-test/screen13-drag.png`）；松开后状态为 `墙端点已连接到参考线`，存储起点为 `(2,0)`，平面在厚墙近侧面 `x=2.5` 截断（`/tmp/oa-drag-test/screen14-zh-result.png`）。英文重复显示 `Wall endpoint joined at reference line`（`/tmp/oa-drag-test/screen6.png`）。一次 `Undo` 恢复 `(0,0)→(4,0)` 的交叉墙且 `Redo` 变为可用（`/tmp/oa-drag-test/screen7.png`）；重做后 `3D Perspective` 无埋入端盖（`/tmp/oa-drag-test/screen8.png`）。该实例随后退出，真实 HOME 实例未触碰。
- 端点交接自动化：多帧 RawInput 回归 `wall_endpoint_drag_snaps_to_another_reference_line_and_joins`、`wall_endpoint_drag_onto_a_foreign_material_wall_body_snaps_and_joins`、`wall_endpoint_release_inside_snap_radius_joins_without_pointer_move`、`wall_endpoint_reference_snap_wins_over_a_coarse_grid_snap` 均通过。最后一例使用 1 m 粗网格和目标基线 `x=2.4`，断言普通网格会把点拉到 `x=2.0` 时仍由原始指针捕获 `x=2.4` 基线并完成接头。上述测试同时断言目标墙存储基线不被改写、3D 隐藏被埋端盖。
- 派生接头可视（重新载入模型，不是桌面拖动）：持久化项目 `Wall Join Final`，2 个构件；水平墙 `(0,0)→(4,0)`、厚 `0.3 m`，竖向墙 `(2,2)→(2,-2)`、厚 `1.0 m`，水平墙起点已存为 `(2,0)`。重新打开后 Plan 正确裁接到厚墙表面（`/tmp/openarchi-final-plan-t.jpg`），3D 无埋入端盖（`/tmp/openarchi-final-3d-t.jpg`）；平面与 3D 共用 `plan_wall_footprints`。该检查只证明派生轮廓，不证明桌面拖动手势。

### 未完成与未知

- 曲面墙、复合墙板、柱交接优先级、PolyWall 单一元素与 pet palette 未实现；当前为直线墙的多段 `Shape::Wall`。
- 接头算法是几何近似（同侧斜接/面裁剪），不是 `SetWall2PointAndInd` 的逐行复刻。
- 浅灰填充使用默认笔集实测；其他笔集仍会按其 `fill_tint` 对墙填充做色调混合，未逐笔集截图。
- macOS 应用菜单标签按设计保持 `OpenArchi`，不随窗口标题翻译。
- 没有 AC29 运行时 REA Evidence ID；静态符号与哈希只支持 `docs/features/wall-joins.md` 中声明的有限结论。
- 未提交、未打标签、未推送、未发布。

## 2026-10-18 当前状态摘要（0.2 阶段 0–5）

本节覆盖 OpenArchi 0.2 计划的阶段 0–5，取代下方较早轮次的测试数字作为当前结论；旧记录保留为历史证据。源码基线为 `6c3110e`，工作树尚未提交。

### 自动检查

`python3 scripts/ac29-workflow.py verify --force` 通过，报告为 `target/ac29-workflow/runs/20261018T051430Z-f89453d6/report.json`。该轮执行了 format、check、localization、tests、tool-tests、diff 和 package：

- Rust 测试 **223 项通过**（177 项库测试、7 项 Guides、10 项门窗洞口、29 项工作流），另有 1 项性能基线测试标记为 ignored。新增洞口回归覆盖放置默认值、墙端钳制、宿主/楼层/尺寸/重叠/竖向偏移校验、拖动钳制、级联删除与撤销、复制宿主墙、单独粘贴拒绝、平面命中和 JSON 往返。
- 工具测试 **15 项通过**，包括本地化字典检查和打包脚本测试。
- 本地化检查 **456 个 key**，无缺失、重复 key 或未翻译项。
- `python3 scripts/check-localization.py` 已接入 `scripts/ac29-workflow.py verify` 和 GitHub Actions；`.github/workflows/ci.yml` 另外运行 `cargo clippy --locked --all-targets -- -D warnings`。本地 clippy 在清零初始告警后通过；GitHub Actions 远程任务本轮未实际运行，因此远程 CI 不能记为已验证。
- 最终包 `dist/OpenArchi.app` 的 Release 与包内 arm64 UUID 均为 `A5BCB793-F9D4-3652-A4C4-076E3CFBDFC5`；`codesign --verify --deep --strict dist/OpenArchi.app` 通过。
- Release 可执行文件 SHA-256：`f2ea27c6cf78d9e2e9c0bb357a0ac2f8d966e83257fbbff30f14f43cbec69280`；包目录树 SHA-256：`634722a7c689faea5d82831c14b74048e0af0d30c994e16cee57761ff17f70b2`。
- verify 报告的 `desktop_ui` 为 `unverified`：自动流程不执行桌面交互，桌面结论只采用下文的实际冒烟记录。

### 阶段 3 架构拆分

- `src/app.rs` 已拆为 `src/app/mod.rs`、`shell.rs`、`plan.rs`、`model_view.rs`、`inspector.rs`、`info_box.rs`、`quick_options.rs`、`dialogs.rs`、`file_io.rs` 和 `ui_tests.rs`；`src/app/mod.rs` 为 **2,058 行**，达到 2,000–3,000 行的阶段目标。
- 拆分只搬移方法并统一为 `pub(super)`，未改写方法体；对比拆分前后，133 个方法名和 83 个测试函数全部保留，无缺失或重复。
- 新增 `split_modules_render_through_workspace_shell`，同时渲染标题栏/菜单、Info Box 和 Quick Options，验证拆分后的模块入口仍然接线。
- 该阶段当时 `./scripts/cargo.sh fmt --all -- --check`、`check --locked --all-targets`、`clippy --locked --all-targets -- -D warnings` 和 `test --locked --all-targets` 均通过；Rust 测试 **203 项通过**（167 项库测试、7 项 Guides、29 项工作流）。

性能基线见 [performance-baseline.md](performance-baseline.md)。1k / 10k / 50k 墙的墙轮廓准备约为 61 ms / 5.4 s / 133 s，明显超线性；阶段 4 没有加入缓存或空间索引，不能据此宣称大型项目可用。

### 阶段 4 撤销与性能

- 新增 `src/command.rs`：`ModelCommand`（`AddElement`、`RemoveElements`、`RestoreElements`、`UpdateElement`、`MoveElements`、`SetOptions` 和过渡用 `ChangeSet`）、`CommandRecord { command, inverse }` 与 `ProjectDelta`。
- `Editor.undo/redo` 从 `Vec<Project>` 改为 `Vec<CommandRecord>`；每个记录只保存受影响元素的前后状态和变化过的项目头字段，100 步上限保留。
- `Editor::apply` 返回受影响 ID；`delete` 与 `move_selection` 已迁移到 typed command。其余复杂操作继续通过 `commit(before)` 计算 diff，代码路径逐步迁移。
- 回归覆盖一次命令一次 undo、undo/redo 精确恢复、无操作不写历史、undo 后新命令清空 redo、删除后按原顺序恢复，以及 100 步上限。
- 1k / 10k / 50k 墙、固定 1,000 个移动构件：移动约 `1.04 / 3.09 / 12.88 ms`，撤销约 `0.41 / 0.67 / 2.80 ms`，重做约 `0.41 / 0.62 / 1.56 ms`。每步历史只保留 `2,000` 个元素副本（1,000 个构件的前后状态）；50k 时旧快照每步需保留 `50,000` 个。
- 墙轮廓准备仍是 50k 的主要瓶颈，约为 `132.5 s`，本轮未改变。

### 阶段 5 门窗洞口

- 新增 `Shape::Opening { host_wall, kind, width, height, sill, offset }`；`offset` 为洞口中心距宿主墙参考线起点的距离，`sill` 从宿主墙所在楼层标高起算。
- Door 默认 `0.90 × 2.10 m`、窗台 `0`；Window 默认 `1.20 × 1.40 m`、窗台 `0.90 m`。
- 校验覆盖宿主存在且为墙、同楼层、墙内、正尺寸、洞口顶不超墙顶、窗台不低于墙底偏移，以及同墙洞口区间重叠。
- 平面先绘制墙体，再以平面背景色擦出洞口带并绘制门扇/开启弧或窗玻璃线；3D 对直墙生成墙垛、窗台和过梁，窗附加薄玻璃板，带洞口墙跳过联合墙轮廓优化。
- 移动墙时洞口保持相对偏移；单独拖动洞口只沿宿主墙滑动，并在宿主墙长度内钳制。删除墙级联删除洞口并记为一次撤销；复制宿主墙时重映射洞口宿主，单独粘贴洞口会被跳过。
- 自动测试 10 项通过；JSON 往返、级联删除/撤销、复制重映射与拖动钳制均有回归。平面背景色擦除不会透出底层楼板，Marquee 裁切带洞口的墙仍显示为实墙，这两点保留为第一版限制。

### 桌面实测（阶段 2 包）

以下操作先在 UUID `8E2CB02A-D36F-3161-9AC5-51DA6D212957` 的 `dist/OpenArchi.app` 上完成，使用简体中文或英文界面，并记录构件数与恢复文件状态。当前 UUID `7266453C-B925-3BDB-86E4-C6AD60BC9A61` 的包包含恢复节流修正与 `app` 模块拆分，之后已在同一台机器上单独完成桌面冒烟，结果见下一节。

- 原生 Browse 保存到 `/private/tmp/openarchi-0.2-smoke.openarchi.json`：保存后 13 个构件，恢复文件被清理；重新打开文件后仍为 13 个构件。原生保存面板返回路径后，仍需在 egui 模态中确认保存。
- 干净项目关闭窗口直接退出，没有提示；有未保存修改时显示 `保存并关闭 / 放弃并关闭 / 取消`（英文为 `Save and close / Discard and close / Cancel`）。
- 取消关闭后，构件数、选区和撤销/重做可用性保持不变；英文关闭提示取消后仍为 12 个构件。
- 保存并关闭写入 14 个构件、`next_id=15` 的项目文件，清除恢复文件并退出进程。
- 放弃并关闭清除恢复文件、不写项目并退出；重启后不再出现恢复提示。
- 崩溃恢复：制造未保存修改后 `killall -9 openarchi` 强杀；重启显示 `恢复未保存的项目`。选择恢复后得到 13 个构件且保持 dirty；选择放弃后恢复文件被清理，再次重启无提示。
- macOS 上 `Cmd+Q` 不会进入窗口关闭保护对话框，而是直接退出应用；退出前仍写入了恢复文件，下次启动可选择恢复（本轮以恢复后 13 个构件验证）。这是当前已知行为，不是“窗口关闭提示”的替代证据。
- 语言切换两个入口均实测：标题栏选择器和 `视图 > 语言` 都能立即切换；主窗口、菜单、工具栏、Info Box、工具箱、视图标签、构件数、状态栏和 Quick Options 同步变化。
- 选择 `简体中文` 后退出并重启最终包，首帧即为中文；构件数保持 12，撤销/重做仍不可用，未出现恢复提示。偏好文件 `~/Library/Application Support/OpenArchi/language` 为 `zh-CN`。

### 桌面实测（阶段 2/3 包，2026-10-18）

在 UUID `7266453C-B925-3BDB-86E4-C6AD60BC9A61` 的 `dist/OpenArchi.app` 上执行，简体中文，`Courtyard House` 示例（12 个构件）：

- 首帧为简体中文；选中 `墙 004`，在 Info Box 将高度由 `3.000 m` 改为 `3.005 m` 并应用，标题栏出现未保存标记，`撤销` 启用，状态栏显示 `信息框墙体设置已应用 · 平面图与三维视图已同步`。
- 点击窗口关闭按钮，出现中文 `未保存的更改` 对话框（`保存并关闭 / 放弃并关闭 / 取消`），此状态下 `recovery.openarchi.json` 已存在且包含 `3.005 m`。
- 选择 `取消`：对话框关闭，窗口保持打开，仍为 12 个构件、未保存标记与 `撤销` 可用，墙高仍为 `3.005 m`。
- 再次关闭并选择 `放弃并关闭`：进程退出，`recovery.openarchi.json` 被删除，未出现保存面板或项目写入。
- 重新启动同一包：首帧为简体中文，未出现恢复提示，12 个构件，`撤销`/`重做` 均禁用；随后正常关闭窗口，进程退出。语言偏好仍为 `zh-CN`。
- 该包 `codesign --verify --deep --strict dist/OpenArchi.app` 通过，包内可执行文件 UUID 与 `target/release/openarchi` 均为 `7266453C-B925-3BDB-86E4-C6AD60BC9A61`。

### 桌面实测（阶段 4 包，2026-10-18）

在 UUID `6C3627BB-371A-340E-9799-8964FB1A6471` 的 `dist/OpenArchi.app` 上启动一次：首帧为简体中文，`Courtyard House` 显示 12 个构件，`撤销`/`重做` 均禁用；窗口正常关闭，进程退出。该轮没有在桌面重复执行移动/撤销手势，typed command 的 undo/redo 语义由单元测试和 release 基线覆盖。

### 桌面实测（阶段 5 包，2026-10-18）

在 UUID `A5BCB793-F9D4-3652-A4C4-076E3CFBDFC5` 的 `dist/OpenArchi.app` 上启动一次，简体中文：

- 首帧为简体中文，`Courtyard House` 显示 12 个构件，`撤销`/`重做` 均禁用。
- 工具箱显示 `门` / `窗`；点击 `门` 后 Info Box 显示 `门 · 洞口`、`宽度 0.900 m`、`高度 2.100 m`、`窗台高度 0.000 m`，状态提示为 `Click on a wall to place a door`。
- 标题栏语言选择器可打开，菜单同时列出 `English` 与 `简体中文`。
- 该轮未能在桌面完成墙体点击放置：自动化指针坐标受窗口缩放映射影响，点选未命中墙体；因此 2D 洞口外观、3D 洞口外观、英文新标签和原生保存/重开仍为 `unknown`。这些路径仅由自动测试和代码检查覆盖，不记为桌面验收。

### 仍未验证

- Windows 实机启动、中文首帧、保存/重开、F2/F3 仍未在当前机器验证；Windows 包只能记录为构建产物。
- GitHub Actions 的 macOS/Windows 远程任务本轮未运行。
- `Cmd+Q` 的 AppKit 退出路径没有弹出应用内关闭对话框；是否需要拦截属于后续决策。
- 各对话框尚未逐一在最终包上用两种语言重读；已检查的主窗口、墙设置、保存/打开、关闭与恢复提示之外仍标记为 `unknown`。
- 阶段 5 最终包上的指针放置、2D/3D 洞口视觉、英文新标签、原生保存/重开仍为 `unknown`；自动测试只证明模型、几何查询、持久化和回归语义。

日期：2026-10-12。设备：Apple Silicon，macOS 15.7.5。工具链：Rust 1.98.1，aarch64-apple-darwin。同材质墙体平面交接清理更新后，所有自动测试通过：42 项（21 项工作流、21 项单元/egui 测试）。`cargo check --locked --offline --all-targets` 和 Release 构建通过。

## 自动检查

执行入口：

```sh
./scripts/cargo.sh check --locked --offline --all-targets
./scripts/cargo.sh test --locked --offline
./scripts/cargo.sh build --release --locked --offline
```

自动测试覆盖：

- 21 项工作流测试：原有墙/板建模、撤销重做、楼层、吸附、属性、存储、3D 拾取，加上 Marquee 范围/节点选择、组移动/复制/剪切/粘贴、90°旋转、镜像、相邻楼层迁移、节点拉伸回滚、3D XY/高度体积筛选。
- 9 项几何测试、1 项渲染网格测试：覆盖共线墙端线消除、T 形连接（隐藏被主墙覆盖的 3D 端封头）、L 角正交连续轮廓、20 cm 墙面落点偏差自动交接与边线相交；并断言交接轮廓没有斜线。也覆盖旋转矩形、Marquee 节点选择/几何相交、凹多边形裁切、透视投影，以及带 Z 范围的墙体裁切。
- 11 项 egui 交互/渲染测试：墙/板建模、点选与拖动、撤销重做、Tracker、Marquee 绘制、F2/F3/F5、Shift+中键旋转、⌘T 对话框，以及三种视图与三种侧栏内容渲染。
- ⌘T 交互回归测试确认快捷键打开独立的元素设置模态框，未切换侧栏或修改 Inspector 可见状态；应用的属性修改进入撤销栈。
- egui 自动测试注入 `RawInput` 鼠标/键盘事件并执行实际画布代码，但仍需与桌面实机测试区分；测试本身不覆盖 macOS 窗口事件分发和 GPU。

## 实机检查

本轮实际从 `dist/OpenArchi.app` 启动最新构建，并用桌面 UI 自动化选中示例模型中的墙、按下 ⌘T、在弹出的独立对话框将墙高从 3.000 m 改为 3.500 m，按 Apply 后确认右侧属性与面积更新，然后执行 ⌘Z 恢复为 3.000 m。首次验证发现磁盘中的应用包早于对话框实现，因此已用当前通过检查的可执行文件更新 `dist/OpenArchi.app` 并重新进行本地 ad-hoc 签名。CUA 原有应用名绑定指向旧 Bundle ID；用测试副本绑定后完成了实际窗口操作。测试副本的临时标识不影响交付包的 `com.openarchi.desktop`。

根据后续校正，墙体交接不应添加斜面。当前 2D 与普通 3D 透视共用吸附并延伸后的矩形显示轮廓，让内外墙面线以正交方式相交；交叠的墙端封头和内部边线隐藏。回归断言覆盖 20 cm 偏移的 L 角、T 形支墙封头，以及轮廓不存在斜线。项目里的墙仍是独立构件；Marquee 裁切依旧逐墙处理。

2026-10-13 使用 REA 实机观察 Archicad 29 arm64。REA `observe_native_ui` 记录了平面视图，`capture_native_ui_scenario` 通过当前窗口可访问的 3D 标签切换到透视模型；受控后的截图可见两面墙在 L 角正交相接、没有对角斜接面。证据 ID 和截图边界见 [research.md](research.md)。据此普通 3D 的常见同高度连通 L/T 墙组改为由单一暴露轮廓挤出，避免分墙实体重叠造成内部面；当前仅覆盖单闭环外轮廓，带孔或分离组保留原逐墙绘制路径。

此项代码通过 22 项单元/egui 测试、21 项工作流测试及 `cargo check --all-targets`；Release 包已重建并通过本地 `codesign --verify --deep --strict`。本次 CUA 按包路径启动仍被旧 Launch Services 记录映射到 `studio.atelier.bim.demo`，所以本次更改尚未从最终 `.app` 完成目视回归；不要把上一轮的属性对话框测试当作这次新网格的运行时验证。

首次 GPU 启动发现 wgpu 30 不允许 LineList 使用 depth bias，已修复为仅三角形偏移；保留深度测试的边线使用零偏移。该问题只能通过 GPU 运行发现，编译与领域测试不足以覆盖。

## 验证边界

未完成长期稳定性、不同 macOS/显卡、百万级构件、性能基准、签名公证或安装器测试。REA 原生 UI 已观察当前本机 Archicad 29 的窗口并完成只读 3D 视图切换；Marquee 的完整逐步交互、不同材质/高度墙交接和高倍细部仍未观察，按公开 Graphisoft AC29 帮助或 unknown 标记。3D Marquee 的 macOS 实机目视检查不在本轮 ⌘T 回归范围内。

最终 Release 包为约 14 MB 的 arm64 Mach-O 可执行文件；已通过 `codesign --verify --deep --strict` 本地签名检查。相机为 45° 视场的透视投影，缩放会按视距重新取景。

2026-10-13 根据本机 Archicad 29 工作区创建 [界面与菜单清单](archicad-interface-inventory.md)，记录主菜单、菜单分组、当前架构工作区工具箱、面板分布与菜单状态范围。后续扩展已增加标准/编辑工具栏、与 Archicad 类似的 Design 工具长列、Viewpoint/Document 工具组、四种 Navigator 地图、搜索框，以及各核心菜单的可浏览层级。墙、板、直跑楼梯、Marquee、Arrow 与模型后端连接；其他工具和专业命令仍为交互式 UI 预览，不修改项目模型。所有 3 种主视图、3 种侧栏面板、4 种 Navigator 地图及配置预览页都纳入 egui 渲染回归。本轮 `cargo check --locked --offline --all-targets` 与 44 项自动测试通过，`dist/OpenArchi.app` 已重建且签名校验通过。尝试经 Finder/Launch Services 以包路径启动时收到 `-10827 kLSNoExecutableErr`，尽管包内 `Contents/MacOS/openarchi` 存在；本轮界面布局没有完成最终桌面目视回归。

## 2026-10-14 参数化直跑楼梯

本轮继续完成 Kimi 留下的楼梯纵向切片，并补上 Design → Architectural Tools 菜单的实际楼梯入口；该菜单项此前仍是界面预览。Toolbox、S 快捷键与 Design 菜单现在都会进入同一参数设置/绘制流程。Inspector 数量摘要也显示楼梯数量。参考脚本仅做可读源静态分析，目标文件、GBK 编码、SHA-256、函数位置与未检查依赖见 [楼梯脚本分析](stair-script-analysis.md)；没有运行 Archicad、AutoCAD 或天正，也没有 REA 运行时证据。

验证结果：`./scripts/cargo.sh check --locked --all-targets` 通过；`./scripts/cargo.sh test --locked` 通过，共 51 项（30 项单元/egui 测试、21 项工作流测试）。egui 测试覆盖 RawInput 下按 S 激活真实 Stair 工具并打开设置、参数化创建、平面踏步生成、JSON 往返、撤销/重做、属性修改撤销以及设置窗不改动模型。自动化不等同于桌面实机验证。本机 `./scripts/cargo.sh fmt --all -- --check` 未能启动：stable 工具链未安装 `cargo-fmt`。没有重建/更新 `dist/OpenArchi.app`，也没有执行上述验收步骤的桌面手动检查。

## 2026-10-14 多段线绘制

实现了 OpenArchi 的开放式 2D Polyline 竖切：`P` 快捷键、Document 工具组和 Document 菜单进入同一个绘制工具；左键连续放节点，右键完成开放多段线，点击起点闭合并完成，Esc 取消。Tracker 显示相对上一点的距离/角度，可编辑并锁定；画布预览加上 X/Y 与 30° 方向、节点对齐和垂直方向推断。节点属性、选区移动/节点拉伸/镜像/旋转、稳定 ID、模型校验、JSON 和撤销/重做仍接入原共享模型。多段线只画在平面，不生成 3D 网格；弧段、替代几何方法、Info Box 笔号/线分类编辑仍未实现。

REA 运行观察只确认 Archicad 29 Polyline 工具已激活及可见 Info Box 默认值；未观察参考软件的实际节点/完成/取消序列。本文新增的光标、Tracker、对齐与完成顺序来自用户提供的行为描述，证据、历史 REA 观察和功能约定见[研究记录](research.md)与[多段线功能说明](features/polyline.md)。OpenArchi 仍保留 Enter 作为快捷结束（Tracker 锁定时则添加数值点），但右键是主要完成操作；这些行为不表示已证明 Archicad 的全部细节。

本次复核：`./scripts/cargo.sh check --locked --offline --all-targets` 通过。未添加或运行测试；此前多段线测试仍覆盖旧的 Enter 完成流程，不能作为本次 Tracker、鼠标光标、右键或闭合行为的验证证据。`cargo-fmt` 缺失，格式检查未能运行；没有重建应用包或进行桌面目视检查。

验证：`./scripts/cargo.sh check --locked --all-targets` 通过；`./scripts/cargo.sh test --locked` 通过，共 54 项（32 项单元/egui 测试、22 项工作流测试）；`git diff --check` 和中英文 JSON 解析通过。测试覆盖真实 RawInput 计划画布节点添加、Enter 提交、Esc 取消、模型校验、命中/吸附、JSON 往返、选择移动和撤销/重做。`./scripts/cargo.sh fmt --all -- --check` 无法启动，因为 stable 工具链未安装 `cargo-fmt`。Release 包由 `./scripts/package-macos.sh` 构建并本地 ad-hoc 签名成功。新包桌面目视验证未完成：CUA 对新 Bundle ID 的绑定失败，并将请求解析为旧标识 `studio.atelier.bim.demo`（`Running application not found`）；因此界面运行结果仍标记为未验证。

## 2026-10-14 智能光标反馈

给现有工具添加了选择命中、绘图定位、端点/中点吸附、Marquee、元素拖动、视图平移与 3D 旋转的指针反馈。功能范围和逐项复现清单见[智能光标功能说明](features/smart-cursor.md)。Graphisoft AC29 官方帮助列有更多光标家族，但不对应当前 OpenArchi 工具集。

验证结果：仓库脚本 `./scripts/cargo.sh check --locked --offline --all-targets` 通过。`./scripts/cargo.sh fmt --all -- --check` 无法运行，因为 stable 工具链未安装 `cargo-fmt`。未添加或运行测试。REA 已绑定本机 Archicad 29 arm64 Mach-O，但 `observe_native_ui` 因所选窗口没有匹配的 Accessibility 几何返回 `ambiguous-window`；可用的 REA 原生场景操作不支持移动指针，因此没有 Archicad 运行时光标证据。Graphisoft 官方帮助是参考行为证据，不能视为本机逐态观察。直接调用系统 Cargo 的初次结果受旧版本影响；以仓库脚本的检查结果为准。

后续桌面核对：使用更新后的 Release 可执行文件，在 `/private/tmp/openarchi-polyline-cursor-smoke/OpenArchi.app` 临时包中启动 `com.openarchi.desktop`，未修改 `dist/OpenArchi.app`。CUA 实际点击完成了新建 Polyline、添加节点、右键结束开放线和点击起点结束闭合线；画面新增两个构件，闭合尝试结束后恢复“Tracker · click a start node”，与工具重置一致。第一点之后 Tracker 输入行维持相同画布布局，连续点选时画布没有再次跳动；截图也显示节点间蓝色预览线、距离/角度标签和 Alignment/Endpoint/Grid 提示。CUA 的点击会直接投递按下/抬起事件，不能验证真实鼠标连续移动时每一帧的光标形态；十字、实心笔、空心笔在自由移动过程中的完整过渡仍未目视确认。该记录是 OpenArchi 临时构建的桌面检查，不是 Archicad 运行观察；测试副本中的操作未保存到项目文件。

多段线选中外观更新：plan 绘制路径将已选线段改为绿色高亮，并为每个可编辑节点绘制黑色实心圆点。`./scripts/cargo.sh check --locked --offline --all-targets` 和 `./scripts/cargo.sh build --release --locked --offline` 通过。CUA 在临时包 `/private/tmp/openarchi-polyline-cursor-smoke/OpenArchi.app` 中用 V 激活选择、Esc 清除创建后的自动选中，再点击线段；截图分别确认未选中时为普通暗色线，选中时为绿色线且三个节点都是黑色实心圆点。随后运行 `./scripts/package-macos.sh` 更新 `dist/OpenArchi.app`；包内与 Release 可执行文件的 Mach-O UUID 都是 `0A7C0125-3FE3-3A44-9F8A-33E9E145363D`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。未添加或运行自动化测试。

## 2026-10-14 多段线节点与边编辑

选择多段线后，再点节点或线段会出现原生 egui 浮动面板。默认节点操作可在同一按下拖动中移动节点，左右相邻线段随指针预览，松开记作一个撤销事务。面板提供倒直角、倒圆角、缩放边线、移动、旋转、镜像和按数量多重复制；多重复制默认放置 2 个副本，生成新 ID 并选中副本。圆角以 8 段折线近似圆弧，倒角/圆角仅支持非共线的内部节点。

行为参考依据为 Graphisoft AC29 Pet Palette、Move Nodes、Adding Element Nodes 公开帮助，以及 Graphisoft AC18 Fillet/Chamfer 帮助；它们支持上下文面板、节点移动/边插点和单角倒角/圆角的总体行为，不证明本实现完整菜单与手势完全一致。边缩放和本面板命令列表依照用户描述实现；未进行 Archicad 运行时操作。本轮自动验证为 `./scripts/cargo.sh test --locked --offline`：37 项单元/egui 测试和 22 项工作流测试，共 59 项通过；`./scripts/cargo.sh check --locked --offline --all-targets` 通过。`git diff --check`、英语/简体中文 JSON 解析通过。格式检查未启动：stable 缺少 `cargo-fmt`。

`./scripts/package-macos.sh` 构建标准包 `dist/OpenArchi.app`；Release 二进制与包内 arm64 Mach-O UUID 都是 `FDC2E05E-3963-391D-AAAB-E0C079A7AC98`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。`open dist/OpenArchi.app` 返回成功，但桌面交互复核未完成：CUA 无法绑定新包（返回 `Invalid app: com.openarchi.desktop`），因此新浮动面板未做实机视觉检查；运行结果明确标记为未验证。

## 2026-10-14 Esc 命令返回

Esc 现在取消未完成的绘制、放置、拖动或打开的命令对话框，并返回前一个建模工具；再次按 Esc 回到 Select。首次进入工具时，前一个工具为 Select；只记住一层命令历史。Select 状态下清除选中和移除持久 Marquee 的已有顺序保留。详细交互约定见 [Esc 命令返回](features/escape-command-return.md)。

验证：`./scripts/cargo.sh test --locked --offline` 通过，33 项单元/egui 测试与 22 项工作流测试共 55 项通过；新增 RawInput 回归覆盖 W → P → Esc 恢复 Wall → Esc 回到 Select，并确认当前 Polyline 草稿被取消。`./scripts/cargo.sh check --locked --offline --all-targets` 通过。实际从仓库标准包 `dist/OpenArchi.app` 启动并按 W → P → Esc，底部状态显示 `Command cancelled · Wall restored`；再按 Esc 显示 `Command cancelled · Select restored`。`./scripts/package-macos.sh` 已运行；包内与 Release 可执行文件的 arm64 Mach-O UUID 均为 `4E69402D-32D7-3435-975C-D7A6DB34559D`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。桌面场景验证了工具返回，但未在桌面逐项取消每一种工具的草稿。格式检查未运行成功：stable 工具链缺少 `cargo-fmt`。

## 2026-10-14 多段线节点编辑光标

修正 Move Node 编辑状态下的光标：节点移动时保持箭头；只有中键平移视图显示抓取手形。新增 RawInput egui 回归覆盖节点编辑悬停、按住主键移动和中键拖动画布。Archicad 的光标仍没有本机运行观察证据；这是 OpenArchi 根据用户描述实现的反馈映射。

验证：`./scripts/cargo.sh test --locked --offline` 通过，共 60 项（38 项单元/egui 测试、22 项工作流测试）；`./scripts/cargo.sh check --locked --offline --all-targets` 通过。RawInput 用例在节点编辑开面板后、主键拖动中和中键平移时分别检查光标；应用层在整帧 UI 绘制结束后再应用 Move Node 光标，避免面板控件覆盖箭头。`git diff --check` 通过。格式检查未能启动：stable 工具链未安装 `cargo-fmt`。`./scripts/package-macos.sh` 已重建标准包 `dist/OpenArchi.app`；Release 与包内 arm64 可执行文件 UUID 均为 `7A8D262A-A9CB-3E80-9AC9-24E64838D935`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。桌面工具仍将 OpenArchi 解析为旧 Bundle ID `studio.atelier.bim.demo`，新 ID 返回 `Invalid app: com.openarchi.desktop`；因此新包的节点光标桌面视觉检查未完成，光标状态由 RawInput egui 回归验证。

## 2026-10-14 选择光标节点与边提示

根据 Graphisoft AC29《[The Intelligent Cursor](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-39.htm)》检查 Arrow Tool 光标表：物体表面用普通箭头，参考/其他节点用重/轻对号，参考线/其他边用重/轻 Mercedes 标记，另外还区分交点、切线和垂直。按用户指定实现一个简化子集：表面普通箭头、节点箭头旁对号、线段箭头旁三向星标；节点命中优先于边，边优先于物体表面。提示图形为 OpenArchi 自绘，不复制 Graphisoft 图片。REA/CUA 当前没有可用的纯悬停指针动作，Archicad 实际光标外观没有做运行时目视验证。`agent-reach doctor --json` 受 `/Users/ren/.agent-reach` 权限错误阻断；使用公开 Graphisoft 页面直接核对文字说明。

验证：新增几何测试覆盖节点/边/表面命中优先级，egui RawInput 测试覆盖选择模式光标输出。`./scripts/cargo.sh test --locked --offline` 通过，共 62 项（40 项单元/egui 测试、22 项工作流测试）；`./scripts/cargo.sh check --locked --offline --all-targets` 通过；`git diff --check` 通过。格式检查未运行：stable 工具链未安装 `cargo-fmt`。`./scripts/package-macos.sh` 重建了 `dist/OpenArchi.app`；Release 与包内 arm64 Mach-O UUID 同为 `36130ECE-D741-3DF3-9E1C-68D57C0E6778`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。没有完成桌面目视检查；当前 UI 自动化没有纯悬停动作。


## 2026-10-15 多段线整线拖动与边偏移修正

更正上一轮对“缩放边线”的理解。Graphisoft 官方《[Reshaping Polygons and Chained Elements](https://helpcenter.graphisoft.com/user-guide/136853/)》明确说明 Offset edge 偏移整条边并拉伸相邻边，且适用于链式元素。OpenArchi 面板现在称为“偏移边线”；拖动按边线法向量计算偏移，相邻边与偏移后的边求交，开放端点直接随偏移移动，闭合多段线会保持首尾点重合。选择状态下从多段线边段拖动会移动整条多段线；此动作不打开浮动面板，光标反馈为箭头，松开后记为单个撤销事务。节点拖动仍是节点编辑。

验证：`./scripts/cargo.sh test --locked --offline` 通过，42 项单元/egui 测试与 22 项工作流测试，共 64 项；新增 UI 回归验证选中后拖动边段、整条多段线移动、箭头光标、无面板及单步撤销；偏移几何回归覆盖开放与闭合多段线的边平移和相邻边相交。此版本当时还采用点击边打开 Offset edge 面板的旧交互，后续行为见下方“多段线边上单击插入节点”。`./scripts/cargo.sh check --locked --offline --all-targets`、`git diff --check` 和双语 JSON 解析通过。格式检查未能运行：stable 工具链未安装 `cargo-fmt`。

`./scripts/package-macos.sh` 已重建 `dist/OpenArchi.app`。`target/release/openarchi` 和包内 `Contents/MacOS/openarchi` 的 arm64 UUID 均为 `9DC9D38E-09A1-3CBF-AB55-855A62F952ED`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。以 `open -n dist/OpenArchi.app` 启动新实例并确认窗口出现；CUA 无法绑定该窗口，`getApp` 错误仍指向旧 Bundle ID `studio.atelier.bim.demo`，因此此次没有桌面交互验证 Offset edge 面板和实时预览，不能把编译/自动化结果描述为手工桌面通过。


## 2026-10-15 多段线边上单击插入节点

依据 Graphisoft AC29 [Adding Element Nodes](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-90.htm)：官方流程是选中构件、点击边调出 Pet Palette、选 Insert new node，再点击确定位置；双击只在 Insert Node 已激活时直接插入。Move Node 帮助说明节点可用 Pet Palette 移动。OpenArchi 按用户要求采用单击边段直接插入的快捷操作：将点击位置投影到边上，插入后立即打开新节点的编辑面板；面板保留 Move node、倒角/圆角等操作，并提供该上下文边的 Offset edge。插入会保持原折线顺序、拒绝任一新线段短于 0.01 m，并提交为一个可撤销操作。这是 OpenArchi 的单击简化，区别于 Graphisoft 帮助描述的两阶段操作。

验证：`./scripts/cargo.sh test --locked --offline` 通过，42 项单元/egui 测试与 22 项工作流测试，共 64 项；RawInput 回归确认选中多段线后单击边段能按投影位置插入节点、打开该节点面板并由一次 Undo 恢复原几何。`./scripts/cargo.sh check --locked --offline --all-targets`、`git diff --check` 和双语 JSON 解析通过。格式检查仍因 stable 未安装 `cargo-fmt` 而无法运行。

`./scripts/package-macos.sh` 重建 `dist/OpenArchi.app`；Release 与包内 arm64 UUID 均为 `2F15F008-45AA-3927-A961-2EB79F26FDC1`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。使用 `open -n dist/OpenArchi.app` 启动并目视确认新窗口出现；CUA 的 `getApp` 绑定错误仍引用旧 Bundle ID `studio.atelier.bim.demo`，故没有完成桌面手工插入/面板操作验证。

## 2026-10-15 多段线整体偏移与圆弧线段

按用户反馈，将偏移从单边改为整条 Polyline 的等距偏移：所有线段沿同一法向距离移动，节点取相邻偏移线交点；拖动指向轮廓内侧/外侧决定内偏移或外偏移。开放路径的两个端点按端边法向移动。节点编辑面板新增 Curve edge，拖动点与上下端点求过三点圆弧，并用采样节点预览和保存。模型当前只有折线顶点，故弧段以弦段采样保存，不是精确圆弧实体。

依据 Graphisoft AC29《Offset All Edges》与 AC30/AC19《Curve/Straighten Element Edge》公开帮助核对语义；官方 Offset All Edges 是整多边形比例增大/缩小预览，和用户指定的等距内/外偏移不同。REA 工具未提供，本轮只查阅了官方文档，没有 Archicad 运行时观察。

自动验证：`./scripts/cargo.sh test --locked --offline` 通过，45 项单元/egui 测试与 22 项工作流测试，共 67 项；新增覆盖开放/闭合路径整体偏移、内外偏移、三点圆弧过点、RawInput 拖动预览、提交与 Undo。`./scripts/cargo.sh check --locked --offline --all-targets`、双语 JSON 解析及 `git diff --check` 通过。格式检查未能运行：stable 工具链未安装 `cargo-fmt`。

已运行 `./scripts/package-macos.sh` 重建 [dist/OpenArchi.app](../dist/OpenArchi.app)。Release 与包内 arm64 Mach-O UUID 均为 `E699FEC1-20FA-34B4-9BE6-0BE40D08776C`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过；执行 `open -n dist/OpenArchi.app` 返回成功。桌面实机操作未验证：CUA 仍将名称解析为旧 Bundle ID `studio.atelier.bim.demo`，直接按当前 ID `com.openarchi.desktop` 绑定也返回 `Invalid app`。因此自动测试通过，不代表用户界面已做桌面手工验收。

## 2026-10-15 Select 节点拖动判定修正

修正直接拖动多段线端点/节点时误进 Move Node 的分流：Select 下没有打开节点面板时，从构件任意位置按住并拖动都移动整条 Polyline；按下并拖动不再自动打开节点编辑。单击后释放节点仍可打开节点面板，从面板进入节点编辑。新增 RawInput 回归覆盖从端点拖动整条路径、没有打开编辑面板、一次 Undo 撤销；原有节点面板拖节点测试继续验证两条相邻线段会更新。

验证：`./scripts/cargo.sh test --locked --offline` 通过，45 项单元/egui 测试与 22 项工作流测试，共 67 项；`./scripts/cargo.sh check --locked --offline --all-targets` 与 `git diff --check` 通过。已重新构建 [dist/OpenArchi.app](../dist/OpenArchi.app)，Release 与包内 arm64 UUID 均为 `5CFB30A1-4851-3AAF-9BBF-FA3A1892B721`，签名验证通过，`open -n dist/OpenArchi.app` 返回成功。CUA 仍无法绑定此 Bundle ID，因此桌面手工交互未验证。

## 2026-10-15 Polyline 精确圆弧段与实时半径预览

将 Curve edge 从“把圆弧采样点写进顶点数组”改为每条边可选的精确 `ArcSegment` 参数（半径、顺逆时针与大/小弧标记）。拖动预览每帧重算圆弧，并在指针旁显示半径；绘制采样只存在于视图路径。选中既有圆弧边进入边编辑时不插入节点，模型节点仍只有边两端。移动节点保留弧段半径参数；镜像翻转弧方向；倒圆角也保存为精确圆弧。旧项目 JSON 没有 `arcs` 字段时按全直线载入。

验证：`./scripts/cargo.sh test --locked --offline` 通过，47 项单元/egui 测试与 22 项工作流测试，共 69 项。新增覆盖三点圆弧、动态半径、端点/顶点数不变、弧段 JSON 往返、旧 JSON 兼容，以及点击既有圆弧不插入节点。`./scripts/cargo.sh check --locked --offline --all-targets` 与 `git diff --check` 通过。格式检查未运行：stable 工具链未安装 `cargo-fmt`。

已重建 [dist/OpenArchi.app](../dist/OpenArchi.app)。Release 与包内 arm64 Mach-O UUID 均为 `EB326EDF-9F46-3E2C-AA1D-67570023F74E`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过；`open -n dist/OpenArchi.app` 成功。桌面交互仍未验证：CUA 应用清单显示 OpenArchi 正在运行，但 `getApp` 将它解析为旧 ID `studio.atelier.bim.demo` 并返回 `Running application not found`，所以这不是手工 UI 通过记录。用户可从 `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app` 启动，在平面图选择 Polyline、点选圆弧边、面板选 Curve edge，再按住拖动边段；观察鼠标旁半径变化及仅有两个弧端点，松开提交，⌘Z 撤销。

## 2026-10-15 Guides 菜单第一轮实现

对应 Archicad 29.0.0 v1 INT (5101) 的 Guides 菜单，完成平面范围的五种分点、三种分点范围、数值设置对话框、临时参照、固定线段／无限线／圆／三点圆弧、边缘标签拖入、手柄拖动、数值偏移、擦除与全部移除，以及已有墙模型的距离辅助与数值定位／编辑。辅助线和设置随 JSON 持久化，兼容旧 JSON，支持撤销／重做。门窗参照明确禁用；完整三维辅助线、切线／角平分线推断、距离参照重连仍未实现。不能据此宣称 Guides 全部交互与 Archicad 等价。行为与证据清单见 [Guides](features/guides.md)。

检查结果：`./scripts/cargo.sh check --locked --all-targets`、`./scripts/cargo.sh fmt --all -- --check`、单独 `rustfmt --edition 2024 --check src/guides_ui.rs`、`git diff --check` 通过。缺失的官方 rustfmt 组件已安装；保留已有源代码内容并统一 Rust 格式。`./scripts/cargo.sh test --locked` **114 项通过**：85 项库测试（含 11 项 Guides RawInput 交互测试）、7 项 Guides 保存／撤销集成测试、22 项既有工作流测试，零失败。交互覆盖创建、预览取消、拖动与撤销、跨采样擦除、标签拖入、分点与辅助线独立开关、交点、设置无效值／Apply／Escape、距离输入与选中墙数值移动。

`./scripts/package-macos.sh` 成功重建 [dist/OpenArchi.app](../dist/OpenArchi.app)。Release 与包内程序 UUID 均为 `179E1B19-1215-33AD-8C9A-5E141F72A195`（arm64）；`codesign --verify --deep --strict dist/OpenArchi.app` 通过。CUA 按完整应用包路径启动／绑定时返回 `Computer Use server error -10005: cgWindowNotFound`。随后进程检查确认 PID 35996 来自该 `dist` 包；这仅验证已启动当前包，**不构成桌面窗口或手工交互验收**。本轮未完成 OpenArchi 桌面菜单、辅助线与距离输入操作；不得将 RawInput 测试或旧版截图当作本次实机验证。

## 2026-10-15 AC29 项目技能与可复用验证工具

新增仓库技能 `skills/openarchi-ac29/SKILL.md`，通过 `~/.codex/skills/openarchi-ac29` 符号链接安装；`AGENTS.md` 提供项目入口，单份仓库内容同时服务自动发现与项目续做。技能记录本机 AC29 5101 的路径、bundle ID、版本/hash，历史观察材料与原生窗口故障的证据边界。新建 `scripts/ac29-workflow.py`（Python 3.11+ 标准库）提供 `context`、`feature`、`verify`、`bundle`；没有自动软件安装、GUI 操作或网络修复。

技能校验：本机 `python3` 缺少 quick_validate 所需 PyYAML，因此使用已存在的 `/Users/ren/miniconda3/bin/python`（PyYAML 6.0.2）运行 skill-creator 的 `quick_validate.py`，仓库副本与全局链接均通过。脚本本身不需要 PyYAML。上下文工具实际核对目标为 `29.0.0 v1 INT (5101)` / `29.0.0.5101`，SHA-256 与既有证据一致；第二次调用准确复用目标文件身份缓存。

真实 `verify` 成功记录：`target/ac29-workflow/runs/20261015T141306Z-e87ef32a/report.json`（该目录可由 cargo clean 清除，不作为永久研究证据）。实际执行格式及 included-Rust 格式检查、全目标编译、**114 项 Rust 测试**、**11 项工具测试**、diff 检查和 `./scripts/package-macos.sh`，全通过。工具测试覆盖未跟踪输入/环境失效、签名后产物变更、文档差异检查、无 receipt 时 unknown、检查失败不打包、构建期间源码变化拒绝认证、不覆盖既有契约、目标缓存更新及拥有的子进程超时终止。

紧接着第二次 `verify` 返回 `reused`（未重跑编译/测试/打包，保留原验证时间），并重新检查产物哈希、UUID、签名和 `git diff --check`；`bundle` 返回 `passed` / `matches_successful_build_receipt`。交付仍是 `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`，Release 与包内 arm64 UUID 均为 `179E1B19-1215-33AD-8C9A-5E141F72A195`，签名通过。此轮未修改应用 Rust 行为，未重新操作原生窗口；桌面状态保留 `unverified`，没有把此前 `cgWindowNotFound` 改写成成功或永久不可用。

## 2026-10-15 使用仓库图像作为 macOS 应用图标

打包脚本从根目录 `icon.png` 生成多尺寸 PNG 图层并封装为 `Contents/Resources/OpenArchi.icns`，`Info.plist` 将 `CFBundleIconFile` 指向 `OpenArchi`。重建后的 `dist/OpenArchi.app` 中图标资源识别为 1024×1024 ICNS；Info.plist 校验通过。`./scripts/package-macos.sh` 成功，Release 与 bundle 内 arm64 UUID 均为 `179E1B19-1215-33AD-8C9A-5E141F72A195`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。Finder 首次打开 `dist` 时仍显示旧的通用图标缓存；打开“显示简介”后预览显示指定的 A 图标，返回文件夹后 Finder 图标视图也已刷新并显示该图标。未检查 Dock 显示。

## 2026-10-16 Dock 图标复核与修正

实际截图确认 Dock 显示的黑白图标是 eframe 默认图标，而 Finder 已显示仓库 `icon.png`。原因是打包资源虽已正确写入 `.icns`，Rust 启动配置仍未给 eframe 提供图标，启动时 eframe 会采用自带默认图标。`src/main.rs` 现在通过 `eframe::icon_data::from_png_bytes(include_bytes!("../icon.png"))` 解析同一张源图，并用 `ViewportBuilder::with_icon` 设置原生应用图标。另用 `iconutil -c iconset` 从成品 `.icns` 解包检查各个图层：16、32、128、256、512 像素图均显示红色 A。

验证：`./scripts/cargo.sh fmt --all -- --check` 与 `./scripts/cargo.sh check --locked --offline --all-targets` 通过；`./scripts/package-macos.sh` 成功。Release 和 `dist/OpenArchi.app/Contents/MacOS/openarchi` 的 arm64 UUID 均为 `F0D42325-DB48-3E6B-A236-A27E60A52680`；`codesign --verify --deep --strict dist/OpenArchi.app`、Info.plist 校验和 ICNS 解包均通过。退出先前运行的三个实例后，从新包启动单一实例并截取实际桌面：Dock 中 OpenArchi 图标与 Finder 一样显示红色 A。桌面窗口可打开；本次只验证图标，没有执行建模交互。

## 2026-10-16 Dock 图标透明底与尺寸调整

按 Dock 截图反馈处理 `icon.png`：将黑色圆角图标外的白底改为透明，并在 1254×1254 画布中居中缩小主体。透明像素占 42.33%，主体可见范围为 980×951 像素；黑色圆角底与 A 标志保留。用 `iconutil -c iconset` 解包新 ICNS 并检查透明边缘；重启打包应用后目视 Dock 截图，OpenArchi 红色 A 与相邻图标视觉尺寸接近，外侧没有白色方底。

`./scripts/package-macos.sh` 成功重建 [dist/OpenArchi.app](../dist/OpenArchi.app)，Release 与包内 arm64 UUID 均为 `DD047422-B4FF-3AE4-A118-6900BE3FC0E2`；`codesign --verify --deep --strict dist/OpenArchi.app` 和 Info.plist 校验通过。桌面窗口正常启动；本轮只检查图标。

### Polyline correction validation — 2026-10-16

Fresh workflow run `target/ac29-workflow/runs/20261015T175007Z-6b9534a2/report.json`: formatting, all-target cargo check, 115 Rust tests, 11 tooling tests, diff whitespace and macOS packaging passed. Bundle `dist/OpenArchi.app`; release/bundle arm64 UUID `5F844F1A-8E1B-32EB-B48C-0AD97E9098EE`; strict deep signature verification passed. Existing dirty workspace preserved; no commit.

Current native CUA: launched the final packaged app; P, two point clicks, right-click finish, V and two edge clicks produced a selected straight segment with its edge palette. The prior same-source palette capture visibly showed only two endpoint nodes, plus separate Insert node / Curve edge commands. Final app launch and palette entry passed. CUA drag attempts did not produce a visible committed arc; continuous drag preview and pencil appearance are therefore desktop-unverified, despite passing RawInput/geometry regressions. No Archicad runtime comparison was performed. The final change prioritizes an active polyline edit over guide gestures. Full AC29 equivalence remains unknown, including numeric radius input, straighten and edge-only offset. The missing source icon was recovered from the existing bundle's ic10 PNG resource to unblock compilation.

### 2026-10-16 — preserve arcs across continued whole-path offsets

Implemented exact support offset in `geometry::offset_polyline_with_arcs`: straight supports translate, circular supports remain concentric with adjusted radius; nearest line/circle or circle/circle intersections join segments. Arc metadata, direction and editable node count survive; sweep branch is recalculated from joined endpoints. Invalid radii, missing intersections, reversed segments and sampled path self-crossings reject the preview. The application takes the baseline from the current gesture-start project, including previous confirmed edits; an arc-context drag uses its local normal. Straight-only paths retain the prior offset routine. Source evidence: `src/app.rs::apply_polyline_drag_preview`, `src/geometry.rs::offset_polyline_with_arcs`; this is an original implementation of the user requirement, not new Archicad runtime evidence.

Fresh verification report: `target/ac29-workflow/runs/20261015T175904Z-4393a096/report.json`. Formatting, all-target check, 118 Rust tests, 11 tooling tests, whitespace check and packaging passed. Added geometry coverage for repeated arc offsets, mixed tangent line/arc joins, clockwise/counterclockwise paths, closed circular paths and radius collapse. RawInput coverage confirms curve → offset → offset, one transaction per gesture, undo/redo back to the confirmed curve, and JSON round-trip.

Deliverable: `dist/OpenArchi.app`; release and bundle arm64 UUID `FA7B879D-7FDF-3FE4-AD91-5FD53BFA1CBD`; strict deep signature passed. Desktop continuous editing was not manually verified in this change. Current CUA inspection found an existing running app with user edits (13 elements), which was left intact; no desktop edits or restart performed. New bundle is on disk; the existing process does not automatically load it. Self-crossing rejection uses view-independent arc tessellation, so near-tangent curve intersections remain a numerical limitation. No commits, tags or pushes.

## 2026-10-17 Grids 菜单与网格设置

按 Archicad 29 的 Standard Toolbar `Grids` 菜单实现 OpenArchi 的网格显示、捕捉模式、原点显示、正交/旋转网格、设置旋转网格和 `Grids & Background` 对话框。网格设置是项目文档中的类型化数据，平面与三维共用同一组间距、步数、原点、角度和颜色；对话框编辑持久草稿，确定提交一次可撤销的设置事务，取消/点击外部不改数据。编辑平面相关命令保留但禁用。行为契约与证据见 [Grids](features/grids.md)。

验证（2026-10-17 13:08 UTC，`target/ac29-workflow/runs/20261017T130803Z-d6e497e3/report.json`）：`python3 scripts/ac29-workflow.py verify` 通过格式（含 `src/grids_ui.rs` 的独立 rustfmt）、全目标 `cargo check`、**132 项 Rust 测试**、11 项工具测试、`git diff --check` 和 macOS 打包。`dist/OpenArchi.app` 的 release/包内 arm64 UUID 均为 `23A2BCE9-1470-3087-A872-755B2F66F1CC`，严格深度签名验证通过。

桌面实测（最终包、简体中文、演示项目）：工具栏 `网格` 菜单 14 项及顺序正确，编辑平面命令禁用；`网格与背景…` 可打开并用 Escape 取消；再次打开后以 Tab/Space 切换 `构造网格显示` 关/开，勾选 `旋转网格` 后 25% 预览立即变为 45° 网格，选择 `不捕捉` 并确定；状态显示 `Grids & Background applied`，标题变脏、撤销可用，平面网格保持正交显示；撤销后恢复原设置、清除脏标记并禁用撤销。CUA 对 egui 模态内部控件的指针点击未稳定送达，故对话框改用键盘焦点操作，这是工具限制而非应用路径。桌面未覆盖：CUA 向 DragValue 输入数字、模态内指针点击、Shift+S/Alt+S、方向键微移、设置旋转网格、点击外部关闭、JSON 保存重开和最终包的三维视图。未做本轮 Archicad 逐项交互对比；既有运行观测仅作为菜单顺序、默认值和对话框字段的证据。

## 2026-10-17 Trace & Reference 描图菜单

按 Archicad 29 INT 5101 的 `Trace` 工具栏开关、下拉菜单与 `View → Trace Options` 实现 OpenArchi 的楼层平面／整个模型描图参考。参考解析、显示变换、构件类型过滤、强度、统一颜色、分栏、置顶、透明填充、激活／互换、拖动／旋转／复位／临时偏移均接入实际平面绘制；参考不可选择或编辑。描图状态不进入项目 JSON、撤销历史或脏标记。行为契约、证据类型与缺口见 [Trace](features/trace.md)。

验证：`python3 scripts/ac29-workflow.py verify` 通过格式（含独立 `rustfmt --edition 2024 --check src/trace_ui.rs src/trace.rs`）、全目标 `cargo check`、**142 项 Rust 测试**（112 项库测试、7 项 Guides 集成测试、23 项既有工作流测试）、11 项工具测试、`git diff --check` 和 macOS 打包。新增覆盖相对楼层解析、Elevation 排序、整个模型、变换／复位、指针模式取消、启用时无下方楼层的最近楼层回退、参考与活动内容互换不改模型和撤销、RawInput 拖动只移动参考、Escape 恢复临时偏移，以及调色板／浏览对话框／分栏渲染不改模型。

`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `009A285C-2326-30F3-AF2C-9BCFFFACE4B6`；`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（2026-10-17，最终包 PID 54133，英文界面，演示项目 `Courtyard House`，12 个构件）：

- `Trace` 开关为 `on` 时状态显示 `Trace Reference on · 01 · First floor`，平面出现淡蓝色一层阳台，与当前零层内容区分明显。
- `Trace ▾` 菜单按顺序显示 `Choose Reference`、`Activate Reference`、`Switch Reference with Active`、`Drag Reference`、`Rotate Reference`、`Reset to Default Position`、`Rebuild Reference`、`Temporarily displace Reference`、`Show/Hide Splitter`、`Reference on Top`、`Transparent Fills and Zones`、`Turn off Trace everywhere`、`Trace & Reference`。
- `Window → Trace & Reference` 打开调色板；可见 `Show Reference`、`Reference: 01 · First floor`、强度滑块（120/255）、`Original colors`、旋转与 `Reset`、`Activate`/`Switch with Active`/`Drag`/`Rotate`/`Rebuild`/`Displace`/`Splitter`、`Reference on Top`、`Transparent Fills and Zones`、折叠的 `Visible Elements` 及不可用的 `Apply Settings to All References`。
- `Switch with Active` 后活动楼层变为 `01 First floor`、参考变为 `00 Ground floor`；状态栏构件数仍为 12，`Undo`/`Redo` 保持禁用。
- `Splitter` 开启后平面出现竖直分隔线，左侧参考、右侧活动内容，右上显示 `Active` 标签，调色板出现 `Splitter position` 滑块。
- `Drag` 进入 `Drag Reference · drag in the plan · Esc exits` 指针模式；按 Escape 后状态变为 `Trace pointer mode cancelled`，构件数与撤销状态未变。
- CUA 对 egui 浮动调色板控件的指针坐标映射不稳定，菜单和按钮通过逐次校准才命中；CUA 的 `drag` 手势未在平面中产生参考位移，因此参考拖动本身未获得桌面实证，仅有 RawInput 覆盖。桌面未覆盖：Rotate、Displace、Reference on Top、透明填充、可见构件过滤、颜色/强度调整、Reference 选择器浏览、`Turn off Trace everywhere`、关闭调色板、保存重开和三维视图。

## 2026-10-17 Quick Options 底栏与浮动调色板

Archicad 29 实机（29.0.0 v1 INT 5101，arm64 macOS，本机截图与菜单走查）确认：平面模型窗口底部默认是停靠在状态栏上方的横向 Quick Options 栏，不是浮动面板；另有独立的 `Window > Palettes > Quick Options` 浮动调色板，底栏与调色板的显隐互不控制。底栏从左到右为 Zoom、Orientation、Scale、Layer Combination、Partial Structure、Pen Set、Model View Options、Graphic Overrides、Renovation Filter、Design Options、Dimensions；默认值分别为 `5.9%`、`0.00°`、`1:100`、`02 Drafting`、`Entire Model`、`03 Architectural 100`、`03 Building Plans`、`No Overrides`、`00 Show All Elements`、`Main Model Only`、`Plain Meter`。`Document` 菜单顺序为 Layers、Floor Plan Scale…、Partial Structure Display…、Pen Sets、Model View、Graphic Overrides、Design Options、Renovation。证据类型与边界见 [Quick Options](features/quick-options.md)。

OpenArchi 已按该结构重写：底栏位于状态栏上方，包含上述 11 项原创矢量图标控件；`窗 > 隐藏/显示快捷选项栏` 只切换底栏，`窗 > 调色板 > 快捷选项` 只切换独立浮动调色板。11 项均打开同一组真实菜单，修改 `Project::plan_options`，一次变更进入一次撤销事务；图层组合、局部结构、模型视图选项、翻新过滤和设计选项也参与平面可见性。缩放与栏/调色板显隐是会话状态，不修改项目、不增加撤销步骤。

验证：`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、**150 项 Rust 测试**（120 项库测试、7 项 Guides 集成测试、23 项工作流测试）、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T153742Z-d1ec79b7/report.json`。回归覆盖 11 项设置与菜单共用状态、七项设置的一步撤销、图层与设计选项可见性、底栏/调色板独立显隐、方向 world/screen 往返和缩放状态文本。

`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `04848A83-4BDC-33DE-8469-316CAC7D2103`；`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（简体中文，`Courtyard House` 演示项目，12 个构件，平面视图）：

- 最终包放大窗口后，底栏 11 项全部落在窗口内，最右侧 `Plain Meter` 完整可见；`100.0%` 完整显示，长画笔名与翻新过滤名按 AC29 习惯显示省略号。最终包也复核了尺寸格式菜单可以打开。
- 在布局收紧前的同源构建上逐项打开 11 个菜单并核对内容（之后只调整了底栏宽度、字号和图标尺寸，菜单逻辑未改）：缩放含当前缩放、保存/重命名/删除与 10–1000% 预设；方向含图形拾取、`3.22°`、`183.22°`、`0°`、`90°`、`180°`、`270°`；比例含 Custom Scale 与 `1:300` 至 `1:5000`；图层组合含 `01 Site` 至 `10 Show 3D Zones as Solid` 和 `Show All Layers`；局部结构含 `Entire Model`、`Without Finishes`、`Core Only`、`Core of Load-Bearing Elements Only`；画笔集含 `01 Color Conceptual` 至 `09 Grayscale`；模型视图选项含 `01 Site` 至 `05 Sections and Elevations`；图形覆盖、翻新过滤、设计选项管理器和尺寸格式菜单均正常打开。
- 同一构建中，`窗 > 调色板 > 快捷选项` 打开右侧浮动调色板，底栏同时保留，状态为 `Quick Options palette shown`；调色板显示与底栏相同的 11 项及 `隐藏快捷选项栏` 按钮。
- 同一构建中，`窗 > 隐藏快捷选项栏` 后底栏消失、浮动调色板保留，状态为 `Quick Options bar hidden`；再次执行后底栏恢复、调色板仍在，状态为 `Quick Options bar shown`。
- 再次执行 `窗 > 调色板 > 快捷选项` 后调色板关闭、底栏保留，状态为 `Quick Options palette hidden`。两个显隐路径互不影响。

未完成项：CUA 对 egui 弹出菜单项的指针坐标映射不稳定，本轮只验证了菜单打开和菜单内容，未在最终包上通过菜单项完成一次值切换；值切换、共享状态、一步撤销和持久化由上述自动测试覆盖。调色板的关闭按钮未单独点击，使用 `窗 > 调色板 > 快捷选项` 完成关闭。未逐图标比对 AC29 专有图标外观（OpenArchi 使用原创矢量图），未验证 AC29 各属性对话框内部实现，底栏/调色板显隐也尚未跨重启保存。

## 2026-10-18 主窗口中英文切换

主窗口现在可从标题栏语言下拉框或 `View > Language` 在 English / 简体中文之间切换；选择立即刷新界面并写入 `~/Library/Application Support/OpenArchi/language`。语言切换只改变显示，不修改项目、选区或撤销历史。顶部下拉框与菜单入口现在都会把状态栏更新为当前语言下的 `Language · English` / `语言 · 简体中文`，避免残留上一语言的提示。行为与证据边界见 [主窗口语言切换](features/ui-language.md)。

验证：`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、**155 项 Rust 测试**（125 项库测试、7 项 Guides 集成测试、23 项工作流测试）、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T162257Z-8188aedd/report.json`。`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `6DC25E99-EB9A-35C9-9857-9C46F63E30F2`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（最终包，`Courtyard House` 演示项目，12 个构件，平面视图）：

- 从标题栏下拉框选择 `简体中文`：AX 树中的顶部菜单变为 `文件 / 编辑 / 视图 / 设计 / 文档 / 选项 / 协作 / 窗 / 帮助`，工具栏显示 `二维平面 F2`、`分屏视图`、`三维透视 F3`，视图标签显示 `分屏`、`南立面`，工具箱显示 `墙 / 楼板 / 楼梯 / 柱 / 梁`，状态栏显示 `就绪`、`选择构件或开始绘制`、`已选 0 个`、`12 个构件`，底栏 Quick Options 显示 `02 绘图`、`整个模型`、`03 建筑 100`、`03 建筑平面`、`无覆盖`、`00 显示所有构件`、`仅主模型`、`米制`。偏好文件为 `zh-CN`。
- 从 `视图 > 语言 > English` 切回：AX 树恢复 `File / Edit / View / …`、`Split view`、`South Elevation`、`0 selected`、`12 elements`、`READY`，状态栏显示 `Language · English`。偏好文件为 `en`。
- 选择 `简体中文` 后退出并重新启动最终包，首个 AX 树已经是中文，标题栏下拉框为 `简体中文`，证明语言偏好跨重启生效。
- 两次切换前后 `Undo`/`Redo` 均保持禁用，构件数保持 12，项目未因语言切换产生编辑或撤销步骤。

未完成项：本轮只逐项核对主窗口、顶部菜单/工具栏、工具箱、视图标签、状态栏与 Quick Options 底栏；其他对话框未在最终包上逐语言复核。CUA 对该 eframe 窗口的坐标点击返回 `noWindowsAvailable`，桌面点击改用系统辅助功能点击并记录窗口屏幕矩形后换算坐标；观察到的应用结果不受影响。未进行 Archicad 29 语言切换的运行时对照。

## 2026-10-18 墙设置与属性复刻

按 Archicad 29 的 `⌘T` 墙设置结构实现可复用的原生 Rust 墙属性编辑器：选中墙打开墙设置，未选墙且墙工具激活时打开墙默认设置；六个分组均带本地化标签、类型化草稿、校验、Apply/OK/Cancel/Escape、项目持久化和一步撤销。高度、厚度、楼层、图层、翻新状态、墙体位置、参考线偏移及顶/底部偏移同步到平面与三维；轮廓、平面覆盖、表面、结构分析、分类和属性值作为元数据保存，未声称生成本体几何或分析方法。行为与证据边界见 [墙设置与属性](features/wall-settings.md)，语言入口见 [主窗口语言切换](features/ui-language.md)。

验证：`./scripts/cargo.sh test --all-targets` **164 项通过**（133 项库测试、7 项 Guides 集成测试、24 项工作流测试）；`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T171001Z-37369a54/report.json`。`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `9AAE1B31-4869-3AB8-97B0-176E92519638`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（最终包，简体中文恢复后，`Courtyard House`，12 个构件）：

- 选中 `Wall 005`，`⌘T` 打开墙设置；高度由 `3.000 m` 改为 `4.250 m`，点 Apply 后状态为 `墙设置已应用 · 平面图与三维视图已同步`，项目出现未保存标记且 `Undo` 启用；OK 关闭对话框。一次 Undo 后重新打开设置确认高度恢复 `3.000 m`。
- 在三维透视中对同一面墙重复高度修改，应用后该墙明显高于其余墙；一次 Undo 后恢复统一高度。该检查是可视同步验证，不是数值几何审计。
- 最终包上六组标题均可见：`几何与定位`、`轮廓偏移修改器`、`平面图与剖面`、`模型`、`结构分析参数`、`分类与属性`；英文为 `GEOMETRY AND POSITIONING`、`PROFILE OFFSET MODIFIERS`、`FLOOR PLAN AND SECTION`、`MODEL`、`STRUCTURAL ANALYTICAL PARAMETERS`、`CLASSIFICATION AND PROPERTIES`。第一组的字段、枚举值与按钮在两种语言下逐项读取；其余五组在最终包上只核对标题，字段由类型化草稿、校验、持久化与回归测试覆盖。
- 语言切换实测：标题栏选择器完成中文→English，`视图 > 语言` 完成 English→简体中文；同一重启实例上再次双向切换。切换前后均保持 `已选 1 个`/`1 selected`、`12 个构件`/`12 elements`，`Undo`/`Redo` 均为禁用；选择 `简体中文` 后退出并重启最终包，首帧已为中文。

未完成项：最终包桌面未逐字段编辑其余五组，也未与 Archicad 29 做逐值运行时对照；CUA 观察没有 REA Evidence ID。轮廓生成、平面覆盖/投影裁剪、表面纹理、结构分析求解、砖量/成本公式和顶部链接几何仍明确为元数据或未实现。未提交、推送、打标签或发布。

## 2026-10-18 选中墙体的参考线高亮

按 Archicad 29 选择墙后的可见样式实现 OpenArchi 的墙体基线与端点高亮：选中墙使用淡绿 RGB `195, 213, 155` 填充、深绿轮廓、纯蓝 RGB `0, 0, 245` 的实体参考线（宽度 2.5）和两个 6 px 黑色方形端点。蓝色基线取墙体的 `start/end` 参考线锚点；`reference_offset` 只移动墙身。多选时先画全部选中墙填充，再统一画基线，避免相邻墙互相遮盖。形状绘制封装在 `paint_selected_wall_baseline`，模型侧入口为 `geometry::wall_reference_line`，供后续平面视图或预览复用。行为契约、证据和未知项见 [墙体选择参考线](features/wall-selection.md)。

Archicad 29 观察：本机 `29.0.0 v1 INT (5101)`，SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。CUA 截图显示选中墙为淡绿填充、深绿轮廓、蓝色参考线和黑色方形端点，未选墙没有蓝线与端点；Info Box 为 `Reference Line Location: Core Outside`。截图位于会话临时路径 `/tmp/ac29-wall-selected2.png`、`/tmp/ac29-wall-unselected.png` 和两个局部放大图；CUA 观察没有 REA Evidence ID，也未修改 Archicad 项目。

验证：`./scripts/cargo.sh test --all-targets` **166 项通过**（135 项库测试、7 项 Guides 集成测试、24 项工作流测试）；`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、锁定测试、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T174645Z-2cb2d847/report.json`。`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `A47040EE-D8A9-3601-AC75-67026B6A0354`，release SHA-256 为 `34f14f9d24d501f63b443e0ade610777d0866c53c0c8967f42154b4d4fe40837`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（最终包 `dist/OpenArchi.app`，简体中文，`Courtyard House`，12 个构件）：在平面中选择左侧外墙后，状态为 `已选 1 个`；截图可见墙体淡绿填充、深绿轮廓、贯穿整面墙的蓝色参考线以及上下两端黑色方形端点。精确截图中纯蓝基线位于 x≈901–903、y≈785–1406，与墙体参考线端点一致；取消选择前的普通平面没有该蓝线。该检查是最终包的可视回归，不是对 AC29 的像素级或数值几何对照。

未完成项：未在最终包上逐项比较多选、锁定/分组墙、悬停和不同 DPI 缩放；AC29 只观察了 `Core Outside` 一种参考线位置，未取得其精确画笔表定义或 REA Evidence ID。未提交、推送、打标签或发布。

## 2026-10-18 参考线位置区分内外墙

用户反馈：修改墙的基线位置后，平面图上墙体没有变化；墙需要能区分内外。Archicad 29 复核确认：`Reference Line Location` 的 `Center / Inside / Outside` 改变的是墙身相对参考线的位置，蓝色参考线仍固定在 `start/end` 基准线上。OpenArchi 之前只应用了数值 `reference_offset`，没有叠加位置预设，所以切换 `Inside/Outside` 时墙身不动。

修改：`WallSettings::body_offset(thickness)` 返回 `reference_line.preset_offset(thickness) + reference_offset`；`geometry::footprint` 与 `plan_wall_footprints` 改用该偏移，`geometry::wall_reference_line` 仍返回固定的 `start/end`。新增回归 `geometry::marquee_tests::wall_inside_and_outside_locations_move_the_body_not_the_line`，覆盖 `Inside`、`Outside + 0.1 m` 以及基线不移动。

验证：`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、**167 项 Rust 测试**、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T175558Z-19472aec/report.json`。`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `A6FCD002-EF9A-32DC-8E7A-D628912467E2`，release SHA-256 为 `bb3d12853f63190a4b4095efa6ca609ada460a1282b4a9025628587ebff14e4e`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（最终包 `dist/OpenArchi.app`，简体中文，`Courtyard House`，12 个构件）：选中 `墙 005`（厚度 `0.220 m`），用 `⌘T` 打开墙设置，依次切换 `参考线` 为 `居中`、`内部`、`外部`，每次 Apply/OK 后在平面同一扫描线（物理 y=1000）采样：

- `居中`：墙身填充 x=895..908，蓝色基线 x=900..903，墙身中心 901.5。
- `内部`：墙身填充 x=886..898，蓝色基线 x=901..903，墙身中心 892.0。
- `外部`：墙身填充 x=906..917，蓝色基线 x=900..903，墙身中心 911.5。

三种状态下蓝色基线位置基本不变，墙身中心分别向参考线两侧移动约 9.5 px，对应半墙厚 `0.11 m`；状态栏显示 `墙设置已应用 · 平面图与三维视图已同步`。一次 Undo 后墙身回到居中，重新打开墙设置确认 `居中` 已恢复，项目未保存标记消失。截图保存在 `/tmp/openarchi-center-closed.jpg`、`/tmp/openarchi-inside-closed.jpg`、`/tmp/openarchi-outside-closed.jpg`。

限制：只检查了一面墙、一个缩放比例和单墙选择；CUA 截图没有 REA Evidence ID，未比较其他 DPI、多选或 AC29 像素级画笔定义。未提交、推送、打标签或发布。

## 2026-10-18 Window > Palettes > Info Box 墙属性

按 Archicad 29 的 `Window > Palettes > Info Box` 交互复刻单面墙的常用属性横条：选中一面墙且信息框开启时，显示墙 ID、`全部已选: 1`、图层、结构功能、几何方法、参考线、参考线偏移、厚度、高度、顶部/底部偏移、顶部链接、结构、`平面图与剖面…`、`设置…` 和 `应用`。编辑通过 `WallSettingsDraft::apply_to_element` 与 `Editor::replace_element` 写入共享模型，平面图和三维透视读取同一墙体；一次成功提交只产生一条撤销记录。切换选择、隐藏信息框、打开 `⌘T` 或设置对话框前会先提交未完成草稿；校验失败时保留旧墙和错误文本。行为契约、字段来源和明确排除项见 [Info Box 墙属性](features/info-box/contract.md)。

自动验证：`./scripts/cargo.sh check --all-targets` 通过；`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、**176 项 Rust 测试**（145 项库测试、7 项 Guides 集成测试、24 项工作流测试）、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T181434Z-5696ce98/report.json`。新增 9 项 Info Box 回归覆盖单墙选择跟踪、中英文渲染、显隐、厚度/高度/参考线提交、一次撤销、非法厚度保留模型、切换选择提交旧草稿、`⌘T` 前提交草稿，以及墙面洞口打开对应分组。

`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `71A268CE-CC04-3D6F-B1E8-E6D8B347566B`，release SHA-256 为 `e07dac769dfed0a420a86bec8051e64944bc0d48d715e37bffb53337da45728e`，bundle tree SHA-256 为 `7e685dbefe818913786ee920d0102cc504e7053238b3664d30d03a8077b4287c`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（最终包的同内容唯一 Bundle ID 测试副本，简体中文，`Courtyard House`，12 个构件；测试副本只改 `CFBundleIdentifier` 并重新 ad-hoc 签名，用于让桌面自动化绑定新实例，不改变二进制内容）：

- 在平面图选中 `墙 009` 后，信息框立即出现 `墙 009`、`全部已选: 1`、图层 `03 建筑`、结构功能 `未定义`、几何方法 `单线`、参考线、厚度 `0.220 m`、高度 `3.000 m`、顶部/底部偏移、顶部链接、`Generic Wall/Shell`、`平面图与剖面…`、`设置…` 和 `应用`。
- 将高度从 `3.000 m` 改为 `3.500 m` 并回车，状态栏显示 `信息框墙体设置已应用 · 平面图与三维视图已同步`，`撤销` 启用；切到三维透视后该墙明显高于相邻墙。一次 `⌘Z` 后三维恢复统一高度，信息框随撤销清除的选择一起关闭。
- 依次选择 `内部`、`外部` 和参考线偏移 `0.200 m`，平面截图中蓝色参考线在物理 y=140–145 保持不变，墙身随参考线位置和偏移移动；截图保存在 `/tmp/ref-inside-crop.png`、`/tmp/ref-offset-crop.png`、`/tmp/ref-outside.png`。这验证了“修改基线位置”作用于墙身而非移动基线本身，也验证了内外墙位置预设。
- `设置…` 打开墙设置并带入当前值；`平面图与剖面…` 打开同一对话框且滚动后可见 `平面图与剖面` 分组已展开；Esc 取消后状态为 `墙设置已取消`。
- `窗 > 调色板 > 信息框` 子菜单可打开并显示带选中标记的 `信息框` 项；本轮 CUA 对该 egui 弹出菜单的点击会关闭菜单但未触发切换，因此菜单项的桌面切换动作未完成。开关逻辑由 `info_box_renders_wall_fields_and_follows_palette_visibility` 和 `toggle_info_box_visibility` 的单元/渲染回归覆盖，不能把菜单项点击记为桌面通过。

语言：标题栏选择器完成 `简体中文 → English → 简体中文`，状态栏分别显示 `Language · English` 和 `语言 · 简体中文`，信息框字段同步切换；切换前后均为 12 个构件，模型、选择和撤销/重做可用性不变。选择 `简体中文` 后退出并重启测试实例，首帧信息框和主窗口即为中文。英文墙字段的逐项渲染由 `info_box_renders_wall_fields_and_follows_palette_visibility` 覆盖；最终包桌面只逐项读取了中文墙字段。`视图 > 语言` 子菜单已打开并显示 `English / 简体中文`，但本轮 CUA 无法把点击送入该弹出菜单；上一轮记录的 `视图 > 语言` 桌面切换不被本轮反驳，也不作为本轮新证据。

限制：未在最终包上逐字段编辑图层、结构功能、几何方法、顶部链接和结构名称；这些字段的持久化与校验由类型化草稿和回归覆盖。CUA 观察没有 REA Evidence ID；未比较 AC29 的像素级控件尺寸、浮动面板停靠或所有下拉选项。桌面菜单项激活缺口已明确标为未验证。未提交、推送、打标签或发布。

## 2026-10-18 Info Box 墙工具默认设置

在已复刻的 `Window > Palettes > Info Box` 单墙属性之外，补齐 Archicad 29 在墙工具激活、尚未生成墙体时的 `Default Settings` 状态。信息框可直接编辑下一面墙的图层、几何方法、参考线及偏移、结构、厚度、高度、顶部链接和顶部/底部偏移；有效修改写入会话级墙工具默认值，不产生撤销记录，随后生成的墙继承这些值。`⌘T`、`设置…` 与 `平面图与剖面…` 在无选择时均打开 **Wall Default Settings**。行为契约、AC29 工具态证据和逐项验收见 [Info Box 墙属性](features/info-box/contract.md)，默认值链路见 [墙设置](features/wall-settings.md)。

自动验证：`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标 `cargo check`、**181 项 Rust 测试**（150 项库测试、7 项 Guides 集成测试、24 项工作流测试）、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T184754Z-e03c76ce/report.json`。本轮新增 5 项回归：墙工具态中文渲染；默认值应用到下一面墙（图层、几何方法、参考线、偏移、厚度、高度、顶部链接、结构名称和上下偏移）；非法厚度不改变默认值；`⌘T` 先提交待处理默认值；两个设置按钮打开默认设置对话框。语言文件去重后又执行了一次 `verify --force`，产物哈希以本节末尾的最终包为准。

`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `C64008A3-1EA6-3CE7-8B11-0A469EF35FB6`；release SHA-256 为 `86a21e6be1b7a824332031b57332596e7df5479db3609dec55fc938a3e1a9ec3`，bundle tree SHA-256 为 `81145d5491ccd33abe2d3dc824e0f0eb5229d66d8574007286b85a5b7f5f1556`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

桌面实测（打包前的同内容构建、仅改 `CFBundleIdentifier` 并重新 ad-hoc 签名的测试副本，简体中文，`Courtyard House`，12 个构件；随后只删除了语言文件中重复的同值键并重新打包，渲染字符串与行为不变）：

- 墙工具激活且无选择时，信息框显示 `墙`、`默认设置`、图层 `03 建筑`、几何方法 `单线/连续/多边形/矩形`、参考线 `居中/内部/外部`、参考线偏移、结构 `Generic Wall/Shell`、厚度、高度、顶部链接、顶部/底部偏移、`平面图与剖面…`、`设置…` 和禁用的 `应用`。
- 将厚度由 `0.220 m` 改为 `0.360 m`、高度由 `3.000 m` 改为 `4.200 m`，选择参考线 `外部` 并把偏移改为 `0.150 m`；状态栏显示 `墙默认设置已应用 · 新墙将使用这些值`。切到箭头工具再按 `W` 返回墙工具后，这些值仍保留；`撤销` 保持禁用，构件数仍为 12。
- `设置…` 打开 `墙默认设置`，带入修改后的 `4.200 m`、`0.360 m`、`外部` 和 `0.150 m`；取消后状态为 `墙设置已取消`，模型未变。
- 在平面放置起点后，用追踪器输入长度 `4.000` 并回车，生成 `墙 013`。信息框切换到所选墙形态并显示 `外部`、`0.150 m`、`0.360 m`、`4.200 m`、`1. First floor` 和 `Generic Wall/Shell`；构件数变为 13，`撤销` 启用，标题出现未保存标记。测试副本生成的墙未保存，最终包和用户工程未被修改。

语言：墙工具态的中文字段已逐项读取。英文补验在同内容测试副本上完成：把语言偏好写为 `en` 后重启，首帧即为英文，信息框逐项显示 `Wall`、`Default Settings`、`Layer:`、`Geometry Method:`（`Single / Continuous / Polygonal / Rectangular`）、`Reference Line:`（`Center / Inside / Outside`）、`Reference Offset`、`Structure:`、`Thickness`、`Height`、`Top Link:`、`Top Offset`、`Bottom Offset`、`Floor Plan and Section…`、`Settings…` 和 `Apply`。点击 `Outside` 后状态栏显示 `Wall default settings applied · New walls use these values`，构件数仍为 12，`Undo`/`Redo` 仍禁用；`Settings…` 打开英文 `Wall Default Settings`（`GEOMETRY AND POSITIONING`、`Top Link`、`Height`、`Thickness`、`Reference Line: Outside`、`Cancel / Apply / OK`），Esc 后状态为 `Wall Settings cancelled`。验证后退出测试实例并把偏好恢复为 `zh-CN`。CUA 对 egui 弹出组合框的点击仍不稳定，英文补验因此直接写偏好文件并重启，未新增标题栏或 `视图 > 语言` 的选择器证据；图层下拉选项的桌面选择仍未实测，默认值继承由回归覆盖。

限制：未在桌面逐字段修改图层、顶部链接和结构名称，未在工具态输入非法值；这些路径由类型化草稿、渲染回归和单元测试覆盖。英文工具态已逐项读取，但语言切换本身来自偏好文件写入与重启，不是桌面选择器点击。CUA 观察没有 REA Evidence ID；未做 AC29 像素级控件尺寸或复合结构弹出层的 OpenArchi 对比。未提交、推送、打标签或发布。

## 2026-10-18 墙的四种几何方法

墙工具的 `单线 / 连续 / 多边形 / 矩形` 现在对应四套独立输入状态机，而不是同一套两点交互换标签。行为契约、AC29 帮助证据和限制见 [墙几何方法](features/wall-geometry-methods.md)。

桌面实测（最终包 `dist/OpenArchi.app`，简体中文，`Courtyard House`，12 个既有构件；未保存用户工程）：

- 单线：点击起点后，用追踪器输入长度 `4.000 m` 并回车，生成一段墙，状态为 `已创建构件 016`；一次撤销移除。
- 连续：用追踪器逐点添加四个节点并完成，状态为 `已创建 4 段连续墙`，构件 12 → 16，相邻墙共享输入节点；一次撤销恢复 12。
- 矩形：点击一个角和对角，生成四条世界轴对齐墙，状态为 `已创建 4 段矩形墙`，构件 19 → 23；撤销移除整组。
- 多边形：用追踪器逐点添加轮廓并闭合，状态为 `已创建 7 段多边形墙`，构件 12 → 19；一次撤销恢复 12。
- 每种方法绘制期间的平面预览与对应结果一致，完成后平面和三维同步显示。测试后恢复为墙工具、连续方法、12 个构件、无未保存标记、撤销禁用。

自动验证：`./scripts/cargo.sh fmt --all -- --check` 通过；`./scripts/cargo.sh test --all-targets` **189 项通过**（153 项库测试、7 项 Guides 集成测试、29 项工作流测试）；`python3 scripts/ac29-workflow.py verify --force` 通过格式、`cargo check --locked --all-targets`、189 项 Rust 测试、11 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T191638Z-fec311ce/report.json`。`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `F923A6C0-852A-36C9-9E87-8F9524477B2C`；release SHA-256 为 `09bd7c41c5c6f0a148efe8557756c048df70fa48a4caca58ab7054579d47a315`，bundle tree SHA-256 为 `44c1da88d8effd71223222267f0ee5bfae3b1b575b245eaf5b78aa53887e486f`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过。

语言实测（同一最终包）：标题栏选择器完成简体中文 → English；英文下逐项读取 `Wall`、`Geometry Method`、`Single / Continuous / Polygonal / Rectangular`、`Reference Line: Center / Inside / Outside`、`Reference Offset`、`Thickness`。`视图 > 语言` 完成 English → 简体中文，恢复 `墙`、`几何方法`、`单线 / 连续 / 多边形 / 矩形`、`参考线：居中 / 内部 / 外部`、`参考线偏移`、`厚度`。两次切换前后构件数均为 12、选择均为空、项目均无未保存标记，撤销/重做可用性未因切换改变。选择简体中文后退出并重启最终包，首帧为中文（标题栏 `简体中文`、菜单 `文件 / 编辑 / 视图`、状态 `12 个构件`）。

未完成项：连续、矩形和多边形结果仍是多段 `Shape::Wall`，没有 AC29 单一 PolyWall 元素的参考边、曲边、pet palette、自交警告和专用表面/复合结构限制；连续方法没有弧形段选择；旋转矩形不在本轮四种方法内；修改已有墙的几何方法不会转换既有几何；整条链或整圈在提交后不能作为一个逻辑组重选。AC29 对照依据 Graphisoft 公开帮助，本轮未用 REA 捕获 AC29 运行时交互，因此没有 REA Evidence ID。未提交、推送、打标签或发布。

## 2026-10-18 macOS / Windows 0.1 打包

新增跨平台打包：macOS 继续生成本地 ad-hoc 签名的 `.app`，Windows 通过 Rust 1.98.1、`x86_64-pc-windows-msvc` 与 cargo-xwin 0.23.1 在 macOS 上交叉编译为 GUI 子系统 PE32+。平台相关路径和标签已分离：macOS 语言偏好仍写入 `~/Library/Application Support/OpenArchi/language`，Windows 使用 `%APPDATA%\OpenArchi\language`；侧栏渲染器标签在 macOS 显示 `Apple Silicon / Metal`，Windows 显示 `Windows x64 / DirectX 12`。Windows 可执行文件不弹出控制台窗口。

产物：

- `dist/OpenArchi-0.1-macOS-arm64.zip`（内含 `OpenArchi.app`），SHA-256 `cfc677ebeffedbc686e5b7e96daa1848a23a1135eec3a98af6802ddf7831d4e4`。
- `dist/OpenArchi-0.1-Windows-x64.zip`（内含 `OpenArchi.exe`、README、LICENSE），SHA-256 `8be9b77deb2d9a3c314bccd6fec62e38647d1acaeb7f0c9cd008b8a4d9a55130`；`OpenArchi.exe` SHA-256 `a341ca61f174a54d54583401ec8fad590f065f56507772d6b454c0b6db1046be`。
- macOS 包内 `CFBundleShortVersionString` 为 `0.1`；release 与包内 arm64 UUID 均为 `58A6E014-97DB-3A15-8403-E8D3EF950327`，release SHA-256 为 `8d45c32aa0ce5603979548b681dfc5d601b42b32dcf3002b66b3a2e8c84cc2f9`，bundle tree SHA-256 为 `da361d5e3045482842fb77b77c98eb6a8995b5d813b58d3f802cda2d11b7f2f5`，`codesign --verify --deep --strict` 通过。

验证：`cargo test --all-targets` **189 项通过**（153 库、7 Guides、29 工作流）；`python3 scripts/ac29-workflow.py verify --force` 通过格式、全目标检查、189 项 Rust 测试、11 项工具测试、`git diff --check` 与 macOS 打包，报告为 `target/ac29-workflow/runs/20261017T195808Z-285cf98e/report.json`。macOS 最终包已启动，简体中文首帧正常，显示 12 个构件和 `Apple Silicon / Metal`。Windows 产物只做了 PE 类型、压缩包内容与哈希检查，**未在真实 Windows 或 Wine 中启动验证**；Windows 包未签名，也没有安装器。

## 2026-10-18 同材质墙交会轮廓与构造网格默认关闭

本轮修复两项：同材质墙体相交时平面填充连续、交会处不再出现内部墙端线；新项目默认隐藏构造网格并关闭网格捕捉（`GridSnapMode::None`）。

自动验证：`python3 scripts/ac29-workflow.py verify --force` 通过格式、`cargo check --locked --all-targets`、**260 项 Rust 测试**（含 1 项性能基线忽略）、15 项工具测试、`git diff --check` 和 macOS 打包，报告为 `target/ac29-workflow/runs/20261018T102915Z-cfd2a361/report.json`。新增回归 `same_material_wall_join_has_no_internal_plan_outline_across_renovation_groups` 和 `construction_grid_and_snap_are_off_by_default`；原先依赖旧默认值的捕捉测试改为显式开启网格捕捉。`dist/OpenArchi.app` 的 release 与包内 arm64 UUID 均为 `D0FD5583-C853-3379-9E3F-50329DF4C803`，release SHA-256 为 `6f9150fc286acc675171c5637590c61730b2eaf91c756fe209bfb49b15b359fa`，bundle tree SHA-256 为 `56860af12c613d6605fa14db5453bd2b3cf66eb701f0d2ff6697dca085adb5b5`，`codesign --verify --deep --strict dist/OpenArchi.app` 通过；`dwarfdump --uuid` 的 release 与包内可执行文件 UUID 一致。该报告早于本轮纯文档收尾，源码与包未再变化。

桌面实测（最终包 `dist/OpenArchi.app`，隔离 HOME `/tmp/oa-join-final.AFg79r/home`，英文界面，恢复项目 `Same Material Join`）：

- 模型为两面 `Plaster` 墙：`Wall 001=(0,0)→(4,0)`、`Existing`、厚 `0.2 m`；`Wall 002=(2,0)→(2,2)`、`New`、厚 `0.2 m`。恢复后平面立即显示 T 形交会，`2 elements`，未保存标记存在。
- 平面截图 `/tmp/join-recovered.png`（3024×1976）：横向墙填充在交会处连续；在填充行 y=1330 上，x=1330–1439 全部为 `(211,211,211)`，没有内部深色竖线。横向墙顶边在 y=1318–1322 处按竖向墙身范围 x=1368–1395 断开，竖向墙两侧轮廓在 y=1322 终止，不再穿入横向墙内部。
- 同一截图中平面画布为空白纸面，没有构造网格线；两个空白采样区（x=1760–2320，y=850–1700；x=650–2320，y=1400–1700）非背景像素均为 0。恢复项目的 `grid_settings` 序列化为 `show_construction_grid=false`、`grid_snap_enabled=false`、`snap_mode=None`。
- 限制：自定义 `View` 菜单未能通过本轮自动化稳定展开，因此没有直接读取 `Snap Options > Grid Snap` 复选框；网格捕捉关闭由项目状态、画布空白和自动回归共同支持。未重复检查不同材质、不同墙高、旋转墙或 AC29 像素级画笔画定义；3D 只沿用既有端帽隐藏回归，本轮未对这两个测试墙截图三维交会。未提交、推送、打标签或发布。

## 2026-10-24 图层设置对话框（AC29 Layer Settings 复刻）

本轮交付动态图层系统与 AC29 风格图层设置对话框：Cmd+L / Options > Element Attributes > Layers… / Quick Options 图层菜单三个入口；左侧图层组合（选择即预览、Update 写回、New… 新建），右侧搜索 + 专业分类树 + 状态表（眼睛/锁/三维模式图标、扩展=相交组编号、Show: X of Y、New/▾/Delete）；OK 提交（被删图层元素迁往 Archicad Layer，与 AC 的 catch-all 语义一致），Cancel/Esc 丢弃；全部改动经 `ProjectHeader.layers` 可撤销；锁定图层禁止编辑/删除/放置；隐藏图层不渲染但墙相交清理仍按帮助语义参与；同非零相交组才清理墙端。图标为原创矢量绘制（复刻 AC29 观察到的形态，未拷贝资源）。行为契约与证据见 `docs/features/layer-settings/contract.md`。

自动验证：`cargo fmt`/`cargo check` 干净；`./scripts/cargo.sh test --all-targets` **301 项通过**（252 库、7 Guides、10 工作流、1 性能基线忽略项之外的新布局用例、31 集成等，含新增 4 个图层对话框用例、相交组回归 `collinear_walls_on_different_intersection_groups_keep_end_caps`、legacy 图层 JSON 迁移与锁定守卫测试）；`check-localization` 通过；`python3 scripts/ac29-workflow.py verify --force` PASSED（test_count 300、tool_test_count 15，含打包 receipts）。

打包：`dist/OpenArchi.app` release 与包内 arm64 UUID 均为 `1CA89A55-D18D-35D0-8E31-F8F5AA25886D`，`codesign --verify --deep --strict` 通过。

桌面冒烟（隔离 HOME `/private/tmp/oa-layer-home`）：英文实例 Cmd+L 打开对话框、面板/图标/OK/Cancel 正常（`/private/tmp/ac29-layers/openarchi-layers-en.png`）；写偏好 `zh-CN` 重启后首帧窗口标题「OpenArchi — 建筑设计工作台」，Cmd+L 对话框全部控件中文。首轮截图发现组合列表第 3 行被滚动条遮挡——根因是 egui `ui.horizontal` 把行高钳到当前行高、模态内滚动区塌缩（组合列表视口仅 252×112、表格 440×64，临时探针测试实测）；改为 `allocate_ui(718×420)` + 两个 `scope_builder` 定宽定高侧栏后复测视口 252×280/456×252，重新打包复验：10 个组合全部完整可见、无遮挡、对勾+高亮在 03 平面 - 初步（`/private/tmp/ac29-layers/openarchi-layers-zh-final-v3.png`，AX 树 271 元素含「图层 (模型视图)/图层组合/搜索图层/显示: 2/32/确定/取消」佐证）。语言切换仅改变界面文案，未改变构件、选择、撤销历史。

限制与 unknown：AC29 New 分裂菜单与对话框左上菜单内容未运行时观察（像素点击有崩溃风险，见契约 Risks：调查中目标 Archicad PID 6914 曾在合成点击后 FATAL 退出，已立即重启并停止该类交互，恢复对话框留给用户）；闭眼/闭锁图标像素形态按帮助语义推断；图层数据名（模板图层/组合名键之外的 "Archicad Layer" 等）按项目数据处理不随界面语言翻译；顶部工具栏同步开关未证实未实现。未提交、推送、打标签或发布。


## 2026-10-28 — Edit menu implementation (partial AC29 coverage)

See [behavior contract](features/edit-menu/contract.md) and [command coverage, limits and verification](features/edit-menu/coverage.md). Added shared native/egui catalog, validated atomic edit commands, grouping/locking, transform/copy, alignment/distribution, numeric reshape and non-mutating preview. No global Edit-menu equivalence claim: 12 commands remain disabled and several implementations intentionally support bounded model types/workflows.

Final fresh forced verification: 324 Rust tests passed, 1 existing ignored performance test; 15 tooling tests passed; format, compile, localization, diff and package checks passed. App `dist/OpenArchi.app`; arm64 release/bundle UUID `A93D50AE-DC6E-34FB-8468-FE928B4EBB94`; deep strict signature passed. Final receipt under `target/ac29-workflow/runs/` (2026-10-28 07:36 UTC).

Desktop **unverified**: isolated-HOME packaged app launched; CUA returned `-10005: cgWindowNotFound` for both bundle-id and full-path binding. Bilingual Drag OK/Cancel, Escape and model/history invariance passed RawInput tests; these are not native desktop acceptance. Actual language switch/restart and remaining dialogs unknown. Existing title-bar language picker was removed by earlier native-title work; only View > Language is present. Reusable lesson candidates are in coverage.md, pending developer confirmation; AGENTS.md was not edited by this task.


## 2026-10-28 — AC29 Edit keyboard shortcuts

[Shortcut contract and exact mapping](features/edit-menu/shortcuts.md). Read installed Standard Shortcuts 29 global/non-text XML; active user-custom scheme unknown. Replaced legacy Cmd+D Duplicate with Drag, added transforms/copies/grouping/selection and standard Cmd+Alt+Z redo, retained Cmd+Shift+Z alias, and unified menu hints. Native-style Copy/Cut/Paste events handled, including Cmd+Alt+C injection. Text focus/modal safety and exact modifiers tested.

Fresh verification: **332 Rust + 15 tooling tests passed**, 1 existing ignored performance test. Formatting, compile, localization, diff and packaging passed. `dist/OpenArchi.app` UUID **A89C6B50-EEF7-350B-8A11-0902CCA10313** matches release; deep strict signature passed. New isolated-HOME English instance launched (PID 67397); CUA full-path bind failed with **-10005 cgWindowNotFound**. Actual desktop shortcuts, bilingual language switches and Chinese restart remain unknown. Existing missing title-bar selector remains absent. No full native localization claim.


## 2026-10-28 — 编辑快捷键验收边界纠正（研究/记录）

此前 332 Rust / 15 工具测试的通过仅证明当时源码的测试结果；按键路由到数值对话框不能证明 AC29 鼠标驱动的移动/旋转/镜像正确。当前这一用户目标仍未完成。已查询官方帮助并保存[独立交互契约](features/edit-menu/pointer-interaction.md)与[171 条标准快捷键](features/edit-menu/standard-shortcuts-full-29.md)。本轮仅文档与配置事实提取：无新源码变更、无新 app 构建、无新增 runtime observation 或 REA Evidence。文档存在性、配置计数、JSON 与本地链接另行核对；不复用历史测试为新增交互背书。


## 2026-10-28 — Cmd+D 直接鼠标移动，取消数值对话框

- `dispatch_edit` 的 Drag / Drag a Copy / Drag Multiple Copies 进入 `PointerDrag`。平面轮廓和三维当前楼层水平面预览不改项目；单击提交，Esc/右键取消当前预览。Option 单副本，Option+Cmd（兼容物理 Ctrl）多副本，模式锁存；逐份 Undo，近位置双击结束。具体假设和限制见 [交互契约](features/edit-menu/pointer-interaction.md)。旋转、镜像对话框不在此次修正范围。
- 新增/更新 egui 事件测试：Cmd+D 无模态、两种语言、鼠标移动不改模型/撤销历史、点击位移、取消、单副本锁存、多次放置、锁定拒绝、序列化、Undo、三维水平面。已通过完整 `python3 scripts/ac29-workflow.py verify --force`：336 Rust tests passed、1 ignored performance test；15 tooling tests passed；格式、check --locked --all-targets、本地化检查、diff 检查通过。工作区既有 unused import / unused_mut 警告未扩大修改。
- 新包 `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`；release/bundle arm64 UUID 均 `20722E96-CE86-3AD6-9C90-30AEB71F7628`，codesign deep strict 通过。源码新鲜度 receipt：`target/ac29-workflow/runs/20261028T090926Z-8ee66de6/report.json`；随后 bundle 核验 `matches_successful_build_receipt`。
- 桌面：用隔离 HOME `target/pointer-drag-smoke-home`（预写 en）启动最终包，PID 10931 的 HOME 已由 ps 证实。CUA 按应用名和最终包路径均绑定到现有用户实例（中文“未命名”、12 个构件），清单没有提供可选新实例窗口。为避免修改用户原有工作，没有向该旧实例发送编辑按键。**新包 Cmd+D、鼠标落位与中英文切换桌面实测未验证**；未用旧实例截图代替新包证据。原有用户实例 PID 97630 保持不动。
- 本次无新增语言文本，沿用命令名称和已有状态翻译，画布反馈为数值与 +/++。英文/中文 RawInput 验证通过；语言两个入口、重启持久化和完整窗口表面未在最终包实测，见 ui-language.md。
- 经验候选（待开发者确认，未写入 AGENTS）：连续多副本的双击结束需同时判定落点距离，快速点击不同位置仍需继续；编辑过程的 Option 需优先于参数传递光标。


## 2026-10-28 REA移动实测修正最终验收


`python3 scripts/ac29-workflow.py verify --force`：340 Rust tests passed、1 performance test ignored；15 tooling tests passed；格式、cargo check --locked --all-targets、本地化检查和diff检查通过。

最终包：/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app。release与bundle arm64 UUID 均 `E25DBA40-EC95-3424-AEA1-D96269C31FD8`；codesign deep/strict通过；bundle检查与成功源码receipt一致。
最终运行PID45500，隔离HOME `target/rea-drag-smoke-home`，用户实际HOME偏好没有改动。新包完成英文移动/复制/多副本/取消，中文基点/单副本/多副本/取消，以及菜单撤销。

语言：真实新实例首帧为中文；同一最终实例经 View > Language 完成 English→简体中文→English；标题、顶层菜单、工具栏、Info Box、工具箱、视图标签、选择/构件数量、状态栏、Quick Options条已查看。切换前后恢复项目JSON完全相同；选择保持，Undo/Redo菜单状态及撤销结果正常。Quick Options浮动面板、无关对话框和原有部分3D子菜单译名未逐项验收，不能声称全应用本地化完成。标题栏语言选择器已在此前用户要求的原生标题栏改造中移除（native-menu-title/contract.md已有记录），本轮使用现有唯一语言入口，没有恢复它。


详见 [本轮验收](</Users/ren/MAC工作/工作/code/开源项目/openarchi/docs/features/edit-menu/drag-runtime-acceptance.md>)。AC29 REA10份，最终包REA1份，English/中文阶段AX记录在 edit-menu/evidence/。

## 2026-10-28 按住左键直接移动/复制补丁

详见[直接拖动验收](features/edit-menu/direct-drag-acceptance.md)。AC29独立样本按压即拖→Option→保持Option按Cmd→释放→两次点击→Esc已实测，REA及输入记录归档；原用户项目未修改。OpenArchi平面整体拖动在吸管判断前创建共享移动会话，释放不提交，点击落位。

verify --force：342 Rust passed/1 ignored，15 tooling passed，格式/check/localization/diff/package passed。receipt `target/ac29-workflow/runs/20261028T151445Z-adfcd132/report.json`；最终`dist/OpenArchi.app`与release UUID `E856826B-CF89-38E1-99ED-1F24094199EF`，strict/deep签名通过。

桌面：最终包已启动并渲染英文恢复模态；系统前台loginwindow锁屏，CUA cgWindowNotFound且无法激活，已请求解锁。当前最终包完整拖动、中英切换/重启均unknown，不能沿用前一版本桌面通过结论。

## 2026-10-28 Cmd+D物体上取点修复

[修复记录](features/edit-menu/cmd-d-object-click-fix.md)：完整工作区回归先复现按下后立即移动丢失基点，再改为按下取点、消费本次释放。覆盖墙端点/墙身/其他元素/空白、轻微抖动和三种复制模式。verify --force：344 Rust passed/1 ignored、15 tooling passed；format/check/localization/diff/package passed。最终UUID `6D9C20DF-E324-3692-A1EE-1AB1C1CF6D9A`，strict/deep签名通过，bundle与源码receipt匹配：`target/ac29-workflow/runs/20261028T154449Z-77cbd3ec/report.json`。最终包PID67637启动后系统再次loginwindow锁屏，桌面实际操作与中英切换待解锁；原用户PID69192未操作。

## 2026-10-29 三维旋转光标与底视

[契约及桌面证据](features/3d-orbit/contract.md)。左右跨水平线的仰角范围改为±1.53，原创环形双箭头显示于左键/Shift中键旋转，暂停保持、松开恢复；普通中键平移保留。345 Rust passed/1 ignored、15 tooling passed，format/check/localization/diff/package通过。receipt `target/ac29-workflow/runs/20261028T161755Z-dd034f54/report.json`；dist/OpenArchi.app与release UUID `6C1ABA5B-EE6F-3647-AEC0-98D09BD2991A`、strict/deep签名通过、当前源码receipt匹配。

最终包PID22934独立HOME桌面实测通过：英文左键越水平看到底面+环形光标、释放；Shift中键回水平上方；中文Shift中键底视、普通中键平移；切回英文。截图/AX归档。用户PID72149未改。不复制AC资源，不声明Archicad整体等价；不含极点翻滚，未全量验证本地化。

## 2026-10-29 移动/复制距离角度Tracker

[实现及快捷键速查](features/edit-menu/drag-tracker.md)。D/R距离、A角度、Tab切换、Enter提交、Esc两阶段取消；单位随当前DimensionStyle，Option复制模式保留输入，多副本放置后继续。348 Rust passed/1 ignored、15 tooling passed；format/check/localization/diff/package全部通过。最终app与release UUID `7908B563-1CC2-3B50-82B0-FACA46863462`，strict/deep签名通过、源码receipt匹配：`target/ac29-workflow/runs/20261028T164152Z-9ef85f21/report.json`。

桌面：独立最终包PID20870启动后前台loginwindow锁屏，输入守卫拒绝，已请求解锁；实际键盘/中英切换/重启/视觉检查均unknown。不将自动事件测试冒称桌面实测。

## 2026-10-29 旋转光标精修

仅精修原创矢量光标：更小的细线双弧、实心箭头、浅色细描边、无中心圆点。348 Rust passed/1 ignored、15 tooling passed；format/check/localization/diff/package通过。最终dist/OpenArchi.app UUID `A7A5838A-8DD5-3A95-ABF9-1974AFDA730F`，签名通过且源码receipt匹配（`20261028T165109Z-ae746401`）。独立最终包PID95076已在真实GPU视图浅色墙面、深色底板上保持旋转检查，截图见[旋转契约](features/3d-orbit/contract.md)。无文字变化，未重做无关语言与Tracker验收。

## 2026-10-29 吸管光标精修

原创细管/尖口/窄挡圈/圆角胶帽替换粗管与大圆头，蓝色液柱保留。348 Rust passed/1 ignored、15 tooling passed；format/check/localization/diff/package通过。当前dist/OpenArchi.app UUID `C892F68C-7EF2-3053-8F70-27163929543B`，签名通过且源码receipt匹配（20261028T165741Z-02b0dbe3）。最终包独立PID51947已实测平面空白/墙体悬停和Option点击吸取参数；AX确认Picked up Wall parameters from Wall 001。截图与边界见[吸管精修](features/parameter-transfer-evidence/pickup-refinement-2026-10-29.md)。无新文字，3D和无关Tracker桌面验收未重复。

## Active wall method switching — 2026-10-29

`python3 scripts/ac29-workflow.py verify --force` passed: 350 Rust tests, one ignored performance test, 15 tooling tests; formatting, all-target check, localization and diff checks passed. Fresh `dist/OpenArchi.app` release/bundle arm64 UUID `512DBB7E-75EA-3F49-BCA9-03FA867FBF06`; strict deep codesign passed. Receipt: `target/ac29-workflow/runs/20261028T171643Z-903a89aa/report.json`. Final `bundle` freshness recheck passed.

New regressions: `active_wall_method_switch_retains_nodes_through_info_box_clicks` exercises full-workspace egui method-button events, retained nodes, exact preview/committed endpoints, and undo/redo. `active_wall_method_switch_rectangle_and_cancel_preserve_transaction_boundary` covers retained prefix + four rectangle edges, single undo and cancellation.

Native smoke on final package: isolated HOME `target/wall-method-home`, PID 6999 (closed after verification), existing user PID 37444 untouched. Started wall with W; clicked (600,650), AXPressed Continuous, clicked (750,650), AXPressed Single, moved/clicked (750,750). Preview retained both segments; elements 12→14; Cmd+Z returned 12. Repeated in Chinese through native View > Language; elements 12→14; undo returned 12; repeated partial input and Escape canceled with 12 remaining. Restored English. Evidence: `docs/features/wall-method-evidence/` screenshot and AX dumps. No labels added/edited. Language restart persistence was not rerun; the title-bar selector remains absent by prior user request. Settings-dialog route shares default application code but was not manually exercised. Rectangle/Polygonal mixed transitions have no current desktop comparison with AC29; arbitrary other commands are not claimed supported.

Reusable lesson candidate (awaiting developer confirmation; not written into AGENTS): changing a construction method should transition pending input state, retain accepted nodes, and reset incompatible numeric locks; test preview and final geometry together, and verify switching alone leaves the model/undo history unchanged.

## 2026-10-29 — Standard toolbar / Edit > Reshape

- Delivered existing-model subset: Trim, Magic Wand, Adjust, Intersect, Fillet/Chamfer and numeric/graphical Resize; seven original icons and shared menu dispatch. Roof/Shell Trim disabled explicitly. Full per-command object matrix, AC29 help/runtime evidence and unsupported curved/3D/hosted cases: [contract](features/reshape-toolbar/contract.md).
- Forced verify `target/ac29-workflow/runs/20261028T175117Z-2836eec4/report.json`: fmt/includes, check all targets, localization, **364 Rust tests**, **15 tooling tests**, diff and packaging passed. 14 added regressions include geometry, actual egui pointer sequences, rollback, undo/redo and persistence.
- Fresh `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **BD0A3CB4-A003-3F81-AA7C-CB1D4EC3A237**; strict deep codesign passed. Supersedes intermediate E37C9E59 / 363-test run.
- Final native PID 2944 / window 41654: toolbar layout, EN/ZH native menu labels, whole main-window representative surfaces, Resize dialog EN/ZH, View > Language switching and Chinese restart first-frame confirmed via native AX menus/screenshots. Model edit was never confirmed in desktop process. [Evidence](features/reshape-toolbar/contract.md#final-packaged-desktop-evidence).
- Desktop **partial**: CUA cannot bind (`cgWindowNotFound`); egui AX controls unavailable; bounded CG clicks/keys did not activate custom controls. Canvas completion/cancel/undo/save, Fillet dialog, floating Quick Options and title-bar language selector are unknown (selector absent from native-title configuration). Headless UI tests are not substituted for these gaps. No global AC equivalence or complete localization claim.

## 2026-10-29 — Ctrl temporary Trim follow-up

Holding physical Ctrl temporarily activates scissors Trim; release restores the prior tool/draft. Mouse release remains owned by Trim when Ctrl is released first. Five additional full-workspace regressions pass. Forced verify receipt `target/ac29-workflow/runs/20261028T180350Z-ab1bd890/report.json`: **369 Rust + 15 tooling tests**, fmt/check/localization/diff/package passed. Final `dist/OpenArchi.app` release/bundle arm64 UUID **E8A6B374-EEA4-3641-889F-03610EBD897D**; strict/deep signature passed.

Final isolated native instance verified View > Language English→Chinese→English and Chinese restart first-frame title; user's older process preserved. Native Ctrl-click remains unknown because CUA binds the existing older user instance. New tooltip translations verified in automated events; title-bar selector absent and floating Quick Options not rechecked. Details and screenshots: [follow-up contract](features/reshape-toolbar/contract.md#ctrl-follow-up-validation--final-package).

## 2026-10-29 — Cmd / continuous interval Trim correction

User corrected Ctrl to **Cmd**. Temporary Cmd mode, repeated menu/toolbar mode, nearest end interval, middle deletion and endpoint-only final deletion implemented. **373 Rust + 15 tooling tests passed**; fmt/check/localization/diff/package passed. Receipt `target/ac29-workflow/runs/20261028T181812Z-05046f41/report.json`. Final `dist/OpenArchi.app` release/bundle arm64 UUID **967D7AFF-53BC-36FE-B4CF-A75C0B0FDF99**; strict/deep signature and source-freshness check passed.

Native final package isolated PID 32074: actual Cmd hover/click, sequential end cuts, middle deletion, last-interval deletion, Chinese repeated menu Trim, Escape, Undo/Redo passed; recovery JSON confirms exact results. EN→ZH→EN menu switch and both revised tooltips checked. Chinese restart PID 51563 confirmed first frame and recovered final geometry. Test instances closed, user PID 52583 untouched. Title-bar selector absent by prior configuration; floating Quick Options and unrelated dialogs unknown. Details: [Cmd acceptance](features/reshape-toolbar/contract.md#cmd-final-verification-and-native-acceptance).

## 2026-10-29 — Rotated wall middle-edge node editing

Actionable wall palette with Insert node and existing transform entries, shared plan/3D preview and atomic two-wall commit; original endpoints fixed, hosted-opening checks/remap, numerical Distance/Angle, cancel and undo. 3D straight-wall insertion is an explicit OpenArchi extension, not AC29 equivalence. **377 Rust + 15 tooling tests passed**, fmt/includes/check/localization/diff/package passed. Receipt `target/ac29-workflow/runs/20261028T183751Z-e2a64a9f/report.json`.

Final dist/OpenArchi.app release/bundle UUID **1BA174C1-BFB7-3218-8144-D1F78A68D3F2**, strict/deep signature passed; source freshness matches receipt. Native final instance passed rotated wall plan edit; orbit then 3D edge edit; preview/cancel; numerical 2.5 m/90° input; exact saved node geometry; Undo/Redo; EN→ZH→EN; Chinese restart/recovery. User PID 99546 untouched, test instances closed and isolated language restored. Title-bar selector absent by existing configuration; floating Quick Options and transform alias dialogs not rechecked. Full [contract and evidence](features/wall-editing.md#2026-10-29-final-verification).

## 2026-10-29 — Complete two-row wall command palette

`verify --force` passed **399 Rust tests + 15 tooling tests**, format/includes, all-target check, localization, diff and fresh packaging. Receipt `target/ac29-workflow/runs/20261028T191127Z-5671ac64/report.json`. Deliverable `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **36F797A9-B20D-3043-9BD2-CC877AB1B026**, strict/deep signature passed; final bundle freshness matches source receipt.

Final native isolated app exercised all ten commands with model-result JSON: insert node, curve, tangent, two-point trapezoid exterior, trapezoid split, drag, three-point rotate, two-point mirror, +1.25 m elevate and three incremental copies. Actual curve plan/3D/3D-palette-preview-cancel, Undo/Redo and recovery reopening passed. EN→ZH→EN and Chinese restart passed. One native-discovered zero-distance clamp/stage lock was fixed and fresh-built before final acceptance. [Detailed screenshots, command table, limits and language coverage](features/wall-pet-palette/validation.md). Custom contour walls still bypass old straight-wall junction cleaning; legacy unsupported partial edits reject atomically. No full Archicad equivalence claim.

## 2026-10-29 — Baseline default node, solid/hollow pencil, hover commands

Fresh `verify --force`: **402 Rust + 15 tooling passed**, formatting/includes, all-target check, locale validation, diff/package. Receipt `target/ac29-workflow/runs/20261028T195222Z-83063bfc/report.json`; app `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`, release/bundle arm64 UUID **0A21A3A6-9135-3AD1-99D0-8AD06964E908**, strict/deep signature passed, final freshness matches receipt.

Native final package: baseline click immediately starts Insert node; original-line solid pencil/outside hollow pencil in plan and 3D; mouse motion alone activates/highlights Curve and Rotate; return to Insert; confirm two-wall result; Cancel, Undo/Redo; EN→ZH→EN and Chinese restart passed. Three new UI regressions cover all-ten hover transitions, visible highlight, original curved path and stage preservation. Scope remains wall reference editing; [detailed evidence, setup retry, localization boundary and experience candidates](features/wall-pet-palette/cursor-followup/validation.md).

## 2026-10-29 — Compact wall palette and mouse-release confirmation

Fresh `verify --force`: **404 Rust + 15 tooling passed**, formatting/includes, all-target check, localization, diff/package. Receipt `target/ac29-workflow/runs/20261028T201755Z-a93fb169/report.json`. App `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **85D62F1A-641C-3977-B41A-7F73F94C586C**, strict/deep signature passed; source freshness matches receipt.

Compact 148×64-point ten-icon palette; command instructions and Apply/Cancel block removed. Pointer-adjacent numeric tracker, real text+Enter confirmation, plan/3D selected-baseline drag-release, held-drag hover switching and UI-release exclusion verified. Native final app passed EN→ZH→EN, Chinese restart, undo/redo and exact saved-model comparison. Title-bar language selector unavailable by prior product choice; floating Quick Options, unrelated dialogs, other command-specific tracker fields not native rechecked. Full evidence and explicit localization limits: [compact follow-up](features/wall-pet-palette/compact-followup/validation.md).

## 2026-10-29 — Selected multi-wall junction movement

Fresh `verify --force`: **413 Rust + 15 tooling passed**, formatting/includes, all-target check, locale validation, diff/package. Receipt `target/ac29-workflow/runs/20261028T203304Z-0a93b003/report.json`. App `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **E13465E1-CFD3-37B2-AC26-638D87358165**, strict/deep signature passed; source freshness matches receipt.

Selected straight-wall junction drag now moves every participating node atomically; fixed far ends, private preview, click/Enter/release, cancel, one undo/redo and JSON round trip. T/X through-walls split with existing metadata/opening handling. Nine new regressions cover geometry, selection, rejection and plan/3D event handling. Final native package verified three-wall shared plan/3D preview and release, exact saved coordinates/remote endpoints, Undo/Redo exact JSON, EN→ZH→EN, Chinese restart and reopen. Move node uses existing EN/ZH translation keys. Title-bar selector unavailable; unrelated dialogs/floating Quick Options and curved/profiled junctions unverified/unsupported. [Full evidence and scope](features/wall-junction-edit/validation.md); [experience candidates](features/wall-junction-edit/contract.md#implementation-evidence-and-candidate-lessons) await developer confirmation.

## 2026-10-29 — three-state palette and multi-wall endpoint cleanup

Fresh `dist/OpenArchi.app` built from the verified source receipt `target/ac29-workflow/runs/20261029T025513Z-71e440bb/report.json`: **428 Rust tests (380 library + 48 integration), 15 tooling tests passed**, one existing performance test ignored. Format/includes, all-target check, localization validation, diff, package and strict/deep signature pass. Release/bundle arm64 UUID **ECA2334E-2A55-32E6-A959-B0D0A70DD16A**; final `ac29-workflow.py bundle` reports source freshness matches receipt.

Three palette states now coexist while browsing; click/canvas-return promotes a candidate. Multi-way straight-wall endpoints use adjacent sector face intersections, center caps and bounded short-wall bevels; plan, selection, Trace, private preview and ordinary 3D share arbitrary polygons. Added geometry/UI/render regressions cover holes, reverse spikes, concavity, order/reversal/rotation, reference offsets and short walls. AC29 INT 5101 was separately reproduced/measured: symmetric Y/T/X match observed unions; the four-ray AC case itself retains a reverse spike, intentionally removed by this OpenArchi policy. [Contract, measurements, screenshots and limits](features/wall-junction-cleanup/contract.md).

**Native final-bundle UI is unverified:** isolated process 81807/window 43250 launched, but macOS was locked (`CGSSessionScreenIsLocked=Yes`, foreground loginwindow 420), preventing guarded mouse input and leaving a blank viewport. Plan/3D visual acceptance, palette interaction, language switching/restart and native undo/redo could not run. Automated tests are not substituted for those checks. Original user OpenArchi 27005 and Archicad 51530 were preserved. [Full validation / blocker](features/wall-junction-cleanup/validation.md). Pending experience proposals are in the contract; no reusable AGENTS.md rules were changed for this follow-up.

## 2026-10-29 — Polyline editing aligned with the shared wall session

Fresh `python3 scripts/ac29-workflow.py verify --force` passed formatting, all-target compilation, localization/duplicate-key validation, **431 Rust tests** (one performance test ignored), **15 tooling tests**, whitespace and packaging. Receipt: `target/ac29-workflow/runs/20261029T045457Z-c7623878/report.json`. Deliverable: `dist/OpenArchi.app`; release and bundle arm64 UUID **1E951660-57EF-36A1-82DB-F6D93FBE7E45**; deep strict signature passed. The preceding CD1A15D5 UUID was superseded after the overlapping-node priority correction.

Final-package native smoke used isolated HOME, PID 68716 then 75249: English/Chinese node and edge palettes + numeric tracker, off-edge insertion, node drag (other vertices remain), undo, language switches and Chinese restart title passed within the recorded scenario. The existing user process (14 elements, 1 selected) remained intact. All disposable test instances were closed. Numeric Enter, staged transforms, arc/corner geometry, JSON and original wall callers have automated coverage; not every command was re-executed natively. Exact cursor hotspot/timing, full AC29 equivalence, untouched dialogs and native verification of every dirty/history field remain unknown. The existing title-bar selector has been removed by the native-title contract; View → Language was exercised. See [feature contract, finite claim outcomes and screenshots](features/polyline-editing.md#2026-10-29--implementation-and-final-validation).

## 2026-10-29 — wall/polyline palette click activation and command lock

Fresh forced verification `target/ac29-workflow/runs/20261029T061018Z-d96a38ff/report.json`: format, all-target check, localization validation, **432 Rust tests** (one performance case ignored), **15 tooling tests**, whitespace and packaging passed. Release/bundle UUID **4095BCED-962A-36A1-8AEC-BD27D78FB11F**; strict deep ad-hoc signature passed. Deliverable `dist/OpenArchi.app` is current to the successful receipt.

Removed hover-candidate promotion on canvas return from the shared wall/polyline session. Left-click icon activation locks the active command against mouse movement; hover only paints feedback. Both objects/both languages, held drags, staged/numeric state, cancellation, undo and wall projected-3D are covered by passing event regressions. Final native English plan smoke passed for both objects: default Insert node remains after hover/return; click Curve activates it; hover Rotate/return retains Curve. Original user instance (12 elements, 1 selected) untouched; disposable isolated instance closed. No new native Chinese/3D replay or exact cursor-hotspot measurement is claimed for this no-label-change fix. See [contract](features/wall-pet-palette/contract.md#click-lock-correction-implementation-and-verification) and [native evidence](features/wall-pet-palette/click-lock-20261029/native.md).

## 2026-10-29 — remove selected-wall passive length

Fresh forced workflow passed formatting, all-target check, localization validation, 432 Rust tests (one performance case ignored), 15 tooling tests, diff whitespace and packaging. Receipt `target/ac29-workflow/runs/20261029T062421Z-2231a9dd/report.json`; final `dist/OpenArchi.app` arm64 UUID **5858F0D1-77C2-36F5-9485-0F512AA2329B**, matching release UUID; strict deep signature passed. Native final-package English/Chinese selection screenshots confirm no wall-length annotation above the baseline while blue reference line, black handles, real overall dimensions and gap feedback remain. Isolated instance closed; user instance/project preserved. Static endpoint-palette length removal is bilingual-test covered, Chinese restart first title `未命名` passed on a second isolated final-package instance. Native popup replay and untouched dialogs are not newly verified. See [wall selection contract and screenshots](features/wall-selection.md#removal-delivered-and-checked).

## 2026-10-30 — endpoint menu matches Insert node menu

Fresh `verify --force` receipt `target/ac29-workflow/runs/20261030T125243Z-57ab5fec/report.json`: format, all-target check, localization validation, **433 Rust tests** (one performance case ignored), **15 tooling tests**, diff whitespace and packaging passed. `dist/OpenArchi.app` release/bundle UUID **8A99BFD7-6D77-3566-B8E0-5F2404D5D802**; strict deep ad-hoc signature passed.

Endpoint and Insert node use the same compact ten-icon control. First endpoint slot remains Move node; shared-command roundtrip restores existing endpoint drag/snap behavior. Both ends/both languages and undo/redo are event tested. Final native start-endpoint English/Chinese palette, explicit Curve activation/hover lock, return to Move node, language roundtrip and Chinese restart title passed with isolated test instances; original user process left running. Native end-endpoint/every-command replay, untouched dialogs, exact hotspots/timing and new AC29 equivalence are not claimed. See [contract](features/wall-pet-palette/contract.md#endpoint-menu-delivered-and-verified--2026-10-30) and [evidence](features/wall-pet-palette/endpoint-menu-20261030/native.md).


## 2026-10-31 Slab straight-edge insert

439 Rust / 15 tooling tests passed through forced ac29 verify; fresh dist/OpenArchi.app arm64 UUID 500A51F6-6954-3181-892E-8B7D15A2280B, release/bundle match and strict/deep codesign passed. Native final-package CUA: isolated HOME, edge click→concave perimeter, undo/redo, 3D solid, save→restart→Open contour roundtrip; English/Chinese status and representative workspace surfaces, Chinese first-frame restart then English restored. Title-bar language entry absent, floating Quick Options and exact cursor hotspot unknown. Straight single-ring insert only; full Slab reconstruction incomplete. [Evidence/limits](features/slab-edge-editing/contract.md).


## 2026-10-31 完整边线编辑包验证结果

`python3 scripts/ac29-workflow.py verify --force` PASS：461项Rust测试（413 lib + 7/10/31 integration，1 ignored）、15项工具测试、格式/include格式、all-targets check、本地化、diff检查和fresh打包。凭据：`target/ac29-workflow/runs/20261030T191701Z-0793e2d5/report.json`。新增完整小面板13项实际egui事件回归全部通过，含双语角度45/135、表面/三作用域、孔洞输入、曲边保弧插点显式释放立即提交、变换/切线/倒圆角、候选分裂ID、undo/redo/JSON；另有7项完整kernel与2项geometry专项回归。

交付包：`dist/OpenArchi.app`；arm64 release/bundle UUID均 `78EE8804-D188-39AD-9986-35B2635C18B5`，deep/strict codesign PASS。本轮没有提交或推送。

最终包桌面限制：隔离HOME `/tmp/oa-slab-full-home-20261031`，当次新进程73397、CGwindow52526（逻辑1512×988），ps核对HOME正确；用户旧进程89567保留。CUA getApp路径返回用户旧实例（中文13个构件），未对其发输入；精确PID的REA启动观察`ev_592c3e705aec3e489c5426956fa1845fb7cb264e804998ff495aa599fc2b5ba4`确认新包英文启动、Recover对话框，完整记录[evidence/20261031-openarchi-final-native.json](evidence/20261031-openarchi-final-native.json)。它只证明启动/界面，不证明恢复已点击或完整鼠标编辑。

当前CUA对Archicad已知窗口的点击持续返回 `-10005 noWindowsAvailable`（重读AX/AXRaise及重新绑定后仍失败）；重置CUA会话没有解决多实例精确绑定。未绕过用户实例保护。故本轮新包完整真鼠标手势、EN→中文→EN即时菜单切换、中文重启、精确系统光标热点为unknown，不能套用上一版原生验证冒充通过；双语新增命令的自动事件回归通过。圆弧半径显示已实现，独立数值半径锁定待维护者确认当前分支保持规则，异步问题已提出，未将超时视为同意。


## 2026-10-31 — P0 baseline and save integrity

Fresh `verify --force`: **465 Rust + 15 tooling passed**, one existing performance test ignored; formatting/includes, all-target check, localization, whitespace and fresh packaging passed. Receipt `/Users/ren/MAC工作/工作/code/开源项目/openarchi/target/ac29-workflow/runs/20261030T201614Z-d8346a8c/report.json`. Deliverable `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **0B031BCF-7C67-336C-97DC-26A39F4660C5**, strict/deep signature passed. Four new filesystem regressions fail against the old save implementation and pass after exclusive temporary-file creation, file sync, atomic rename and owned cleanup. No UI strings or operation gestures changed.

Native isolated process30131: verified HOME/current bundle, English main window and File→Save JSON open passed. Save/cancel input on the custom modal did not execute after coordinate/keyboard/AX Raise retries; native disk save/reopen, 3D, full slab gestures and bilingual switching remain unknown. No user instance/project touched. [P0 report and remaining gates](features/ai-native-workbench/p0-execution-2026-10-31.md), [capability/write-entry matrix](features/ai-native-workbench/p0-capabilities-2026-10-31.md), [pre-code storage contract](features/persistence/contract.md). P0 overall remains in progress; P1 not started.


## 2026-10-31 — P0 model write atomicity guards

Fresh `verify --force` PASS: **470 Rust + 15 tooling**, one performance test ignored; formatting/includes, all-target check, localization, diff and fresh packaging passed. Receipt `/Users/ren/MAC工作/工作/code/开源项目/openarchi/target/ac29-workflow/runs/20261030T203045Z-c6ecacb6/report.json`. Deliverable `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **ECCF275E-93C5-34C9-9D78-F3C4AC6AFA27**, strict/deep codesign passed. Five new `editor_atomicity` regressions cover legacy invalid-candidate rollback, max-ID overflow rejection, locked-layer restore batch rejection, valid mixed command history/JSON and allowed restore ordering. Two old UI fixtures now set valid next_id before testing; their original behavior assertions remain.

Native isolated PID53022: HOME/current package verified; English launch, AX-click plan→3D→plan and visible three-dimensional wall/slab rendering passed. 12 elements/0 selected visible counts preserved; this alone is not complete model/history proof. No new UI strings/gestures. Bilingual switching, native invalid edits, full slab gestures and disk save/reopen were not run on this package and remain unknown. Test instance closed; user preferences/projects preserved. [Pre-code contract, first failed run, final evidence and pending lessons](features/ai-native-workbench/p0-execution-2026-10-31.md).
