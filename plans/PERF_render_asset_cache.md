# Cache immutable raster assets and bounded resized ball sprites

## Status

- **Status:** Accepted candidate; implementation plan only.
- **Priority:** High for PNG rendering, no expected effect on SVG.
- **Confidence:** High for removing repeated PNG decode/resize work; medium for the incremental value of a cross-render resized-sprite cache until its bounded-cache stage is benchmarked under contention.
- **Dependencies/order:** The committed table-rich `render_stages` PNG case is a valid stage-isolated warm diagnostic; the nominal transparent case is currently table-backed and must be corrected/rebaselined first. Neither case is a correctness golden or exercises aliases, custom diameters, cache bounds, or fresh-process initialization. Capture the remaining goldens and add the candidate-specific warm/cold fixtures before production edits. This work is independent of SVG write streaming and direct `DiagramScene` construction. If those plans also edit `src/diagram.rs` or `benches/throughput.rs`, land one cleanly and baseline this candidate from its immediate parent; do not combine their measurements.

## Objective and observable contract

Build one raster-asset pipeline with three explicit lifetimes:

1. process-lifetime, immutable decoded table pixels and ten decoded original ball sprites;
2. process-lifetime, strictly bounded resized-ball entries for repeated frames;
3. render-lifetime, strictly bounded resized-ball reuse for hot keys, while arbitrary valid diameters that exceed local admission limits are still resized, drawn in order, and dropped immediately.

The renderer must continue to:

- produce exactly the same RGBA dimensions and pixel bytes for every fixture;
- preserve the existing ball iteration and overlay order;
- derive each ball's diameter with the current `DiagramViewport::ball_diameter_px` calculation and support every diameter that the current renderer supports;
- retain no globally unbounded collection of diameter keys;
- keep cached originals and resized sprites immutable so drawing one frame cannot alter later frames;
- preserve deterministic output under concurrent renders.

PNG encoding remains in the timed operation and keeps its current encoder settings. Decoded RGBA equality is the authoritative correctness gate; encoded PNG byte equality is also expected because this plan does not change encoding.

## Problem and evidence

### Measured facts

- `benches/throughput.rs` now contains `render_stages/backend/png_table_rich_trace` and `render_stages/backend/png_transparent_rich_trace`, with quick unchanged-tree results of approximately **13.58 ms** and **13.36 ms**. The table case is a valid prebuilt-scene render-only measurement.
- The nominal transparent case is currently mislabeled: `scene` is built once with `DiagramBackground::Table`, and `PngBackend::render` reads `scene.background`, not `options.background`. Passing `transparent_options` later does not change the scene, so the 13.36 ms result is another table-backed render and is **not** evidence for the transparent bypass. Correct that fixture on unchanged production code and save a new baseline before measuring this candidate.
- The existing static-PNG profile recorded in `plans/performance_engineering.md` attributes 28.01% to `image::imageops::sample::resize`, 10.12% to `fdeflate::decompress::Decompressor::read`, and 9.12% to PNG unfiltering. PNG encoding filtering is a separate 26.76% and is not addressed here.
- The same document records an older static table PNG at about 22.1 ms, versus about 1.8 ms for static SVG. These older numbers motivate isolation but are neither substitutes for the committed rich-scene baselines nor acceptance baselines for the candidate-specific fixtures.

### Source-grounded mechanism

- `BALL_IMGS` and `ball_img` in `src/assets.rs` embed ten compressed ball PNG byte slices and copy the selected compressed slice into a new `Vec<u8>` for every ball. The observable aliases are `YellowCue -> One` and `Red -> Three`.
- `draw_raster_balls` in `src/diagram.rs` calls `ball_img`, decodes the copied PNG, converts it to `RgbaImage`, and applies Catmull-Rom resize for every ball on every frame. It then overlays immediately in `scene.balls` order.
- `PngBackend::render` in `src/diagram.rs` decodes and converts `TABLE_DIAGRAM` on every PNG render. The embedded asset is 1089 x 1938. For `DiagramBackground::Transparent`, the decoded pixels are discarded; only their dimensions are used before creating a zeroed frame.
- The decoded table's RGBA pixel payload is `1089 * 1938 * 4 = 8,441,928` bytes. The current ten original sprite assets are 378 x 378 RGBA after decode, for `10 * 378 * 378 * 4 = 5,715,360` bytes. Implementation tests must validate those dimensions instead of silently trusting constants.

### Hypotheses to test

- Replacing per-render table decode/conversion with a clone of immutable decoded RGBA will reduce warm table-background latency despite retaining the required mutable output-frame copy.
- Transparent renders will improve both cold and warm latency because they can avoid table decoding entirely.
- Immutable original-sprite decode plus per-render key reuse will remove repeated decode/resize work for alias/duplicate keys. A bounded cross-render resized cache should further improve repeated-frame workloads.
- The process-wide resized cache may cease to help if lock traffic or eviction churn dominates. That final stage is benchmark-gated and must be rejected without weakening the immutable-original and per-render stages if it fails its gates.

