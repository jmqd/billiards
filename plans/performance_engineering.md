# Performance engineering

Date: 2026-07-02

## Goal

Make the billiards library substantially faster on the critical path from physics simulation to rendering a scene, without weakening physics correctness or scene determinism.

The current hot path is not primarily parser overhead. The largest observed costs are repeated exact-decimal conversions and allocations in hot physics/pocket geometry, expensive pocket-capture scanning, full pocket-aware event-cache rebuilds after each event, tiny heap allocations in fixed-size math, and repeated rendering work/asset decoding.

## Critical pipeline

Representative CLI path:

```text
parse_dsl_to_scenario
-> simulate_shot_trace_with_preferred_physics_on_table_until_event_limit/rest
-> simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit
-> PocketAwareEventCache::build / next_event / rebuild
-> ScenarioShotTrace::rendered_final_layout_with_trace_options
-> GameState::add_rendered_ball_path_styled
-> GameState::render_2d_diagram_with_options
-> diagram::render_scene_to_bytes
-> SvgBackend or PngBackend
```

Relevant files:

- `src/lib.rs` - physics core, event prediction, pocket-aware scheduling, trace rendering overlays.
- `src/dsl.rs` - DSL scenario trace bridge and final rendered layout construction.
- `src/diagram.rs` - scene representation and SVG/PNG backends.
- `src/drawing.rs` - raster drawing primitives.
- `src/assets.rs` - embedded table and ball image assets.
- `benches/physics.rs` - current Criterion point and end-to-end physics benchmarks.
- `benches/throughput.rs` - current batch/throughput benchmarks.
- `PERF.md` - performance workflow and benchmark inventory.

## Evidence gathered

Tools used from the available performance toolchain:

- `hyperfine` for CLI wall-time timing.
- `perf stat`, `perf record`, and `perf report` for CPU samples and counters.
- `heaptrack` and `heaptrack_print` for allocation counts and allocation stacks.
- `cargo llvm-lines` for generated-code footprint.
- `cargo bloat` for binary/code-size context.
- `cargo asm` for generated assembly around pocket capture.
- `just perf` through `nix develop` for the project Criterion suites.

Representative observations:

| Workload | Observed result |
| --- | ---: |
| `physics` bench: `compute_next_transition_on_table/sliding` | ~8.35 us |
| `physics` bench: `compute_next_ball_ball_collision_during_current_phases_on_table` | ~7.18 us |
| `physics` bench: `compute_next_ball_rail_impact_on_table` | ~7.76 us |
| `physics` bench: `trace_ball_path_with_rails_on_table/bank_duration_1s` | ~68.1 us |
| `physics` bench: `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8` | ~241 ms |
| `physics` bench: `direct/pocket_aware_until_rest_cached` | ~48.1 ms |
| `physics` bench: `direct/pocket_aware_until_rest_manual` | ~48.0 ms |
| CLI `three_ball_pinball` to SVG, 8 events | 311.4 ms +/- 11.1 ms |
| CLI `three_ball_pinball` to PNG, 8 events | 318.8 ms +/- 15.2 ms |
| CLI `three_ball_pinball` sparse trace render | 280.1 ms +/- 7.5 ms |
| Static table to SVG | ~1.8 ms |
| Static table to PNG | ~22.1 ms |

CPU profile facts from the CLI SVG path:

- One 8-event `three_ball_pinball` SVG render used roughly 990M cycles and 3.07B instructions.
- `std::io::Write::write_fmt` showed 24.48% children in `perf report --children`.
- `bigdecimal::...BigDecimalRef::to_f64` showed 18.86% children / 12.25% self.
- `num_bigint::biguint::convert::to_radix_le` showed 9.27% self.
- `malloc` showed 6.96% self.
- `cfree` showed 4.34% self.
- Pocket target math and trig-heavy functions were visible, including `slow_pocket_target_sleft_point`, `corner_pocket_slow_sleft_point_wall_wall`, `__sincos_fma`, `__sin_fma`, and `__cos_fma`.

Allocation profile facts from `heaptrack_print` on the same CLI SVG path:

- 5,525,334 allocation-function calls.
- 2,746,613 temporary allocations.
- 1.03 MiB peak heap; allocation count, not retained heap, is the problem.
- 1,845,542 allocation calls from `num_bigint::biguint::convert::to_radix_le`.
- 1,646,534 allocation calls from `bigdecimal` multiplication.
- Hot allocation stacks run through `pocket_capture_gap_during_current_phase_raw`, `compute_next_ball_pocket_capture_on_table`, `PocketAwareEventCache::build`, and `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`.

