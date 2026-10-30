# Parameter Transfer（吸管吸取 / 注射器注入）— Archicad 29 INT 5101

状态：已实现并验证（自动化 280 项测试 + 桌面双语实测；悬停光标/亮显与吸取后工具激活的真机视觉待手持验证）。日期：2026-10-24。目标基线：AC29 `29.0.0 v1 INT (5101)`，主程序 SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。验证结果（包 UUID、桌面步骤、剩余限制）见 [validation.md](../validation.md) 2026-10-24 条目；要点：`⌥C` 吸取、工具栏/菜单注入、Element Transfer Settings、双语与中文重启首帧均桌面实测通过；`⌘⌥C` 命令与 `⌥+点击`/`⌘⌥+点击` 手势被本机系统输入拦截无法经自动化送达，由无头 RawInput 回归覆盖（含 `⌘⌥+点击` 注入目标构件的正向用例）；按住修饰键的吸管/注射器悬停光标、构件亮显与吸取后自动激活对应工具已实现并有无头回归（真机需手持验证）。

## 用户目标

复刻 Archicad 29 的参数传输：吸管（Pick Up Parameters）从构件吸取参数，注射器（Inject Parameters）把已吸取的参数注入同类型构件；快捷键与 AC29 INT macOS 完全一致。原始实现，不复制 Graphisoft 图标或资源。

## 证据

| 断言 | 证据类型 | 来源与限制 |
|---|---|---|
| 工具栏吸管按钮提示为 "Pick Up Parameters (Option+C)"，并注明 "Use Element Transfer Settings to set which parameters to pick up" | runtime observation（用户提供的正在运行的 AC29 INT 5101 截图，2026-10-23） | 只证明该按钮的名称、快捷键和提示文本 |
| 按住 Alt（Mac Option）点击构件 = 吸取：该构件参数成为该构件类型的默认值；光标变为吸管 | public documentation — [AC29 Parameter Transfer](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-98.htm) | 文档语义；本轮未对 AC29 运行时逐步复现 |
| 按住 Ctrl+Alt（Mac Alt+Cmd）点击构件 = 注入已吸取参数；光标变为注射器；旁边显示最近使用的 Transfer Set 名称，可点击切换并成为下次默认 | public documentation（同上） | 同上 |
| 参数只能在同类型构件间传输；Line/Arc/Polyline/Spline 视为同一类型（2D 草绘）；"certain basic parameters are always transferred"（未列出清单）；命令也可从 Standard Toolbar 与 Edit > Element Settings 使用；任意视图可用 | public documentation（同上） | basic parameters 清单 unknown |
| Inject 命令把参数应用到选中的构件 | public documentation（同上页摘要："inject (apply) them to other selected elements"） | 工具栏命令与选中元素交互的完整边界为 inference |
| Element Transfer Settings 对话框：Transfer Set 列表，可新建/编辑/复制/删除/重命名；复选框决定注入哪些设置；一个默认 set；未列出的设置始终传输；set 随工程存储；可导入/导出 XML | public documentation — [Element Transfer Settings](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-99.htm) | 对话框逐项分组清单未能完整抓取 |
| Inject Parameters 命令快捷键 Mac = ⌘⌥C（Windows Ctrl+Alt+C） | public documentation（第三方快捷键表：Scribd Archicad Shortcuts、Economical by Design），与官方注入修饰键模式（Ctrl+Alt / Alt+Cmd）一致 | 第三方来源 + 官方模式推断，非 AC29 一方证据；unknown 保留 |
| 按住 Alt/⌥ 时指针变为空吸管样式；悬停构件时构件亮显、吸管出现液体；点击左键完成；注入同理（⌘⌥，注射器样式） | runtime observation（用户口头描述其使用的 AC29 INT 交互，2026-10-24） | 描述的是语义顺序与状态（空→悬停亮显+液体→点击）；光标逐像素样式无一方截图，OpenArchi 按语义自绘 |
| 吸取改变的是工具默认值，不产生模型撤销记录；注入是模型修改，需可撤销 | inference（由官方"成为该类型默认值"+ 建模常识） | — |

## AC29 行为契约

