# AC29直接按压拖动：有序输入记录

日期2026-10-28，AC29 INT5101 / PID69108 / window40640。目标哈希与主契约相同。原用户PID51530未修改。独立新实例最初无项目；在该实例New建立Untitled空模板。两条测试墙画在窗口相对(480,520)→(600,520)、(480,700)→(750,700)。输入工具源码见direct-drag-input.swift，验证PID/前台/窗口内坐标；native AX使用read-only定点状态读取（[0,6,0]）及REA截图。原生按钮仅AXPress。

首次已选墙点击后拖动，进入Reposition Polyline Node，REA ev_2c141056d0384d31001fd39acf42f03d392e189607a4426b572a3db2e718e9b2、ev_3f908e04cbdf3e093b2d3b17671a396011bcba9d91db663591e4638396773def。Esc取消，不能冒称这两份是移动成功证据。

成功序列（原生AX返回，未逐步都有独立REA）：
1. Esc清空选择；光标移到(580,700)。leftMouseDown；10ms后开始10次leftMouseDragged，每步(+3,+4)间隔10ms，不发up。状态Enter Drag-to Point / All Selected:1。
2. Option down (keycode58, flags524288)，保持左键；状态Enter Drag-to Point. [Drag a Copy]。
3. Command down (55, flags1572864)，仍保持Option和左键；状态Enter Drag-to Point. [Drag Multiple Copies]。
4. REA ev_7caac104caa943738e400fa9edcde7ccc32e6677dad66ab0dd4225f748a950cf 捕获多副本预览。
5. leftMouseUp(610,740)，再Cmd up、Option up；状态仍为多副本；移动到(640,780)后Esc，All Selected:1，无新副本，释放不是放置。
6. Esc清选择，重复步骤1–3，释放鼠标和修饰键，点击(640,780)、(670,820)，Esc。All Selected:3。REA ev_262ccda10b7d729b8862949b26ef959dcd1d1049165ddd82fce1da54828a11aa。

只比较交互状态和数量，不推断实际AC尺寸或所有宠物面板的操作；全部5份REA记录保存于direct-drag-runtime-2026-10-28.json。
