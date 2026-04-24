# Physics audit: event scheduling, geometry, and multi-ball assumptions (2026-04-24)

Scope: audit event scheduling, table/pocket geometry, ball/path tracing, and N-ball/contact-solver assumptions against the local literature and current tests. This is report-only; no generated `agent_knowledge/` files were edited.

## Literature evidence consulted

Primary local sources:

- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`
  - Treats impacts as instantaneous/non-smooth impulses with restitution and friction; explicitly separates ball-ball and ball-supporting-surface interactions.
  - Notes billiard-ball fit values around `e = 0.97`, ball-ball dynamic friction `mu ~= 0.07`, ball-table dynamic friction `mu' ~= 0.15`, and that different slip/stick regimes can occur.
  - States the discontinuous approach is useful but conditioned by simplifying assumptions: constant coefficients and idealized impulse events.
- `whitepapers/computational_pool_an_or_optimization_point_of_view.pdf`
  - Splits computational pool into physics simulation, execution uncertainty, and planning.
  - Describes trajectories as continuous `s(tau)` and emphasizes simulator accuracy as a black-box dependency for planning.
  - Calls out clusters/collisions with other balls as involving elastic-collision singularities and more intricate optimization.
- `whitepapers/toward_a_competitive_pool_playing_robot.pdf`
  - Describes a continuous-domain event simulator that predicts pending events: ball-ball, ball-rail, ball-pocket, and motion transitions.
  - For two moving balls it derives event time from separation as a function of time; the resulting equation is a quartic polynomial solved iteratively or closed-form.
  - Explicitly contrasts this with fixed numerical integration: no discrete time step, higher accuracy, and much lower cost.
  - Also emphasizes shot noise over five stroke parameters and planning under uncertainty.
- `whitepapers/robotic_pool_an_experiment_in_automatic_potting.pdf`
  - Reports robot/table calibration and potting accuracy limits; useful evidence that ideal geometry alone is insufficient for real play.
- `whitepapers/robotic_billiards_understanding_humans_in_order_to_counter_them.pdf`
  - Models pool as continuous-state/action, stochastic in real execution due to motor/perception/model errors; uses stroke-difficulty ranges and pocket probability.
- Pocket/table technical papers:
  - `whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf`
  - `whitepapers/tp_3_6_effective_target_sizes_for_slow_shots_into_a_corner_pocket_at_different_angles.pdf`
  - `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf`
  - `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf`
  - `whitepapers/tp_b_15_pocket_geometry_calculations.pdf`
  - `whitepapers/billiard_university_bu_part_iv_table_difficulty.pdf`
  - These define effective target size from ball radius, mouth width, wall/facing angle, hole radius, shelf depth, speed, and approach angle. BU table difficulty also treats mouth size, throat/facing angle, and shelf depth as first-order table-difficulty inputs.

Generated lookup files used: `agent_knowledge/agent_reading_guide.md`, `agent_knowledge/whitepapers_index.jsonl`, `agent_knowledge/whitepapers_formula_candidates.txt`, and `agent_knowledge/whitepapers_corpus.txt`.

## Code and tests audited

Code paths:

- `src/lib.rs`
  - Event prediction: `compute_next_ball_ball_collision_on_table`, `compute_next_ball_ball_collision_during_current_phases_on_table`, `compute_next_ball_rail_impact_on_table`, `compute_next_ball_jaw_impact_on_table`, `compute_next_ball_pocket_capture_on_table`, `select_earliest_n_ball_event_from_states`, `compute_next_n_ball_system_event_with_rails_and_pockets_on_table`.
  - Event execution/simulation: `advance_to_next_n_ball_event_with_scheduler`, `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table`, `simulate_n_balls_on_table_until_rest`, `simulate_n_ball_system_with_physics_and_pockets_on_table_until_rest`, `PocketAwareEventCache`.
  - Tie handling: `earlier_n_ball_event_candidate`, `earlier_n_ball_pocket_aware_event_candidate`, `prefer_explicit_jaw_over_nearby_capture`, `simultaneous_disjoint_zero_time_ball_ball_collisions_from_state_refs`.
  - Geometry/unit model: `Position`, `TableSpec`, `BallSpec`, `BallSetPhysicsSpec`, `Rail`, `Pocket`, `PocketSpec`, `PocketShapeSpec`, `pocket_jaw_reference_point_in_inches`, `pocket_capture_radius_in_inches`, `pocket_entry_axis`.
  - Ball/path tracing: `BallPath`, `BallPathSegment`, `trace_ball_path_with_rail_profile_on_table`, `projected_points`, `sampled_projected_points`.
  - Collision response: `ideal_ball_ball_collision_velocities`, `frictional_collision_outcome_on_table_with_config`, `collide_ball_ball_*`, `collide_ball_rail_*`, `collide_ball_jaw_*`.
