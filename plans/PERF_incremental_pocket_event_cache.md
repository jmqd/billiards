# Incremental pocket-aware event cache

## Status

- **Status:** Accepted implementation plan
- **Priority:** P0 after pocket prediction is stabilized; every resolved event currently rebuilds all unary and pair predictions.
- **Confidence:** High that dependency-local repair removes repeated predictor work; medium that cached deadlines can reproduce full-rebuild payload bits across every motion phase. The debug oracle and stop conditions below resolve that uncertainty before release.
- **Owner scope:** private `PocketAwareEventCache`, private resolution-effect reporting, the pocket-aware simulation loop in `src/lib.rs`, focused cache benchmarks in `benches/physics.rs`, and exact cache/full-rebuild equivalence tests.

## Dependencies and landing order

1. Add the benchmark matrices and a test-only full-rebuild oracle against the unchanged implementation; save the Criterion baseline.
2. Land `PERF_current_phase_prediction_context.md` so each dirty ball refresh computes phase/horizon once. Otherwise this plan still repeats avoidable unary setup and confounds attribution.
3. Land `PERF_validated_simulation_fast_path.md` first, or coordinate it as a prerequisite private resolver cutover. Incremental repair needs an already-validated resolver that reports which states it changed; it must not create a second resolver convention.
4. Land `PERF_prepared_pocket_geometry.md` and `PERF_adaptive_pocket_capture_search.md` before final cache measurements. Capture prediction is the dominant rebuild cost, and its output semantics must be frozen before predictions are persisted across epochs.
5. Before any prediction persists across rebuild epochs, make pair traversal and near-tie reduction deterministic and independent of `HashMap` iteration as described below. Freeze that ordering with exact event-stream tests; this prerequisite is correctness stabilization, not a license to change event precedence.
6. Introduce the cache clock without rebasing fresh relative durations, then add dirty-set and epoch-dependency reporting, then enable persistence only for proven-rebasable classes. Do not combine these phases in one unreviewable patch.

Dense pair storage, heap-based minimum selection, and scheduler scratch removal are separable follow-ups. The correctness cutover may retain `HashMap` lookup/storage, but `next_event` must traverse canonical pair indices and fold with the existing comparator in that fixed order. The current tolerance comparator is not associative, so calling the reduction “order-independent” or relying on randomized map iteration is prohibited.

## Problem and evidence

`PocketAwareEventCache` (`src/lib.rs:1981-2266`) currently stores:

- on-table ball-ball and unsupported-airborne pair predictions in two `HashMap<(usize, usize), ...>` values;
- jaw, pocket-capture, rail, table-bounce, and motion-transition predictions in per-ball vectors.

`PocketAwareEventCache::build` allocates all containers and calls `refresh_ball` for every index (`src/lib.rs:1993-2013`). `refresh_ball` recomputes all unary predictions for an on-table ball and then revisits every pair touching it (`src/lib.rs:2015-2116`). `next_event` scans every populated category and uses `earlier_pocket_aware_event_time_and_source`; candidates within `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` retain the existing source order (`src/lib.rs:1777-1951, 2118-2265`). Shared ball-ball contacts are materialized from all earliest-time pairs.

The simulation loop builds once at entry but then discards and rebuilds the entire cache after every resolved event (`src/lib.rs:11613-11616, 11651-11653`). That recomputes all $n(n-1)/2$ pair candidates and all unary candidates even when one rail impact or motion transition changed only one ball.

Measured facts:

- In the unchanged three-ball/eight-event profile, 3,842/3,893 samples were below `PocketAwareEventCache::build`; 3,476 were below pocket capture prediction and 3,470 below its scan.
- Quick Criterion results show no useful benefit from the current nominal cache: `end_to_end/direct/pocket_aware_until_rest_cached` is about 39.2 ms and the manual recomputation control about 39.6 ms. Earlier measurements in `plans/performance_engineering.md` were likewise essentially tied at about 48 ms.
- The committed `pocket_cache_rebuild` group already contains `one_ball_bank/event_limit_{1,2,4}` and `two_ball_pocket/event_limit_{1,2,4}`. Quick points include about 22.5 ms for one-ball cap 1, 34 ms for one-ball cap 2, and 37.5-38.4 ms across the two-ball pocket caps. These fixtures black-box complete simulations, but their preflight asserts only a nonempty event list of length at most the cap; they do not guarantee the requested count, classify dirty sets, isolate initial build, or establish ball-count scaling.
- The committed one-/two-ball cells are current evidence and must remain as adjacent controls. The `[2,4,9,15] x [1,2,4,8]` exact-count non-pocketing and early-capture matrices, initial-schedule controls, and repair counters described below are missing future harnesses identified by `agent://BenchCoverageScout`.