## Scope

### In scope

- `src/assets.rs`: stable internal sprite identity and borrowed embedded bytes; remove the copying selector.
- `src/diagram.rs`: immutable decoded asset ownership, transparent-table bypass, render-local sprite reuse, and a bounded process-wide resized-sprite cache.
- `Cargo.toml`, the existing `render_stages` cases in `benches/throughput.rs`, a shared `benches/render_fixtures.rs`, and a cold-render benchmark target: retain the committed rich-scene diagnostics and add the missing alias/custom-diameter, golden, and fresh-process measurements.
- `tests/rendering_geometry.rs` and private unit tests in `src/diagram.rs`: golden RGBA, order, cache bound, mutation, and concurrency coverage.
- Checked-in PNG golden fixtures listed below, captured before the production change.

### Explicit non-goals

- No SVG formatting, allocation, escaping, or serialization changes.
- No `GameState`/`DiagramScene` clone or ownership changes.
- No physics, table geometry, viewport projection, diameter rounding, raster primitive, alpha-compositing, PNG encoder, resize filter, or whole-frame scale-factor changes.
- No grouping or sorting of balls for drawing. Cache lookup may be grouped internally only if it cannot alter the immediate overlay sequence; the planned implementation does not group.
- No public cache-control API and no compatibility alias for `ball_img`; `assets` is private and its only caller is migrated in the same change.
- No replacement image crate, unsafe code, custom PNG decoder, SIMD kernel, GPU backend, or asynchronous preloader.

## Target cache design

### Stable asset identity in `src/assets.rs`

Replace `ball_img(BallType) -> Vec<u8>` with an internal, copyable identity and borrowed bytes:

```rust
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum BallSpriteId {
    Cue,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

pub(crate) fn ball_sprite_id(ball_type: &BallType) -> BallSpriteId;
pub(crate) fn ball_sprite_png(id: BallSpriteId) -> &'static [u8];
```

`ball_sprite_id` must preserve the current mapping exactly: `YellowCue -> One`, `Red -> Three`, and every other variant to its matching sprite. Accept `&BallType` so the draw loop no longer clones the enum. `BallSpriteId` supplies an index for a fixed ten-element decoded-original cache. Keep the embedded byte arrays private or `pub(crate)`; do not expose mutable asset storage.

Define `TABLE_WIDTH_PX = 1089` and `TABLE_HEIGHT_PX = 1938` beside `TABLE_DIAGRAM`. The transparent path may use these constants without decoding. The table decode initializer and an asset test must assert that the embedded PNG still has exactly these dimensions, turning asset drift into an immediate failure rather than a mis-sized render.

### Process-lifetime immutable originals

Add a private `RasterAssetCache` in `src/diagram.rs` and obtain the singleton through `std::sync::OnceLock<RasterAssetCache>`. Do not add a cache crate.

```rust
struct RasterAssetCache {
    table: OnceLock<RgbaImage>,
    original_balls: [OnceLock<RgbaImage>; 10],
    resized_balls: Mutex<BoundedResizedSpriteCache>,
}

fn raster_asset_cache() -> &'static RasterAssetCache;
```

Use `std::array::from_fn` when constructing the ten `OnceLock`s. The table initializer decodes `TABLE_DIAGRAM`, converts once to RGBA8, checks 1089 x 1938, and returns an immutable shared reference. A transparent render must not call that initializer.

Each original-ball initializer decodes the borrowed bytes selected by `BallSpriteId`, converts once to RGBA8, asserts exactly 378 x 378, and thereafter returns only `&RgbaImage`. The exact check is part of the documented global-memory bound, not merely a nonzero/square sanity check. The cache itself never exposes `&mut RgbaImage`. Decode failures keep the current fail-fast behavior and include the asset identity in the panic message.

For a table background, clone the cached table into the mutable frame and draw on the clone. For a transparent background, allocate `RgbaImage::new(TABLE_WIDTH_PX, TABLE_HEIGHT_PX)` directly. The frame allocation is required and is not claimed as removed.

### Resized-sprite key

