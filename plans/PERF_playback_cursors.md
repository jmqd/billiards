# Monotonic Playback Schedule and Per-Ball Cursors

## Status and decision

- **Status:** Accepted implementation plan; not yet implemented.
- **Priority:** High for trace playback and SVG-report generation.
- **Confidence:** High that the current algorithm performs avoidable sorting, repeated timeline scans, and peak-live allocation; medium-high on the end-to-end report win because physics and SVG rendering remain substantial costs.
- **Order/dependencies:** Independent of `PERF_single_pass_trace_capture.md`. Implement this after benchmark fixtures can construct prepared `ScenarioShotTrace` values outside timed loops. It may land before or after single-pass capture; neither plan's API is a prerequisite for the other.
- **Compatibility decision:** Preserve `ScenarioShotTrace::playback_frames(Seconds) -> Vec<ScenarioPlaybackFrame>` and all frame semantics. Add a borrowing iterator as an optional streaming API; do not add a deprecated alias or compatibility shim.

## Problem and evidence

### Observed facts

`src/dsl.rs::ScenarioShotTrace::playback_frames` currently:

1. allocates a `Vec<f64>` containing `0`, simulation elapsed time, every event time, and every subdivided time from every ball timeline segment;
2. sorts that global vector with `f64::total_cmp` and epsilon-deduplicates it;
3. visits every surviving time and every ball; and
4. calls `ScenarioBallTrace::state_at_elapsed`, which scans `timeline_segments` from index zero for every query.

`ScenarioBallTrace::state_at_elapsed` is a correct arbitrary-time API, but its repeated from-zero search is inappropriate for playback's monotonically increasing times. For $B$ balls, $F$ emitted frames, and $E$ timeline segments per ball, state lookup can perform $O(BFE)$ segment examinations. Schedule construction first materializes roughly $BES$ time candidates, where $S$ is subdivisions per segment, then sorts them. Ball timelines produced by system simulation normally contain the same event intervals, so most candidates are duplicates.

`src/svg_generator.rs::push_playback_json` calls `trace.playback_frames`, retains the complete `Vec<ScenarioPlaybackFrame>` and every per-frame ball vector/state while serializing it, and then drops the collection. The report path therefore needs only sequential consumption but pays the peak memory of the owned API.

Current committed benchmark coverage and quick, non-acceptance snapshots on the unchanged tree:

- `playback_scaling/two_ball/2_5ms` is about **34.35 ms/call**.
- `playback_scaling/three_ball_event_limit_8/2_5ms` is about **20.49 ms/call**.
- The same prepared two- and three-ball traces are already covered at `20ms`, `5ms`, and `2_5ms`; `throughput_rendering/trace_playback_frames_2_5ms` remains the older two-ball continuity case.
- These are single quick runs, not paired statistical evidence. The committed matrix still lacks a ten-ball/event-dense trace and owned-versus-streaming/report cases.
- `src/dsl.rs` already defines `SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS = 1e-9`; current tests require exact event-time inclusion, gaps no greater than the requested step plus epsilon, interpolation between events, and pocketed-ball disappearance.

### Hypothesis to verify

A lazy, monotonically merged time schedule removes the candidate-time peak and global sort; one forward segment cursor per ball changes timeline search to amortized $O(B(F+E))$. Having SVG JSON consume the iterator directly should reduce report peak-live memory from retained playback state proportional to $F B$ to one frame proportional to $B$. The magnitude of wall-time improvement is a hypothesis and must pass the gates below.

## Scope

### In scope

- `src/dsl.rs`: playback schedule, per-ball monotonic cursor, public borrowing iterator, and the existing owned wrapper.
- `src/svg_generator.rs`: sequential consumption of prepared playback frames.
- `benches/throughput.rs`: prepared-trace ball/event/sample-step scaling and owned-versus-streaming coverage.
- A focused allocation harness for allocation count, total allocated bytes, and peak live bytes.
- `tests/dsl.rs`, `tests/scenario_examples.rs`, and `tests/svg_generator.rs`: schedule, state, pocket, airborne, iterator, and JSON equivalence.

### Non-goals

- No parser cleanup.
- No physics, collision, pocket, event scheduler, cache, or timeline-generation optimization.
- No change to state integration, event ordering, epsilon values, float formatting, JSON schema, ball ordering, or pocket disappearance.
- Do not replace `ScenarioBallTrace::state_at_elapsed`; it remains the arbitrary/random-access API.
- Do not combine this with single-pass trace capture, rendering asset caches, SVG write changes, SIMD, GPU work, or a new JSON library.

