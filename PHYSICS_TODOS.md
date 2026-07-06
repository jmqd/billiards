# Physics TODOs: swerve, massé, and elevated shots

Audit date: 2026-07-02.

Scope: current Rust physics implementation cross-checked against the in-repo whitepaper corpus and focused code/tests. The core TP A.4 sliding solver looks source-aligned; the items below are the discrepancies or material limitations that remain.

## Confirmed aligned pieces

- **TP A.4 sliding cloth-contact model is implemented coherently.** `src/lib.rs` computes cloth contact velocity as `(vx - R*wy, vy + R*wx)`, matching TP A.4's `v_C = v + omega x R` and correctly excluding `omega_z` side spin from sliding contact velocity. Sliding advancement uses the `2/7` slip decay and `5/(2R)` angular update.
  - Code: `src/lib.rs:4034-4123`, `src/lib.rs:4258-4298`, `src/lib.rs:11665-11721`
  - Tests: `tests/advance_ball_state.rs:369-392`, `tests/non_ideal_ball_collisions.rs:1516-1705`
  - Sources: `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf:76-83`, `:130-142`, `:196-244`

- **TP B.2 rolling side-spin turn math is present as an estimator.** The estimator matches Dr. Dave's published examples: `2 mph, 8 ft -> 0.305 deg / 0.217 in`, and `5 mph, 3 ft -> 0.012 deg / 0.003811 in`.
  - Code: `src/lib.rs:11390-11448`, `src/lib.rs:11449-11464`
  - Tests: `tests/advance_ball_state.rs:606-688`
  - Source: `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf:244-450`

- **Elevated side-tip spin decomposition is directionally consistent with TP A.19.** The current strike model splits side offset into vertical-axis spin scaled by `cos(elevation)` and shot-direction massé spin scaled by `sin(elevation)`, matching the normalized-offset form of TP A.19's initial spin components.
  - Code: `src/lib.rs:3726-3763`, `src/lib.rs:3806-3855`
  - Tests: `tests/shot_strikes.rs:117-158`, `tests/scenario_examples.rs:47-146`
  - Source: `whitepapers/tp_a_19_masse_shot_aiming_method_and_curved_cue_ball_paths.pdf:54-98`, `:154-257`

## TODOs

### DONE P0 — Implement TP B.10 table-bounce tangential impulse and spin loss

**Problem.** Airborne table contact currently applies only vertical restitution. It preserves planar velocity and all angular velocity through contact. TP B.10 predicts each bounce changes tangential speed, total speed, spin, rebound angle, and next hop distance.

- Current code: `src/lib.rs:3882-3902` preserves planar velocity/angular velocity during flight; `src/lib.rs:3935-3961` computes only `rebound_vertical_speed` and clones planar velocity/angular velocity into the post-contact state.
- Existing tests only verify vertical bounce/settle behavior and do not assert horizontal speed or spin loss: `tests/n_ball_pockets.rs:175-265`.
- Source expectation: TP B.10 gives `v_t,i+1`, `v_i+1`, `omega_i+1`, `theta_i+1`, and `d_i+1` for each bounce and states spin/spin-ratio take hits at bounces, not while airborne: `whitepapers/tp_b_10_draw_shot_cue_elevation_effects.pdf:132-210`, `:240-319`, `:511-583`; see also `whitepapers/draw_shot_physics_part_iv_cue_elevation_effects.pdf:29-39`, `:53-59`.

**Target.** Add an airborne-table-contact resolver using TP B.10-style normal/tangential impulse:

- normal velocity after bounce: `v_n' = e_t * v_n`,
- tangential impulse from ball-cloth friction during contact,
- resulting tangential speed, spin, rebound angle, and hop distance,
- tests proving elevated draw/follow loses spin at each bounce and retains spin during airborne intervals.

**Done 2026-07-02.** `BallSetPhysicsSpec` now carries `AirborneTableContactConfig` with TP B.10 defaults (`e_t = 0.6`, `mu_s = 0.2`) plus a settle threshold. Initial elevated launch and later table contacts both route through the same tangential impulse resolver, reducing tangential speed/spin at contact while preserving ballistic airborne intervals.


### DONE P0 — Add 3D airborne ball-ball contact diagnostics

**Problem.** Airborne balls now schedule rail, jaw, and pocket interactions before their next table contact, but airborne ball-ball contact was not modeled. A jump/elevated cue ball could pass through an object ball before landing without either a 3D collision response or an explicit diagnostic event.

