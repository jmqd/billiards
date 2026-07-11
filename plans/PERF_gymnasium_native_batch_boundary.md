# Gymnasium native batch boundary

## Status and decision

- **Status:** Accepted performance candidate; implementation is benchmark-gated.
- **Priority:** High for Gymnasium/RL callers that already use the packed batch API.
- **Confidence:** High that the current boundary performs avoidable allocation and conversion work; medium that every stage will clear the wall-time gate. A standalone smoke benchmark is now committed, but it is not yet the fresh-process paired protocol required for an acceptance decision.
- **Dependencies/order:** First repair/extend the committed boundary benchmark and freeze the public contract. Then land and measure the three implementation stages in this order: typed flat input, shared immutable batch context, direct typed result projection. Each stage must pass its own equivalence and performance gates before the next stage is measured. This work has no dependency on the core-physics performance plans.
- **Public compatibility:** Preserve `billiards_gymnasium.core.simulate_shots_batch(...)`, its array-like input convenience, and its dict of 13 arrays. This is a clean internal cutover: do not retain the nested-list native entry point, old row projection, aliases, feature flags, or dual implementations after a stage is accepted.

## Problem and evidence

`gymnasium/benchmarks/bench_native_batch.py` now exercises the public exported batch call with prepacked arrays, batch sizes `1, 8, 32, 128, 512`, workload tokens `boundary` and `mixed`, configurable samples/warmups, and an optional thread count set before importing NumPy and the extension. Existing Criterion benches still exercise Rust physics directly and omit Python normalization, PyO3 extraction, GIL handling, Rayon scheduling, row-result construction, and NumPy result creation.

The avoidable mechanisms are source-visible:

1. **Python objects are created between two already-typed representations.**
   - `gymnasium/billiards_gymnasium/core.py:105-144` builds dense `uint8`/`float64` arrays in `layouts_and_shots_to_batch_arrays`.
   - `core.py:169-178` normalizes all four arguments with `np.ascontiguousarray`, then unconditionally calls `.tolist()` on each.
   - `gymnasium/native/src/lib.rs:287-318` extracts those Python lists into four `Vec<Vec<_>>` values. This materializes Python scalars and row lists, walks them again in PyO3, and allocates one Rust vector per row even though the data was contiguous before the conversion.

2. **Immutable simulation configuration is reconstructed for every row.**
   - `gymnasium/native/src/lib.rs:418-462` creates string-bearing `BallInput`, `ShotInput`, and `SimRequest` values for each row and clones `SimConfig`.
   - `lib.rs:557-588` constructs `TableSpec::brunswick_gc4_9ft`, `BallSetPhysicsSpec::default`, `CueStrikeConfig`, `human_tuned_preview_motion_config`, `BallBallCollisionConfig::human_tuned`, and `RailCollisionProfile::default` for every request.
   - `src/lib.rs:14761-14780` shows that the table constructor parses/clones `BigDecimal` values and builds six pocket specifications.
   - `src/lib.rs:11742-11766` accepts the table, ball, motion, collision, and rail configuration by shared reference. The physics API does not require per-row ownership or mutation.

3. **The batch path builds scalar/report output that it immediately throws away.**
   - `gymnasium/native/src/lib.rs:821-911` creates `SimOutcome`, including `EventOutput`, `PocketedOutput`, `BallStateOutput`, and many event/ball/state/pocket strings.
   - `lib.rs:464-500` then maps those strings back to numeric IDs and allocates five ten-element `Vec`s per row.
   - `lib.rs:364-391` transposes rows into 13 more Rust vectors, including nested vectors.
   - `lib.rs:394-413` and `set_numpy_array` at `lib.rs:542-555` call Python `numpy.array`, causing another Python conversion and final allocation/copy.

4. **The existing concurrency structure is already the right baseline.**
   - `lib.rs:345-362` releases the GIL only after extraction/validation, uses the indexed Rayon range `0..batch_size`, and collects to an ordered `Vec`.
   - Rayon work stealing is appropriate for rows with different event counts. Manual chunks would reduce load balancing and do not remove boundary or per-row allocations.

Measured facts and hypotheses must remain distinct:

- **Measured elsewhere:** the core Criterion filters listed below have stable baselines, but none includes this binding.
- **Certain from source:** `.tolist()`, nested extraction, per-row context construction, scalar string projection, nested result vectors, and `numpy.array` conversion all occur.
- **Unmeasured hypothesis:** removing them improves end-to-end public-call latency and throughput enough to justify the dependency and code complexity. The staged benchmark gates decide that.

The committed script is strong fixture/smoke coverage, not statistical acceptance coverage. It runs all cells in one interpreter and has no baseline/candidate controller or paired fresh workers. It validates all keys, exact shapes, exact dtypes, C-contiguity, writability, and a complete deterministic output digest after every timed sample; timed regions contain only repeated public API calls. It still has no checked semantic golden, invalid-input coverage, or process-level pairing. Add those remaining pieces before treating script output as acceptance evidence; distinguish that committed extension from the future workload matrix below.

## Scope

Implement only the native batch boundary used by `billiards_gymnasium.core.simulate_shots_batch`:

1. accept typed, two-dimensional, C-contiguous NumPy inputs at the PyO3 function;
2. make exactly one defensive flat Rust copy of each of the four normalized input buffers before releasing the GIL;
3. build one immutable simulation context per public batch call and share it across Rayon workers;
4. derive only the 13 batch fields directly from `NBallSystemSimulation` and its ball metadata;
5. flatten matrix results into typed Rust buffers and construct typed NumPy results through safe rust-numpy ownership/conversion APIs, without a Python scalar/list round-trip or raw-pointer ownership;
6. extend the committed standalone public-call benchmark into the focused contract/equivalence and fresh-process paired protocol required by this cutover.

### Explicit non-goals

- Do not change the public 13-array result schema, keys, meanings, row order, or standard-ball column order.
- Do not change physics models, event ordering, arithmetic, table geometry, cue-strike behavior, or deterministic output.
- Do not remove the public wrapper's support for coercible Python array-like inputs.
- Do not read caller-owned NumPy memory after releasing the GIL. Setting `WRITEABLE=False` is not a sufficient ownership proof because a writable base or alias may still exist.
- Do not pursue zero-copy input across `Python::allow_threads`; the defensive native copy is intentional.
- Do not make a blanket zero-copy output claim or use raw-pointer/capsule construction. Safe ownership-moving conversion may be used where rust-numpy supports it, but correctness and lifetime do not depend on proving that every dtype/reshape path avoids an internal copy.
- Do not change the scalar JSON API or its rich `SimOutcome`/event report path.
- Do not replace the result dict with a structured array, record array, dataclass, or object wrapper.
- Do not add custom SIMD, NEON intrinsics, GPU code, or accelerator transfer formats.

## Public contract to freeze before implementation

### Input normalization and shape contract

The Python wrapper remains the public coercion boundary:

- `ball_ids` is normalized with `np.ascontiguousarray(value, dtype=np.uint8)`.
- `ball_xs`, `ball_ys`, and `shot_values` are normalized with `np.ascontiguousarray(value, dtype=np.float64)`.
- Coercible lists, tuples, differently typed arrays, Fortran-order arrays, non-contiguous views, and read-only arrays remain accepted. Normalization may copy when dtype/order requires it.
- Conversion failures continue to surface as the native exception raised by NumPy or `float(...)` (including `TypeError`, `ValueError`, or `OverflowError` as applicable); do not translate them into a generic native error.
- After normalization, every array must be two-dimensional. Add explicit wrapper checks so dimensionality errors are stable public `ValueError`s rather than version-specific PyO3 extraction text, and repeat defensive shape checks in Rust before slicing.

For shapes, let `ball_ids.shape == (B, M)` and `shot_values.shape == (B, C)`:

- `M >= 1`.
- `ball_xs.shape == ball_ys.shape == (B, M)`.
- `C` is exactly `2`, `3`, or `4`, matching the documented columns `[heading_degrees, speed_ips, optional tip_side_r, optional tip_height_r]`. Reject `C > 4`; the current native code silently ignores extras even though `core.py:162-163` documents `2..4`.
- `B == 0` is accepted once all four arrays satisfy the shapes above. It returns the 13 correctly typed empty arrays described below. This intentionally resolves the current loss of `M` and `C` caused by `[].tolist()`; record it as a contract clarification in the implementation change.
- Before allocation/indexing, use checked multiplication for `B * M`, `B * C`, and `B * STANDARD_BALL_COUNT`; overflow is a `ValueError`, never wraparound or panic.

Use stable, named shape messages in the wrapper/native validation and assert them in tests:

- `"ball_ids must be a 2-D array"` and the corresponding name for each other input;
- `"ball_ids must have at least one ball slot per row"`;
- `"ball_xs shape (...) must match ball_ids shape (...)"` and the corresponding `ball_ys` message;
- `"shot_values batch size ... must match ball_ids batch size ..."`;
- `"shot_values must have 2, 3, or 4 columns"`;
- `"batch dimensions are too large"` for checked-size overflow.

### Value and error contract

Shape, configuration, and simulation-domain failures returned by Rust remain Python `ValueError` through `PyValueError::new_err`; native array-construction `PyErr`s are the exception described below:

- Ball IDs are `0..=9`, with `255` meaning absent. Preserve `"unknown ball id {id}; expected 0..9 or 255 for absent"`.
- Every non-empty row must contain at least one non-absent ball and exactly one cue ball. Preserve the material messages `"at least one ball is required"`, `"exactly one cue ball is required, but none were supplied"`, and `"exactly one cue ball is required, but multiple were supplied"`.
- Preserve current row iteration semantics for duplicate object-ball IDs; do not silently introduce deduplication in a performance change. Later entries overwrite the same standard-ball output column exactly as the current projection does.
- `speed_semantics` is normalized once with the current trim/lowercase/underscore-to-hyphen rules and accepts the current token families for cue-stick-at-impact and cue-ball-launch. Preserve the material unknown-token message from `shot_from_input`.
- Missing shot columns 3 and 4 mean `tip_side_r = 0.0` and `tip_height_r = 0.0` respectively.
- Negative/invalid shot speed, invalid tip contact, invalid cue mass ratio, invalid collision energy loss, invalid initial geometry, and simulation errors remain `ValueError` with the current cause text. Hoisting validation may make an invalid batch fail before row work starts, but must not change the accepted domain.
- Structural and shared configuration validation happens before `allow_threads`. For row work, collect an indexed `Vec<Result<BatchResultRow, RowError>>` and inspect it in row order after the parallel section (or validate all row-domain failures serially first); do not rely on Rayon short-circuit timing. The same input must report the lowest failing row deterministically. Prefix row-domain errors with `"batch row {index}: "` only if the baseline tests are updated to require that uniformly.
- Preserve native `PyErr`s from result-array allocation/reshape, including `MemoryError`; do not stringify or convert them to `ValueError`. Convert only domain/shape/configuration errors to `PyValueError`.

