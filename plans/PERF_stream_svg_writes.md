# Stream SVG and report writes into final buffers

## Status

- **Status:** Accepted implementation plan; not implemented.
- **Priority:** High for allocation removal, medium for wall-clock latency until isolated measurements confirm the magnitude.
- **Confidence:** High that the current code allocates and copies avoidable formatting intermediates; medium that the isolated backend latency will move enough to meet the thresholds. The committed rich-scene and long-polyline cases establish cost but not the candidate delta.
- **Dependencies/order:** None. The committed `render_stages` backend cases are the starting diagnostics. Add the remaining byte-equivalence goldens and serializer guards, save a baseline from the immediate parent, then change serialization. This change is independent of `PERF_direct_scene_build.md` and `PERF_render_asset_cache.md`; if another rendering plan lands first, recreate the baseline from its parent so every comparison isolates this change.

## Problem and evidence

### Current SVG backend

`SvgBackend::render` in `src/diagram.rs` currently builds the final SVG as a `String`; `render_scene_to_bytes` then transfers that allocation with `String::into_bytes`. The transfer is already zero-copy and is not a target. The avoidable work happens before it:

- The destination starts as `String::new` with no capacity hint.
- The root, orientation and layer tags use `svg.push_str(&format!(...))`. Table definitions and numeric table primitives follow the same pattern. Each `format!` allocates a temporary `String`, formats into it, copies it into `svg`, and drops it.
- `push_svg_corner_pocket` and `push_svg_side_pocket` first allocate `shelf_path` or `back_curve`, then interpolate that temporary into two more temporary element strings.
- `push_svg_element` allocates a `String` per smooth-polyline point, collects those strings in `Vec<String>`, joins them into a second path string, and then interpolates that into a third element string. Heading chevrons repeat the same point/vector/join pattern for three points.
- `svg_color` allocates a seven-byte RGB string for each call. `svg_rgb` does the same for spin-glyph colors, and `SpinGlyphMetrics` owns both resulting strings.
- `escape_xml` always allocates, including inputs with no escapable characters. Event markers and text labels then copy that allocation into another temporary element string. Spin-glyph title generation nests `format!`, `escape_xml`, and another outer `format!`, and copies the same title twice.
- Ball and spin-glyph element writers have the same outer temporary allocation pattern.

The committed `render_stages/backend/svg_rich_trace` and `render_stages/backend/svg_long_polyline_1000` cases time only `render_scene_to_bytes` on prebuilt scenes and black-box complete output bytes. Quick unchanged-tree results are approximately **482 us** and **565 us** per render, respectively. These are valid stage baselines, unlike the older combined result, but they do not by themselves prove how much temporary formatting costs.

The macOS profile recorded in `local://perf-plan-context.md` sampled the combined `throughput_rendering/trace_final_layout_svg` path: 3730/3895 samples were below `ScenarioBallTrace::sampled_points`, with 2932 below `advance_timeline_ball_state`, and only a few reached `SvgBackend`. The approximately 18.5 ms combined result remains an end-to-end guard and cannot attribute serializer speed.

### Current SVG report generator

`src/svg_generator.rs` already imports `std::fmt::Write` and uses `write!` for several numeric fields, but `render_svg_report_from_dsl_with_options` starts its final JSON with `String::new`. Both that function and `push_playback_json` allocate `format!("({})", index + 1)` before immediately escaping/copying it. `push_json_string` itself streams escaping correctly, but the destination receives no capacity planning for the already-known SVG, event, ball, frame and per-frame-ball counts. `event.kind.format_human()` and `event.format_human()` return owned strings; changing those APIs belongs to a separate trace/event-formatting candidate unless the isolated report evidence proves they dominate.

## Goal and observable contract

Write SVG and SVG-report syntax directly into their final `String` buffers using `std::fmt::Write`, while preserving **every output byte**: element order, whitespace, separators, fixed precision, case of hexadecimal digits, escaping, attribute order, negative-zero spelling and trailing newline. Keep `SvgBackend::Output = String`, `render_scene_to_bytes` and all public rendering/report APIs unchanged.

