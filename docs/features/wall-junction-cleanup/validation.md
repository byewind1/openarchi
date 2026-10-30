# Junction cleanup and palette feedback — 2026-10-29

## Scope and evidence

See [behavior contract](contract.md). AC29 INT 5101 was exercised in a separate copied project on API port 19724. Created and measured centered Y, T, moved T, Fan4, X, plus unequal-width Y, narrow fan and Inside-reference Y. Captured generated floor-plan primitive polygons through Tapir and actual native AX Zoom to Selection/screenshots: [moved T](archicad-moved-t.png), [Fan4](archicad-fan4.png). The Fan4 screenshot visibly retains AC's reverse miter spur. Original user AC PID 51530/project was untouched; our copy was saved to `target/ac29-junction-probe.pln` and only our PID 35645 was quit through AX. Our REA target session was closed; failed native REA capture supplied no Evidence ID.

[Measured union boundaries](measured-perimeters.txt) and [reproducible analysis](compare-observed-perimeters.py) document where the angular-sector hypothesis agrees and differs. Production regression tests additionally exercise bounded sectors. This is bounded behavior research, not recovered Archicad code or universal equivalence.

## Implementation

Active icon background gray 24 / border 170; hover candidate gray 42 / border 100; normal uses the palette background. Browsing preserves the active command/staged points/numeric input. Click or return to canvas adopts the candidate; a held drag may return and release in the same frame. Keyboard cycling clears a stale hover candidate.

Three or more shared finite straight-wall endpoint rays are sorted by angle. Each sector intersects the two adjacent actual body faces; bounded bevels handle short/near-parallel cases and a center cap maintains coverage. Story, nonzero layer group and 1 mm endpoint tolerance bound membership. Two-wall and branch-to-through joins retain established rules. Plan fills, selected highlights, Trace, private editing preview, and ordinary selected/unselected 3D consume the same arbitrary polygons. Concave caps use triangulation, and exposed-outline clipping respects concavity. 3D merge groups use actual bottom/top levels including offsets.

## Limits

Reference lines entirely outside a wall body and duplicate same-direction rays use conservative original caps, avoiding invented bridges across an offset gap. Material-priority/explicit Junction Order reconstruction, composite/profiled/curved joins and arbitrary non-endpoint multi-way crossings are outside this slice. Different heights retain closed individual 3D caps; exact heterogeneous intersection subtraction is not implemented.

Walls containing openings keep their existing 3D pier/lintel path; Marquee, outside-Marquee and selection-only 3D keep their existing clipping/individual paths. Those specialized paths do not yet use the new junction extrusion. Ordinary plan and selected/unselected 3D are the acceptance scope.

## Automated coverage

Nine geometry regressions cover measured Y/T/X unions, deformed T, four-ray fan, all 24 input permutations, direction reversal, widths/reference sides/offsets, strict story/group/node membership, concave outline clipping, ordinary two-wall L/T/face-join equivalence, two-ended 0.1 m short-wall caps and 42.3° rotation. Three renderer tests verify arbitrary concave prism coverage/all vertices, selected/mixed-material junction geometry, and actual vertical offsets. Palette event regressions exercise all three colors, browsing/click/return adoption, internal gaps, stage/numeric preservation, keyboard cycling and held-drag release; a plan painter test ensures the concave notch stays empty.

The first full run caught an existing UI test asserting that selection fills must be an egui Path. The painter now emits a triangulated Mesh, so that test was updated to assert nonempty selection-colored triangles while retaining its reference-line/handle checks. No assertion about behavior was removed.

## Final checks and native verification

Final app `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **ECA2334E-2A55-32E6-A959-B0D0A70DD16A**; deep/strict signature **passed**. Forced receipt `target/ac29-workflow/runs/20261029T025513Z-71e440bb/report.json`: **428 Rust tests (380 library + 48 integration), 15 tooling tests passed**, one existing performance case ignored. Format including included files, all-target compile, localization checks, diff and fresh packaging all passed.

Native attempt: isolated HOME `target/wall-junction-cleanup-home`, PID 81807 / window 43250, bounds (204,1,1512,988); process HOME confirmed before input. The Mac was locked (`CGSSessionScreenIsLocked=Yes`, foreground loginwindow PID 420), so the foreground-guarded input helper refused delivery. Window capture was blank and AX contained only menu structure. See [blocker](native-blocker.txt), [capture](blocked-startup.png), [AX](blocked-startup-en-ax.txt). These are failed-startup evidence, not UI acceptance.

**Native final-build plan/3D, hover/click/drag, Undo/Redo, EN→ZH→EN and Chinese restart are unverified due to the locked desktop.** Automated event/model/renderer tests establish the recorded behavior, but do not replace that verification. The title-bar language selector remains unavailable under the existing native-title product choice; View > Language could not be exercised in this locked session. No new user-visible strings were introduced; prior screenshots from older bundles are not reused as acceptance for this build.

After recording the blocker, owned test PID 81807 was terminated; no model edits were performed. Original user processes were not closed. Final source/bundle freshness recheck passed.
