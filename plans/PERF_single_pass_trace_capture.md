# Capture Scenario Trace History During the Authoritative Simulation Pass

## Status and decision

- **Status:** Accepted benchmark-gated implementation plan; not yet implemented.
- **Priority:** Medium pending event-dense matched evidence.
- **Confidence:** High that a complete second event-resolution pass exists; low-to-medium on observable total speedup because the newly committed matched three-ball benchmark shows only a small quick-run gap and pocket prediction dominates.
- **Order/dependencies:** Independent of `PERF_playback_cursors.md` and scheduler/cache plans. Preserve the already committed matched system-versus-trace fixture while implementing against the current scheduler; rebase mechanically after scheduler work if necessary. The observer contract must not own scheduler decisions.
- **Compatibility decision:** Keep all public simulation and `DslScenario::simulate_shot_trace_*` signatures and owned result types. Add only crate-private observer/driver machinery. Cleanly remove `ball_traces_from_simulation` after every trace caller uses capture; do not retain a replay shim.

## Problem and evidence

### Observed facts

`src/dsl.rs::DslScenario::simulate_shot_trace_with_physics_on_table_until_rest` and `simulate_shot_trace_with_physics_on_table_until_event_limit` currently perform these phases:

1. construct initial system states;
2. call the full pocket-aware N-ball simulation;
3. rebuild `event_log` from the stored events; and
4. call `DslScenario::ball_traces_from_simulation`.

`ball_traces_from_simulation` clones initial states into `current_states`, then for every stored event:

- advances every live ball over the event interval to create timeline start/end states;
- formats the event again and creates visible segment marker strings; and
- calls `resolve_n_ball_system_event_with_physics_and_pockets_on_table` again for the complete system.

It therefore replays every event after the authoritative simulation has already advanced and resolved it.

The authoritative loop is `src/lib.rs::simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`. It selects an event from `PocketAwareEventCache`, clones `states_before` for zero-time progress detection, resolves the event, updates elapsed time, stores the event, applies terminal/no-progress rules, and rebuilds the cache. The resolver already calls private `advance_n_ball_system_without_event`, producing the authoritative states at the event instant before impact/transition resolution. Those pre-resolution values are precisely the timeline endpoints that the DSL layer recomputes.

`NBallSystemSimulation` intentionally stores only final states, elapsed time, and events, so it cannot supply intermediate history after completion. This plan does not add history to every simulation result; it makes capture optional during the existing pass.

Current committed benchmark evidence:

- `end_to_end/dsl/preparsed_three_ball_pinball_system_only_event_limit_8` is about **172 ms/call** in a quick run.
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8` is about **174 ms/call** in the corresponding quick run.
- The roughly **2 ms/call** difference is a single quick comparison, not paired statistical evidence, and is too small to support the previous assumption of a large trace-replay cost.
- The system-only helper deliberately builds initial DSL shot states inside each timed iteration, as the trace entry point does internally, before calling the core event-limit simulator. This makes the committed pair materially closer than a prepared-core-only comparison, but only the candidate matrix and paired runs can attribute the residual delta.
- Profiles still show pocket prediction/cache work dominates the fixture, so a large total win is not assumed.
- The nine-ball break fixture contains 10 balls and a 32-event trace cap and remains the required future event-dense workload; the cap must not be reported as an observed event count.

### Hypothesis to verify

A generic no-op observer intended to compile away on ordinary simulation, plus an owning DSL trace collector for traced simulation, can consume authoritative interval-start, pre-response, and post-response state borrows during the first pass. This should remove the second resolver invocation and its state-vector/validation work while preserving the exact scheduler loop. Release-code inspection and the ordinary-path benchmark must verify that the no-op instantiation adds no measurable dispatch/allocation cost. The trace output still must own its timeline/visible states and strings, so retained output size will not disappear. The committed ~172 ms versus ~174 ms quick pair means this is now primarily an incremental-overhead and dense-fixture hypothesis; do not promise a 5% win on the current three-ball total.

## Scope

### In scope

- `src/lib.rs`: factor event resolution only enough to expose borrowed before/pre-resolution/post-resolution step observations; share one scheduler loop between observed and unobserved simulation.
- `src/dsl.rs`: a trace collector that builds `ScenarioShotTraceEvent`, `ScenarioBallTimelineSegment`, `BallPathSegment`, and final states during that pass.
- `benches/physics.rs`: matched simulation-only versus simulation-plus-trace event/ball scaling.
- `benches/throughput.rs`: a secondary downstream trace-plus-playback sample-step matrix to prove complete consumer equivalence, not to attribute trace-capture CPU cost.
- Focused structural equivalence, scheduler noninterference, error, allocation, and peak-live-memory tests.

### Non-goals

- No parser cleanup.
- No change to event prediction, cache construction/rebuild, tie-breaking, simultaneous-contact policy, zero-time stopping, maximum-event behavior, validation/recovery, physics coefficients, integration, collision response, pocket capture, or terminal diagnostics.
- No change to `NBallSystemSimulation`'s public fields and no always-on history vector.
- No physics/cache optimization, no incremental cache work, and no playback cursor implementation in this change.
- No approximate trace comparison, event coalescing, output reformatting, label change, or new serialization format.
- No observer callbacks that may alter states, events, cache, or control flow.

## Observable contract

For identical inputs, ordinary and traced simulation must preserve:

- exact `Result` success/error variant and error payload;
- exact `NBallSystemSimulation` equality: final states, elapsed `Seconds`, ordered `NBallSystemEvent` values and all nested data;
- identical maximum-event semantics, including `max_events = 0`;
- identical terminal-diagnostic inclusion and stopping;
- identical consecutive-zero-time counter and no-progress decision;
- identical cache build/rebuild timing and selected event sequence;
- identical deterministic simultaneous/shared-contact ordering.

For traces, the entire old and new `ScenarioShotTrace` must compare equal through its derived `PartialEq`, including:

- simulation;
- event-log time and event kind;
- ball trace order/type/initial/final state;
- every visible `BallPathSegment` field, including marker boolean, label, title, duration, and exact start/end states;
- every `ScenarioBallTimelineSegment` start time, duration, start, and end state;
- cloned ball-set and motion configuration.

Rendered SVG and SVG-report JSON for the same trace must remain byte-identical. Determinism is part of the contract; tolerances are not an acceptable substitute where current types already support exact equality.

## Implementation design

### 1. Freeze replay as a test-only oracle

Before deleting `DslScenario::ball_traces_from_simulation`, move its current behavior and `scenario_event_log_from_simulation` into a `#[cfg(test)]` legacy trace constructor. It must call the same public replay resolver and produce a complete `ScenarioShotTrace`. Use it only to compare old versus one-pass structures across the matrix below. Remove all production replay call sites in the final cut; do not retain a hidden fallback.

### 2. Define a borrow-only step observer

Add crate-private types in `src/lib.rs` (names may vary, semantics may not):

```rust
pub(crate) struct NBallSystemStepBeforeResolution<'a> {
    pub elapsed_before: Seconds,
    pub event_index: usize,
    pub event: &'a NBallSystemEvent,
    pub states_before: &'a [NBallSystemState],
    pub states_at_event_before_response: &'a [NBallSystemState],
}

pub(crate) struct NBallSystemStepAfterResolution<'a> {
    pub elapsed_after: Seconds,
    pub event_index: usize,
    pub event: &'a NBallSystemEvent,
    pub states_after_response: &'a [NBallSystemState],
}

pub(crate) trait NBallSystemSimulationObserver {
    fn before_resolution(&mut self, step: NBallSystemStepBeforeResolution<'_>);
    fn after_resolution(&mut self, step: NBallSystemStepAfterResolution<'_>);
}

struct NoopNBallSystemSimulationObserver;
```

`states_before` means the validated/recovered state at the beginning of the selected event interval. `states_at_event_before_response` means that state advanced by exactly `event.time()` before collision/rail/jaw/pocket/transition response. `states_after_response` means the successfully resolved and post-validated/recovered state that becomes the next scheduler input. For a zero-time event the first two slices can be value-equal, but they remain semantically distinct; a collision response can still change the third.

All slices and the event are immutable borrows valid only for the callback. The observer must not retain them. The DSL collector owns only cloned output fields and references to immutable scenario metadata/config for the duration of simulation. Its finished value contains no borrow from the simulation's working state.

Use a generic observer parameter, not `Box<dyn ...>`, so the ordinary no-op path monomorphizes without virtual dispatch or allocation. Do not add `Send`, `Sync`, or `'static` bounds: simulation and callbacks are synchronous and stack-scoped on the calling thread.

### 3. Expose the resolver's authoritative pre-resolution state without duplicating it

Refactor `resolve_n_ball_system_event_with_physics_and_pockets_on_table` into a public wrapper over a crate-private generic helper. The helper must keep current operations and order:

The private helper also accepts `elapsed_before` and `event_index` from the scheduler solely to populate the callback view; they are not recomputed inside the resolver. The public resolver passes a no-op observer, so any placeholder metadata used by that wrapper is unobservable and must optimize away.

1. validate/recover input states;
2. call `advance_n_ball_system_without_event` exactly once;
3. invoke `before_resolution` with immutable borrows of the validated interval-start vector and the just-advanced pre-response vector;
4. execute the existing event `match` against that advanced vector;
5. run the existing post-resolution validation/recovery; and
6. return the resulting vector.

The public resolver supplies a no-op callback and remains exactly equivalent at its API boundary. Do not create a second pre-response vector merely to make pre- and post-response values available simultaneously. The collector clones required timeline endpoints during `before_resolution`; after the helper has returned successfully, the scheduler updates elapsed time and stores the event, then calls `after_resolution` with the returned post-response state. The pre-response borrow is over before the post-response callback, which avoids redundant history and makes the lifetimes explicit.

If validation or resolution fails after the before callback, the overall trace construction returns the same error and drops the partial collector. No `after_resolution` callback fires and no partial trace may escape.

### 4. Share one scheduler driver

Introduce a crate-private generic driver corresponding exactly to `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`, for example:

```rust
pub(crate) fn simulate_n_ball_system_with_physics_and_pockets_on_table_observed<O>(
    states: &[NBallSystemState],
    /* existing physics arguments */
    max_events: Option<usize>,
    observer: &mut O,
) -> Result<NBallSystemSimulation, NBallGeometryError>
where
    O: NBallSystemSimulationObserver;
```

The existing public event-limit function delegates with `NoopNBallSystemSimulationObserver`; the public until-rest function continues to delegate with `None`. The DSL trace entry points call the observed driver with their collector and `None`/`Some(max_events)`.

The loop's statements controlling behavior must remain in their current relative order: event-limit check, cached event selection, nonnegative-time assertion, `states_before` clone for no-progress detection, resolution, elapsed update, event clone/push, after callback, terminal check, cache rebuild, and zero-time/no-progress accounting. The callback observes decisions; it must not choose events or decide whether to stop. In particular:

- `after_resolution` occurs only after successful resolution and event storage, and sees the same state that becomes the next loop input;
- terminal events are stored and receive `after_resolution` before the loop breaks;
- cache rebuild and zero-time checks remain unmodified and outside observer code;
- an observer panic is not caught or converted into a physics error.

### 5. Build trace output incrementally in `src/dsl.rs`

Add an owning `ScenarioTraceCapture<'scenario>` initialized from `DslScenario::game_state.balls()` and the already validated initial system states returned by `initial_shot_system_states_on_table`. It holds:

- borrowed ball metadata only for formatting/type lookup during the call;
- `Vec<ScenarioBallTrace>` preallocated to ball count, each initialized with the same fields as current replay;
- an initially empty `Vec<ScenarioShotTraceEvent>`; do not preallocate directly from caller-controlled `max_events`, because a huge cap must not cause a new eager allocation failure;
- pending event formatting/marker metadata scoped to one callback, not a second event history;
- no cloned full-system state vector.

In `before_resolution`:

1. compute `event_time = elapsed_before + event.time()` with the current `Seconds::new` expression;
2. call `scenario_event_kind_from_system_event` once, format its human text once for marker titles, and append the same kind/time to `event_log`;
3. assert/debug-assert state and trace cardinalities before indexing;
4. zip traces, `states_before`, and `states_at_event_before_response` in original ball order;
5. skip a ball whose interval-start state is `Pocketed`;
6. append exactly one `ScenarioBallTimelineSegment` with current start/end clones and the event duration;
7. for an on-table start and on-table event endpoint, call existing `scenario_event_involves_ball` and `push_visible_trace_segment` with the same event index, label, title, visibility predicate, and field values;
8. preserve airborne/on-table/pocketed branch behavior exactly—do not synthesize visible 2-D segments for airborne intervals.

In `after_resolution`, consume only borrowed `states_after_response`. Record/validate the chain invariant that the next callback's `states_before` is the previous `states_after_response`; do not clone every full post-event vector into a new history structure. For the last/terminal step, the returned simulation's final states remain authoritative. At `finish(simulation)`, clone each final simulation state into the corresponding `ScenarioBallTrace::final_state`, then move out event log and traces.

