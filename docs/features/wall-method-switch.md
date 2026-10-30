# Active wall construction-method switching — 2026-10-29

User goal: change Single/Continuous (and existing Rectangular/Polygonal) in the Info Box while drawing without restarting the command.

Evidence: readable source: `info_box_wall_tool_properties` explicitly canceled the draft after changing method. `Editor::finish_wall` commits all draft nodes as one undo transaction. Archicad 29 exact mixed-method transition semantics remain unknown; this change implements the requested OpenArchi behavior, not a claim of runtime equivalence.

Contract / reversible implementation choice: retain the anchor and all accepted chain nodes. Switching itself changes defaults/preview only, with no model or undo entry. Single finishes the retained prefix plus one last segment; Rectangular finishes the retained prefix plus a rectangle from the last anchor. Switching back to Continuous resumes the prefix. Polygonal closes the retained nodes using the existing closed-chain behavior (not Archicad solid PolyWall). Incompatible numeric tracker locks reset on method change. Confirm commits one transaction; Escape discards the entire uncommitted draft. Completed model elements remain unchanged. Settings dialog and Info Box share the same transition. Existing labels are unchanged.

Acceptance: start Single, switch Continuous, add nodes, switch Single and finish without losing nodes; switch Rectangular and inspect prefix+rectangle preview; switching alone has no model/undo effects; cancel removes draft; undo/redo the complete result. Exact AC29 mid-draft semantics and other tools are outside this slice.

## Verification — 2026-10-29

`python3 scripts/ac29-workflow.py verify --force` passed: 350 Rust tests, one ignored performance test, 15 tooling tests; formatting, all-target check, localization and diff checks passed. Fresh `dist/OpenArchi.app` release/bundle arm64 UUID `512DBB7E-75EA-3F49-BCA9-03FA867FBF06`; strict deep codesign passed. Receipt: `target/ac29-workflow/runs/20261028T171643Z-903a89aa/report.json`. Final `bundle` freshness recheck passed.

New regressions: `active_wall_method_switch_retains_nodes_through_info_box_clicks` exercises full-workspace egui method-button events, retained nodes, exact preview/committed endpoints, and undo/redo. `active_wall_method_switch_rectangle_and_cancel_preserve_transaction_boundary` covers retained prefix + four rectangle edges, single undo and cancellation.

Native smoke on final package: isolated HOME `target/wall-method-home`, PID 6999 (closed after verification), existing user PID 37444 untouched. Started wall with W; clicked (600,650), AXPressed Continuous, clicked (750,650), AXPressed Single, moved/clicked (750,750). Preview retained both segments; elements 12→14; Cmd+Z returned 12. Repeated in Chinese through native View > Language; elements 12→14; undo returned 12; repeated partial input and Escape canceled with 12 remaining. Restored English. Evidence: `docs/features/wall-method-evidence/` screenshot and AX dumps. No labels added/edited. Language restart persistence was not rerun; the title-bar selector remains absent by prior user request. Settings-dialog route shares default application code but was not manually exercised. Rectangle/Polygonal mixed transitions have no current desktop comparison with AC29; arbitrary other commands are not claimed supported.

Reusable lesson (confirmed for persistence by the user’s 2026-10-29 follow-up; now recorded in AGENTS.md under “Active-command construction-method transitions”): changing a construction method should transition pending input state, retain accepted nodes, and reset incompatible numeric locks; test preview and final geometry together, and verify switching alone leaves the model/undo history unchanged.
