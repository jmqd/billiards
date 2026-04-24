# Coordinator summary: physics audit 2026-04-24

Sub-agent reports created in this directory:

- `2026-04-24-cue-cloth-motion.md`
- `2026-04-24-ball-ball-collisions.md`
- `2026-04-24-rail-pocket.md`
- `2026-04-24-event-geometry-multiball.md`

Note: the first `hive_worker` launches failed because the project `nix develop` shell did not include `pi`. I relaunched the same subtasks in isolated hive worktrees with the host `pi` binary, then copied the final reports back here.

## Highest-priority findings

1. **Ball-ball throw is not quantitatively correct.**
   `ThrowAware` maps almost any nonzero stun-shot cut to a fixed `±5°` throw. The TP A.14/A.24 / Coriolis model depends on cut angle, speed/slip, spin, friction, and the `1/7` no-slip-reversal cap. The current stationary-object branch also mixes an object-ball throw heuristic with a separate cue-ball TP A.8/A.24 branch, so horizontal momentum is not guaranteed.

2. **Phase-aware collision prediction can miss grazing contacts.**
   The N-ball/ball-ball predictor scans 512 fixed intervals and refines only sign changes. A real thin/grazing collision can enter and leave contact between samples. The event audit includes a concrete numeric reproducer.

3. **Shared simultaneous contacts are not physically solved.**
   Disjoint simultaneous pairs are batched, but shared contact graphs/frozen clusters are resolved by deterministic pair order. The non-smooth literature points toward coupled impulse solves or an explicit unsupported-contact contract.

4. **The default preview cloth sliding friction is far too low for the local Dr. Dave typical-cloth values.**
   `human_tuned_preview_motion_config()` uses `15 in/s²`; `μs ~= 0.2` implies about `77.2 in/s²`. For a 7 mph stun shot, that stretches roll-development distance from about `4 ft` to about `20.7 ft`.

5. **Rail and pocket models are useful first passes, not paper-calibrated full physics.**
   The spin-aware rail solver lacks a true no-slip/adherence branch at cushion/table contacts; pocket capture is a disk-plus-angle gate rather than the TP 3.5–3.8 effective-target model; fast corner acceptance is looser than TP 3.8's `59.841°` value.

6. **Important omitted effects remain explicit model gaps.**
   Side-offset cue strikes omit squirt; rolling side-spin turn is disabled; ball-ball impacts omit vertical velocity/hops/table impulses; code comments still cite several renamed whitepaper paths.

## Small fix applied by coordinator

I added an immediate rail-impact case for balls already touching a rail and moving into it. Previously `compute_next_ball_rail_impact_on_table(...)` skipped `initial_gap <= 0`, so a frozen-to-rail ball driven into the cushion could tunnel through instead of rebounding at `t = 0`.

Changed files:

- `src/lib.rs`
- `tests/rail_event_scheduling.rs`

Focused verification:

```bash
cargo test --test rail_event_scheduling
```

## Suggested next implementation order

1. Replace `ThrowAware` with a single impulse-based TP A.14/A.24-compatible solve and add momentum/throw-trend tests.
2. Add the grazing collision reproducer as a failing test, then replace fixed-step event scans with analytic/adaptive bracketing.
3. Introduce a contact-graph path for shared simultaneous contacts; initially it can report unsupported if not solved.
4. Retune/add named cloth presets (`dr_dave_typical`, `human_preview`) with explicit `μg` conversion.
5. Add rail no-slip/adherence handling and explicit TP/Mathavan rail profiles.
6. Expand pocket geometry (`facing_angle`, throat/hole/shelf) and implement TP-derived effective-target gates.
