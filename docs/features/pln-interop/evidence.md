# PLN 互操作方案：证据与范围

记录时间：2026-10-20。结论与方案见 [plan.md](plan.md)。这里只记录实际检查，不把拟议 API 链路写成运行成功。

## 源码与当前进展

基线 `13101c5caf665afa7a2ae44f8ac5545bb09c7dee`；开始时 `git status --short` 无输出。

| 类型 | 源与锚点（本次行号） | 支持的结论 |
|---|---|---|
| readable source | `Cargo.toml:19`；`src/app/mod.rs`；`docs/architecture.md` | Rust + egui/eframe + wgpu 原生结构 |
| readable source | `src/model.rs:7 Point`、`:29 Material`、`:109 Shape`、`:153 Element`、`:244 Project`、`:440 validate` | f32 坐标；有限 Shape/材质；没有外部 GUID/图库实例/源基线；项目校验 |
| readable source | `src/model.rs:703 ModelAdapter`；`src/storage.rs:3 JsonAdapter`；`src/app/file_io.rs:4 browse_project_path`、`:44 save_project` | 当前仅 JSON 读写，适配器只是同步字节契约；保存后 mark_saved |
| readable source | `src/command.rs:31 ProjectHeader`、`:286 ModelCommand`；`src/editor.rs:156 commit`、`:171 apply`、`:1011 undo`、`:1023 redo` | typed command 与 ChangeSet 混用；应以基线 diff 生成写回 |
| readable source | `src/app/inspector.rs:598 library`、`:704` | 图库是内置墙板预设，无外部图库提供器 |
| readable source | `src/render.rs:536 SceneRenderer::render`、`:643`、`:660`、`:766`、`:800` | 按类型生成几何；需新增代理与对象表示 |
| 历史验证记录 | `docs/validation.md` 2026-10-18 最新墙体节、0.2阶段摘要；`docs/performance-baseline.md` | 历史258项Rust/15项工具测试、已测桌面范围、规模瓶颈；本轮未重跑 |

关键文件 SHA-256：

```text
src/model.rs   8345a7962ac14350ed41ac7033995c04bb4a4e7eeb1ef7f13c968aabafa9d5ec
src/storage.rs a4a862dbba8ded53623c6adfb36aebb653b45de832b4ced71d9edfcf9a53dd55
src/command.rs 1b3adbc38857d669985787cbc356c40a5300479ab943bbe23ebae33df56cca64
src/editor.rs  f57aad0f57115e62c39c9fb86a6dc2480e0c34e52ec043bd5ebda8f57a9c94f5
```

## 安装目标与 REA

`python3 scripts/ac29-workflow.py context pln` 返回：

- `/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`
- `com.graphisoft.archicad29`；`29.0.0 v1 INT (5101)` / `29.0.0.5101`；arm64。
- 主程序 SHA-256：`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`，与已有记录相同，命令复用了文件身份哈希缓存。
- context 关键词无匹配不表示全仓库不存在相关材料；随后检查了源码、架构、路线图、研究和验证记录。

