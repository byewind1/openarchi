# OpenArchi 的实现与验收易错点

## 本机工具链

- 工作流脚本只用 Python 3.11+ 标准库，本机 `python3` 为 3.14.6。维护技能时，skill-creator 的 `quick_validate.py` 额外依赖 PyYAML；本机 `python3` 没有此包，现成 `/Users/ren/miniconda3/bin/python` 已有 PyYAML 6.0.2，可用来运行该校验器，不必重新安装环境。

- 运行 `./scripts/cargo.sh …`，不要裸用系统 `cargo`。本机系统曾指向旧 Homebrew Rust 1.72；仓库 wrapper 选择 rustup stable。当前项目要求 Rust >= 1.95，edition 2024。
- rustfmt 曾缺失，后来已安装。验证工具只诊断；缺失时按当前权限安装官方组件，不把自动安装写进每次开发流程。
- `src/app.rs` 通过 `include!("guides_ui.rs")` 引入 UI；`cargo fmt --all -- --check` 不保证检查 include 文件。工具扫描 Rust include 并补做 rustfmt 检查。
- 几何单位是 f32 米；AC29 此环境的距离输入显示毫米。把换算写在契约和测试预期里，避免将 1000 mm 传成 1000 m。

## 几何和事件

- 几何/验证留在域模块，UI 负责预览与提交；一次手势一次 undo。序列化新字段使用兼容旧 JSON 的默认值，并在 import/export 验证。
- `Project` 快照是历史记录，`Editor::undo/redo` 会清除绘制锚点；测试不要假定 undo 后仍保留未完成的绘制。
- RawInput 要按真实 pointer move → press → move/release 分帧。拖动起点取 `press_origin`，不要只命中移动后的鼠标位置；快速拖动可能早已离开细线。
- 擦除不能只测试每帧光标点，应检测相邻采样间的扫过线段。弹窗必须阻止背景工具和快捷键产生模型变化；Apply/Cancel/Escape 都验证。
- egui 0.36.2 无渲染器的测试，返回的 `FullOutput.textures_delta` 必须显式 `clear()`，否则测试析构会 panic。现成例子：`src/guides_ui.rs::guide_ui_tests`、`src/app.rs::ui_tests`。
- 精确圆弧按参数计算长度、投影和交点，采样只用于显示；最佳距离等分比较实际段长误差，不是直接四舍五入段数；折线路径最近端点用光标空间距离判断。这些例子可复用 `src/guides.rs`，不必为新工具重复造轮子。
- Element Snap、网格、分点、临时参照、固定辅助线分别控制，检查关闭一种是否残留其他路径返回的吸附。

## 可复用入口（符号优先，行号不固定）

| 关注点 | 文件/符号 |
|---|---|
| 持久模型与校验 | `src/model.rs::Project`, `Shape`, `ArcSegment`, `Project::validate` |
| 编辑/撤销 | `src/editor.rs::Editor`, `commit`, `undo`, `redo` |
| 平面几何/相机 | `src/geometry.rs`, `Camera` |
| 辅助线与分点 | `src/guides.rs::GuidePrimitive`, `GuidePath`, `snap_candidates` |
| 墙体距离 | `src/distance_guides.rs::measurements`, `apply_distance` |
| 工具/事件/UI | `src/app.rs::AtelierApp`, `shortcuts`, `plan`, `model_view`; `src/guides_ui.rs` |
| 共享模型三维显示 | `src/render.rs::SceneRenderer` |
| 文件适配边界 | `src/storage.rs::JsonAdapter` |
| 中英文标签 | `locales/en.json`, `locales/zh-CN.json` |

## 交付和复用边界

- 正式本地交付是 `dist/OpenArchi.app`。签名会改变可执行文件字节；Release 与包内用 Mach-O UUID 配对，分别保存自己的哈希用于检测之后是否变更，不能要求两者原始 SHA 相等。
- UUID 相等只说明 Release 和包内属于同一次链接，不能证明它们覆盖了当前源码。`verify` 在输入前后哈希一致、构建成功后才记录源码到交付物的关系。
- 验证缓存包含未提交/未跟踪的 src/tests/locales/packaging/scripts 输入、Cargo 配置/锁文件、工具版本及构建环境指纹；文档修改不重新编译。遇到缓存未涵盖的外部依赖、工具内部变更或想重测不稳定行为，用 `--force`。本工具不是可复现构建/供应链审计系统。
- 编译通过、RawInput 通过、启动进程、窗口出现、真实手工交互是不同级别。缺少最后一项就保留 `unverified`；没有全局“与 Archicad 等价”标志。
- 格式检查失败先看是否本任务造成，局部格式化即可；不要为通过检查静默重排无关的脏文件。缓存失效不意味着应该删除用户工作或清空 REA。


## 2026-10-29 墙编辑复盘：可复用实现技巧

- 临时修饰键应路由到既有域操作，不调用会清空草稿的显式命令入口；mouse-down 的拥有者负责消费后续 release，即使中途释放 Cmd。检查 Cmd+Z 等键盘组合及字段焦点。egui 的 Mac 快捷键测试要同时设置物理 `mac_cmd` 和平台 `command`。
- 图标面板分别保存 active、hovered、pending candidate；浏览候选不清掉已接受节点和数值锁。候选晋升才转换命令；键盘切换要清旧候选，避免静止鼠标覆盖键盘选择。此晋升时序是 OpenArchi 选择，不是已证明的 AC29 规则。
- 画布拖动跨过浮动控件时仍归画布拥有，拖动中禁止字段夺焦；释放在控件上不能提交。排除各控件实际矩形，不用矩形并集吞掉中间可用画布。数值字段本帧解析后再处理 Enter，避免提交上一帧预览。
- 多选共节点编辑按精确有限几何收集参与者，屏幕命中容差仅用于选点。预览使用私有 candidate，一次事务同步全部节点；穿过墙的分段、opening重宿主或拒绝必须原子处理。平面和3D应测真实事件路径。
- 多端汇交不是逐对最大/最小延伸的选择题：独立极值会留尖片或中心洞。相邻sector须同时负责两侧收口和中心覆盖；短墙要为两端预算，bevel共享点也须受双方预算约束。仅修一端可能让旧厚度容差把自由端误接回节点。
- 四角 footprint 扩为任意多边形时一起审计平面填充、选择、Trace、私有预览、描边裁剪和选中/未选中3D。凹形用三角剖分；只画真实外周，不描内部三角边。凹多边形质心可能在外部，共线边内侧按边界绕向判断。合并3D按实际bottom/top（含偏移）分组。
- 几何域测试通过不能证明新消费者走了它：旧quad API的回归还会绿。增加新polygon API兼容断言、renderer顶点/面积检查与UI真实入口；更新Path→Mesh表示相关断言时保留其可见填充、基线和手柄要求。
- 浮动tracker靠近鼠标但须可停住编辑，避免与图标面板重叠；没有必要的操作说明、标题和确认按钮不加入用户已指定的紧凑流程。
- 锁屏时先记录当前系统锁定状态和前台进程。窗口出现、AX仅菜单、启动空白不等于GPU/交互通过；前台守卫拒绝输入后不继续盲点，不碰登录认证。自动回归和包新鲜度可独立完成，桌面/双语验证记unknown，待解锁再续。

精确原型结论及限制见[操作研究](operation-research.md)。带洞、Marquee、selection-only等专用路径没有接入新几何时必须逐项记录，不因普通3D已修就宣称全覆盖。