- Tests read:
  - Required set: `tests/n_ball_events.rs`, `tests/n_ball_advance.rs`, `tests/n_ball_simulation.rs`, `tests/two_ball_simulation.rs`, `tests/advance_two_ball_events.rs`, `tests/next_events.rs`, `tests/position_geometry.rs`, `tests/table_and_pocket_geometry.rs`, `tests/rack_and_displacement.rs`.
  - Additional relevant pocket/cache coverage: `tests/n_ball_pockets.rs`.

## Confirmed bugs / inaccuracies, prioritized

### P1 — Phase-aware event scheduling can miss real grazing ball-ball collisions

`compute_next_ball_ball_collision_during_current_phases_on_table` samples 512 evenly spaced times over the current phase horizon and only refines when the signed distance-squared gap changes from positive to non-positive. That is a fixed-step bracketing scheme, not the event-polynomial approach described in `toward_a_competitive_pool_playing_robot.pdf`.

Confirmed counterexample using the public API (ad-hoc `/tmp` Rust program; not committed because this task requested report-only):

- Ball radius `R = 1.125 in`, contact distance `2R = 2.25 in`.
- Rolling ball starts at `(0, 0)` with `v = (10, 0) in/s`, rolling spin `wy = v/R`, rolling deceleration `5 in/s^2`.
- Stationary ball center placed at:
  - `x = 4.389638900756836 in`
  - `y = 2.249999 in`
- At `t = 128.5 / 256 = 0.501953125 s`, the rolling ball's x-position equals the target x and the center separation is `2R - 0.000001 in`, so a collision exists.
- The phase horizon is `2.0 s`, so the scan step is `2/512 = 0.00390625 s`. The contact interval is much narrower than one scan interval, and the nearest samples fall outside the contact disk.
- Result: `compute_next_ball_ball_collision_during_current_phases_on_table(...)` returns `None`.

Why this matters: near-tangent contacts are precisely where pocket points, thin cuts, kisses, and cluster grazes live. The local robot paper's quartic/event-root framing is intended to avoid this class of missed event.

Suggested change:

- Replace fixed scan-only collision timing with an analytic or adaptive event solve.
  - For current within-phase motion, relative position is polynomial within a phase; solve/minimize the contact-distance function per interval, or at least bracket all local minima before bisection.
  - Add a regression test with the numeric scenario above in `tests/n_ball_events.rs` or a dedicated `tests/ball_collision_timing.rs` case.
  - Apply the same audit to rail/jaw/pocket predictors where curved sliding paths can briefly enter and exit a capture/contact region between samples.

### P1 — Shared simultaneous contacts are sequential/tie-broken, not a coupled multi-contact solve

Current code has deterministic tie ordering and limited batching:

- N-ball events break ties by event source and index.
- `simultaneous_disjoint_zero_time_ball_ball_collisions_from_state_refs` batches only disjoint ball-ball pairs.
- Shared-contact graphs, e.g. one cue ball simultaneously contacting two object balls, a frozen rack cluster, or ball-ball plus rail/jaw at the same instant, are resolved by lexicographic sequential events.

This is explicitly an approximation, but it is physically important. The non-smooth collision paper supports impulse-based impacts with friction/restitution, while the computational-pool survey calls cluster/collision singularities more intricate. A shared-contact cluster generally needs a coupled contact impulse solve or a deliberate approximation contract; pairwise order changes can change momentum distribution.