### Output contract

Return exactly these keys; no key is optional:

| Key | Shape | Exact dtype | Sentinel/meaning |
|---|---:|---|---|
| `elapsed_seconds` | `(B,)` | `np.float64` | simulation elapsed seconds |
| `cue_pocketed` | `(B,)` | `np.bool_` | cue final state is pocketed |
| `nine_pocketed` | `(B,)` | `np.bool_` | nine final state is pocketed |
| `legal_nine_pocketed` | `(B,)` | `np.bool_` | nine pocketed, cue not pocketed, and first cue contact was the lowest present object ball |
| `first_cue_contact` | `(B,)` | `np.int64` | ball ID `0..9`, `-1` when absent |
| `lowest_object_ball` | `(B,)` | `np.int64` | lowest present object-ball ID, `-1` when none |
| `first_contact_lowest_object_ball` | `(B,)` | `np.bool_` | `false` when first contact or lowest object is absent |
| `event_count` | `(B,)` | `np.int64` | exact number of simulation events |
| `pocketed_mask` | `(B, 10)` | `np.bool_` | columns cue, one, ..., nine |
| `final_state` | `(B, 10)` | `np.int64` | `0=absent`, `1=on_table`, `2=pocketed` |
| `final_x` | `(B, 10)` | `np.float64` | final/capture `Position::x().as_f64()`, `NaN` only for absent columns |
| `final_y` | `(B, 10)` | `np.float64` | final/capture `Position::y().as_f64()`, `NaN` only for absent columns |
| `final_pocket` | `(B, 10)` | `np.int64` | `0..5` in current `TopRight, CenterRight, BottomRight, BottomLeft, CenterLeft, TopLeft` order; `-1` otherwise |

The integer dtype is deliberately `int64`, not the narrow Rust intermediates currently named in `BatchSimRow`: `numpy.array` currently exposes Python integers as 64-bit arrays. The cutover must not silently narrow public results to `int16`/`int32`.

Every output is C-contiguous and writable. Matrix strides are the ordinary C-order strides for `(B, 10)`. Arrays must remain valid when the returned dict and all input arrays are dropped. Do not promise `array.flags.owndata`, a specific NumPy base object, or absence of an internal safe-constructor copy. The observable ownership contract is independent lifetime, no alias to any input or sibling output, and no Rust pointer retained after return.

For a valid non-empty input, results are bitwise equal to the old batch implementation for every key, including exact floating bits and `NaN` placement. Rayon preserves row order only because results are collected/indexed and flattened in input order; work-stealing completion order is not observable. Do not weaken comparison to a tolerance unless an independently reviewed arithmetic change requires it.

For `B == 0`, scalar fields have shape `(0,)`, matrix fields have shape `(0, 10)`, all use the dtypes above, and all are C-contiguous.

## Dependency and version decision

`gymnasium/native/Cargo.toml` currently uses `pyo3 = { version = "0.23", features = ["abi3-py39"] }`; `gymnasium/native/Cargo.lock` resolves PyO3 `0.23.5`. It has no rust-numpy dependency. `gymnasium/pyproject.toml` requires Python `>=3.10` and NumPy `>=2,<3`.

Add:

```toml
numpy = "0.23"
```

and update `gymnasium/native/Cargo.lock`. rust-numpy `0.23` depends on `pyo3 ^0.23.0`, so it is the compatible series for the existing binding and should resolve to the same PyO3 `0.23.5` rather than introducing a second PyO3 runtime. It also introduces `ndarray` transitively. Do not upgrade PyO3, change `abi3-py39`, widen the Python NumPy range, or add an explicit `ndarray` dependency in this performance change.

Use the non-deprecated rust-numpy 0.23 APIs:

- `PyReadonlyArray2<'py, u8>` / `PyReadonlyArray2<'py, f64>` and `as_slice()` for typed contiguous input;
- safe typed array constructors such as `IntoPyArray::into_pyarray` where the concrete buffer type supports the required ownership conversion;
- `PyArrayMethods::reshape([B, STANDARD_BALL_COUNT])` for flat matrix buffers, propagating its `PyResult`.

rust-numpy's safe conversion APIs establish the result lifetime without raw pointers. Where `IntoPyArray` consumes a `Vec<T>`, use that ownership transfer, but do not generalize it into a zero-copy promise for every dtype or reshape implementation and do not make performance correctness depend on `flags.owndata`. Destructive resize is not part of the result contract. Keep the current `abi3` feature and verify release builds/imports on the supported Python/NumPy matrix, especially NumPy 2.0 and the latest NumPy 2.x. Reject the dependency stage if it produces duplicate PyO3 versions, breaks the abi3 wheel/import path, or requires an unrelated PyO3 upgrade.

