# OpenArchi · AC29 标准快捷键总表

日期：2026-10-28。状态：标准配置提取完成；不是应用实现完成，也不是用户当前自定义方案的导出。

- 目标：Archicad 29.0.0 v1 INT (5101)，macOS。
- 证据类型：readable shipped configuration。源文件：`/Applications/GRAPHISOFT/Archicad 29/Defaults/Archicad/WorkEnvironment/Shortcuts/Standard Shortcuts.xml`。
- SHA-256：`367d16bf8773a80c6128e26b37c47ee88bdaa3777dc52fdd9292091acd831a2b`。
- 完整收录该 XML 的 **171 条绑定**：Global 115、Non-Text 47、Text 9。保留别名、上下文、特殊键和命令 ID，不跨上下文去重。
- 符号：⌘ Command、⌥ Option/Alt、⇧ Shift、⌃ 物理 Control。**不能把 Mac 的 Command 与 Control 混成同一个 Ctrl。**
- 本表覆盖的是这份标准配置的全部记录；不声称覆盖未写入 XML 的固定按键、GDL 编辑器键、鼠标手势或用户自定义方案。
- 某个快捷键已列入本表，不代表 OpenArchi 已有对应功能；未命名条目不猜名字。
- [编辑交互契约](pointer-interaction.md) · [机器可读数据](standard-shortcuts-full-29.json)。
- 同步仓库：`docs/features/edit-menu/standard-shortcuts-full-29.json`，2026-10-28。