These are measured/source facts. The expected incremental slope is a hypothesis until the missing benchmark matrices and work counters verify it.

## Semantic contract and non-goals

The cache is an optimization only. Its public observable result must be exactly the same as rebuilding from the post-event state after every step:

- same ordered `NBallSystemEvent` sequence and event variants;
- same source indices/pairs and shared-contact grouping;
- same relative event times, pocket/rail identities, resolution metadata, and state-at-impact/capture payloads;
- same elapsed time, terminal diagnostics, zero-time stopping behavior, and final `NBallSystemState` vector;
- same deterministic tie ordering from `NBallPocketAwareSystemEventSource` and `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`.

Explicit non-goals:

- no physical model, target gate, collision, rail, jaw, airborne, or geometry-recovery change;
- no event-order or simultaneous-event tolerance change;
- no approximate spatial neighborhood invalidation: every pair touching a dirty ball is refreshed;
- no stale-candidate “validation at execution time” fallback; invalidation must be correct before selection;
- no public cache API and no backwards-compatibility wrapper for private cache internals;
- no pair heap/min-index optimization until incremental repair is independently proven and profiled.

If cached time normalization cannot reproduce the full-rebuild observable output, reject persistence for that prediction class or reject the plan. Do not hide drift behind a wider epsilon.

## Design

### 1. Separate exact fresh-relative times from optional persistent deadlines

Today every payload embeds a duration relative to the state from which it was predicted. Preserve that freshly computed `Seconds` value verbatim. Do not turn a fresh `relative` into `(now + relative) - now`: binary64 addition followed by subtraction is not lossless and cannot satisfy exact full-rebuild output.

Use explicit private slot state so a computed absence is cache state rather than indistinguishable from “never evaluated”:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RebasePolicy {
    FreshEachPositiveEpoch,
    CertifiedPersistent,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedResult<T> {
    predicted_epoch: u64,
    predicted_at_seconds: f64,
    original_relative_time: Option<Seconds>,
    absolute_deadline_seconds: Option<f64>,
    rebase_policy: RebasePolicy,
    prediction: Option<T>,
}

#[derive(Clone, Debug, PartialEq)]
enum CachedSlot<T> {
    Ineligible,
    Evaluated(CachedResult<T>),
}
```

`PocketAwareEventCache::build` becomes `build_at(states, ..., epoch, now_seconds)`. Every eligible unary/category or canonical pair slot receives `Evaluated`, including a predictor result of `None`; variant-inapplicable and pocketed rows use `Ineligible`. In the epoch in which a `Some` prediction is built, `next_event_at` compares and materializes its untouched payload time directly. `original_relative_time` and `absolute_deadline_seconds` are `None` for a cached absence. An absolute deadline is metadata only for a `CertifiedPersistent` hit that crosses a positive-time epoch; it must never round-trip an exact fresh prediction.

The default policy for every predictor result, including `None`, is `FreshEachPositiveEpoch`. A class may become `CertifiedPersistent` only after both of these are established:

1. a source-level proof that passive advancement cannot change `Some`/`None`, the first bracket/root, source identity, or state payload for that algorithm; and
2. an exact differential oracle showing that a retained absence or the rebased complete public event is bit-identical to a fresh prediction for all required fixtures, including ULP and tolerance boundaries.

If either condition is absent, any strictly positive elapsed step invalidates that class even when its ball indices were passively advanced. A duration satisfying the scheduler's “zero-time/no-progress” tolerance but greater than `0.0` still creates epoch invalidation. At exactly zero duration, a dependency-disjoint slot may be retained verbatim only if passive resolution leaves every dependency state bit-identical; zero duration alone is not sufficient.

Two current classes are unconditionally `FreshEachPositiveEpoch`:

- pocket capture, because both the current scan and the prerequisite adaptive search use a remaining-horizon-relative 512-cell semantic lattice; an old cached `None` can become a fresh `Some` when passive advancement changes the horizon and cell locations;
- unsupported-airborne ball-ball contact, because `predict_unsupported_airborne_ball_ball_contact` derives `sample_count` and sample times from the remaining horizon (`src/lib.rs:1548-1553).