Suggested change:

- Introduce an explicit simultaneous-contact collection step with a tolerance window.
- Build a contact graph and handle three cases separately:
  1. disjoint pairs: current batching is acceptable;
  2. shared graph: either solve coupled impulses or return/record `UnsupportedSharedContact` instead of silently imposing lexicographic physics;
  3. simultaneous rail/jaw + ball-ball: define a contract and test it.
- Add tests for a cue ball hitting two frozen object balls symmetrically and for ball-ball + rail contact at the same time.

### P2 — Pocket geometry is under-modeled relative to the pocket literature

The current `PocketSpec` stores only `ty`, `depth`, `width`, and jaw nose shape. In prediction:

- `pocket_capture_radius_in_inches` uses half mouth width, with a constant corner scale.
- `pocket_center_in_inches` uses `Pocket::aiming_center()`, not a hole center derived from shelf/depth.
- `PocketSpec.depth` is not used by jaw/capture prediction.
- There is no explicit throat width, facing angle, shelf-depth-to-hole relation, or `Rhole` parameter.
- Jaw points are inferred from mouth width only (`pocket_jaw_reference_point_in_inches`).

The TP 3.5--3.8 papers derive effective target size from `R`, mouth width `p`, wall/facing angle `alpha`, hole radius `Rhole`, shelf depth `b`, speed class, and approach angle. TP B.15 and BU Part IV show that facing angle/throat and shelf depth are core table measurements.

This is not necessarily a code bug for a first-pass acceptance gate, but it is an inaccuracy relative to the cited papers. It can pocket too early/late or accept/reject the wrong jaw-rattle cases because it reduces a multi-parameter pocket to a circular capture region and a few angle constants.

Suggested change:

- Expand `PocketSpec` toward measured table geometry:
  - mouth width,
  - throat width or facing angle,
  - shelf depth / hole-center offset,
  - hole radius,
  - jaw/facing line geometry.
- Move the TP 3.5--3.8 constants into named calibration structs, and derive side/corner acceptance from the same inputs rather than global constants.
- Add tests for:
  - BU/WPA example table measurements,
  - TP B.15 facing-angle conversion,
  - slow vs fast side-pocket acceptance around the published `68.292°` and `50.688°` side-pocket bounds,
  - corner-pocket fast bound around `59.841°` from TP 3.8.

### P2 — Default table/pocket calibration is plausible but not well tied to a measured table

`TableSpec::brunswick_gc4_9ft` uses:

- 9 ft playing area as `4 x 8` diamonds and `12.5 in/diamond`, matching a `50 x 100 in` nose-to-nose playing area.
- corner pocket width `4.5 in`, side pocket width `5.0 in`, pocket depth `1.4 in`, jaw nose radius `0.125 in`.

The 9 ft unit convention is consistent. The pocket numbers need clearer calibration provenance. BU Part IV lists a Brunswick Gold Crown example with different corner-mouth/shelf-style measurements, and the TP pocket papers use specific `p`, `alpha`, `Rhole`, and `b` values. Current code says "typical Brunswick GC IV" but does not document which measured mouth/throat/shelf convention those values correspond to.

Suggested change:

- Rename the default if it is a tight/pro-cut profile, or add a comment with the measurement convention/source.
- Add named presets: `wpa_standard_9ft`, `brunswick_gold_crown_example`, `diamond_pro_cut`, etc.

### P3 — Code-cited whitepaper paths have drifted

Several `src/lib.rs` comments cite old/nonexistent local paths, even though `TODO.org` records old-to-new mappings. Examples checked locally:

- missing `whitepapers/Physics Of Billiards.html` -> present `whitepapers/the_physics_of_billiards.html`
- missing `whitepapers/TP_A-4.pdf` -> present `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf`
- missing `whitepapers/TP_4-2.pdf` -> present `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf`
- missing `whitepapers/Alciatore_pool_physics_article.pdf` -> present `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf`
- missing `whitepapers/billiards_ball_collisions.pdf` -> present `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`

