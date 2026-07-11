# Build owned diagram scenes directly from game state

## Status

- **Status:** Accepted implementation plan; not implemented.
- **Priority:** Medium-high for traced diagrams with long owned overlays; lower for small static layouts.
- **Confidence:** High that the current implementation performs a redundant full clone and high that it can be removed without changing the public ownership model. The committed isolated scene case establishes a roughly 150.75 us cost; the candidate delta remains benchmark-gated.
- **Dependencies/order:** None. Retain the committed `render_stages/scene_build/rich_trace` case, add the remaining boundary fixtures and ownership/equivalence tests, and save a baseline from the immediate parent. This plan is independently committable and does not depend on streamed SVG writes or PNG asset caches.

## Problem and evidence

`GameState::to_diagram_scene` in `src/lib.rs` currently does this:

1. `let mut resolved = self.clone()`. `GameState` is `Clone` and owns `table_spec`, `ball_positions`, game metadata, and `lines_to_draw`; the clone duplicates all of them.
2. `resolved.resolve_positions()` iterates only `ball_positions` and calls `Position::resolve_shifts`. It does not modify overlays, game type, cueball modifier, or table geometry.
3. The method iterates `resolved.ball_positions` and clones every ball's type, position and spec into a new `Vec<DiagramBall>`.
4. It iterates `resolved.lines_to_draw` and clones every overlay into a new `Vec<DiagramElement>`. `Overlay::SmoothPolyline` owns a `Vec<Position>`; circle markers own optional label/title strings; text labels own strings. These nested allocations were already cloned into `resolved`, so they are allocated/copied a second time.
5. Only `resolved.table_spec` is moved into the returned scene. The temporary cloned ball/overlay vectors and all their first-generation nested allocations are dropped.

`Position::resolve_shifts` only applies pending inch shifts to the cloned position and clears its two pending fields. Therefore the required behavior can be obtained by cloning and resolving each ball position as that `DiagramBall` is built. Overlay-builder methods already resolve overlay positions when they are inserted. More importantly, current `to_diagram_scene` never calls a resolver over overlays, so direct overlay mapping must preserve them exactly as stored rather than add new resolution behavior.

The committed `render_stages/scene_build/rich_trace` case constructs the traced `GameState` and render options before `b.iter`, times only `to_diagram_scene`, and black-boxes/drops the complete owned result. Its quick unchanged-tree baseline is approximately **150.75 us** per scene. The current setup validates associated backend outputs only as nonempty and does not assert stable overlay counts/layers, ownership, or exact equivalence; those gates remain below. Its scale-by-speed trace consists of many short two-point `SmoothPolyline` elements; the separate 1,000-point state is used only by the SVG backend benchmark.

The older `throughput_rendering/trace_final_layout_svg` case remains combined traced-layout + scene + SVG work at approximately 18.5 ms. Its recorded profile is dominated by trace sampling, so it cannot attribute a scene-construction speedup.

## Goal and observable contract

Construct the same fully owned `DiagramScene` directly from `&GameState`:

- clone, resolve and store each ball position exactly once;
- copy each ball type/spec once into the final ball vector;
- clone every overlay directly into the final element vector once;
- clone the table spec once into the final scene;
- preserve viewport/background, ordering, layer semantics and all rendered bytes;
- keep the returned scene independent of the source state and valid after the source is mutated or dropped.

Do not introduce borrowed scene lifetimes. The existing owned `DiagramScene` contract is intentional and remains public behavior.

## Scope

### In scope

- `src/lib.rs`: `GameState::to_diagram_scene`, a small private overlay-to-element conversion helper if it improves clarity, and a same-module test that can construct a private pending-shift `Overlay`.
- `benches/throughput.rs`: retain `render_stages/scene_build/rich_trace`, strengthen its setup assertions without changing its workload, and add the missing static, overlay-only, and empty scene-construction fixtures plus adjacent rendering guards.
- `tests/rendering_geometry.rs`: position-resolution, global/per-layer ordering, exact backend equivalence and owned-scene lifetime/mutation tests.
- Optional test/benchmark-only allocation instrumentation, provided it adds no production runtime dependency.

### Explicit non-goals

- SVG serialization, report JSON, PNG decoding/encoding, sprite/table caches, or raster algorithms.
- Changing `DiagramScene`, `DiagramBall`, or `DiagramElement` to borrow `GameState` data.
- Adding `Cow`, reference-counting, arenas, interners or shared mutable overlay storage.
- Changing `GameState::resolve_positions`, overlay insertion behavior, physics, table geometry, ball specs, viewport defaults or layer order.
- Resolving overlay positions in `to_diagram_scene`; that would be a behavior change relative to current code.
- Compatibility shims, a second scene-builder API, or retaining the clone-based path.

