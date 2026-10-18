# Performance baseline

Date: 2026-10-18
Machine: Apple Silicon, macOS 15.7.5, Rust 1.98.1 (release build)
Entry point:

```sh
./scripts/perf-baseline.sh
# or subset:
OPENARCHI_PERF_SIZES=1000,10000 ./scripts/perf-baseline.sh
```

The harness is an ignored integration test (`tests/performance_baseline.rs`).
It is a local comparison baseline, not a supported performance promise.

## Method

- Create 1,000 / 10,000 / 50,000 straight plaster walls on story 0.
- Select up to 1,000 elements, move them once, undo once, then redo once. The
  10k/50k cases deliberately keep the selection at 1,000 so the number measures
  validation and history cost rather than an O(n²) selection scan.
- The move path uses `Editor::apply(ModelCommand::MoveElements)` and records
  before/after copies of the 1,000 affected elements only. The history snapshot
  count printed by the harness is the sum of those element copies.
- Mesh preparation proxy: `plan_wall_footprints` + `wall_outline_paths` +
  `geometry::corners` + `triangulate_polygon`. GPU upload and rendering are not
  included.
- Pick: one `geometry::hit_plan` call on the first wall.

## Recorded results

### Snapshot undo (previous baseline, for comparison)

Release build, one run on 2026-10-18. Times are milliseconds. Selection is
1,000 elements for every size; undo was a whole-project snapshot swap and
therefore ~0 ms, while each step retained a complete project clone.

| Walls | Create | Move 1k | Undo | Mesh prep | Footprints | Wall outlines | Corners | Triangulate | Pick |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1,000 | 0.34 | 0.60 | 0.00 | 64.75 | 2.61 | 62.02 | 0.07 | 0.05 | 0.00 |
| 10,000 | 3.41 | 6.48 | 0.00 | 5,785.48 | 422.23 | 5,361.87 | 0.90 | 0.48 | 0.07 |
| 50,000 | 16.86 | 36.59 | 0.00 | 155,493.35 | 22,219.19 | 133,267.78 | 3.90 | 2.49 | 0.83 |

Full 50k run duration: about 161 seconds.

### Reversible-delta undo (current)

Same machine, recorded after `ModelCommand` / `CommandRecord` and
`ProjectDelta` were introduced. `Move` and `Delete` now go through
`Editor::apply`; other operations still record a diff through `commit(before)`.

| Walls | Create | Move 1k | Undo | Redo | History element snapshots | Mesh prep | Footprints | Wall outlines |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1,000 | 0.34 | 1.04 | 0.41 | 0.41 | 2,000 | 63.28 | 2.50 | 60.70 |
| 10,000 | 3.63 | 3.09 | 0.67 | 0.62 | 2,000 | 5,809.89 | 422.47 | 5,385.85 |
| 50,000 | 15.69 | 12.88 | 2.80 | 1.56 | 2,000 | 153,222.48 | 20,713.76 | 132,502.70 |

The 1,000-element move/undo/redo path stays under 3 ms at 50k walls. A
previous whole-project snapshot step retained 50,000 elements at this size;
the delta retains 2,000 copies (before and after for the 1,000 moved
elements), an approximately 25× reduction in retained element copies per step
before accounting for the 100-step cap.

## What the baseline says

- Delta move cost at 50k dropped from about 37 ms to about 13 ms for a
  1,000-element selection, including full validation. Undo/redo are under 3 ms.
- Wall-outline preparation is superlinear: about 62 ms for 1k, 5.4 s for 10k
  and 133 s for 50k. Footprint preparation grows with it (22 s at 50k).
- Triangulation and picking are negligible at these sizes.
- Wall-outline preparation remains the dominant 50k cost and is unchanged by
  the undo work. It still needs profiling before any cache or spatial index is
  introduced.
- No cache or index was added in this change; these numbers are the comparison
  point for the next stage.
