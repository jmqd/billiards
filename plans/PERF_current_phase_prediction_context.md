# Share one current-phase prediction context per on-table ball

## Status

- **Status:** Accepted implementation plan; not implemented.
- **Priority:** P1, immediately after prepared pocket-capture geometry.
- **Confidence:** High that duplicate raw-state/phase/transition preparation exists; medium that the committed one-ball scheduler fixture can clear the latency gate after the pocket geometry prerequisite.
- **Dependencies/order:** Benchmark and land `PERF_prepared_pocket_geometry.md` first so the committed one-ball scheduler measurement is not dominated by repeated mutex/decimal work in each capture gap. This plan has no physics dependency on that change and must be benchmarked as a separate commit against a baseline that already contains accepted prepared geometry. It must precede incremental `PocketAwareEventCache` lifecycle work so context ownership remains simple and call-local.

## Problem and evidence

### Measured facts

The unchanged-tree profile recorded in `local://perf-plan-context.md` attributes 3,842 of 3,893 three-ball/eight-event samples to `PocketAwareEventCache::build`. The committed `pocket_predictors` group now measures `scheduler/one_ball_slow_side_capture` at about 9.58 ms/call, alongside the direct slow-side capture at about 9.45 ms and the direct jaw path at about 34.7 us. The older Criterion snapshots remain approximately 495–504 ms/call for `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`, 39.2 ms for `end_to_end/direct/pocket_aware_until_rest_cached`, and 39.6 ms for the manual recomputation control.

The committed scheduler fixture has one on-table ball, so it excludes pair prediction, constructs all inputs outside `b.iter`, and black-boxes the complete `Option<NBallSystemEvent>` after the timed call's geometry `Result` is unwrapped with `expect`. Its current preflight asserts only `is_some()`: it does not characterize the exact event or prove the four duplicate preparation calls. The fixture and profile establish that cache construction is hot, but they conflate capture scanning with boundary setup. Exact preflight characterization and call-count corroboration below are required.

### Source-observed duplicate preparation

All symbols below are currently in `src/lib.rs`.

`PocketAwareEventCache::refresh_ball` handles an on-table ball by calling four public predictors independently, in jaw, capture, rail, then transition order:

1. `compute_next_ball_jaw_impact_on_table`;
2. `compute_next_ball_pocket_capture_on_table`;
3. `compute_next_ball_rail_impact_on_table`; and
4. `compute_next_transition_on_table`.

For the same state, ball specification, and motion configuration:

- `compute_next_ball_jaw_impact_on_table` builds `RawOnTableBallState`, converts `ball.radius` to `f64`, calls `classify_motion_phase`, calls `raw_compute_next_transition_on_table`, and derives the phase horizon;
- `compute_next_ball_pocket_capture_on_table` repeats the same sequence;
- `compute_next_ball_rail_impact_on_table` repeats the same sequence; and
- `compute_next_transition_on_table` classifies again, builds the same raw state, converts the same radius, and calls the same raw transition function.

Thus a one-ball boundary refresh currently performs four raw-state conversions, four radius conversions, four phase classifications, and four raw transition predictions before pair prediction begins. `classify_motion_phase` is not trivial: it converts speed/threshold values, normalizes a non-airborne state, clones the radius into cloth-contact calculation, and computes contact slip. `raw_compute_next_transition_on_table` may advance the state and classify the phase endpoint.

`RawOnTableBallState` is a seven-`f64`, `Copy` private struct. `MotionPhase` is a small public enum that is `Clone` but not `Copy`. `NextTransition` is owned and `Clone`. The public standalone predictors must retain their existing independent-call behavior.

### Hypothesis to test

Building one immutable current-phase context for an on-table ball in `refresh_ball`, borrowing it across jaw/capture/rail prediction, and moving its already-computed transition into the cache will reduce one-ball scheduler latency by at least 5%. The duplicate call count is source-certain; the practical latency percentage is a benchmark hypothesis.

## Scope