Add the same classification for any curved ball-ball, jaw, rail, table-bounce, or transition path whose root/subdivision boundaries depend on the remaining horizon. Absence is an `Evaluated(CachedResult { prediction: None, .. })` and needs the same proof as a hit; never persist only hits while assuming misses are stable.

For a proposed `CertifiedPersistent` class, retain the original relative time and absolute deadline for a hit, but materialize a later relative value only through a class-specific, proven exact rebasing implementation. A generic `deadline - now` helper is prohibited. A certified absence needs an explicit proof of absence stability, not a synthetic deadline. If exact rebasing or absence stability cannot be supplied, keep the class fresh. This may reduce the attainable speedup; the statistical stop conditions decide whether the conservative implementation remains worthwhile.

Phase one keeps full rebuilds after every resolved nonterminal event and merely threads `(epoch, now)` through the cache while returning untouched relative payloads; an exact-zero step does not increment the epoch. Use the existing accumulated `elapsed` as the one authoritative clock: after the existing `Seconds::new(elapsed.as_f64() + step)` operation, set `now_seconds = elapsed.as_f64()`. Do not maintain a second independently accumulated `f64` clock. Require exact old/new simulation equality before enabling any persistence. This isolates clock plumbing from invalidation and rebasing.

### 2. Report resolution effects instead of guessing from the primary event

The primary event is not a sufficient invalidation map. `resolve_n_ball_system_event_with_physics_and_pockets_on_table` can also:

- resolve disjoint same-time collisions not encoded as the primary pair (`src/lib.rs:11379-11410`);
- apply the TP B.29 three-ball line-contact resolution (`src/lib.rs:11366-11377`);
- propagate shared zero-time contact resolution (`src/lib.rs:11345-11355, 11413-11420`);
- turn a jaw impact into a terminal pocket capture (`src/lib.rs:11422-11444`);
- run validation/recovery after resolution (`src/lib.rs:11472`).

Make the already-validated private resolver return:

```rust
struct ResolutionEffects {
    states: Vec<NBallSystemState>,
    dirty_balls: DirtyBalls,
}

struct DirtyBalls {
    bits: Vec<bool>, // one bit per stable system index; a bitset is optional after profiling
}
```

Mark every index whose kinematic/state trajectory may differ from passive advancement:

- `MotionTransition`: the named ball, even though the event match arm does not assign a state; it reached a phase boundary and its next-phase predictions must be rebuilt;
- `BallRailImpact`, `BallJawImpact`, `BallPocketCapture`, `BallTableBounce`: the named ball;
- ordinary `BallBallCollision`: both primary balls plus every ball touched by TP B.29, disjoint simultaneous collision resolution, or the subsequent shared-contact cascade;
- `SharedBallBallContact`: every ball actually touched by the coupled solver, conservatively at least every reported `ball_index` and every endpoint of `ball_ball_pairs`;
- `UnsupportedAirborneBallBallContact`: terminal diagnostic, so no repair is needed before loop exit;
- validation/recovery: every index whose state is moved or otherwise rewritten by recovery;
- exact-zero passive advancement: conservatively mark every `Airborne` index dirty because the current `advance_airborne_ball(..., Seconds::zero())` path reconstructs exact-decimal position/height/vertical velocity through `f64`. The on-table path currently returns an exact clone at `dt == 0`; assert that invariant rather than assuming all variants behave alike. A future exact airborne clone fast path may remove that invalidation only after separate exact-output proof.

Thread a private dirty marker through resolution helpers that mutate states. Extend geometry recovery's private result to report changed indices. Do not reconstruct dirtiness solely from the public event after the fact. “Dirty” includes a consumed event dependency such as `MotionTransition`, even where the final state equals ordinary passive advancement.