## Observable contract

For every existing `ScenarioShotTrace` on which the current implementation terminates and every positive finite `max_time_step`, the owned result must be structurally identical to the current result:

- the same frame count and order;
- bit-identical `Seconds::as_f64()` values in each frame;
- the same `ScenarioPlaybackBall` order as `ball_traces`;
- exact `BallType` and `BallState` equality for every emitted ball;
- inclusion of `t = 0`, finite nonnegative event times, finite nonnegative segment starts/subdivision endpoints, and finite nonnegative simulation elapsed time under the current sort/dedup rule;
- the same smallest sorted representative retained when the next candidate differs from the **last retained** value by at most `1e-9`;
- the same interpolation and endpoint snapping behavior;
- a pocketed ball present through its capture endpoint under current epsilon semantics and absent afterward;
- the same panic and message for a non-positive or non-finite step;
- byte-identical SVG-report JSON, including duration, six/three-decimal formatting, event duplication required by the schema, escaping, and array ordering.

The new iterator may be partially consumed or dropped. Each yielded `ScenarioPlaybackFrame` is fully owned and remains valid after the iterator advances or is dropped.

## Implementation design

### 1. Freeze the old algorithm as a test oracle

Before replacing production code, copy the current `playback_frames` body into a `#[cfg(test)]` helper named, for example, `legacy_playback_frames`. Keep it test-only and delete it after the equivalence matrix is green and a fixed expected fixture is retained. It must not ship as a second production convention.

Use exact `assert_eq!` between legacy and candidate vectors. Do not weaken equality to coordinate tolerances: both implementations use the same advancement function and formulas, and complete structural equivalence is the acceptance contract.

### 2. Add monotonic time sources

Introduce private structures in `src/dsl.rs`:

```rust
struct PlaybackSegmentTimes<'a> {
    segments: &'a [ScenarioBallTimelineSegment],
    segment_index: usize,
    sample_index: usize,
    sample_count: usize,
    max_time_step: f64,
}

struct PlaybackTimeHead {
    time: f64,
    source_index: usize,
}

struct PlaybackTimeSchedule<'a> {
    sources: Vec<PlaybackSegmentTimes<'a>>,
    heap: BinaryHeap<Reverse<PlaybackTimeHead>>,
    event_index: usize,
    events: &'a [ScenarioShotTraceEvent],
    endpoint_index: usize,
    endpoints: [f64; 2],
    last_emitted: Option<f64>,
}
```

The exact concrete representation may combine event/endpoints into the same source abstraction, but it must retain these properties:

- one ordered source per ball timeline plus ordered event and endpoint sources on the normal simulation-generated path;
- a custom `Ord`/`Eq` implementation using `f64::total_cmp`, then stable source index as a tie-breaker; do not use `partial_cmp().unwrap()` or derive floating-point ordering;
- no global candidate-time vector on the normal simulation-generated path;
- each segment source computes the same values as today: `sample_count = max(ceil(duration / step) as usize, 1)` and `start + duration * sample_index / sample_count` for `1..=sample_count`, plus the segment start;
- invalid candidates are skipped using the current `is_finite() && time >= 0.0` predicate;
- the merge emits candidates in total order and retains a candidate only when its absolute difference from the **last retained** value exceeds `SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS`. This reproduces `sort_by(total_cmp)` followed by `dedup_by`; do not deduplicate transitively against every popped member of an epsilon cluster.

Advance a source immediately after popping its head and push only its next valid head. The schedule owns only $O(B)$ heads for ordinary traces. Equal simulation-generated timelines still produce duplicate heads, but they are never retained or globally sorted; an exact identical-source coalescing optimization may be considered only after this version is correct and measured.

All trace/event/timeline fields are public, and the current global sort also defines behavior for manually constructed traces whose event log or segment sources are not monotonic. Validate source monotonicity before selecting the heap merge. If any source would decrease under `f64::total_cmp`, use a private exact materialize/sort/filter/dedup schedule fallback for that trace; the forward ball-state cursor may still consume the resulting monotonic times. This is a semantic fallback, not a second public API. Add an unsorted manual event/timeline oracle case so the optimization does not silently narrow the public data model. Do not sort or mutate the public trace in place.

Manual construction of public trace structures is supported today. Therefore, do **not** derive the schedule solely from `simulation.events`: current tests include timeline samples that are absent from `event_log`. Merge all current sources exactly.

