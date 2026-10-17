---
name: openarchi-ac29
description: 为 OpenArchi 学习和实现 Archicad 29 的原生建模、菜单与交互功能；复用本机版本身份、已有证据、行为契约和 macOS 验证交付工具。用于本仓库的功能复刻与继续开发，不用于通用逆向或其他项目。
---

# OpenArchi / Archicad 29

本技能属于 OpenArchi 仓库；全局安装仅链接到同一份文件。先确认当前仓库有 `Cargo.toml`（package `atelier-bim`）和 `scripts/ac29-workflow.py`。从别处显式调用时，读取本文件真实路径，仓库是 `skills/openarchi-ac29` 的上两级；不要在其他项目运行这些命令。

遵循仓库 `AGENTS.md`；此处只补充可复用的本地知识，不替代用户要求。默认参照 **Archicad 29 INT**，不把 AC30 或较老帮助的相似名称当作 AC29 行为证据。

## 从已知上下文继续

在仓库运行：

```sh
python3 scripts/ac29-workflow.py context guides
```

将 `guides` 换成具体菜单、工具或手势；省略关键词会列出已有功能记录。输出包括工作区状态、本机目标身份、匹配笔记、源码入口和最近验证记录。工具只读取指定应用、仓库资料和本地构建信息，不启动 Archicad，不操作项目，不连接 REA，不访问网络。

- 先读匹配的功能笔记，再补一个具体未知点。源码行号会变化，优先按记录的符号搜索；旧文件哈希只属于当时的证据。
- 本机目标 SHA-256 按文件身份缓存；目标变更会重新计算并提示历史证据不可直接等同。要完整重新核对用 `context --refresh-target`。路径找不到时指定 `--archicad-app`，不要遍历无关应用。
- 历史 PID、window ID、AX 控件编号不是可复用地址。重新确认当前窗口再操作；保留并复用的是版本、证据文件、交互前提和未知点。
- 新功能可以用 `python3 scripts/ac29-workflow.py feature <slug> --title '<标题>'` 生成不覆盖既有文件的行为契约。已有专题继续原笔记；不要给同一功能再建一套文档。

## 调查与实施路由

仅需本项目代码时，直接检查源码，不启动 REA。确实需要 Archicad 观察时读 [本机观察参考](references/local-observation.md)，并使用已安装的 `reverse-engineer-anything` 技能及当前实际工具 schema；没有工具时如实记录边界，不自动重装 MCP。

菜单包含多少项不等于多少功能已经实现。逐项记录支持的工具、视图、默认值、单位、确认/取消、撤销和持久化。菜单可见、文档语义、模型实测与推断分开。门窗等模型不存在时标明缺口，不能放一个空开关后声称完成。几何实现和 UI 事件分别做验证；本地易错点见 [实现与验收参考](references/implementation.md)。

## 检查、交付与复用

针对新改动先跑有意义的局部测试；需要用户试用的应用源码改动完成后：

```sh
python3 scripts/ac29-workflow.py verify
```

此命令依次检查格式（含 `include!("*.rs")` 文件）、编译、完整测试、差异空白，再执行现有打包脚本，核对 UUID 和签名，写入 `target/ac29-workflow/runs/`。只在所有步骤通过且输入没有在运行中变化时记录成功。遇错即停，旧包不能成为成功交付。不会安装组件、联网修复环境、启动 GUI、提交或推送。

再次执行时，仅当输入内容、工具链/构建环境、Release 与签名后应用包均符合上次成功记录时复用；报告明确显示 `reused` 和原验证时间。`verify --force` 强制重跑；`bundle` 只读核对现有交付物，缺少与当前源码对应的成功记录时返回 `unknown`，不会把 UUID 相等当作源码新鲜。

本工具的状态目录不是永久研究证据：`cargo clean` 可清除它。将最终检查、测试数、包路径/UUID、实际桌面步骤和剩余缺口写回专题与 `docs/validation.md`。工具永远把桌面验证标为 `unverified`；桌面证据由当前 CUA/REA 返回结果另行记录。历史 `cgWindowNotFound` 不是永久禁用 GUI 的理由，但不要无变化地重试相同失败调用。