## Implementation design

### 1. Establish a pre-change equivalence oracle

Before editing `to_diagram_scene`, add deterministic integration fixtures that exercise:

1. Mixed already-resolved and pending X/Y inch shifts, including both axes on one ball and a shift whose result rounds near an SVG `.3` boundary.
2. A deterministic 16-ball pool layout with varied `BallType` and custom `BallSpec` radii.
3. A carom table with carom ball specs.
4. Every overlay variant, both layers, a long smooth polyline, event label/title, text label and spin glyph.
5. Empty state, balls-only state, overlays-only state and rich traced state.

Capture the current exact SVG bytes and current PNG output for each applicable fixture. For PNG, compare both encoded bytes and decoded dimensions/RGBA bytes; decoded RGBA is authoritative if an unrelated encoder version ever changes. Keep fixtures deterministic and free of simulation inside the assertion itself.

Because every public overlay insertion API resolves pending inch shifts before storage, add one same-module `src/lib.rs` unit test that directly pushes a private `Overlay` containing a pending-shift `Position`. Build the scene and assert the corresponding `DiagramElement` position equals the stored pending position exactly. Do not pretend an integration fixture using only public builders can exercise this invariant.

### 2. Build balls directly

Replace the `self.clone()`/`resolved.resolve_positions()` path with a final vector allocated from the exact source length:

```text
let mut balls = Vec::with_capacity(self.ball_positions.len());
for ball in &self.ball_positions {
    let mut position = ball.position.clone();
    position.resolve_shifts(&self.table_spec);
    balls.push(DiagramBall {
        ty: ball.ty.clone(),
        position,
        spec: ball.spec.clone(),
    });
}
```

An equivalent `map(...).collect::<Vec<_>>()` is acceptable only if release allocation evidence confirms the exact-size iterator creates one allocation. The explicit `with_capacity` loop makes the invariant obvious and guarantees no vector growth.

Resolve only the new cloned `Position`; do not mutate `self`. Pass the original `self.table_spec` by reference during resolution, then clone that table spec once for the returned scene. Ball order must remain source order.

Do not call `self.resolve_positions()` or clone a `Ball` wholesale: only type, position and spec belong in `DiagramBall`.

### 3. Clone overlays directly once

Allocate `elements` with `Vec::with_capacity(self.lines_to_draw.len())` and match over `&self.lines_to_draw` in source order. Construct the corresponding owned `DiagramElement` directly. Preserve the current field-by-field behavior exactly:

- clone dashed-line endpoints/style;
- clone each smooth-polyline point vector/style once;
- copy `Angle` for heading chevrons and clone its position/style;
- clone ghost/origin centers and styles;
- clone event label/title options once;
- clone text and anchor once;
- clone spin center, angular/linear velocity, radius and style once.

A small private `diagram_element_from_overlay(&Overlay) -> DiagramElement` helper is acceptable because both types are local, but only if it keeps the exhaustive mapping explicit and is used solely by this builder. Do not add a public conversion trait, a generic abstraction, or a duplicate mapping path.

Crucially, do not call `resolve_shifts` on overlay fields. The current full-state clone followed by `GameState::resolve_positions` resolves balls only. Adding overlay resolution here would silently change observable geometry.

### 4. Construct the scene with one table clone

Return:

```text
DiagramScene {
    table_spec: self.table_spec.clone(),
    viewport: DiagramViewport::default(),
    background: options.background,
    balls,
    elements,
}
```

`options.scale_factor` remains backend input and must not enter scene construction. `background` continues to be copied from options. Do not copy `GameState::ty` or `cueball_modifier`; they are not part of the current scene.

### 5. Clean cutover

Delete `let mut resolved = self.clone()`, the call to `resolved.resolve_positions()`, and every read from `resolved`. Leave no legacy helper, feature flag, clone-based fallback, alternate public method or deprecation alias. Retain `GameState: Clone`; other callers use it and removing it is outside scope.

## Benchmark plan

### Coverage already committed

Retain `render_stages/scene_build/rich_trace` in `benches/throughput.rs`. Its source `GameState`, trace-derived overlays, and `DiagramRenderOptions` are already built before `b.iter`; the closure calls only:

```text
black_box(black_box(&state).to_diagram_scene(black_box(&render_options)))
```

The returned owned scene is the complete output and is dropped normally in each iteration. The quick baseline is approximately 150.75 us, but acceptance still requires a saved immediate-parent baseline and paired runs. Before production edits, strengthen fixture setup to record and assert stable ball/element counts, both overlay layers, event titles/labels, spin glyphs, the number of smooth-polylines, and their total point count. Do not require a long polyline or otherwise change the committed workload; the current nonempty backend controls are not equivalence goldens.

