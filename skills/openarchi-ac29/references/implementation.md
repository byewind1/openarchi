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