Relevant upstream API evidence:

- rust-numpy 0.23 crate and dependency list: <https://docs.rs/numpy/0.23.0/numpy/>
- `PyReadonlyArray::as_slice`: <https://docs.rs/numpy/0.23.0/numpy/borrow/struct.PyReadonlyArray.html#method.as_slice>
- safe `IntoPyArray` ownership conversion: <https://docs.rs/numpy/0.23.0/numpy/convert/trait.IntoPyArray.html>
- reshape API: <https://docs.rs/numpy/0.23.0/numpy/array/trait.PyArrayMethods.html#method.reshape>

## Implementation plan

### Preliminary: repair the benchmark and freeze the contract

1. Keep `gymnasium/benchmarks/bench_native_batch.py` as the committed smoke fixture. Preserve its complete post-timing digest and exact key/shape/dtype/contiguity/writability validation. Add checked semantic reference digests for both workloads and extend it with the fresh-process controller/worker/compare modes below; do not present the current one-process loop as that protocol.
2. Extend `gymnasium/tests/test_native.py` with the output/dtype/shape/error contract before changing the binding. Where the clarified `B == 0`, `C <= 4`, and exact dtype contract differs from current behavior, keep those assertions in a clearly identified cutover test rather than pretending they describe the old implementation.
3. Save benchmark JSON and Criterion baselines from the unchanged implementation. Benchmark fixtures and complete output validation stay outside timed loops.

### Stage 1: typed contiguous input with one defensive copy

1. Keep the four `np.ascontiguousarray` calls in `core.py`. Add explicit two-dimensional checks, then pass the arrays themselves to `_native.simulate_shots_batch`; delete all four `.tolist()` calls.
2. Change the PyO3 signature from four `Vec<Vec<_>>` arguments to:

   ```rust
   ball_ids: PyReadonlyArray2<'py, u8>,
   ball_xs: PyReadonlyArray2<'py, f64>,
   ball_ys: PyReadonlyArray2<'py, f64>,
   shot_values: PyReadonlyArray2<'py, f64>,
   ```

3. While holding the GIL, read each shape and enforce the contract. Obtain each C-contiguous slice with `as_slice()` and copy it exactly once with `to_vec()` into a flat Rust-owned buffer. Do not call `to_owned_array()` and then copy again. Treat a surprising non-contiguous array as `ValueError`, even though the public wrapper should already have normalized it.
4. Introduce a private owning value with checked dimensions, for example:

   ```rust
   struct BatchInputs {
       batch_size: usize,
       max_balls: usize,
       shot_cols: usize,
       ball_ids: Vec<u8>,
       ball_xs: Vec<f64>,
       ball_ys: Vec<f64>,
       shot_values: Vec<f64>,
   }
   ```

5. Complete all interaction with `PyReadonlyArray` before `allow_threads`. Move only `BatchInputs`, normalized shared scalar values, and Rust-owned data into the closure. No Python handle, NumPy view, borrowed slice into Python memory, or raw NumPy pointer may cross the GIL release.
6. Keep `0..batch_size.into_par_iter()` and one ordered slot per input row. Collect row results without exposing completion order; if failures remain possible in workers, materialize indexed results and choose the lowest failing index deterministically after parallel work. Do not introduce manual chunks or change the global Rayon pool.
7. Run equivalence/error tests and the stage-1 benchmark against the saved baseline. Keep this stage only if it clears the gate below.

**Why the copy is required:** `PyReadonlyArray` prevents a rust-numpy mutable borrow in the same process, but it is not proof that every Python/base/native alias is immutable. Another Python thread could mutate through an alias after the GIL is released. A bulk snapshot into four flat Rust vectors makes Rayon reads ordinary immutable Rust reads and prevents a data race/undefined behavior during simulation. The caller must still avoid concurrent mutation during the initial public call/copy, as with ordinary NumPy consumers; after the copy completes, mutations cannot affect the batch. This plan makes no zero-copy input claim.

### Stage 2: one shared immutable simulation context per batch

1. Add a private `BatchSimulationContext` containing the per-call immutable values:

   ```rust
   struct BatchSimulationContext {
       table: TableSpec,
       ball_set: BallSetPhysicsSpec,
       cue_strike: CueStrikeConfig,
       motion: OnTableMotionConfig,
       collision_config: BallBallCollisionConfig,
       rail_profile: RailCollisionProfile,
       speed_semantics: BatchSpeedSemantics,
   }
   ```

   `BatchSpeedSemantics` is a private two-variant enum produced by running the current token normalization once. `CollisionModel::ThrowAware` and `RailModel::SpinAware` may remain explicit constants at the call site rather than fields.