- Former gap: ball-ball predictions required both states to be `OnTable`, so airborne/on-table and airborne/airborne object-ball contacts were removed from the cache.
- Source expectation: jump/elevated-shot material explicitly warns that if the cue ball reaches the object ball while still bouncing/airborne, the cut angle changes and fouls are possible: `whitepapers/draw_shot_physics_part_iv_cue_elevation_effects.pdf:37-39`; `whitepapers/veps_gems_part_xv_the_jump_shot.pdf:53-59`, `:107-111`; `whitepapers/jump_shots_made_simple.pdf:49-50`.

**Done 2026-07-02.** `NBallSystemEvent::UnsupportedAirborneBallBallContact` now reports pre-landing 3D ball-ball contact instead of silently tunneling. `PocketAwareEventCache` tracks airborne contact candidates separately from executable on-table collisions, chooses them before later rails/jaws/pockets/table bounces, and DSL traces expose the diagnostic event by ball labels.

- Code: `src/lib.rs:1436-1537`, `src/lib.rs:1730-1808`, `src/lib.rs:1951-1954`, `src/lib.rs:2066-2094`, `src/lib.rs:2120-2133`, `src/lib.rs:2351-2361`, `src/dsl.rs:1031-1081`, `src/dsl.rs:1415-1419`, `src/dsl.rs:1459-1466`
- Tests: `tests/n_ball_pockets.rs:334-375`

### DONE P1 — Use TP B.2 rolling side-spin turn in actual motion, not only metadata

**Problem.** The TP B.2 turn estimator was correct as analysis data, but rolling motion integration still advanced in a straight line. Consumers of `advance_motion_on_table`, trace playback, rail prediction, and future ball-collision prediction did not follow the estimated curved path.

- Former straight-line rolling integrator: `src/lib.rs:4124-4171`.
- Estimator/test evidence: `src/lib.rs:11390-11448`, `tests/advance_ball_state.rs:622-649`.
- Source expectation: TP B.2 defines actual turn rate `Omega_t(v)` and integrates it into path angle/lateral error: `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf:263-365`, `:391-443`.

**Done 2026-07-02.** The rolling integrator now computes TP B.2 curved displacement during rolling advancement, decays side spin over the same interval, and preserves the published estimator as analysis metadata. Regression coverage exercises both the estimator and the actual rolling-path displacement.

### P1 — Add a real massé/swerve model surface, not only hand-tuned elevation examples

**Problem.** Current massé support is a qualitative elevated-side-spin path: hand-pick heading, speed, side/height, and elevation; the ball may hop; then TP A.4 bends it after landing. The code does not expose Coriolis/BAR massé aiming or validate final direction/curve magnitude against TP A.19.

- Current `Shot` only stores `cue_elevation`; no massé aim primitive: `src/lib.rs:2987-3050`.
- DSL supports `.elevation(...)` and `.jump(...)`, not `.masse(...)`: `src/dsl.rs:3388-3394`, `src/dsl.rs:3519-3530`; test expects `.masse(30deg)` to fail in `tests/dsl.rs:1269-1272`.
- Existing massé/swerve tests assert sign and qualitative bend only: `tests/shot_strikes.rs:117-158`, `tests/scenario_examples.rs:47-146`.
- Source expectation: TP A.19 and VEPS XVI frame massé with contact point `B`, aim point `A`, resting point `R`, and final direction parallel to `RA`; speed controls where along the path the curve completes: `whitepapers/tp_a_19_masse_shot_aiming_method_and_curved_cue_ball_paths.pdf:114-154`, `:254-257`, `:296-415`; `whitepapers/veps_gems_part_xvi_the_masse_shot.pdf:21-25`, `:71-86`, `:100-106`.

**Target.** Add source-calibrated massé/swerve APIs and tests:

- compute final Coriolis/BAR direction from tip offset/elevation,
- derive or validate cue setup from desired final direction / aim point,
- distinguish continuous on-cloth swerve from jump-then-curve behavior,
- add magnitude regression examples from TP A.19, not just sign checks.

### P1 — Decide how ordinary English shots get realistic cue elevation/swerve

**Problem.** `Shot::new` defaults to `0°` cue elevation, so ordinary side-English shots have squirt but no cue-elevation-driven swerve unless `.elevation(...)` is explicitly specified. Whitepaper material says a truly level cue is usually unrealistic and swerve is part of effective squirt under normal play.

- Current default: `src/lib.rs:2995-3010`.
- DSL only applies elevation if `.elevation(...)` or `.jump(...)` appears: `src/dsl.rs:2711-2804`.
- Source expectation: TP A.3 gives a physical rail-clearance elevation example of `1.384°`: `whitepapers/tp_a_3_minimum_cue_elevation_required_for_a_head_spot_to_foot_spot_center_ball_hit_shot.pdf:63-85`; squirt/swerve article says perfectly level/no-friction shots have no swerve but are not realistic, and speed/elevation/cloth determine effective squirt: `whitepapers/squirt_part_iii_follow_draw_squirt_and_swerve.pdf:57-89`, `:129-133`.

