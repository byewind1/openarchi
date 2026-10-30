# REA Ghidra “unsupported binary format” 诊断（2026-10-20）

## 结论

本次报错目标按前文上下文认定为：`/Users/ren/MAC工作/工作/code/开源项目/su2bim/tests/test1.pln`。其文件角色是一个候选 Archicad PLN 项目文档（这是由项目上下文和文件名推断的用途）；本机 `file` 只识别出通用 `data`，没有识别出标准可执行文件/容器格式。目标的 CPU 架构因此未知/不适用，不能从 `.pln` 扩展名推断。

准确故障链：REA 6.4.0 的 `open_binary` 在选择 provider 前先执行 `parseBinaryTarget`。它只识别本链路实现的 ELF、MZ/PE、thin/fat Mach-O 等头；PLN 首字节不匹配，`parseExecutableHeader` 返回 `unsupported binary format`，REA 外层包装为 `target_unavailable`。所以这是 **REA 的目标格式解析/准入限制，在交给 Ghidra 前主动拒绝**，不是 Ghidra import failure、架构准入拒绝、目标角色拒绝、managed-target 拒绝，也不是 decompiler/JDK 启动失败。

Ghidra 12.1.4 默认 Headless Loader 对同一 PLN 也报告 `No load spec found`、`Import failed`。但使用 `BinaryLoader` 强制原始映像、并人工指定 `AARCH64:LE:64:v8A` 后，headless importer 能把 26 MB 文件导入项目。这仅证明 Ghidra 能把任意字节当作 raw blob 装入地址空间；这个架构是本次人为指定的，不能证明 PLN 是 AArch64、没有解析出 PLN 结构，也不能支持有意义的反汇编/PLN 语义。

对照 Archicad 29 主程序，REA MCP 的 Ghidra provider 成功打开；Ghidra Headless 用 `Mac OS X Mach-O` loader 和 `AARCH64:LE:64:AppleSilicon` 成功导入。Ghidra 12.1.4、JDK 26.0.1 arm64 与 macOS arm64 原生 decompiler 均通过 `rea doctor` 检查。因此目标文件格式才是本次 PLN 错误的原因。

## 目标身份与文件角色

### PLN 候选目标

- 路径：`/Users/ren/MAC工作/工作/code/开源项目/su2bim/tests/test1.pln`
- `file`：`data`
- `stat -f '%N %z bytes'`：26,119,696 bytes
- SHA-256：`73ec87bd2fc839d2a6b8502d3b28fc22720eb445b624ee432ce721f33a7dd57b`
- `lipo -info`：`can't figure out the architecture type`（因其不是 Mach-O）
- `otool -hv`：`is not an object file`
- 角色：按路径/扩展名与 OpenArchi 研究上下文，视为 Archicad 项目文件候选；未通过 Archicad 打开来验证有效性/版本。文件本身不提供可识别 CPU 架构。

### Archicad 对照目标

- App：`/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`
- 实际主程序：`/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app/Contents/MacOS/Archicad`
- `file`：Mach-O 64-bit executable arm64
- `stat`：177,952,352 bytes
- `lipo -info`：非 universal，architecture 为 arm64
- `otool -hv`：`MH_MAGIC_64`，`CPU_TYPE ARM64`，`filetype EXECUTE`
- SHA-256：`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`
- REA `inspect-macho` 通过 Apple 原生 Mach-O 检查产生 Evidence `ev_eb64fa34faae01bfecba50bdee98ba9658d44c6e1623dd8b880c377a1897b059`；结果同样给出 `format=mach-o`、`architecture=arm64`、Mach-O filetype `2`（MH_EXECUTE）。此操作不启动 Hopper。

## 配置与 provider 状态