1. 吸取（吸管）：悬停源构件 → 按住 Alt/⌥ → 点击。该构件的可传输参数成为该构件类型的创建默认值；同时参数驻留在"吸管"中等待注入；**点击后自动激活对应构件的工具**（吸取墙 → Wall 工具，吸取门 → Door 工具，以此类推），带吸取到的默认值直接进入可绘制状态。
2. 注入（注射器）：悬停目标构件 → 按住 Ctrl+Alt / ⌥⌘ → 点击。使用当前 Transfer Set 把参数写入目标构件。
3. 命令（Standard Toolbar 按钮、Edit 菜单、⌥C / ⌘⌥C）：有选中构件时直接对选中构件执行（吸取取单个选中源；注入写入全部选中目标）；无选中时进入一次性光标模式，下一次点击执行并退出。
4. Element Transfer Settings：按构件类型分组的复选框决定吸取/注入范围；未列出的设置始终传输；set 随工程保存。
5. 类型规则：同类型才能传输；2D 草绘类互通。
6. Esc 取消一次性光标模式。状态栏与光标给予反馈。
7. 按住修饰键期间的悬停反馈（2026-10-24 用户运行时补充描述）：按下 Alt/⌥ 时指针变为空的吸管样式；移到构件上时构件被亮显、吸管出现液体；此时点击左键才完成吸取。注入同理，只是多按 Cmd（⌘⌥），指针为注射器样式。
8. 吸取完成后激活对应工具、准备绘制（2026-10-24 用户运行时补充描述）：空白处光标为空管，构件上方为半充满，点击吸取属性并激活该构件的工具进入构建状态。

## OpenArchi 实现（原创设计，假设标注）

- 快捷键：`⌥C` = 吸取命令，`⌘⌥C` = 注入命令；手势 `⌥+点击` = 吸取，`⌘⌥+点击` = 注入（平面与 3D 视图均支持）。假设：命令快捷键 ⌘⌥C 采用第三方表与官方修饰键模式；若日后取得 AC29 一方证据不同再修正。
- 传输类型：`Wall`、`Slab`、`Stair`、`Polyline`、`Door`、`Window`（门窗分开，如 AC 的 Door/Window）；Polyline 即 2D 草绘类。
- 传输组（Element Transfer Settings 复选框）：
  - Wall：`Geometry`（厚度、高度、参考线/偏移、垂直偏移、Profile）、`Material & Layer`、`Surfaces`（表面覆盖元数据）、`Floor Plan Display`、`Data`（结构/分类元数据）。
  - Slab：`Geometry`（板厚）、`Material & Layer`。
  - Stair：`Geometry`（StairSpec 除起终点外）、`Material & Layer`。
  - Polyline：`Material & Layer`（点/弧属于形状，不传输）。
  - Door/Window：`Geometry`（宽、高、 sill）、`Material & Layer`。
- 永不传输：id、名称、楼层、形状位置字段（起终点/边界/点列/宿主墙/offset/门窗类型）、renovation 状态、design option。这是 OpenArchi 的范围决定（AC 的 always-transferred 清单 unknown）。
- 吸取：快照存入每类型缓冲（`Editor::pick_up_buffer`），同时按当前 set 更新工具创建默认值（`editor.wall_settings/thickness/height/material/wall_layer`、`door_defaults/window_defaults`、`stair_spec`、共享 `material`），并 `set_tool` 激活对应工具（吸取即进入该工具的绘制状态；工具属视图状态，不产生撤销记录）。OpenArchi 的创建材质是全局共享字段（区别于 AC 的每工具默认），吸取会更新它——记为本实现的偏差。
- 注入：对每个目标克隆 → 按 set 应用组 → 整体 `Project::validate` → 失败则整体放弃（模型不变、状态栏报错）；成功则一次 `commit(before)` = 一条撤销记录（与 mirror/rotate 组变换同模式）。
- Transfer Settings 存于 `Project.transfer_settings`（serde default，旧 JSON 兼容，默认单个 "All Parameters" set），参与 ProjectHeader 差量，对话框 OK 产生一条撤销记录。
- 一次性模式（无选中时的命令）：工具栏按钮高亮 + 状态栏提示；Esc 取消。
- 悬停反馈（`src/transfer_cursor.rs` + 平面/3D 视图拦截块，2026-10-24 第二版改为画布内绘制）：按住 `⌥`（吸取）或 `⌘⌥`（注入）且指针在画布上时，隐藏系统指针并在指针位置绘制吸管/注射器字形——空白处为空管，悬停到"点击会生效"的构件上时管内出现液体（primed）且该构件被亮显（平面：白色浅罩+亮青描边；3D：亮蓝基色渲染）。首版曾用 `PlatformOutput::cursor_image` → NSCursor 位图，真机不可见，已废弃。注入的字形只在吸取缓冲已有该构件类型时 primed/亮显。指针离开画布、松开修饰键、平移/拖拽或 Space 时恢复常规指针。工具栏命令进入的一次性模式复用同一套字形与亮显。**修饰键按住期间指针独占**：捕捉点/悬停示意标记（`CursorFeedback` 箭头勾、三射线、捕捉目标样式）整体隐藏，只保留吸管/注射器两种状态；松开后恢复。
- 绘制状态光标：六个绘图工具（墙/板/楼梯/多段线/门/窗）激活且未按修饰键时，指针为 AC29 官方帮助所示的**简洁十字**，定位于吸附点（端点/中点/自定义捕捉点；自由时跟随指针），系统指针隐藏。节点、边与交点的智能反馈仍由原有分支处理；Select 箭头与 Marquee 十字不变。吸管/注射器 primed 液体绘制在管体上层，以免被管壁覆盖。
- 延后（记为缺口）：Transfer Set 导入/导出 XML；renovation 状态传输；Find & Select 联动（无对应面板）；AC 的 3D 注入细节未运行时复核。

