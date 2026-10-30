# 吸管光标精修

2026-10-29，用户授权调整吸管图形。仅修改`src/transfer_cursor.rs::paint_pick_up`，使用原创矢量管身，不复制Archicad资源：旧粗管线+大圆头改为尖口细管、窄挡圈、圆角胶帽；主轮廓0.9pt、浅色细边增强对比度，总轴向长度约21pt。管尖保持实际指针热点；可吸取时蓝色液柱绘制在管身之上。注射器、工具栏图标及参数传递语义未改，无文字/语言包变化。

保留液柱层级回归，按新管身polygon结构调整测试的body查找，仍断言液柱位于管身绘制之后。verify --force：348 Rust passed/1 performance ignored、15 tooling passed；format/check/localization/diff/package全部通过。最终dist/OpenArchi.app与release arm64 UUID `C892F68C-7EF2-3053-8F70-27163929543B`，codesign --verify --deep --strict passed，bundle确认与当前源码成功receipt匹配：`target/ac29-workflow/runs/20261028T165741Z-02b0dbe3/report.json`。

桌面：独立最终包PID51947/window41364、隔离HOME target/pickup-polish-home，恢复4m单墙测试样本；原用户PID18621未操作。Option悬停墙体(690,550)，轮廓细、蓝色液柱可见；移到空白(690,500)，显示空管；Option点击墙体、释放Option后AX返回Picked up Wall parameters from Wall 001。输入使用用户AGENTS指定的PID/前台守卫CoreGraphics工具，恢复按钮AXPress。测试实例结束后退出，用户项目/偏好未改。

- [命中墙体](pickup-refined-primed.png)
- [空管状态](pickup-refined-empty.png)

范围：实际检查平面吸管；3D复用同一绘制函数但本次未单独桌面复测；不将此前Tracker桌面待验改成通过。未改文字，不重复全套中英切换验收。

经验候选（待确认，不写AGENTS）：细化工具光标时保留尖端热点、空/满状态与液柱层级；缩减描边不应牺牲有效目标的反馈。