Use exactly:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ResizedSpriteKey {
    sprite: BallSpriteId,
    diameter_px: u32,
}
```

The resize filter remains the fixed `FilterType::CatmullRom`, so it is not initially part of the key. If a future change makes filtering selectable, the filter must be added to the key before that option lands. `scale_factor` is deliberately absent: the sprite is drawn at scene resolution and the completed frame is scaled afterward, exactly as today.

Compute `diameter_px` for every ball with the existing `scene.viewport.ball_diameter_px(&scene.table_spec, &ball.spec)` before lookup. Do not canonicalize, clamp beyond the current minimum of one pixel, bucket, or substitute a standard ball radius. Different computed diameters are different keys.

### Render-lifetime reuse and draw order

At entry to `draw_raster_balls`, construct a render-local fixed-vector LRU with hard bounds:

```rust
const MAX_RENDER_LOCAL_RESIZED_ENTRIES: usize = 64;
const MAX_RENDER_LOCAL_PIXEL_BYTES: usize = 16 * 1024 * 1024;
const MAX_RENDER_LOCAL_ENTRY_PIXEL_BYTES: usize = 4 * 1024 * 1024;
```

Use the same checked pixel-byte accounting as the process cache. The local LRU stores at most 64 `(ResizedSpriteKey, Arc<RgbaImage>, pixel_bytes)` entries; move a hit to most-recent position instead of maintaining unbounded recency metadata. For each ball in the existing order:

1. compute `sprite`, the exact `diameter_px`, checked `pixel_bytes`, and whether the key is locally admissible;
2. look up that key in the bounded render-local cache;
3. on an admissible local miss, evict least-recent entries **before** obtaining the image until `resident_bytes + pixel_bytes <= 16 MiB` and the new entry will fit under 64 entries;
4. ask the bounded process cache for a miss, falling back to resizing the immutable original;
5. insert an eligible image into the already-reserved local slot before drawing; if arithmetic failed or it exceeds 4 MiB, keep only the one current `Arc`;
6. calculate the same rounded center and clamp, overlay immediately, then proceed to the next ball.

The local cache drops when `draw_raster_balls` returns. It retains at most 16 MiB of resized payload and 64 keys per render, regardless of scene length or diameter diversity. The 600 px fixture is 1,440,000 bytes, so it remains locally admissible and is resized once per render even though it is too large for the process cache. Larger valid sprites remain supported but may be resized again if the same non-admitted key recurs; bounded peak memory takes precedence over an unlimited once-per-key guarantee.

Never collect balls by sprite, iterate cache entries for drawing, or draw in cache/LRU order. In overlapping regions, source-over alpha composition is order dependent.

### Strictly bounded cross-render resized cache

Implement `BoundedResizedSpriteCache` with the following hard limits:

```rust
const MAX_RESIZED_ENTRIES: usize = 64;
const MAX_RESIZED_PIXEL_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESIZED_ENTRY_PIXEL_BYTES: usize = 1 * 1024 * 1024;
```

A small fixed-entry/vector implementation is preferred over an unbounded `HashMap + VecDeque`: each entry stores key, `Arc<RgbaImage>`, exact logical pixel bytes, and a last-used counter. Compute bytes by converting `diameter_px` to `usize` and applying checked `diameter * diameter * 4`; after resize, assert it equals `image.as_raw().len()` before admitting the entry. Linear lookup/oldest-entry selection over at most 64 entries is bounded and occurs only once per unique render-local key. An equivalent map is acceptable only if its recency metadata and backing capacity are also strictly bounded; a queue that accumulates duplicate keys on hits is prohibited.

Advance the recency counter with checked arithmetic. On overflow, renumber the at-most-64 resident entries in their current least-to-most-recent order and continue; do not panic in debug builds or let wrapping arithmetic invert eviction order in long-lived processes.

On lookup:

- return an `Arc` clone and update recency on a hit;
- if checked size arithmetic fails, the entry exceeds 1 MiB, or it exceeds the total budget, resize outside the global cache and return the `Arc` only to the caller; the bounded local cache decides whether to retain it after drawing;
- otherwise, while holding the mutex, recheck the key, create the resized image once, evict least-recently-used entries until both the 64-entry and 8 MiB payload limits will hold, insert, and return an `Arc` clone.

Holding the mutex through an admitted small resize prevents duplicate same-key allocations on a cold concurrent miss. Oversized/non-admitted resizes occur outside the mutex. The stage-3 benchmark must decide whether this simple critical section is acceptable; do not introduce per-key condition variables or unsafe lock-free machinery speculatively. Handle mutex poisoning with a clear fail-fast message, consistent with the renderer's existing infallible API.

Eviction changes only reuse, never pixels. An evicted global `Arc` can remain alive in an active bounded render-local cache; therefore the **persistent cache-owned** resized payload is bounded at 8 MiB, while each active render retains at most 16 MiB of local resized payload plus the one image currently being resized/drawn if that image is not locally admissible.

### Retained-memory accounting

After every asset has been touched, the maximum logical RGBA pixel payload retained by global cache ownership is:

- decoded table: 8,441,928 bytes;
- ten decoded 378 x 378 originals: 5,715,360 bytes;
- resized-sprite cache: at most 8,388,608 bytes;
- **total: at most 22,545,896 bytes (about 21.50 MiB)**, plus fixed metadata for at most 64 resized entries, ten `OnceLock`s, `Arc` headers, mutex/container capacity, and allocator overhead.

There is no global map whose key count can grow with user-provided radii. Unit tests must assert both resized-cache counters after every insertion/eviction. If decoded sprite dimensions change, update the measured original-payload assertion and this documented bound in the same asset change; never hide drift by weakening the test.

Per-render memory is not included in that retained global bound: the mutable 1089 x 1938 frame already exists, scale-factor output may exist, the bounded local cache retains at most 16 MiB/64 entries, and one currently processed non-admitted sprite may be arbitrarily large exactly as in the existing one-at-a-time loop. Concurrent transient memory therefore scales with active renders, but neither arbitrary scene key count nor key diameter can accumulate in process-lifetime storage.

## Staged implementation and gates

### Stage 0: complete fixtures and save the formal unchanged baseline

1. Fix `render_stages/backend/png_transparent_rich_trace` on the unchanged renderer by building `transparent_scene = rendered.to_diagram_scene(&transparent_options)` and passing that scene to both an untimed control render and the timed closure. Keep the exact filter name, record that the old 13.36 ms result was table-backed, and save a new transparent baseline; never compare the candidate against the mislabeled workload.
2. Retain `render_stages/backend/png_table_rich_trace` and its approximately 13.58 ms quick result as orientation, then save the formal immediate-parent baseline after the transparent fixture correction.
3. Add `benches/render_fixtures.rs` with the direct `DiagramScene` factories below, and reuse those immutable factories from warm, cold, and correctness code.
4. Capture the six checked-in PNG golden fixtures below. Add the RGBA/golden tests and prove they pass on unchanged production code; benchmark controls that only assert nonempty output are not goldens.
5. Add the missing alias/custom-diameter warm cases and the fresh-process cold harness before production edits.

### Stage 1: borrowed IDs, immutable original decode, and table bypass

1. Replace `ball_img` with `BallSpriteId`, borrowed sprite bytes, and the exact alias mapping.
2. Add lazy immutable table/original caches.
3. Clone the cached table for table backgrounds and bypass table decode for transparent backgrounds.
4. Continue resizing once per ball at this intermediate point so stage-1 measurements isolate decode/table effects.
5. Run focused golden, mutation, and cold-concurrency tests, then measure the table/transparent and sprite fixtures against `render-assets-before`.
6. Apply the Stage-1 gate below before adding local reuse. Save the accepted result as `render-assets-stage1`.

Stop immediately on any decoded RGBA mismatch. Treat the table cache, transparent bypass, and immutable-original sprite path as independently rejectable: remove table caching if its isolated table gate fails; remove the transparent bypass if its gate fails; and remove the original-sprite cache and stop before Stages 2-3 if either warm sprite gate fails. Do not let later resized reuse mask a Stage-1 regression.

### Stage 2: bounded render-local resized reuse

1. Add `ResizedSpriteKey` and the bounded render-local LRU.
2. Keep the ball loop and immediate overlay operation in original order.
3. Measure alias/duplicate and mixed-diameter fixtures against `render-assets-stage1`. Allocation evidence must show one resized allocation per unique locally admitted key, not per ball; the 600 px key must be locally admitted.
4. Stress a scene with many distinct large diameters and assert local resident entries/payload never exceed 64/16 MiB. Non-admitted repeated keys may resize again and must not accumulate.
5. Keep Stage 2 only if the incremental gate below passes. If it fails, remove the local LRU and stop; Stage 3 depends on local reuse to bound lock traffic and must not proceed.

This stage preserves arbitrary diameters, bounds per-render retained sprites, and removes duplicate work for admitted hot keys, but those properties do not justify retaining a slower complete-render path.

### Stage 3: bounded cross-render resized reuse

1. Add the mutex-protected, 64-entry/8 MiB/1 MiB-per-entry LRU described above.
2. Add eviction, oversized-bypass, concurrent-cold-miss, and resident-counter tests.
3. Measure repeated warm frames and allocation counts. Compare against a stage-2-only baseline, not only the original renderer.
4. Keep stage 3 only if its repeated-frame gate passes and its uncontended/parallel guardrails hold. If rejected, remove the global resized cache completely; do not leave dormant types or feature flags. Stages 1-2 remain coherent.

## Exact benchmark fixtures

### Coverage already committed versus still required

Already committed in `benches/throughput.rs`:

- `render_stages/backend/png_table_rich_trace` — prebuilt rich table scene, complete PNG encode timed; quick baseline approximately 13.58 ms.
- `render_stages/backend/png_transparent_rich_trace` — currently uses that same table-background scene despite its name; quick result approximately 13.36 ms, invalid as a transparent baseline.

Correct the second fixture as Stage 0 specifies, prime each corrected rich case with its own untimed control render, and save new immediate-parent baselines before production edits. Retain both corrected filters as warm rich-scene guards. Neither replaces exact pixel fixtures, alias/custom-radius cases, eviction/contention tests, or the fresh-process harness below.

### Shared factories

Give the factories stable names and reuse them between warm, cold, and correctness code:

- `png_empty_table`: no balls/elements, table background, scale 1. Isolates decoded table clone versus repeated table decode.
- `png_empty_transparent`: no balls/elements, transparent background, scale 1. Proves no table pixels are needed.
- `png_all_sprites_and_aliases`: transparent scene and matching transparent render options; twelve separated balls in this order: Cue, One through Nine, YellowCue, Red, all at the default diameter. It touches all ten original sprites and repeats the One/Three asset identities through aliases without table-decode noise.
- `png_aliases_mixed_diameters`: transparent scene/options; separated One/YellowCue and Three/Red pairs at the default diameter, plus repeated One keys at deliberately constructed 27 px and 61 px diameters and one 600 px valid diameter. The 600 px key exceeds the 1 MiB global admission cap but must be reused locally and rendered correctly.
- `png_overlap_draw_order`: transparent scene/options; partially overlapping One, Three, YellowCue, and Red balls with centers offset enough to expose translucent antialiased edges. The vector order is intentionally not sprite-ID order.
- `png_edge_clamped_scale_2`: table-background scene and matching table render options; mixed default and custom-radius balls at all four scene edges, whole-frame scale factor 2.

The benchmark factories may share construction helpers but not mutable scenes. `DiagramScene` and `DiagramRenderOptions` are fully built before timing. Always pass the complete returned `Vec<u8>` through `black_box`; do not time an internal decode/resize helper.

### Warm Criterion cases

Retain the two committed rich-scene filters above. Add the following render-only cases to the existing `render_stages` group, using `diagram::render_scene_to_bytes` on prebuilt scenes:

- `render_stages/backend/png_warm/empty_table_scale_1`
- `render_stages/backend/png_warm/empty_transparent_scale_1`
- `render_stages/backend/png_warm/all_sprites_and_aliases_scale_1`
- `render_stages/backend/png_warm/aliases_mixed_diameters_scale_1`
- `render_stages/backend/png_warm/edge_clamped_scale_2`

Before each new warm `bench_function`, perform one untimed render of that exact case and black-box its complete output. Do the same for both rich-scene filters when correcting the transparent scene. This removes benchmark-order dependence and makes “warm” explicit; Criterion's own warmup is not the cache contract.

The alias cases naturally render the same prebuilt scene across Criterion iterations and therefore measure repeated frames. Keep PNG encode and the full mutable frame in the timed body; the optimization is accepted on complete-output latency, not a helper microbenchmark.

### Cold fresh-process cases

A `OnceLock` cannot be reset safely inside ordinary Criterion iterations. Add a dedicated `render_cold` bench target with `harness = false` and a custom main:

- child mode must be detected before Criterion setup and accept only a fixed allowlist of fixture names;
- parent mode registers Criterion `iter_custom` cases;
- for every requested operation, the parent spawns the same benchmark executable once with an explicit child-mode environment variable;
- the child constructs the named scene before `Instant::now()`, performs exactly one complete PNG render, black-boxes the bytes, stops the timer, validates output length and dimensions, and prints exactly one integer duration in nanoseconds;
- the parent rejects extra output, malformed/overflowing durations, unknown cases, signals, or nonzero exit; it sums only validated child-reported render durations and never substitutes zero.

This measures a fresh **process cache** (`OnceLock` and bounded resized cache) for every operation while excluding process startup and fixture construction. It is not a claim of cold filesystem/page-cache I/O because the assets are embedded in the executable.

Name the cases:

- `render_stages/backend/png_cold/empty_table_scale_1`
- `render_stages/backend/png_cold/empty_transparent_scale_1`
- `render_stages/backend/png_cold/all_sprites_and_aliases_scale_1`

Use a Criterion sample size of at least 20 for this expensive harness and report the spawned-operation count. Validate exact pixels in the separate golden tests, not inside the measured interval.

Retain `throughput_rendering/trace_final_layout_svg` as an adjacent-path guard only. `trace_playback_frames_2_5ms` is not a rendering benchmark and need not be run for this plan.

Run the focused filters as:

```text
cargo bench --bench throughput -- 'render_stages/backend/png'
cargo bench --bench render_cold -- 'render_stages/backend/png_cold'
cargo bench --bench throughput -- 'throughput_rendering/trace_final_layout_svg'
```

Use Criterion's `--save-baseline render-assets-before` on the corrected-benchmark unchanged production tree and compare the candidate with `--baseline render-assets-before`. Save/compare `render-assets-stage1` for the Stage-2-only delta and `render-assets-stage2` for the Stage-3-only delta. Preserve the project's existing one-second Criterion warmup for warm cases; the cold bench uses fresh children instead.

## Exact correctness and equivalence tests

### Checked-in PNG goldens

Capture these unchanged-production-renderer files under `tests/fixtures/render_png/` after correcting the transparent benchmark fixture and before production edits:

- `empty_table_scale_1.png`
- `empty_transparent_scale_1.png`
- `all_sprites_and_aliases_scale_1.png`
- `aliases_mixed_diameters_scale_1.png`
- `overlap_draw_order_scale_1.png`
- `edge_clamped_scale_2.png`

For each fixture, render after the change, decode actual and golden with `image`, assert identical dimensions, color conversion to RGBA8, and byte-for-byte equality of `as_raw()`. Also assert exact encoded bytes because encoder configuration is unchanged. If only encoded bytes differ while RGBA matches, stop and explain the unexpected encoder change; do not casually regenerate goldens in this cache-only change.

The all-sprites golden binds every `BallSpriteId` to its pre-change pixel payload, including aliases; mapping/dimension tests alone cannot detect two equal-sized PNG assets being swapped. The remaining fixtures detect missing custom-diameter key material, draw reordering, changed clipping, wrong table initialization, changed transparent pixels, and scale-factor interaction.

### Asset identity and dimensions

In `src/assets.rs` unit tests:

- assert every `BallType` maps to the expected `BallSpriteId`;
- explicitly assert One equals YellowCue and Three equals Red identities, with no other accidental alias;
- decode the table bytes and assert 1089 x 1938;
- decode all ten originals, assert 378 x 378 RGBA payloads, and assert the summed payload is 5,715,360 bytes.

### Mutation isolation

Using a fresh private `RasterAssetCache` in `src/diagram.rs` unit tests:

1. snapshot the cached table and every initialized original sprite's raw bytes;
2. render `png_overlap_draw_order` and `png_aliases_mixed_diameters` twice through an internal render function that accepts that cache;
3. assert render 1 equals render 2 in encoded bytes and decoded RGBA;
4. assert every cached original raw buffer still equals its snapshot;
5. render an empty table afterward and compare it with `empty_table_scale_1.png`.

This fails if the mutable destination is accidentally shared with the cached table or if `overlay` ever receives a mutable cached sprite.

### Concurrency and cold initialization

Construct one fresh `Arc<RasterAssetCache>`, a barrier, and eight worker threads. Each worker waits at the barrier and then renders `png_all_sprites_and_aliases` and `png_empty_table` ten times through the injectable internal renderer. Compare every encoded result and decoded RGBA buffer with a single-thread golden. Join every worker and fail on panic.

After joining, assert exactly one initialized table slot, all ten original slots initialized, no poisoned mutex, resized entry count at most 64, and resident resized payload at most 8 MiB. This test must start with the local cache, not the process singleton, so test order cannot make “cold” false.

### Bounded keys, eviction, and arbitrary diameters

Private cache tests must:

- request at least 80 distinct process-cache-admissible `(sprite, diameter)` keys, asserting after every request that entries are at most 64 and payload at most 8 MiB;
- insert nine distinct 512 px keys (1 MiB each) and assert byte-budget eviction occurs while the entry count remains below 64, proving the independent 8 MiB aggregate limit is enforced;
- call the checked byte-size/admission helper with `u32::MAX` and assert overflow returns non-admitted without attempting a resize;
- touch a known key, force eviction pressure, and verify the least-recently-used untouched key is evicted while the touched key remains;
- initialize the test recency counter near `u64::MAX`, perform enough hits/inserts to force renumbering, and assert no panic, unchanged resident bounds, and the same least-recently-used eviction order;
- request the 600 px sprite (`1,440,000` logical RGBA bytes), assert it is not inserted globally, render two balls with that same key in one scene, and require exactly one resize plus local payload no greater than 16 MiB;
- exercise more than 64 locally admissible keys whose aggregate exceeds 16 MiB and assert local entry/byte eviction after every insertion;
- render a repeated sprite larger than the 4 MiB local-entry cap, assert it is never retained and peak retained local payload stays bounded, while exact RGBA remains unchanged;
- render default, 27 px, and 61 px instances of the same sprite and compare with the mixed-diameter golden, proving keys do not collide;
- force eviction between two renders and require identical RGBA afterward, proving eviction affects only performance.

Test-only counters and cache-stat accessors stay under `#[cfg(test)]`; production does not expose cache internals.

