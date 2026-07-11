# Validated simulation resolver fast path

## Status

- **Status:** Accepted implementation candidate; benchmark-gated.
- **Priority:** P1. This removes work from every event resolved by the cached pocket-aware simulation loop.
- **Confidence:** High that redundant validation and cloning exist; medium that the isolated wall-time improvement will clear the acceptance threshold because pocket prediction dominates the longer workloads.
- **Dependencies/order:** None. This plan can land independently of prepared pocket geometry, shared prediction contexts, and cache representation work. Land and measure it before incremental cache invalidation so the ownership/validation effect remains attributable.

## Problem and evidence

### Measured facts

- The committed `pocket_cache_rebuild` matrix now measures `one_ball_bank/event_limit_{1,2,4}` and `two_ball_pocket/event_limit_{1,2,4}`. Current quick means span about 22.5–38.2 ms/call (one-ball event-limit 1 about 22.5 ms; one-ball limits 2/4 about 34–35 ms; two-ball cases about 37.5–38.2 ms). These calls include initial prediction, resolution, output recovery, and a post-event full cache rebuild, so they do **not** isolate resolver validation or cloning.
- The older quick snapshots put `end_to_end/direct/pocket_aware_until_rest_cached` at about 39.2 ms/call and `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8` at about 495–504 ms/call. The pocket profile is dominated by capture prediction, so the source-visible resolver cleanup must be accepted only with allocation/call-tree corroboration plus the narrowest committed event-limit cell, not inferred from the longer scenarios.

### Source facts

- `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit` validates the input once and owns the resulting `Vec<NBallSystemState>` for the loop.
- Every iteration nevertheless calls the public `resolve_n_ball_system_event_with_physics_and_pockets_on_table`.
- That public resolver revalidates its input, advances into a fresh vector, performs event resolution, and calls `validate_and_recover_n_ball_system_states` again on the output.
- `validate_and_recover_n_ball_system_states` allocates/collects an `original_indices` vector and an `on_table_states` vector, then clones the full input with `states.to_vec()` before writing recovered on-table states into it.
- The simulation loop clones every state unconditionally into `states_before`, but reads that snapshot only when `step_elapsed <= SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`.
- The loop owns `event` from `PocketAwareEventCache::next_event` but clones it into `events`. `NBallSystemEvent` derives `Clone` and contains owned payloads, including vectors in `SharedBallBallContact`.
- `NBallSystemSimulation` derives `PartialEq` and includes final states, total elapsed time, and the complete ordered event vector, so exact whole-result equivalence is directly testable.

### Hypothesis to test

Skipping only the redundant resolver-entry validation, avoiding positive-time state snapshots, and moving the already-owned event should reduce per-event allocation and copying. The one-event fixture should expose the effect; the existing long scenarios may show a smaller relative gain because event prediction remains unchanged.

## Scope

1. Add a private, zero-runtime-cost validated-state wrapper and a private resolver entry point that accepts only that wrapper. Instances may be created only by `validate_and_recover_n_ball_system_states` at the arbitrary-input boundary or by the retained output validation of the preceding private resolver call.
2. Keep the public resolver's validation contract unchanged.
3. Keep validation and geometry recovery of every resolved output unchanged.
4. Route only the internally owned, already-validated cached simulation state through the private entry point.
5. Clone `states_before` only for nonterminal zero-time/no-progress detection.
6. Move the owned event into the result vector after retaining the small facts needed later in the iteration.
7. Use the committed `pocket_cache_rebuild` fixture, exact equivalence coverage, and allocation/profile evidence. Add no duplicate one-event benchmark.

## Explicit non-goals

- No physics, collision, rail, pocket, tolerance, event ordering, or recovery-model change.
- No public API signature or public validation behavior change.
- No incremental or selective `PocketAwareEventCache` invalidation; the cache is still fully rebuilt after each nonterminal resolved event.
- No pocket scan, root-finding, target acceptance, or pocket candidate algorithm change.
- No attempt to remove output validation/recovery.
- No broad state-buffer reuse, unsafe aliasing, arena, `SmallVec`, or new allocator dependency.
- No compatibility shim or second public resolver API.

