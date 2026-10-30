# OpenArchi · Cmd+D 移动 REA 实测与修正

日期：2026-10-28。状态：**移动命令的基点、预览、复制切换、结束选择与撤销已修正，自动检查及限定桌面对照通过**。不是整个 Edit 菜单等价验收；旋转/镜像、Tracker、吸附仍有缺口。

## 本轮确实使用的工具和证据

已读 openarchi-ac29 与 reverse-engineer-anything v42，连接 REA 6.7.0，open_binary 绑定本机 AC29 INT 5101。使用 native-macos 的 observe_native_ui 和 capture_native_ui_scenario，不进行无关反编译。原项目 PID51530 未编辑/保存；New > Launch a new instance 新建独立 Untitled 测试，PID91857/window39607。直墙按钮采用 AXPress，画布和按键采用验证前台 PID 的原生输入；完整原始记录保存在仓库证据文件中。

- 首次 Cmd+D 后等待基点：`ev_d2bfe99ef6f908f114662312b514fe108940fd1c44a76c6f8a980588d63c6f6e`。
- 点击基点后跟随鼠标预览：`ev_8c95f6d58e657693d0c19b440b559632899c9bd0a33fa6236eb49c984428f16c`。
- Option 松开后单副本保持：`ev_c4b069aa65f91e38d2649239d4c8c50c8f923f21df9a5b7c258028bd964f6f8a`。
- Cmd+Option 多副本：`ev_2e959a58f7deac81075d6a7ca47c551cffec3724cbbe3937ddf2d3085b315073`。
- 两份副本后 Esc，原件加副本共3项全选：`ev_f9ed88a036cd307fb1b6b3fcf6325cc9cd637694180f2ffa310f1131cc388528`。
- Undo 一次保留2项：`ev_420382cb48664a580e34f29a127131b522f97a4571c0e8cb5da18527d627b3a6`。
- 最终 OpenArchi 包中文完成态：`ev_43d6fd8e65002e7a1e809da5d217f1146156ac2d9ae4dfd5954b9a458f9ab88a`。完整有序操作另有最终包英文/中文 AX 记录，不能用这一张最终态截图单独证明整个流程。

## 改正了什么

1. Cmd+D/菜单 Drag 不打开模态，也不沿用上一帧鼠标点：第一击确定基点，移动鼠标预览，第二击确认。
2. Option 切换单副本并锁存；Cmd+Option 切换多副本并锁存。多副本再 Option 或再次 Cmd+Option 返回普通移动。Mac 物理 Ctrl 在本机试验中不触发多副本，已移除先前猜测的别名。
3. 多副本每次落位一次 Undo；结束/取消下一份预览时，保留已放置份数，选择原件及全部副本。Undo 保留还存在的已选元素，排除已经撤除的 ID。
4. 预览不写项目、历史或恢复文件；一帧内的修饰键按下/松开也能处理。英文/中文阶段提示都有对应词条，结束状态使用已有 READY/就绪。

## 有限对照结果

双方起点均为平面中单选一条直墙，无锁定；仅比较交互阶段、命令状态、对象和选择数量。AC 墙长未读取数值，**不宣称几何尺寸级跨产品等价**。

| 检查 | AC29 | 最终 OpenArchi | 结论 |
|---|---|---|---|
| Cmd+D 无模态，等待基点 | REA状态/截图 | 英文/中文AX | pass |
| 第一击基点，随后移动预览 | REA截图 | 原生操作/事件回归 | pass |
| Option 锁存单副本、Cmd+Option 多副本 | REA+定点AX | 原生AX与事件回归 | pass |
| 模式重复按键退出、物理Ctrl不充当Mac多副本 | 原生事件后定点AX | 事件回归 | pass（OpenArchi自动验证） |
| 两份放置后Esc，原件+两份选择 | REA 3项 | 最终包3项 | pass |
| 一次Undo剩2项且仍选中 | REA 2项 | 最终包原生菜单2项 | pass |
| 拷贝坐标/原件不变 | 未读取AC尺寸 | 4m墙，副本偏移2m/4m，JSON已核对 | OpenArchi pass；跨产品 unknown |
| 序列化及恢复 | 本轮未测AC保存 | 自动JSON回归、真实恢复单墙、恢复文件含副本 | partial；非PLN兼容声明 |
| 完整3D编辑面、Tracker、吸附、全部类型 | 未覆盖 | 仅当前楼层水平面自动测试；Tracker/吸附未接入 | unknown/未完成 |
| 旋转与镜像 | 仅入口提示观察 | 仍为数值入口 | 未完成 |

## 验证与应用交付

`python3 scripts/ac29-workflow.py verify --force`：340 Rust tests passed、1 performance test ignored；15 tooling tests passed；格式、cargo check --locked --all-targets、本地化检查和diff检查通过。

最终包：/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app。release与bundle arm64 UUID 均 `E25DBA40-EC95-3424-AEA1-D96269C31FD8`；codesign deep/strict通过；bundle检查与成功源码receipt一致。
最终运行PID45500，隔离HOME `target/rea-drag-smoke-home`，用户实际HOME偏好没有改动。新包完成英文移动/复制/多副本/取消，中文基点/单副本/多副本/取消，以及菜单撤销。

语言：真实新实例首帧为中文；同一最终实例经 View > Language 完成 English→简体中文→English；标题、顶层菜单、工具栏、Info Box、工具箱、视图标签、选择/构件数量、状态栏、Quick Options条已查看。切换前后恢复项目JSON完全相同；选择保持，Undo/Redo菜单状态及撤销结果正常。Quick Options浮动面板、无关对话框和原有部分3D子菜单译名未逐项验收，不能声称全应用本地化完成。标题栏语言选择器已在此前用户要求的原生标题栏改造中移除（native-menu-title/contract.md已有记录），本轮使用现有唯一语言入口，没有恢复它。

## 经验候选（等待开发者确认，不写入AGENTS）

- 编辑复刻先观察状态提示的阶段迁移；“Cmd+D无对话框”不能推导“激活时鼠标位置就是基点”。
- 同时验收取消与Undo后的选择集合，不能只核对构件数量与几何。
- 修饰键轻按要消费事件序列；只读帧末按键状态会漏掉同帧按下/松开。

源码与构建receipt：/Users/ren/MAC工作/工作/code/开源项目/openarchi/target/ac29-workflow/runs/20261028T142846Z-b5b581b0/report.json

## 证据文件

- [完整AC29 REA证据](</Users/ren/MAC工作/工作/code/开源项目/openarchi/docs/features/edit-menu/evidence/drag-runtime-2026-10-28.json>)
- [最终OpenArchi REA证据](</Users/ren/MAC工作/工作/code/开源项目/openarchi/docs/features/edit-menu/evidence/openarchi-delivery-2026-10-28.json>)
- [原生输入序列](</Users/ren/MAC工作/工作/code/开源项目/openarchi/docs/features/edit-menu/evidence/native-input-sequence-2026-10-28.md>)
- [交互契约](</Users/ren/MAC工作/工作/code/开源项目/openarchi/docs/features/edit-menu/pointer-interaction.md>)