### 3. Add a forward-only ball cursor

Introduce:

```rust
struct PlaybackBallCursor<'a> {
    trace: &'a ScenarioBallTrace,
    segment_index: usize,
}
```

Its `state_at_monotonic(time, ball_set, motion)` must implement the current `state_at_elapsed` branch order and epsilon checks exactly, but it may advance `segment_index` only after the monotonically increasing target is greater than that segment's end plus epsilon. Important details:

- never rewind; only the private playback iterator may call it with schedule output;
- preserve the `target_time.max(0.0)` behavior;
- for a gap before the current segment, return that segment's start without advancing past it;
- at a start/end epsilon boundary, clone the stored endpoint rather than reintegrating;
- inside a segment, call the existing `advance_timeline_ball_state` once;
- after the final segment, use the existing final-state match, including `Pocketed` capture cutoff based on the last declared segment end;
- handle empty timelines, zero-duration segments, overlapping or nonmonotonic declared ranges, back-to-back times within epsilon, airborne states, and manually constructed traces exactly as the old method.

Do not change the public random-access `state_at_elapsed`; keeping it separate prevents a stateful cursor from silently changing arbitrary query behavior.

### 4. Expose a lazy owned-frame iterator

Add:

```rust
pub struct ScenarioPlaybackFrames<'a> {
    trace: &'a ScenarioShotTrace,
    times: PlaybackTimeSchedule<'a>,
    balls: Vec<PlaybackBallCursor<'a>>,
}

impl<'a> Iterator for ScenarioPlaybackFrames<'a> {
    type Item = ScenarioPlaybackFrame;
}

impl ScenarioShotTrace {
    pub fn playback_frames_iter(
        &self,
        max_time_step: Seconds,
    ) -> ScenarioPlaybackFrames<'_>;

    pub fn playback_frames(
        &self,
        max_time_step: Seconds,
    ) -> Vec<ScenarioPlaybackFrame> {
        self.playback_frames_iter(max_time_step).collect()
    }
}
```

Fields stay private. The iterator borrows the trace for `'a`; it owns its schedule/cursor vectors. Each `next()` allocates only that frame's `Vec<ScenarioPlaybackBall>`, obtains states from forward cursors in trace order, and returns a completely owned frame. This makes partial consumption safe and preserves the existing owned API as a thin collector, without a shim or duplicate algorithm.

Validate the step once in `playback_frames_iter` with the current assertion and message. If exact remaining length cannot be known cheaply, do not claim `ExactSizeIterator` or implement a misleading `size_hint`.

### 5. Stream SVG-report serialization

Change only `src/svg_generator.rs::push_playback_json` to iterate over `trace.playback_frames_iter(max_time_step)` and serialize/drop one frame at a time. Do not retain a `Vec`.

The JSON header needs duration before frames. It must remain `legacy_frames.last().map(...).unwrap_or(0.0).max(simulation.elapsed)`. Do **not** replace that with the maximum raw endpoint/event candidate: epsilon dedup keeps the smaller representative, so the raw maximum can differ from the last emitted frame. Compute the last retained schedule time by exhausting a time-only schedule pass (including the exact fallback when required), then create the frame iterator for serialization. This repeats schedule traversal but not state integration or frame retention; benchmark it. Keep all existing formatting calls and comma behavior unchanged.
Do not bundle event-string caching unless allocation profiling proves it materially affects this path; the accepted mechanism is schedule/cursor complexity and streaming frame lifetime.

### 6. Clean cutover

After exact equivalence passes:

- remove the old production schedule/map implementation;
- retain no `legacy_*` production function, alternate iterator, deprecated alias, or feature flag;
- keep the test oracle only while developing, then replace it with checked structural fixture expectations and iterator-versus-owned assertions;
- update all in-tree immediate consumers that do not require retention (`push_playback_json` and benchmarks) to use the iterator; leave callers that require random frame access on `playback_frames`.

## Correctness test plan

### Schedule and cursor unit tests (`tests/dsl.rs` or module-private tests)

