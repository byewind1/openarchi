# OpenArchi roadmap

## Vision

Build a lightweight BIM workbench for the AI era: fast to iterate on, native on the desktop, and familiar to architects through workflows benchmarked against Archicad. Architects should be able to work directly in the model; AI should accelerate deliberate work rather than hide how the model changed.

The project is independent software. It uses its own code, domain model, interface components, icons, and assets. Archicad is a reference for observable workflow and interaction expectations.

## Product principles

1. **The model remains editable and inspectable.** AI and UI actions use the same explicit operations over the same project model.
2. **Changes are reviewable.** AI-generated changes should appear as proposals or previews, validate before commit, and participate in undo/redo.
3. **Fast feedback matters.** Keep build, test, and interaction loops short; prefer small increments with visible behavior.
4. **Architects stay in control.** Numeric input, direct manipulation, selection, properties, and undo remain first-class.
5. **Interoperability is an adapter boundary.** The native model is not identified with IFC, PLN, GSM, or another file format.
6. **Evidence beats imitation by assumption.** Reference behavior from authorized runtime observation or public documentation; mark unknown behavior as unknown.

## Staged direction

### 1. Reliable native prototype

- Keep the macOS Apple Silicon application easy to build and run.
- Stabilize the core create/select/edit/save/undo workflow for walls and slabs.
- Improve junction behavior and geometry correctness with focused regression models.
- Finish bilingual UI coverage and make window close/save behavior dependable.
- Add automated build and interaction checks for every change.

### 2. A small, dependable BIM core

- Separate domain operations from UI event handling and rendering.
- Add typed, validated model commands with stable IDs and meaningful undo history.
- Grow architectural primitives only alongside plan, 3D, property, persistence, and undo behavior.
- Measure project size and responsiveness before adding caching or more complex geometry.

### 3. AI-assisted iteration

- Expose a narrow, typed operation interface over the independent model.
- Let AI read relevant project context and propose bounded edits, with human preview and approval.
- Return structured results, validation issues, and affected element IDs so changes can be inspected and undone.
- Keep the core usable without a network connection or an AI provider; integrations remain optional.

### 4. Interoperability and extension

- Define versioned import/export adapters and a documented extension boundary.
- Evaluate open formats and APIs when implementation begins; do not imply current support for IFC, PLN, GSM, or BSM.
- Build compatibility tests from lawful, user-provided or openly licensed fixtures.

## Current status

The current build is an early native prototype. It has basic wall and slab creation, synchronized plan and perspective views, selection, properties, snapping, undo/redo, Marquee clipping, and temporary JSON persistence. See the README's implemented and limitation sections for exact scope. This roadmap records direction, not delivery dates or a promise of feature parity.