## Statistical acceptance

### Protocol

- Use the same host, power mode, Rust toolchain, locked dependencies, release profile, Criterion configuration, and otherwise idle system for baseline and candidate.
- Save the unchanged production renderer as `render-assets-before` only after the transparent rich fixture is corrected and its new baseline recorded. Save Stage 1 as `render-assets-stage1` before local reuse and Stage 2 as `render-assets-stage2` before the process-wide resized cache.
- Run at least three independent paired baseline/candidate **process** pairs for each filter. Alternate order (`A/B`, `B/A`, `A/B`) to reduce thermal/order bias. Do not treat Criterion samples from one process as three runs.
- Report Criterion's 95% bootstrap confidence interval for relative time change from every pair and the aggregate direction across pairs. A required improvement passes only when the entire 95% CI is below the threshold in all three pairs; a guardrail passes when the upper CI bound stays below its limit in all three pairs.
- Black-box complete PNG bytes. Report output size to prove equivalent work.

### Stage-1 incremental gate

Relative to `render-assets-before`, require all of the following in every paired run before proceeding:

- `render_stages/backend/png_warm/empty_table_scale_1`: full 95% CI at or below -10% to retain decoded-table caching.
- `render_stages/backend/png_warm/empty_transparent_scale_1`: full CI at or below -20% to retain the transparent bypass.
- `render_stages/backend/png_warm/all_sprites_and_aliases_scale_1`: full CI at or below -10%.
- `render_stages/backend/png_warm/aliases_mixed_diameters_scale_1`: full CI at or below -5%.
- `render_stages/backend/png_cold/empty_table_scale_1` and `render_stages/backend/png_cold/all_sprites_and_aliases_scale_1`: upper CI bound no greater than +5%.
- Corrected `render_stages/backend/png_table_rich_trace` and `render_stages/backend/png_transparent_rich_trace`: upper CI bound no greater than +3%.