Static PNG render profile facts:

- `image::imageops::sample::resize`: 28.01%.
- `png::filter::filter_internal`: 26.76%.
- `fdeflate::decompress::Decompressor::read`: 10.12%.
- `png::filter::unfilter`: 9.12%.

Generated-code and assembly facts:

- `cargo llvm-lines --lib --release --filter ...` highlighted relevant generated-code-heavy functions:
  - `advance_to_next_n_ball_event_with_scheduler`: 1396 LLVM IR lines.
  - `compute_next_ball_pocket_capture_on_table`: 1124 LLVM IR lines.
  - `GameState::add_rendered_ball_path_styled`: 747 LLVM IR lines.
  - `PocketAwareEventCache::next_event`: 707 LLVM IR lines.
  - `trace_ball_path_with_rail_profile_on_table`: 551 LLVM IR lines.
- `cargo asm` for `compute_next_ball_pocket_capture_on_table` showed a 1464-byte stack frame, all-pocket loops, calls into pocket-gap/scan/raw-advance helpers, and a tiny heap allocation path for analytic candidate times.

## Ranked performance targets

### 1. Move hot physics/pocket/render geometry off `BigDecimal`

#### Target

Introduce cached raw numeric geometry/config structs for hot paths:

```rust
struct RawBallSpec {
    radius: f64,
}

struct RawMotionConfig {
    rest_linear_speed: f64,
    rest_vertical_speed: f64,
    rest_angular_speed: f64,
    sliding_acceleration: f64,
    rolling_deceleration: f64,
    spin_deceleration: f64,
}

struct RawTableGeometry {
    kind: TableKind,
    diamond_length: f64,
    width_in: f64,
    height_in: f64,
    rail_planes: RawRailPlanes,
    pockets: [RawPocketGeometry; 6],
}

struct RawPocketGeometry {
    pocket: Pocket,
    ty: PocketType,
    center_x: f64,
    center_y: f64,
    mouth_width: f64,
    slow_capture_radius: f64,
    jaw_geometry: [RawPocketJawGeometry; 2],
    entry_axis_x: f64,
    entry_axis_y: f64,
}
```

Keep the public `Inches`, `Diamond`, `Scale`, `TableSpec`, `BallSpec`, and DSL-facing API initially. Convert once at simulation/render setup, not inside every predicate and root search.

#### Why

The engine already has `RawOnTableBallState` for the same reason: hot physics math wants plain `f64`. Table, pocket, ball, and motion geometry need the same split.

Hot unit types are currently exact-decimal wrappers:

- `Diamond { magnitude: BigDecimal }`.
- `Scale { magnitude: BigDecimal }`.
- `Inches { magnitude: BigDecimal }`.
- `TableSpec::diamond_to_inches` and `inches_to_diamond` clone/divide/multiply `BigDecimal`.

The profiles show exact-decimal conversion and arithmetic in the critical path, not only at parse/setup boundaries.

#### Changes

- Add `RawTableGeometry::from_table_spec(&TableSpec)`.
- Add `RawBallSpec::from_ball_set(&BallSetPhysicsSpec)`.
- Add `RawMotionConfig::from_motion_config(&OnTableMotionConfig)`.
- Convert these functions first:
  - `compute_next_ball_pocket_capture_on_table`.
  - `compute_next_ball_jaw_impact_on_table`.
  - `compute_next_ball_rail_impact_on_table`.
  - `PocketAwareEventCache::{build, refresh_ball}`.
  - `trace_ball_path_with_rail_profile_on_table`.
  - `GameState::add_rendered_ball_path_styled` and projection helpers.
- Replace repeated helpers like:
  - `pocket_center_in_inches`.
  - `pocket_mouth_width_in_inches`.
  - `pocket_slow_capture_radius_in_inches`.
  - `pocket_jaw_geometry_in_inches`.
  - repeated `table.diamond_to_inches(Diamond::four/eight()).as_f64()`.

#### Expected effect

This should be the broadest win because it attacks both CPU samples and allocation counts across physics, pocket scheduling, and render projection. The current code pays exact-decimal costs in places that immediately convert to `f64`.

#### Verification

- Add Criterion comparisons for:
  - `compute_next_ball_pocket_capture_on_table`.
  - `compute_next_ball_jaw_impact_on_table`.
  - `compute_next_ball_rail_impact_on_table`.
  - `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`.
- Run focused correctness coverage:
  - `nix develop -c cargo test --test n_ball_pockets`.
  - `nix develop -c cargo test --test rail_event_scheduling`.
  - `nix develop -c cargo test --test rendering_geometry`.
  - `nix develop -c just perf`.