- 本机 CLI：`rea --version` → `6.4.0`。
- REA shell 的 `GHIDRA_INSTALL_DIR` 与 `JAVA_HOME` 默认为空；因此裸跑 `rea doctor --provider ghidra --json` 会报告 Ghidra `missing_analysis_engine`。这是普通 shell CLI 的环境传递问题。
- REA MCP `binary_session` 本轮显示 Ghidra candidate 可用，安装目录 `/Users/ren/Applications/ghidra_12.1.4_PUBLIC`，headless `/Users/ren/Applications/ghidra_12.1.4_PUBLIC/support/analyzeHeadless`，JDK `/opt/homebrew/Cellar/openjdk/26.0.1/libexec/openjdk.jdk/Contents/Home`，平台/架构 `darwin/arm64`。MCP `open_binary(Archicad executable, provider_id=ghidra)` 成功绑定；随后已正常关闭本次会话。
- 用上述路径给 CLI 单命令传环境后，`rea doctor --provider ghidra --json` → `healthy=true`、Ghidra 12.1.4、JDK 26.0.1、64-bit full JDK（满足 21+）、native decompiler 报告 Mach-O arm64 且与宿主一致。
- Ghidra decompiler 文件：`/Users/ren/Applications/ghidra_12.1.4_PUBLIC/Ghidra/Features/Decompiler/build/os/mac_arm_64/decompile`；`file` 报告 Mach-O 64-bit executable arm64。
- 结论：CLI 不带 env 时有配置缺口；MCP 环境当前配置正确。本次 PLN 拒绝发生在 target parser，故修正 CLI env 不会让 REA 开始解析 PLN。没有修改任何配置文件。

## 重现的实际命令与关键输出

```sh
PLN='/Users/ren/MAC工作/工作/code/开源项目/su2bim/tests/test1.pln'
AC='/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app/Contents/MacOS/Archicad'
file "$PLN"
stat -f '%N %z bytes' "$PLN"
lipo -info "$PLN"
otool -hv "$PLN"
file "$AC"
stat -f '%N %z bytes' "$AC"
lipo -info "$AC"
otool -hv "$AC"
rea --version
rea doctor --provider ghidra --json
GHIDRA_INSTALL_DIR='/Users/ren/Applications/ghidra_12.1.4_PUBLIC' \
JAVA_HOME='/opt/homebrew/Cellar/openjdk/26.0.1/libexec/openjdk.jdk/Contents/Home' \
REA_ANALYSIS_PROVIDER=ghidra rea doctor --provider ghidra --json
rea capabilities --format json
rea providers --format json
```

MCP `open_binary({path: PLN, provider_id: "ghidra"})` 实际返回：

```json
{"error":{"code":"target_unavailable","category":"unavailable","message":"Cannot open artifact: unsupported binary format. Check that the path exists, is readable, and points to a supported file.","retryable":false,"details":{"path":"…/test1.pln","reason":"unsupported binary format"}}}
```

REA 6.4.0 源码（本机 npm 安装包）中，`BinaryTargetResolver.parseBinaryTarget` 先运行 `parseExecutableHeader`，失败即把错误作为 `BinaryTargetError` 返回，尚未调用 Ghidra provider。`parseExecutableHeader` 只对 ELF/MZ/thin Mach-O/fat Mach-O 分派；其余直接 `unsupported binary format`。安装源码的精确位置：

- `dist/application/BinaryTargetResolver.js:20-35, 70-90`
- `dist/domain/binaryTarget.js:7-21`
- `dist/ghidra/GhidraProvider.js:78-124, 209-238`

Headless 对照使用新的 `/tmp` 项目目录，不改目标文件：

```sh
GH='/Users/ren/Applications/ghidra_12.1.4_PUBLIC/support/analyzeHeadless'
"$GH" /tmp/rea-headless-diagnosis.Dwrlzq PLNDefault \
  -import "$PLN" -noanalysis
```

关键行（注意该脚本进程最后退出码是 0，所以以导入报告文本而非退出码判断）：

```text
INFO No load spec found for import file: test1.pln (ProgramLoader)
ERROR The ProgramLoader could not successfully load ... test1.pln ...
ERROR REPORT: Import failed for file: ... test1.pln
```

Ghidra 通用 raw 对照：

```sh
"$GH" /tmp/rea-pln-raw-import.YmHxJl PLNRaws \
  -import "$PLN" -loader BinaryLoader \
  -processor 'AARCH64:LE:64:v8A' -noanalysis
```