## Implementation design

### 1. Encode the validation trust boundary

In `src/lib.rs`, place the wrapper in a private child module so the crate-root code containing the resolver cannot invoke its tuple constructor directly:

```rust
mod validated_n_ball_system_states {
    use super::*;

    pub(super) struct ValidatedNBallSystemStates(Vec<NBallSystemState>);

    impl ValidatedNBallSystemStates {
        pub(super) fn validate(
            states: &[NBallSystemState],
            ball: &BallSetPhysicsSpec,
        ) -> Result<Self, NBallGeometryError> {
            validate_and_recover_n_ball_system_states(states, ball).map(Self)
        }

        pub(super) fn as_slice(&self) -> &[NBallSystemState] {
            &self.0
        }

        pub(super) fn into_inner(self) -> Vec<NBallSystemState> {
            self.0
        }
    }
}

use validated_n_ball_system_states::ValidatedNBallSystemStates;
```

The type and its checked methods are visible to the parent module, but the tuple field/constructor is not. Do not add `from_vec`, `new_unchecked`, `DerefMut`, `AsMut`, or any other path that can wrap or mutate arbitrary state without validation. The wrapper adds no allocation or copy beyond the existing validated vector and makes bypassing entry validation an explicit type error rather than a comment-only precondition.

Extract the body after the public resolver's current entry validation into a private function:

```rust
fn resolve_validated_n_ball_system_event_with_physics_and_pockets_on_table(
    states: &ValidatedNBallSystemStates,
    event: &NBallSystemEvent,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    collision_config: &BallBallCollisionConfig,
    rail_model: RailModel,
    rail_profile: &RailCollisionProfile,
) -> Result<ValidatedNBallSystemStates, NBallGeometryError>
```

Move the existing advance-and-match body into this function without changing branch order or event payload use. Its final operation remains full output validation:

```rust
ValidatedNBallSystemStates::validate(&states_after, ball)
```

That checked constructor performs the existing full output validation and recovery. It is required because collision and jaw resolution can produce tiny overlaps which the current recovery pass normalizes. The optimization removes only the redundant *entry* pass for state whose sealed private type proves the loop invariant.

The public `resolve_n_ball_system_event_with_physics_and_pockets_on_table` remains the arbitrary-input boundary:

```rust
let states = ValidatedNBallSystemStates::validate(states, ball)?;
resolve_validated_n_ball_system_event_with_physics_and_pockets_on_table(
    &states,
    event,
    ball,
    table,
    motion,
    collision_model,
    collision_config,
    rail_model,
    rail_profile,
)
.map(ValidatedNBallSystemStates::into_inner)
```

This is a clean cutover: there is one resolution implementation and two callers with different ownership/validation policy, not duplicated match bodies or a public escape hatch.

### 2. Use the trusted path only inside the cached loop

At `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`, create the initial state with `ValidatedNBallSystemStates::validate` and keep that type for the entire loop. Pass `states.as_slice()` to cache build, state comparison, and final result construction as needed. Replace the per-iteration call to the public resolver with the private validated function. The invariant remains true because:

1. the initial wrapper is created only by successful input validation;
2. every next wrapper is created only by retained output validation;
3. the wrapper constructor is sealed in its child module and every exposed constructor validates; and
4. no caller can inject or mutate arbitrary state through the wrapper between iterations.

Do not route `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table` or any public single-step API around entry validation. The existing manual benchmark is an adjacent correctness/control path and should remain unchanged.

### 3. Make the no-progress snapshot conditional

Compute `step_elapsed` and terminal status as today. Before resolution, snapshot only when the current no-progress branch can observe it:

```rust
let step_elapsed = event.time().as_f64();
let is_terminal_diagnostic = event.is_terminal_diagnostic();
let states_before = (!is_terminal_diagnostic
    && step_elapsed <= SIMULTANEOUS_EVENT_TOLERANCE_SECONDS)
    .then(|| states.as_slice().to_vec());
```

After successful resolution and the required cache rebuild, run the current consecutive-zero-time and `n_ball_system_collision_delta` logic only inside `if let Some(states_before) = states_before`. The positive-time branch still resets `consecutive_zero_time_events` to zero. Preserve these ordering constraints:

- snapshot before resolution;
- output validation before comparing states;
- terminal diagnostic break before cache rebuild/no-progress handling, exactly as today;
- rebuild the cache before the current nonterminal no-progress break, exactly as today;
- use the same inclusive `<= SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`, `MAX_CONSECUTIVE_ZERO_TIME_N_BALL_EVENTS`, and `SHARED_BALL_BALL_CONTACT_STATE_EPSILON` comparisons.

Do not replace the full-state comparison with an event-specific heuristic. A terminal diagnostic needs no snapshot because the current loop exits before the comparison.

### 4. Move the owned event

Before resolution, save only values still needed after the event is moved:

```rust
let step_elapsed = event.time().as_f64();
let is_terminal_diagnostic = event.is_terminal_diagnostic();
```

Resolution continues to borrow `&event`. Once it succeeds and elapsed time is updated, use `events.push(event)`, not `event.clone()`. Test terminal status using `is_terminal_diagnostic`. Do not push the event on resolver error; that preserves the current all-or-error behavior.

### 5. Keep unrelated ownership work out

`advance_n_ball_system_without_event` and output validation still construct their current result vectors. Do not combine this patch with an owned in-place advance or an owned recovery API: those are separate changes with separate correctness and allocation questions. The accepted contract here is removal of redundant entry work, conditional snapshots, and the event clone only.

## Benchmark plan

### Committed primary fixture

Use the already-added Criterion benchmark:

`pocket_cache_rebuild/one_ball_bank/event_limit_1`

It calls `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(..., Some(1))` with a one-element bank state and all specs constructed outside `b.iter`; the complete `NBallSystemSimulation` is black-boxed. Its current quick mean is about 22.5 ms/call under the committed 8-second/10-sample group policy.

The committed preflight currently proves only that the event vector is nonempty and no longer than the cap. Before the production change, strengthen it outside timing to assert exactly one `BallRailImpact { ball_index: 0, .. }`, elapsed time exactly equal to the event time, one returned on-table state, and a full exact simulation signature. This positive-time workload makes conditional snapshot removal observable, but its 22.5 ms includes two full `PocketAwareEventCache::build` calls (initial and post-resolution), prediction, resolution, and output recovery. Treat it as the narrowest public integration fixture, not as an isolated resolver microbenchmark.

Do not add the draft's differently named `end_to_end/direct/pocket_aware_event_limit_1/one_ball_rail`; it would call the same public API on materially the same workload and duplicate the committed fixture.

### Committed matrix and existing controls

Run all already-added cells separately:

- `pocket_cache_rebuild/one_ball_bank/event_limit_{1,2,4}`;
- `pocket_cache_rebuild/two_ball_pocket/event_limit_{1,2,4}`;
- `end_to_end/direct/pocket_aware_until_rest_cached`;
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`; and
- `end_to_end/direct/pocket_aware_until_rest_manual`.

The current event-limit cases span roughly 22–38 ms/call. Limits 2 and 4 can complete with fewer events than the requested cap under the present fixture; the committed assertion is `0 < events.len() <= max_events`. Report the actual preflight event count for every cell and never label latency “per event” using the requested cap. The manual path is the public-validation control and is not required to improve.

### Baseline protocol

On the same host, power mode, Rust toolchain, release profile, and committed Criterion configuration, save three independent pre-change baselines and compare three post-change processes against their corresponding baseline:

```text
cargo bench --bench physics -- 'pocket_cache_rebuild/one_ball_bank/event_limit_1' --save-baseline validated-fast-path-before-1
cargo bench --bench physics -- 'pocket_cache_rebuild/one_ball_bank/event_limit_1' --baseline validated-fast-path-before-1
```

Repeat with suffixes `-2` and `-3`; do not treat repeated samples within one Criterion process as three process runs. Record the relative-mean point estimate and both bounds of Criterion's 95% relative-change confidence interval. If the 10-sample committed policy is too noisy, change it before collecting both sides of every pair; do not compare unlike policies.

## Allocation and profile contract

Use an allocation call-tree profiler on a small benchmark-only driver or the filtered Criterion executable after warm-up, with exactly 1,000 one-event invocations in both builds. Fixture/spec construction is outside the counted region.

The implementation is accepted only when all of the following are observed:

1. The cached simulation call tree has no per-event *entry* call to `validate_and_recover_n_ball_system_states`; only the initial simulation validation and the post-resolution validation remain.
2. For the positive-time one-ball rail fixture, no allocation/copy stack is rooted in `Vec<NBallSystemState>::clone` for `states_before`.
3. No `NBallSystemEvent::clone` payload work occurs before `events.push`; the event vector's own capacity growth is allowed.
4. Relative to baseline, each resolved event removes the redundant entry pass's `original_indices`, `on_table_states`, and recovered-state `Vec` constructions. The private validated wrapper itself performs no allocation or copy. Report total allocations and bytes before/after as corroboration; do not claim zero total allocations because state advancement, output validation, both cache builds, and result vectors still allocate.
5. A nonterminal zero-time fixture used by the equivalence tests still takes the snapshot and preserves the current no-progress decision; a terminal diagnostic does not snapshot because that comparison is unreachable.

If call-tree attribution is unavailable or inlined beyond recognition, add temporary feature-gated counters around the three intended sites for measurement and remove those counters before landing. Do not ship a global counting allocator or permanent instrumentation for this change.

## Correctness and equivalence tests

Place these equivalence tests in a `#[cfg(test)]` module in `src/lib.rs`, where the reference loop can call the private `n_ball_system_collision_delta` helper and use the production constants exactly. Keep public-boundary geometry tests in `tests/n_ball_geometry.rs`; do not expose private helpers to integration tests.

Add focused tests before the production refactor, then keep them as regression coverage. The primary oracle is a test-local manual simulation loop which:

1. calls the public `compute_next_n_ball_system_event_with_rails_and_pockets_on_table` for each step;
2. resolves the selected event with the public `resolve_n_ball_system_event_with_physics_and_pockets_on_table`, thereby exercising the unchanged arbitrary-input validation boundary;
3. applies the same event limit, terminal diagnostic break, cache-equivalent recomputation, zero-time counter, and `n_ball_system_collision_delta` stop condition; and
4. returns `NBallSystemSimulation`.

For each fixture below, compare that reference result with `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit` using one `assert_eq!` on the full `NBallSystemSimulation`. Exact equality is required for final states (including every numeric field), elapsed time, event count/order/variant/indices, and every event payload:

- positive-time one-ball rail event using `bank_state_near_top_rail` geometry;
- curved CenterRight jaw event using `curved_rolling_ball_reaches_the_center_right_first_jaw`;
- CenterRight pocket capture using `a_single_ball_heading_into_the_side_pocket_predicts_capture_before_the_rail` and `rolling_toward_center_right_side_pocket`;
- ordinary two-ball collision using the established collision-predictor geometry;
- shared nonterminal zero-time contact using `system_zero_friction_nonideal_shared_contact_matches_coupled_normal_limit`;
- terminal unsupported airborne/on-table contact using `airborne_ball_ball_contact_is_reported_before_later_table_contact`; and
- an event limit of zero, which must return the initially recovered state, zero elapsed time, and no events even though the current function constructs its initial cache before entering the loop.