- Re-run `perf record/report` on the same `three_ball_pinball` CLI path.
- Re-run `heaptrack_print`; target: remove the BigDecimal/num_bigint allocation stacks from hot output.

### 2. Replace the fixed pocket-capture scan with gated analytic/bracketed capture

#### Target

Optimize `compute_next_ball_pocket_capture_on_table` and helpers:

- `pocket_capture_gap_during_current_phase_raw`.
- 60-iteration bisection refine.
- fixed 512-step scan.
- all-pockets capture prediction.

#### Current problem

For every live ball and every cache rebuild, the pocket predictor loops all 6 pockets. For each pocket, if analytic candidate times do not immediately validate, it runs a fixed 512-step scan. Each scan step calls `pocket_capture_gap_during_current_phase_raw`, which advances raw motion and recomputes target bounds, entry angle, lateral offset, mouth-plane gap, back-plane gap, and pocket acceptance.

The default scan is fixed size:

```rust
const POCKET_CAPTURE_SCAN_STEPS: usize = 512;
```

This fallback is too expensive to be the common path.

#### Evidence

- `heaptrack` showed repeated stacks through:
  - `pocket_target_bounds_in_inches`.
  - `pocket_capture_gap_during_current_phase_raw`.
  - `scan_ball_pocket_capture_time_during_current_phase_raw`.
  - `compute_next_ball_pocket_capture_on_table`.
  - `PocketAwareEventCache::build`.
- `perf report` showed pocket target math and trig functions.
- `cargo asm` confirmed all-pocket loops, pocket-gap calls, scan/refine calls, and tiny candidate allocation/sort/dealloc paths.
- End-to-end pocket-aware benchmarks dwarf the core microbenchmarks.

#### Changes

- Precompute `RawPocketGeometry` once.
- Add cheap broad-phase rejections before expensive gap checks:
  - pocket is behind velocity and acceleration cannot reverse before horizon;
  - swept AABB over current phase cannot intersect a padded pocket/mouth region;
  - closest approach to pocket center is outside slow capture radius plus tolerance;
  - mouth plane crossing is outside horizon or wrong direction;
  - side/corner pocket acceptance is impossible for the current signed entry direction.
- Keep analytic candidate roots:
  - radial capture circle entry;
  - mouth plane crossing;
  - back plane crossing;
  - side-pocket centerline crossings where applicable.
- Only run bisection around a proven sign change or a narrow analytic window.
- Make the 512-step scan a rare fallback covered by regression tests, not the default path.
- Preserve `PredictedBallPocketCapture` semantics.

#### Expected effect

This is likely the largest single physics algorithm win for pocket-aware scenes. The 8-event CLI run and heap profile show pocket capture doing large repeated work even when no ball is pocketed.

#### Verification

- Keep/expand:
  - `tests/n_ball_pockets.rs`.
  - pocket mouth tests embedded in `src/lib.rs`.
  - `tests/scenario_examples.rs` for real scenarios.
- Add targeted tests for:
  - grazing side-pocket acceptance;
  - late-drop jaw/capture;
  - high-speed corner rejection;
  - mouth/back plane order edge cases.
- Benchmark:
  - `direct/pocket_aware_until_rest_cached`.
  - `dsl/preparsed_three_ball_pinball_event_limit_8`.
  - new `compute_next_ball_pocket_capture_on_table` microbench.

### 3. Make `PocketAwareEventCache` incremental and array-backed

#### Target

- `PocketAwareEventCache`.
- Pocket-aware simulation loop.
- Manual scheduler path.

#### Current problem

The cached scheduler still rebuilds the whole cache after every resolved event. That rebuild recomputes:

- all ball-ball pairs;
- every ball's jaw impact;
- every ball's pocket capture;
- every ball's rail impact;
- every ball's motion transition.

The cache currently stores ball-ball candidates in a `HashMap<(usize, usize), PredictedBallBallCollision>`. For small dense ball sets, a pair-indexed `Vec<Option<_>>` is cheaper and deterministic.

#### Evidence

- Criterion showed no useful win from the current cache:
  - `direct/pocket_aware_until_rest_cached`: ~48.101 ms.
  - `direct/pocket_aware_until_rest_manual`: ~47.996 ms.
- Heap/profile stacks repeatedly hit `PocketAwareEventCache::build`.
- `PocketAwareEventCache::refresh_ball` recomputes pocket/jaw/rail/transition and scans all other balls.

#### Changes

- Replace `HashMap<(usize, usize), ...>` with dense pair storage:

```rust
struct PairIndex {
    n: usize,
}

impl PairIndex {
    fn index(&self, i: usize, j: usize) -> usize {
        // deterministic upper-triangle mapping
    }
}
```

- Store event times as absolute simulation time or maintain a cache epoch.
- After each event:
  - ball-ball collision: refresh the two participants and all pairs involving them;
  - shared contact: refresh all involved balls and pairs involving them;
  - rail impact: refresh impacted ball and pairs involving it;
  - jaw impact: refresh impacted ball and pairs involving it;
  - pocket capture: mark ball pocketed, clear its row/column, refresh affected event selection;
  - motion transition: refresh that ball and pairs involving it.
- Keep a best-candidate index per category or a small binary heap keyed by event time.
- Build full cache once at simulation start; rebuild all only after events that can globally invalidate assumptions.
- Preserve simultaneous-event tolerance behavior.

#### Expected effect

This should turn pocket-aware simulation from full rescan after each event into local repair after each event. It compounds with raw geometry and pocket-capture improvements.

#### Verification

- Run:
  - `tests/n_ball_events.rs`.
  - `tests/n_ball_advance.rs`.
  - `tests/n_ball_simulation.rs`.
  - `tests/n_ball_pockets.rs`.
- Bench cached vs manual; cached should start winning.
- Add event-count-scaled benchmarks: 2, 4, 9, and 15 balls with pockets.
- Profile target:
  - `PocketAwareEventCache::build` should largely disappear after the first event.
  - `HashMap` allocation/removal/retain stacks should disappear.

### 4. Remove tiny heap allocations from roots, candidates, wrappers, and render samples

#### Target

Broad hot allocation sites:

- `real_roots_quadratic`.
- `real_roots_cubic`.
- pocket analytic candidates.
- scheduler candidate vectors.
- N-ball advance wrappers.
- two-ball wrappers and conversion paths.
- render path sampled points.
- `GameState::to_diagram_scene` clone/collect path.

#### Current problem

The engine allocates for fixed-size data:

- quadratic roots: max 2 roots;
- cubic roots: max 3 roots;
- analytic pocket entries: max 3 entries;
- rail roots: max 2 roots per rail;
- scheduler candidates can be reduced to best-so-far in the common case;
- `state_refs = states.iter().collect::<Vec<_>>()` allocates every advance step;
- two-ball wrappers clone to temporary slices/Vecs;
- render sampled points are rebuilt per segment and sometimes immediately split into per-edge overlays.

#### Evidence

- `heaptrack`: 405,780 calls through `alloc::raw_vec::finish_grow`.
- `cargo asm` for pocket capture shows dynamic allocation of a tiny `Vec<f64>` for candidate times and sort/dealloc paths.
- Root helpers return `Vec<f64>` for fixed-size root sets.
- Scheduler selection builds temporary candidate vectors before reducing.

#### Changes

Introduce fixed-capacity return types:

```rust
#[derive(Clone, Copy)]
struct Roots<const N: usize> {
    len: u8,
    values: [f64; N],
}
```

or explicit root structs:

```rust
struct QuadraticRoots {
    len: u8,
    values: [f64; 2],
}

struct CubicRoots {
    len: u8,
    values: [f64; 3],
}
```

Then:

- iterate roots by slice over `values[..len]`;
- sort 2 or 3 elements with branch swaps, not general slice sort;
- replace `analytic_entries.collect::<Vec<_>>()` with a tiny fixed buffer;
- in scheduler selection, track best candidate in one pass and only build shared-contact scratch if a ball-ball collision is near the earliest time;
- specialize two-ball simulation paths to avoid N-ball Vec conversion;
- allow caller-provided scratch buffers in N-ball simulation for state advancement and shared-contact resolution.

#### Expected effect

This will reduce allocator pressure and make later profiles easier to read. It should improve the broad pipeline modestly by itself and compound with the algorithmic changes.

#### Verification

- `heaptrack_print` on the same CLI path should show a large reduction in `alloc::raw_vec::finish_grow`.
- `cargo asm` for `compute_next_ball_pocket_capture_on_table` should no longer show tiny allocation/sort/dealloc for analytic entries.
- Run:
  - `tests/ball_collision_timing.rs`.
  - `tests/n_ball_pockets.rs`.
  - `tests/rail_event_scheduling.rs`.
- Run `nix develop -c just perf`.

### 5. Make trace-to-scene rendering one-pass and cache raster assets

#### Target

Trace/scene/render path:

- `ScenarioShotTrace::rendered_final_layout_with_trace_options`.
- `GameState::sampled_points_for_ball_path_segment`.
- `GameState::add_heading_chevrons_for_ball_path_segment`.
- `GameState::add_rendered_ball_path_styled`.
- `GameState::to_diagram_scene`.
- PNG backend.
- asset helper.
- raster drawing primitives.

#### Current problem

Rendering repeats work after physics:

1. `add_rendered_ball_path_styled` samples segment points.
2. In `ScaleBySpeed`, it re-advances physics twice per point-window to compute segment speed.
3. Heading chevrons re-advance again at separate sample points.
4. `GameState::to_diagram_scene` clones the whole `GameState`, resolves positions, clones balls, clones overlays, and clones polyline point Vecs.
5. PNG rendering decodes the table PNG every render.
6. `draw_raster_balls` calls `assets::ball_img(ball.ty.clone())`, gets a fresh `Vec<u8>`, then decodes and resizes every ball sprite every render.

#### Evidence

- CLI dense default SVG render: 311.4 ms.
- Same scenario with fewer render extras and larger sample step: 280.1 ms.
- Comparable physics-only Criterion case: ~241 ms.
- Static SVG table: ~1.8 ms.
- Static PNG table: ~22.1 ms.
- Static PNG profile shows expensive resize, PNG filter encode, PNG decode, and PNG unfilter work.

#### Changes

Build a single sampled trace representation:

```rust
struct RenderedPathSample {
    position: Position,
    speed_ips: f64,
    phase: MotionPhase,
    elapsed: Seconds,
}

struct RenderedPathSegment {
    samples: Vec<RenderedPathSample>,
    event_marker_at_end: bool,
    event_marker_label: Option<String>,
}
```

Use this once for:

- polyline points;
- speed-scaled width;
- fade-by-time color;
- motion-phase color;
- heading chevrons;
- labels/markers.

Pre-reserve and avoid clone-heavy scene construction:

- Precompute sample counts and reserve `Vec` capacity.
- Reserve `GameState.lines_to_draw` for expected overlays.
- Change `to_diagram_scene` to avoid cloning `self` just to call `resolve_positions`.
- Avoid cloning point Vecs where the backend can borrow.

Cache raster assets:

- Change `assets::ball_img(ball)` from `Vec<u8>` to `&'static [u8]` immediately.
- Add decoded `OnceLock` assets:
  - table RGBA;
  - ball sprite RGBA per `BallType`;
  - resized ball sprite per diameter.
- For repeated library renders, expose a reusable `DiagramRenderer` or `PngRendererCache`.
- Consider restricting `image` crate features to the required codecs for embedded assets.

#### Expected effect

This is the main rendering-side target. It matters less than pocket simulation for the profiled SVG scenario, but it matters for PNG output, repeated diagrams, and rich traces.

#### Verification

There is no dedicated Criterion render bench today. Add one before changing this path:

- static table SVG/PNG;
- 9-ball layout SVG/PNG;
- `three_ball_pinball` trace-to-scene only;
- `three_ball_pinball` full render SVG/PNG;
- rich overlays with labels/markers/ghosts/chevrons.

Run:

- `nix develop -c cargo test --test rendering_geometry`.
- `nix develop -c cargo test --test dsl`.
- `nix develop -c cargo test --test scenario_examples`.
- `hyperfine` CLI SVG/PNG commands from the profile pass.
- `perf record/report` for static PNG.
- `heaptrack_print` for rich SVG trace render.

## Implementation order

1. Add the missing render Criterion bench first. Current `physics`/`throughput` suites do not cover `render_2d_diagram_with_options`.
2. Implement raw cached geometry/config.
3. Replace pocket capture scan/gating.
4. Make `PocketAwareEventCache` incremental and array-backed.
5. Remove tiny root/candidate/wrapper allocations after the APIs settle.
6. Optimize render sampling/assets.

## Non-goals for the first pass

- Do not start with compiler flags, PGO, or BOLT. The current profiles show algorithmic and allocation costs first.
- Do not weaken pocket-capture correctness to win a benchmark.
- Do not introduce compatibility shims for old internal APIs. This repository does not prioritize backwards compatibility; prefer clean in-tree refactors and update call sites in the same change.
- Do not add mocks. Performance verification should use real scenarios and real render paths.

## Follow-up tooling after code changes

After the algorithmic work stabilizes, evaluate:

- PGO/BOLT with the same CLI scenario set.
- `llvm-mca` or deeper `cargo asm` only for remaining numeric kernels.
- `cargo bloat` after restricting image features and asset decode paths.
- Full `nix develop -c just perf` before claiming net wins.