2. Validate/construct `CueStrikeConfig` and the normalized speed semantics once. Construct the table, ball set, motion config, collision config, and rail profile once before entering the row map. Return a batch-level `ValueError` immediately if shared configuration is invalid.
3. Pass `&BatchSimulationContext` to each Rayon row. Rust must prove the captured shared references are `Sync`; do not use `unsafe`, interior mutability, `Mutex`, per-worker clones, thread-local storage, or a global lazy context.
4. Split the scalar path from the batch path. The scalar JSON path continues to use `SimRequest` and `simulate_request`. The batch row builds `Ball`/`BallType`, cue index, `Shot`, and initial state directly from its flat numeric slices and the shared context; it must not create ball names, `"inches"`, speed-semantics strings, `BallInput`, `ShotInput`, or `SimRequest`.
5. Call `simulate_n_balls_with_physics_and_pockets_on_table_until_rest` with shared references from the context and propagate its current typed error as the same Python `ValueError` cause. Row-local balls, states, simulation, and result remain owned by that worker.
6. Preserve per-call construction rather than making configuration global. This keeps future call-specific configuration isolated and avoids global initialization/error semantics.
7. Run tests and compare stage 2 against stage 1 with the same saved fixture data and fixed thread counts. Reject this stage independently if `O(B)` to `O(1)` construction does not yield a practical gain.

### Stage 3: direct batch projection into typed flat result storage

1. Replace `BatchSimRow`'s five heap vectors with a row-local fixed representation:

   ```rust
   struct BatchResultRow {
       elapsed_seconds: f64,
       cue_pocketed: bool,
       nine_pocketed: bool,
       legal_nine_pocketed: bool,
       first_cue_contact: i64,
       lowest_object_ball: i64,
       first_contact_lowest_object_ball: bool,
       event_count: i64,
       pocketed_mask: [bool; STANDARD_BALL_COUNT],
       final_state: [i64; STANDARD_BALL_COUNT],
       final_x: [f64; STANDARD_BALL_COUNT],
       final_y: [f64; STANDARD_BALL_COUNT],
       final_pocket: [i64; STANDARD_BALL_COUNT],
   }
   ```

   Initialize absent state as `false`, `0`, `NaN`, and `-1` exactly as in the output contract. Use a checked `usize -> i64` conversion for event count.
2. Add a batch-only projector that consumes/borrows `NBallSystemSimulation`, the row's `BallType` metadata, and cue index. It must:
   - scan events in existing order to find the first cue/object `BallBallCollision` and count all events;
   - find the lowest initial object-ball number using the same cue exclusion as `ball_number`;
   - scan final states in ball input order and write standard-ball columns directly;
   - read on-table positions and pocket capture positions exactly as the scalar projector does;
   - map `Pocket` enum variants directly to the existing `0..5` order;
   - derive scratch/nine/legal-nine flags from those same final states and first-contact facts.
3. Do not call `outcome_from_simulation`, `event_output`, `ball_type_name`, `pocket_name`, `optional_ball_name_to_id`, or `pocket_id_from_name` from the batch path. Leave them for scalar/report consumers. This avoids all batch-only strings and reverse string mapping without changing scalar output.
4. Ordered/indexed Rayon collection returns one `BatchResultRow` or row error per input index. Still inside `allow_threads`, choose the lowest row error deterministically or flatten successful rows once into a `BatchOutputBuffers` struct containing 13 typed vectors with exact capacities (`B` or checked `B * 10`). Workers do not share mutable output; the only shared values are immutable inputs/context.
5. Reacquire the GIL only after flat buffers are complete. Construct each typed NumPy array through safe rust-numpy APIs. Reshape the five `B * 10` buffers to `[B, 10]` with `reshape(...)?`; never `assert` or panic on reshape. Insert those arrays into `PyDict` directly. Delete `set_numpy_array`, the Python `import("numpy")`, nested result vectors, and the old row transpose.
6. Do not allocate Python scalar/list intermediates or invoke Python `numpy.array`/`np.asarray` for results. Do not use raw pointers or require a blanket no-copy guarantee; allocation profiling must establish the actual behavior of the selected safe constructors.
7. Run all equivalence/ownership tests and compare stage 3 against stage 2. Profile allocations to confirm that event/name strings, per-row ten-element heaps, nested transpose vectors, and Python-object result conversion are gone.

## Concurrency and ownership invariants

The implementation is acceptable only if all of these are obvious in the types and control flow:

1. Python/NumPy input is inspected and copied while the GIL is held.
2. The `PyReadonlyArray` values and their slices are not captured by `allow_threads`.
3. `BatchInputs` owns every byte Rayon reads. Its flat vectors are never mutated after the parallel map begins.
4. `BatchSimulationContext` is constructed once, is immutable, and is borrowed by workers through scoped Rayon execution. No `Arc` is necessary unless an API boundary genuinely requires `'static`; do not add one merely for shared reads.
5. Every worker owns its balls, initial states, simulation, metadata, and `BatchResultRow`. There is no shared mutable result buffer and therefore no unsafe disjoint-slice bookkeeping.
6. Input indices, not worker completion order, determine output row order and the lowest reported row error.
7. Flat `BatchOutputBuffers` are complete before Python array creation. Safe rust-numpy constructors own or copy from those buffers according to their documented API; Rust retains no pointer or alias after return.
8. No Python C API, NumPy API, Python exception construction, or Python reference-count operation occurs while the GIL is released.
9. Panics are not used for input, shape, reshape, allocation, or simulation failures. Domain failures become the specified `ValueError`; native array-construction `PyErr`s propagate unchanged.

