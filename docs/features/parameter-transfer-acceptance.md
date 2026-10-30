# 参数传输（吸管/注射）用户验收清单

来源：用户 2026-10-24 五轮反馈的完整蒸馏。每条均为可执行验收步骤。目标基线 AC29 INT 5101；实现与证据见 [parameter-transfer.md](parameter-transfer.md) 与 [../validation.md](../validation.md) 2026-10-24 各条。

## A. 吸取链路（按住 ⌥/Option）

- A1 按住 `⌥`：指针 = 空吸管字形。此时**不允许出现任何其他指针样式或捕捉标记**——箭头、勾、三射线、手型、捕捉准星全部隐藏。
- A2 按住 `⌥` 移动到空白处：仍是空吸管。
- A3 按住 `⌥` 悬停到构件（墙/板/楼梯/多段线/门/窗）：**构件亮显（白色浅罩+亮青描边；3D 亮蓝基色）+ 吸管显示半管液体**。
- A4 此时点击左键：吸取该构件参数（成为同类型创建默认值 + 进入每类型吸取缓冲），状态栏显示 "Picked up … parameters from …"；**并自动激活对应工具**（墙→Wall、板→Slab、楼梯→Stair、多段线→Polyline、门→Door、窗→Window），工具箱对应项点亮，下一次点击以吸取值直接绘制。
- A5 松开 `⌥`：恢复常规指针——绘图工具下为简洁十字（随捕捉交点/线中点），Select 下为箭头；捕捉标记恢复显示。
- A6 吸取不产生模型撤销记录（工具默认值属视图状态）。

## B. 注入链路（按住 ⌘⌥/Cmd+Option）

- B1 按住 `⌘⌥`：指针 = 注射器字形，同样独占（无其他标记）。
- B2 悬停构件：**仅当吸取缓冲中已有该构件类型的参数**时亮显 + 半管液体；否则保持空管不亮显。
- B3 点击：把缓冲中该类型参数按当前 Transfer Set 注入被点构件；位置/参考线/宿主不动；一条撤销记录；**工具不切换**。
- B4 松开 `⌘⌥`：恢复正常指针。

## C. 命令路径（与手势等效）

- C1 入口：Standard Toolbar 吸管/注射按钮、Edit > Pick Up Parameters / Inject Parameters、快捷键 `⌥C` / `⌘⌥C`。
- C2 有选中构件：`⌥C` 直接吸取单个选中源并激活对应工具；`⌘⌥C` 注入全部选中目标（一条撤销）。
- C3 无选中：进入一次性模式（指针同 A/B），点击一次执行并退出；Esc 取消。

## D. 绘制状态光标

- D1 六个绘图工具（Wall/Slab/Stair/Polyline/Door/Window）激活且无修饰键：指针为**简洁细十字**（AC29 Intelligent Cursor 帮助中的 CrosshairCursor），绘制在吸附点位置（端点/中点/自定义捕捉点；自由时跟随指针），系统箭头隐藏；不是方框加粗刻线、不是铅笔、不是手型。
- D2 Select 工具：箭头 + 节点勾/边三射线标记（保持原状）。
- D3 Marquee：十字（保持原状）；平移/拖拽为抓取样式（保持原状）。

## E. Element Transfer Settings

- E1 Edit > Element Transfer Settings…：传输集新建/复制/删除/重命名；按构件类型分组的复选框；OK = 一条撤销记录；设置随工程保存/重开。
- E2 取消勾选的组不传输（如取消 Geometry 后注入不改厚/高，材质仍更新）。

## F. 类型与校验

- F1 同类型才能传输；跨类型注入被拒并本地化提示。
- F2 注入导致校验失败 → 整体失败，模型与撤销栈不变。
- F3 永不传输：位置、宿主墙、offset、id、名称、楼层。
- F4 门窗传输宽/高/sill；楼梯传输除起终点外的参数；板传输厚度。

## G. 双语

- G1 所有新增文案（工具提示、菜单、对话框、状态消息）en/zh 双语完整；切换不改模型与撤销。

## 当前实现与验证状态

- A/B/C/D/E/F/G 全部已实现。`./scripts/cargo.sh test --all-targets` 281 项通过；参数传输与光标专项 21 项（`src/transfer.rs` 9 项模型层 + `src/app/ui_tests.rs` 12 项交互回归）。
- 真机历史：第 1 版无悬停链路；第 2 版吸管经 macOS 光标位图（NSCursor）送达、真机不可见，已废弃；现版为画布内绘制字形（隐藏系统指针 + 每帧绘制）。**逐条手持验收待用户在新包执行**。

## 检查环境提示（避免误判）

1. 本自动化环境无法按住修饰键移动鼠标（系统级拦截）：`⌥`/`⌘⌥` 悬停链路只有无头 egui 回归证据，只能人手验证。
2. 本环境同会话非首个 OpenArchi 实例窗口内容不合成（与 bundle id 无关）：截图验证只对会话首个实例有效。
3. egui 画布点击经自动化不落地：画布交互验收需人手执行。

## 检查入口

- 行为契约（含证据分级）：[parameter-transfer.md](parameter-transfer.md)
- 实现：`src/transfer.rs`（模型层传输逻辑）、`src/transfer_cursor.rs`（画布字形绘制）、`src/transfer_ui.rs`（命令/一次性模式/对话框/亮显）、`src/editor.rs`（`pick_up_from`/`inject_into`/`set_transfer_settings`）、`src/app/plan.rs` + `src/app/model_view.rs`（手势拦截与指针反馈）、`src/cursor.rs`（常规反馈样式）
- 回归测试：`src/transfer.rs` tests、`src/app/ui_tests.rs`（`transfer` / `alt_hover` / `cmd_alt_hover` / `drawing_tool` 相关用例）
- 验证记录：[../validation.md](../validation.md) 2026-10-24 各条（含历次失败原因与打包 UUID）