实际启用的快捷键由 Work Environment 决定。以后核对用户方案，使用 Options > Work Environment > Keyboard Shortcuts > Show Shortcut List in Browser；本轮没有修改或导出当前用户方案。[AC29 官方说明](https://help.graphisoft.com/AC/29/INT/_AC29_Help/020_Configuration/020_Configuration-25.htm)

## Global（115 条）

| 按键 | 命令 | 命令 ID |
|---|---|---|
| ⌘N | New... | `513-0x00000000-0x00000000` |
| ⌘O | Open... | `515-0x00000000-0x00000000` |
| ⌘⌥⇧O | Place External Drawing from Teamwork Project … | `534-0x00000000-0x00000000` |
| ⌘⌥W | Close Project | `537-0x00000000-0x00000000` |
| ⌘W | Close Tab Project | `2582-0x00000000-0x00000000` |
| ⌘S | Save | `518-0x00000000-0x00000000` |
| ⌘⇧S | Save as... | `519-0x00000000-0x00000000` |
| ⌘⌥P | Page Setup... | `526-0x00000000-0x00000000` |
| ⌘P | Print... | `527-0x00000000-0x00000000` |
| ⌘Q | Quit | `528-0x00000000-0x00000000` |
| ⌘D | Drag | `779-0x00000000-0x00000000` |
| ⌘E | Rotate | `780-0x00000000-0x00000000` |
| ⌘9 | Elevate... | `785-0x00000000-0x00000000` |
| ⌘K | Resize... | `789-0x00000000-0x00000000` |
| ⌘- | Adjust | `791-0x00000000-0x00000000` |
| ⌘0 | Crop to Single-plane Roof... | `792-0x00000000-0x00000000` |
| ⌘⌥G | Ungroup | `1282-0x00000000-0x00000000` |
| ⌥G | Suspend Groups | `1283-0x00000000-0x00000000` |
| ⌘7 | Story Settings... | `6401-0x00000000-0x00000000` |
| ⌘⌥A | Filter and Cut Elements in 3D... | `1794-0x00000000-0x00000000` |
| ⌘⌥Y | 未提供名称（保留命令 ID，待核对） | `1795-0x00000000-0x00000000` |
| C | Switch Construction Methods | `16898-0x00000000-0x00000000` |
| T | Top Elevation | `16899-0x00000000-0x00000000` |
| ⇧T | Top Elevation | `16899-0x00000000-0x00000000` |
| B | Bottom Elevation | `16900-0x00000000-0x00000000` |
| ⇧B | Bottom Elevation | `16900-0x00000000-0x00000000` |
| X | X Coordinate | `16902-0x00000000-0x00000000` |
| ⇧X | X Coordinate | `16902-0x00000000-0x00000000` |
| Y | Y Coordinate | `16903-0x00000000-0x00000000` |
| ⇧Y | Y Coordinate | `16903-0x00000000-0x00000000` |
| Z | Z Coordinate | `16904-0x00000000-0x00000000` |
| ⇧Z | Z Coordinate | `16904-0x00000000-0x00000000` |
| R | Radius/Distance | `16905-0x00000000-0x00000000` |
| ⇧R | Radius/Distance | `16905-0x00000000-0x00000000` |
| A | Angle | `16906-0x00000000-0x00000000` |
| ⇧A | Angle | `16906-0x00000000-0x00000000` |
| H | Home Story | `16959-0x00000000-0x00000000` |
| ⇧H | Home Story | `16959-0x00000000-0x00000000` |
| PageUp | Previous Tool in Toolbox | `16926-0x00000000-0x00000000` |
| PageDown | Next Tool in Toolbox | `16927-0x00000000-0x00000000` |
| - | Zoom Out | `16936-0x00000000-0x00000000` |
| + | Zoom In | `16935-0x00000000-0x00000000` |
| ⇧Right | Nudge Right (Larger) | `16954-0x00000000-0x00000000` |
| ⇧Left | Nudge Left (Larger) | `16953-0x00000000-0x00000000` |
| ⇧Down | Nudge Down (Larger) | `16952-0x00000000-0x00000000` |
| ⇧Up | Nudge Up (Larger) | `16951-0x00000000-0x00000000` |
| Right | Nudge Right | `16958-0x00000000-0x00000000` |
| Left | Nudge Left | `16957-0x00000000-0x00000000` |
| Down | Nudge Down | `16956-0x00000000-0x00000000` |
| Up | Nudge Up | `16955-0x00000000-0x00000000` |
| ⌘⌥⇧R | Rebuild & Regenerate | `2563-0x00000000-0x00000000` |
| ⌘= | Explode into Current View | `1290-0x00000000-0x00000000` |
| ⌘⇧' | Zoom to Selection | `2567-0x00000000-0x00000000` |
| ⌘' | Fit in Window | `2568-0x00000000-0x00000000` |
| ⌘[ | Previous Zoom | `2575-0x00000000-0x00000000` |
| ⌘] | Next Zoom | `2576-0x00000000-0x00000000` |
| ⌘Ctrl+F | Enter Full Screen | `2571-0x00000000-0x00000000` |
| ⇧S | Grid Snap | `1539-0x00000000-0x00000000` |
| ⌥S | Change Grid Snap States | `16930-0x00000000-0x00000000` |
| ⌘⌥D | Drag a Copy | `782-0x00000000-0x00000000` |
| ⌘⌥E | Rotate a Copy | `783-0x00000000-0x00000000` |
| ⌘⌥R | Rotate a Copy | `783-0x00000000-0x00000000` |
| ⌘⌥M | Mirror a Copy | `784-0x00000000-0x00000000` |
| F2 | Floor Plan | `2818-0x00000000-0x00000000` |
| ⌘⌥F2 | Go to Story... | `6404-0x00000000-0x00000000` |
| F3 | 3D Window | `2819-0x00000000-0x00000000` |
| ⌘F3 | Axonometry | `18698-0x00000000-0x00000000` |
| ⌥F3 | Perspective | `18697-0x00000000-0x00000000` |
| ⌥⇧F6 | Hidden Line | `15108-0x00000000-0x00000000` |
| ⌘⌥F3 | 3D Projection Settings... | `1793-0x00000000-0x00000000` |
| O | Orbit | `18694-0x00000000-0x00000000` |
| ⇧V | Gravity | `18955-0x00000000-0x00000000` |
| ⌥V | Gravity | `16914-0x00000000-0x00000000` |
| P | Switch to the Next Pet Palette Item | `16939-0x00000000-0x00000000` |
| ⌥P | Switch to the Previous Pet Palette Item | `16940-0x00000000-0x00000000` |
| ⇧P | Switch to the Previous Pet Palette Item | `16940-0x00000000-0x00000000` |
| ⌥L | Polyline | `17177-0x00000000-0x00000000` |
| / | Prefer X - Y coordinates in Tracker | `18947-0x00000000-0x00000000` |
| M | Measure | `18968-0x00000000-0x00000000` |
| ⇧M | Measure | `18968-0x00000000-0x00000000` |
| ⌘Down | Go Down a Story | `6403-0x00000000-0x00000000` |
| ⌘Up | Go Up a Story | `6402-0x00000000-0x00000000` |
| ⇧D | Radius/Distance | `16905-0x00000000-0x00000000` |
| D | Radius/Distance | `16905-0x00000000-0x00000000` |
| ⌥* | Activate Previous Tracker Value | `18971-0x00000000-0x00000000` |
| G | Switch Geometry Methods | `16897-0x00000000-0x00000000` |
| ⇧G | Switch Geometry Methods Backward | `16968-0x00000000-0x00000000` |
| ⇧E | Element Snap | `1552-0x00000000-0x00000000` |
| W | Arrow Tool/Last Tool Toggle | `16929-0x00000000-0x00000000` |
| ⇧Q | Cursor Snap Variants | `16916-0x00000000-0x00000000` |
| N | Edit value in Tracker | `18945-0x00000000-0x00000000` |
| F6 | Last Section | `9986-0x00000000-0x00000000` |
| F7 | Last Layout | `10008-0x00000000-0x00000000` |
| ⇧F7 | Go to Next Layout | `20743-0x00000000-0x00000000` |
| ⌘F7 | Go to Previous Layout | `20741-0x00000000-0x00000000` |
| ⌘Right | Go to Next Layout | `20743-0x00000000-0x00000000` |
| ⌘Left | Go to Previous Layout | `20741-0x00000000-0x00000000` |
| ⇧F7 | Save View and Place on Layout | `531-0x00000000-0x00000000` |
| ⌘⌥Left | Go to First Layout | `20740-0x00000000-0x00000000` |
| ⌘⌥Right | Go to Last Layout | `20744-0x00000000-0x00000000` |
| ⌘F4 | Show All in 3D | `1805-0x00000000-0x00000000` |
| F5 | Show Selection/Marquee in 3D | `1807-0x00000000-0x00000000` |
| F4 | Show Selection/Marquee in 3D | `1807-0x00000000-0x00000000` |
| ⌥F4 | Hide Selection in 3D | `1829-0x00000000-0x00000000` |
| ⌘⌥S | Send Changes | `2327-0x00000000-0x00000000` |
| ⌘⇧O | Open/Join Teamwork Project... | `533-0x00000000-0x00000000` |
| ⌘⌥O | Open Object... | `14338-0x00000000-0x00000000` |
| Ctrl+Tab | Show Next Tab | `2838-0x00000000-0x00000000` |
| ⇧Ctrl+Tab | Show Previous Tab | `2839-0x00000000-0x00000000` |
| ⌘⇧N | Pop-up Navigator | `2845-0x00000000-0x00000000` |
| ⌘⇧A | Show/Hide Tab Overview | `2855-0x00000000-0x00000000` |
| ⇧F2 | Select on Floor Plan | `2853-0x00000000-0x00000000` |
| ⇧F4 | Select in 3D | `2854-0x00000000-0x00000000` |
| ⌘⇧H | Archicad Home | `1025-0x47246d64-0xe1076b70` |
| ⇧C | Select Connected Horizontal Routes | `1027-0x47246d64-0x3bbe5ae9` |

## Non-Text（47 条）

| 按键 | 命令 | 命令 ID |
|---|---|---|
| ⌘⌥N | New & Reset All | `514-0x00000000-0x00000000` |
| ⌥X | Lock X Coordinate | `16907-0x00000000-0x00000000` |
| ⌥⇧X | Lock X Coordinate | `16907-0x00000000-0x00000000` |
| ⌥Y | Lock Y Coordinate | `16908-0x00000000-0x00000000` |
| ⌥⇧Y | Lock Y Coordinate | `16908-0x00000000-0x00000000` |
| ⌥Z | Lock Z Coordinate | `16909-0x00000000-0x00000000` |
| ⌥⇧Z | Lock Z Coordinate | `16909-0x00000000-0x00000000` |
| ⌥R | Lock Radius/Distance | `16910-0x00000000-0x00000000` |
| ⌥⇧R | Lock Radius/Distance | `16910-0x00000000-0x00000000` |
| ⌥D | Lock Radius/Distance | `16910-0x00000000-0x00000000` |
| ⌥⇧D | Lock Radius/Distance | `16910-0x00000000-0x00000000` |
| ⌥A | Lock Angle | `16911-0x00000000-0x00000000` |
| ⌥⇧A | Lock Angle | `16911-0x00000000-0x00000000` |
| ⌘Y | 3D Cutaway | `1796-0x00000000-0x00000000` |
| ⌘X | Cut | `772-0x00000000-0x00000000` |
| ⌘C | Copy | `773-0x00000000-0x00000000` |
| ⌘V | Paste | `774-0x00000000-0x00000000` |
| ⌘A | Select All | `776-0x00000000-0x00000000` |
| ⌘B | Repeat Last Command | `771-0x00000000-0x00000000` |
| ⌘⌥Z | Redo | `770-0x00000000-0x00000000` |
| ⌘Z | Undo | `769-0x00000000-0x00000000` |
| ⌘U | Multiply... | `786-0x00000000-0x00000000` |
| ⌘M | Mirror | `781-0x00000000-0x00000000` |
| ⌘L | Layer Settings (Model Views)... | `6657-0x00000000-0x00000000` |
| ⌘T | Wall Default Settings | `797-0x00000000-0x00000000` |
| ⌘⌥T | Edit Selection Set... | `796-0x00000000-0x00000000` |
| ⌘F | Find & Select... | `777-0x00000000-0x00000000` |
| ⌘G | Group | `1281-0x00000000-0x00000000` |
| * | Activate Next Tracker Value | `18970-0x00000000-0x00000000` |
| ⌥§ | Lock to Guide Line / Snap Guide | `7947-0x00000000-0x00000000` |
| ⌥C | Pick Up Parameters | `799-0x00000000-0x00000000` |
| ⌘⌥C | Inject Parameters | `800-0x00000000-0x00000000` |
| ⌥F2 | Trace | `21761-0x00000000-0x00000000` |
| ⌘, | Rotate Orientation | `2580-0x00000000-0x00000000` |
| E | Next Plane | `1821-0x00000000-0x00000000` |
| ⌥E | Horizontal | `1809-0x00000000-0x00000000` |
| L | Guide Lines | `7937-0x00000000-0x00000000` |
| ⇧L | Create Guide Line Segment | `7942-0x00000000-0x00000000` |
| Q | Force Snap Reference | `16938-0x00000000-0x00000000` |
| F | Change Flip Status | `16967-0x00000000-0x00000000` |
| ⌘⌥I | IFC Project Manager... | `2130706433-0x47732f64-0x08427fea` |
| Home | Open Favorite Popup for Tool in Toolbox | `16973-0x00000000-0x00000000` |
| ⌘⌥⇧S | Send & Receive | `2329-0x00000000-0x00000000` |
| ⌘J | Reserve Elements... | `2334-0x00000000-0x00000000` |
| ⌘⇧J | Release All | `2335-0x00000000-0x00000000` |
| ⌘⇧R | Quick Rotate Clockwise | `2129985537-0x47732f52-0x1d169483` |
| ⌘R | Quick Rotate Counter-Clockwise | `2129985538-0x47732f52-0x1d169483` |

## Text（9 条）

| 按键 | 命令 | 命令 ID |
|---|---|---|
| ⌘Z | Undo | `1025-0x00000000-0x00000000` |
| ⌘⌥Z | Redo | `1026-0x00000000-0x00000000` |
| ⌘B | Repeat Last Command | `1027-0x00000000-0x00000000` |
| ⌘X | Cut | `1028-0x00000000-0x00000000` |
| ⌘C | Copy | `1029-0x00000000-0x00000000` |
| ⌘V | Paste | `1030-0x00000000-0x00000000` |
| ⌘A | Select All | `1032-0x00000000-0x00000000` |
| ⌘T | Replace Again | `1037-0x00000000-0x00000000` |
| ⌘F | Find... | `1047-0x00000000-0x00000000` |

## XML 之外的重要上下文按键

以下来自公开帮助，不混入上面的 171 条配置记录：

- 编辑中复制模式：Mac Option，Windows Ctrl；多重复制的官方 Mac 组合为 Cmd+Option，Windows Ctrl+Alt。具体锁存/释放行为见交互契约的待实测项。
- Tracker：N 临时显示；数字输入默认参数；Tab 切换字段；X/Y/Z、D/R、A 定位参数；Enter 应用并退出；Esc 取消数值输入；`/` 切换直角/极坐标显示。工作环境设置影响直接数字输入和坐标键的解释。[Tracker](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-40.htm)
- Esc 分层退出数值输入、导航、约束/参考、当前输入、选择、选区框，最终回到箭头工具；Backspace 在输入中取消，存在选中构件时可能删除。不要把所有状态下的 Esc/Backspace 写成同一动作。[取消操作](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-68.htm)
- GDL 固定快捷键应另从应用生成的快捷键网页底部记录；官方说明这些快捷键不可编辑，本轮未取得该固定列表，不能宣称已完整收录。[GDL Editor Shortcuts](https://help.graphisoft.com/AC/29/INT/_AC29_Help/140_UserInterfaceDialogBoxes/140_UserInterfaceDialogBoxes-66.htm)