## Benchmark: standalone public-boundary protocol

The committed `gymnasium/benchmarks/bench_native_batch.py` is a one-interpreter smoke runner. Its `--threads` option is set before imports, so a separately launched invocation can select Rayon initialization safely, but changing that option cannot create a new pool after the extension has been used. It does not spawn workers, pair revisions, randomize AB/BA order, or isolate cells. Extend it with a parent/controller mode that launches a **fresh interpreter for every revision/workload/thread/batch/pair observation**, a private one-cell worker mode, and a compare mode. The controller must set `RAYON_NUM_THREADS` in each child environment before the child imports `billiards_gymnasium`; the worker must not mutate thread count after import or run multiple thread-count cells.

### Committed smoke workloads and future acceptance fixtures

The current script directly preconstructs `(B, 10)`/`(B, 4)` arrays for `B in [1, 8, 32, 128, 512]`:

1. CLI `boundary`: cue only, zero speed, with row-varying positions/headings.
2. CLI `mixed`: cue plus an active prefix of 1–9 object balls on a deterministic grid, with deterministic heading/speed/tip cycles.

Retain these as smoke coverage, but do not claim they satisfy the intended workload diversity until untimed validation proves active-ball and event-count variation. For acceptance, make the semantics explicit (the CLI tokens may remain stable):

1. **`boundary_light` (`boundary`):** each row contains only cue ball ID `0` at `(25.0, 50.0)`, nine `255` slots, heading `90.0`, speed `0.0`, and zero tip offsets.
2. **`mixed_duration` (`mixed`):** hard-code one valid ten-ball, non-overlapping layout in table inches, then cycle active prefix counts `2..10`. Preserve cue in slot 0 and fill inactive slots with `255`. Cycle deterministic headings, speeds, and tip offsets from fixed literal tables chosen to include direct contacts, misses, rails/pockets, and different event counts. At least use the repository's validated direct rows `(cue 10,50; one 25,50; heading 90; speed 128)` and `(cue 10,50; one 25,50; nine 37.5,50; heading 90; speed 180)` from `gymnasium/tests/test_native.py`; add fixed non-overlapping positions for IDs 2..8. Assert the fixture spans 2 through 10 active balls and at least three distinct event counts before timing.

Build arrays directly with exact target dtypes and assert `C_CONTIGUOUS`. Reuse identical arrays across all samples for a worker cell. Do not generate random values, call an environment, pack layout dictionaries, validate/hash outputs, import modules, or allocate fixture arrays inside the timed loop.

### Timing mechanics

- Canonical fixed thread counts are exactly `RAYON_NUM_THREADS=1` and `RAYON_NUM_THREADS=8`. The controller places one value in each fresh worker's inherited environment before import and records it. Do not substitute the machine default, run two counts in one interpreter, or compare unlike counts.
- Build/install each measured revision with `cd gymnasium && maturin develop --release` into its own otherwise equivalent virtual environment. Use the same Python patch version, NumPy version, Rust toolchain, compiler flags, target, power mode, and host.
- Record revision label/commit, dirty-state declaration, Python, NumPy, rustc, cargo, maturin, OS, CPU model, physical/logical core counts, and power/thermal mode in metadata. Benchmark artifacts are not committed.
- For each revision/workload/thread-count/batch-size cell, launch **30 independent one-cell worker processes**. Randomize paired baseline/candidate execution as AB or BA per pair using a saved deterministic controller seed. A pair runs back-to-back on the same host; a worker exits after emitting exactly one JSON record.
- In a worker, import after inheriting the thread environment, construct and validate its one fixture, make at least five unmeasured warm-up calls, then calibrate an integer iteration count so one timed sample lasts at least `0.5 s`. Time only repeated complete public `billiards_gymnasium.core.simulate_shots_batch` calls with `time.perf_counter_ns`; assigning each complete result to a module-level sink is the only per-iteration consumption. Report nanoseconds/call, calls/s, and shots/s.
- After timing, consume every key and every byte of the last complete output into a deterministic digest and compare exact key/dtype/shape/stride/content to the untimed expected output. This is the Python equivalent of black-boxing the complete result and prevents a benchmark from accidentally timing a partial API.
- Store one JSON Lines record per process/cell. The compare mode verifies exact metadata compatibility before computing statistics; incompatible records fail rather than being pooled.

Example controller interface to implement:

```text
python gymnasium/benchmarks/bench_native_batch.py capture-paired \
  --baseline-python /path/to/baseline-venv/bin/python \
  --candidate-python /path/to/candidate-venv/bin/python \
  --baseline-label baseline --candidate-label candidate \
  --threads 1,8 --batches 1,8,32,128,512 --pairs 30 \
  --seed 20260711 \
  --baseline-output /tmp/native-batch-baseline.jsonl \
  --candidate-output /tmp/native-batch-candidate.jsonl

python gymnasium/benchmarks/bench_native_batch.py compare \
  --baseline /tmp/native-batch-baseline.jsonl \
  --candidate /tmp/native-batch-candidate.jsonl
```