Success means the isolated SVG serializers allocate materially less and become faster without changing exact SVG or report bytes. Semantic DOM or decoded-JSON equality alone is insufficient.

## Scope

### In scope

- `src/diagram.rs`: `SvgBackend::render`, all `push_svg_*` functions, `svg_color`, `svg_rgb`, `escape_xml`, and private serialization-only support types/functions.
- `src/svg_generator.rs`: destination reservation and direct event-label emission in `render_svg_report_from_dsl_with_options` and `push_playback_json`; exact JSON escaping remains in `push_json_string`.
- `benches/throughput.rs`: retain the committed stage-isolated rich/long SVG backend fixtures and add the missing static, carom, transparent-label, and public-report guards.
- `tests/rendering_geometry.rs` and `tests/svg_generator.rs`: exact byte fixtures and targeted escaping/precision tests.

### Explicit non-goals

- PNG table or ball asset caching, PNG encoding, raster drawing, and PNG output.
- Scene construction or `GameState` cloning; that is `PERF_direct_scene_build.md`.
- Trace sampling, playback cursor generation, physics, geometry, event ordering, or numerical formatting changes.
- Changing SVG whitespace, minifying markup, changing attribute order, reducing precision, switching XML/JSON escape spellings, or accepting merely equivalent XML/JSON.
- Replacing `format_human` APIs without separate evidence.
- Changing the `String` backend into an I/O stream or adding fallible public render APIs.

## Implementation design

### 1. Establish byte-exact fixtures before changing production serialization

Add deterministic golden fixtures generated by the current implementation and committed as ordinary test inputs; tests must never auto-update them. Cover:

1. Pool/table background with standard balls and both overlay layers.
2. Three-cushion carom table.
3. Transparent background.
4. A 1,000-point smooth polyline, including coordinates that render as negative zero and values that round across a `.3` boundary.
5. All overlay variants: dashed line, smooth polyline, heading chevron, ghost ball, origin marker, circle marker with label/title, text label, and stun/planar/z/combined spin glyphs.
6. Text and event metadata containing `&`, `<`, `>`, `"`, `'`, non-ASCII text, newline and tab. Assert the current exact XML spellings `&amp;`, `&lt;`, `&gt;`, `&quot;`, and `&#39;`.
7. A static no-shot SVG report and a bounded rich traced report. The latter covers event labels, both human event strings, playback ball metadata, multiple frames and JSON escapes for `<`, `>`, `&`, quotes, slashes/control characters and U+2028/U+2029 where constructible.

Use full `assert_eq!(actual.as_bytes(), expected_bytes)`, not substring checks or hashes. Keep the existing geometry/DOM assertions because they produce better diagnostics for geometry regressions; the new golden tests enforce serialization identity.

### 2. Write formatted fragments directly to the destination

Add `use std::fmt::Write as _;` to `src/diagram.rs`. Replace every `svg.push_str(&format!(...))` with `write!(svg, ...).expect("writing SVG to string should not fail")`. Keep literal-only fragments as `push_str`; it is cheaper and clearer than formatting. Do not introduce a generic writer abstraction: all current callers target the owned final `String`, and an extra trait layer would add no value.

Preserve each existing format specifier exactly (`.0`, `.1`, `.2`, `.3`, lowercase `02x`, and the current default formatting where used). Convert one helper at a time and run the byte fixtures after each logical group: document/root, table, pockets/sights, overlays, spin glyphs, then balls.

### 3. Emit point lists sequentially

For `SmoothPolyline`, write the opening tag through `points="`, then iterate source points in order. For each point:

1. Convert it once with `scene.viewport.position_to_scene_point`.
2. Emit a single ASCII space only when the index is nonzero.
3. `write!(svg, "{:.3},{:.3}", point.x, point.y)`.

Write the remaining attributes and closing tag directly afterward. Do the same for the fixed three heading-chevron points. This removes the per-point `String`, `Vec<String>`, join string and outer element string while retaining the exact current separator and precision. Empty/one-point smooth polylines must still emit nothing.