In test/debug builds, independently clone the passively advanced pre-resolution states, compare them with the final validated states, and assert that every unequal index is in `dirty_balls`. Separately assert that every consumed event dependency is dirty. These checks include passive rewrites and event consumption, not only response mutations. Over-invalidation is allowed; under-invalidation is a correctness failure. The release build must not retain this full-vector clone solely for checking.

System indices are stable for the entire simulation: pocketed balls remain as inert entries rather than being removed. This lets unary slots and pair keys remain index-addressed across every event.

### 3. Encode state and epoch dependencies conservatively

Index dependencies alone are insufficient. Every slot has a state dependency and a time-advance policy:

- unary jaw/capture/rail/table-bounce/transition prediction for ball `i`: `State(i)` plus its `RebasePolicy`;
- on-table collision or unsupported-airborne contact for canonical pair `(min(i,j), max(i,j))`: `State(i) | State(j)` plus its `RebasePolicy`.

After resolving an event, form two invalidation sets:

1. **state invalidation:** every unary slot whose ball is in `dirty_balls`, and every pair slot with a dirty endpoint. `dirty_balls` includes conservatively known passive rewrites such as every airborne state advanced through the current zero-duration reconstruction path;
2. **epoch invalidation:** after any strictly positive step, every `FreshEachPositiveEpoch` slot, including dependency-disjoint passively advanced balls and pairs. This explicitly includes every on-table pocket-capture slot and every eligible unsupported-airborne pair slot. A zero-time step creates no epoch invalidation, but its passive-rewrite dirty set still applies.

Then repair the union:

1. clear invalidated unary slots and both invalidated pair categories;
2. recompute each invalidated ball/category exactly once from the current state, storing its fresh relative payload unchanged;
3. recompute each invalidated unordered pair/category exactly once, canonicalizing `(i,j)` and deduplicating overlap between state and epoch invalidation;
4. leave a slot unchanged only when neither its state dependencies nor its epoch policy invalidated it;
5. if a state became `Pocketed`, leave its unary row empty and clear every pair touching it;
6. if a state changed between `OnTable` and `Airborne`, rebuild its table-bounce slot and all touching pairs under the new variant; never manufacture planar jaw/rail/capture events for an airborne state.

Use generation-marked category/ball and category/pair work lists, or deterministic nested index loops, so overlap never causes duplicate predictor calls and no per-event `HashSet` is allocated. For state-local repair of a `CertifiedPersistent` pair class, $k$ dirty balls among $n$ require

$$k(n-k) + \frac{k(k-1)}{2}$$

unique pair refreshes; the common $k=1$ case is `n-1`. A `FreshEachPositiveEpoch` pair class instead refreshes all eligible pairs after a positive step, and the work counters/benchmarks must report that honestly. Do not cite the local formula for horizon-sensitive classes.

Add explicit stale-absence regressions:

- a pocket-capture state for which a cached `None` under the old remaining horizon becomes `Some` after passive advancement changes the adaptive/legacy lattice;
- an airborne pair for which the horizon-derived `sample_count`/sample locations change an old `None` to `Some`;
- the equivalent `Some` bracket/time shifts;
- a disjoint zero-time on-table collision with an airborne state seeded at non-binary exact-decimal coordinates/height/vertical velocity, proving the reconstructed airborne state and every dependent cached payload refresh exactly as a full rebuild does.

Those tests must show that state/epoch invalidation recomputes each affected slot before selection. Seeded differential tests alone are not a proof of absence stability or passive bit identity.

A whole-cache rebuild remains permitted only for initialization and the test/debug oracle. Category-wide epoch repair is not a full rebuild: storage remains allocated, certified classes remain cached, and each invalidated slot—including a cached `None`—is refreshed once. No production “unknown event => rebuild” branch is acceptable once all current event and predictor classes are exhaustively classified.

### 4. Make selection deterministic before persistence

The current pair maps cannot define traversal order: `HashMap` iteration is randomized, while the pairwise tolerance comparator is non-associative and can choose different near-tie winners when candidates are encountered in different orders. Rebuilding new maps and mutating one persistent map can therefore produce different primary pair sources even when every cached value is correct.

Before enabling persistence, make selection deterministic while preserving the comparator and the current fixed category order:

1. traverse sources in exact `NBallPocketAwareSystemEventSource` order: on-table pair keys in canonical nested order `for i in 0..n { for j in i+1..n { ... } }`, then unsupported-airborne pairs in the same canonical order, then the existing jaw/capture/rail/table-bounce/transition category and ascending ball-index order, using maps only for lookup;
2. apply `earlier_pocket_aware_event_time_and_source` unchanged as a left fold in exactly that order. A strictly earlier candidate still replaces the current candidate because that is the comparator's first branch, even inside the simultaneous tolerance; an exact-time candidate uses source order. Do not call this fold order-independent, reorder it, construct a `minimum_time + tolerance` eligible set, or replace the current subtraction/absolute-difference operations;
3. compute `earliest_ball_ball_time` with the existing numeric `min`, then collect shared-contact pairs by canonical pair traversal before the existing sort/dedup/materialization.

This freezes the current fixed unary/category behavior and resolves only pair-map traversal ambiguity; it does not redefine near-tie precedence. Add a prerequisite matrix containing every category pair, disjoint pair indices, monotone three-candidate chains that expose non-associativity, strictly earlier/later candidates inside the simultaneous tolerance, exact ties, and subtraction-boundary values. Require every established exact event-stream/final-state fixture to remain unchanged. Because randomized `HashMap` pair order has no stable observable definition, record the canonical pair-index expectation before cache persistence and use the same traversal in both full-rebuild and incremental oracles. Any proposed precedence change beyond canonical pair traversal is a separate correctness change and blocks this plan.

`next_event_at` must use untouched fresh-relative payloads for the current epoch. It may use a later-epoch candidate only when that slot is `CertifiedPersistent` and its class-specific exact rebasing contract has passed. The resolver still receives a relative event exactly as today. Simulation elapsed accumulation and no-progress detection remain unchanged.

### 5. Add a full-rebuild debug oracle

Retain a test/debug-only `PocketAwareEventCache::full_rebuild_at`. After every incremental repair:

1. build a fresh cache from the resolved states at the same `(epoch, now)`;
2. canonicalize both caches by slot category/source/index, never by `HashMap` iteration order;
3. require exact equality of `Ineligible` versus `Evaluated`, cached `Some` versus cached `None`, category identity, policy/epoch metadata, and original fresh-relative values;
4. require every evaluated slot omitted from epoch repair to be `CertifiedPersistent` and compare a retained absence or its rebased complete payload exactly with the fresh result;
5. compare deterministic `next_event_at(epoch, now)` as a complete `Option<NBallSystemEvent>` with `assert_eq!`;
6. compare a complete full-rebuild simulation with the incremental simulation using `assert_eq!`.

Any semigroup, horizon, sampling, or rounding mismatch demotes that class to `FreshEachPositiveEpoch`; do not relax the oracle. Persist a class only when its source proof and exact observable equivalence are both established.

The oracle must run for every event in deterministic fixture matrices and seeded legal-state fuzz/property tests. It is absent from release benchmarks.

### 6. Clean cutover

The final simulation loop preserves the current event-limit, terminal, refresh, and no-progress ordering:

```text
validate once
epoch = 0; elapsed = Seconds::zero(); now_seconds = elapsed.as_f64()
cache = build_at(states, epoch, now_seconds)
loop:
  stop if event limit was already reached
  event = cache.next_event_at(epoch, now_seconds) or stop
  states_before = states
  resolve already-validated event -> states + dirty_balls
  step = event's untouched relative time
  elapsed = Seconds::new(elapsed.as_f64() + step); now_seconds = elapsed.as_f64()
  if step > 0 then epoch += 1
  append the same event
  if terminal diagnostic: stop without repair
  cache.refresh_invalidated_at(dirty_balls, step > 0, states, epoch, now_seconds)
  apply the existing <= SIMULTANEOUS_EVENT_TOLERANCE_SECONDS cascade count,
    MAX_CONSECUTIVE... cap, and state-delta no-progress stop exactly as today
```

In particular, an `UnsupportedAirborneBallBallContact` is still resolved as passive advancement to its contact time, contributes exactly once to elapsed/events, preserves the resulting advanced states and complete diagnostic payload, and exits before cache repair. Event-limit checking remains at loop entry, so a nonterminal final capped event receives the same repair placement as the current post-event rebuild.