1. Preserve `playback_frames_snap_to_logged_event_times_and_sample_between_them` and require exact legacy/new equality before retiring the oracle.
2. Preserve `playback_frames_omit_pocketed_balls_after_their_capture_time`; additionally assert the ball exists exactly at capture and is absent at capture plus more than epsilon.
3. Construct divergent manual timelines for two balls so segment starts/durations are not shared. Include `0`, simulation elapsed, event times, positive segment starts, segment ends, and interior samples; compare the entire vector with the legacy oracle.
4. Add a public-structure fixture with unsorted event times and out-of-order timeline segments; require the fallback's entire result to equal the legacy oracle.
5. Add times at `x`, `x + 0.5e-9`, `x + 1.5e-9`, and `x + 2.0e-9` to catch incorrect transitive epsilon grouping and wrong retained representative. Make the largest raw candidate fall within epsilon of the prior retained candidate and assert report duration uses the last emitted time, not that raw maximum.
6. Cover empty event/timeline lists, an empty timeline with on-table final state, a zero-duration segment, multiple zero-time event boundaries, and a segment starting after a gap.
7. Cover on-table interpolation, airborne advancement, landing/table-bounce boundaries from existing scenario fixtures, and pocket capture.
8. Assert invalid steps (`0`, negative, `NaN`, positive infinity) panic with the existing message.
9. Consume only the first two iterator items and drop it; separately collect the iterator and assert exact equality with `playback_frames`.
10. Assert frame and ball order, exact endpoint clones, and monotonic strictly-more-than-epsilon retained times.

### Integration tests

- Keep `tests/scenario_examples.rs::elevated_side_spin_examples_expose_height_and_z_spin_for_gallery_playback` and jump-obstacle coverage green.
- In `tests/svg_generator.rs`, add report cases for a small two-ball shot, a pocket capture, an elevated/airborne shot, and `include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards")` capped at 32 events. During implementation, compare old and new report strings byte-for-byte; retain parsed-JSON structural assertions plus a stable byte fixture for the small report so ordering, numeric precision, escaping, duration, and ball disappearance remain protected.
- Parse report JSON and assert every streamed frame equals the corresponding projection/kinematics from `trace.playback_frames` for the small case; this catches a serializer that skips or reorders balls even if the JSON remains valid.

## Benchmark plan

### Committed prepared fixtures and future extensions

`benches/throughput.rs` already constructs both traces before timed loops and contains:

- `playback_scaling/two_ball/{20ms,5ms,2_5ms}`;
- `playback_scaling/three_ball_event_limit_8/{20ms,5ms,2_5ms}`;
- `throughput_rendering/trace_playback_frames_2_5ms`.

Preserve those exact filters as continuity coverage. Before implementation timing, record their actual balls, events, timeline segments, frames, and emitted ball states; the `event_limit_8` suffix is a cap, not proof of eight stored events.

Future matrix extensions should prepare, outside `b.iter`, `THREE_BALL_PINBALL_DSL` at event caps 1 and 4, a ten-ball `include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards")` trace at caps 1/8/32, and a focused pocket fixture through capture. Never parse, simulate, clone the full trace, or read files in a timed playback iteration.

For every prepared trace, publish fixture metadata before timing: source name/bytes, balls, actual events, total timeline segments, retained frames, emitted ball states, and playback JSON bytes where applicable. A cap is not an event count; report the actual count.

### Criterion cases

Keep the committed `playback_scaling/...` and continuity names, then add:

- `playback_owned/{fixture}/{step}`;
- `playback_stream_fold/{fixture}/{step}`;
- `playback_svg_report/{fixture}/{step}` for the complete public report consumer.

Run the future cross-product of prepared two/three/ten-ball fixtures and sample steps **20 ms, 5 ms, and 2.5 ms**. The event-cap variants above provide the event axis. Set `Throughput::Elements` to emitted ball states, not calls. For owned cases, `black_box` the complete `Vec`. For streaming cases, consume the iterator to exhaustion and `black_box` a deterministic checksum containing frame count, ball count, times, IDs, and all state scalars; never benchmark iterator construction alone. For report cases, `black_box` the complete returned string and set `Throughput::Bytes` from one untimed expected output.

Criterion settings for these point cases: 2 s warm-up, 15 s measurement, 30 samples. Save the baseline with the exact toolchain and run filters; do not compare unrelated Criterion histories.

### Allocation and peak-memory measurements

Add a dedicated single-thread allocation test binary/harness using a transparent counting wrapper over `System`. Counting is enabled only around one operation after fixture construction; track allocations, reallocations, deallocations, total requested bytes, current live bytes, and peak live bytes. Run with `--test-threads=1`; do not use these instrumented timings as Criterion evidence.

Measure owned collect, stream-to-checksum, and complete SVG report on the 3-ball/event-cap-8 and 10-ball/event-cap-32 traces at 20 ms and 2.5 ms. Record and use each trace's actual stored event count in all reports. Keep output alive through the measurement endpoint for comparable peak semantics.