The two warm sprite gates and allocation evidence decide whether immutable-original caching survives and whether Stages 2-3 may proceed. Table and transparent decisions remain independent as described above.

### Practical gates against the unchanged renderer

After the staged decisions, compare the final retained composition with `render-assets-before`:

- If decoded-table caching remains, `render_stages/backend/png_warm/empty_table_scale_1` must retain a full relative-time CI at or below -10%; if removed, that case becomes a +3% upper-bound guardrail.
- If the transparent bypass remains, `render_stages/backend/png_warm/empty_transparent_scale_1` must retain a full CI at or below -20% and `render_stages/backend/png_cold/empty_transparent_scale_1` at or below -10%; if removed, both become +3% upper-bound guardrails.
- If immutable-original sprite caching remains, final `render_stages/backend/png_warm/all_sprites_and_aliases_scale_1` and `render_stages/backend/png_warm/aliases_mixed_diameters_scale_1` must retain full CIs at or below -10% and -5%, respectively. If the sprite pipeline was rejected at Stage 1, both become +3% upper-bound guardrails and Stages 2-3 are absent.
- `render_stages/backend/png_cold/empty_table_scale_1` and `render_stages/backend/png_cold/all_sprites_and_aliases_scale_1` remain startup guardrails with upper CI no worse than +5%.
- `render_stages/backend/png_warm/edge_clamped_scale_2`, corrected `render_stages/backend/png_table_rich_trace`, corrected `render_stages/backend/png_transparent_rich_trace`, and `throughput_rendering/trace_final_layout_svg` each have an upper CI bound no worse than +3%.

