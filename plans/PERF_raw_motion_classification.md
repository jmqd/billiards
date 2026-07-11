# Raw on-table motion classification

## Status, priority, confidence, and order

- **Status:** Accepted for implementation, subject to the equivalence and benchmark gates below.
- **Priority:** P1. This removes exact-unit reconstruction from a measured hot path used by transition and event prediction.
- **Confidence:** High that the work removes real overhead; medium-high that the end-to-end caller benchmarks will clear the required thresholds.
- **Dependencies:** The committed `motion_phase_classification/{rest,spinning,rolling,sliding}` fixtures establish the four ordinary branch baselines. Add the missing airborne and threshold-boundary cells, plus the non-sliding transition cells, and save a full unchanged-code baseline before changing `src/lib.rs`. No production dependency is required.
- **Order:** Land this before broader current-phase prediction-context or prepared-pocket-geometry work, because those plans touch several of the same predictor call sites. Measure this change as its own commit so its effect is not attributed to caching or geometry changes. The collision-refinement-stall change is logically independent; benchmark it separately after this cutover if both are developed on one branch.

## Problem and evidence

### Measured facts

The repository's unchanged-tree quick Criterion snapshots report:

- `core_functions/compute_next_transition_on_table/sliding`: approximately **8.62–8.77 us/call**.
- `throughput_functions/compute_next_transition_on_table/10000`: approximately **105,000 transitions/s**.
- `motion_phase_classification/rest`: **1.552–1.566 us/call**.
- `motion_phase_classification/spinning`: **1.993–2.004 us/call**.
- `motion_phase_classification/rolling`: **3.405–3.423 us/call**.
- `motion_phase_classification/sliding`: **3.600–3.621 us/call**.

The last four are quick current-tree baselines from the already committed direct classifier fixtures, not paired acceptance measurements and not evidence of a future speedup.

A macOS `sample` profile of the sliding-transition benchmark placed **3,675/4,076 samples** below `compute_next_transition_on_table` and **1,269 samples** below `classify_motion_phase`. The profile attributed substantial work to BigDecimal formatting/parsing/allocation on the exact-unit conversion path. These are measurements of the current implementation, not estimates of the eventual speedup.

### Source-grounded facts

The exact current symbols are all in `src/lib.rs`:

- `RawOnTableBallState` and `RawOnTableBallState::{from_on_table,speed,cloth_contact_velocity,cloth_contact_speed}` at lines 4841–4888 already represent planar position, velocity, and angular velocity as seven `f64` values.
- `classify_motion_phase` at lines 5250–5303 first applies `is_airborne`, then classifies `Rest`, then `Spinning`, and only then chooses `Rolling` or `Sliding` from cloth-contact slip.
- Its linear-speed path calls `ball_speed`/`Velocity2::speed`; `Velocity2::speed` at lines 606–610 converts the norm back into an exact `InchesPerSecond`, after which the classifier immediately calls `as_f64()`.
- Its rolling/sliding path calls `try_on_table_state` at lines 5351–5355, which clones the whole `BallState` and normalizes it through `OnTableBallState::try_new_with_thresholds`. It then calls `cloth_contact_speed_on_table`, whose lines 5197–5228 construct exact-unit velocity components and an exact-unit magnitude before the classifier again calls `as_f64()`.
- The validated on-table hot callers construct `RawOnTableBallState` independently immediately before or after classification: `compute_next_transition_on_table` (5379–5390), `compute_next_ball_ball_collision_during_current_phases_on_table` (6234–6332), `compute_next_ball_rail_impact_on_table` (6460–6527), `side_pocket_post_jaw_path_reaches_centerline_inside_capture_region` (7992 onward), `compute_next_ball_jaw_impact_on_table` (9041–9116), and `compute_next_ball_pocket_capture_on_table` (9135 onward). `advance_within_phase_on_table` and `advance_motion_on_table` also classify and then enter raw advancement.
- Other validated on-table consumers use the general classifier even when they only need a phase: the resting-state validation around line 3278, `trace_ball_path_with_rail_profile_on_table` at line 11874, `estimate_post_contact_cue_ball_curve_on_table` at line 12772, and `estimate_post_contact_cue_ball_bend_on_table` at line 13004.
- `OnTableBallState::try_new_with_thresholds` at lines 3021–3045 zeroes height and vertical velocity after accepting threshold-sized noise. Therefore the public general-state classifier and a classifier whose input is already `OnTableBallState` are related but not interchangeable without explicitly preserving the general-state vertical-speed checks.

