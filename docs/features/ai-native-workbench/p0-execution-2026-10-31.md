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