## 验收清单

> 验证状态（2026-10-24）：第 1–7、9、10 项的语义由 20 项自动化测试覆盖（`src/transfer.rs` 9 项 + `src/app/ui_tests.rs` 11 项，其中 1 项在 `src/transfer_cursor.rs` 无；含 `cmd_option_click_gesture_injects_into_target_element` 正向验证 `⌘⌥+点击` 注入手势、悬停光标/亮显/抑制回归、`drawing_tool_shows_square_crosshair_cursor`、`pick_up_activates_the_matching_tool`，见 [Validation](../validation.md)）；第 8 项（双语）en/zh 已落盘且 localization 脚本通过，语言入口与中文重启首帧已在桌面实测（见 validation.md）。绘制字形与亮显的**真机视觉**因本环境"第二实例窗口不合成内容"无法截图验证，标记 unknown，需用户手持验证。

1. `⌥+点击` 墙 → 状态 "Picked up Wall parameters from Wall 00X"，工具箱 Wall 工具点亮，新建墙沿用源墙厚/高/材质/参考设置，下一次点击即开始画墙。
2. `⌘⌥+点击` 另一面墙 → 注入成功；一次 Undo 恢复，Redo 重放；工具不切换。
3. 选中一面墙按 `⌥C` → 等价吸取（同样激活工具）；选中两面墙按 `⌘⌥C` → 两面都注入且只有一条撤销记录。
4. 墙 → 楼板注入被拒，状态栏本地化提示"同类型"限制。
5. 门 → 窗被拒；门 → 门注入宽/高/sill，offset 与宿主不变。
6. 对话框取消勾选 Wall Geometry 后注入：厚度/高度不变，材质仍更新。
7. 注入导致校验失败（如洞口高于宿主墙）→ 整体失败报错，模型与撤销栈不变。
8. 工具栏吸管/注射按钮、Edit 菜单项、对话框、全部状态消息双语完整；语言切换不改变模型与撤销。
9. Esc 取消一次性模式；旧 JSON 打开得到默认 "All Parameters" set；传输设置随 JSON 保存/重开。
10. 注入多面墙时楼板等多余类型不受影响（类型过滤）。
11. 按住 `⌥`：画布上指针变空吸管；悬停墙 → 墙亮显 + 吸管出现液体；点击完成吸取并激活 Wall 工具；移到空白处液体消失；松开 ⌥ 恢复常规指针。按住 `⌘⌥`：同样链路用注射器光标，且仅在吸取缓冲已有该类型时才亮显/出现液体（手持验证）。
12. 工具栏按钮进入的一次性模式显示同一套吸管/注射器光标与亮显；Esc 取消。
13. 吸取楼板 → Slab 工具点亮且板厚默认随之；吸取门/窗 → Door/Window 工具点亮（手持验证）。
