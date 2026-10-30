# 移动/复制距离角度Tracker

2026-10-29，用户授权。证据：用户截图要求两行距离/角度，D/A切换；AC29官方Tracker帮助 https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-40.htm 说明D/R、A、Tab、Enter、Esc。未新增REA运行时观察，不声称所有Tracker功能等价。

实施契约：移动基点确定后跟随光标显示Distance[D]与Angle[A]，中文距离/角度；0°为模型+X、逆时针为正，不沿用任意屏幕旋转角。D/R激活距离，A角度，输入替换当前值；Tab应用当前值并换字段，Enter有效时提交当前移动/副本；Esc先退出数值输入并解除本轮数值约束，移动会话仍在，再Esc结束会话。距离单位跟当前DimensionStyle：米/毫米/英尺/英寸（US Architect用十进制英尺并明确ft，不支持英尺英寸混合字符串），角度°。距离有限且>=0，角度有限可带负号；非法输入不提交，无Undo。数值输入/预览不改项目；切换Option/Cmd+Option保留数值；多副本每份一次Undo、放置后清除数值约束继续下一份。直接键入数字默认距离。只实现DA，不包含XY/Z、表达式、单位后缀、吸附参考相对角、设置面板或旋转/镜像Tracker。

验收：D距离→A角度→Enter；Tab切换；负角、毫米换算；非法/非有限拒绝；Esc两阶段；单/多副本、Undo/序列化；平面和既有3D水平编辑面共用；最终包中英文真实键盘及菜单语言切换。

## 已实现与键盘速查

实现：`src/app/drag_tracker.rs::DragTracker`管理两字段编辑、约束、校验与跟随面板；`edit_menu.rs::pointer_drag_canvas`将其结果用于同一个Transform预览和提交，参考线终点也跟随精确输入；`shortcuts`优先处理Tracker的Esc。预览始终留在临时Editor，不污染项目或恢复文件。

| 键 | 当前行为 |
|---|---|
| D / R | 编辑距离，下一次输入替换显示值 |
| A | 应用当前有效字段，切到角度 |
| Tab | 应用当前有效字段，切到另一字段 |
| 数字 / 正负号 / 小数点 | 默认开始距离输入；已有活动字段则输入该字段 |
| Backspace / Delete | 删除字符 / 清空字段 |
| Cmd+A | 下一次输入替换整个字段 |
| Enter | 有效时按当前模式放置；距离0无位移不提交 |
| Esc | 先取消数值输入与约束、回鼠标预览，再按一次结束移动 |
| Option / Cmd+Option | 保留输入，切换单/多复制，沿用既有锁存逻辑 |

示例：当前距离单位m时，确定基点后D、5、A、90、Enter，沿模型+Y移动5m。单位mm时D、2500、A、0、Enter，沿+X移动2.5m。角度可输入-90。多副本每次Enter后继续输入下一份；每份相对初始基点，Esc结束保留已放置副本。

不支持直接点击字段、左右方向键移动文本光标、混合单位字符串或表达式；当前键盘编辑按整字段替换/末尾退格工作。不是完整系统文本编辑器，不把该界面当作所有AC Tracker功能已完成。

## 自动验收与交付

verify --force通过：348 Rust passed、1 performance ignored、15 tooling passed；格式（含include）、cargo check --locked --all-targets、本地化检查、git diff --check、macOS打包通过。

新增3组事件回归：三种模式D→A→Enter、Option锁存与数值保留、两份多副本/逐次Undo、负角、Tab与毫米换算、无效/非有限/负距离拒绝、Esc两阶段、序列化、3D水平编辑面精确输入、中英字段与错误文本。纯鼠标路径保留原始向量，避免无输入时极坐标往返引入误差。

app `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`。
release和bundle arm64 UUID `7908B563-1CC2-3B50-82B0-FACA46863462`，codesign --verify --deep --strict passed；bundle检查确认源码receipt匹配。
receipt `target/ac29-workflow/runs/20261028T164152Z-9ef85f21/report.json`（UTC28日，本地29日）。

## 桌面验收状态

最终包独立PID20870/window41294，隔离HOME `target/tracker-smoke-home`，预置英文及合法4m单墙恢复样本；原用户应用PID48445未操作。尝试输入前守卫发现前台com.apple.loginwindow，全部输入被拒绝；AX无法访问恢复按钮。已请求解锁，停止输入。**新包实际键盘输入、中英菜单切换、中文重启、最终视觉质量尚未完成，均unknown。** 自动事件测试不是桌面验证。标题栏语言选择器此前按用户要求删除，唯一入口仍为View > Language；不恢复旧入口。原用户语言偏好未改。

## 经验候选（待确认，未写AGENTS）

- 精确输入必须驱动预览、参考线和最终同一模型命令，不能只改Tracker显示文字。
- 修饰键切换复制模式不应清空距离/角度；多副本每次成功放置后再清空下一份的数值约束。
- 未启用数值约束时保留原始位移向量，避免极坐标往返影响既有几何精度。
