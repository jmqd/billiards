# Ball-ball collision, throw, gearing english, and spin-transfer audit

Date: 2026-04-24  
Worktree: `/Users/jmq/.local/share/agent-hive/worktrees/billiards-7a3de1bbf8bd8b4a/agent-02`

## Scope and workflow

Audited the requested ball-ball collision code and focused tests against the local literature distillation. I read `AGENTS.md` and `agent_knowledge/agent_reading_guide.md` first, then used:

- `agent_knowledge/whitepapers_index.jsonl`
- `agent_knowledge/whitepapers_formula_candidates.txt`
- `agent_knowledge/whitepapers_corpus.txt`

Note: the requested `whitepapers/tp_a_8_the_effects_of_english_on_the_30_degree_rule.pdf` is not present under that exact filename. The local indexed file is `whitepapers/tp_a_8_the_effects_of_sidespin_on_the_30_degree_rule.pdf`, titled “TP A.8 - The effects of English on the 30° rule”; that is what I audited.

## Source evidence used

Primary/local sources checked:

- `whitepapers/collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf`
  - Sets up identical-ball 3D contact with Coulomb ball-ball friction.
  - Relative tangential contact velocity is governed by `2V + rΩz` and `2W - rΩy`; the friction direction is fixed for the no-table sliding case (`dθ/dt = 0`).
  - For sliding friction as the only dissipation, the normal impulse remains the elastic value `P = 2mU(0)` in center-of-mass coordinates; with additional normal dissipation, allowed `P` is lower.
  - Both balls receive equal angular-velocity increments from the ball-ball friction torque.
  - The paper explicitly calls out table complications after impact: vertical velocity components, hops, table impacts, and subsequent rolling-without-slipping evolution.
- `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf`
  - Gives representative ball-ball friction `0.03 <= μ <= 0.08` and restitution `0.92 <= e* <= 0.98`.
  - Shows ball-table static friction during collision can be significant when follow/draw creates vertical friction components; static table friction is reported around `0.2 < μs < 0.4`.
  - Defines initial contact relative velocity components: `vy(0) = -Ui sin φ - Rωz(0)` and `vz(0) = Rωy(0)`, i.e. throw/gearing depend on horizontal slip and follow/draw vertical slip.
- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf`
  - Post-impact sliding cue-ball contact velocity is `vC = (vx - Rωy)i + (vy + Rωx)j`.
  - It explicitly states z-axis spin (`ωz`, side English) does not affect cloth contact-point velocity in this horizontal post-impact model.
  - Sliding acceleration direction remains constant until rolling, giving parabolic cue-ball bend from follow/draw.
- `whitepapers/tp_a_8_the_effects_of_sidespin_on_the_30_degree_rule.pdf`
  - For rolling with English, ball-ball contact slip is `vrel = [v sin φ - Rω] t - v cos φ k`.
  - Gives ball-ball friction impulse components and post-impact cue-ball velocity/spin components for the e=1, stationary-object case.
- `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf`
  - Uses `vrel = sqrt((v sin φ - Rωz)^2 + (Rωx cos φ)^2)`.
  - Object-ball tangential throw speed is based on `min( μ(vrel) v cosφ / vrel, 1/7 ) * (v sinφ - Rωz)`.
  - Throw angle is `atan(vOBt / vOBn)` with `vOBn = v cosφ`.
  - Follow/draw reduce throw by pushing contact slip into the vertical component.
- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`
  - Uses normal/tangential impulses, normal restitution, gross-slip vs stick/slip regimes, and a two-step ball-ball then ball-table interaction.
  - Normal impulse for cue/object spheres is proportional to `(1 + e) v0 cos c`.
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf`
  - Ideal object-ball direction is line of centers and cue-ball ideal direction is tangent line.
  - Throw depends on shot speed, cut angle, spin, and ball friction; Eq. 3 contains the same TP A.14/A.24 structure and the `1/7` no-slip-reversal cap.
  - Practical conclusions: small-cut throw increases with cut angle, maximum throw is near a half-ball hit, and faster shots generally throw less at larger cuts.
- Additional directly relevant local sources:
  - `whitepapers/tp_a_26_the_amount_of_sidespin_required_for_gearing_outside_english.pdf`: gearing condition `vC = v sin φ - ωR = 0`.
  - `whitepapers/tp_a_27_spin_transfer.pdf`: maximum spin-transfer fraction is `5/14 = 35.71%` in the no-slip limit for a straight-on English shot.

## Code paths audited

- `src/lib.rs:3425` `collision_contact_basis`
- `src/lib.rs:3467` `ideal_ball_ball_collision_velocities`
- `src/lib.rs:3522` `compute_next_ball_ball_collision_on_table`
- `src/lib.rs:3587` `compute_next_ball_ball_collision_during_current_phases_on_table`
- `src/lib.rs:6400` `ideal_collision_outcome_on_table_with_config`
- `src/lib.rs:6432` `transferred_spin_from_contact_slip`
- `src/lib.rs:6469` `spin_post_impact_cue_state_from_tp_a8_a24`
- `src/lib.rs:6554` `frictional_collision_outcome_on_table_with_config`
- `src/lib.rs:6699` `throw_aware_collision_outcome_on_table_with_config`
- `src/lib.rs:6707` `spin_friction_collision_outcome_on_table_with_config`
- `src/lib.rs:6737` `collide_ball_ball_detailed_on_table_with_config`
- `src/lib.rs:6831` `estimate_post_contact_cue_ball_curve_on_table`
- `src/lib.rs:6968` `estimate_post_contact_cue_ball_bend_on_table`
- `src/lib.rs:8141` `gearing_english`

Focused tests read:

- `tests/ball_collisions.rs`
- `tests/non_ideal_ball_collisions.rs`
- `tests/ball_collision_timing.rs`
- `tests/physics.rs`
- `tests/next_events.rs`

## Confirmed bugs / inaccuracies, prioritized

### P1: Throw angle saturates to ±5° for almost any nonzero stun-shot cut

`frictional_collision_outcome_on_table_with_config` computes:

```rust
THROW_AWARE_MAX_ANGLE_DEGREES
    * (tangential_contact_slip / throw_direction_scale).clamp(-1.0, 1.0)