### Source inference to validate

It is a source-based inference, supported by the profile but not yet isolated by a branch benchmark, that constructing the raw state once and classifying from scalar components will remove most of the classifier's exact-unit conversion/allocation cost. It is also an inference that LLVM cannot reliably eliminate this work across BigDecimal-backed unit constructors. The benchmark and profile gates below must confirm both claims.

A crucial numerical detail is not optional: current `Velocity2::speed` evaluates `(x.powi(2) + y.powi(2)).sqrt()`, while `RawOnTableBallState::speed` uses `x.hypot(y)`. Those expressions can round differently. The new classifier must **not** reuse `RawOnTableBallState::speed` or `cloth_contact_speed` if doing so changes the current expression. Classification-specific scalar norms must preserve the current `powi`/sum/`sqrt` evaluation order so the optimization removes representation round-trips without changing boundary decisions.

## Scope

Implement one private raw-state construction/classification path for callers that already hold `OnTableBallState`, and make the public `classify_motion_phase(&BallState, ...)` a semantics-preserving wrapper around the scalar classifier after its airborne decision.

Observable contracts to preserve exactly:

1. Public phase precedence is **Airborne → Rest → Spinning → Rolling → Sliding**.
2. Airborne means `height > airborne_height` **or** `abs(vertical_velocity) > airborne_vertical_speed`; equality remains non-airborne.
3. Rest includes the independent `rest_vertical_speed` check.
4. Spinning requires near-zero linear speed and horizontal spin and non-near-zero `wz`; as today, this branch has no separate rest-vertical-speed predicate after the airborne check.
5. `ExactNoSlip` remains `contact_speed <= f64::EPSILON`.
6. `Thresholded` remains `contact_speed <= contact_speed_epsilon`.
7. All caller-visible `Option` shapes, phase labels, transition times, predicted impact/capture times, states, event source/index ordering, and deterministic tie behavior remain unchanged.

## Explicit non-goals

- No change to the cloth, rolling, spin-decay, collision, rail, jaw, or pocket physics models.
- No change to thresholds, comparison operators, event tolerances, or event precedence.
- No persistent phase cache, prepared prediction context, raw config object, or cross-event invalidation scheme; those belong to separate plans.
- No root-storage, root-finding, collision-refinement, or pocket-search work.
- No public raw-state API and no compatibility alias for a superseded private helper.
- No conversion of public exact-unit types to `f64` fields and no relaxation of public airborne behavior.
- No arithmetic reassociation, `fast-math`, approximate norm, or tolerance introduced to obtain a benchmark win.

## Implementation design and clean cutover

### 1. Complete and save the branch baselines first

Keep the committed `motion_phase_classification/{rest,spinning,rolling,sliding}` cells and add only the missing cells specified below while production code is unchanged. Add the missing transition-branch fixtures at the same time. Build every state/config outside Criterion's timed closure. Save baseline data and record each fixture's expected `MotionPhase`, `Option` shape, time bits, and output-state bits before the implementation change.
### 2. Make raw construction canonical

Add one private scalar constructor on `RawOnTableBallState` that reads planar position, planar velocity, and all angular components from `&BallState`. Make construction from `&OnTableBallState` delegate to that constructor, or remove `from_on_table` and use the canonical constructor through `state.as_ball_state()` everywhere. There must be only one field-extraction implementation after the cutover.

Do not add height or vertical velocity fields to `RawOnTableBallState`; they are not used by on-table integration. Pass the general state's vertical speed explicitly to classification where its existing Rest semantics require it.

### 3. Add one scalar classifier with exact precedence

Introduce a private helper with this effective shape:

```rust
fn classify_raw_on_table_motion_phase(
    state: RawOnTableBallState,
    vertical_speed: f64,
    radius: f64,
    config: &MotionPhaseConfig,
) -> MotionPhase
```

