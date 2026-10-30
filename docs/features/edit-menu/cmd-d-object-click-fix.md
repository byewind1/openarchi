# Cmd+D物体上取点丢失修复

日期2026-10-28。用户已授权修复。状态：代码、自动回归和打包通过；最终包桌面实测待解锁。

## 复现与原因

本轮使用openarchi-ac29，复用已有AC29行为契约；普通OpenArchi源码缺陷不启动REA，不新增AC29行为等价声明。

用户报告Cmd+D后点物体不能继续移动/复制。检查发现PointerDrag并未按元素命中过滤；旧包独立实例PID11158中，单墙上普通完整单击可取基点并完成移动。因此不将“所有物体上普通单击必然失败”写成已复现事实。

具体失败路径已用完整workspace的egui事件复现：选中墙→Cmd+D→墙中部左键按下→保持左键移动。旧实现仅在response.clicked()时记录基点，egui将该序列识别为drag后，状态仍为Enter Drag Reference Point。新增测试修复前失败，输出expected Enter Drag-to Point / actual Enter Drag Reference Point。

## 修改

`src/app/edit_menu.rs::pointer_drag_canvas`：命令会话内画布左键按下即记录基点；坐标使用press_origin，避免同帧鼠标位移改变基点；会话的reference_button_down标记消费这一次松开事件，防止小幅抖动被识别成click后意外提交。随后点击按原模式移动、单副本、多副本。画布内不按原件、其他元素、墙身/节点/空白作限制；控件和画布外仍不取点。未激活命令时的选择/端点编辑行为不改。

新增回归：
- cmd_drag_object_reference_press_then_motion_is_not_lost：完整工作区，墙上取点后按住移动，普通/Option单副本/Option+Cmd多副本；墙端点上放置，选择与模型保持、逐份Undo。
- cmd_drag_reference_accepts_any_canvas_point_and_consumes_jitter_release：已选墙端点、墙身、另一条未选墙和空白处取点；1px抖动松开不提交、不改选择；下一击准确移动1m，另一条墙不变，Undo恢复。

## 构建验证

`python3 scripts/ac29-workflow.py verify --force`：344 Rust passed、1性能test ignored；15 tooling passed；format（含include文件）、cargo check --locked --all-targets、localization、diff、package全部通过。

`dist/OpenArchi.app`与release arm64 UUID：`6D9C20DF-E324-3692-A1EE-1AB1C1CF6D9A`；codesign --verify --deep --strict passed。后续bundle检查确认与当前源码成功receipt匹配。

receipt：`target/ac29-workflow/runs/20261028T154449Z-77cbd3ec/report.json`。

## 桌面实际步骤和限制

原用户应用PID69192未操作或关闭。旧包独立测试PID11158在隔离HOME `target/cmd-click-smoke-home`恢复4m单墙，选墙→Cmd+D→墙上完整单击取基点→空白点击落位正常，用于排除“普通单击一定失败”的过度结论；随后退出该测试实例。

最终包PID67637已启动，但系统再次锁屏，NSWorkspace前台为com.apple.loginwindow；输入工具前台PID守卫拒绝全部输入。没有将失败输入或空AX记录归档为成功证据。已请求用户解锁；本包墙上取点、复制/多副本、撤销实际鼠标验证和中英桌面切换仍unknown。没有新增或修改语言词条，复用既有翻译。用户真实HOME偏好未改。不能把旧包普通单击成功视为新包修复已通过桌面验收。

## 经验候选（待确认，不写AGENTS）

- CAD取基点应分别测试mouse-down、按住位移和mouse-up；只用完整click辅助函数会漏掉被框架归为drag的输入。
- 将取点的松开事件与后续确认分开消费，避免轻微抖动产生一次意外模型提交。