```

For a stun/no-English cut, `vertical_contact_slip == 0`, so any nonzero horizontal contact slip produces `±5°`, independent of cut angle, speed, and friction magnitude.

This disagrees with the TP A.14/A.24 / Coriolis article model:

- `vOBt` is proportional to `(v sinφ - Rωz)` and capped by `1/7`.
- `θthrow = atan(vOBt / (v cosφ))`.
- Small-cut stun throw should be small, approximately `atan(tanφ / 7)` in the no-slip-limit region, not 5°.
- Maximum CIT should occur around a half-ball hit, and larger-cut throw should vary with speed/friction.

Current tests catch zero/gearing signs but do not check magnitude or cut-angle trend.

### P1: Stationary-object non-ideal branch mixes two incompatible solvers and can violate horizontal momentum consistency

For stationary object-ball cut shots, the object-ball branch is set by rotating the ideal object speed by the heuristic throw angle, while the cue-ball branch can be replaced by `spin_post_impact_cue_state_from_tp_a8_a24`. Those are independent constructions, not the two sides of one normal+tangential impulse solve.

Consequence: total horizontal momentum is not guaranteed to match the pre-impact total. Example from the implemented formulas for a 45° no-English 10 ips cut gives post-impact velocity sum about `(-0.155, 10.117)` ips instead of `(0, 10)` ips.

All primary impulse models audited compute cue and object linear/spin changes from the same contact impulses. Absent explicit table impulse during the very short ball-ball event, horizontal momentum should be conserved.

### P1/P2: `SpinFriction` for moving object balls is not a physically valid generalization

When `require_stationary_object_ball == false`, the code still computes:

- `object_speed = speed(ideal.b_after)`
- `b_velocity = object_speed * rotated(line_of_centers, throw_angle)`
- `a_velocity = total_pre_momentum - b_velocity`, unless the stationary-only TP A.8 cue branch applies

For a moving object ball, the ideal `b_after` can contain an incoming tangential component unrelated to the line of centers. Collapsing that to a speed and rotating around the line of centers discards the actual normal/tangential impulse structure.

Peskin and Doménech both formulate the general problem with normal/tangential relative velocities and impulses. A moving-object model should update both bodies by impulses in the contact basis, not rotate one outgoing speed.

### P2: Tangential friction / spin-transfer cap ignores restitution and dynamic friction calibration

`transferred_spin_from_contact_slip` caps the no-slip spin increment with:

```rust
μ * normal_relative_speed / contact_slip_norm
```

For equal masses with restitution `e`, the normal impulse per mass is `0.5 * (1 + e) * normal_relative_speed`, not always `normal_relative_speed`. The current cap is correct only in the e=1 case. With configured `e < 1`, it overstates available tangential impulse by `2/(1+e)`.

Also, the local throw sources use speed-dependent dynamic ball-ball friction. The default constant `μ = 0.06` is a reasonable first-order value, but the throw and spin-transfer magnitudes are not calibrated to speed/condition trends.

### P2: Table interaction during the collision is outside the model

The current response is an on-table horizontal model. It does not represent vertical velocity, hops, or ball-table static friction impulses during the ball-ball contact.

That is an acceptable simplification for many shots, but it is not the full model described by Kim 2024, Peskin’s table-complications note, or Doménech’s two-step ball-ball / ball-surface interaction. The gap matters most for strong follow/draw, fast impacts, and vertical contact slip.

### P2/P3: Radius handling is not consistently configurable

- `gearing_english` hardcodes `TYPICAL_BALL_RADIUS`.
- `frictional_collision_outcome_on_table_with_config` infers `ball_radius` as half the current center distance.

The literature formulas use a physical ball radius `R`. For custom ball sets or slightly overlapped/separated numerical collision states, the current API can produce radius-dependent throw/gearing/spin artifacts.

### P3: Phase-aware collision search is robust enough for current tests but can miss narrow sign-change intervals

`compute_next_ball_ball_collision_during_current_phases_on_table` samples 512 intervals and refines only when the squared gap changes sign. Within-phase relative motion can make the gap dip below zero and recover between samples, especially for grazing or high-curvature sliding paths.

This is a numerical scheduling risk, not a literature-physics error. Current focused tests cover head-on, misses, oblique ideal contact, rolling-before-stop, and stop-before-contact cases.

### P3: Public docs are stale for `ThrowAware`

The `CollisionModel::ThrowAware` enum docs still say it does not model transferred spin or later cue-ball bend, but the current code does return `transferred_spin` and seeds a TP A.8/A.24 cue-ball post-impact state. This is documentation drift, not a physics failure.

## Likely calibration gaps

- `HUMAN_TUNED_BALL_BALL_NORMAL_RESTITUTION = 0.95` is within Kim’s `0.92 <= e* <= 0.98`, but it is not tied to shot speed, ball condition, or measurement data.
- `DEFAULT_BALL_BALL_TANGENTIAL_FRICTION_COEFFICIENT = 0.06` is within Kim’s `0.03 <= μ <= 0.08` and matches TP examples, but the local throw model should eventually use dynamic friction vs contact-slip speed.
- The constant `THROW_AWARE_MAX_ANGLE_DEGREES = 5.0` is not a valid replacement for the TP A.14/A.24 throw equation. It roughly resembles a midrange throw magnitude but fails cut-angle and speed trends.
- The current model has no explicit dirty-ball/cling/skid parameter, though local sources emphasize ball conditions can strongly change throw.
- `estimate_post_contact_cue_ball_bend_on_table` reuses the table sliding solver and is qualitatively aligned with TP A.4, but quantitative bend calibration depends on the separate cloth friction model.

## False alarms / things that look OK

- `ideal_ball_ball_collision_velocities` matches equal-mass normal exchange with optional normal restitution and preserves tangential components. This is consistent with the ideal line-of-centers/tangent-line references.
- `collision_contact_basis` uses a consistent normal from ball A to ball B and an in-plane tangent. The gearing and over-gearing tests confirm the sign convention is internally coherent.
- `gearing_english(cut_angle, speed)` implements `|ω| = v sinφ / R`, matching TP A.26 and the zero of `(v sinφ - Rωz)` in the throw formulas. Caveat: it uses the typical radius only.
- The no-slip spin-transfer scale `5/(14R)` matches the solid-sphere maximum spin-transfer fraction `5/14` from TP A.27 and is consistent with the impulse reduction of relative contact slip.
- Returning `None` from `estimate_post_contact_cue_ball_curve_on_table` for side-spin-only horizontal shots is consistent with TP A.4: `ωz` does not affect cloth contact-point velocity. Practical swerve requires cue elevation/massé components outside this horizontal model.
- `estimate_post_contact_cue_ball_bend_on_table` correctly isolates the follow/draw sliding-to-rolling bend component by zeroing `ωz` for that estimate.
- The focused event/timing tests pass and cover the main ideal scheduler cases currently claimed by the code.

## Suggested code/test changes

1. **Replace the throw-angle heuristic with the TP A.14/A.24 structure.**
   - Compute contact slip components `v_t = tangential_contact_slip`, `v_z = vertical_contact_slip`.
   - Compute `v_rel = sqrt(v_t^2 + v_z^2)`.
   - Use a normal impulse per mass `j_n = 0.5 * (1 + e) * normal_relative_speed` for equal masses.
   - Use either configured constant `μ` initially or a dynamic `μ(v_rel)` later.
   - Cap tangential impulse by both Coulomb friction and the no-slip reversal limit (`1/7` for solid equal spheres).
   - Compute `throw_angle = atan2(v_ob_t, v_ob_n)`, not a fixed max-angle scale.
2. **Make the non-ideal collision response a single impulse solve.**
   - Derive both cue and object velocities/spins from the same normal and tangential impulse vector.
   - Preserve horizontal momentum unless a deliberate table-impulse term is added.
   - Add tests checking total momentum for stationary-object non-ideal cuts.
3. **Either restrict or correctly implement moving-object `SpinFriction`.**
   - Short term: document/fallback to ideal outside stationary-object cases.
   - Better: implement general relative-velocity impulse updates in the contact basis.
   - Add tests with object-ball pre-impact tangential velocity and spin.
4. **Add throw trend regression tests.**
   - A 1° or 5° stun cut should throw much less than 5°.
   - 30° stun/no-English throw should be larger than small-cut throw and near the configured constant-μ prediction.
   - Fast vs slow dynamic-friction tests can wait until dynamic friction exists.
   - Gearing outside English should remain zero throw.
   - Follow/draw should reduce throw through the vertical slip denominator.
5. **Thread physical ball radius through collision helpers.**
   - Avoid inferring `R` from current center separation.
   - Provide a radius-aware gearing helper or accept `BallSetPhysicsSpec`.
6. **Improve phase-aware collision search if grazing misses matter.**
   - Use adaptive bracketing/minimization of the squared gap or analytic roots within constant-acceleration segments.
7. **Update public docs.**
   - Bring `CollisionModel::ThrowAware` / `CollisionOutcome::transferred_spin` comments in line with current behavior and limitations.

## Checks

Ran:

```sh
cargo test --test ball_collisions --test non_ideal_ball_collisions --test ball_collision_timing --test physics --test next_events
```

Result: passed, 44 tests total.

## Manual self-review

- Re-read this report for source-path accuracy, especially the TP A.8 filename mismatch.
- Confirmed no generated `agent_knowledge/` files were edited.
- No source or test code changes were made; this audit is report-only.
- Residual risk: I did not implement a reference numerical impulse solver, so quantitative examples are formula/code-inspection based rather than new executable assertions.