Required allocation gates, relative to the saved old implementation on identical fixtures:

- streaming iterator checksum: peak live bytes at least **50% lower** on the 10-ball/event-cap-32/2.5 ms fixture and allocation count no higher;
- complete SVG report: peak live bytes at least **20% lower** on the 10-ball/event-cap-32/2.5 ms fixture and total allocated bytes no higher;
- owned `playback_frames`: candidate-time allocations disappear, total allocated bytes improve by at least **3%** on the dense case, and peak live bytes do not regress by more than **2%**;
- no fixture may increase allocation count, total allocated bytes, or peak live bytes by more than **2%**. Exact integer metrics should normally be deterministic; the percentage permits allocator-capacity variation only.

Corroborate the dense report with Instruments Allocations on macOS (or the same allocator profiler used for baseline/candidate). The old retained `Vec<ScenarioPlaybackFrame>` and global time-vector allocation stack must disappear from `push_playback_json`.

## Statistical acceptance gates

Use the same host, release profile, Rust toolchain, target, power mode, background-load policy, fixtures, and Criterion settings. Run baseline and candidate in alternating **A/B/A/B order across at least three independent process pairs**.

Accept only if all correctness tests pass and:

1. Criterion's 95% confidence interval for relative change excludes zero and the median improvement is at least **5%** for both owned playback and streaming consumption on the 3-ball/event-cap-8 and 10-ball/event-cap-32 cases at 2.5 ms; publish actual stored event counts.
2. The complete 10-ball/event-cap-32 SVG report improves by at least **5%**, with its 95% interval excluding zero. A component-only win is insufficient for the streaming claim.
3. Normalized nanoseconds per emitted ball state flatten across the event axis relative to baseline; report medians, Criterion estimates/CIs, and fixture cardinalities.
4. The existing two-ball continuity benchmark and 20 ms cases have no regression whose upper 95% confidence bound exceeds **+2%**.
5. The allocation/peak gates above pass.

Do not claim success from a quick run, one process, overlapping confidence intervals, or lower time caused by omitted output.

## Risks and mitigations

- **Epsilon non-transitivity:** compare each sorted candidate only with the last retained candidate; oracle cases explicitly cover chains.
- **Wrong numerical representative:** total-order merge must retain the same smallest sorted candidate, not whichever source is visited first.
- **Cursor endpoint drift:** preserve the exact branch order and stored endpoint clones from `state_at_elapsed`.
- **Manual trace divergence:** merge all timeline sources; do not assume system-generated lockstep intervals.
- **Pocket resurrection:** preserve the last-segment capture cutoff and test both sides of it.
- **Airborne mismatch:** continue using `advance_timeline_ball_state`; no on-table-only shortcut.
- **Iterator lifetime/API misuse:** fields remain private and yielded frames are owned; no references to cursor scratch escape.
- **JSON drift:** require byte equivalence before considering performance.
- **Heap merge overhead on tiny traces:** continuity guardrail can reject the design or motivate a measured small-source linear merge without changing semantics.

## Stop conditions, rejection, and rollback

Reject or revise the implementation if exact structural or report-byte equality cannot be obtained, the dense owned and report benchmarks fail the 5% practical threshold, peak-memory gates fail, or tiny playback regresses beyond 2%. Do not relax epsilon/output semantics to rescue timing.

Rollback is a direct reversion of the schedule/cursor/iterator commit because no serialized data or migration is involved. The owned API remains stable throughout. If report streaming wins but heap schedule timing does not, retain only the iterator/streaming cut if it independently passes its report and memory gates; do not ship an unproven algorithmic rewrite.

## SIMD and GPU decision

- **arm64 SIMD/NEON:** Rejected. The bottleneck is schedule duplication, branchy heterogeneous state advancement, allocation lifetime, and repeated segment search. Balls may be on-table, airborne, or pocketed; vector lanes would diverge, and NEON does not remove the sort or rescans.
- **GPU:** Rejected. Frame generation has small irregular workloads, strict sequential event/epsilon ordering, owned decimal-rich state, and immediate CPU-side JSON formatting. Transfer, kernel launch, and synchronization would dominate and threaten deterministic equivalence.

Reconsider neither until the algorithmic/lifetime changes land and a new profile identifies a large homogeneous numeric kernel.

## Candidate commit message

`perf(dsl): stream playback with monotonic cursors`