`capture-paired` must choose and save AB or BA order independently for each pair/cell, then launch the corresponding baseline and candidate worker interpreters back-to-back. The private worker subcommand imports the package only after inheriting its fixed `RAYON_NUM_THREADS`; users do not invoke it directly. Preserve the pair ID, order, controller seed, and executable identity in both JSON records.

### Statistics and stage gates

For each workload/thread/B cell, report paired candidate/baseline ratios for latency and shots/s, median, MAD, and a deterministic bootstrap 95% confidence interval for the paired median ratio (at least 10,000 resamples, saved seed). Also show raw process observations; do not treat calibrated inner iterations as independent samples.

Apply these exact gates separately to each stage and to the final cumulative candidate:

1. Same host/toolchain/config and at least 30 valid paired process observations per cell; otherwise no decision.
2. **Practical win:** in at least one primary cell (`B` 32, 128, or 512, either workload, matching thread count), the median shots/s ratio is at least `1.05` and the bootstrap 95% CI lower bound is greater than `1.00`.
3. **No meaningful regression:** in no workload/thread/B cell may the median shots/s ratio be below `0.97` with the bootstrap 95% CI upper bound below `1.00`.
4. **Stage-specific evidence:** stage 1 must win a `boundary_light` primary cell; stage 2 must win a `boundary_light` or `mixed_duration` primary cell; stage 3 must win a `mixed_duration` primary cell. A gain caused only by noise in an unrelated cell is insufficient.
5. If a stage fails its specific practical-win gate, revert that stage before measuring the next one. Do not keep source-complexity changes merely because they reduce allocations.

### Adjacent-path and allocation/profile guards

The binding-only change should not alter core physics. Still save and compare Criterion output on the same host with at least three paired process runs per revision for these existing filters:

- `core_functions/compute_next_transition_on_table/sliding`;
- `throughput_functions/compute_next_transition_on_table/10000`;
- `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/1000`;
- `end_to_end/direct/pocket_aware_until_rest_cached`.

Use Criterion's reported 95% confidence intervals. Reject a stage if any adjacent filter shows a reproducible regression of at least 3% whose paired/replicated 95% evidence excludes no change. These are regression guards only; they cannot prove boundary speedup.

For allocation corroboration, add an untimed benchmark option that performs one warmed public call under `tracemalloc` and reports Python allocation count/peak bytes, plus an opt-in macOS Allocations/Instruments run of the same fixed cell (`boundary_light`, `B=128`, one Rayon thread). Stage 1 should remove `.tolist()` scalar/list allocation; stage 3 should remove batch event/name strings, five row heaps, nested transpose storage, and Python `numpy.array` conversion. Do not run allocation instrumentation in latency samples, and do not reject a wall-time winner solely because RSS allocator caching obscures peak-memory improvement.

## Correctness and equivalence tests

Add focused tests under `gymnasium/tests/test_native.py` and private Rust unit tests where ownership/indexing is easier to isolate.

### Valid-input equivalence

For each fixture below, derive expected batch fields from scalar `simulate_shot` outcomes and compare all 13 keys with `np.testing.assert_array_equal(actual, expected, equal_nan=True)`, plus exact key set, dtype, shape, C-contiguity, and strides:

- no-event cue-only and cue/object miss;
- direct object pocket;
- cue scratch;
- legal nine and illegal nine;
- missing/non-prefix object slots;
- two-, three-, and four-column `shot_values` defaults;
- multiple rows with deliberately different event counts, proving input order is retained under Rayon;
- all ten standard-ball columns;
- `B=1` and `B=0`.

Repeat a mixed batch many times with thread counts 1 and 8 in separate subprocesses and require bitwise-identical complete arrays. This tests determinism/order, not speed.

### Normalization and ownership

- Pass Python nested lists, `float32` coordinates/shots, signed integer IDs that fit `uint8`, Fortran arrays, strided/reversed non-contiguous views, and read-only arrays. Compare them to explicitly normalized C-contiguous inputs.
- Assert inputs are unchanged after the call.
- Keep individual output arrays, delete the result dict and all inputs, force Python garbage collection, and verify every retained output still has the expected bytes. Mutating an output must not affect any input or sibling output.
- Verify every matrix array is writable, C-contiguous, shape `(B, 10)`, and uses independent storage from every input.

### Invalid inputs and errors

Assert exception class and the stable/material message for:

- each one-dimensional and three-dimensional argument;
- `M=0` for a non-empty and empty batch;
- row-count mismatch, ball-column mismatch, and shot columns 0, 1, and 5;
- an uncoercible dtype/object and ragged Python input;
- ball ID 10 and 254;
- all-absent row, missing cue, and duplicate cue;
- unknown speed semantics including whitespace/underscore normalization controls;
- negative speed and invalid tip offsets;
- invalid `cue_mass_ratio` and `collision_energy_loss`;
- a row-specific invalid initial geometry in a multirow batch, asserting deterministic lowest-row reporting;
- checked dimension overflow through a private Rust constructor test (do not attempt an impossible Python allocation).

### Native structural tests