| 操作 | 实际结果 | 证据边界 |
|---|---|---|
| `binary_session {}` | 无活动 target；Hopper provider 可用，Ghidra 未配置，IDA 未配置 | 仅状态，不证明反编译可工作 |
| `get_evidence_bundle`，按主程序 hash 筛选摘要 | 初始0条记录 | 仅本次服务器保留状态，不否定仓库历史观察 |
| `open_binary`，指定主程序与 Hopper | 成功返回 Mach-O/arm64/hash | 绑定目标，不是执行 PLN 读写 |
| `inspect_macho` | 返回 transport `resource_constraint`，服务器声明分析完成、完整结果保留 | 未取得完整内容，不从未读结果推导符号/依赖 |
| `get_evidence_bundle`，筛选 `inspect_macho` | 1条 `complete-record`；provider `native-macos`；authority `shipped-artifact`；2项限制未在摘要展开 | 记录 ID `ev_eb64fa34faae01bfecba50bdee98ba9658d44c6e1623dd8b880c377a1897b059`。ID/摘要可追溯，不能替代结果检查 |
| `export_evidence_bundle` 到 `/tmp/openarchi-pln-rea-20261020.json` | 客户端拒绝：`MCP tool call requires approval, but approval policy is never` | 没有成功导出，不声称存在可交付完整证据文件 |
| `search_procedures` literal `PLN`、忽略大小写 | Hopper bridge readiness 超时；错误含 `cleanup_incomplete`、启动等待120000ms | 无符号查询结果，未证明存在或不存在 PLN 命名函数 |
| `inspect_plist` 指定 AC29 Info.plist | 同一客户端审批策略拒绝 | 不产生 plist Evidence |
| `close_binary {}` | `cleanup_incomplete`，未观察到 authenticated shutdown acknowledgement | 资源 `hopper-document`、`/tmp/rea-iBpaeU` 清理未确认；没有关闭其他用户会话 |

已读取相关 REA/native/recovery 技能说明；未安装/修改 provider、未重复相同失败查询、未绕过审批导出。当前 `inspect_analysis_view` schema 只列 binary-layout、JavaScript、native analyze_function 等视图，未列 `inspect_macho`，故没有声称能分页读取该记录。

安装目录普通只读文件清单（不是 REA Evidence）发现：

- `Archicad 29.app/Contents/MacOS/LP_XMLConverter.app/Contents/MacOS/LP_XMLConverter`。
- `BuiltInLibraryParts.libpack`、`Archicad Library Packages/` 下的 `Essential Library.libpack`、`Object Macros.libpack`、`Doors and Windows.libpack`、`Chairs.libpack` 等。
- 第一次限定在 Archicad 安装目录的有界文件搜索未返回 PLN 示例或 SDK 头文件；不据此推断整台机器不存在 PLN/SDK。2026-10-20 后续仓库范围搜索发现两份 PLN 样本（见下节）。

没有运行 Archicad 工程操作、图库转换器或 GDL；没有 PLN 样本级解码证据。

## 官方资料

agent-reach 的 Exa 调用返回 `Unknown MCP server 'exa'`；改用可用网页工具，并使用 agent-reach 的 GitHub CLI 路线核对 Graphisoft 官方仓库 tag/release。仅使用官方文档支持技术结论。搜索入口的部分滚动文档已标为 AC30，因此进一步锁定 **29.3100** tag；其 API 文档不能直接当成本机 5101 运行证据。

