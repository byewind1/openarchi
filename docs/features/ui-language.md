# Main-window language switching (English / Simplified Chinese)

## Scope

Goal: switch the OpenArchi main window between English and Simplified Chinese
from either the title-bar language selector or `View > Language`, apply the
change immediately to visible UI, and restore the same language after a restart.
The switch changes presentation only; it must not edit the project, selection or
undo history.

Evidence types:

| Claim | Type | Source / evidence | Limits |
|---|---|---|---|
| Language can be chosen from the title bar and from `View > Language` | runtime observation | Final packaged app, 2026-10-18; title-bar selector switched both ways; `视图 > 语言 > English` and the title-bar selector then restored `简体中文` | Source is authoritative for OpenArchi, not for Archicad |
| The choice is stored as `zh-CN` or `en` under `~/Library/Application Support/OpenArchi/language` | readable source | `src/localization.rs::load_preference`, `save_preference` | File location is a local implementation choice |
| Title bar, top menus, toolbar, Info Box, toolbox, view tabs, counts, status bar and Quick Options bar change together | runtime observation | Final packaged app, 2026-10-18, AX tree and full-window screenshots | Covers the main floor-plan window; every dialog was not visually re-read in both languages |
| The selected language survives an application restart | runtime observation | Final packaged app, 2026-10-18, quit/relaunch after choosing `简体中文` | macOS local preference only |
| The window title follows the language while the macOS application-menu label stays `OpenArchi` | runtime observation | `49906133` bundle, 2026-10-18; AX window title and System Events menu-bar read in both languages | The application-menu label is intentionally language-neutral; it is the bundle/process identity, not a document title |
| Wall Settings dialog labels, choices, group headers and buttons follow the language | runtime observation | Final packaged app, 2026-10-18, `⌘T` on a selected wall; first group read field-by-field, all six group headers read after scrolling | Remaining groups' fields were not edited one-by-one on the final bundle; unit and dictionary tests cover them |
| Info Box wall fields follow the language and stay editable | runtime observation | Final packaged app, 2026-10-18, one wall selected with the Info Box open; Chinese fields read item by item, English render covered by regression | The final bundle's desktop pass read the Chinese Info Box fields individually; the English field list was checked through the render regression |
| Info Box wall-tool defaults follow the language | runtime observation | Final packaged app, 2026-10-18, Wall tool active with nothing selected; Chinese fields read item by item, then English fields read item by item after relaunching with the `en` preference | The English pass used a same-content test copy (`CFBundleIdentifier` changed only) and wrote the preference file directly; it did not exercise the selector gesture |
| Door/Window Info Box labels follow the language | readable source, unit regression and dictionary check | `info_box.rs`, `locales/en.json`, `locales/zh-CN.json`; `door_info_box_shows_and_edits_hosted_opening_fields`, `opening_tool_defaults_are_editable_and_localized`; 462-key localization check | Chinese labels were read on the `A5BCB793` package; English Door/Window Info Box rendering was not re-read on that package |
| Wall reference-line editing and snap text follows the language | unit regression, dictionary check and desktop smoke | `wall_reference_line_palette_renders_in_both_languages`, `wall_endpoint_snap_preview_matches_the_active_language`; `snap` label and join status read on the `638273BF` bundle | Both languages rendered headlessly; on the current desktop bundle the Chinese drag showed `捕捉：墙参考线` and the release status `墙端点已连接到参考线`, but the palette body was not re-read field by field in Chinese |

## Behavior contract

- The title-bar selector shows `English` or `简体中文` and lists both choices.
- `View > Language` exposes the same two choices.
- Choosing a language updates the main-window labels immediately in the current
  frame; no restart or project reopen is required.
- The language change writes the preference file and reports
  `Language · English` or `语言 · 简体中文` in the status bar, replacing any
  status text left in the previous language.
- Switching language does not mutate `Project`, clear or add selection, or add
  an undo/redo entry.
- On the next launch, the saved language is loaded before the first frame.
- The window title is `OpenArchi — Architectural Workbench` or
  `OpenArchi — 建筑设计工作台` and changes in the same frame as the rest of the
  UI. The macOS application-menu label is always `OpenArchi`; eframe derives it
  from the launch-time viewport title, so the launch title stays
  language-neutral and the localized title is applied from the first frame.
- Missing or unreadable preference data falls back to English.

## Main-window coverage

The localized surface includes the application title, top menus and toolbar,
Info Box, toolbox groups and tools, plan/split/3D view tabs, elevation tab,
element and selection counts, Quick Options bar and palette (zoom, orientation,
scale, layer combination, partial structure, pen set, model view options,
graphic overrides, renovation filter, design options and dimensions), status
messages, view-settings and navigator groups, and representative dialogs.

## Acceptance checklist

