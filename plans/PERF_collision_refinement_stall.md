# Terminate stalled collision refinements at binary64 endpoint equality

## Status, priority, confidence, and order

- **Status:** Accepted for implementation, benchmark-gated.
- **Priority:** P1 for the two local loops; the change is small, removes provably redundant work, and affects collision and curved-entry predictors.
- **Confidence:** High that valid brackets reach a representational fixed point before 80 iterations; high that stopping there preserves returned bits; medium that every public predictor benchmark will clear a practical timing threshold because adaptive search and phase advancement also contribute to total cost.
- **Dependencies:** The committed `collision_predictor_paths` group supplies ordinary linear-hit, curved-hit, parallel-miss, and grazing-miss cells. Add the missing curved-rail hit/control and collect temporary refinement counts plus saved unchanged-code outputs before production edits. No root-storage change is required or permitted by this plan.
- **Order:** Implement after the raw-motion-classification cutover when both changes share a branch, so collision benchmark deltas remain attributable and the predictor baseline is stable. Otherwise this plan is independent. Keep it separate from collision root-storage and broader curved-event/pocket-search changes.

## Problem and evidence

### Measured facts

The unchanged-tree quick Criterion snapshots report approximately **21,600 predictions/s** for `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/1000`, plus:

- `collision_predictor_paths/linear_hit`: **6.488–6.498 us/call**;
- `collision_predictor_paths/curved_rolling_hit`: **1.0781–1.0789 ms/call**;
- `collision_predictor_paths/parallel_miss`: **6.754–6.770 us/call**;
- `collision_predictor_paths/grazing_miss`: **5.725–5.953 us/call**.

These quick current-tree ranges are useful scale evidence, not paired acceptance results. The first and curved-hit cells exercise the two successful refinement families; the two miss controls do not by themselves prove whether a refiner was entered.

There is currently no measured iteration-count evidence for either refiner and no committed curved-rail branch fixture. The speedup and representational-stall mechanism must therefore be established by temporary counts/profile evidence and paired runs below; neither is already proven by the quick timings.

### Source-grounded facts

The exact current symbols are in `src/lib.rs`:

- `refine_ball_ball_collision_time_for_relative_motion` at lines 5817–5831 executes `for _ in 0..80`, computes `midpoint = 0.5 * (left + right)`, evaluates `motion.gap_at(midpoint)`, replaces one endpoint, and returns `right`.
- `first_ball_ball_contact_time_for_relative_motion` at lines 5757–5815 builds monotonic intervals and calls that refiner only when `left_gap > 0.0 && right_gap <= 0.0`, after rejecting a tangent/non-closing minimum. Thus a valid refinement bracket has a non-contact left endpoint and a contact-side right endpoint.
- `refine_curved_entry_time` at lines 5910–5923 has the same unconditional 80-step bisection shape and also returns `right`.
- `first_curved_entry_time_adaptive` at lines 5926–5980 calls the curved refiner after an adaptive interval reaches its time tolerance and contains a gap crossing. It then preserves a separate closing-derivative check at lines 5965–5967.
- `compute_next_ball_ball_collision_during_current_phases_on_table` at lines 6234–6332 selects the quadratic relative-motion path or the curved rolling path. The curved helper is also used by `first_rail_collision_time_during_current_phase_raw` at lines 6389–6423, and by fixed-circle curved entry through `first_fixed_circle_entry_time_for_raw_motion`/`first_curved_fixed_circle_entry_time_for_raw_motion`.
- The existing deterministic tests already contain reusable public fixtures: quadratic rolling contact and grazing cases in `tests/ball_collision_timing.rs`, the curved ball-ball hit at lines 317–362, curved ghost/no-hit at lines 365–381, horizon/rolling-stop cases at lines 385–423, and the curved right-rail hit in `tests/rail_event_scheduling.rs` lines 85–109.
- Event time is observable in scheduler precedence. `TwoBallOnTableEvent` documents collision → A rail → B rail → A transition → B transition tie order at `src/lib.rs` lines 1632–1648. `NBallOnTableEvent` documents lexicographic ball-pair ordering before rail and transition at lines 1690–1722, and shared contacts are grouped using `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` around lines 2231–2259.

### Source inference to validate

For finite binary64 endpoints, repeated midpoint bisection eventually reaches a state where `midpoint == left` or `midpoint == right`. Once that occurs, evaluating the same endpoint cannot change the returned `right` bits:

- the quadratic refiner starts with `motion.gap_at(left) > 0.0`; every later assignment to `left` also follows the same strict positive predicate, so `midpoint == left` can only reassign `left` to itself;
- the curved refiner's caller supplies a left endpoint whose gap is greater than `CURVED_EVENT_GAP_TOLERANCE_INCHES`, hence also greater than zero, and the refiner assigns `left` only after `gap_at(midpoint) > 0.0`;
- when `midpoint == right`, either comparison result leaves `right` bit-identical: the contact-side branch reassigns `right` to itself, while the other branch moves only `left` to the existing `right`.

The curved caller's initial `right` endpoint is guaranteed only to be at or below `CURVED_EVENT_GAP_TOLERANCE_INCHES`, not necessarily at or below the refiner's zero-gap predicate. Do not describe it as an initially contact-side endpoint. The left-side invariant, plus the fact that either `midpoint == right` branch preserves `right`, is the exact safety argument.

The remaining fixed-count iterations therefore repeat work and return the same `right` bits. This is a numerical inference from IEEE-754 representability and the two source-specific bracket invariants, not yet an observed count for each benchmark fixture. Instrumentation/profile corroboration must show that representative valid branches actually stall before iteration 80. If they do not, reject this candidate rather than introduce a tolerance.

## Scope

Add endpoint-equality termination to **both** current 80-step bisections:

- `refine_ball_ball_collision_time_for_relative_motion`;
- `refine_curved_entry_time`.

The only new condition is exact binary64 endpoint equality. Keep the 80-iteration cap. Preserve the contact-side `right` endpoint as the return value.

Observable contracts to preserve exactly:

- hit/no-hit `Option` shape;
- returned `Seconds` bits;
- every state advanced to the returned time;
- closing/tangent decisions;
- event source, event time, pair/index ordering, and simultaneous-contact grouping;
- deterministic behavior for finite valid inputs and the existing behavior for invalid/NaN inputs.

## Explicit non-goals

- No absolute, relative, geometric, gap, time, ULP-count, or physics tolerance.
- No reduction of the 80-step safety cap to an assumed fixed count.
- No Newton, secant, Brent, analytic-root, batched-midpoint, speculative, SIMD, or GPU replacement.
- No change to bracket discovery, derivative roots, boundary deduplication, adaptive curved search, gap tolerances, closing-derivative checks, horizon logic, or event tolerances.
- No collision root-storage, fixed-buffer, allocation, prepared-context, pocket-search, or physics-model work.
- No public API change and no compatibility wrapper.

## Implementation design and clean cutover

### 1. Complete and baseline the committed branch fixtures

Keep the committed `collision_predictor_paths/{linear_hit,curved_rolling_hit,parallel_miss,grazing_miss}` cells. Add only the missing curved-rail fixtures below without changing production code. Capture complete outputs and golden time/state bits for every hit. Run one temporary diagnostic build or focused profile to record how many gap evaluations each successful refinement performs and at which iteration endpoint equality first occurs; do not commit counters or expose private helpers publicly.

### 2. Add the same exact termination rule to both loops

In each loop, immediately after computing the midpoint and **before** evaluating its gap, use the equivalent of:

```rust
let midpoint = 0.5 * (left + right);
if midpoint == left || midpoint == right {
    break;
}
```

Then leave the existing gap comparison and endpoint assignment unchanged. Keep `for _ in 0..80` as the cap and keep returning `right`.

Checking before `gap_at(midpoint)` is intentional: once the midpoint equals an endpoint, that evaluation is the redundant work being removed. Do not replace exact `==` with an epsilon or helper whose semantics include approximate equality. Do not use `(right - left)` as the primary condition; subtraction can underflow or introduce a second numerical rule that is unnecessary for representational stall.

### 3. Preserve each caller's bracket and postcondition logic

Do not modify:

- `left_gap > 0.0 && right_gap <= 0.0` or tangent rejection in `first_ball_ball_contact_time_for_relative_motion`;
- the curved adaptive `CURVED_EVENT_GAP_TOLERANCE_INCHES` and `time_tolerance` used to discover a bracket;
- the curved `derivative_at(root) < -derivative_tolerance` closing check;
- the advancement performed after `dt_seconds` is returned;
- rail, jaw, pocket, or scheduler comparison logic.

Endpoint equality is a termination property of an already valid bracket, not a new acceptance criterion for contact.

### 4. Remove temporary diagnostics

After confirming the stall locations and timing effect, remove all counters, logging, feature flags, alternate refiner copies, and benchmark-only public access. The final production diff should be the two matching exact-equality guards and any narrowly necessary comments. Keep the new benchmarks and observable tests.

## Benchmark plan

### Existing filters and committed branch fixtures

Retain and run:

- `collision_predictor_paths/linear_hit`
- `collision_predictor_paths/curved_rolling_hit`
- `collision_predictor_paths/parallel_miss`
- `collision_predictor_paths/grazing_miss`
- `core_functions/compute_next_ball_ball_collision_during_current_phases_on_table`
- `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/100`
- `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/1000`
- `core_functions/compute_next_ball_rail_impact_on_table` as an adjacent ordinary-rail guard

The committed group uses prebuilt inputs, pre-timing hit/miss assertions, eight-second measurement time, 30 samples, input black-boxing, and complete `Option<PredictedBallBallCollision>` output black-boxing:

- `linear_hit` is the current `collision_predictor_states` geometry: rolling cue ball at `y = -(2R + 7.5)`, speed `10 in/s`, matching rolling spin, resting object at the origin, and `5 in/s²` rolling deceleration. It is the quadratic successful-refinement cell.
- `curved_rolling_hit` is the canonical nonzero-`wz` fixture at `(10, 20)` against the object at `(12.253_702_077_725_524, 27.499_997_782_973_136)`. It is the adaptive curved successful-refinement cell.
- `parallel_miss` uses equal-velocity rolling balls in separated lanes and is a polynomial no-hit/control path.
- `grazing_miss` uses a transverse offset of `2R + 1e-4` and is the near-tangent polynomial miss/control path.

Do not rename these committed IDs or add duplicate aliases. Temporary counters must confirm `linear_hit` reaches `refine_ball_ball_collision_time_for_relative_motion` and `curved_rolling_hit` reaches `refine_curved_entry_time`; benchmark names alone are not branch proof. The quick ranges above are historical scale evidence only; acceptance uses fresh saved paired baselines.

### Missing coverage to add

Add public-API rail points under a `rail_predictor_paths` group (or as explicitly named siblings in the existing core group):

1. `curved_rolling_hit`
   - Reuse `curved_rolling_ball_reaches_the_right_rail_before_its_transition`: state `(48.871, 20)`, velocity `(0, 10)`, spin `(-10/R, 0, 2)`, default table/ball set, and the same motion config.
   - Confirm it reaches `refine_curved_entry_time`, returns `Rail::Right`, and impacts before one second.

2. `curved_rolling_no_hit`
   - Use the same state with `wz = -2`, matching `opposite_spin_curve_away_does_not_create_a_right_rail_impact`.
   - Assert `None`; this guards adaptive exclusion behavior and is not a successful-refinement win cell.

Build all `OnTableBallState`, configs, ball specs, and tables outside `b.iter`. Pass inputs through `black_box` and black-box the complete `Option<PredictedBallRailImpact>`. Keep hit and no-hit separate.

The existing 100/1,000 throughput workload is a deterministic **mixed** rolling workload: some rows produce bracketed hits while stop-before-contact and other rows miss. Keep it as an amortized whole-predictor gate rather than claiming every element refines. Temporary counters collected with the unchanged baseline must record the successful-refinement fraction; remove them before delivery. Do not mix curved fixtures into that workload because divergent branch cost would further obscure attribution.

## Correctness and equivalence tests

Extend the public predictor tests rather than exposing either private refiner.

### Bitwise result corpus

Capture unchanged-code golden values with the implementation toolchain, then assert exact results after the change for:

- quadratic direct/head-on hit;
- quadratic oblique hit;
- near-tangent bracketed hit;
- exact tangent and one-ULP miss (`None`);
- contact just before the phase horizon;
- rolling stop exactly at contact (`None`);
- contact just after the phase horizon (`None`);
- curved rolling ball-ball hit;
- curved ghost/no-hit;
- curved right-rail hit;
- opposite-spin curved rail no-hit;
- at least one curved fixed-circle jaw entry, because that path shares `refine_curved_entry_time`.

For each hit, assert:

- the `Option` and event variant;
- `time_until_impact.as_f64().to_bits()` (or capture-time equivalent);
- rail/jaw identity where applicable;
- every returned position, velocity, and angular-velocity component via `as_f64().to_bits()`.

For no-hit cases, assert `None` exactly. Golden bit constants are preferable to a duplicated test implementation of the old 80-step private loop because the contract is the public prediction, not the loop's source shape.
Store the same-host/toolchain golden corpus as the strict paired implementation artifact. Check in bit constants only when portable across supported targets or target-gated to the generating configuration; platform-independent CI retains fixed hit/miss/event expectations and established semantic tolerances. This portability rule does not permit any paired-host bit drift.

### Time-order and simultaneous-event gates

Add deterministic scheduler fixtures around the event boundaries most sensitive to a time-bit change:

- collision exactly tied with a motion transition;
- collision one representable time below and above a transition;
- collision tied with A and B rail candidates in the documented two-ball precedence;
- two N-ball collision pairs within `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`, including one case exactly on the grouping boundary and one immediately outside it;
- a curved collision competing with a rail or transition.