This is “pre/post history” in the event pipeline: timeline ends are pre-response states at the event instant; post-response states become the next timeline starts. It intentionally does not expand the public result with redundant per-event post-state snapshots.

### 6. Replace both DSL trace paths and cut over

Create one private `DslScenario` helper that accepts `Option<usize>` and performs initial-state creation, observed simulation, collector finish, and `ScenarioShotTrace` assembly. Have both public trace functions delegate to it. This removes the current duplicated until-rest/event-limit assembly while leaving public behavior unchanged.

After the structural oracle is green:

- delete production `ball_traces_from_simulation` and its replay resolver call;
- delete production `scenario_event_log_from_simulation` if no caller remains;
- retain existing public resolver and simulation wrappers with no-op observation;
- leave no compatibility alias, optional replay flag, or fallback branch.

## Correctness and equivalence tests

### Full structural oracle matrix

For each fixture, run the saved legacy two-pass constructor and the one-pass constructor from identical freshly built inputs and `assert_eq!` the complete `Result<Option<ScenarioShotTrace>, DslBuildError>`:

1. no shot (`None`);
2. single moving ball until rest;
3. existing two-ball named-physics trace;
4. `THREE_BALL_PINBALL_DSL` with event limits 0, 1, 4, and 8;
5. `include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards")` with limits 1, 8, and 32;
6. pocket capture, including a jaw impact that immediately captures;
7. airborne interval and `BallTableBounce`;
8. zero-time rail impact/rebound;
9. disjoint simultaneous collisions;
10. shared three-ball contact;
11. unsupported airborne ball-ball terminal diagnostic;
12. overlapping/invalid initial geometry and unsupported nonideal shared-contact errors.

Exact equality must cover all nested events, times, states, timeline/visible segments, event labels/titles, configs, and errors. Add explicit chain assertions for every event: each recorded timeline start equals that ball's authoritative prior post-event state; each timeline end equals the resolver's pre-resolution state.

### Scheduler noninterference (`tests/n_ball_pockets.rs` and focused system tests)

- Extend `cached_pocket_aware_until_rest_simulation_matches_manual_event_stepping` and `cached_pocket_aware_shared_contact_matches_manual_event_stepping` with an observed test collector and require observed, no-op, and manual results to be exactly equal (use existing elapsed tolerance only where the existing test already does).
- Count callbacks: before and after counts equal `simulation.events.len()` on success; terminal diagnostic receives both callbacks; `max_events = 0` receives none.
- Assert event index, `elapsed_before`, `elapsed_after`, and `event.time()` chain exactly.
- Assert the ordinary public simulation function does not allocate/store history and returns the same result as the generic no-op driver.
- Add a resolver-error case proving partial collector state is not returned.

### Consumer equivalence

- Keep all existing `tests/dsl.rs` trace/render/playback tests green, especially event-limit, pocket capture, marker label/title, final layout, and timeline behavior.
- Compare `event_lines`, rendered final SVG, `playback_frames` at 20/5/2.5 ms, and SVG-report JSON byte-for-byte between old and new traces for the small, airborne, pocket, three-ball, and rack fixtures.
- Retain at least one complete expected `ScenarioShotTrace` fixture after deleting the legacy oracle so a future change cannot make two implementations wrong in the same way.

## Benchmark plan

### Committed pair and future prepared inputs

`benches/physics.rs` already contains the matched continuity pair:

- `end_to_end/dsl/preparsed_three_ball_pinball_system_only_event_limit_8`;
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`.

Preserve those exact names and their current timed boundaries. The system-only helper currently invokes `DslScenario::initial_shot_system_states_on_table` and then `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`; there is no public DSL event-limit system wrapper. Do not invent one for benchmarking.

Future cases should parse scenario DSL and construct immutable physics/config/profile inputs before `b.iter`. For event-limited system-only cases, mirror the committed helper's initial-state construction inside the timed closure so it matches the public trace path. Embed file fixtures with `include_str!`; no filesystem I/O or parsing belongs in timed loops.

Extend with this event/ball matrix and record actual event counts:

| Fixture | Balls | Event limits | Role |
|---|---:|---:|---|
| single-ball shot | 1 | until rest | adjacent/simple guardrail |
| existing `THREE_BALL_PINBALL_DSL` | 3 | 1, 4, 8 | primary event scaling |
| nine-ball break source | 10 | 1, 8, 32 | ball/event-dense scaling |
| pocket + airborne focused fixtures | 2–3 | through terminal event | branch coverage |

A maximum is not throughput metadata. Run an untimed setup invocation, assert the fixture reaches the intended flavor, and report actual balls, events, live ball-event intervals, timeline segments, visible segments, and retained trace bytes/counts.

### Criterion groups

Keep the exact committed pair above. Add future matched cases named:

- `end_to_end/trace_capture/simulation_only/{fixture}/limit_{n}`;
- `end_to_end/trace_capture/simulation_plus_trace/{fixture}/limit_{n}`.

Each iteration must `black_box` the complete simulation or trace. Set `Throughput::Elements` to actual live ball-event intervals for trace cases and also report ns/event and ns/ball-event interval. Compare both total trace time and incremental trace overhead (`simulation_plus_trace - simulation_only`) from paired process medians; never subtract individual noisy Criterion samples as though they were paired observations.

The trace-capture algorithm itself has no playback sample-step input. To cover downstream step scaling without misattributing it, add secondary prepared integration cases in `benches/throughput.rs`:

- `trace_capture_plus_owned_playback/{three_ball_8,nine_ball_32}/{20ms,5ms,2_5ms}`.

These include trace construction followed by complete `playback_frames`, black-box the full output, and are acceptance guardrails only. The primary capture claim comes from the physics matched pairs; the sample-step axis belongs to the downstream consumer.

Criterion settings:

- single-ball and event-limit-1 cases: 2 s warm-up, 10 s measurement, 30 samples;
- three-ball limits 4/8: 2 s warm-up, 15 s measurement, 10 samples because the unchanged 8-event case is about 0.5 s/call;
- nine-ball limits 8/32: 2 s warm-up, 30 s measurement, 10 samples;
- downstream prepared playback: 2 s warm-up, 15 s measurement, 30 samples where per-sample runtime permits, otherwise the same explicit 10-sample expensive-case rule.

Use identical settings for baseline and candidate. Save raw Criterion estimates, bootstrap 95% confidence intervals, outliers, toolchain, and fixture metadata.

## Allocation and peak-memory plan

Use a dedicated single-thread allocation harness with a transparent counting `System` allocator. Enable counting only after all fixtures are built; track allocation/reallocation count, total requested bytes, current live bytes, and peak live bytes. Run exactly one complete operation per measurement with `--test-threads=1` and keep the returned simulation/trace alive until the endpoint. Do not use allocator-instrumented wall time as performance evidence.

Measure simulation-only and simulation-plus-trace for the three-ball/event-cap-8 and ten-ball/event-cap-32 cases on old and new implementations. Record actual stored events and report both totals and incremental trace overhead relative to the matched simulation-only operation.

Required gates:

- ordinary simulation-only allocation count, total bytes, and peak live bytes must be exactly unchanged where deterministic; no result may exceed baseline by more than **2%**;
- simulation-plus-trace must remove at least **one full-system `Vec<NBallSystemState>` allocation per actually replayed event** (candidate allocation count `<= baseline - actual_event_count`) on both capped fixtures;
- total allocated bytes for simulation-plus-trace must fall by at least **5%** on the ten-ball/event-cap-32 fixture, with its actual event count reported;
- peak live bytes for simulation-plus-trace must not regress by more than **2%**. Because the structurally identical retained trace dominates final live memory, a large peak reduction is not required;
- retained output cardinality and serialized/debug size must be identical, proving the memory win is not omitted history.

Corroborate with Instruments Allocations on macOS (or the same profiler for both revisions). Allocation stacks from the second `resolve_n_ball_system_event_with_physics_and_pockets_on_table` traversal under `ball_traces_from_simulation` must disappear; authoritative resolver allocations in the first pass are expected to remain.

## Statistical acceptance gates

Use the same host, target, release profile, Rust toolchain, power mode, background-load policy, inputs, and Criterion settings. Alternate saved baseline and candidate in **A/B/A/B order for at least three independent process pairs**.

For incremental-overhead inference, run each matched system-only/trace pair in the same benchmark process and treat the process as the sampling unit. Compute one `trace - system` median delta per process, pair baseline/candidate processes by run order, and bootstrap the paired delta ratios/differences with a saved seed. Criterion's per-filter interval remains the total-time evidence; never manufacture an interval by subtracting two independent Criterion confidence bounds.

Accept only if correctness is complete and:

1. On the ten-ball/32-event-cap `simulation_plus_trace` case, the median wall-time improvement is at least **5%** and Criterion's 95% relative-change interval excludes zero. Report the actual event count.
2. On the committed three-ball/8-event-cap pair, incremental trace overhead relative to the matched simulation-only median falls by at least **25%**, its paired bootstrap interval excludes no improvement, and total trace time has no regression whose upper 95% confidence bound exceeds **+2%**. Do not require an impossible 5% total win from a quick baseline whose observed gap is only about 1%.
3. The ten-ball/32-event-cap incremental trace overhead also falls by at least **25%**. Report absolute simulation and trace medians and derived deltas for both primary fixtures; never treat subtraction of unpaired Criterion samples as a confidence interval.
4. The 1/4/8 and 1/8/32 event series show lower or flat normalized ns/ball-event interval rather than a win confined to one cap.
5. Ordinary simulation-only cases have no regression whose upper 95% confidence bound exceeds **+2%**. This is a hard scheduler-path guardrail.
6. Existing two-ball/simple trace and downstream 20/5/2.5 ms integration cases have no regression above **2%**; downstream outputs are structurally identical.
7. All allocation/peak gates pass.

Do not accept a change that merely moves formatting out of the benchmark, omits events/segments, changes event limits, or relies on one quick run. If pocket prediction noise prevents a stable end-to-end conclusion, increase paired process runs and retain the same predeclared thresholds; do not cherry-pick samples.

## Risks and mitigations

- **Observer changes scheduler behavior:** observer receives immutable borrows and no control-flow return; scheduler statements and ordering remain fixed and tested against no-op/manual stepping.
- **Pre-response versus post-response confusion:** use two explicitly named callbacks and fields. Timeline `end` comes from `states_at_event_before_response`; the next timeline `start` comes from prior `states_after_response`.
- **Duplicated advancement during refactor:** the resolver helper calls `advance_n_ball_system_without_event` exactly once; allocation profiling and structural chain tests enforce this.
- **Error after before-callback:** partial collector is dropped and never returned.
- **Terminal event omission:** callback happens after event resolution/push and before terminal break; callback count must equal stored events.
- **Zero-time event semantics:** preserve the existing `states_before` clone and no-progress comparison; zero-duration timeline segments remain present exactly as before.
- **Event formatting drift:** construct kind/time once using existing helpers and reuse its human text for marker title; exact trace equality protects punctuation and numbering.
- **Borrow/lifetime complexity:** callbacks are synchronous, borrow-only, and generic. Collector outputs are owned; no unsafe code, self-referential storage, `'static` bound, or leaked reference is allowed.
- **Ordinary simulation code-size regression from monomorphization:** only no-op and trace collector instantiations are expected. Inspect release symbols/LLVM lines if compile size materially changes; do not replace with boxed dispatch without measurement.
- **Overlap with cache plans:** observer must surround event resolution only and remain ignorant of how the next event/cache is computed, making rebases mechanical.

