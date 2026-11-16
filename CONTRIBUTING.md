# 构建与贡献

OpenArchi 使用 Rust、egui/eframe 与 wgpu，保持原生桌面界面和独立项目模型。欢迎范围明确、可复现的改进。

## 构建

安装 Rust stable（至少 1.95）和 Python 3.11 或更新版本。macOS 需要 Command Line Tools，Windows 需要 Visual Studio C++ 构建工具。依赖固定在 Cargo.lock；macOS 的 AccessKit 修复位于 vendor/，包含原始许可与补丁说明。

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
python3 -m unittest discover -s tests/tooling -v
python3 scripts/check-localization.py
cargo run --locked --bin openarchi
```

`python3 scripts/verify.py` 执行上述公开源码检查；不需要私人目录、内部技能、CAD 应用或真实项目。`cargo clippy --locked --all-targets` 输出额外诊断。默认测试不需要显示器；明确标记的原生 GPU 和私人输入专项不计入默认验收。

## 打包

```sh
python3 scripts/build-release.py
python3 scripts/package.py
python3 scripts/audit-package.py dist
```

跨架构构建可给前两条命令加入 `--target x86_64-apple-darwin` 或 `--target aarch64-apple-darwin`；Windows 原生构建使用 `--target x86_64-pc-windows-msvc`。编译目标、链接器和 SDK 必须已安装。跨编译成功不证明该平台运行正确。

打包会重映射构建路径，并在新临时目录中按明确清单组装。第三方许可已冻结在 assets/licenses/；更新依赖时运行 `python3 scripts/collect-licenses.py`，审核新许可和来源后提交。最终包使用 `scripts/native-smoke.py` 检查原生窗口和 GPU 启动；完整鼠标交互仍需单独验收。

合成示例构造在 src/samples.rs，场地演示可用 `cargo run --example site_study -- NEW_DIRECTORY` 生成。tests/fixtures/geometry 仅含合成几何的数值预期，不包含原始工具现场或真实项目。

## 提交改进

- 先写清用户如何触发问题以及预期结果，附脱敏的最小复现。
- UI、Agent 和外部接入复用有类型、可校验、可撤销的共享模型命令。
- 模型和几何改动覆盖保存、重开、撤销重做及适用的二维/三维结果；修改共享路径时回归已有调用者。
- 界面文本同时维护中英文；源码、图标、资源和示例保持原创或具备适当许可证。
- 不提交个人配置、恢复数据、私人模型、原始录屏或凭据。

贡献按本仓库 Apache-2.0 许可分发，不转移著作权。
