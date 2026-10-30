# 三维环绕旋转与底视

2026-10-29，用户授权实施。使用OpenArchi原创环形双箭头光标，不复制Archicad私有资源，不声明AC运行时等价。

源码依据：model_view的pitch夹在0.08..1.53，导致无法越过水平；结束反馈覆盖Grabbing。Camera::matrix使用世界Z为上方向，模型渲染已包含底面且不剔除背面。

契约：三维左键拖动、Shift+中键旋转显示同一环形箭头；拖动暂停时仍显示，松开恢复正常反馈；平移仍是原反馈，旋转优先于Option吸管。允许pitch从正值经过0到负值，范围±1.53弧度，保留极点前限制避免look_at退化；保持世界竖直、不滚转。导航不改项目/选择/Undo，不写入项目持久化；3D底面使用现有网格。测试水平线穿越、上下限矩阵有限、静止时光标、释放与平移、选择和历史不变。最终包桌面实测另记录，不把编译当作GPU验证。

## 2026-10-29 实施与验收

已实现：`src/app/model_view.rs::model_view`在旋转时使用±1.53仰角限制，旋转状态优先于吸管/普通拖动反馈；Shift+中键状态按按钮保持判断，暂停移动不闪回普通光标。`src/transfer_cursor.rs::paint_orbit`原创双弧箭头，深色外描边+浅色线，适配模型亮/暗表面。图形不含新文字，不改变语言包。

自动验证：`python3 scripts/ac29-workflow.py verify --force`成功，345 Rust tests passed、1 performance ignored，15 tooling passed；format（含include文件）、cargo check --locked --all-targets、localization、diff检查、macOS打包均通过。新增事件回归覆盖左键/Shift中键、穿过水平线、暂停光标、释放、矩阵有限、项目/选择/Undo不变；既有普通中键平移回归通过。

最终app：`/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`。
release和bundle arm64 UUID均`6C1ABA5B-EE6F-3647-AEC0-98D09BD2991A`，codesign --verify --deep --strict passed。bundle命令确认匹配当前源码成功receipt：`target/ac29-workflow/runs/20261028T161755Z-dd034f54/report.json`（UTC日期28日，本地29日）。

桌面验证：最终包独立PID22934/window41218，隔离HOME `target/orbit-smoke-home`，默认12构件院落样本；用户原实例PID72149未操作。采用用户AGENTS指定的PID守卫CoreGraphics输入、精确窗口截图、语言菜单AXPress。

实际序列：F3进入三维→左键按住从窗口相对(690,650)拖到(690,450)→暂停截图，模型底面和环形双箭头清晰可见→松开；Shift+中键(690,450)→(730,550)回水平上方，显示相同光标；View > Language经AXPress切简体中文，Shift+中键(730,550)→(730,450)再次看到底面；松开→普通中键(730,450)→(760,470)平移，模型平移且不再绘制旋转光标；切回English。语言只在隔离HOME变化。未修改/保存用户项目，模型保持12项/0选中；自动测试补充验证Undo为空和项目完全不变。

证据：
- [英文底面+保持旋转](evidence/orbit-bottom-held-en.png)
- [英文释放](evidence/orbit-bottom-released-en.png)
- [英文Shift中键](evidence/orbit-middle-held-en.png)
- [中文底面+旋转](evidence/orbit-bottom-held-zh.png)
- [中文普通平移](evidence/orbit-pan-zh.png)
- [英文AX](evidence/orbit-en-ax.txt)、[中文AX](evidence/orbit-zh-ax.txt)

边界：未仿制Archicad原生光标资源，也没有新增AC29 REA观察。保留距上下极点约2.3°限制，不支持跨极点翻滚。相机导航不持久化到项目。本次未改文字，不宣称原有三维角标/帮助提示等已全部本地化；中文截图仍有既有英文提示。未对语言重启或无关对话框做全量复测。

## 经验候选（等待确认，不写入AGENTS）

- 导航光标应按手势是否持续判断，不能依赖这一帧是否有鼠标位移；否则暂停时会闪回普通光标。
- 水平线不是相机奇点；Z-up环绕应允许正负仰角，只在接近竖直极点处保留稳定性限制。

## 2026-10-29 旋转光标精修

用户反馈原光标粗糙，本次仅修改`paint_orbit`原创矢量绘制：半径11→8.5pt，主线2.2→1.4pt，深重外描边改浅色细keyline，折线箭头改实心三角形，去除中心圆点，弧线采样18→32段。手势和仰角不变，无新文字、无语言包变化。

verify --force：348 Rust passed/1 ignored、15 tooling passed，格式/check/localization/diff/package全部通过。当前dist/OpenArchi.app与release UUID `A7A5838A-8DD5-3A95-ABF9-1974AFDA730F`，strict/deep签名通过、源码receipt匹配：`target/ac29-workflow/runs/20261028T165109Z-ae746401/report.json`。

最终包独立PID95076/window41330，隔离HOME `target/orbit-polish-home`，用户实例PID68395未操作。F3→左键按住从(690,650)拖到(690,450)，看到底面及新版细线光标；释放后再在深色底面(770,630)→(780,630)验证对比度，释放后结束。实际窗口截图：[浅色表面](evidence/orbit-refined-held.png)、[深色表面](evidence/orbit-refined-dark.png)。没有改文字，未重复全套语言/Tracker验收；此前Tracker桌面待验状态不因此变为通过。测试实例结束后退出，未改用户项目或偏好。

新增经验候选（待确认）：小尺寸旋转光标用实心箭头配细弧线，避免两条粗描边在箭尖交叠；在最终包的深浅模型表面检查，而非只看独立图标。