Do not claim a speedup for a removed or neutral component. Every retained Stage 2/3 layer must additionally pass its incremental gate below; later stages may not erase these end-to-end floors.

### Stage-2 incremental gate

Relative to `render-assets-stage1`, the bounded local LRU must make `render_stages/backend/png_warm/aliases_mixed_diameters_scale_1` at least 5% faster with the full 95% CI at or below -5%. `render_stages/backend/png_warm/all_sprites_and_aliases_scale_1`, `render_stages/backend/png_warm/empty_table_scale_1`, `render_stages/backend/png_warm/empty_transparent_scale_1`, corrected `render_stages/backend/png_table_rich_trace`, and corrected `render_stages/backend/png_transparent_rich_trace` must each have an upper CI bound no greater than +3%. Allocation evidence must show one resize per locally admitted unique key, and the 64-entry/16 MiB stress bounds must pass. Otherwise remove Stage 2 and do not implement Stage 3.

### Stage-3 incremental gate

Relative to `render-assets-stage2`, the bounded global resized cache must make both `render_stages/backend/png_warm/all_sprites_and_aliases_scale_1` and `render_stages/backend/png_warm/aliases_mixed_diameters_scale_1` at least 8% faster with the full 95% CI at or below -8%. Its upper CI must be below +3% for `render_stages/backend/png_warm/empty_table_scale_1` and `render_stages/backend/png_warm/empty_transparent_scale_1`. In an eight-thread stress timing using prestarted workers and equal work per thread, total throughput must not regress by more than 5% versus Stage 2. Otherwise remove Stage 3 and ship only immutable originals plus bounded render-local reuse.