Introduce one private raw current-phase prediction context that ties together exactly one current on-table source state, the motion configuration used to derive it, and the state-independent preparation shared by jaw, capture, rail, and transition prediction:

- a borrow of the exact `OnTableBallState` used to construct the context, retained for the current zero-time capture clone;
- a borrow of the exact `OnTableMotionConfig` used for classification, transition prediction, and within-phase advancement;
- `RawOnTableBallState`;
- ball radius as `f64`;
- classified `MotionPhase`;
- owned `Option<NextTransition>`; and
- horizon derived from that exact transition (`time_until_transition.as_f64()` or `f64::INFINITY`).

Use one context per on-table ball during `PocketAwareEventCache::refresh_ball`. Keep each public standalone predictor as a thin context-building wrapper around a private context-consuming implementation. Preserve all public signatures and exact outputs. The private helpers must not accept a second state or motion-config argument that could disagree with the context.

## Explicit non-goals

- Do not persist prediction contexts in `PocketAwareEventCache`, introduce invalidation, or change full/incremental cache rebuild lifecycle.
- Do not share a context across balls, across refresh calls, or after any state mutation/event resolution.
- Do not change pair collision prediction. `compute_next_ball_ball_collision_during_current_phases_on_table` has two-state semantics and remains outside this one-ball boundary context.
- Do not change motion classification, transition equations, thresholds, horizon semantics, infinity/epsilon exits, predictor search math, event ordering, or physics results.
- Do not combine table/pocket geometry with this context. Prepared pocket geometry remains the separately measured responsibility of `PERF_prepared_pocket_geometry.md`.
- Do not make `MotionPhase` `Copy` merely to support this change; cloning a tiny enum is not the target. Do not add a public context type or compatibility API.
- Do not alter validation, simulation ownership, candidate storage, cache arrays/maps, or event materialization beyond moving the already-owned `NextTransition` at the end of a refresh.

## Design

### Context shape

Add a private borrowed/owned value equivalent to:

```rust
struct CurrentPhasePredictionContext<'a> {
    source_state: &'a OnTableBallState,
    motion: &'a OnTableMotionConfig,
    raw_state: RawOnTableBallState,
    radius: f64,
    phase: MotionPhase,
    next_transition: Option<NextTransition>,
    horizon_seconds: f64,
}

impl<'a> CurrentPhasePredictionContext<'a> {
    fn new(
        state: &'a OnTableBallState,
        ball: &BallSetPhysicsSpec,
        motion: &'a OnTableMotionConfig,
    ) -> Self;
}
```

Construction order is fixed:

1. retain `state` and `motion` borrows;
2. `RawOnTableBallState::from_on_table(state)`;
3. `ball.radius.as_f64()`;
4. `classify_motion_phase(state.as_ball_state(), ball, &motion.phase)`;
5. exactly one `raw_compute_next_transition_on_table(raw_state, phase.clone(), radius, motion)`; and
6. `horizon_seconds` from that owned transition, or `f64::INFINITY` for `None`.

Derive the horizon from the stored transition rather than making a second raw transition call. Retaining `source_state` is necessary because the current immediate-capture branch clones the original `OnTableBallState`; rebuilding it from seven raw `f64` values is not guaranteed to preserve the exact public numeric representation. Retaining `motion` prevents a helper from advancing with a configuration different from the one that produced phase and horizon.

The context must not own `BallSetPhysicsSpec`, `OnTableMotionConfig`, `TableSpec`, or cloned `BigDecimal` values, and it must not borrow `TableSpec`. The two borrows are call-local correctness ties, not persistent cache state.

### Validity key and lifetime

This is an ephemeral derived value, not a memoization table. Therefore it has no hash key and no lookup.

Its logical validity identity is the exact tuple:

```text
(borrowed on-table source state,
 source state's seven raw components,
 ball.radius,
 borrowed complete OnTableMotionConfig)
```

