# Quick Options bar and palette — Archicad 29 reference / OpenArchi implementation

## Scope and evidence

Goal: reconstruct the Archicad 29 floor-plan Quick Options controls as original
native OpenArchi controls, and connect them to the same model/view state as the
corresponding Document and Options menu commands. This is a focused subset of
Archicad's attribute and view system, not a claim of full parity.

Reference target: Archicad 29.0.0 v1 INT (5101), arm64 macOS,
`/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`, SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`
(from `scripts/ac29-workflow.py context quick-options`).

| Claim | Type | Source / evidence | Scope and limits |
|---|---|---|---|
| Quick Options reflects the active model window's settings | public documentation | [Archicad Quick Options](https://help.graphisoft.com/AC/29/INT/_AC29_Help/020_Configuration/020_Configuration-20.htm) | Documented behavior; does not enumerate the local icon set |
| The default Quick Options bar is docked at the bottom of the model window | runtime observation | Local AC29 screenshot, 2026-10-17, 3840×2160 capture | Screenshot not committed; no REA Evidence ID |
| A separate floating `Window > Palettes > Quick Options` entry exists | runtime observation | Local AC29 Window menu walk-through, 2026-10-17 | Menu presence and separate palette entry only; no REA Evidence ID |
| Visible control order is Zoom, Orientation, Scale, Layer Combination, Partial Structure, Pen Set, Model View, Graphic Overrides, Renovation Filter, Design Options, Dimensions | runtime observation | Same local screenshot; icons distinguish stacked layers from the pen glyph | The first three and the final four were only read from the capture; no REA Evidence ID |
| Observed defaults are `5.9%`, `0.00°`, `1:100`, `02 Drafting`, `Entire Model`, `03 Architectural 100`, `03 Building Plans`, `No Overrides`, `00 Show All Elements`, `Main Model Only`, `Plain Meter` | runtime observation | Same local screenshot | Project/template-dependent; OpenArchi does not copy the project |
| Document menu order is Layers, Floor Plan Scale…, Partial Structure Display…, Pen Sets, Model View, Graphic Overrides, Design Options, Renovation | runtime observation | Local AC29 menu walk-through, 2026-10-17 | Menu labels only; dialog internals are out of scope |
| OpenArchi control order, menus and shared state | readable source | `src/app.rs::quick_options_bar`, `quick_options_palette`, `modify_plan_options`, `src/view_settings.rs::PlanOptions` | Source is authoritative for OpenArchi, not for Archicad |

An earlier review misread the fourth and sixth AC29 controls because both values
start with `0`. The capture shows `02 Drafting` beside a stacked-layer icon
(Layer Combination), followed by `Entire Model` (Partial Structure), then
`03 Architectural 100` beside a pen glyph (Pen Set). OpenArchi follows that
order. AC29 icon artwork is proprietary; OpenArchi uses original vector glyphs.

## Behavior contract

### Bar and floating palette

- The bar is a full-width strip above the bottom status bar and is visible by
  default. It scrolls horizontally in narrow windows.
- `Window > Hide/Show Quick Options Bar` toggles only the docked bar. The label
  follows the current visibility.
- `Window > Palettes > Quick Options` toggles an independent floating palette.
  Its checked state and the docked bar's visibility do not control each other.
- Bar/palette visibility is session UI state. It does not mutate the project, add
  an undo entry or mark the document dirty.
- The bar and palette render the same eleven settings and open the same menus.

### Controls

1. `Zoom` is view-only. Presets are 10, 20, 50, 100, 200, 500 and 1000 percent;
   the current zoom can be saved under a session name. It does not edit the model.
2. `Orientation` offers 3.22°, 183.22°, 0°, 90°, 180° and 270°, plus a two-point
   graphical pick. The plan rotates about its centre and pointer/world conversion
   uses the same angle.
3. `Scale` sets the drawing scale denominator. Presets and a validated custom
   value update line weights and dimension display.
4. `Layer Combination` selects one of the built-in layer sets or `Show All
   Layers`. The Layers dialog edits the same `PlanOptions` value.
5. `Partial Structure` selects Entire Model, Without Finishes, Core Only or Core
   of Load-Bearing Elements Only.
6. `Pen Set` changes the plan pen palette and line weights. The default is
   `03 Architectural 100`.
7. `Model View Options` controls part visibility, fill policy and stair treads.
8. `Graphic Overrides` applies the selected plan-wide override combination.
9. `Renovation Filter` filters and highlights existing, demolition and new
   elements. `Reset Renovation Status` resets the selection, or the whole model
   when nothing is selected.
10. `Design Options` chooses Main Model Only or one design option. The manager
    creates, deletes and activates options and assigns or returns selected
    elements. Main-model elements remain visible in every option.
11. `Dimensions` selects DIN, Plain Meter, Plain Millimeter, US Architect, US
    Builder or US Detailing formatting for plan dimensions and coordinates.

### Shared state, validation and undo

- Every control changes `Project::plan_options` through `modify_plan_options`
  except view-only zoom and the session-only save-zoom list.
- The same setters back the Quick Options menus, `Document` menu entries and the
  relevant `Options` dialogs. There is no second copy of the state in the UI.
- A changed option is validated, committed as one undoable project edit and
  reported in the status bar. Undo restores the previous option value.
- Design-option membership is stored on elements; deleting an option returns its
  elements to the main model and clears it as the active option.
- `PlanOptions` and `DesignOptions` round-trip through project JSON; older files
  receive defaults through serde defaults.

## Acceptance checklist

| Check | Result | Evidence / gap |
|---|---|---|
| Eleven controls appear in AC29 order | pass — source and packaged desktop | `quick_options_bar`; `dist/OpenArchi.app` full-window capture |
| Bar and floating palette have independent visibility | pass — unit regression | `quick_options_bar_and_palette_visibility_are_independent` |
| Palette/bar visibility does not mutate project or undo | pass — unit regression | same test compares `Project` and undo length |
| Layer combination, partial structure, pen set, MVO, overrides, renovation and dimensions update shared state | pass — unit regression | `quick_options_edit_plan_options_through_undoable_commands` |
| Option changes are undoable | pass — unit regression | same test changes seven options and undoes all seven |
| Layers and design options gate plan visibility | pass — unit regression | `quick_option_visibility_follows_layers_and_design_options` |
| Orientation survives world/screen conversion | pass — unit regression | `plan_orientation_round_trips_between_world_and_screen` |
| Zoom label and bar visibility round-trip | pass — unit regression | `quick_options_zoom_and_bar_visibility_round_trip` |
| Every dropdown opens in the packaged app | pass — desktop menu walk-through | All eleven menus opened and their entries were read; popup value selection was not stable under CUA pointer mapping |
| Exact AC29 icon artwork | intentionally different | original vector glyphs, no proprietary assets |

## Delivery evidence

Source symbols:

- `src/app.rs::quick_options_bar`, `quick_options_palette`, `palette_row`,
  `modify_plan_options`, the `set_*` option commands, and the per-control menu
  contents.
- `src/view_settings.rs::PlanOptions`, `LayerCombination`, `PartialStructureDisplay`,
  `PenSet`, `ModelViewOptions`, `GraphicOverrides`, `RenovationFilter`,
  `DimensionStyle` and `DesignOptions`.
- `src/model.rs` serializes `PlanOptions`, `DesignOptions` and per-element
  `design_option`.
- Menu wiring: `Document` and the `Options > Element Attributes / Project
  Preferences` commands open the same option state; `Window` owns bar and
  palette visibility.

Targeted regressions are listed in the acceptance table. Full-suite counts,
package UUID and desktop results are recorded in [Validation](../validation.md)
after the final build.

## Remaining unknowns

- The exact AC29 implementation of layer/pen/MVO/override dialogs is not
  reconstructed; OpenArchi provides its own typed subset and original UI.
- AC29 stores some of these settings per view; OpenArchi currently stores one set
  of plan options per project.
- 3D styles and other Quick Options variants outside the floor-plan model window
  are out of scope.
- Bar and palette visibility are not persisted across application restarts.
