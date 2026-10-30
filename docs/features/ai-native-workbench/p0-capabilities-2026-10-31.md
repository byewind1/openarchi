# P0 能力与写入口矩阵

2026-10-31；源码调查，非全局 Archicad 等价认证。自动回归按最终收据统一核验；历史桌面证据只属于对应旧包。未逐项操作最终包的格均为 unknown。本文第一版为有限主线盘点，不把有界搜索当作全部写入口已审完。

| 对象/入口 | 视图与操作 | 共享源码/命令 | 现有回归（符号） | 原生证据与边界 |
|---|---|---|---|---|
| 墙工具/Info Box | 平面创建、多方法草稿切换 | editor::click_wall/create_wall_segments；wall-method-switch 契约 | continuous_wall_method_builds_a_connected_chain_in_one_undo；info_box_wall_geometry_method_drives_real_drawing_behavior | [切换契约](../wall-method-switch.md)；保留前缀矩形为批准的产品选择 |
| 墙参考线/节点面板 | 平面、投影3D插点、变换、节点及交点 | wall_node_ui、editing、reshape；ReshapeCommand | wall_endpoint_palette_switch_roundtrip_keeps_endpoint_drag_and_undo；wall_junction_plan_drag_all_selected_ends_and_cancel_in_both_languages | [墙面板](../wall-pet-palette/contract.md)、[交点](../wall-junction-edit/contract.md)；曲墙交点不支持 |
| 楼板工具/选中边 | 平面创建、13项直边面板、曲边保弧插点、节点面板；3D派生/拾取 | slab_geometry、wall_node_ui、ReshapeCommand/SlabEdit | slab_full_pet_all_thirteen_commands_have_real_click_activation_and_cancel；slab_full_pet_custom_angle_numeric_apply_and_undo_both_languages；slab_full_pet_polygon_subtract_waits_for_enter_and_hole_node_remains_editable | [契约及完整包记录](../slab-edge-editing/contract.md)；直边插点旧包已实测，完整面板最终包鼠标未知；斜边裁剪/独立半径锁/复制修饰键未验收 |
| 多段线工具/节点边 | 平面；共享面板、曲边、变换、精确输入 | polyline_edit、editing、wall_node_ui | polyline_pet_staged_rotate_and_mirror_use_same_transform_as_walls；polyline_pet_overlapping_node_wins_over_other_edges_and_hidden_stories | [多段线](../polyline-editing.md)；不进入3D，无区域填充 |
| 门/窗工具/属性 | 直墙宿主矩形洞口；平面/3D | editor::click_opening、openings、ModelCommand | tests/openings.rs；invalid_property_edit_is_atomic | [洞口](../openings.md)；无GDL门窗、曲墙或跨墙开洞；Marquee有边界 |
| 楼梯工具/设置 | 有限直跑；平面/3D | stairs、editor；app/dialogs | pointer_stair_creation_persists_parametric_settings_and_undoes | [楼梯源码](../../../src/stairs.rs)；无完整多跑/栏杆/规范检查；本轮原生未验收 |
| 图层设置/命令 | 隐藏/锁定/交接组，平面/3D | layers、editor::apply/commit | archicad_layer_cannot_be_hidden_locked_or_deleted；pointer_drag_locked_selection_does_not_commit | [图层](../layer-settings/contract.md)；锁定旁路需继续逐入口审计 |
| 文件保存/恢复 | JSON校验、文件替换；不改视图模型 | storage::save/load、recovery::write/read | tests/storage_integrity.rs；recovery_roundtrip_and_clear；disk_save_load_and_failed_save_leave_project_intact | [存储契约](../persistence/contract.md)；唯一临时文件不等于文档并发冲突检测；恢复侧文件共享问题保留 |

## 首轮写入口审计与迁移顺序（P0.4）

| 入口 | 已确认路径/保护 | 缺口与后续 |
|---|---|---|
| Editor::apply | 类型化命令；locked检查；validate失败逆向还原；记录delta | 不是完整批事务/幂等接口；P1逐调用者迁移 |
| Editor::commit(before) | 直接改项目后的差异提交；既有锁定元素变化会回退 | 无统一完整validate；新增到锁定层等须逐入口核对，不能仅以final guard宣称完整保护；本轮不在无契约时扩大行为 |
| editor创建/属性/粘贴/变换 | 多处先clone/候选、validate，再commit | 保留共同行为；下一切片补非法输入、锁定、历史与重开矩阵 |
| app/dialogs、quick_options、shell、native_menu | 仍直接写选项/属性并commit | 迁移前分别区分项目设置与纯显示设置；避免修改dirty/历史语义 |
| app/plan交互拖动 | 预览与before快照、release提交 | 与私有候选路径并存；不得机械禁止当前项目预览，先保留cancel/失焦回归 |
| 文件Open/New/恢复 | 替换Editor或restore_unsaved；不是普通历史命令 | 依赖已存在未保存保护；文档会话身份/revision在P1定义 |
| storage::save/recovery::write | JSON先validate；本轮独占临时文件+同步后rename | 共享恢复位置、多实例覆盖、目录断电耐久、外部文件基线冲突仍未解决 |

标题栏语言选择器按 [2026-10-25原生标题契约](../native-menu-title/contract.md) 移除；当前入口为 View > Language。这是已有批准的呈现差异，不能为了满足旧双入口检查擅自恢复。