- Flat row slicing/indexing returns the exact intended row and never crosses row boundaries.
- `BatchSimulationContext` construction normalizes speed semantics and returns shared validation failures once.
- Direct batch projection matches `outcome_from_simulation`-derived values for first contact, lowest object, scratch, legal nine, on-table/pocketed states, pocket IDs, absent sentinels, and `NaN`s.
- Flattening `Vec<BatchResultRow>` yields exact scalar/matrix lengths and row-major order for zero, one, and multiple rows.
- Result conversion produces exact `int64`, `float64`, and `bool` arrays without calling Python `numpy.array`.

## Verification sequence for implementation

Run only after each implementation stage is complete:

1. Native unit tests:
   `cargo test --manifest-path gymnasium/native/Cargo.toml`
2. Release extension and focused Python tests:
   `cd gymnasium && maturin develop --release && python -m pytest tests/test_native.py -q`
3. Dependency check: confirm one PyO3 0.23.x in the native dependency graph and inspect the updated lockfile; build/import against Python 3.10 and the project's current Python, with NumPy 2.0 and latest 2.x.
4. Capture the fixed 1/8-thread, 1/8/32/128/512 batch benchmark for baseline and candidate, then run `compare`.
5. Run the four exact Criterion regression filters with saved baselines and at least three paired process runs.
6. Run untimed allocation/profile corroboration for the specified `B=128`, one-thread light cell.
7. Before accepting the final cutover, rerun the complete output equivalence suite with `RAYON_NUM_THREADS=1` and `8` in fresh processes.

## Risks and controls

- **Concurrent mutation/undefined behavior:** retaining a NumPy view after GIL release would be unsound. Control: copy each contiguous input slice once into Rust ownership and capture only those vectors.
- **Hidden dtype break:** direct `i16`/`i32` arrays would narrow today's inferred integer results. Control: make all public integer buffers `i64` and assert exact dtypes.
- **Shape information for empty batches:** nested lists currently erase `(0, M)`/`(0, C)`. Control: typed 2-D arrays retain dimensions; explicitly accept and test empty outputs.
- **Result semantic drift:** bypassing `SimOutcome` can change first contact, legal-nine logic, pocket order, overwrite behavior, or `NaN` placement. Control: derive both paths on adversarial fixtures and require exact equality for all keys.
- **Error-order drift under parallelism:** multiple bad rows could race. Control: validate cheap row invariants serially or preserve indexed ordered collection and explicitly choose the lowest failing row.
- **Result lifetime/allocator mismatch:** output storage must be released by the owner established by the selected safe rust-numpy constructor, whether that constructor moves or copies a concrete buffer. Control: use safe typed APIs only, retain no raw pointer/alias, and test lifetime after dropping inputs/dict.
- **Dependency/ABI risk:** rust-numpy adds the NumPy C API to an abi3 extension. Control: keep matching 0.23 series, prove one PyO3 version, and build/import on the supported Python/NumPy matrix before accepting the stage.
- **GIL regression:** flattening or simulation could accidentally move under the GIL. Control: keep copy/validation before `allow_threads`, simulation plus Rust-only flattening inside it, and only array object creation afterward.
- **Load imbalance:** manual chunks can strand a long simulation. Control: retain one indexed Rayon item per row and work stealing.
- **Complexity without value:** source-visible allocations may be small beside physics. Control: independent stage commits and mandatory practical wall-time gates.

## Stop, rejection, and rollback criteria

Stop or reject the affected stage if any of the following occurs:

- exact output, shape, dtype, error-domain, or deterministic row-order equivalence cannot be maintained;
- any Python/NumPy memory or reference must remain borrowed after releasing the GIL;
- implementation requires `unsafe` ownership/index partitioning, manual Rayon chunks, thread-local contexts, or a PyO3 major/minor upgrade;
- rust-numpy 0.23 cannot build/import with the existing abi3 and NumPy 2.x support matrix or introduces duplicate PyO3 runtimes;
- the stage misses its stage-specific 5% practical benchmark gate;
- any benchmark cell or adjacent Criterion filter crosses the defined regression guardrail;
- direct projection becomes a second general report model rather than a small batch-only numeric projector.

Rollback is stage-local: restore the prior measured implementation, remove helpers/dependencies made unused by that rejected stage, and rerun the focused contract test. If stage 1 is rejected, remove rust-numpy and the lockfile additions. Do not leave dormant code paths, flags, aliases, or compatibility shims.

## SIMD and GPU decision

**arm64 SIMD/NEON: reject for this target.** The boundary copies are already contiguous native memory operations handled by NumPy/Rust/libc. The expensive work invoked per row is an irregular event-driven simulation with variable event counts and branches across collision, rail, pocket, jaw, and motion-phase cases. No dominant regular arithmetic loop has been isolated here. Custom NEON for a four-buffer copy or 10-column projection would add platform code without evidence that it dominates optimized memcpy and scalar stores.

**GPU: reject for this target.** A GPU implementation would require transferring inputs/results and redesigning the branch-heavy, variable-length core event scheduler. The accepted work is specifically boundary/configuration/result ownership around the current CPU simulator; no measured accelerator-suitable kernel exists. Revisit SIMD or GPU only if a later profile identifies a concrete regular kernel that dominates total public-call time and has its own equivalence benchmark.

## Candidate commit message

`perf(gymnasium): streamline native batch buffers and row projection`
