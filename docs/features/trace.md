# Trace & Reference — Archicad 29 reference / OpenArchi implementation

## Scope and evidence (2026-10-17)

Goal: reconstruct the Archicad 29 `Trace` button, `View > Trace Options`
commands and the useful floor-plan part of the `Trace & Reference` palette in
the independent native OpenArchi model.

Reference target: Archicad 29.0.0 v1 INT (5101), arm64 macOS,
`/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`, SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`
(reused from `python3 scripts/ac29-workflow.py context trace`).

Runtime observation (CUA, current Archicad process, Untitled project, Ground
Floor plan):

- The Standard Toolbar shows a `Trace` toggle followed by a separate drop-down
  menu. Enabling the toggle changed its AX value to `on`.
- The drop-down contained, in order: `Choose Reference` (with `Browse for Trace
  Reference…`, `Above Current Story`, `Below Current Story`, `Previous Story`,
  `Entire Model Display`); `Activate Reference`; `Switch Reference with
  Active`; `Drag Reference`; `Rotate Reference`; `Reset to Default Position`;
  `Rebuild Reference`; `Temporarily displace Reference`; `Show/Hide Splitter`;
  `Reference on Top`; `Transparent Fills and Zones`; `Turn off Trace
  everywhere`; `Trace & Reference`.
- The visible Trace & Reference palette showed `Reference:` and `Active:`
  intensity sliders, both reporting 255 in the captured state. The palette was
  not exhaustively inspected; its buttons and pop-ups below the sliders remain
  partially observed. CUA captures taken here do not have REA Evidence IDs.

Public documentation (AC29 INT):

- [Virtual Trace: Using References to Edit and Compare Model Views and Drawings](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-103.htm)
- [Choose a Trace Reference](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-104.htm)
- [Trace Reference on/off](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-105.htm)
- [Set Color of Reference Elements](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-106.htm)
- [Show/Hide Elements in Reference](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-107.htm)
- [Move/Rotate/Reset Reference](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-108.htm)
- [Switch Reference with Active](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-109.htm)
- [Activate Reference](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-110.htm)
- [Rebuild Trace Reference](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-111.htm)
- [Compare Reference with Active](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-112.htm)

Documented semantics used here:

- The Reference is a temporary editing aid tied to the active window. A floor
  plan has one current Reference shared by its stories. Reference elements are
  visible and snap-able but cannot be selected or edited until the Reference
  and Active are switched.
- `Above Current Story` and `Below Current Story` are relative to the current
  story. `Previous Story` means the last story the user was on. `Entire Model
  Display` shows the complete model rather than the active story's display.
- Reference and Active intensity sliders fade their respective content. A
  uniform Reference color may replace original element colors. Fill transparency
  is an on-screen-only comparison aid.
- Drag/Rotate move the Reference. Reset restores its default position. Rebuild
  refreshes a stale Reference. Reference on Top reverses display order. The
  splitter compares the two views side by side; Temporarily Displace moves the
  Reference only while inspecting underneath it.

## Contract / reversible implementation choices

- OpenArchi has floor plans rather than sections/elevations/layouts, so this
  slice implements Story references and Entire Model Display. Browse lists the
  project stories plus the Entire Model option. Other Archicad reference types
  remain outside this model.
- The Trace button and `View > Trace Options` share one typed `TraceState`.
  The Reference is rendered from the live project, so it updates whenever the
  model changes; `Rebuild Reference` reports a successful refresh rather than
  caching a second copy of the model.
- Default Reference is `Below Current Story`, matching the state visible in the
  captured Archicad palette. OpenArchi starts with Trace off, as in the captured
  Archicad state before the toggle was pressed. Enabling Trace switches the view
  to Floor plan. If the default has no target (for example on the lowest story),
  enabling Trace selects the nearest other story and reports that selection;
  choosing an unavailable relative reference explicitly stays unresolved.
- The demo project includes a small first-floor balcony so the default
  Below/Above Story reference has visible content on first use.
- OpenArchi uses metre coordinates. Reference offset is stored in world metres;
  rotation is around the Reference bounds center. Reset sets both to zero.
- Reference drawing supports uniform/original color, intensity, element-type
  visibility, transparent fills, Reference on Top and a movable splitter. The
  splitter is a swipe comparison over one shared pan/zoom viewport: the left
  side reveals the Reference, the right side the Active content, and the divider
  can be dragged. It is not two independently navigable views.
- `Switch Reference with Active` changes the active story to the Reference story
  and makes the old active story the new absolute Reference. `Activate
  Reference` switches to the Reference viewpoint and keeps a relative
  Above/Below selection where meaningful.
- Trace settings are view/session state in this slice, not project data; they do
  not create geometry undo entries or mark the model dirty. Persistence of Trace
  settings with the project is deferred.
- Archicad's Active color override and cross-window/per-view Reference settings
  are not implemented. The palette marks `Apply Settings to All References`
  unavailable because OpenArchi has one floor-plan Reference.

## Acceptance checklist

1. The Standard Toolbar `Trace` toggle and drop-down expose the same command
   names and order as the captured AC29 menu; `Trace & Reference` opens the
   palette.
2. Above/Below/Previous Story resolve to the correct story; Browse offers every
   story and Entire Model; choosing a Reference enables it in Floor plan.
3. Reference geometry is visible for walls, slabs, stairs and polylines and can
   be filtered by element type.
4. Reference/Active intensity, uniform Reference color and transparent fills
   change only display; the model and undo history stay unchanged.
5. Drag, Rotate, Reset, Temporarily Displace and Splitter operate without editing
   model geometry; Escape exits the active Trace pointer mode.
6. Reference on Top reverses layer order; Turn off Trace everywhere disables it;
   Activate and Switch update the active story coherently.
7. The packaged app is rebuilt and a native smoke test is attempted.

## Desktop smoke test (2026-10-17)

Run on the packaged `dist/OpenArchi.app` (PID 54133, arm64 UUID
`009A285C-2326-30F3-AF2C-9BCFFFACE4B6`) in English with the demo project
(12 elements).

- Observed: Trace on reported `Trace Reference on · 01 · First floor` and drew
  the pale first-floor balcony over the ground-floor plan; the `Trace ▾` menu
  listed all thirteen commands in the contract order; `Window → Trace &
  Reference` opened the palette with the captured controls; `Switch with
  Active` swapped the active story to `01` and the reference to `00` without
  enabling Undo; `Splitter` drew the divider with the `Active` label; `Drag`
  entered pointer mode and Escape reported `Trace pointer mode cancelled`.
- Limits: CUA pointer coordinates for the egui floating palette required
  iterative calibration, and its drag gesture did not produce a reference
  offset in the plan. Reference displacement is therefore covered only by the
  RawInput test, not by a completed desktop drag. Rotate, Displace, Reference
  on Top, transparency, visibility filters, color/intensity edits, Browse,
  Turn off Trace everywhere, reopening the project and the 3D view were not
  exercised on the desktop in this pass.

## Known gaps

- Only floor-plan Story and Entire Model references exist; Views, Layouts,
  worksheets, sections/elevations and details are absent from the current model.
- Reference elements are drawn but not included in snap or hit-testing yet.
- Trace configuration is not saved in OpenArchi JSON yet.
- Active color override, full/recent Reference lists and multi-window settings
  are not implemented.
- Archicad was observed for command structure, not for pixel-perfect default
  ghost palette, splitter appearance or rotation gesture thresholds.