### Remaining matrix extensions

Add these cases to the existing `render_stages/scene_build` group:

- `render_stages/scene_build/static_16_balls_shifted`
- `render_stages/scene_build/overlays_only_1000_points`
- `render_stages/scene_build/empty`

Fixture definitions:

- `static_16_balls_shifted`: 16 balls, deterministic types/specs, with a mix of no pending shift, X-only, Y-only and X+Y pending shifts. This measures required per-ball resolution and final ownership without overlay noise.
- The committed `rich_trace`: materialize the current traced final layout **once before timing** with both overlay layers, motion-phase path color, event markers/titles and spin glyphs. Record and assert its unchanged count of short smooth-polylines and total points; do not import the separate 1,000-point backend fixture or reuse the 150.75 us baseline after changing workload shape.
- `overlays_only_1000_points`: no balls and one deterministic 1,000-point smooth polyline plus event/text strings. This isolates redundant nested overlay cloning.
- `empty`: empty state guard; it should not allocate ball/element vectors and must not regress materially.

The committed `render_stages/backend/png_transparent_rich_trace` fixture now builds a separate scene with `transparent_options` and uses it for both the control and timed render. Its corrected `--quick` result is 14.34-14.48 ms. Treat the old table-backed 13.36 ms number as invalid for this workload, and save a formal immediate-parent baseline before evaluating this candidate.

Retain these adjacent cases:

- `render_stages/backend/svg_rich_trace`, using a prebuilt scene; its quick baseline is approximately 482 us and it proves scene work has not leaked into backend timing.
- `render_stages/backend/png_transparent_rich_trace`, using a genuinely transparent prebuilt scene; its corrected quick result is 14.34-14.48 ms.
- `throughput_rendering/trace_final_layout_svg`, retained as the end-to-end guard.

There is no cache or one-time initialization in this candidate, so a fresh-process cold harness would not measure a distinct contract. Use the ordinary paired-process Criterion protocol below; keep all source/fixture construction outside the timed closures.

### Allocation corroboration

Use a test/benchmark-only counting allocator or macOS Allocations instrumentation on the stage-isolated cases. Record total allocation count, total allocated bytes and peak live bytes for three independent process runs. Filtered stacks must confirm removal of:

- the temporary cloned `ball_positions` vector when nonempty;
- the temporary cloned `lines_to_draw` vector when nonempty;
- one temporary nested allocation per nonempty `SmoothPolyline.points`;
- one temporary allocation per nonempty cloned event label/title and text label;
- associated temporary `Position`/BigDecimal clone allocations.

The destination ball/element vectors and one owned copy of every nested scene payload are expected and must remain. For `overlays_only_1000_points`, require at least a 40% reduction in total allocated bytes and peak live bytes. For the committed `rich_trace`, require at least a 25% reduction in total allocated bytes. Allocation counts alone are not sufficient because BigDecimal internals and allocator behavior may coalesce small allocations.

## Statistical acceptance

Use the same Rust toolchain, release profile, host, power mode and Criterion configuration for baseline and candidate. Retain the corrected transparent PNG fixture and save a baseline from the immediate parent; its 14.34-14.48 ms quick result and the 150.75 us scene snapshot are orientation, not saved paired baselines. Run at least three independent paired process runs, alternating baseline/candidate order. Record Criterion median, full 95% bootstrap confidence interval and significance result for each filter, plus the allocation metrics.

Accept only when all conditions hold:

1. All correctness/equivalence gates below pass.
2. The committed `render_stages/scene_build/rich_trace` case has its full 95% time-change CI below zero and at least a 15% median improvement.
3. `render_stages/scene_build/overlays_only_1000_points` has its full CI below zero and at least a 25% median improvement.
4. `render_stages/scene_build/static_16_balls_shifted` has an upper 95% regression bound no greater than +2%. A measurable improvement is desirable but not mandatory because required BigDecimal shift resolution may dominate.
5. `render_stages/scene_build/empty`, `render_stages/backend/svg_rich_trace`, the corrected `render_stages/backend/png_transparent_rich_trace`, and existing combined `throughput_rendering/trace_final_layout_svg` each have an upper 95% regression bound no greater than +2%.
6. Both allocation-byte thresholds pass and filtered stacks show the expected eliminated temporary clone path.

Do not claim that the approximately 18.5 ms combined benchmark improved because of scene construction unless the isolated results satisfy these gates. If only allocation metrics improve while isolated timing is inside noise, reject this as a performance implementation rather than weakening the thresholds.

## Correctness and equivalence tests

### Position resolution