Also add a narrowly instrumented unit test for snapshot selection: a positive-time event and a terminal diagnostic must not request `states_before`; a nonterminal `0.0` event and a nonterminal time exactly equal to `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` must request it. Keep the predicate in a small private helper only if doing so makes this behavioral boundary testable without permanent counters; otherwise cover it through zero-time simulation fixtures. The nonterminal boundary must remain inclusive (`<=`).

Run focused verification only:

```text
cargo test --test n_ball_pockets
cargo test --test n_ball_geometry
cargo bench --bench physics --no-run
```

`n_ball_geometry` guards the public arbitrary-input validation/recovery contract. Do not weaken gross-overlap rejection or roundoff-scale recovery to make the fast path pass.

## Statistical acceptance gates

Accept the optimization only if all gates pass:

1. **One-event primary:** across three paired process comparisons, the Criterion 95% relative-mean CI is wholly below 0% in every run, and the median of the three point-estimate improvements is at least 5%. Because the committed call is cache-dominated, failure means this private boundary is not worth retaining; allocation removal alone does not override the latency gate.
2. **Committed cache matrix:** no `one_ball_bank` or `two_ball_pocket` cell, and neither cached nor pinball end-to-end control, has a 95% CI showing a regression greater than 2%. Compare complete-call latency and report actual event counts.
3. **Manual control:** `end_to_end/direct/pocket_aware_until_rest_manual` must remain output-equivalent and must not have a 95% CI showing a regression greater than 2%; it is not expected to speed up.
4. **Allocations/trust boundary:** all five allocation/profile conditions above hold, and the validated wrapper has no unchecked constructor or public visibility.
5. **Correctness:** every full-result equality fixture and existing focused test passes exactly; public gross-overlap rejection and roundoff-scale recovery remain unchanged, with no tolerance-based relaxation.

## Risks, stop conditions, and rollback

### Risks

- Calling the private resolver with unvalidated or stale state would bypass a public safety boundary. The private wrapper and lack of unchecked constructors make that misuse a type error.
- Moving the event before reading terminal status could change terminal handling or fail to compile through a use-after-move.
- Moving snapshot creation after resolution would compare the state with itself and break no-progress detection.
- Snapshotting terminal diagnostics would waste work but not change behavior; skipping snapshots for nonterminal times at or below tolerance would change behavior.
- Accidentally removing output recovery could allow small post-impact overlaps to escape.

### Stop/reject conditions

Reject or split the implementation if any of these occurs:

- the private resolver or validated wrapper must become public, or an unchecked wrapper constructor is needed;
- exact `NBallSystemSimulation` equality fails for any fixture;
- public overlap rejection/recovery changes;
- output validation is removed, deferred, or bypassed by manually wrapping arbitrary state;
- nonterminal zero-time no-progress behavior or terminal break ordering changes;
- the one-event primary gate misses 5% or has any paired CI crossing 0%;
- allocation traces still show the redundant entry pass, positive-time snapshot, or event payload clone; or
- an adjacent benchmark exceeds the 2% regression guardrail.

A failed latency gate means the extra private boundary is not pulling its weight even if it allocates less; retain the benchmark/equivalence tests if useful and revert the production split.

### Rollback

Rollback is local: restore the public resolver call, unconditional snapshot, and event clone in the loop, then remove the private validated wrapper and resolver entry point. Keep the committed `pocket_cache_rebuild` benchmarks; they predate the implementation and remain useful ownership/cache workloads. No data format, public API, or persisted artifact changes are involved.

## arm64 SIMD and GPU decision

- **arm64 SIMD/NEON:** Not applicable. The work removed is validation traversal, cloning, heap traffic, and branchy event dispatch. There is no wide homogeneous arithmetic loop to vectorize, and vectorization would not remove ownership or allocation cost.
- **GPU:** Rejected. A one-event state transition is latency-sensitive, branch-heavy, and operates on a tiny state set. Dispatch and transfer overhead would dominate, and GPU execution would complicate exact deterministic event ordering without addressing the redundant CPU validation boundary.

## Candidate commit message

`perf: bypass redundant validation in cached simulation`