### Allocation/profile corroboration

Capture allocations for one warm render of `png_all_sprites_and_aliases` and `png_aliases_mixed_diameters` using the platform allocation profiler used for the baseline. Expected signatures are:

- no compressed-sprite `Vec` copy;
- no table decoder allocations after table-cache warmup;
- no ball decoder allocations after original-cache warmup;
- one resized-image allocation per unique **locally admitted** key on a stage-2 cold miss; non-admitted repeated keys may allocate again by design;
- zero resized-image allocations on stage-3 warm hits;
- no growth in globally retained key metadata after cycling thousands of arbitrary diameters.

Re-profile a warm rich PNG render. The `fdeflate`/PNG-unfilter stacks should disappear for cached input assets; the output encoder remains and must not be misreported as removed. If allocation evidence contradicts the expected cache hit behavior, investigate before accepting timing results.

## Risks and mitigations

- **Wrong alias identity:** explicit all-variant mapping tests and alias goldens.
- **Wrong diameter reuse:** exact `u32` diameter in the key; mixed 27/39/61/600 px golden.
- **Changed draw order:** immediate overlay in `scene.balls` order and an overlapping order-sensitive golden.
- **Cached-image mutation:** expose only shared immutable references/`Arc`s; clone table into the destination; successive/concurrent render tests.
- **Asset dimension drift:** decode-time assertion plus direct asset-dimension tests; transparent dimensions come from validated constants.
- **Unbounded memory/key growth:** process cache is limited to 64 entries/8 MiB total/1 MiB per entry; every render-local cache is limited to 64 entries/16 MiB total/4 MiB per entry; both use bounded recency metadata and explicit entry-, byte-, overflow-, and eviction-pressure tests.
- **Lock contention or poisoning:** one process-cache lock acquisition per local miss, oversized work outside the lock, concurrent stress gate, explicit poison failure. Reject stage 3 rather than complicating synchronization without evidence.
- **Large custom sprites:** never bucket or reject a diameter that currently renders. Bypass the process cache; reuse it within the render only if it meets the 4 MiB local cap, otherwise draw and drop one image at a time as the current renderer does. Existing behavior for a sprite too large for the canvas is outside this performance change and must not be silently altered.
- **Benchmark contamination:** prebuilt scenes, explicit warm priming, corrected transparent scene ownership, fresh child processes for cold cases, and separate SVG/scene work.
- **Retained-memory tradeoff:** report the 21.50 MiB maximum logical global pixel payload and the 16 MiB per-active-render local cap alongside latency. If the target deployment cannot tolerate them, reduce the bounded budgets or reject the relevant stage; do not make limits configurable through an unbounded default.