The constructor consumes the ball radius synchronously and borrows the state/config immutably. The Rust lifetime prevents mutation of either borrowed source while the context is in use; the context is nevertheless valid only for that one predictor wrapper or `refresh_ball` invocation. In `refresh_ball` it is created after matching `NBallSystemState::OnTable`, borrowed by the three boundary predictors, partially consumed/moved for transition materialization after those borrows end, and dropped before the refresh returns. Public wrappers create and drop one context per call. There is no process, table, cache-build, or event-to-event lifetime and therefore no invalidation mechanism to get wrong.

Do not use pointer identity, ball index, or a partial configuration key. Do not build a context from one state/config and pass another state/config beside it. If future incremental cache work wants to persist contexts, that is a separate design requiring an explicit generation/invalidation contract.

### Private predictor cutover

Introduce private implementations with a uniform context boundary, for example:

```rust
fn compute_next_ball_rail_impact_with_context(
    context: &CurrentPhasePredictionContext<'_>,
    table: &TableSpec,
) -> Option<PredictedBallRailImpact>;

fn compute_next_ball_jaw_impact_with_context(
    context: &CurrentPhasePredictionContext<'_>,
    table: &TableSpec,
) -> Option<PredictedBallJawImpact>;

fn compute_next_ball_pocket_capture_with_context(
    context: &CurrentPhasePredictionContext<'_>,
    table: &TableSpec,
) -> Option<PredictedBallPocketCapture>;
```

The private helpers use only `context.source_state`, `context.motion`, `context.raw_state`, `context.radius`, `context.phase.clone()`, and `context.horizon_seconds` for state/config preparation and advancement. They must not call `classify_motion_phase`, `raw_compute_next_transition_on_table`, `compute_next_transition_on_table`, `RawOnTableBallState::from_on_table`, or `ball.radius.as_f64()`.

`TableSpec` remains an explicit argument because it does not participate in current-phase validity and the boundary geometry consumers require it. Do not reintroduce separate `state` or `config` arguments. Jaw/capture context helpers are called only after a `table.has_pockets()` guard; never let the shared refresh path bypass the pocketless-table early exit.

Each public function retains its current signature and behavior:

```rust
pub fn compute_next_ball_rail_impact_on_table(...) -> Option<_> {
    let context = CurrentPhasePredictionContext::new(state, ball, config);
    compute_next_ball_rail_impact_with_context(&context, table)
}
```

Apply the same wrapper shape to jaw and capture. Preserve the current early `!table.has_pockets()` check in jaw/capture before context construction, so pocketless direct calls remain cheap and retain exact behavior. The rail wrapper always constructs one context as it does logically today. `compute_next_transition_on_table` may also return `CurrentPhasePredictionContext::new(...).next_transition`, but only if that does not obscure ownership or add unrelated table/horizon work; otherwise keep its current direct construction. In either case, its output and standalone benchmark must be unchanged within the regression gate.

### `refresh_ball` data flow

For the `OnTable` branch only:

```rust
let context = CurrentPhasePredictionContext::new(state, ball, config);
if table.has_pockets() {
    self.jaw_impacts[ball_index] =
        compute_next_ball_jaw_impact_with_context(&context, table);
    self.pocket_captures[ball_index] =
        compute_next_ball_pocket_capture_with_context(&context, table);
} else {
    self.jaw_impacts[ball_index] = None;
    self.pocket_captures[ball_index] = None;
}
self.rail_impacts[ball_index] =
    compute_next_ball_rail_impact_with_context(&context, table);
self.transitions[ball_index] = context.next_transition;
```

Keep jaw, capture, rail, and transition assignment order exactly as today. The shared refresh still constructs one context on a pocketless table because rail and transition require it, but jaw/capture slots remain `None` without touching pocket geometry. End immutable helper borrows before moving `next_transition`; do not clone it into the cache. The borrowed source state and motion configuration die with the context at the end of this refresh. Pair prediction remains after this block and unchanged.

Airborne and pocketed branches must not construct a context. Their current clearing/settling behavior remains untouched.

### Horizon invariants

All three private boundary predictors use the same `horizon_seconds` value derived from the exact stored `NextTransition`:

- `None` means `f64::INFINITY`, matching current code;
- non-finite or `<= f64::EPSILON` returns `None` in the same locations as today;
- no predictor may clamp, recompute, or substitute another horizon; and
- transition materialization uses the exact owned `NextTransition` from which the horizon was derived.

This ensures the candidates and the cached transition share one causally consistent phase endpoint.

## Ordered implementation

1. **Prerequisite baseline:** Finish and accept prepared pocket geometry. Do not include either implementation in the same performance comparison.
2. **Characterize the committed one-ball scheduler fixture.** Record its complete expected event once outside timing and prove the unchanged duplicate call counts; do not add a duplicate benchmark simply because this plan predates `bench_pocket_predictors`.
3. **Record exact predictor/event goldens** on the unchanged post-prerequisite tree.
4. **Add `CurrentPhasePredictionContext::new`.** Unit-test source-state/config identity and phase/transition/horizon consistency for Rest, Sliding, Rolling, and Spinning states.
5. **Extract private context-consuming rail, jaw, and capture bodies.** Public wrappers build one context each. Make no loop/math changes and accept no parallel state/config arguments.
6. **Cut `PocketAwareEventCache::refresh_ball` over** to one context and move its transition after the three predictions. Leave pair handling and non-on-table branches untouched.
7. **Verify exact behavior and one-call construction**, then run the committed Criterion filters. Run adjacent public predictor and holistic guardrails separately.
8. **Only after acceptance**, allow later incremental-cache work to build on the private helper boundary; do not persist the context in this change.

## Benchmark plan

### Committed primary fixture

Use the already-added Criterion benchmark:

`pocket_predictors/scheduler/one_ball_slow_side_capture`

It constructs one valid on-table ball, default pool table/specs, and motion configuration outside `b.iter`; there is no pair prediction. The timed closure executes the full geometry-checked query, unwraps the `Result`, and black-boxes the complete `Option<NBallSystemEvent>`. Its current quick baseline is about 9.58 ms/call under the committed 8-second/20-sample group policy. The matched direct `pocket_predictors/capture/slow_side_30deg_hit` baseline is about 9.45 ms, which demonstrates that capture prediction dominates this scheduler fixture and is why accepted prepared geometry is a prerequisite.

Before collecting the post-prerequisite baseline, strengthen the untimed preflight from the current `is_some()` assertion to an exact event characterization: variant, ball index, pocket identity, time bits, and complete capture payload. Use a debugger breakpoint or temporary test-only counter to establish that the unchanged one-ball refresh performs exactly four `RawOnTableBallState::from_on_table` conversions, four radius conversions, four `classify_motion_phase` calls, and four `raw_compute_next_transition_on_table` calls across jaw, capture, rail, and transition preparation. Benchmark names and final `Some` output do not prove those counts.

### Future isolation extension, only if needed

If the accepted prepared-geometry result still leaves the committed capture fixture too dominant to resolve setup savings, add the future fixture

`pocket_predictors/scheduler/one_ball_rolling_center_transition`

using a center-table rolling ball whose exact next event is a normal motion transition or ordinary rail impact, not a jaw/capture or immediate event. Setup stays outside timing and the complete result is black-boxed. This is a future matrix extension, not a currently committed benchmark, and it must be baselined before the context implementation rather than substituted post hoc after seeing candidate results.

### Committed and existing controls

Run these as separate Criterion filters:

