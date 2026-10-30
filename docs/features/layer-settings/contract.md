# Layer Settings 对话框与图层逻辑 (Cmd+L)

日期:2026-10-24(证据采集)/2026-10-25(定稿)。目标版本:Archicad 29.0.0 v1 INT (5101),
可执行文件 SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。

## Goal and bounded scope

复刻 AC29 的 Layer Settings 对话框(macOS 快捷键 Cmd+L 打开,窗口标题
"Layers (Model Views)")及其图层内在逻辑:图层状态(显示/隐藏、锁定/解锁、
3D 实体/线框)、相交组(Extension 列)、图层组合(Layer Combinations)、
专业分类树(Design > Architectural/MEP/Structural;Documentation;Viewpoint)、
搜索与 "Show: X of Y"、OK/Cancel 语义,以及图层状态对平面/3D 显示与编辑的实际影响。

排除(本轮不做,不放假开关):Quick Layers 悬浮工具条、Teamwork 预留、按组合打印
PDF 列表、XREF 图层过滤、组合删除(帮助页未覆盖)、图层名称扩展(name extension)
排序与 "Filter by extension" 弹出菜单。

## Entry point

- Cmd+L(运行时观察:发送 Cmd+L 后新窗口 "Layers (Model Views)" 出现)。
- 帮助文件(025_Attributes-4):Ctrl/Cmd+L、Quick Options 的 Layer Settings 图标、
  Options > Element Attributes > Layers、Document > Layers > Layers、Attributes 工具栏图标。

## Observed dialog structure (runtime observation)

证据截图(AX 快照 s00000022–s00000027,窗口 id 5452,825×580 pt,深色主题):

- `evidence/ac29-layers-dialog-collapsed.png` — 树收起、根节点选中。
- `evidence/ac29-layers-architectural-selected.png` — Design 展开、Architectural 选中。

结构(自 AX 树与截图):

- 模态对话框,底部右侧 OK / Cancel。
- 左面板 **Layer Combinations**(列头 Name | Status;底部 New… 按钮)。
  本项目模板默认 10 个组合:01 Site、02 Drafting、03 Plans - Preliminary、
  04 Plans - Approval、05 Plans - Detailed、06 Plans - Mechanical、
  07 Plans - Structural、08 Rendering、09 Layouting、10 Show 3D Zones as Solid。
- 右面板 **Layers**:
  - 顶部 "Search Layers" 搜索框;
  - 分类树(文件夹视图):根 "Layers" > Design >(Architectural、MEP、Structural)、
    Documentation、Viewpoint;根节点还直接挂着 "Archicad Layer" 与 "- Hidden" 两个图层
    (根选中时表格恰好显示这 5 行,Show: 5 of 5);
  - 图层表格列(截图判读,static artifact analysis):[眼睛][锁][3D 模式(实心方块)]
    Name | Extension(本模板所有行均为 1);列头 Name/Extension 可排序(帮助文件);
  - 表格底部 "Show: X of Y";选中 Architectural 后表格过滤为其 15 个图层
    (Finish - Ceiling/Floor/Wall、Interior - Equipment/Furniture/Lamp/Partition/
    Stair & Railing、Model Unit - Module/Zone、Morph - General、Shell - General/Roof、
    Site & Landscape - General/Terrain);
  - 表格底部 "New" 按钮 + 带下拉菜单的小按钮(菜单内容未观察)。
- 未能观察(进程在后续像素点击后崩溃,详见 Risks):眼睛/锁切换后的闭合图标形态、
  组合点击后的整表状态覆盖、New 分裂菜单内容、左上角 16×16 选项菜单内容。

## Internal logic from official help (public documentation)

来源:AC29 INT 帮助 025_Attributes-4/-5/-6/-8(Layers dialog、Layer status、
Layer Combinations、Use layers to prevent intersections),交叉参考 AC18 附录
Layer Settings Dialog Box(标注为 AC18,仅用于列语义细节)。

