# Escape command return

## Behavior source

The two-stage Escape behavior below is based on the user's requested workflow, not a runtime observation of Archicad. OpenArchi tracks its active modeling tool and one previous tool.

## Feature contract

- Goal: let the user stop the current modeling command and return to the last active command, then return to Select with another Escape.
- Entry: activate a tool or command from the toolbox, shortcut, menu, or preset.
- Interaction: Escape cancels unfinished drawing/placement, drag, or open command dialog, then restores the previous tool. A second Escape returns to Select. If the previous tool is already Select, the first Escape returns to Select. Escape in Select keeps the existing behavior of clearing the current selection, then removing a persistent Marquee.
- Effects: canceling an unfinished action does not commit it to the model or undo history. Canceling a select-tool drag restores its pre-drag project snapshot. Returning between tools clears their in-progress anchors and Polyline nodes through the shared `Editor::set_tool` path.
- History scope: one previous tool is remembered. The second Escape targets Select; this is not an arbitrary-depth command history.
- Exclusions: Escape does not undo a completed model operation; use Undo for that. Application quit and OS-level commands are outside this interaction.

## Acceptance checklist

1. Activate Wall, then Polyline; place an unfinished Polyline node. Press Escape and verify the draft is discarded and Wall is active.
2. Press Escape again and verify Select is active.
3. Repeat with a Wall start point, Slab corner, Stair placement, or Marquee polygon; Escape must discard the partial operation and restore the previous tool.
4. Start a plan drag and press Escape before release; the model returns to its pre-drag state.
5. Confirm canceled actions add no undo entry and completed actions remain undoable through the normal Undo command.

The implementation remembers one previous modeling tool. UI-only preview commands share the same tool path when available; this does not claim parity with every Archicad command or dialog.