- `pocket_predictors/capture/slow_side_30deg_hit`;
- `pocket_predictors/capture/slow_side_30deg_target_miss`;
- `pocket_predictors/capture/fast_side_analytic_hit`;
- `pocket_predictors/jaw/fast_side_hit`;
- `pocket_cache_rebuild/one_ball_bank/event_limit_1`;
- `pocket_cache_rebuild/two_ball_pocket/event_limit_1`;
- `core_functions/compute_next_ball_rail_impact_on_table`;
- `core_functions/compute_next_transition_on_table/sliding`;
- `end_to_end/direct/pocket_aware_until_rest_cached`;
- `end_to_end/direct/pocket_aware_until_rest_manual`; and
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`.

The standalone rail, transition, capture, and jaw wrappers each still construct their own context once and are negative controls, not expected beneficiaries. The manual end-to-end control invokes public predictors independently and may not improve. Cache-rebuild and holistic cases are mixed integration controls.

### Paired procedure and statistical gate

Use the same pinned arm64 host, power mode, Rust toolchain, release profile, feature set, environment, and committed Criterion settings. The baseline must contain accepted prepared pocket geometry and no current-phase context change.

For each of three independent process pairs, save and compare the committed filter:

```text
cargo bench --bench physics -- 'pocket_predictors/scheduler/one_ball_slow_side_capture' --save-baseline phase-context-r1
cargo bench --bench physics -- 'pocket_predictors/scheduler/one_ball_slow_side_capture' --baseline phase-context-r1
```

Repeat as `r2` and `r3` from fresh processes. Report the relative mean point estimate and both 95% confidence bounds for all pairs, plus the median point estimate. If the future center-transition fixture is required, declare it primary before collecting all baselines and apply the same paired protocol; do not mix estimates from the two workloads.

Accept only when all are true:

1. the relative-mean 95% CI is wholly below 0% for all three primary comparisons;
2. the median primary improvement is at least 5%;
3. call-count corroboration proves exactly one shared preparation sequence in `refresh_ball`;
4. standalone rail, transition, jaw, and all direct capture filters have no 95% CI upper bound above +2%; and
5. cache-rebuild and cached/manual/pinball end-to-end controls each have no 95% CI upper bound above +2%.

The manual control may not improve because it invokes public predictors independently; that is expected. It must remain behaviorally exact and within the regression guardrail.

### Work-removal corroboration

For the one-ball fixture, use debugger breakpoint counts or temporary test-only counters in an instrumented non-timed run. After cutover, one `PocketAwareEventCache::refresh_ball` must produce exactly:

- one `RawOnTableBallState::from_on_table` for the jaw/capture/rail/transition boundary-preparation path;
- one `ball.radius.as_f64()` for that path;
- one `classify_motion_phase`; and
- one `raw_compute_next_transition_on_table`.

The before count is four of each. Ignore calls from validation or unrelated helpers only if stack traces prove they are outside boundary preparation; the one-ball fixture is chosen to make attribution unambiguous. Pair prediction contributes no calls because there is no pair.

Profile equal fixed invocation counts before/after. Require the three duplicate classification/transition stacks to disappear beneath `PocketAwareEventCache::refresh_ball`, with no new allocation or hash lookup attributable to `CurrentPhasePredictionContext`.

## Correctness and exact-output gates

Because this refactor only shares already-identical preparation, all outputs must be bit-for-bit identical. Before extraction, record golden signatures from the accepted post-prepared-geometry baseline. Permanent tests must compare complete `PartialEq` values where possible and `f64::to_bits()` for every exposed time/state scalar; do not weaken checks to tolerances.

### Context invariants

Add private unit tests proving for Rest, Sliding, Rolling, and Spinning inputs:

- `source_state` is the exact borrowed input used by the immediate-capture clone path;
- `motion` is the exact borrowed configuration used by phase classification, transition prediction, and all within-phase advancement;
- context raw components equal `RawOnTableBallState::from_on_table(source_state)` exactly;
- radius bits equal `ball.radius.as_f64().to_bits()`;
- phase equals current `classify_motion_phase`;
- `next_transition` equals current `raw_compute_next_transition_on_table` exactly; and
- `horizon_seconds` is the stored transition time bit-for-bit, or positive infinity exactly when transition is `None`.

Include a custom threshold/config case that changes classification or transition time. Add a compile-time/API-shape review or focused private test demonstrating that context-consuming helpers cannot be supplied a different state or motion configuration beside the context.

### Predictor and scheduler matrix

Require exact before/after outputs for:

1. a normal rail impact using `bank_state_near_top_rail` geometry;
2. the canonical curved CenterRight first-jaw fixture `curved_rolling_ball_reaches_the_center_right_first_jaw`;
3. the known CenterRight capture from `a_single_ball_heading_into_the_side_pocket_predicts_capture_before_the_rail` and its `rolling_toward_center_right_side_pocket` helper;
4. a center-table rolling ball whose next system event is `MotionTransition`;
5. a resting ball with no event;
6. a sliding state and a spinning-only state, exercising different transition payloads;
7. a horizon at or below `f64::EPSILON`, preserving category-specific early exits;
8. a pocketless table, proving public jaw/capture wrappers return before context construction, shared refresh leaves jaw/capture slots `None`, and rail/transition behavior remains unchanged;
9. an airborne system state, proving `refresh_ball` does not create an on-table context and retains table-bounce behavior; and
10. a pocketed state, proving all candidate slots remain cleared.

For scheduler cases compare full `Option<NBallSystemEvent>`: variant, indices, pocket/rail/jaw identity, transition phases, time bits, and every state payload component. Also run the existing jaw-vs-scheduler cross-check and all focused pocket-aware tests. Event precedence and tie ordering must be unchanged.

### Ownership/lifetime check

A focused cache unit test should refresh two distinct on-table balls sequentially with different phase/config-sensitive states and compare each candidate slot to its standalone public predictor. Then mutate/advance one state, rebuild/refresh, and prove only the new state's outputs appear. Include an immediate zero-time capture case and assert that its `state_at_capture` is exactly the borrowed source state, not a reconstructed raw state. This is an observable stale-context defense; do not test private source text or struct layout.

## Risks and mitigations

- **Stale context after mutation:** Persisting a context beyond one refresh would predict from the wrong state. Keep it call-local with immutable source/config borrows and add the sequential refresh/rebuild behavior test.
- **Horizon/transition mismatch:** Recomputing the horizon separately or advancing with another configuration could diverge from the cached transition. Derive both from the one owned `Option<NextTransition>` and use `context.motion`.
- **Zero-time representation drift:** Reconstructing `state_at_capture` from raw `f64` values could differ from the exact public input. Clone `context.source_state` as the current branch does.
- **Ownership regression:** Cloning `NextTransition` into the cache would replace duplicate compute with duplicate payload work. Borrow for predictions, then move the owned transition last.
- **Early-exit drift:** Moving context construction ahead of `table.has_pockets()` would slow pocketless direct calls. Preserve public jaw/capture guard placement.
- **Configuration omission:** A separate helper config could disagree with classification/horizon. Do not accept one; retain the constructor's immutable motion borrow.
- **Scope creep into pairs:** Pair prediction may perform its own classifications, but forcing a one-state context into a two-state API risks incorrect horizons. Leave it alone.
- **Masked benchmark:** Pocket scan cost can hide setup savings. Enforce the prepared-geometry prerequisite, the one-ball fixture, and call-count corroboration.

## Stop, rejection, and rollback

Reject or revise the implementation if any exact output changes, a helper can mix the context with another state/config, boundary preparation exceeds one classify/transition call per one-ball refresh, the context is persisted or partially keyed, the median isolated gain is below 5%, any paired isolated CI crosses 0%, or any control's upper confidence bound exceeds +2%.

Rollback is isolated: restore the four public calls in `refresh_ball` and remove the private context-consuming helpers/context. Do not leave an unused private abstraction or compatibility shim. Keep the committed `pocket_predictors` fixture; it predates the implementation and remains a useful cache-build workload.

## arm64 SIMD and GPU decision

**arm64 SIMD: not justified.** This change removes four repetitions of scalar classification/config conversion and one-state transition setup. The consumers are divergent jaw, capture, rail, and transition algorithms; there is no homogeneous lane-wise kernel for NEON.

**GPU: not justified.** A single ball supplies tiny scalar setup followed by branch-heavy predictors. Dispatch and data marshaling would exceed the eliminated work. Existing batch parallelism is a separate concern; this plan changes only per-ball CPU preparation.

## Candidate commit message

`perf: share current-phase context across boundary predictors`
