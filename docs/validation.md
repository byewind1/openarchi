# 验证记录

日期：2026-10-12。设备：Apple Silicon，macOS 15.7.5。工具链：Rust 1.98.1，aarch64-apple-darwin。同材质墙体平面交接清理更新后，所有自动测试通过：42 项（21 项工作流、21 项单元/egui 测试）。`cargo check --locked --offline --all-targets` 和 Release 构建通过。

## 自动检查

执行入口：

```sh
./scripts/cargo.sh check --locked --offline --all-targets
./scripts/cargo.sh test --locked --offline
./scripts/cargo.sh build --release --locked --offline
```

自动测试覆盖：

- 21 项工作流测试：原有墙/板建模、撤销重做、楼层、吸附、属性、存储、3D 拾取，加上 Marquee 范围/节点选择、组移动/复制/剪切/粘贴、90°旋转、镜像、相邻楼层迁移、节点拉伸回滚、3D XY/高度体积筛选。
- 9 项几何测试、1 项渲染网格测试：覆盖共线墙端线消除、T 形连接（隐藏被主墙覆盖的 3D 端封头）、L 角正交连续轮廓、20 cm 墙面落点偏差自动交接与边线相交；并断言交接轮廓没有斜线。也覆盖旋转矩形、Marquee 节点选择/几何相交、凹多边形裁切、透视投影，以及带 Z 范围的墙体裁切。
- 11 项 egui 交互/渲染测试：墙/板建模、点选与拖动、撤销重做、Tracker、Marquee 绘制、F2/F3/F5、Shift+中键旋转、⌘T 对话框，以及三种视图与三种侧栏内容渲染。
- ⌘T 交互回归测试确认快捷键打开独立的元素设置模态框，未切换侧栏或修改 Inspector 可见状态；应用的属性修改进入撤销栈。
- egui 自动测试注入 `RawInput` 鼠标/键盘事件并执行实际画布代码，但仍需与桌面实机测试区分；测试本身不覆盖 macOS 窗口事件分发和 GPU。

## 实机检查

本轮实际从 `dist/OpenArchi.app` 启动最新构建，并用桌面 UI 自动化选中示例模型中的墙、按下 ⌘T、在弹出的独立对话框将墙高从 3.000 m 改为 3.500 m，按 Apply 后确认右侧属性与面积更新，然后执行 ⌘Z 恢复为 3.000 m。首次验证发现磁盘中的应用包早于对话框实现，因此已用当前通过检查的可执行文件更新 `dist/OpenArchi.app` 并重新进行本地 ad-hoc 签名。CUA 原有应用名绑定指向旧 Bundle ID；用测试副本绑定后完成了实际窗口操作。测试副本的临时标识不影响交付包的 `com.openarchi.desktop`。

根据后续校正，墙体交接不应添加斜面。当前 2D 与普通 3D 透视共用吸附并延伸后的矩形显示轮廓，让内外墙面线以正交方式相交；交叠的墙端封头和内部边线隐藏。回归断言覆盖 20 cm 偏移的 L 角、T 形支墙封头，以及轮廓不存在斜线。项目里的墙仍是独立构件；Marquee 裁切依旧逐墙处理。

2026-10-13 使用 REA 实机观察 Archicad 29 arm64。REA `observe_native_ui` 记录了平面视图，`capture_native_ui_scenario` 通过当前窗口可访问的 3D 标签切换到透视模型；受控后的截图可见两面墙在 L 角正交相接、没有对角斜接面。证据 ID 和截图边界见 [research.md](research.md)。据此普通 3D 的常见同高度连通 L/T 墙组改为由单一暴露轮廓挤出，避免分墙实体重叠造成内部面；当前仅覆盖单闭环外轮廓，带孔或分离组保留原逐墙绘制路径。

此项代码通过 22 项单元/egui 测试、21 项工作流测试及 `cargo check --all-targets`；Release 包已重建并通过本地 `codesign --verify --deep --strict`。本次 CUA 按包路径启动仍被旧 Launch Services 记录映射到 `studio.atelier.bim.demo`，所以本次更改尚未从最终 `.app` 完成目视回归；不要把上一轮的属性对话框测试当作这次新网格的运行时验证。

首次 GPU 启动发现 wgpu 30 不允许 LineList 使用 depth bias，已修复为仅三角形偏移；保留深度测试的边线使用零偏移。该问题只能通过 GPU 运行发现，编译与领域测试不足以覆盖。

## 验证边界

未完成长期稳定性、不同 macOS/显卡、百万级构件、性能基准、签名公证或安装器测试。REA 原生 UI 已观察当前本机 Archicad 29 的窗口并完成只读 3D 视图切换；Marquee 的完整逐步交互、不同材质/高度墙交接和高倍细部仍未观察，按公开 Graphisoft AC29 帮助或 unknown 标记。3D Marquee 的 macOS 实机目视检查不在本轮 ⌘T 回归范围内。

最终 Release 包为约 14 MB 的 arm64 Mach-O 可执行文件；已通过 `codesign --verify --deep --strict` 本地签名检查。相机为 45° 视场的透视投影，缩放会按视距重新取景。

2026-10-13 根据本机 Archicad 29 工作区创建 [界面与菜单清单](archicad-interface-inventory.md)，记录主菜单、菜单分组、当前架构工作区工具箱、面板分布与菜单状态范围。后续扩展已增加标准/编辑工具栏、与 Archicad 类似的 Design 工具长列、Viewpoint/Document 工具组、四种 Navigator 地图、搜索框，以及各核心菜单的可浏览层级。墙、板、Marquee、Arrow 仍是唯一与模型后端连接的工具；其他工具和专业命令均有交互式 UI 预览与四页本地设置窗口，不修改项目模型。所有 3 种主视图、3 种侧栏面板、4 种 Navigator 地图及配置预览页都纳入 egui 渲染回归。本轮 `cargo check --locked --offline --all-targets` 与 44 项自动测试通过，`dist/OpenArchi.app` 已重建且签名校验通过。尝试经 Finder/Launch Services 以包路径启动时收到 `-10827 kLSNoExecutableErr`，尽管包内 `Contents/MacOS/openarchi` 存在；本轮界面布局没有完成最终桌面目视回归。
