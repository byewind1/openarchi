# AC29 Drag 原生事件补充记录 · 2026-10-28

此文件记录本轮实际发送的有序动作与即时 AX 返回；它不是 REA 生成的 Evidence。REA 捕获 ID 和目标身份见 ../pointer-interaction.md 与 drag-runtime-2026-10-28.json。输入脚本限定应用 bundle ID，执行前按 PID 激活并检查前台 PID；坐标为屏幕 point，截图为 REA 缩放后像素，勿混用。原生对话框仅 AX 操作。

测试实例 PID 91857，window 39607，Untitled，空白默认模板创建一条直墙。模板自带 4 个立面标记；最初 Cmd+A 包含 5 个元素，退出并单击墙后后续测试均 All Selected: 1。原项目 PID 51530 未编辑/保存。

1. 通过 REA AXPress `[0,1,5,0,0,0]` 选直墙；画布点击 (490,520)、(610,520) 创建墙；Escape 退出工具。
2. 单击墙 (540,520)，Cmd+D；移动至 (690,580)，状态 Enter Drag Reference Point，墙未移动、无跟随预览。
3. 单击 (540,520) 定基点，移至 (650,580)，状态 Enter Drag-to Point，显示参考连线和偏移轮廓。
4. Option down/up，移至 (680,600)，AX 状态 Enter Drag-to Point. [Drag a Copy] 保持。
5. 物理 Control down/up，仍 Drag a Copy；Command down/up，仍 Drag a Copy；Command+Option down/up，改为 Drag Multiple Copies。
6. 多副本下 Option down/up → 普通；再次 Option → 单副本；Command+Option → 多副本。
7. 点击 (680,600)，点击 (760,650)：每次后仍多副本、All Selected:1；Escape → All Selected:3，三个墙轮廓留下。
8. Cmd+Z 一次 → All Selected:2，最后一份消失，原墙与第一份留下。
9. 取消选择，重新单击原墙、Cmd+D、点击基点；Cmd+Option → 多副本；松开再 Cmd+Option → 普通。
10. Escape；Cmd+E → Enter Rotation Centerpoint；Escape；Cmd+M → Enter First Point of Mirroring Axis；Escape。仅确认入口，不宣称完整旋转/镜像链。

修饰键虚拟键码：Option58、Command55、Control59，事件 flagsChanged；flags Option524288、Command1048576、Control262144；释放时 flags0。普通快捷键以对应 flags 发送 keyDown/keyUp。Cmd+D key2、Cmd+Z key6、Cmd+E key14、Cmd+M key46、Esc53。

运行时限制：几何位移没有读取 AC 模型数值，只通过轮廓和状态验证交互；双击结束依官方帮助未在本段单独测量。尚未覆盖所有实体类型/吸附/3D/Tracker。