## Stop conditions, rejection, and rollback

Stop implementation immediately on any RGBA mismatch, nondeterministic concurrent output, changed ball order, poisoned-lock failure, or violation of entry/byte bounds. Do not regenerate goldens to bless a cache bug.

Apply rollback at the failing stage: remove any Stage-1 component that misses its isolated gate; if the original-sprite path fails, omit Stages 2-3 entirely. Remove the bounded local LRU if Stage 2 misses its incremental timing/memory gates. Remove only the mutex/process-wide resized LRU if Stage 3 misses its speed or contended-throughput gates. Allocation reduction without the stated complete-render timing threshold is not sufficient to retain a failed layer, and no dormant type, feature flag, or compatibility path remains.

Rollback is a direct reversal of `RasterAssetCache`, local key reuse, and the borrowed selector. There are no persisted cache files, public APIs, feature flags, or compatibility shims. Benchmark and correctness fixtures should remain because they fill previously missing PNG coverage.

## SIMD and GPU decision

**arm64 SIMD: reject for this plan.** The optimization removes PNG decoding, copies, and duplicate library-owned resize calls. Remaining resize, PNG codec, and compositing kernels are inside `image`/`imageproc` dependencies or are order-sensitive alpha operations. There is no profile evidence for a new hand-written vector kernel, and introducing one risks pixel-rounding divergence.

**GPU: reject for this plan.** Upload/readback, device initialization, synchronization, and a second backend would add overhead and nondeterminism to 1089 x 1938 offline PNG generation. The measured opportunity is reuse of immutable CPU assets, not throughput that justifies GPU residency. Exact RGBA identity and portable concurrent rendering are better served by the bounded CPU cache.

## Candidate commit message

`perf(render): cache immutable raster assets and bounded sprites`