关键行：

```text
INFO Using Loader: Raw Binary (ProgramLoader)
INFO Using Language/Compiler: AARCH64:LE:64:v8A:default (ProgramLoader)
INFO REPORT: Save succeeded for: /test1.pln (PLNRaws:/test1.pln)
INFO REPORT: Import succeeded (HeadlessAnalyzer)
```

这次 raw 导入的语言是人为强塞的，禁止据此称为架构识别或 PLN 解析。

Mach-O 对照：

```sh
"$GH" /tmp/rea-headless-diagnosis.Dwrlzq AC29Executable \
  -import "$AC" -noanalysis
```

关键行：

```text
INFO Using Loader: Mac OS X Mach-O (ProgramLoader)
INFO Using Language/Compiler: AARCH64:LE:64:AppleSilicon:default (ProgramLoader)
INFO /Archicad: file created
INFO REPORT: Import succeeded (HeadlessAnalyzer)
```

REA MCP 对照：`open_binary(AC, provider_id="ghidra")` 成功返回 `format=mach-o`, `kind=executable`, `architecture=arm64`，SHA-256 与上述身份一致；之后 `close_binary` 成功。

## 分类与各类文件边界

| 分类 | 对本目标的判定 |
|---|---|
| `target_format_unsupported` | 根因所属语义分类：REA target resolver 不识别 PLN。外层 MCP 实际返回通用 `target_unavailable` + `reason=unsupported binary format`，并非 provider 的 typed `target_format_unsupported` 响应。 |
| `architecture_unsupported` | 不是本次错误：PLN 未识别为可执行文件，根本没有架构输入。Ghidra v1 在 macOS/Linux 对某些 MIPS profile 另有限制；源码还明确拒绝多 slice universal Mach-O。 |
| `target_role_unsupported` / `managed_target_unsupported` | 不是本次错误：这些准入判断只在目标已解析且进入 Ghidra provider 后进行；REA v6.4 provider 的明确 role/managed 检查位于 Windows P0 PE 分支。 |
| Ghidra import 失败 | PLN 默认 Headless importer 确实失败 (`No load spec found`)，这是独立对照结果；REA 的原始报错更早发生，尚未到此层。 |
| native decompiler / Java 失败 | 排除：`doctor` 检查 native arm64 decompiler 和 full JDK 通过；Archicad Mach-O 可被 Ghidra 正常导入。 |

| 文件/容器种类 | REA 6.4/Ghidra 路由边界 |
|---|---|
| macOS `.app` | REA target resolver 特别解析 `.app`，读取 bundle executable 后交给目标解析；Ghidra 分析的是其中 executable，不是整个 bundle。Archicad 主程序成功。 |
| thin Mach-O arm64 | 当前已验证支持：Archicad 主程序经 REA Ghidra 和 headless Mach-O loader 成功导入。 |
| universal/fat Mach-O | REA parser 可读架构表，但 Ghidra v1 明确因无法控制所选 slice 而拒绝多架构输入 (`architecture_unsupported`)；先用 `lipo -thin arm64` 输出到新临时文件，再分析。 |
| framework | `.framework` 是目录且 target resolver 只对 `.app` 特别解析；对 framework 目录本身应预期拒绝。指定其内部 Mach-O executable 后，按相同 Mach-O 路线分析。 |
| `.dylib` / Mach-O shared library | 魔数/架构可被解析，Ghidra Mach-O loader 有相应 import 路线；但本次未对具体 dylib 做验证。REA 6.4 在 macOS/Linux 没有 Windows P0 那种 PE executable-role 检查；不应把静态准入等同于每种 dylib 都能完整分析。 |
| dyld shared cache | 不是本 REA Ghidra target parser 已支持的常规 ELF/Mach-O executable 输入。REA `trace_dylib_resolution` 可以报告依赖路径，但明确将系统 dylib 留作 unknown（系统映射在 dyld cache 中）；没有在本次把 cache 当 executable 导入。若强制 `BinaryLoader`，也只是 raw bytes。 |
| core dump | 没有独立 core-dump/线程寄存器路线经本次 REA Ghidra 开通；Mach-O core 的直接 REA/Ghidra 语义本轮未测试，状态 unknown。不可把它当普通 Mach-O app 结论外推。Linux ELF core 则走 REA 的独立 `inspect_recorded_crash` 路线，仅适用于该工具规定的 Linux x86-64 core。 |
| 分析数据库 | `.hop` 可由 REA 归类为 `analysis-database`，但 Ghidra v1 只接收 `executable`，因此不是 Ghidra 可直接导入的源二进制；Ghidra 自有项目数据库应由 Ghidra 项目打开。 |
| ZIP/IPA/APK/MSIX/AppX/ASAR/DMG/PKG 等容器 | REA 可用 artifact inventory/extract 路线观察容器、选取成员；它们不是 Ghidra v1 executable 输入。应将容器中的具体 Mach-O/ELF/PE 成员作为目标。 |
| 原始二进制/PLN | Ghidra `BinaryLoader` 可导入任意字节，但须人工给架构/地址。REA 6.4 只暴露 headerless DOS COM 解释，不暴露此类通用 PLN raw import；raw import 不等于文件格式解析。 |