Do not batch, sort, or reorder points or elements.

### 4. Stream duplicated pocket paths without scratch strings

Represent each bounded shelf/back-curve geometry as a private borrowed `fmt::Display` adapter, or use a private `write_*_path(&mut String, &Geometry)` helper called at the precise insertion point. Emit the same path coordinates directly into each of the two destination elements. Do not retain a heap `String` scratch buffer.

This repeats numeric formatting for the two duplicated attributes whereas the current code formats the path once and copies it twice. Keep this design only if the isolated table fixture passes the timing guard below. If repeated formatting regresses, retain one reusable `String` scratch owned by `SvgBackend::render`, `clear()` it between bounded path uses, and document that exception; never allocate a new scratch per pocket.

### 5. Remove color and XML temporary strings

Introduce small private zero-allocation display adapters:

- `SvgRgb([u8; 3])` implements `Display` as the exact lowercase `#{:02x}{:02x}{:02x}` representation.
- `EscapedXml<'a>(&'a str)` implements `Display` by writing each current entity spelling directly to the formatter.

Change `svg_color` to return `(SvgRgb, f32)` and retain the current clamped alpha calculation. Store `[u8; 3]` or `SvgRgb`, not `String`, in `SpinGlyphMetrics`; `svg_rgb` becomes allocation-free. Use `EscapedXml` directly in event-marker and text-label writes.

For the generated spin title, use a private display adapter over `SpinGlyphMetrics` and write it directly at both current positions. Its static punctuation and numeric output contain no XML metacharacters; nevertheless, keep the general byte fixture as the authority. Do not build `format!` and then escape it.

### 6. Use an explicit, overflow-safe reserve policy

Capacity is a hint, never a correctness precondition. Do not make a formatting or coordinate-conversion counting pass: doing that work twice can cost more than a reallocation. A linear scan of external text solely to compute exact escape expansion is acceptable and must not invoke geometry conversion or formatting.

Add `svg_capacity_hint(scene) -> Option<usize>` using checked arithmetic and these initial calibrated terms:

- 16 KiB fixed document/style/defs/table allowance.
- 384 bytes per ball.
- 640 bytes per non-polyline element.
- 32 bytes per smooth-polyline point and per emitted heading-chevron point.
- Input UTF-8 byte length plus the exact XML escape expansion delta for event labels, titles and text labels.

The helper must use `checked_mul`/`checked_add`; on overflow return `None` and keep the destination at `String::new()`. Initialize with `String::new()` and best-effort `try_reserve(hint)` only for `Some(hint)`; a failed capacity hint must not panic or suppress output. Before a long point list, compute `estimated_additional = constant_tag_bytes + 32 * points.len()` with checked arithmetic. If `capacity() - len()` is smaller, call `try_reserve(estimated_additional)`, **not** `try_reserve(deficit)`: `String::try_reserve` takes bytes additional to the current length, so passing the deficit can be a no-op while still leaving insufficient headroom. Ignore reserve failure and let ordinary writes preserve current behavior. During implementation, record actual `len` and capacity for all benchmark fixtures; adjust the constants once so the static, rich-trace and 1,000-point fixtures each have no more than one growth after initialization and final unused capacity is no more than 25% of `len`. These are reserve-policy gates, not output gates.

For report JSON, add an exact `json_string_encoded_len(&str)` matching all bytes emitted by `push_json_string`, including its surrounding quotes. Best-effort reserve the known top-level fixed bytes plus the exact encoded SVG length before the first write. For event strings, format each existing owned human string once, reserve its exact encoded length, write it, and drop it. Before playback frames are emitted, best-effort reserve by structural counts after `playback_frames` exists: fixed record bytes plus 64 bytes per playback ball and 192 bytes per frame-ball record. Use checked arithmetic and `try_reserve` with the same overflow/allocation-failure fallback. Do not hold a second copy of the SVG or frames merely to size the destination. Tune the structural constants under the same <=25% unused-capacity and <=1-growth fixture gates.

