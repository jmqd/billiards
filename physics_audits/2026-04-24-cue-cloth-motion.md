# Physics audit: cue strike, cloth phases, and spin decay

Date: 2026-04-24

## Scope

Audited the current cue-strike and on-cloth single-ball motion model against local literature. No source or test code was changed; this is a report-only audit.

## Local source evidence consulted

Repository workflow sources:

- `AGENTS.md`
- `agent_knowledge/agent_reading_guide.md`
- `agent_knowledge/whitepapers_index.jsonl`
- `agent_knowledge/whitepapers_formula_candidates.txt`
- `agent_knowledge/whitepapers_corpus.txt`

Primary physics sources:

- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf`
  - Gives the cue-tip miscue/grip condition `μ_static >= (b/R) / sqrt(1 - (b/R)^2)` and notes usual play is below roughly `φ < 35°` for `μ ~= 0.7`.
  - For side hits, the ball generally does not leave exactly on the cue line; the paper models nonzero squirt. A realistic flexible-cue model suppresses squirt to order `m_cue_end / M_ball`, but it is still nonzero and increases with impact parameter.
  - For horizontal strokes with small squirt, the summary formulas make post-strike speed and spin scale with cue speed and impact offset; angular velocity comes from the cue contact radius crossed with the outgoing ball velocity.
- `whitepapers/art_of_billiards_play_files/bil_praa.html` (code-cited support source)
  - §7.2 Eq. `1c`: `Wi = W'a * [1 + sqrt(1 - e - e K^2 M'/M)] / [K^2 + M/M']`, `K^2 = 1 + (5/2)(D/R)^2`.
  - §7.2 split condition: `(D/R)^2 < (2/5)(1 + M/M')(1 - e(1 + M'/M))`.
  - §7.3 Eqs. `M4`, `M8`, `M10`, `M10'`: cloth slip velocity, `Wc = Wi - (2/7) WEi`, and sliding transition time.
  - §7.4 Eqs. `M11`-`M12`: rolling resistance as constant deceleration opposite travel.
  - §7.5 Eqs. `M13`-`M14`: vertical-axis spin decays linearly under a constant spin-friction torque.
- `whitepapers/sliding_and_rolling_the_physics_of_a_rolling_ball.pdf`
  - Separates the initial rolling+slipping phase from rolling without slipping.
  - During slip, linear speed and angular speed are not related by `v = Rω`; when `v = Rω`, sliding friction disappears and rolling resistance is much smaller than sliding friction.
- `whitepapers/rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf`
  - Shows a ball can translate while spinning about a near-vertical axis with low rolling-like friction; both `v` and `ω` can decay roughly linearly, and spin can persist after forward motion stops.
  - Documents nonzero curve for side-spin/near-vertical-axis rolling, though this is not directly a pool-cloth calibration.
- `whitepapers/tp_4_1_distance_required_for_stun_and_normal_roll_to_develop.pdf`
  - Uses sliding acceleration `a = ± μ g` and angular acceleration `α = ± 5 μ g / (2R)`.
  - Sliding-to-roll time: `td = ± 2(v - Rω)/(7 μ g)`.
  - Final roll speed: `v' = (5/7)v + (2/7)Rω`; for a stun shot, `v' = (5/7)v`.
  - For a stun-drag shot: `d = 12 v^2 / (49 μ g)`.
- `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf`
  - Immediate natural roll occurs for a horizontal impulse at `a = (2/5)R` above center, i.e. a contact height `0.4R` above the ball center.
- `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf`
  - Uses a typical rolling-resistance coefficient `μr ~= 0.01`.
  - Reports static spin-down deceleration about `10 rad/s^2` and assumes similar spin-down torque while rolling.
  - Predicts a small nonzero side-spin ball-turn effect; example: about `0.217 in` lateral error over `8 ft` at `2 mph`.
- `whitepapers/tp_b_8_draw_shot_physics.pdf`
  - Uses typical `μs = 0.2`, `μr = 0.01`, ball-to-cue mass ratio `mb/ms = 6/19`, and safe miscue limit `bmax = R/2`.
  - Repeats the cue-speed/tip-offset speed and spin relations; with typical tip inefficiency, center-ball CB speed is about `30%` greater than cue speed, while maximum-offset speed is about `75%` of cue speed.
- Dr. Dave draw/follow notes:
  - `whitepapers/draw_shot_physics_part_i_basics.pdf`: tip offset/miscue limit, draw phases, drag spin loss, and that slick/fast cloth increases draw distance.
  - `whitepapers/draw_shot_physic_part_iii_spin_ratio.pdf`: spin and spin-to-speed ratio at object-ball contact; maximum useful draw offset can be below the miscue limit for long drag distances.
  - `whitepapers/follow_control.pdf`: follow is easier to control when the cue ball reaches full topspin roll before object-ball contact.

## Code and tests audited

Primary code paths:

- `src/lib.rs`
  - `CueTipContact`
  - `Shot`
  - `CueStrikeConfig`
  - `validate_shot_and_cue_for_strike(...)`
  - `compute_post_strike_speed(...)`
  - `strike_resting_ball_on_table(...)`
  - `RawOnTableBallState::cloth_contact_velocity(...)`
  - `classify_motion_phase(...)`
  - `raw_advance_within_phase_on_table(...)`
  - `raw_compute_next_transition_on_table(...)`
  - `try_cloth_contact_velocity_on_table(...)`
  - `cloth_contact_velocity_on_table(...)`
  - `advance_motion_on_table(...)`
  - `advance_spin_on_table(...)`
  - `human_tuned_preview_motion_config()`
  - `estimate_post_contact_cue_ball_curve_on_table(...)`

Focused tests read:

- `tests/shot_strikes.rs`
- `tests/motion_phase_classifier.rs`
- `tests/motion_transitions.rs`
- `tests/advance_ball_state.rs`
- `tests/simulation_units.rs`

## Prioritized confirmed bugs / inaccuracies

### P0 - Preview sliding-friction calibration is far too low for Dr. Dave's local typical cloth values

`human_tuned_preview_motion_config()` uses:

- sliding acceleration: `15 in/s^2`
- spin decay: `10.9 rad/s^2`
- rolling deceleration: `5 in/s^2`

The sliding equations in code interpret `acceleration_magnitude` as `μ g`. Dr. Dave's TP B.8 / TP 4.1 typical sliding value is `μs = 0.2`, so `μs g ~= 77.2 in/s^2`.

Consequence for a 7 mph stun shot:

- TP 4.1 with `μ = 0.2`: `d = 12 v^2 / (49 μ g) ~= 48.1 in ~= 4.0 ft`.
- Current preview config with `a = 15 in/s^2`: `d ~= 247.8 in ~= 20.7 ft`.

That is a roughly `5.15x` longer stun/roll-development distance in the CLI preview path. The symbolic solver is correct, but the named `human_tuned_preview_motion_config()` is not well calibrated to the local Dr. Dave typical-cloth references.

### P1 - Side-offset cue strikes intentionally omit squirt

`strike_resting_ball_on_table(...)` always launches the cue ball along `shot.heading`; side offset only seeds `ωz`.

That matches the simplified Petit / `art_of_billiards_play` cue-collision assumption that the transverse shock is absorbed and the percussion is parallel to the cue. It does **not** match the fuller Kim cue-stroke paper for side hits, where side offset produces a nonzero squirt angle. Kim's flexible-cue model says good low-end-mass cues can keep this under about a degree for ordinary offsets, but the effect is still real and matters over long distances.

Priority is medium for current simulation because the omission is explicit and conservative, but it should not be presented as a complete side-spin cue-strike model.

### P2 - Rolling side-spin turn is currently zeroed

The current solver advances rolling balls in a straight line even with residual `ωz`, and `estimate_post_contact_cue_ball_curve_on_table(...)` returns `None` by design.

This avoids the earlier over-strong cloth-turn heuristic, and it is defensible as a conservative horizontal model. However, it is not fully physical:

- TP B.2 predicts a small side-spin-induced turn while rolling.
- `rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf` experimentally shows side-spin/near-vertical-axis rolling can curve and can keep spinning after translation stops.

For pool calibration, TP B.2 frames the turn as small; this is a lower-priority inaccuracy than the sliding-friction preset.

### P3 - Cue-strike calibration parameters are configurable but not anchored by tests

The cue-strike formula itself matches the code-cited Petit equation, but the focused tests use artificial values (`cue_mass_ratio = 1.0`, `collision_energy_loss = 0.1`). Dr. Dave TP B.8's typical draw-shot values imply roughly:

- cue-to-ball mass ratio `ms/mb ~= 19/6 ~= 3.17`, and
- center-ball speed about `1.3x` cue speed with typical tip inefficiency.

The API can represent this, but no regression test anchors a realistic preset or documents how `collision_energy_loss` maps to Dr. Dave's `ηtip`-style cue-tip efficiency.

## Likely calibration gaps

- Sliding friction should be exposed or preset as a coefficient `μs` times a named gravity constant instead of a raw acceleration, or at least documented with a typical value near `77 in/s^2` for `μs = 0.2`.
- Rolling resistance is closer: `5 in/s^2` corresponds to `μr ~= 0.013`, versus TP B.2 / TP B.8's `μr ~= 0.01` (`3.86 in/s^2`). This is plausible but should be documented.
- Spin decay `10.9 rad/s^2` is close to TP B.2's measured `10 rad/s^2`; this looks reasonable.
- Draw/follow outcome calibration is not yet tested against TP B.8 examples: tip offset vs. drag distance, maximum useful draw near `70%-80%` offset for long draw, and spin-to-speed-ratio behavior near the miscue limit.
- Cue elevation, swerve, masse, jump, and detailed tip-size/miscue probability remain out of scope for the current horizontal on-table model.

## False alarms / things that look correct

- `cloth_contact_velocity_on_table(...)` uses `(vx - Rωy, vy + Rωx)`, matching the local TP A.4 / Petit contact-point velocity convention; `ωz` correctly does not enter the reduced horizontal slip velocity.
- `classify_motion_phase(...)` correctly classifies draw and overspin as `Sliding` until cloth slip vanishes; it does not confuse spin direction with rolling.
- A stationary ball with only `ωz` is classified as `Spinning`, while a stationary ball with horizontal-axis spin is classified as `Sliding`, which is consistent with cloth-contact slip.
- The sliding advance equations match Petit §7.3 / TP 4.1: `Wc = Wi - (2/7)WEi`, `tc = (2/7)||WEi||/(μg)`, and horizontal angular velocity is updated with the coupled `5/(2R)` factor.
- The `0.4R` high-center strike test is correct: TP 4.2 gives immediate natural roll at `(2/5)R` above center.
- Vertical-axis spin decay as constant angular deceleration through sliding, rolling, and pure spinning is supported by Petit §7.5 and is close to TP B.2's measured spin-down number.
- The rolling phase's straight-line constant deceleration is a valid first approximation for rolling resistance; the main issue is missing small side-spin turn, not the basic rolling-stop formula.

## Concrete suggested code / test changes

1. Add a local-gravity constant and a Dr. Dave typical motion preset, or retune `human_tuned_preview_motion_config()`:
   - `sliding_friction = 0.2 * g ~= 77.2 in/s^2`
   - `rolling_resistance = 0.01 * g ~= 3.86 in/s^2` (or document the current `5 in/s^2` as slightly slower cloth)
   - `spin_decay ~= 10 rad/s^2`
2. Add a TP 4.1 regression test for a 7 mph stun shot:
   - transition distance `~= 4.01 ft` with `μ = 0.2`
   - transition time `~= 0.456 s`
   - final rolling speed `= 5/7` of initial speed.
3. Add a realistic cue-strike fixture using `cue_mass_ratio ~= 19/6` and a documented `collision_energy_loss` chosen to reproduce TP B.8's typical center-ball speed ratio.
4. Add a documentation/test seam for side-offset cue strikes:
   - either explicitly assert the current no-squirt approximation, or
   - add an optional Kim-style cue-end-mass/squirt model that produces a small lateral velocity component.
5. Add a small TP B.2 side-spin-turn estimator or rename/document `estimate_post_contact_cue_ball_curve_on_table(...)` as intentionally disabled until calibrated.
6. Fix code-doc path names that still reference legacy names like `whitepapers/TP_A-4.pdf` instead of the generated-index paths such as `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf`.

## Verification

Focused checks run successfully:

```text
cargo test --test shot_strikes --test motion_phase_classifier --test motion_transitions --test advance_ball_state --test simulation_units
```

Result: all focused tests passed (`49` tests total across the five test binaries).

## Self-review

- Re-read the report after writing for consistency with the audited code paths and source evidence.
- No edits were made under generated `agent_knowledge/`.
- No source/test changes were made because the clearest issues are calibration/model-scope findings rather than a tiny isolated correctness fix.