This is documentation rot, not runtime behavior, but it weakens future audits.

Suggested change:

- Update code comments to canonical generated filenames.
- Optionally add a small script/check that extracts `whitepapers/...` references from code/docs and verifies they exist or are listed in the old-to-new map.

## Likely calibration gaps

- **Ball-ball coefficients:** `BallBallCollisionConfig::human_tuned()` uses `e = 0.95` and `mu = 0.06`; Doménech reports regulation billiard balls near `e = 0.97 ± 0.02`, `mu = 0.07 ± 0.02`, with ball-table friction near `0.15`. Current values are close but should be table/ball calibrated.
- **Rail model:** current rail coefficients are pragmatic and partially literature motivated, but rails are highly table-specific. Cushion height/compliance and rail-cloth simultaneous contact remain simplified.
- **Shot/execution uncertainty:** robotics papers model calibration, motor error, and shot noise. Current event simulation is deterministic and has no stochastic shot-success layer.
- **Airborne/leaves-table:** explicitly unsupported in `NBallSystemState` and on-table APIs. Doménech/Mathavan note cue-ball/object-ball lift/jump effects during impacts; current code intentionally excludes them.
- **Pocket drops:** pocket capture is a gate/removal event, not a full falling-ball or shelf/jaw/rattle dynamics model.

## False alarms / acceptable current assumptions

- **Diamond/inch unit convention:** `TableSpec`'s `12.5 in/diamond` with a `4 x 8` diamond playing rectangle gives `50 x 100 in`, matching the 9 ft nose-to-nose table convention in BU Part IV.
- **Rail contact planes:** using `x = radius`, `x = 50 - radius`, `y = radius`, `y = 100 - radius` is consistent with modeling rail contact at the cushion nose line for on-table center positions. `cushion_diamond_buffer` appears to be diagram/asset geometry, not the physics rail plane.
- **Rack spacing:** `racked_ball_positions()` uses `2R` and `sqrt(3) R`; tests verify the nine-ball triangle has the expected 16 touching pairs without overlaps.
- **Disjoint simultaneous pair handling:** for independent pairs, current batching is tested and reasonable. The issue is shared-contact graphs, not disjoint pair batching.
- **Ball path tracing:** `trace_ball_path_*` is documented as an event-vertex trace, and `BallPath::sampled_projected_points` exists for densifying within segments. Coarse `projected_points()` is not a hidden physics claim.
- **Pocket-aware cache:** for simple non-tie pocket simulations, `tests/n_ball_pockets.rs` shows cached simulation matches manual event stepping. More tie/cache tests are still recommended.

## Concrete next code/test changes

1. Add a regression test demonstrating the grazing collision miss, then replace fixed-step scan bracketing with analytic/adaptive event timing.
2. Add shared simultaneous-contact tests and make the API contract explicit for coupled contacts.
3. Expand `PocketSpec` to include measured facing/throat/shelf/hole parameters and derive capture/acceptance from TP 3.5--3.8 / TP B.15 data.
4. Add named table presets and documentation for pocket measurement conventions.
5. Update stale code-cited whitepaper paths and add an existence check for `whitepapers/...` references.
6. Include `tests/n_ball_pockets.rs` in future focused pocket/cache audit commands; it is directly relevant to `PocketAwareEventCache` even though not in this task's mandatory check list.

## Verification

Focused checks requested by the task:

```bash
cargo test --test n_ball_events --test n_ball_advance --test n_ball_simulation --test two_ball_simulation --test advance_two_ball_events --test next_events --test position_geometry --test table_and_pocket_geometry --test rack_and_displacement
```

Status: passed (`43` tests across the requested nine integration-test targets).

## Manual self-review

- Re-read the report for scope, source evidence, prioritized findings, false alarms, and concrete changes.
- Confirmed no source/test code changes were made; only this tracked audit report is intended for commit.
- Residual risk: this was an audit pass, not a full physics rewrite; the event-miss counterexample should be turned into an automated test before changing scheduling internals.
