# P0 执行记录 · 2026-10-31

授权：用户当前对话“按计划去实施”。主线为2026-10-31架构计划的P0；本轮交付首个构建/能力盘点/存储完整性切片，P0整体及P1–P5未完成。

## P0.1 当前环境与历史冲突

工作树初始快照：[源码SHA-256和git状态](p0-source-start-2026-10-31.json)。保留所有已有改动，未提交、重置或清理用户文件。macOS arm64、Rust 1.98.1、Cargo 1.98.1可用。AC29 INT5101身份复用，未启动或操作Archicad。

计划记录的 `20261030T191401Z-c09e65e3` 链接失败是历史收据；当前latest为后续 `20261030T191701Z-0793e2d5`，461 Rust/15 tooling、包78EE8804通过。此发现只解决记录先后冲突，不将失败归因于缓存；当前源码新鲜度仍以本轮force验证为准。

## 本轮实施

- P0.2 第一版[对象/入口/视图/回归/限制矩阵](p0-capabilities-2026-10-31.md)，区分源码、自动与历史原生证据。
- P0.4 首轮写入口审计：apply与legacy commit共存，commit缺少统一validate，仍须继续按入口核验；未借重构改变既有手势。
- P0.5 修复具体临时文件破坏反例；[改代码前契约](../persistence/contract.md)。4项新增文件系统回归，涵盖16写者并发、符号链接、非法项目和rename失败。
- P0.6 更正文档中墙方法切换丢草稿、直接插点、楼板仅矩形、无墙连接、板无法任意旋转和固定源码行数等陈旧说明；修复指向不存在根目录计划的入口。

## 退出门槛与下一切片

P0.3 完整楼板面板仍缺当前包原生全手势、双语与重开证据；不将已有自动测试提升为原生验收。保持斜边裁剪、独立半径锁、复制修饰键为unknown。P0.5共享恢复位置和文档级多实例覆盖仍未解决；本轮只保证候选文件互不截断。P0未达到整体退出门槛，暂不进入P1。

经验候选，待维护者确认后才可写AGENTS.md：保存完整性测试应覆盖“已有临时文件/符号链接/并发写者/rename失败”，唯一临时文件只解决候选所有权，不能代替文档revision冲突检测。

## 本轮最终验证与交付

`python3 scripts/ac29-workflow.py verify --force` PASS：465项Rust测试，15项工具测试，1项既有性能测试ignored；格式（含include）、all-targets check、本地化、diff空白、新打包均通过。收据：`/Users/ren/MAC工作/工作/code/开源项目/openarchi/target/ac29-workflow/runs/20261030T201614Z-d8346a8c/report.json`。本轮未运行可选性能基线，不更新历史性能数字。

应用：`/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`。release/bundle arm64 UUID均 `0B031BCF-7C67-336C-97DC-26A39F4660C5`，deep/strict签名PASS，源码来源绑定见收据。新增4项存储回归；同组测试对旧实现4项全失败（exit101），修复后全通过。

最终包原生烟测（CUA桌面证据，不是REA Evidence）：新建隔离HOME，启动本轮唯一openarchi进程PID30131，ps确认HOME及可执行文件为本次dist包。窗口Untitled，AX显示英文Info Box、Toolbox、12 elements、View/File菜单；截图3024×1976像素（与1512×988点布局一致，但未独立读取CG窗口尺寸，显示比例标为推断2×）。通过File > Save JSON实际打开Save project面板。其后CUA截图坐标及点坐标点击Save、Return、AX Raise后Escape均未触发保存/取消；AX状态不变，截图仍显示模态面板，故保存/重开、3D切换、完整楼板鼠标、双语切换均unknown。不能把AXPress/点击返回成功作为应用执行证据。测试进程结束前未产生保存文件，未操作用户工程、Archicad或原语言偏好。

本轮无用户可见文案变动；locale验证与全量双语自动事件回归通过，不声称新包全局双语原生验收。旧标题栏入口按批准差异仍不恢复。

## P0.4 第二切片：改代码前核对（2026-10-31）

本轮用户“继续做该做的，去开发”授权继续P0。受影响入口：`Editor::commit(before)`、类型化 `AddElement`/`RestoreElements`。命令前提是有效Project；只强化已有模型有效性和锁定规则，保持合法操作、手势、预览、确认取消、图标、界面布局、文件schema、Undo/Redo含义不变。真实鼠标/热点不涉及新增状态；最终包启动仍须核验。

可读源码依据：commit仅检查既有锁定元素后直接建delta，未运行Project::validate；AddElement先计算`element.id + 1`，`u64::MAX`会debug panic或release回绕；RestoreElements没有图层锁定预检，虽然AddElement已拒绝同图层放置。复用Project::validate、既有错误文本和delta历史，不实现第二套几何校验。

