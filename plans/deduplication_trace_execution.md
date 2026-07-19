# Deduplication: traced simulation execution

## Rank and scope

5 of 12. DSL scenario execution and trace finalization.

## Problem

The rest and event-limited trace methods repeat initial-state extraction, engine dispatch, geometry-error mapping, event-log derivation, ball-trace reconstruction, physics cloning, and `ScenarioShotTrace` assembly. Preferred-physics wrappers separately repeat stop selection and the `min(requested, preset)` limit rule.

## Decision

Add a private `ScenarioTraceStop` enum with `UntilRest` and `EventLimit(usize)` plus a limit-composition method. Add one private trace executor that resolves initial states, performs only the stop-specific engine call, and finalizes `ScenarioShotTrace` once. Named and preferred public entry points resolve physics and stop policy, then delegate.

Keep public methods as thin typed entry points where they are part of the current API; do not generalize the lower-level physics engine.

## Files

- `src/dsl.rs`
- `tests/dsl.rs`

## Invariants

- Rest and event-limit simulations retain current engine entry points.
- Requested and preset limits preserve current minimum precedence.
- Geometry errors and final trace fields remain identical.
- Preferred and explicit named simulation paths remain equivalent.
- No extra event/state allocation is introduced.

## Verification

Run focused preferred-simulation, named-simulation, event-limit, trace reconstruction, and final-layout tests. Cover requested limits below, equal to, and above preset limits.

## Commit boundary

Only trace stop selection, execution dispatch, and trace finalization. Timeline sampling remains separate.