The helper handles only `Rest`, `Spinning`, `Rolling`, and `Sliding`, in that order. It must use:

- a classification-only planar norm equivalent to `(vx.powi(2) + vy.powi(2)).sqrt()`;
- contact components in the current order, `vx - radius * wy` and `vy + radius * wx`;
- a classification-only contact norm equivalent to `(contact_vx.powi(2) + contact_vy.powi(2)).sqrt()`;
- the exact existing `abs() <= threshold` predicates and exact existing `SlidingToRollingModel` comparisons.

Do not call `RawOnTableBallState::speed()` or `.cloth_contact_speed()` from this helper unless bitwise differential tests first prove those implementations equivalent over the required corpus; their current `hypot` arithmetic is not generally bitwise equivalent.

Add a single private validated entry point with this effective data flow:

```rust
fn raw_on_table_state_and_phase(
    state: &OnTableBallState,
    radius: f64,
    config: &MotionPhaseConfig,
) -> (RawOnTableBallState, MotionPhase)
```

It constructs the raw value once, calls the scalar classifier with `vertical_speed = 0.0`, and returns both. A small private phase-only wrapper may discard the raw value for validated consumers that do not otherwise need it, but it must delegate to this same path rather than recreate a second convention.

### 4. Preserve the public general-state wrapper

Keep `classify_motion_phase` public with its current signature. Its flow must be:

1. Run the current `is_airborne(state, &config.thresholds)` first and immediately return `MotionPhase::Airborne` when true.
2. Construct the scalar raw fields through the canonical field-extraction helper.
3. Call `classify_raw_on_table_motion_phase` with the original `state.vertical_velocity.as_f64()`, not zero.

This preserves below-airborne-threshold vertical noise: it can still fail the Rest branch because of `rest_vertical_speed`, while the Spinning/rolling/sliding precedence remains exactly as it is today. Remove `try_on_table_state` only if no other caller remains; do not leave a private compatibility shim or dead helper.

### 5. Convert every validated direct caller

Use `raw_on_table_state_and_phase` wherever an `OnTableBallState` is classified. In callers that already need raw state, destructure the pair once and pass that same raw value to downstream raw kernels:

- `compute_next_transition_on_table`;
- `advance_within_phase_on_table`'s debug assertion and raw advancement;
- `advance_motion_on_table`;
- `compute_next_ball_ball_collision_during_current_phases_on_table` for both balls;
- `compute_next_ball_rail_impact_on_table`;
- `side_pocket_post_jaw_path_reaches_centerline_inside_capture_region` on each phase iteration;
- `compute_next_ball_jaw_impact_on_table`;
- `compute_next_ball_pocket_capture_on_table`.

For `advance_motion_on_table`, compute `(raw, phase)` once per recursive phase segment, call `raw_compute_next_transition_on_table(raw, phase.clone(), ...)`, and use the same `raw` in `raw_advance_within_phase_on_table`. Do not route back through public wrappers that reconstruct/reclassify the same state. Preserve the existing zero-duration return and first-transition reporting behavior.

Convert validated phase-only consumers—the resting-state check, `trace_ball_path_with_rail_profile_on_table`, `estimate_post_contact_cue_ball_curve_on_table`, and `estimate_post_contact_cue_ball_bend_on_table`—to the phase-only delegate. Leave `BallState::motion_phase` on the public general-state wrapper because its receiver may be airborne.

After migration, search for `classify_motion_phase` call sites. Remaining production calls must be genuinely general `BallState` entry points; there must not be two internal on-table classification conventions.

### 6. Keep downstream arithmetic and ownership stable

Do not change `raw_advance_within_phase_on_table`, `raw_compute_next_transition_on_table`, collision roots, event comparison functions, or exact-unit reconstruction of public output states. The implementation is accepted only if it removes input-side round-trips while preserving downstream operation order and outputs.

## Benchmark plan

### Existing filters and fixtures

Run these existing Criterion filters as adjacent-path evidence:

- `motion_phase_classification/{rest,spinning,rolling,sliding}`
- `core_functions/compute_next_transition_on_table/sliding`
- `throughput_functions/compute_next_transition_on_table/10000`
- `core_functions/compute_next_ball_ball_collision_during_current_phases_on_table`
- `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/1000`
- `core_functions/compute_next_ball_rail_impact_on_table`

The direct classifier group is already committed with four independently named non-airborne points, eight-second measurement time, 30 samples, prebuilt states/config, pre-timing phase assertions, input black-boxing, and complete-enum output black-boxing. The quick ranges above are historical context only; acceptance uses freshly saved paired baselines on the implementation host.

### Missing coverage to add with this candidate

Extend the existing `motion_phase_classification` group; do not create a duplicate `core_functions/classify_motion_phase` namespace. Add:

- `airborne_height`
- `airborne_vertical_speed`
- `exact_no_slip_rolling`
- `thresholded_rolling_inside`
- `thresholded_sliding_outside`
- `below_airborne_noise_not_rest`

The committed `rolling` fixture is an ordinary exact-no-slip state under the benchmark's current motion config, but it does not isolate the `ExactNoSlip` model or either side of a configured threshold. Use explicit configs for the added cells. For the two thresholded cases, construct contact speeds immediately inside and immediately outside a representable `contact_speed_epsilon`; add exact-equality cases to correctness tests, not to a noisy timing aggregate. `below_airborne_noise_not_rest` must have height and vertical speed accepted by the airborne thresholds but vertical speed above `rest_vertical_speed`, so it protects the public wrapper's distinct vertical semantics.

Also add `core_functions/compute_next_transition_on_table/{rest,spinning,rolling}` beside the existing `/sliding` point. These validated fixtures exercise the shared raw construction path and each transition branch. `rest` must black-box `None`; the other points must black-box the complete `NextTransition`.

All states, unit values, ball specs, and configs are constructed before `b.iter`. Pass inputs through `black_box` and black-box the complete enum/`Option`/prediction result, not a selected scalar. Do not combine branch distributions into one mean.

## Correctness and equivalence tests

Extend `tests/motion_phase_classifier.rs` with a table-driven boundary matrix that compares the public result against fixed expected phases for:

- both airborne predicates below, equal to, and above their thresholds;
- all five Rest component thresholds below/equal/above, including vertical speed separately;
- zero linear/horizontal spin with `wz` equal to and one representable value above `rest_angular_speed`;
- exact no-slip, `f64::EPSILON`, and the next representable contact speed above it;
- thresholded contact speed below, equal to, and above `contact_speed_epsilon`;
- ordinary sliding with draw and overspin.

Add validated-caller equivalence coverage for the same on-table Rest/Spinning/Rolling/Sliding states. For every fixture, assert enum equality. For transition-producing states, assert phase-before, phase-after, `Option` shape, and `time_until_transition.as_f64().to_bits()` against pre-change golden values.

Add a deterministic predictor corpus covering all phase pairings used by ball-ball prediction plus rolling rail, jaw, and pocket paths. Compare pre-change golden outputs at the bit level:

- event/prediction `Option` shape and variant;
- every returned time via `as_f64().to_bits()`;
- returned position, velocity, and angular-velocity components via `as_f64().to_bits()`;
- rail, jaw, pocket, and ball indices.

Finally, add two-ball and N-ball cases in which collision, rail, and transition candidates are equal or adjacent around `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`. Assert the selected event source, ball/index or pair ordering, shared-contact pair ordering, and event-time bits. This is the time-order gate: any changed winner or deterministic ordering rejects the optimization even if phase labels happen to match.

Capture golden bits from the unchanged implementation with the same toolchain and keep the paired artifact as the strict implementation gate. Checked-in bit constants must be restricted to expressions proven portable across supported targets or gated to the target/toolchain that generated them; platform-independent CI keeps fixed phase/event expectations and established semantic tolerances. Do not weaken the same-host paired gate to approximate equality. If any paired value differs because the raw classifier used a different norm or reassociated arithmetic, fix the arithmetic; do not broaden tolerances.

## Statistical acceptance

