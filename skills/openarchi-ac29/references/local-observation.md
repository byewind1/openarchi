# 本机目标与观察边界

以下是 2026-10-15 的已知身份，使用前由 context 检查，不是永久常量：

- macOS 15.7.5 (24G624)、Apple Silicon arm64。
- Archicad bundle ID：`com.graphisoft.archicad29`。
- 应用：`/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`。
- 可执行文件：应用内 `Contents/MacOS/Archicad`。
- 版本：`29.0.0 v1 INT (5101)`。
- SHA-256：`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。
- 当前 OpenArchi bundle ID：`com.openarchi.desktop`；`studio.atelier.bim.demo` 是旧身份，不能继续默认使用。

## 已有材料优先

仓库 `docs/archicad-interface-inventory.md` 有菜单盘点及当时的 REA 观察 ID；`docs/research.md` 是研究入口；`docs/features/` 是具体契约。盘点中的 PID 14896 / window 38986 是历史值，不能直接用于新的 capture。Guides 的 CUA 菜单与设置观察没有 REA Evidence ID；不要借用别的功能的 ID 来补齐。

`context` 的功能索引来自现有 Markdown，关键词匹配可能不完整。无匹配时再用 `rg` 搜索，而不是断言没有实现。匹配多个笔记时先读最聚焦的一个，避免全文载入所有历史。

## 工具使用经验

- CUA 按显示名 `Archicad` 曾返回 `Invalid app`，按 `com.graphisoft.archicad29` 成功；优先使用已核对的 bundle ID。第一次按当前工具文档初始化；不要复制历史控件编号。每次动作后重新取状态。
- `binary_session` 当时是未绑定；`open_binary` 可绑定以上 Mach-O。但进程、窗口和 REA 会话都需按当前情况判断，不能假定前一次会话还在，也不要反复 readiness。
- 已观察到 `observe_native_ui` 需要 PID 和 window ID；`capture_native_ui_scenario` 的能力不能假定包含自由拖拽。需拖拽时使用当前提供的 CUA `drag`。macOS CUA 当前未必暴露数字窗口 ID；缺少时记录缺口，不能猜 ID。
- REA 绑定不是行为观察，CUA 文字树不是模型尺寸测量。仅菜单列项不足以证明预览、取消或撤销。只在明确一次交互的前后状态和测量后作相应断言。
- 上次绑定 OpenArchi 完整应用包路径返回 `Computer Use server error -10005: cgWindowNotFound`，虽然进程实际已启动。该次调用异常拖延，后续应使用工具支持的短执行预算，并在同一条件失败后停止重试。短预算并不保证底层服务严格终止；继续可独立的代码/证据工作，如实标记桌面未验证。只有新窗口身份、服务修复或其他具体条件变化才值得重试。
- 不为读取目标身份启动用户程序，不为观察创建/关闭用户未保存项目。交互用当前授权的最小场景，记录修改与恢复。

## 官方帮助

优先 AC29 `https://help.graphisoft.com/AC/29/INT/_AC29_Help/`。具体页面已在专题笔记中，先复用再查缺口。别因搜索返回 AC30 或 AC19 就静默替换版本；作为交叉参考时标注版本差异和未确认项。需要网络时按当前可用检索技能执行；上次 agent-reach 的 Exa 未配置，是历史故障，不是保证今后不可用，也不应因此先跑一遍所有平台健康检查。

REA 只关闭本任务打开的会话；不关闭用户或其他任务的会话。技能不授予额外软件安装、项目修改或系统权限。
