# Performance / benchmarking notes

This repo now has two `criterion` benchmark suites:

- `benches/physics.rs` — targeted function benchmarks plus a few end-to-end flows
- `benches/throughput.rs` — batch/throughput workloads intended to mirror future search / Monte Carlo / GPU-style use cases

## What each suite covers

## `physics`

This suite is for **point measurements** and **holistic single-run flows**.

It includes:

- direct vs DSL setup
  - direct `GameState` construction
  - `parse_dsl_to_game_state(...)`
  - direct shot-input construction
  - `parse_dsl_to_scenario(...)`
- specific functions
  - `compute_next_transition_on_table(...)`
  - `compute_next_ball_ball_collision_during_current_phases_on_table(...)`
  - `compute_next_ball_rail_impact_on_table(...)`
  - `compute_next_two_ball_event_with_rails_on_table(...)`
  - `collide_ball_ball_detailed_on_table(...)`
  - `trace_ball_path_with_rails_on_table(...)`
- end-to-end flows
  - direct strike + trace until rest
  - DSL parse + trace until rest
  - pre-parsed DSL trace until rest
  - direct strike + two-ball simulate to completion
  - DSL parse + strike + two-ball simulate to completion

Use this suite when asking questions like:

- “Did this particular function get faster or slower?”
- “Is DSL overhead actually material?”
- “What is the current single-shot latency?”

## `throughput`

This suite is for **many independent operations**.

It includes batched workloads for:

- DSL parsing throughput
  - `parse_dsl_to_game_state(...)`
  - `parse_dsl_to_scenario(...)`
- function throughput
  - `compute_next_transition_on_table(...)`
  - `compute_next_ball_ball_collision_during_current_phases_on_table(...)`
- end-to-end throughput
  - tracing many seeded single-ball shots until rest
  - simulating many two-ball shots to completion

Use this suite when asking questions like:

- “How does performance scale with batch size?”
- “What might planner/search throughput look like?”
- “Would a CPU-parallel or GPU backend have enough work to amortize overhead?”

---

## Verified commands

### Fast smoke checks

```bash
just perf
```

Equivalent explicit commands:

```bash
cargo bench --bench physics -- --quick
cargo bench --bench throughput -- --quick
```

These are the fastest useful runs.

### Compile-only bench sanity check

```bash
just perf-build
```

Equivalent explicit commands:

```bash
cargo bench --bench physics --no-run
cargo bench --bench throughput --no-run
```

Useful after refactors when you just want to ensure the benchmark binaries still build.

### Full benchmark runs

```bash
just perf-full
```

Equivalent explicit commands:

```bash
cargo bench --bench physics
cargo bench --bench throughput
```

These take longer but produce more stable numbers.

### Format before benchmarking

```bash
cargo fmt --all
```

---

## Where results go

Criterion writes reports under:

```text
target/criterion/
```

That directory contains per-benchmark reports and historical comparison data for repeated local runs.

To open the top-level Criterion HTML report after a run:

```bash
just perf-open
```

This tries `open` first, then `xdg-open`, and otherwise prints the report path.

---

## Recommended workflow

## 1. Start with a smoke run

```bash
cargo bench --bench physics -- --quick
cargo bench --bench throughput -- --quick
```

This is enough to catch obvious regressions and identify which broad category got slower.

## 2. If something moved, rerun the relevant suite fully

```bash
cargo bench --bench physics
# or
cargo bench --bench throughput
```

## 3. Interpret by benchmark class

### If setup / DSL numbers changed

Likely causes:

- parser changes
- DSL builder changes
- extra validation work
- more allocations during build/setup

### If `compute_next_transition_on_table(...)` changed

Likely causes:

- motion-phase classification changes
- numeric conversion churn
- extra branching or allocation in the motion path

### If collision / rail prediction changed

Likely causes:

- more scanning/refinement work
- more expensive within-phase advancement
- additional conversions in hot loops

### If end-to-end tracing or simulation changed

Likely causes:

- any of the above
- more event scheduling work
- more rail events / motion transitions encountered
- more cloning or intermediate object creation

---

## Current qualitative takeaways

From the initial smoke runs:

- **DSL overhead is currently small** relative to the actual physics work in the covered end-to-end cases.
- The heavier hotspots are in the simulation core, especially:
  - rail-impact prediction,
  - path tracing,
  - full two-ball simulation-to-completion flows.
- This supports the current roadmap direction:
  - optimize hot simulation math first,
  - then add batch APIs,
  - then revisit CPU-parallel and GPU paths.

In other words: the likely first wins are in the **physics engine core**, not in shaving parser overhead.

## Current smoke-run baseline

These numbers are machine-dependent and should be treated as a rough local baseline, not a portable truth.

### From `cargo bench --bench physics -- --quick`

Very roughly:

- direct `GameState` build: ~`5.5 µs`
- `parse_dsl_to_game_state(...)`: ~`6.1 µs`
- `compute_next_transition_on_table(...)`: ~`6.0 µs`
- `compute_next_ball_ball_collision_during_current_phases_on_table(...)`: ~`0.42 ms`
- `compute_next_ball_rail_impact_on_table(...)`: ~`6.6 ms`
- `trace_ball_path_with_rails_on_table(...)` for the covered bank case: ~`24.5 ms`
- direct strike + trace-until-rest: ~`9.3 ms`
- direct two-ball simulate-to-completion: ~`28.5 ms`

### From `cargo bench --bench throughput -- --quick`

Very roughly:

- DSL parse throughput: ~`140k` docs/sec
- `compute_next_transition_on_table(...)`: ~`148k` states/sec
- `compute_next_ball_ball_collision_during_current_phases_on_table(...)`: ~`1.8k–1.9k` pairs/sec
- end-to-end single-ball trace-until-rest: ~`27–30` shots/sec
- end-to-end two-ball simulate-to-completion: ~`27–30` shot pairs/sec

Those batch results are especially useful for the GPU plan because they show where the current throughput ceiling appears to be.

---

## How to use these benchmarks with the GPU plan

The GPU/thruput roadmap in `GPU_PHYSICS_PLAN.md` assumes that GPU is most likely to help with **many-shot throughput**, not single-shot authoritative simulation.

Use the suites like this:

- `physics` answers: “What is the latency and hotspot profile of the current reference engine?”
- `throughput` answers: “How much parallel work exists across independent shots / calls?”

That distinction is important:

- poor single-shot latency does **not** automatically imply GPU is the answer;
- strong scaling potential on batched workloads is the better signal for GPU payoff.

---

## Practical advice when changing hot code

When editing performance-sensitive paths:

1. benchmark before changing anything,
2. change one thing at a time where possible,
3. rerun `physics -- --quick`,
4. rerun `throughput -- --quick`,
5. only trust a claimed win if both correctness and benchmark behavior make sense.

If a change improves a microbenchmark but hurts end-to-end path tracing or batched throughput, it is probably not a net win.

---

## Next likely perf investigation targets

Based on the current code and benchmark coverage, the most promising areas to investigate next are:

1. hot-path numeric conversions around `BigDecimal` / `f64`
2. repeated work inside rail-impact prediction
3. repeated work inside event-driven path tracing
4. batched evaluation APIs for many independent shots
5. CPU parallelism before any GPU backend