| Check | Result | Evidence |
|---|---|---|
| Simplified Chinese is applied from the title-bar selector | pass — desktop | AX tree changed to `文件 / 编辑 / 视图 / 设计 / 文档 / 选项 / 协作 / 窗 / 帮助`, `分屏视图`, `楼层平面 · F2`, `南立面`, `已选 0 个`, `12 个构件`, `就绪`, and Chinese Quick Options values |
| English is restored from the title-bar selector | pass — desktop | AX tree returned to `File / Edit / View / …`, `Split view`, `Floor plan · F2`, `South Elevation`, `0 selected`, `12 elements`, `READY`, `Language · English` |
| `View > Language` exposes and activates both choices | pass — desktop | On the final bundle the `视图 > 语言` submenu listed `English` and `简体中文`; choosing `English` changed the main window and status to `Language · English`; the title-bar selector then restored `简体中文` |
| Preference is written | pass — desktop | Preference file changed `en` → `zh-CN` → `en` |
| Language survives restart | pass — desktop | After choosing `简体中文` and relaunching the final bundle, the first AX tree was already Chinese |
| Window title follows the language | pass — desktop | Title-bar selector: English `OpenArchi — Architectural Workbench` → 简体中文 `OpenArchi — 建筑设计工作台`; both updated immediately and survived restart |
| Application menu stays language-neutral | pass — desktop | `osascript … get name of every menu bar item of menu bar 1` returned `Apple, OpenArchi` in both languages; it never showed a stale translated title |
| Switching does not edit the project or undo history | pass — desktop and unit regression | `Undo`/`Redo` stayed disabled and the model remained at 12 elements; `simplified_chinese_workspace_renders_without_changing_model_state` compares the project before/after rendering |
| Main labels and counts follow the language | pass — unit regression | `main_window_status_tabs_and_counts_follow_the_selected_language`, `status_text_localizes_dynamic_messages_without_rewriting_editor_state`, `main_window_and_status_labels_are_localized_in_chinese` |
| Quick Options labels follow the language | pass — unit regression | `quick_options_bar_and_palette_render_in_the_selected_language` |
| Wall Settings dialog follows the language | pass — desktop and source | English: `Wall Settings`, `GEOMETRY AND POSITIONING`, `Height`, `Top Link`, `Center / Inside / Outside`, `Cancel / Apply / OK`; Chinese: `墙设置`, `几何与定位`, `高度`, `顶部链接`, `居中 / 内部 / 外部`, `取消 / 应用 / 确定`; all six English and Chinese section headers were read after scrolling |
| Switching language leaves selection and undo history unchanged | pass — desktop | With one wall selected, `1 selected` / `已选 1 个`, `12 elements` / `12 个构件`, and disabled `Undo`/`Redo` stayed identical across the switches |
| Info Box wall fields follow the language | pass — desktop and unit regression | Chinese: `墙 009`, `全部已选: 1`, `图层`, `结构功能`, `几何方法`, `参考线`, `厚度`, `高度`, `顶部链接`, `平面图与剖面…`, `设置…`, `应用`; English field labels covered by `info_box_renders_wall_fields_and_follows_palette_visibility` |
| Info Box wall-tool defaults follow the language | pass — desktop (Chinese and English) | Chinese: `墙`, `默认设置`, `图层：`, `几何方法：`, `参考线：`, `参考线偏移`, `结构：`, `厚度`, `高度`, `顶部链接：`, `顶部偏移`, `底部偏移`, `平面图与剖面…`, `设置…`, `应用`. English: `Wall`, `Default Settings`, `Layer:`, `Geometry Method:` (`Single`/`Continuous`/`Polygonal`/`Rectangular`), `Reference Line:` (`Center`/`Inside`/`Outside`), `Reference Offset`, `Structure:`, `Thickness`, `Height`, `Top Link:`, `Top Offset`, `Bottom Offset`, `Floor Plan and Section…`, `Settings…`, `Apply`. Selecting `Outside` reported `Wall default settings applied · New walls use these values` with 12 elements and disabled `Undo`/`Redo`; `Settings…` opened the English `Wall Default Settings` dialog and `Escape` reported `Wall Settings cancelled` |
| Language switching keeps the Info Box editable and model-stable | pass — desktop | After `简体中文 → English → 简体中文`, the wall stayed selected at `12 elements`, `Undo`/`Redo` availability did not change, and the Info Box still accepted edits |
| Door/Window fields follow the language | pass — unit regression and dictionary | `门 · 洞口`, `宽度`, `高度`, `窗台高度` render in Chinese; `Host Wall`, `Width`, `Sill Height` are present in both dictionaries and covered by the opening UI regressions |

## Implementation

- `src/localization.rs`: `UiLanguage`, preference load/save, and the embedded
  `locales/en.json` / `locales/zh-CN.json` dictionaries.
- `src/app/shell.rs` and `src/app/mod.rs`: language selector, menu entry,
  translated main-window labels, `status_text` and count formatting.
- `src/editor.rs`: canonical default status text, translated only for display.

## Remaining unknowns

- No Archicad 29 runtime comparison was performed for language switching; this
  feature follows OpenArchi's own menu structure and labels.
- Dialogs outside the main floor-plan window were not each re-read in both
  languages after the final build. Wall Settings was re-read in both languages;
  the remaining wall groups' individual fields were not edited one-by-one on
  the final bundle.
- The English Info Box wall-tool defaults were read after writing the `en`
  preference directly to a same-content test copy. This supports label
  localisation and editability, not an independent selector gesture for that
  pass; both selector entry points themselves are covered by the final-bundle
  checks above.
- The stage 5 package `A5BCB793-F9D4-3652-A4C4-076E3CFBDFC5` was only checked
  for Chinese first-frame rendering, the `门` / `窗` toolbox labels and the
  Door Info Box's Chinese fields. Its title-bar menu opened with both choices,
  but English activation was not repeated on that package; the prior
  final-bundle selector evidence predates the opening labels.