## Stop conditions, rejection, and rollback

Reject or revise the implementation if any complete trace differs, ordinary simulation events/states/errors change, the no-op path regresses beyond 2%, the ten-ball/32-event-cap trace fails the 5% total-time gate, either primary fixture fails the incremental-overhead gate, or allocation evidence still shows a second resolver traversal. Do not relax deterministic equality or scheduler rules to obtain a win.

If the dense end-to-end trace cannot clear 5% because event prediction dominates, do not land the refactor solely on theoretical complexity unless the project explicitly accepts it for architecture; the accepted performance implementation requires the stated practical win. If only formatting consolidation wins, treat that as insufficient for this candidate. The three-ball continuity pair is a no-regression and incremental-overhead check, not a 5% total-time stop condition.

Rollback is a direct reversion of the observer/collector commit. There is no persistent-state or serialized-data migration. During implementation, keep the change in one coherent commit so the public functions always delegate to a complete driver and never expose partial capture.

## SIMD and GPU decision

- **arm64 SIMD/NEON:** Rejected. The waste is a duplicated, branch-heavy event/state traversal with heterogeneous enum variants and collision responses. SIMD cannot eliminate the second pass and would complicate exact event semantics.
- **GPU:** Rejected. Event resolution is sequential, latency-sensitive, irregular, and tightly coupled to CPU scheduler decisions; trace capture mostly clones owned states and formats metadata. Transfer and synchronization would dominate and jeopardize determinism.

Reconsider neither unless a post-cutover profile identifies a separate large homogeneous numeric kernel; that would be a different plan.

## Candidate commit message

`perf(dsl): capture traces during system simulation`