1. **显示/隐藏(眼睛)**:隐藏图层的元素不显示。可多选整组/整文件夹切换;
   文件夹内状态不一致时显示"混合"状态。全部显示:Document > Layers > Show All Layers。
2. **锁定/解锁(锁)**:锁定图层的元素不能编辑、不能删除,也不能在其上放置新元素。
3. **3D 视图模式(第三列)**:实体 vs 线框,仅影响 3D 视图,独立于全局 3D 样式。
4. **Archicad Layer**:不能删除、不能隐藏、不能锁定;收集丢失图层定义的元素。
5. **相交组(Extension 列数字)**:只有图层相交组编号相同的元素才相互连接/打断;
   默认 1;两元素同为 0 时不相交;编号与优先级无关;隐藏不影响相交
   (同组隐藏可导致"丢失线",修复方法是改组号)。
6. **图层组合**:命名的图层状态集合;每个视图随存一个组合;出厂默认 10 个;
   应用途径:Quick Options 弹出菜单、Layer Settings 中选择后 OK、Document > Layers
   点击组合、View Settings。编辑:选中图层改状态后点 Update 写回组合;
   New… 新建组合、命名、设状态、Update。
7. **文件夹视图**:按分类树组织;选中文件夹可批量编辑其下所有图层状态;
   列表视图/文件夹视图可切换。
8. **删除图层**:其上元素一并清除、不可撤销,需警告并可改移到替代图层
   (AC18 附录;AC29 页未覆盖。OpenArchi 按可撤销实现:非空图层弹警告,
   元素改挂替代图层后删除,一次撤销恢复)。
9. **排序/过滤**:点列头按 Name/Extension/状态排序;搜索框过滤;
   底部显示 "Show: X of Y"。

## Evidence table

| Claim | Type | Source | Limits |
|---|---|---|---|
| Cmd+L 打开 "Layers (Model Views)" 模态窗 | runtime observation | CUA hotkey 2026-10-25,快照 s00000022 | 单次观察 |
| 左=组合列表(10 个默认名)、右=图层树+表格 | runtime observation | 同上 + 截图 | 模板项目 GDL 测试 2.pln |
| 分类树 Design>Architectural/MEP/Structural、Documentation、Viewpoint;根挂 Archicad Layer、- Hidden | runtime observation | 展开操作 s00000024–27 | 仅展开 Design 一层;MEP/Structural 子图层未观察 |
| Architectural 15 图层名单 | runtime observation | 选中 Architectural 后表格 | — |
| 表格列:眼睛/锁/3D/Name/Extension(值 1) | static artifact analysis | 截图放大判读 | 图标像素级形态有限;闭合态未见 |
| 状态语义(隐藏/锁定/3D/Archicad Layer 保护) | public documentation | AC29 025_Attributes-5 | — |
| 组合语义(存储状态、Update、应用途径) | public documentation | AC29 025_Attributes-6 | Update 按钮在截图中未直接可见,位置 unknown |
| 相交组语义(同号相交、0 不相交、默认 1、隐藏仍相交) | public documentation | AC29 025_Attributes-8 | — |
| 删除图层不可撤销+元素清除 | public documentation | AC18 附录(标注版本) | AC29 页未覆盖;按可撤销保守实现 |
| New 分裂菜单/左上角 16×16 菜单内容 | unknown | 未观察 | 不声称 |
| MEP/Structural/Documentation/Viewpoint 的具体默认图层 | inference | 未观察;按专业常识构造 | 用户可自行增删 |

## Behavior contract (OpenArchi 实现)

Entry:Cmd+L(命令修饰键);Options > Element Attributes > Layers…;Document > Layers > Layers…。

1. 对话框为模态:打开时克隆当前图层文档为工作副本;OK 提交(一次可撤销命令,
   含元素图层改挂);Cancel 丢弃并关闭;Esc 等同 Cancel。
2. 左面板组合列表:单击选中组合,OK 时应用(把状态写回全部图层);
   New… 以当前状态快照新建 "New Combination N";Update 把当前图层状态写入选中
   组合(工作副本内);Status 列对"当前生效"组合打勾(状态与组合快照一致才算)。