**Target.** Pick an explicit policy:

- keep `0°` as low-level API default but make scenario DSL require/derive realistic elevation for side-English examples, or
- add a table/bridge/rail-clearance cue-elevation default/config with opt-out for idealized tests.

### DONE P2 — Source-calibrate vertical launch and rebound coefficients

**Problem.** Initial elevated launch and later table rebounds use hard-coded coefficients that are not the same as TP B.10's typical `e_t = 0.6` table restitution and are not coupled to tangential impulse.

- Current constants: `CUE_ELEVATION_TABLE_REBOUND_COEFFICIENT = 0.58`, `AIRBORNE_TABLE_BOUNCE_RESTITUTION = 0.42`, `MIN_AIRBORNE_TABLE_REBOUND_SPEED_INCHES_PER_SECOND = 4.0`: `src/lib.rs:3707-3710`.
- Initial launch uses `v * sin(elevation) * 0.58`: `src/lib.rs:3818-3839`.
- Later table bounce uses `incoming_vertical_speed * 0.42`: `src/lib.rs:3935-3961`.
- Source expectation: TP B.10 uses a table coefficient of restitution and impact equations, not two independent constants: `whitepapers/tp_b_10_draw_shot_cue_elevation_effects.pdf:48-75`, `:132-210`.

**Target.** Replace the constants with named, source-calibrated config fields and tests covering hop height/time and repeated-bounce damping.

**Done 2026-07-02.** Initial elevated launch and later table bounces now share `AirborneTableContactConfig` coefficients instead of independent hard-coded values. The default normal restitution is TP B.10's `0.6`; the default sliding friction coefficient is TP B.10's `0.2`; the existing 4 ips minimum rebound speed remains named as a numerical settle threshold.

### DONE P2 — Update stale DSL shot mini-spec for elevation/jump/massé state

**Problem.** `DSL_SHOT_MINI_SPEC.md` still says cue elevation, jump, massé, and cue-elevation-driven swerve are not represented, while current parser/tests implement elevation and jump and intentionally omit `.masse(...)`.

- Stale spec: `DSL_SHOT_MINI_SPEC.md:18-20`, `DSL_SHOT_MINI_SPEC.md:188-190`.
- Current parser/tests: `src/dsl.rs:3388-3394`, `src/dsl.rs:3519-3530`, `tests/dsl.rs:1220-1272`.

**Target.** Update the spec to say:

- `.elevation(deg)` exists,
- `.jump([deg])` exists and defaults to `45°`,
- `.masse(...)` does not yet exist,
- current massé/swerve behavior is a qualitative elevated-side-spin model with the limitations above.

**Done 2026-07-02.** `DSL_SHOT_MINI_SPEC.md` now documents supported `.elevation(angle)`, `.jump()`/`.jump(angle)`, and the intentional absence of declarative `.masse(...)` sugar while keeping current elevated-side-spin behavior described as a lower-level physics model.

### P3 — Make trace/rendered paths sample within-phase curvature

**Problem.** Some trace/path helpers store event vertices and can under-display the actual within-phase TP A.4 curvature. Scenario playback samples can show curvature, but path polylines and route summaries may miss it.

- Current path helpers can reduce to segment endpoints: `src/dsl.rs:1211-1264`; related comments noted by audit around `src/lib.rs:2236-2243` and `src/lib.rs:10632-10639`.
- Scenario tests manually sample `advance_motion_on_table` to observe lateral curve: `tests/scenario_examples.rs:113-143`.
- Source expectation: TP A.4/TP A.19 curved paths are continuous parabolic sliding trajectories, not just event vertices: `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf:264-280`; `whitepapers/tp_a_19_masse_shot_aiming_method_and_curved_cue_ball_paths.pdf:226-252`.

**Target.** Use phase-aware sampling for rendered/diagnostic trace paths whenever a segment is sliding with nonzero cloth-contact slip or rolling with TP B.2 side-spin curvature enabled.

### P3 — Decide whether Kim object/table friction should be default physics

**Problem.** Kim-style object/table static friction for topspin ball-ball impacts is implemented and tested only as opt-in. Kim argues the effect can be significant, but human-tuned defaults leave the coefficient at zero.

- Current opt-in config/default-off: `src/lib.rs:1215-1270`.
- Implementation/test: `src/lib.rs:11094-11144`, `tests/non_ideal_ball_collisions.rs:1180-1240`.
- Source: `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf:80-150`, `:700-790`.

**Target.** Decide whether source-accurate default physics should include a nonzero object/table static-friction coefficient, or document it clearly as an optional first-order extension.
