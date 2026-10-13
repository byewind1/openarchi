# Archicad 29 interface inventory for OpenArchi

This is a reconstruction checklist based on the locally installed Archicad 29
running on Apple Silicon. It records visible workflow organization, not internal
implementation. Menu contents vary by workspace, installed extensions, active
view, selection, and licensing.

## Evidence and scope

- Target: `/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`, arm64 Mach-O,
  SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`.
- Runtime: existing process PID 14896, window 38986, project title `Untitled`.
- REA native UI observation: `ev_95c1c652b282ef9e446a3e7abc38fd65d51e1d8ff181f386aece828456b0cc75`.
  The accessibility tree confirmed a single native work window, tool/control
  groups, view tabs, Navigator and inspector regions. The macOS menu bar is not
  part of that window tree; top-level menu contents below were independently
  read by opening and dismissing each native menu in the same running app using
  CUA. No menu command was executed.
- A separate 3D-window observation: `ev_4c0116ea01b5b94855f5e210e11735d241a5c8b2dc9e1ccb8d9c0a57dd9d0366`.
- This is a workspace snapshot, not a promise that every template, license,
  add-on, or tool profile exposes identical entries. Disabled entries were
  visible but not tested. Dialog contents, hidden submenus and alternative
  workspace profiles remain unverified.
- Read-only CUA expansion covered 14 top-level menus and 1,272 accessibility
  tree lines in the current workspace: 10 core menus plus OpenBrep,
  Extensions JPN, BIMOMScratch and Tapir. The menu trees were opened then
  dismissed without activating a command. The categorized inventory below
  condenses that capture; it does not claim each inactive command was tested.

## Main-window layout checklist

| Region | Archicad 29 observation | OpenArchi target/check |
|---|---|---|
| macOS app menu | About, Work Environment, project preferences, Services, Hide/Quit | ☐ Use an OpenArchi app menu; product identity remains OpenArchi |
| Main menu row | File, Edit, View, Design, Document, Options, Teamwork, Window, Help; this installation also has extension menus | ☐ Keep a compact native-style row; extensions are not core menus |
| Standard/tool bars | Standard commands, drafting aids, element edit, arrangement, attributes, 3D navigation and tool-family bars can be shown from Window | ☐ Separate standard actions from contextual Info Box controls |
| Toolbox | Arrow and Marquee first; architectural tools grouped below | ☐ Persistent left rail with grouped, legible tool names and shortcuts |
| Info Box | Changes with active tool/selection; common drawing attributes and tool-specific controls | ☐ Dedicated band under main toolbar; contextual fields, not generic settings |
| Tracker | Numeric values appear while drawing/editing; coordinate input and constraint settings are configurable | ☐ Keep live numeric input adjacent to drawing workflow |
| View tabs | Floor plan, 3D, elevations and other viewpoints coexist as tabs | ☐ Use one document window and tabs; F2 plan, F3 perspective 3D, F5 local 3D |
| Work area | Plan, perspective/axonometric 3D, section/elevation, schedules, layouts, etc. | ☐ Shared model and clear active-view indication |
| Navigator | Project Map, View Map, Layout Book and Publisher sets; current project tree shows stories, sections/elevations, worksheets, details, 3D docs, schedules, indexes, lists, info/help | ☐ Right tree with expandable story/view categories and selected-story state |
| Properties/selection | Contextual element settings and project/view properties appear in palettes/dialogs | ☐ Right inspector with Properties, Library and AI Assistant tabs |
| Status/quick options | Prompt/status, view scale, layer/selection and quick options | ☐ Persistent bottom status and coordinate feedback |

## Visible Toolbox tools

Directly visible in the current architectural workspace: Arrow, Marquee, Wall,
Slab, Column, Beam, Door, Window, Object, Lamp, Roof, Shell, Stair, Railing,
Curtain Wall, Skylight, Corner-Window, Opening, Wall End, Mesh, Morph, Zone.
The menu also groups independent drafting/documentation tools (Section,
Elevation, Interior Elevation, Detail, Worksheet, Camera, Dimension, Level
Dimension, Radial Dimension, Angle Dimension, Text, Label, Fill, Line,
Arc/Circle, Polyline, Spline, Hotspot, Figure and Drawing). Structural and MEP
profiles expose additional specialized tools listed under Design below.

OpenArchi status: ☑ Arrow/select, ☑ Marquee, ☑ Wall, ☑ Slab are interactive.
☐ Other architectural/drafting tools are interface previews only or absent;
do not imply these create native model elements yet.

## Menu inventory observed in this workspace

Items are grouped as they appeared under each top-level menu. This is a breadth
inventory; commands marked disabled by Archicad were inactive in the blank
project/current view. Add-on menu names are installation-specific.

### Archicad (application menu)

- About Archicad; Work Environment; Project Preferences (working units,
  dimensions, calculation units/rules, zones, reference levels, legacy, stair
  standards, structural analytical/load units, location, project north, survey
  point); Services; Hide/Hide Others/Show All/Quit.

### File

- Home; New / New & Reset All / Share; Open / Open & Join Teamwork / Download
  and Open from BIMcloud / Import Travel Pack / recent files; Close Project;
  Save / Save As / Save as Template / Save Selected as Module; BIMcloud save and
  publish; Publish/BIMx.
- Interoperability: Merge; IFC project manager, translators, local preferences,
  export/import, change detection/update; SAF translators; DXF-DWG translation;
  classifications and properties exchange; Grasshopper, Bimplus, IDS, Bluebeam
  and CALHYDRA connections; survey point-cloud/mesh import; Google Earth export.
- External Content: hotlinks, external drawings, Xrefs, Drawing Manager.
- Libraries and Objects: Library Manager/BIMcloud Libraries, create/extract
  library container, New/Open Object, Save Selection as object parts, import
  blocks/RFA/Rhino/OBJ/FBX/PARAM-O.
- Info: Project Info, Action Center, Project Notes, Preview and Credits,
  Session Report, Teamwork comments. Plot Setup, Plot, Page Setup and Print.

### Edit

- Undo/Redo (contextual action label), Repeat Tool Settings; Cut/Copy/Paste/
  Delete/Select All; Find & Select; Search & Replace.
- Groups; Display Order; Locking; Move (drag, rotate, quick rotate, mirror,
  elevate, copy, multiple copies, multiply, edit by stories); Align; Distribute.
- Reshape: Trim, Stretch, Resize, Split, Fillet/Chamfer, Intersect, Adjust,
  Offset, Explode, Unify, Linework, Fill Consolidation, roof/slab, beam/column/
  wall merge operations.
- Element Settings, Edit Selection Set, Pick Up/Inject settings, transfer
  settings, Relink Home Story; modify/invert Wall Reference Line and Slab
  Reference Plane.

### View

- On-Screen View Options (including clean wall/beam intersections); Trace and
  Trace Options; Construction Grid, Editing Plane and Origin; grid/ruler/guides.
- Element Snap, Surface Snap, Snap Guides, Snap Points/options, Grid Snap,
  Distance Guides/options.
- Elements in 3D View: 3D Cutaway, Cutting Planes, Filter and Cut Elements in
  3D, Show Selection, Hide/Show All.
- 3D View Options: projection, perspective/axonometry, 3D styles (Basic,
  Vectorial, Simple Shading, Technical, Detailed, Shadows, White Model,
  Wireframe, hardware acceleration); structural analytical model options.
- Zoom, Fit, Home Zoom, Rotate Orientation, Orbit, Explore Model, 3D navigation
  extras; Refresh, Rebuild and Rebuild & Regenerate.

### Design

- Architectural Tools: Wall, Wall End, Column, Beam, Slab, Roof, Shell, Stair,
  Railing, Curtain Wall, Components, Door, Window, Corner-Window, Skylight,
  Opening, Zone, Mesh, Morph, Object, Lamp.
- Structural Engineering Tools: Support, Link, Point/Line/Surface Load.
- MEP Tools: Ductwork, Pipework and Cabling routes, branches, fittings,
  terminals, accessories and equipment.
- Stories and Connections: Story Settings, Edit Elements by Stories, trim/merge,
  trimming bodies, wall-to-curtain-wall, openings and connection cleanup.
- Solid Element Operations; Magic Wand; Modify Morph/Railing/Curtain Wall;
  Roof Extras/RoofMaker (rafters, trimmers, purlins, collar/tie beams, wizard);
  Design Extras/TrussMaker/hydronic settings; Grid System and Zone updates.
- Model Check (collision and physical/analytical quality/report), Model Compare,
  Structural Analytical Model generation/rules/elements; Energy Evaluation
  (review, profiles, environment/climate, systems, factors/costs, thermal
  bridges and simulation).

### Document

- Documenting Tools: Section, Elevation, Interior Elevation, Detail, Worksheet,
  Camera; independent Section/Elevation/Detail/Worksheet; dimensions, level,
  radial and angle dimensions; Text, Label, Grid Element and 2D drafting tools.
- Layers and view presentation: layer/model views, show/hide/lock, active layer,
  3D scale, partial structure display, pen sets, model view options, graphic
  override combinations/rules, design options, renovation filters/status,
  floor-plan cut plane.
- Issues/change/keynotes; annotation and automatic/secondary dimensioning;
  spell checking; rendering/sun studies/fly-through; schedules and listing
  extras; project indexes and property-based find. Many were disabled because
  no editable element or applicable view was active.

### Options

- Element Attributes: layers, lines, pens/color, fills, surfaces, building
  materials, composites, profile manager, zone categories, operation profiles,
  attributes/attribute manager; Property and Classification Managers.
- Structural load manager/combinations/mapping/categories; Add-On Manager.
- Project Preferences and Work Environment. Work Environment subdivisions
  include dialog/palette arrangement, selection/info, Tracker/coordinate input,
  input constraints/guides, rebuild, publisher, appearance, data safety/network,
  keyboard shortcuts, Toolbox, tool settings dialogs, Info Box, toolbars and
  menus.

### Teamwork

- Project Share, migration, Open/Join Teamwork Project, travel pack, Local Data
  Manager; BIMcloud manager, send/receive/reserve/release, assign elements,
  messages/reviews/change tracking; Teamwork Palette. Collaboration entries
  were disabled where the blank local project did not support them.

### Window

- Navigate: stories, sections/elevations/interior elevations/worksheets/details/
  3D documents, perspective/axonometry, schedules, indexes, element/component/
  zone lists and layout navigation.
- Tab/window actions: close/undock tab, close other tabs, overview, tab bar,
  scroll bar, full screen, hide palettes/toolbars.
- Toolbars: 3D Visualization, Arrange Elements, Attributes, Classic 3D
  Navigation, Drafting Aids, Drag/Edit Elements, Library Parts, Layouts and
  Drawings, MEP, Mini Navigator, Model Check, On-Screen View Options, Simple 3D,
  Standard, Structural Engineering/Loads, Teamwork, Tools–Architectural.
- Palettes: Toolbox, Info Box, Statusbar, Navigator, Quick Options, Organizer,
  Navigator Preview, Drawing Manager, Check Markers, Model Check Report, Issue
  Manager/Organizer, Change Manager, Design Options, Renovation, Trace &
  Reference, Keynotes, Favorites, Attributes, Profile Manager, rendering,
  IFC Project Manager, Energy Model Review, Element Information, Selections,
  Coordinates, Control Box, Quick Layers and extension palettes. Floor Plan,
  3D Window and open elevation tabs appear at the end.

### Help

- AI Assistant; Learning Center; Archicad Help; tool-tip/learning-content
  visibility; license/reference documentation; online support/resources;
  support package, updates, license information, account and About Archicad.

### Installed extension menus (not core Archicad)

- OpenBrep: OpenBrep launch and GDL Copilot (disabled in this snapshot).
- Extensions JPN: Japanese extensions for classification/properties, ceiling
  creation, GDL elevations, multi-wall opening, library parts, 2D utilities,
  alignment, instant materials, terrain and dimension helpers.
- BIMOMScratch: Scratch GDL palette.
- Tapir: Tapir palette, updates and about.
- Other extension visibility depends on installation/profile; do not copy
  extension names or commands into OpenArchi as product claims.

## OpenArchi GUI implementation checklist

- [x] Dark CAD frame, one main workspace, resizable side panels, status line.
- [x] Tool categories and active states for Arrow, Marquee, Wall and Slab.
- [x] Contextual Info Box for geometry/type/material, snapping and drawing aids.
- [x] Navigator Project Map, View Map, Layout Book and Publisher maps; story/view tree and Properties/Library/AI tabs.
- [x] Plan/3D view tabs and single-window F2/F3/F5 workflow.
- [x] Match the visible menu grouping and hierarchy across all core menu
      families; uncovered data exchange, collaboration and specialist commands
      remain menu previews, while the common menu families are browsable.
- [x] Expand Toolbox across architecture, openings, structure, site/objects,
      viewpoints, documentation, structural engineering and MEP. Core four
      remain connected to model operations; other entries are interactive UI
      previews with honest status messaging.
- [x] Add separate standard/edit toolbars and selection-specific Info Box
      controls.
- [ ] Review screenshot proportions, typography, separators, row density,
      selected/hover states and Navigator tree indentation against the captured
      native window.

### 2026-10-13 second-pass GUI work

The REA-observed current Archicad window (`ev_95c1c652b282ef9e446a3e7abc38fd65d51e1d8ff181f386aece828456b0cc75`)
showed the full Toolbox rail, two top toolbar bands, view tabs, Navigator map
icons, and the vertically stacked Properties area. Based on this, OpenArchi now
has separate Standard and Edit toolbar rows, a scrollable extended Toolbox,
four Navigator maps (Project Map, View Map, Layout Book, Publisher), searchable
Navigator header, and Info Box controls that differ between selection,
Marquee, Wall and Slab. Top-level menus were expanded into the observed core
families and common submenu groupings. Less common commands are clickable UI
previews; only commands already supported by the demo alter the project/model.

Do not reproduce Archicad/Graphisoft logos, icons, exact pixel-art assets,
private resources or source. OpenArchi uses original vector symbols, names and
colors while following common architectural CAD workflow patterns.