| 编号 | 官方源 | 本次核对内容与限制 |
|---|---|---|
| S1 | [AC29 Project Operations](https://raw.githubusercontent.com/GRAPHISOFT/archicad-api-devkit/29.3100/docs/group___project_operation.html) | `ACAPI_ProjectOperation_Open/Save`、`APIFType_PlanFile`；Open 会影响当前工程，不能在 undoable 操作内调用；接口存在不代表本机已调用 |
| S2 | [AC29 Element](https://raw.githubusercontent.com/GRAPHISOFT/archicad-api-devkit/29.3100/docs/group___element.html) | `ACAPI_Element_GetMemo`、`ACAPI_Element_Change` 及按类型约束；只支持白名单字段的设计基础 |
| S3 | [AC29 Library Part](https://raw.githubusercontent.com/GRAPHISOFT/archicad-api-devkit/29.3100/docs/group___library_part.html) | `ACAPI_LibraryPart_GetParams` 是默认参数；`GetSection` 等库部件访问。实际实例参数、联动与求值仍需验证 |
| S4 | [AC29 Command Connection](https://raw.githubusercontent.com/GRAPHISOFT/archicad-api-devkit/29.3100/docs/group___command_connection.html) | JSON请求、HTTP端口和 Add-On handler 关联；运行时握手尚未知 |
| S5 | [AC29 Model Access](https://raw.githubusercontent.com/GRAPHISOFT/archicad-api-devkit/29.3100/docs/group___model_access.html) | `GetComponent`、`Get3DInfo`、`GetNum`、`DecomposePgon` 等模型访问入口；不证明所有2D符号和纹理均可提取 |
| S6 | [LP_XMLConverter 官方说明](https://gdl.graphisoft.com/tips-and-tricks/how-to-use-the-lp_xmlconverter-tool/) | GSM/LCF 与 XML/HSF 转换、Mac工具位置；本机转换未执行 |
| S7 | [图库格式转换](https://gdl.graphisoft.com/tips-and-tricks/conversion-between-library-formats/) | libpack 解包到 LCF 及附属文件、宏/子类型等依赖；仅作为格式路线 |
| S8 | [AC29 GDL 新特性](https://gdl.graphisoft.com/new-features-guide/new-gdl-features-in-archicad-29/) | 库版本适配以代码与参数功能为条件；不能以文件后缀保证跨版本运行 |
| S9 | [AC29 Registration](https://raw.githubusercontent.com/GRAPHISOFT/archicad-api-devkit/29.3100/docs/_registration.html)、[MDID说明](https://archicadapi.graphisoft.com/how-to-get-mdid-for-my-add-on) | 开发者注册与正式 Add-On 加载前提；用户账户状态未查询 |
| S10 | [SDK 29.3100 release](https://github.com/GRAPHISOFT/archicad-api-devkit/releases/tag/29.3100) | 官方发布 v29.0.1/build3100，提供Mac/Windows SDK；本轮未下载安装 |

## 结论清单

| 问题 | 状态 | 当前结论 |
|---|---|---|
| 现有架构能否接入 | answered | 原生架构与命令层可复用；需增强文档、模型及资源边界 |
| 官方桥是否有技术基础 | partially answered | AC29 API支持关键操作类别；本机完整链路尚未运行 |
| 图库是否只是读取几何文件 | answered | 应区分元数据、实例显示与参数求值；转换器不能代替运行时 |
| 本机已读懂 PLN 字节格式吗 | unresolved | 没有样本解码，不能作此宣称 |
| 能否任意工程无损往返 | unresolved | 必须建立类型/字段/文档内容范围，源副本也需要保存重开验证 |
| 能否不启动 AC 而完整编辑 PLN/GDL | unresolved | 独立研究路线，不是首期承诺 |
| 本轮是否交付可运行功能 | answered | 没有；仅交付待核准方案 |

架构选择、模块名、协议字段、缓存规则、阶段与验收容差均为本项目的 **设计建议/inference**，不是从 Archicad 内部结构恢复的代码或规格。

## 2026-10-20：按当前 REA MCP 链路重新调查 PLN

### 输入对象

重新运行 `python3 scripts/ac29-workflow.py context pln`，命中既有证据笔记并复用目标 hash。目标仍为 Archicad 29 INT 5101 / build 5101 / arm64；主程序 SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。

本次只对样本做路径、大小、文件识别和 SHA-256 检查，没有把文件内容交给 shell 解析器，也没有在 Archicad 中打开/保存。

| 样本 | 本机路径 | 大小 | SHA-256 |
|---|---|---:|---|
| A | `/Users/ren/MAC工作/工作/code/开源项目/su2bim/tests/test1.pln` | 26,119,696 bytes | `73ec87bd2fc839d2a6b8502d3b28fc22720eb445b624ee432ce721f33a7dd57b` |
| B | `/Users/ren/MAC工作/工作/code/开源项目/_legacy_backup/obr存档/gdl 案例/bak/chair-gdl.pln` | 18,212,624 bytes | `ab2b9d1b4f02dd692ed9a03fc497709f8a4d06a677ea20c4fbd2de4e9ed29ef1` |

普通 `file` 命令均仅识别为 `data`，不据此推断容器、版本或损坏状态。这两份是本机发现的候选样本，本轮尚未打开验证其 Archicad 可读性或用户授权来源。

### REA 当前配置与实际操作

| 操作 | 结果 | Evidence / 限制 |
|---|---|---|
| `binary_session {}` | 无活动目标；Ghidra 12.1.4 可用，Hopper 不可用（launcher 缺失），IDA 未配置 | 仅记录此时工具状态 |
| `open_binary(test1.pln, provider_id=ghidra)` | 明确返回 `target_unavailable: unsupported binary format` | 直接证明当前 REA `open_binary` 路由不能以该 PLN 为 Ghidra 目标；不是 PLN 格式结构证据，无 Evidence ID |
| `open_binary(Archicad executable, provider_id=ghidra)` | 成功绑定 AC29 Mach-O/arm64，目标 SHA 与已知身份匹配 | 绑定成功不等于分析完成；未返回分析 Evidence ID |
| `binary_overview {}` | 等待约 2.5 分钟仍未返回；调用被停止 | 无结果、无 Evidence ID；不将“没有返回”解释为程序无符号或无字符串 |
| strings：`LP_XMLConverter`、`APIFType_PlanFile`；procedures regex：`ProjectOperation_Open|PlanFile` | 三项聚焦查询并行发起，约 2 分钟未返回；调用被停止 | 无任何结果或 Evidence ID；未完成这条静态调用链 |
| `close_binary {}` | 等待约 2 分钟未返回；停止等待后重新查询显示 Ghidra 目标仍 open、无活动请求 | 当时关闭未确认；后续状态见最终状态行 |
| 最终 `binary_session {}`（CLI 搜索结束后） | `open=false`、无 provider binding、无活动请求 | 本次 MCP Ghidra 会话最终关闭已确认；上面 close 调用当时没有及时返回，但最终状态查询确认清理完成 |

REA 工具表的 `open_binary` 支持列表包括 Mach-O/ELF/PE、DOS、ZIP/IPA/APK、ASAR、DMG、PKG、plist、JS 与分析数据库，没有 PLN/general-data 目标类型。样本 A 的直接调用进一步证实 Ghidra 当前不接受 PLN。此结论只限定于**当前 REA 接入的 open_binary 路由和所选 Ghidra provider**；不表示所有可能的二进制工具都绝无能力读取它。

本次没有用人工偏移、脚本扫字节或自行猜测结构去补足 REA 缺失。此前 Hopper 查询失败仍是另外一条 provider 故障；新的会话显示本机 Hopper launcher 缺失，不能把旧等待结果当成 PLN 证据。

### 更新后的判断与下一步

- “PLN 解析”仍未完成。当前 REA MCP 可对 Archicad 可执行文件建立 Ghidra 目标，但在本机此目标上的概览和聚焦查询本轮均未返回。
- 当前 REA 不接受 PLN 样本为 Ghidra 的 `open_binary` 目标。故现成 MCP 链路目前不能直接解析这两份 PLN。
- 可继续的安全研究路线：先从 AC29 官方 API 的可读源与自有 Add-On 语义读数建立对象模型，再对授权的原创测试工程做受控单变量保存；要求 REA 侧提供/确认原始二进制或文件布局分析能力后，才开展字节级 PLN 研究。不要声称已经得到文件格式。
- 目前没有重新授权或实施 Add-On、修改样本、转换图库、解包商业库或写入 PLN。

结论清单更新：工具链对 PLN 直接读入的问题为 **answered within current route**（拒绝 unsupported format）；PLN 结构和语义仍为 **unresolved**。此前“没有样本”已由候选样本发现纠正；样本能否打开以及版本仍 unknown。

### REA CLI 补充验证（2026-10-20）

发现 Obsidian 中已有 Ghidra 无 Hopper 工作流记录：REA 6.4.0 CLI 的成功原生分析曾在该工作流中被验证，且 Codex MCP env 与普通 shell env 分开。按该记录继续检查了本机 CLI：

| 操作 | 结果 | 证据边界 |
|---|---|---|
| rea --version / rea --help | CLI `/Users/ren/.hermes/node/bin/rea`，版本 6.4.0；存在 inspect-artifact、inspect-binary-layout、inspect、search 等命令 | 命令清单；不等于每个命令都支持 PLN |
| rea inspect-binary-layout --schema | 输入描述明确限定为 Linux ELF binary 或 relocatable object | 不适用于本机 macOS Mach-O 或 PLN |
| rea inspect-artifact test1.pln | 返回 target_unavailable / unsupported binary format | CLI 路由同样不接受此 PLN；没有格式布局结果 |
| rea search AC29 --provider ghidra（未设 env） | provider_unavailable：GHIDRA_INSTALL_DIR 未设置 | 证明 MCP 专属环境未自动传入 shell CLI |
| rea search AC29 --provider ghidra（显式传入笔记记录的 GHIDRA_INSTALL_DIR、JAVA_HOME、REA_ANALYSIS_PROVIDER） | 运行约 210 秒后 Ctrl+C；REA 返回 cancelled，cleanup=complete | 已完整清理本次 CLI 分析；无搜索结果或 Evidence |

首次对 AC29 的 MCP 静态查询仍无返回；CLI 有环境变量缺失后已按已有 Ghidra 配置补齐，但完整静态搜索仍未在有限等待内完成。没有重复启动 AC29 搜索。

REA `inspect-artifact` CLI 与 MCP 的两次 PLN 目标调用均拒绝相同样本；`inspect-binary-layout` 的实际支持范围只有 ELF。本机 Ghidra 成功分析其他 Mach-O 的既有 Evidence 来自另一工作流的独立目标，不能转用为 Archicad 或 PLN 证据。当前结论因此更明确：**当前可用 REA 路径可解析受支持的 native executable，但没有一个已验证的 REA operation 能把这份 PLN 当作结构化/反汇编目标；PLN 字节结构本身仍未研究成功。**

## 2026-10-20 REA Ghidra unsupported format 分层诊断

对前文 `test1.pln` 错误按文件身份、REA 6.4.0 源码、provider 环境与 Ghidra 12.1.4 headless 复核。结论为 REA `parseBinaryTarget`/`parseExecutableHeader` 在 provider 选择前拒绝 PLN（外层 `target_unavailable`，内部原因 `unsupported binary format`）；Ghidra 默认 importer 也对 PLN 报 `No load spec found`，但 `BinaryLoader` + 人工 AArch64 设定能导入 raw bytes，这不是 PLN 语义解析。对照 Archicad 29 arm64 Mach-O 经 REA Ghidra 和 Ghidra headless Mach-O loader 均成功。Ghidra/JDK/decompiler 配置健康；CLI 裸 shell 缺少 env，但 MCP 已配置。完整命令、hash、证据及 App/framework/dylib/dyld-cache/core/container 边界见 [REA Ghidra unsupported format 诊断](rea-ghidra-unsupported-format.md)。没有改配置或输入文件，没有调用 Hopper。

### REA 6.7.0 升级后复测（2026-10-20）

用户授权后，更新当前 npm global prefix 中 REA CLI 至 6.7.0。Ghidra 12.1.4/JDK 26.0.1/native arm64 decompiler doctor 检查全部通过。默认 PLN 路线仍报 `target_unavailable` / `unsupported_target` / `file_format_rejected`。新版显式 raw-binary 路线可成功：`rea read-bytes test1.pln 0x0 --length 16 --target-format raw-binary --architecture arm64 --entry-point 0x0 --provider ghidra`，Evidence `ev_68805c7c6103f77640c35a7f58e85f89b84660f3b0dd2e808d8f8802555aafb8`，provider Ghidra 12.1.4，读取 `524f4620464442201000020000000101`。architecture/entry 均为本次人工参数；结果只证明 Raw BinaryLoader 可读字节，不是 PLN parser。复测后样本 SHA-256 未变。当前 MCP 仍有 v6.4.0 进程，本次使用 v6.7.0 CLI。详见 [诊断报告](rea-ghidra-unsupported-format.md)。