Assert the selected event source, event time bits, ball/index or lexicographic pair ordering, shared-contact `ball_indices`, and `ball_ball_pairs`. Before/after must be identical. A result that remains geometrically close but changes the scheduled winner, grouping, or pair order fails this plan.

### Invalid-input behavior

Do not add public invalid inputs solely for this optimization. If existing private/unit coverage exercises NaN or malformed brackets, retain its exact behavior. The equality check naturally remains false for NaN; no new finite/NaN normalization or panic is allowed.

## Statistical acceptance

Use the same arm64 host, Rust toolchain, release profile, feature set, Criterion configuration, power source, and thermal state for baseline and candidate. Save Criterion baselines. Run at least **three paired process runs**, alternating baseline/candidate order. Configure each new point for at least **30 samples** and **8 seconds** measurement time. Use Criterion's bootstrap **95% confidence interval** for the after/before time ratio.

Required gates:

1. `collision_predictor_paths/linear_hit`: upper bound of the 95% CI for after/before time is **≤ 0.95**.
2. `collision_predictor_paths/curved_rolling_hit`: upper bound is **≤ 0.97**.
3. The added curved rolling rail hit: upper bound is **≤ 0.97**.
4. Existing 1,000-element collision throughput: upper bound is **≤ 0.97**.
5. `parallel_miss`, `grazing_miss`, the added curved rail no-hit, and the existing generic rail adjacent path must each have an after/before ratio 95% CI upper bound of **≤ 1.03**.
6. All three paired runs must agree in direction for the linear hit, curved ball-ball hit, and 1,000-element batch. Rerun an anomalous pair; do not average it away.

Before final timing, use temporary call-count instrumentation or a focused Time Profiler capture to corroborate the mechanism:

- every successful representative refinement must reach endpoint equality before iteration 80;
- no `gap_at` call occurs for the equality-stalled midpoint after the change;
- the returned right endpoint bits match the 80-step baseline.

Remove instrumentation before delivery. There is no allocation claim for this plan; allocation counts are neither a success criterion nor a substitute for branch timings.

Correctness and time-order gates run first. Any mismatch rejects the change regardless of benchmark improvement.

## Risks and mitigations

- **Checking after gap evaluation:** this preserves output but retains one avoidable expensive curved advancement. Check equality before `gap_at`.
- **Returning the wrong endpoint:** returning `left`, `midpoint`, or an average can move the event earlier/later. Continue returning the current contact-side `right`.
- **Tolerance creep:** reusing adaptive `time_tolerance` as a bisection exit changes time bits and potentially event ordering. Use endpoint equality only.
- **Broken bracket assumptions:** the proof relies on valid finite brackets produced by current callers. Do not broaden the refiner API or hide invalid brackets with a new early return.
- **Curved post-check drift:** the closing derivative must still be evaluated at the same returned root and compared with the same tolerance.
- **Benchmark branch ambiguity:** a nominal fixture may be rejected before refinement. Confirm branch entry with temporary diagnostics, then remove them.
- **Noise masking a tiny quadratic win:** use branch points, three paired runs, and the practical CI thresholds; do not justify the change solely from a theoretical iteration count.

## Stop conditions, rejection, and rollback

Reject the implementation if:

- any output time or state bit changes;
- any hit/no-hit result, tangent decision, scheduler winner, simultaneous group, or deterministic order changes;
- representative valid hit fixtures do not stall before iteration 80;
- either bisection requires a tolerance to show improvement;
- the quadratic hit, curved ball-ball hit, or throughput primary gate misses its required threshold in repeated paired runs;
- implementation expands into root storage, adaptive search policy, or model/tolerance changes.

Rollback is the direct removal of the two equality guards. Retain the branch-specific benchmarks and public bitwise/time-order tests because they improve coverage independently. Do not keep an old/new switch, alternate refiner, compatibility wrapper, or feature flag.

## arm64 SIMD and GPU decision

**No explicit SIMD.** Each bisection midpoint depends on the previous bracket, so the loop is a serial recurrence. Evaluating speculative midpoints in NEON would change evaluation count and ordering and would still require branch selection before the next interval. The curved gap function also advances one state at one candidate time, making packing overhead and divergence dominant. LLVM scalar optimization is appropriate.

**No GPU.** Each refinement contains at most one small dependent chain and its result is immediately consumed by an irregular event scheduler. Kernel launch, transfer, synchronization, and divergent bracket counts overwhelm the work. A separate large offline batch API could be researched only with demonstrated demand; it is not supported by current profiles or this scalar event path.

## Candidate commit message

`perf(collision): stop bisection when the bracket stalls`