- Build one state with unresolved X-only, Y-only and X+Y ball positions and a second state containing explicitly resolved equivalents. Assert their built scenes render to exact equal SVG bytes and exact equal decoded PNG RGBA buffers.
- Snapshot each source ball as `(ty.clone(), position.clone(), spec.radius.clone())`, call `to_diagram_scene`, and compare those fields again one-for-one. `Ball`/`BallSpec` do not implement whole-struct equality, while `Position` equality includes its pending-shift fields; do not write an unimplementable whole-slice `assert_eq!`.
- Call `to_diagram_scene` twice on the same unresolved state and require exact equal backend output; this catches accidental mutation or double application of shifts.

### Field/order preservation

- For the 16-ball fixture, assert the scene ball sequence, types, positions and custom radii correspond one-for-one with source order.
- For the all-overlay fixture, interleave below/above-layer variants in source insertion order. Assert the exact variant/layer sequence directly on public `scene.elements` **and** assert each `elements_for_layer` subsequence before requiring exact SVG bytes. Backend goldens alone cannot detect a stable cross-layer partition because both backends render layers in separate passes.
- Exercise pool and carom `TableSpec` and both background options; assert viewport remains `DiagramViewport::default()` and scale factor does not alter the scene.

### Overlay pending-shift preservation

In the same-module private fixture described above, create pending X/Y shifts, store the position directly in at least one `Overlay` variant, call `to_diagram_scene`, and compare the resulting `DiagramElement` position to the original stored `Position`, including its pending fields. This is the implementable oracle for the rule that scene construction clones overlays without resolving them.

### Owned-scene semantics

Construct a rich scene, record its SVG and decoded PNG bytes, then mutate the source state by resolving positions, adding balls and adding long overlays; finally drop the source. Render the existing scene again and require exact equality with its recorded outputs. The test must access the scene after source drop, so any attempted borrowed-lifetime redesign fails at compile time as well as behaviorally.

### Backend equivalence

For empty, shifted, static-16, carom and rich-overlay fixtures:

- exact pre/post SVG bytes are mandatory;
- encoded PNG bytes should remain exact with the unchanged encoder;
- decoded PNG dimensions and complete RGBA bytes are authoritative;
- repeat two renders to detect hidden mutation.
- Share one immutable rich `GameState` across eight joined worker threads, build scenes repeatedly, and require every SVG byte string and decoded PNG RGBA buffer to equal the single-thread golden. The direct builder must use only call-local mutable storage.

Keep existing `drawing_resolves_pending_inches_shifts_before_rendering` and `diagram_scene_exposes_backend_neutral_balls_and_overlay_layers`; strengthen them rather than replacing useful diagnostics with only goldens.

## Risks and mitigations

- **A ball shift is skipped or applied twice:** per-axis unresolved fixtures, repeated construction and explicitly resolved oracle.
- **Overlays are accidentally resolved:** the same-module private pending-overlay fixture compares pending fields exactly; public overlay builders cannot create this state.
- **Owned nested data becomes borrowed/shared:** retain owned public types and source-mutation/drop test; no `Cow`, `Arc` or lifetime parameter.
- **Order/layer changes:** push in source order and assert the global interleaved `scene.elements` sequence, each layer-filtered subsequence, and full backend bytes.
- **Table or custom ball spec is lost:** pool/carom and custom-radius fixtures.
- **Iterator collection allocates unexpectedly:** prefer exact `with_capacity` loops and verify allocation stacks.
- **Benchmark dilution:** construct trace layout and scenes outside unrelated timed loops; acceptance is scene-only.
- **Concurrent rendering behavior:** the implementation is local and immutable; no global state or cache is introduced.

## Stop conditions, rejection and rollback

Reject the implementation if any exact output changes, the source is mutated, overlay behavior changes, owned-scene semantics weaken, an allocation-byte threshold fails, either principal scene fixture misses its practical timing threshold, or an adjacent-path upper CI exceeds +2%. Do not rescue the result with borrowing, caching, changed rendering output or reduced fixture richness.

Rollback is one independent commit restoring the clone/resolve/map body and removing only candidate-specific tests/bench fixtures. There is no schema, serialized-data or public-API migration.

## arm64 SIMD and GPU decision

Neither is appropriate. The work is heterogeneous owned-object cloning, BigDecimal-backed position resolution, enum matching and allocation removal. Standard vector/string copies already use optimized library/compiler paths where applicable; there is no stable uniform numeric loop for ARM64 SIMD. A GPU cannot help construct Rust ownership graphs and would add transfers and synchronization. Do not add unsafe intrinsics, architecture-specific branches, GPU code or dependencies.

## Candidate commit message

`perf(render): build diagram scenes without cloning game state`