3. 右面板:搜索框按名称子串过滤(不分大小写);分类树展开/收起,选中分类过滤表格
   (根=全部);表格行:眼睛/锁/3D 三态按钮(Archicad Layer 三者禁用)、
   名称(双击重命名)、Extension(0-99);表头 Name/Extension 点击排序;
   底部 "Show: X of Y";New=新建图层(挂选中分类,无则挂根),下拉含
   New Layer / New Folder。
4. 删除:非空图层先警告(元素数),确认后元素改挂替代图层(默认 Archicad Layer)
   再删除,整包一次撤销;空图层直接删除;Archicad Layer 不可删。
5. 模型效果:element_visible 由图层文档驱动(隐藏即不渲染平面/3D);锁定图层元素
   不可编辑/删除,工具不可在锁定图层放置(状态栏提示);墙体相交清理仅当两墙
   图层同组且组≠0。
6. Quick Options:组合弹出菜单列出文档组合,点击=应用(可撤销);
   "Show All Layers" 勾选 ⇔ 全部图层可见;勾上=全部可见;取消=重新应用当前组合。
7. 持久化:Project.layers(LayerDocument)随项目 JSON 保存;旧文件缺省=默认文档;
   旧 Element.layer 枚举字符串按原 8 枚举映射到对应默认图层。
8. 本地化:全部界面文案 en/zh-CN 双语。

## Finite acceptance checklist

| Scenario | OpenArchi result | AC evidence |
|---|---|---|
| Cmd+L 打开/OK/Cancel/Esc | pass(ui_tests `cmd_l_opens…` + 桌面冒烟) | runtime observation |
| 根节点选中显示全部(5 行:3 分类+2 特殊) | pass(默认根选择,32 图层,Show: 2/32) | runtime observation |
| Architectural 选中过滤 15 图层,Show: 15 of 15 | pass(ui_tests `tree filter` 用例) | runtime observation |
| 搜索过滤(如 "wall" → Finish - Wall) | pass(ui_tests `layers search` 用例) | documentation + 实现语义 |
| 隐藏图层后平面/3D 不渲染;Cancel 复原 | pass(`element_visible` 测试 + 组合预览测试) | documentation |
| 锁定图层:元素不可删/编辑、不可放置 | pass(editor 守卫测试) | documentation |
| Archicad Layer:眼睛/锁/删除禁用 | pass(`protected layer` 测试) | documentation |
| 相交组:同组墙清理,异组/0 组不清理;隐藏不影响 | pass(geometry `collinear_walls_on_different_intersection_groups…`) | documentation |
| 组合应用覆盖全部状态;Update 写回;New… 创建 | pass(layers.rs 组合 round-trip 测试) | documentation |
| 删除非空图层:警告+元素迁移+可撤销 | pass(ui_tests 删除警告 + remap 提交测试) | AC18 documentation |
| 保存/重开保留图层文档与组合 | pass(serde 往返测试,Project.layers 持久化) | 实现要求 |
| 双语界面完整 | pass(桌面冒烟 EN+zh-CN 截图复核) | AGENTS 要求 |

## Risks / incidents

- **Archicad 崩溃事件**:2026-10-25 00:37,在对对话框做后台合成像素点击(切换眼睛)
  后,目标进程 PID 6914 FATAL 退出(GSReport `Archicad-20261023(19-44-32)-[6914]-1-FATAL.rpt`,
  栈为启动/Protection 代码,事件日志空,因果不确定)。已立即 `open -a` 重启 Archicad
  (PID 9043),恢复对话框留给用户处理。后续不再对该对话框做像素点击,交互细节改由
  帮助文档佐证。此事件须在最终汇报中如实告知用户。

## Delivery evidence (2026-10-24)

**源码符号**