Cutter 没有作为 REA provider 使用；本次没有调用任何 Hopper 操作或尝试绕过 Demo 限制。REA `doctor` 会打印 Hopper 的 informational status 字段，但本次目标配置只检查 Ghidra；没有启动 Hopper。

## 上游 issue / 后续版本

- 对 REA GitHub issue 标题/正文按 `PLN` 搜索，本次无匹配。这只是截至本次调查的检索结果，不能证明未来不会新增。
- 最接近的上游讨论是仍 open 的 [REA #4246：raw-binary 导入的 caller-selected Ghidra language/base address](https://github.com/morluto/rea/issues/4246)。它讨论的是已知 CPU/入口的 headerless code image，并不提 Archicad PLN，也不提供 PLN parser。
- 本机安装为 REA 6.4.0；截至 2026-10-20，上游已有 6.7.0。6.6/6.7 增加/扩展了显式 raw m68k lane，但没有 PLN 支持或对当前 `unsupported binary format` 的 PLN 修复。安装包 6.4 的 `--target-format` help 仅显示显式 DOS COM interpretation。更新到 6.7 即便得到 raw image 能力，也不能解读 PLN 项目结构。
- 上游 6.4 源码：[`BinaryTargetResolver.ts`](https://github.com/morluto/rea/blob/rea-agents-6.4.0/src/application/BinaryTargetResolver.ts)、[`binaryTarget.ts`](https://github.com/morluto/rea/blob/rea-agents-6.4.0/src/domain/binaryTarget.ts)、[`GhidraProvider.ts`](https://github.com/morluto/rea/blob/rea-agents-6.4.0/src/ghidra/GhidraProvider.ts)。Ghidra 官方 [`analyzeHeadless` 文档](https://github.com/NationalSecurityAgency/ghidra/blob/master/Ghidra/RuntimeScripts/support/analyzeHeadlessREADME.md) 明确列出 `BinaryLoader`、Mach-O loader 与 `-processor`，解释了本次“默认识别失败、raw 强制装载成功”的结果。

## 结论与最小可行替代方案

- Ghidra 本身：原生支持 Archicad 的 arm64 Mach-O 主程序，且 Ghidra 12.1.4 headless 已成功导入。对 PLN：没有自动识别格式的 loader；通用 Raw BinaryLoader 可以装字节，但它不理解 PLN。
- REA：6.4.0 Ghidra provider 没有 PLN/general-data 目标准入；REA target resolver 先拒绝。不是当前 Ghidra adapter 将有效 Mach-O 拒绝，而是它的输入边界不包含该文档格式。
- 其他 REA 静态能力：可对 Archicad `.app` 做 artifact/bundle/Info.plist/Mach-O/load-command/依赖信息盘点，并对选定 arm64 executable 使用 Ghidra 搜索函数、字符串、反编译；这些只能帮助理解应用，不会自动解析 `.pln`。
- 最小方案：继续用 REA Ghidra 研究 Archicad executable；对 PLN 采用受控的 Archicad 可读性验证/API 或 Add-On 导出对象语义，配合授权原创小工程的单变量保存对照。若临时用 Ghidra 查看 PLN 字节，只能在确认 CPU/地址后用 Raw BinaryLoader 作为有限人工探查，并明确不代表结构化解析。
- 下一步命令（当前 shell CLI 临时提供正确 provider env）：

```sh
GHIDRA_INSTALL_DIR='/Users/ren/Applications/ghidra_12.1.4_PUBLIC' \
JAVA_HOME='/opt/homebrew/Cellar/openjdk/26.0.1/libexec/openjdk.jdk/Contents/Home' \
REA_ANALYSIS_PROVIDER=ghidra rea doctor --provider ghidra --json

GHIDRA_INSTALL_DIR='/Users/ren/Applications/ghidra_12.1.4_PUBLIC' \
JAVA_HOME='/opt/homebrew/Cellar/openjdk/26.0.1/libexec/openjdk.jdk/Contents/Home' \
REA_ANALYSIS_PROVIDER=ghidra rea analyze \
  '/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app/Contents/MacOS/Archicad' \
  --provider ghidra --format json

# PLN 默认导入会失败；以下仅人工 raw-byte 检查，不具备 PLN 解析意义：
'/Users/ren/Applications/ghidra_12.1.4_PUBLIC/support/analyzeHeadless' \
  /tmp/rea-pln-raw-review PLNRawReview \
  -import '/Users/ren/MAC工作/工作/code/开源项目/su2bim/tests/test1.pln' \
  -loader BinaryLoader -processor 'AARCH64:LE:64:v8A' -noanalysis
```

没有改配置文件、没有修改 PLN、没有调用 Hopper、没有使用 Cutter。Headless Ghidra 仅在新建的 `/tmp` 项目目录写入数据库；输入文件只读。诊断结果同步自仓库 `docs/features/pln-interop/rea-ghidra-unsupported-format.md`，日期 2026-10-20。iCloud 云端同步状态未验证。

## 2026-10-20 升级到 REA 6.7.0 后复测

用户授权本机升级后，通过既有 `/Users/ren/.hermes/node` npm global prefix 执行 `npm install --global rea-agents@6.7.0 --prefix /Users/ren/.hermes/node`。安装后 `rea --version` 和包 `package.json` 均为 `6.7.0`。未改 Codex/REA 配置文件；当前 Codex MCP 后台仍有 `npm exec rea-agents@6.4.0 mcp` 进程，因此本次新版本复测用新版 CLI，MCP 工具进程不会因 CLI 包替换自动升级。

- Ghidra 环境传入后，`rea doctor --provider ghidra --json`：`healthy=true`，Ghidra 12.1.4 available；macOS/arm64、JDK 26.0.1 full 64-bit、Ghidra native decompiler mac_arm_64 全部通过。
- 新版默认检测：`rea analyze "$PLN" --provider ghidra --format json` 仍返回 `target_unavailable`, `category=unsupported_target`, `constraint=file_format_rejected`, `reason=unsupported binary format`。故新版没有添加自动 PLN 格式识别/解析。
- 新版显式 raw lane：命令 `rea read-bytes "$PLN" 0x0 --length 16 --target-format raw-binary --architecture arm64 --entry-point 0x0 --provider ghidra --format json` 成功，Evidence `ev_68805c7c6103f77640c35a7f58e85f89b84660f3b0dd2e808d8f8802555aafb8`；报告 provider Ghidra 12.1.4、format `raw-binary`、architecture `arm64`、loader `BinaryLoader`、language `AARCH64:LE:64:v8A`，并返回 16 bytes `524f4620464442201000020000000101`。这是 caller-supplied raw image interpretation；arm64 与 entry 0x0 是本次人工选择，不是从 PLN 检测出的架构或对象入口。
- 复测后 `shasum -a 256 "$PLN"` 仍为 `73ec87bd2fc839d2a6b8502d3b28fc22720eb445b624ee432ce721f33a7dd57b`；输入未变。REA raw import 使用 ephemeral Ghidra project；没有执行目标程序。

更新结论：REA 6.7.0 **不能默认识别 PLN，但提供了需要人工架构与入口的 raw-binary 检视路径**。它适用于已知代码映像的 headerless firmware，不构成 PLN 格式支持；本次 26 MB PLN 在人为 arm64 假设下产生的反汇编不可作为有意义的 PLN 语义结论。可再分析 Archicad executable，也可用 raw mode 读指定字节；若要解释这些字节，仍需独立确定 PLN 容器/版本和数据结构，或使用 Archicad 官方 API/插件提取语义。

### 当前 MCP 注册状态与本次未做的改动

`ps` 观察到 Codex MCP 仍有两个 `npm exec rea-agents@6.4.0 mcp` 进程。因此 6.7.0 复测实际走的是刚升级的 CLI，不是当前 MCP 长进程。升级后的 `rea setup --client codex --dry-run --json` 是只读预览：计划写 `/Users/ren/.codex/config.toml`（预计备份 `/Users/ren/.codex/config.toml.rea.backup.6bf135d4702756dc3edec93f5711ab5bfbcb337755e201d2617ba39899082e38.recovery`）、增加新版 REA MCP 注册并更新 bundled skill；预览检测到 `HOPPER_LAUNCHER_PATH=/Users/ren/Applications/Hopper Disassembler.app/Contents/MacOS/hopper`，且 doctor 显示 Ghidra env 未配置。基于本轮明确的“不继续使用 Hopper”边界及配置变更超出这次 CLI 升级/复测范围，没有应用预览，也没有改 Codex config/skill。之后若要让 MCP 会话切到 6.7，应先形成一个不注入/启动 Hopper、明确传入 Ghidra/JDK env、列明备份与回滚的专用配置变更。

## 2026-10-24 ZCode MCP 接入完成（按本文要求的专用配置变更）

用户要求实际搭建 REA 环境后完成。发现 `/Users/ren/.zcode/cli/config.json` 已于 2026-10-23 23:50 注册 `rea`（`/Users/ren/.hermes/node/bin/rea mcp` + Ghidra/JDK env，无 Hopper），命令本身有效；但 ZCode 沙箱会话（HOME=/tmp/…）不读取真实 HOME 的用户配置，故工具未出现。本次注册：

- 工作区 `<repo>/.zcode/config.json` 与沙箱用户 `/tmp/oa-transfer-smoke/home/.zcode/cli/config.json`：`rea`（6.7.0，`REA_ANALYSIS_PROVIDER=ghidra`、`GHIDRA_INSTALL_DIR=/Users/ren/Applications/ghidra_12.1.4_PUBLIC`、`JAVA_HOME=/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home`，timeoutMs 60000，无 HOPPER_LAUNCHER_PATH）+ `cua-driver`（0.21.0 `mcp`，桌面输入/截图通道，daemon 具 Accessibility+Screen Recording）。
- 真实用户配置 `/Users/ren/.zcode/cli/config.json` 未改动。
- 验证：`rea mcp doctor` healthy（207 tools/6 prompts 无缺失，请求流 OK）；`rea doctor --provider ghidra` healthy（node/host/ghidra 全通过，hopper False）；完整 stdio 序列（initialize→initialized→tools/list）实测 rea=207 工具、cua-driver=56 工具。
- 生效方式：MCP 在会话启动时自动连接——需重启 ZCode 会话（或整个 app）后在 Settings → MCP 确认 `rea`/`cua-driver` 为 connected。回滚：删除上述两个新增配置文件即可；无其他系统改动。
- 遗留：REA host 的 native-macos 截图仍需用户在系统设置授予屏幕录制权限（仅影响 UI 捕获，不影响 Ghidra 静态分析）；cua-driver 对 egui 自绘窗口的点击有效性待下会话实测（本轮 foreground 投递未见效果，见 validation.md 2026-10-24 节）。
