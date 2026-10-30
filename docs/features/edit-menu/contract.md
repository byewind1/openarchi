# Edit menu — AC29 INT 5101 — 2026-10-28

> 2026-10-28 用户纠正：现有快捷键仍进入数值对话框，**不等于完成 AC29 鼠标编辑交互**。后续实现必须查阅 [鼠标交互与过程修饰键契约](pointer-interaction.md)；该记录包含官方帮助、用户描述差异及未决项。完整标准配置见 [171 条快捷键](standard-shortcuts-full-29.md)。


## Evidence and scope

Target identity: Archicad 29.0.0 v1 INT (5101), SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`.
Runtime observation: CUA AX menu capture on 2026-10-28, existing `GDL 测试 2.pln`, no selection; clicked Edit then cancelled the menu. No project mutation. CUA supplies no REA Evidence ID. This proves labels/hierarchy only, not editing semantics.

Observed entries:
- Undo, Redo, Repeat Last Command, Cut, Copy, Paste, Delete, Select All, Find & Select…, Search & Replace Text….
- Groups: Group, Ungroup, Suspend Groups, Autogroup.
- Display Order: Bring to Front, Bring Forward, Send Backward, Send to Back, Reset Default Order.
- Locking: Lock, Unlock, Unlock All.
- Move: Drag, Rotate, Quick Rotate Clockwise, Quick Rotate Counter-Clockwise, Mirror, Elevate…, Drag a Copy, Rotate a Copy, Mirror a Copy, Drag Multiple Copies, Rotate Multiple Copies, Multiply…, Edit Elements by Stories….
- Align: Horizontal Left/Center/Right, Vertical Top/Center/Bottom, Special Align.
- Distribute: Along X, Along Y, Along XY, Special Distribute.
- Reshape: Trim, Stretch, Resize…, Split, Fillet/Chamfer…, Intersect, Adjust, Offset, Explode into Current View, Unify, Linework Consolidation…, Fill Consolidation…, Split into Single-plane Roofs, Adjust Elements to Slabs…, Merge Beams, Merge Columns, Merge Walls.
- Element Settings: Tool Settings, Edit Selection Set…, Pick Up Parameters, Inject Parameters, Element Transfer Settings…, Relink Home Story….
- Reference Line and Plane: Modify Wall Reference Line…, Invert Wall Direction, Modify Slab Reference Plane….

Public documentation:
- https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-100.htm (groups, nested groups, suspend, hosted openings, locked members)
- https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-84.htm (resize, reference point, grouped elements)
- https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-86.htm (adjust to line/edge)

## Implementation contract (reversible OpenArchi assumptions)

Use a shared command catalog for native and egui menus. Numeric dialogs operate on a captured selection and a private draft; Cancel/Escape leaves the model/history unchanged. Coordinates/distances are metres, angles degrees. Validate the complete candidate project before one undo record. Invalid operations must leave model, selection and history unchanged. Undo/redo restore exact project values, including grouping and locks. Copy retains all element metadata and remaps host IDs and groups. Locked members reject mutations atomically.

Existing model types are walls, axis-aligned rectangular slabs, stairs, polylines (including arcs), hosted openings. Arbitrary rotation of rectangular slabs cannot be approximated by an enlarged bounding box. Operations without a corresponding model or faithful geometry are explicitly unavailable, not preview-only successes. Numeric workflows are an original first implementation, not evidence of identical AC pointer interaction.

Acceptance: multi-selection, invalid input/locked member rollback, hosted copy remapping, nested group selection, suspend/ungroup, exact undo/redo and JSON roundtrip, numeric transform cancel/apply, EN/ZH menus/dialogs/status, restart language and final packaged app identity. Record every unimplemented command and unobserved UI surface honestly in validation.