- `src/layers.rs`:`LayerDocument`/`Layer`/`LayerStatus`/`LayerCombination`/`LayerCategory`/
  `ElementLayer`、`template()`(32 模板图层、6 分类、10 组合、active=3)、
  `legacy_layer_id()`、serde 迁移与全部单元测试。
- `src/app/layers_ui.rs`:`layer_settings_dialog`、`layer_combinations_panel`、
  `layer_tree_and_table`、`layer_settings_footer`、矢量图标
  (`eye_icon`/`lock_icon`/`model_display_icon`/`folder_icon`/`triangle_icon`/`check_mark`)。
  关键布局:模态正文用 `allocate_ui(718×420)` + 两个 `scope_builder` 定宽定高侧栏
  (egui `ui.horizontal` 会把行高压到当前行高,滚动区塌缩;不能在此处使用)。
- `src/app/mod.rs`:Cmd+L(`consume_key(Modifiers::COMMAND, Key::L)`)与
  Options > Element Attributes > Layers… 入口;`src/app/quick_options.rs`:
  `apply_layer_combination`/`apply_show_all_layers`/`layers_menu_contents`。
- `src/editor.rs`:`set_layer_document()`(经 `commit` 可撤销)+ 全部
  Add/Remove/Update/Move/放墙的锁定图层守卫;`src/command.rs`:
  `ProjectHeader.layers` 捕获/恢复;`src/view_settings.rs`:
  `element_visible(&element, &layers)`;`src/geometry.rs`:墙相交组参数贯穿 join 管线。
- `src/model.rs`:`Element.layer: ElementLayer`(legacy JSON 兼容反序列化)、
  `Project.layers: LayerDocument`。

**检查与测试**

- `cargo fmt` + `cargo check` 干净;`./scripts/cargo.sh test --all-targets`:
  252 lib + 7 + 10 + 1 + 31 = 301 项全部通过(含新增 4 个图层对话框用例、
  相交组回归、迁移与锁定守卫测试);`check-localization` 通过。
- `python3 scripts/ac29-workflow.py verify --force` PASSED(test_count 300,
  tool_test_count 15)。

**打包**

- `dist/OpenArchi.app`,UUID arm64 `1CA89A55-D18D-35D0-8E31-F8F5AA25886D`
  (与 `target/release/openarchi` 一致),`codesign --verify --deep --strict` 通过。

**桌面冒烟(隔离 HOME=/private/tmp/oa-layer-home)**

- 英文:Cmd+L 打开对话框,面板/图标/OK/Cancel 正常
  (`/private/tmp/ac29-layers/openarchi-layers-en.png`)。
- 中文:写偏好 `zh-CN` 后重启,首帧窗口标题「OpenArchi — 建筑设计工作台」,
  Cmd+L 后对话框全部控件为中文;修复组合列表遮挡缺陷(egui 水平行高压塌缩,
  见上方布局说明)并重新打包复验:10 个组合全部完整可见、无滚动条遮挡、
  对勾+高亮在 03 平面 - 初步(与底部「组合: 03 平面 - 初步」一致)
  (`/private/tmp/ac29-layers/openarchi-layers-zh-final-v3.png`,
  AX 树另证:271 元素含图层 (模型视图)/图层组合/搜索图层/显示: 2/32/确定/取消)。
- 设计决策:图层**数据名**(如 "Archicad Layer"、模板图层/组合名)按项目数据处理,
  界面 chrome 全部双语;组合名键如 "01 Site" 有 zh 译文("01 场地")故组合行随语言显示。

**剩余 unknown**

- AC29 New 按钮分裂菜单与对话框左上 16×16 菜单的具体菜单项:未运行时观察
  (像素点击有崩溃风险,已停止);OpenArchi 的 ▾ 菜单实现为 New Layer/New Folder,
  与 AC29 命名可能不同 — 待后续无风险证据补充。
- 闭眼/闭锁图标的像素形态:按帮助语义推断绘制(空心/实心),未与 AC29 像素级比对。
- AC29 顶部工具栏同步开关(眼睛/锁图标)是否存在未证实;OpenArchi 未实现。