### 7. Stream report event labels

Replace each temporary `format!("({})", index + 1)` with direct JSON string syntax:

```text
json.push_str("\"(");
write!(json, "{}", index + 1);
json.push_str(")\"");
```

Digits and parentheses require no JSON escaping, so this is byte-identical to passing the temporary label through `push_json_string`. Keep the current six-decimal numeric formats and field ordering. Keep `push_json_string` for all externally derived text.

### 8. Clean cutover

Delete the old allocating `svg_color`, `svg_rgb`, and `escape_xml` return-`String` implementations after all callers use the adapters. Leave no aliases, deprecated helpers, compatibility shims, or parallel serializer. `String::into_bytes` remains unchanged because it transfers ownership of the existing allocation.

## Benchmark plan

### Coverage already committed

`benches/throughput.rs` already constructs the inputs outside `b.iter`, times only `render_scene_to_bytes`, and black-boxes the complete returned `Vec<u8>` for:

- `render_stages/backend/svg_rich_trace` — prebuilt traced scene; quick baseline approximately 482 us.
- `render_stages/backend/svg_long_polyline_1000` — prebuilt transparent scene with one deterministic 1,000-point `SmoothPolyline`; quick baseline approximately 565 us.

Retain those exact filter names. The current setup asserts only nonempty control output, so it does not replace byte goldens or prove that the rich fixture contains every escaping/layer variant. Before production edits, add explicit setup assertions for the intended layer, event-marker/title, and spin-glyph content, and commit the byte fixtures below.

### Remaining matrix extensions

Add these prebuilt-scene cases to the existing `render_stages` group:

- `render_stages/backend/svg_static_pool`
- `render_stages/backend/svg_three_cushion_carom`
- `render_stages/backend/svg_transparent_labels`

Fixture definitions:

- `svg_static_pool`: table background, a deterministic 16-ball layout and no trace generation in the timed loop.
- The committed `svg_rich_trace`: reuse the current two-ball scenario and options, but require both element layers, event titles/labels and spin glyphs during setup; keep `rendered_final_layout_with_trace_options` and `to_diagram_scene` outside timing.
- The committed `svg_long_polyline_1000`: retain its transparent prebuilt scene and deterministic 1,000-point path; no physics or scene construction belongs in the loop.
- `svg_three_cushion_carom`: a prebuilt carom scene that exercises the alternate table serializer.
- `svg_transparent_labels`: a prebuilt transparent scene covering XML expansion and both overlay layers.

Retain `throughput_rendering/trace_final_layout_svg` as the combined end-to-end guard, not as evidence of serializer speed.

Add a separately reported `render_stages/svg_report/rich_trace_public_api` case calling `render_svg_report_from_dsl_with_options` on a bounded rich DSL. It includes parsing, simulation, scene construction and playback generation and therefore is a regression guard only; do not use it to accept a serializer speed claim. Do not expose benchmark-only production APIs to isolate private report helpers.

All added inputs must be fully constructed before their timed closures wherever the public API permits. Only the report guard intentionally includes upstream work. Black-box the complete returned string/bytes, and validate exact expected bytes outside timing.

### Allocation corroboration

Capture allocation counts/bytes for the committed `svg_rich_trace` and `svg_long_polyline_1000` cases and the added `svg_static_pool` case with the same release binary and macOS Allocations instrumentation, filtering stacks to `SvgBackend::render` and `push_svg_*`. The long-polyline expected signature is removal of approximately one allocation per point plus the point-vector-of-strings, join and outer-format allocations. Accept only if allocations attributed to SVG serialization fall by at least 90% for `svg_long_polyline_1000` and by at least 50% for `svg_rich_trace`; the final output allocation is expected and must remain.

## Statistical acceptance

Use the same Rust toolchain, release profile, host, power mode and Criterion configuration for all comparisons. Save a baseline from the immediate parent; the quick 482 us/565 us snapshots are orientation, not a saved paired baseline. Run at least three independent **paired process runs**, alternating baseline/candidate order across pairs. For every case record Criterion median, 95% bootstrap confidence interval and significance result, output length/capacity/growth count, and the allocation evidence above.

