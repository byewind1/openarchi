# OpenArchi · 按住左键直接移动与复制

2026-10-28。已按用户要求实施；AC29限定流程已实测，OpenArchi自动验收通过，新包已生成。**最终包桌面交互验收待解锁完成，不能声称已通过。**

## 已核实的原软件行为

使用openarchi-ac29与reverse-engineer-anything v42。AC29 INT5101，可执行文件SHA-256：`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。独立测试实例PID69108/window40640；原用户项目PID51530未编辑或保存。调查结束已关闭REA会话并退出本任务测试实例。

平面中未选中直墙上按下左键，10ms后立即移动且不松左键：进入Enter Drag-to Point。保持左键按Option，变为Drag a Copy；保持Option再按Cmd，变为Drag Multiple Copies。松开左键与修饰键不提交；随后连续点击两次、Esc结束，原件加两副本共选中3项。

- 多副本预览REA：`ev_7caac104caa943738e400fa9edcde7ccc32e6677dad66ab0dd4225f748a950cf`。
- 两次放置后3项选择REA：`ev_262ccda10b7d729b8862949b26ef959dcd1d1049165ddd82fce1da54828a11aa`。
- [完整5份REA记录](evidence/direct-drag-runtime-2026-10-28.json)，[逐步输入与即时AX记录](evidence/direct-drag-sequence-2026-10-28.md)，[输入辅助程序](evidence/direct-drag-input.swift)。

前两次预先选中热点拖动进入Reposition Polyline Node，是另一种编辑入口；如实保留失败/分支观察，不把它们列为整体移动通过证据。松开不提交的判断来自释放后Esc仍只选中原件；没有读取AC几何尺寸。

公开文档：[AC29 Dragging Elements](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-71.htm)，说明点击放置及多副本连续点击、双击结束；本轮精确按压入口和修饰键顺序来自运行时。

## 实现与边界

源码`src/app/plan.rs::plan`在参数吸取判断前建立`PointerDrag`，基点取press_origin，沿用`src/app/edit_menu.rs::pointer_drag_canvas`的预览、复制锁存、类型化Transform、撤销与取消。旧的整体拖动直接修改Project/释放提交路径已移除。

- 支持未选中元素按压即拖；命中已有选择保留选择集合，命中其他元素切换选择。
- 平面整体拖动Option单复制，Option保持时再按Cmd多复制；编辑过程中参数吸管不占用光标。
- 松开不提交；点击放置；多副本每次点击一次Undo，Esc保留已放置副本并选中原件和副本。
- 预览不写模型或历史；锁定验证和复制几何/元数据沿用已有模型命令。
- 已选墙线段整体拖动采用同会话，是本轮用户要求的产品选择；墙端点拉伸和显式折线形状编辑保留。不能声明全部宠物面板等价。
- 3D直接拖动仍用于视角旋转；旋转/镜像鼠标流程、Tracker和对象吸附不在此次补丁范围。

## 自动验证与包

`python3 scripts/ac29-workflow.py verify --force`成功：342 Rust tests passed，1性能测试ignored；15工具测试passed；格式（含include文件）、cargo check --locked --all-targets、本地化、git diff --check、macOS打包均通过。新增2个复合事件回归覆盖未选中/已选中、按住左键切换复制、无吸管、释放不提交、2次放置、选择3项、序列化、逐次Undo、普通移动/单副本、Esc与锁定。4个旧测试从释放提交更新为点击提交，保留原几何/撤销断言。完整墙端点测试通过。

包：`/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`。
release和bundle arm64 UUID均`E856826B-CF89-38E1-99ED-1F24094199EF`；codesign --verify --deep --strict passed。
receipt：`target/ac29-workflow/runs/20261028T151445Z-adfcd132/report.json`。

## 桌面阻塞和待验项目

隔离HOME：`target/direct-drag-smoke-home`，语言en，恢复文件为已有合法单墙样本。最终包实际启动并渲染英文恢复项目对话框；窗口截图可读取，但无法激活：CUA返回cgWindowNotFound；原生激活后前台仍为PID420 `/System/Library/CoreServices/loginwindow.app/Contents/MacOS/loginwindow`，顶层锁屏窗口覆盖桌面。确认锁屏后停止输入并请求用户解锁。曾尝试的输入不能视为成功交互证据。

待解锁后在最终包验证：恢复单墙→直接按压拖动→Option→保持Option按Cmd→释放不提交→两次点击→Esc→Undo；检查无吸管；View > Language切换中英、重启中文偏好。用户HOME偏好未修改。当前最终包这些桌面项目全部unknown；自动事件测试通过不代替它们。标题栏语言选择器此前按用户要求移除，参见native-menu-title/contract.md，保持唯一原生View > Language入口。

## 可复用经验候选（待开发者确认，未写AGENTS）

- 同名移动功能的快捷键、首次按压拖动、已选热点拖动应分别观察；入口和按压时机可能决定编辑状态。
- 在吸管/注射器处理前建立整体移动会话，修饰键由正在执行的编辑会话优先消费。
- 直接拖动验收必须拆开“按住移动”“释放”“再次点击”三步，不能把释放默认当作提交。