Delete the unconditional `cache = PocketAwareEventCache::build(...)` at `src/lib.rs:11651-11653`. Keep no runtime full-rebuild mode or compatibility alias. The test/debug oracle remains private.

## Benchmark plan

### Committed quick controls

Keep `pocket_cache_rebuild/one_ball_bank/event_limit_{1,2,4}` and `pocket_cache_rebuild/two_ball_pocket/event_limit_{1,2,4}` as current adjacent controls. Record the quick unchanged-tree points (about 22.5 ms and 34 ms for one-ball caps 1 and 2; about 37.5-38.4 ms across the two-ball caps), but do not label them scaling or differential coverage. Strengthen their untimed preflight to assert the exact complete simulation and actual event count for each cell; if a fixture naturally terminates before its requested cap, name and interpret it as that fixed endpoint rather than as an `events_e` scaling cell.

### Missing initial-schedule control

Add Criterion group `pocket_cache_initial_schedule/{2,4,9,15}`. Each cell calls public `compute_next_n_ball_system_event_with_rails_and_pockets_on_table` once on deterministic, non-overlapping states and black-boxes the complete `Result<Option<NBallSystemEvent>, _>`. This includes validation, initial allocation/build, deterministic selection, and result materialization; label it an initial-schedule control rather than a pure cache-build benchmark. It distinguishes the unchanged initial $O(n^2)$ path from loop repair without exposing the private cache.

Fixture state/config construction and exact complete expected-event assertions stay outside `b.iter`.

### Ball-count by event-count scaling

Add group `pocket_cache_rebuild/non_pocketing/balls_{n}/events_{e}` for every cross product:

- balls: `[2,4,9,15]`;
- event limits: `[1,2,4,8]`.

Use deterministic, non-pocketing workloads dominated by one-ball rail impacts and motion transitions, with non-overlapping lanes and no accidental pair collisions. Confirm outside timing that each fixture produces exactly `e` events and that each event dirties the intended number of balls. Time `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit` and black-box the complete `NBallSystemSimulation`.

Add a second family `pocket_cache_rebuild/early_capture/balls_{n}/events_{e}` using one early side-pocket capture plus continuing deterministic motion among the remaining balls. Assert the captured ball becomes inert, its row/column stays empty, and the requested event count is reached where physically possible. If a cell cannot naturally reach `e`, construct a deterministic fixture that can; do not time an early-terminating substitute under that name.

Report total latency, ns/event after subtracting the matched initial-schedule control only as a secondary derived metric, and fitted slopes against pair count and event count. Primary acceptance uses complete-call latency.

### Density/invalidator controls

Add these exact focused cells under `pocket_cache_repair`:

- `one_ball_transition_15` — one state-dirty ball plus positive-epoch invalidation; require all 15 capture slots to refresh and report every other category by policy;
- `one_ball_rail_15` — the same positive-epoch accounting for a rail response;
- `two_ball_collision_15` — two state-dirty balls plus positive-epoch invalidation;
- `shared_three_ball_contact_15` — at least three state-dirty balls and all touched pairs;
- `zero_time_shared_contact_15` — no epoch invalidation, isolating dependency-local repair;
- `pocket_capture_15` — one inert row/column removal plus positive-epoch invalidation;
- `airborne_table_bounce_15` — variant-changing state repair and epoch-wide refresh of unsupported-airborne sampled pairs.

Use test-only work counters outside Criterion to assert the full-build count, dirty indices, and unary/pair refresh counts per category and reason (`state`, `epoch`, or both) before timing. For a certified pair class with one state-dirty ball, require 14 rather than 105 refreshes; for a fresh-each-positive-epoch class, require all eligible slots and never report that work as local. Do not expose counters in the public API.

### Existing adjacent/end-to-end filters

Compare:

- `end_to_end/direct/pocket_aware_until_rest_cached`;
- `end_to_end/direct/pocket_aware_until_rest_manual`;
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`;
- the committed and missing `pocket_predictors/capture/*` branch cells from the adaptive-search plan as adjacent predictor guards.

Manual stepping is a correctness/performance control and is not expected to improve. Predictor microbenchmarks must not change because this plan only changes cache ownership and refresh scheduling.

## Correctness and equivalence tests

### Exact full-rebuild differential

Expose two private test entry points: the current full-rebuild-after-every-event loop and the incremental loop. For every fixture, require exact equality of the complete `NBallSystemSimulation` with `assert_eq!`, including:

- elapsed `Seconds`;
- event count and ordered variants;
- all event times, indices/pairs, pocket/rail identities, contact-resolution metadata, and prediction state payloads;
- final state variants, positions, velocities, angular velocities, airborne height/vertical velocity, and pocketed capture states.

Required fixtures:

- all `[2,4,9,15] x [1,2,4,8]` non-pocketing and early-capture benchmark setups;
- one-ball transition, rail, jaw, capture, table-bounce, and rest/no-event paths;
- ordinary ball-ball impact, disjoint simultaneous impacts, TP B.29 three-ball line contact, shared-contact graph, and zero-time cascade;
- jaw impact that late-drops into a pocket;
- on-table-to-airborne and airborne-to-table transitions, plus terminal unsupported-airborne contact;
- a disjoint zero-time on-table collision plus a non-binary-exact-decimal airborne state, requiring exact rewritten airborne state and dependent payload equality;
- geometry recovery that changes an event participant and a recovery case that moves an additional ball;
- a pocket capture while another ball continues spinning/moving (`tests/n_ball_pockets.rs:1716-1826`);
- existing cached-versus-manual fixtures (`tests/n_ball_pockets.rs:1886-1993`);
- every source-order tie within, at, and just outside `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`;
- event limit zero/one/eight, natural rest, and `MAX_CONSECUTIVE_ZERO_TIME_N_BALL_EVENTS`/no-progress termination.

A fixed-seed legal-state corpus must run both loops for bounded event counts and compare complete results. It supplements, not replaces, targeted invariants.

### Dependency-map tests

For each event variant, assert the exact or conservative dirty set. In debug tests, compare passive advancement to final resolved states and fail if any changed index lacks a dirty bit. Then assert:

- a slot is retained across an exactly zero-duration event only when no state dependency is dirty and passive resolution left every dependency state bit-identical;
- after every strictly positive event—including one at or below the no-progress tolerance—every `FreshEachPositiveEpoch` unary/pair slot refreshes exactly once even when its indices were passively advanced;
- a `CertifiedPersistent` hit with no dirty state dependency retains its epoch/deadline and rebases through only its class-specific exact implementation; a certified cached absence retains no synthetic deadline and must match a fresh absence;
- dirty and epoch invalidation overlap never refreshes a slot twice;
- every pair with a state-dirty endpoint is cleared/recomputed;
- pocketed rows have no unary or pair candidates;
- category changes between on-table and airborne clear incompatible slots;
- for a certified pair class, one state-dirty ball among 15 causes 14 rather than 105 pair predictions;
- the stale pocket-capture and unsupported-airborne `None -> Some` fixtures refresh before selection;
- the disjoint zero-time/non-binary-decimal airborne fixture marks the airborne ball dirty and refreshes its table-bounce and every touching unsupported-contact payload.

### Clock/deadline tests

Exercise long sequences, strictly positive steps, exactly zero-time sequences, and positive steps immediately below/at/above `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`. For every selection, compare incremental `next_event_at(epoch, now_seconds)` exactly with a fresh `build_at(..., epoch, now_seconds).next_event_at(epoch, now_seconds)`. Include candidate times one ULP around `now_seconds`, simultaneous-tolerance subtraction boundaries, large accumulated elapsed values, and the explicit horizon/sample-lattice stale-absence cases.

Assert that a fresh prediction's original relative `Seconds` is returned without any add/subtract deadline round trip and that `now_seconds.to_bits() == elapsed.as_f64().to_bits()` after every accumulation. For each proposed persistent class, directly test retained-absence identity or its class-specific hit rebase identity against a fresh prediction. Negative/non-finite remaining time, a changed `Some`/`None`, or any payload inequality demotes the class to `FreshEachPositiveEpoch`; it is not clamped or tolerated.

Do not accept approximate event-sequence or final-state comparisons. If conservative epoch invalidation erases the measured benefit, reject the implementation rather than persisting an unproved class.

## Statistical acceptance gates

Use the same release profile, Rust toolchain, host, power mode, and Criterion configuration before and after. Save a baseline from the unchanged revision. Run at least three paired before/after processes on an otherwise idle host. Use at least 30 samples and 8-10 seconds measurement for repair/initial-schedule cells; use at least 20 samples for slow simulation cells. Report Criterion 95% bootstrap confidence intervals, outliers, fitted event-count slopes, and fitted ball/pair-count slopes.

Ship only if all gates pass:

- `pocket_cache_rebuild/non_pocketing/balls_9/events_8` and `/balls_15/events_8`: relative-time CI wholly below zero in all three runs and pooled median improvement at least 20%;
- for 9 and 15 balls, the fitted incremental cost per additional local event improves by at least 50% and its 95% CI does not overlap the baseline slope;
- `early_capture/balls_15/events_8`: wholly negative CI and median improvement at least 15%;
- initial-schedule controls and every exact-count `events_1` scaling cell show no regression greater than 3% at 95% confidence;
- `shared_three_ball_contact_15` and `airborne_table_bounce_15` show no regression greater than 3%; they are correctness-heavy guard paths even if their dirty sets are larger;
- the existing cached until-rest and three-ball/eight-event cells do not regress by more than 3%, and at least one improves by 3% with a wholly negative CI;
- manual stepping and direct pocket predictor cells do not regress by more than 3%;
- counters show one initial full build, zero production whole-cache rebuilds after resolved nonterminal events, exact per-category state/epoch invalidation counts, and no duplicate refresh when the causes overlap;
- a corroborating profile of the 15-ball/eight-event local workload shows `PocketAwareEventCache::build` only at initialization and removes repeated work for every certified-persistent class. It must also show the expected positive-epoch capture and sampled-airborne refreshes; do not claim those classes were cached. Timing alone is insufficient.

Inspect and rerun noisy/discordant cells. Do not claim success from the two-ball endpoint alone or from a lower point estimate whose CI crosses zero.

## Risks, stop conditions, and rollback

- **Stale deadline/payload:** any full-rebuild oracle mismatch is a release blocker. Demote the class to `FreshEachPositiveEpoch` or stop; never validate lazily after selecting a stale event.
- **Hidden resolver mutation:** the passive-state dirty-set assertion catches unreported TP B.29, disjoint, shared-contact, late-drop, and recovery effects. Any under-invalidation rejects the dependency map.
- **Horizon-relative stale absence:** capture and unsupported-airborne sampling are epoch-invalidated after every positive step. Any newly found horizon/sample-dependent class receives the same policy; seeded coverage is not used as proof of stability.
- **Floating rebasing drift:** exact public-event and full-simulation equality is mandatory. Fresh relative values never take an absolute-deadline round trip; unproved persistent rebasing is rejected rather than tolerated or clamped.
- **Tie-order drift:** canonical pair traversal and the fixed-order left fold with the unchanged non-associative comparator must be frozen before persistence. Any different established event winner rejects the cutover and is resolved separately, never by relying on `HashMap` iteration.
- **Repair overhead:** if 9/15-ball event slopes do not meet the gates or one-event/initial-schedule controls regress, remove persistence and retain only benchmark/oracle coverage.
- **Over-invalidation:** safe but may erase performance. Measure state-versus-epoch unary/pair counts; seek a source proof for a predictor class rather than weakening correctness.
- **Memory growth:** stale slots must be replaced/cleared, not accumulated by epoch. Allocation/profile runs must show bounded cache storage.

Rollback restores the unconditional post-event `PocketAwareEventCache::build`; benchmark matrices, dirty-effect tests, and the full-rebuild oracle remain valuable. Do not retain a runtime incremental/full feature switch.

## SIMD and GPU decision

arm64 SIMD is rejected. The target is eliminating predictor invocations through irregular dependency invalidation over small ball counts, variant-rich states, branch-heavy roots, and deterministic event ordering. NEON cannot reduce the dominant unnecessary $O(n^2)$ recomputation, and vectorizing pair repair before scalar invalidation is proven would add complexity without profile support.

GPU offload is rejected. Each event depends serially on the previous resolution, common systems contain 2-15 balls, transfer/dispatch and synchronization dominate, and exact CPU event ordering/payloads must be retained. The existing Rayon batch model for independent simulations is the appropriate outer parallelism; this plan does not add manual chunking.

## Candidate commit message

`perf(physics): repair pocket event cache incrementally`