Use the same arm64 host, Rust toolchain, release profile, feature set, Criterion configuration, power source, and thermal conditions for baseline and candidate. Save Criterion baselines. Run at least **three paired process runs** per primary filter, alternating baseline and candidate order. Configure the new point groups for at least **30 samples** and **8 seconds** measurement time. Use Criterion's bootstrap **95% confidence interval** for the after/before time ratio; point estimates alone are not acceptance evidence.

Required performance gates:

1. `compute_next_transition_on_table/sliding`: upper bound of the 95% CI for after/before time is **≤ 0.90**.
2. `compute_next_transition_on_table/10000`: upper bound is **≤ 0.93**.
3. Every existing and added non-airborne `motion_phase_classification` cell must be no slower than baseline at the 95% boundary, and the committed Rest, Spinning, Rolling, and Sliding cells must each show a statistically significant improvement.
4. Every added airborne classifier branch, transition branch, ball-ball predictor point/batch, and rail predictor point must have an after/before ratio 95% CI upper bound of **≤ 1.03**.
5. All three paired runs must agree in direction on both primary transition gates; one anomalous run is rerun, not averaged away.

Corroborate the mechanism with a focused macOS `sample` or Instruments Time Profiler capture of the 10,000-transition batch. `classify_motion_phase` must no longer show BigDecimal formatting/parsing beneath its speed or cloth-contact calculation, and the profile must show the validated caller using one raw extraction per operation. Where Allocations can be scoped reliably to the benchmark, the new raw classifier itself must perform **zero exact-unit temporary allocations**. Timing without the expected stack/allocation change is insufficient and must be investigated.

Correctness gates run before timing. Any bitwise or event-order failure rejects the candidate regardless of speed.

## Risks and mitigations

- **Different norm rounding:** `hypot` is not a drop-in replacement for the current sum/square-root expression. Preserve the current expression and enforce golden bits.
- **Vertical-noise semantic collapse:** zeroing vertical speed too early can turn a below-airborne but non-rest state into Rest. Pass the original vertical speed in the public wrapper and zero only for validated `OnTableBallState`.
- **Precedence drift:** a convenient combined predicate can move Spinning ahead of Rest or add a vertical predicate to Spinning. Keep explicit ordered returns and boundary tests.
- **Duplicate classification/conversion:** wrappers can accidentally reconstruct raw state or reclassify during advancement. Migrate the complete direct-caller set and inspect remaining call sites.
- **Misattributed pocket wins:** pocket predictors are dominated by search work. Treat them as correctness/adjacent guards, not as primary evidence for this micro-optimization.
- **Compiler already optimized part of the path:** require the profile shape and practical transition thresholds; do not retain extra helpers for a statistically negligible result.

## Stop conditions, rejection, and rollback

Stop and reject the implementation if any of the following occurs:

- a phase boundary, `Option` shape, output bit pattern, event winner, or deterministic ordering changes;
- preserving bitwise output requires retaining the exact-unit reconstruction in the validated hot path;
- either primary transition benchmark misses its practical threshold across three paired runs;
- profiles still attribute the same formatting/parsing/allocation work to classification;
- the implementation requires a public raw API, approximate arithmetic, cached phase ownership, or physics changes.

Rollback is a single-code-path revert: restore the public classifier and validated callers to their previous implementation while retaining the branch fixtures and equivalence tests, which remain useful coverage. Do not add a runtime switch, old/new compatibility path, or deprecated alias.

## arm64 SIMD and GPU decision

**No explicit SIMD.** A state contains only a few scalar values, classification has ordered branches, and the public/validated APIs process one state at a time. NEON packing, masks, and extraction would add overhead and risk changed norm rounding. A future structure-of-arrays batch classifier could permit auto-vectorization across independent states, but introducing a second layout/API solely for this change is unsupported by current evidence and is outside scope.

**No GPU.** Transfer, dispatch, synchronization, and result-readback costs dwarf a handful of scalar operations, while event simulation needs each phase result immediately for branchy scheduling. The kernel has neither sufficient arithmetic intensity nor a large independent batch at this API boundary.

## Candidate commit message

`perf(motion): classify validated states in the raw domain`