行为证据：图层锁定禁止编辑/删除/新放置来自[AC29图层契约](../layer-settings/contract.md)的public documentation（025_Attributes-5，目标INT5101）。RestoreElements作为外部执行的添加批次必须受此规则；Undo/Redo直接revert/replay历史delta，继续按已有历史语义恢复，不调用新放置的锁定预检。无新AC29运行观察，不声称全局等价。

目标：非法legacy候选恢复完整before，不改变revision、undo、redo、saved或dirty基线；类型化非法ID返回现有身份错误而非panic；RestoreElements批次任一元素处于锁定图层时整个批次拒绝，且不消耗redo。合法创建→commit→undo/redo→JSON仍可用。现有锁定元素守卫保留。其它旁路、目的图层重新指派规则、同事务解锁并放置、文档级冲突不在本切片内，不以本修复宣称其已解决。

验收反例：非法厚度与重复ID/无效楼层legacy候选；存在redo时失败不清历史；最大ID添加；合法+锁定图层的混合恢复批次；合法legacy提交后typed更新、撤销重做与JSON往返。先运行失败回归，再修复和全量验证打包。经验候选：校验必须先于delta生成，基于稳定ID的差异不能用于恢复非法重复ID候选；待结果后提交维护者确认，不修改AGENTS.md。

首次全量验证（`20261030T202945Z-4e993304`）在2个UI测试失败，未打包：`quick_option_visibility_follows_layers_and_design_options`、`layer_settings_ok_applies_combination_and_undo_restores` 直接设置elements但保留默认next_id=1，导致ID1/2不满足`id < next_id`。这两项基线夹具原本无效。只在构造夹具处设置正确next_id并先断言validate通过，保留原显示/图层组合/Undo断言；不削弱生产校验，不改变图层语义。其余411个lib测试已通过，但失败运行不记为最终通过。

### 第二切片最终结果

已修复：legacy commit在差异生成前验证完整Project，失败恢复before；AddElement对ID递增使用checked_add，最大整数返回身份错误；RestoreElements预检整个批次的锁定图层。合法旧/新命令、顺序恢复、undo/redo、dirty、revision、JSON均有回归。新增5项测试，旧实现前三项反例失败、合法路径通过，见[失败节选](p0-editor-atomicity-before-2026-10-31.txt)。两个既有UI夹具补齐next_id并断言有效，原断言保留。

`verify --force` PASS：470项Rust + 15项工具测试，1项既有性能测试ignored；格式/include、all-target check、localization、diff、新打包通过。成功收据 `/Users/ren/MAC工作/工作/code/开源项目/openarchi/target/ac29-workflow/runs/20261030T203045Z-c6ecacb6/report.json`，源码输入SHA-256 `ccf332dad3e7eb9bc29ef8a6f91838a7c17593b3cfb50db15b77d8e74e1e0972`。应用 `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`，arm64 release/bundle UUID均 `ECCF275E-93C5-34C9-9D78-F3C4AC6AFA27`、deep/strict codesign PASS。首次2项夹具失败的运行已被本次成功收据替代；没有把失败运行记为通过。

最终包CUA原生烟测：隔离HOME `/var/folders/4k/21kvrwq54vn__xzl25vgwkt80000gn/T/oa-p0-writeguard-48gwok72`、新PID53022，ps核对HOME/current dist路径；英文Untitled、12 elements启动通过；通过实际AX控件点击Perspective 3D，AX切换状态且截图显示墙板三维实体；点击Floor plan返回，AX两项状态恢复，12 elements/0 selected可见字段不变。截图3024×1976像素；未独立测量CG窗口逻辑尺寸，不推断热点或比例。测试实例已结束，未碰用户项目/偏好。此检查不证明完整模型/脏状态/历史不变，仅自动回归验证对应维度。

本轮没有新增或修改界面文案、图标、布局或鼠标手势；复用现有验证错误及锁定错误。未执行本轮新包双语切换、全套楼板鼠标、故障状态的真实手势、保存重开；这些原生维度仍unknown，不套用旧包结果。提交错误传播（void commit调用者覆盖错误状态）、其余锁定旁路、共享恢复文件冲突与P0整体退出仍待后续工作。本轮未提交或推送，保留工作期间出现的非本任务目录existing-building-reconstruction。

待确认可复用规则：

- 先验证完整候选再生成基于ID的delta；非法重复ID候选必须从完整before回退，失败不得清redo或推进revision。
- UI回归夹具直接设置elements时必须同步next_id并在操作前validate；不能因旧夹具不合法而放宽生产校验。