Accept the implementation only when all conditions hold:

1. All exact byte fixtures pass for SVG and report JSON.
2. Criterion's full 95% time-change CI is below zero and median improvement is at least 15% for `render_stages/backend/svg_long_polyline_1000`.
3. The full CI is below zero and median improvement is at least 5% for `render_stages/backend/svg_rich_trace`.
4. `render_stages/backend/svg_static_pool`, `render_stages/backend/svg_three_cushion_carom`, `render_stages/backend/svg_transparent_labels`, the existing combined `throughput_rendering/trace_final_layout_svg`, and the public report guard each have an upper 95% regression bound no greater than +3%.
5. The allocation and reserve-policy gates above pass.

If timing is inside noise but allocation gates pass, reject the change as a performance commit; do not relabel it as a speedup. A smaller cleanup may be reconsidered separately, but this plan stops.

## Correctness and equivalence gates

- Full byte equality against all pre-change SVG/report fixtures is mandatory.
- Parse both pre/post report strings with a test-only JSON parser and assert the values equal as a diagnostic in addition to exact bytes; exact bytes remain authoritative.
- Preserve tests for pool pocket path geometry, carom sights, layer ordering, transparent backgrounds, event tooltips, spin glyph attributes and heading-chevron width/opacity.
- Add a focused sequential-point test with 0, 1, 2 and 1,000 points to catch missing/extra spaces, accidental output for short polylines, point reordering and scratch concatenation.
- Add focused XML cases for all five current entity spellings and ordinary Unicode. Add JSON cases for every branch of `push_json_string`, including control characters and U+2028/U+2029.
- Assert root dimensions/orientation, element/attribute order, final `</svg>\n`, `.3`/`.6` formatting, lowercase hex and negative-zero output exactly.
- Render the same immutable rich scene concurrently on eight joined worker threads and require every complete SVG byte string to equal the single-thread golden. Run the report golden concurrently from identical source/options as well. No serializer cache, shared scratch buffer, or mutable static is permitted.

## Risks and mitigations

- **Whitespace/separator drift:** full byte fixtures and focused point tests.
- **Precision or negative-zero drift:** preserve format strings literally and include rounding/negative-zero fixtures.
- **Escaping vulnerability:** keep escaping adapters private and branch-for-branch identical; never interpolate raw external text.
- **Pocket scratch concatenation or repeated-format CPU cost:** no per-pocket heap scratch; table benchmark decides whether a single reusable scratch is warranted.
- **Reserve overflow or pathological over-allocation:** checked arithmetic, overflow fallback, <=25% fixture headroom gate; capacity never affects emitted bytes.
- **Misattributed end-to-end result:** acceptance relies on prebuilt-scene backend fixtures, not the combined trace benchmark.
- **`fmt::Write` error handling noise:** `String` formatting is infallible in practice; use the existing explicit `expect` wording consistently.

## Stop conditions, rejection and rollback

Stop and reject or narrow the implementation if any golden byte changes, escaping differs, a stage-isolated principal fixture misses its practical threshold, allocations do not meet the stated reduction, reserve headroom exceeds 25%, or any adjacent path crosses +3%. Do not compensate by changing SVG output, precision or test fixtures.

Rollback is a single independent commit: restore the prior private serializers and remove only the new fixtures/bench cases that exist solely for this candidate. No data migration or public API rollback is required.

## arm64 SIMD and GPU decision

Neither is justified. SVG/JSON emission is branchy, variable-length formatting and escaping; the optimization removes heap allocations and copies rather than accelerating a uniform numeric kernel. ARM64 SIMD would add bespoke conversion/escaping complexity while standard formatting remains dominant and exact byte spelling is mandatory. A GPU would introduce transfers, synchronization and a different execution domain for a small serial text stream. Do not add SIMD intrinsics, unsafe vector code, shaders, or GPU dependencies.

## Candidate commit message

`perf(render): stream SVG serialization into destination buffers`
