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

**Done 2026-07-02; corrected 2026-07-10.** The rolling integrator computes TP B.2 curved displacement during rolling advancement, decays side spin over the same interval, and preserves the published estimator as analysis metadata. The 2026-07-10 correction applies curvature whenever finite-speed rolling and side spin coexist; it no longer suppresses the entire curve merely because side spin is predicted to outlast translation.

- The reportable TP B.2 interval ends when side spin stops or the configured positive linear-speed cutoff is reached, whichever occurs first. Residual side spin can outlast translation into `MotionPhase::Spinning`.
- Regression coverage exercises low-spin and long-lived-spin trajectories, continuity across the former lifetime gate, mirrored English, the finite speed cutoff, estimator metadata, and the actual rolling-path displacement.


### DONE P1 — Use canonical curved rolling trajectories for continuous events

**Done 2026-07-10.** Ball-ball, rail, and fixed-circle jaw prediction now evaluate the same TP B.2 side-spin path as rolling advancement. A curved rolling interval uses chronological adaptive subdivision with a speed-based exclusion bound and bisection only after it has bracketed an entering contact; genuinely quadratic paths retain their polynomial root solvers.

- The curved path is applied to both participants of ball-ball timing, so a curved ball can no longer tunnel through a stationary object ball or receive a straight-path ghost impact. Rail and jaw paths use signed plane and radial gaps, respectively, with the same canonical raw advancement.
- Regression coverage includes deterministic rightward-curve ball-ball, right-rail, and center-right jaw contacts that the former fixed-direction surrogate missed; the companion left-side ball, rail, and jaw curve-away cases remain event-free. The N-ball and pocket-aware schedulers receive the same accepted predictions.
- Source expectation: TP B.2's finite rolling turn is integrated in `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf:275-417`; the event solver preserves analytic polynomial roots only where the trajectory is actually quadratic.

### DONE P2 — Reject overlapping N-ball states at public boundaries

**Done 2026-07-10.** Aggregate N-ball prediction, advance, resolution, and simulation APIs now return `NBallGeometryError` for material on-table penetration. The validator permits exact frozen contacts and only recovers a sub-microinch construction residue with midpoint-preserving, velocity-free canonical-pair projection.

- `NBallGeometryError::OverlappingOnTableBalls` reports original pair indices, measured and required center distances, positive penetration, and the fixed `1e-6 in` recovery policy. Query APIs normalize an internal snapshot; state-producing APIs return the recovered nonpenetrating snapshot or fail before candidate scheduling or impulse resolution.
- System and DSL construction paths apply the same invariant. `DslBuildError::InvalidNBallGeometry` preserves both named ball identities and the core indexed/unit-bearing error.
- Regression coverage rejects stationary, closing, and indexed system overlaps; verifies inclusive tolerance recovery, exact frozen-contact preservation, and DSL failure before a cue strike. Existing rack and break fixtures remain valid.
- Source expectation: `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf:1-3` defines impulses at contact as velocity-level operations, while TP B.29's frozen construction uses `D=2R`; neither supports treating material rigid penetration as a resolved terminal state.

### DONE P1 — Add a real massé/swerve model surface, not only hand-tuned elevation examples

**Done 2026-07-06; corrected 2026-07-10.** `Shot::masse_aim_estimate`, `coriolis_masse_curve_angle_degrees`, `coriolis_masse_final_heading`, `masse_curve_mode_for_launch`, and `validate_coriolis_masse_bar_relationship` expose a TP A.19 / Coriolis-BAR aiming surface instead of only hand-tuned heading/elevation examples.

- The helper computes the final post-curve cue-ball direction from normalized side offset `a`, TP A.19's below-positive height `b`, and cue elevation `phi` using `atan2(a sin(phi), cos(phi) - b)`. The public `CueTipContact::height_offset` is above-positive, so the helper explicitly converts `b = -height_offset`.
- `validate_coriolis_masse_bar_relationship` validates the source-style `B`/`A`/`R` relation: cue-ball point `B`, aim point `A` on the cue vertical plane, and final-reference point `R` with the final direction parallel to `RA`.
- `MasseCurveMode` reports whether the current engine will treat the shot as continuous on-cloth swerve or jump-then-curve after table contact.
- Tests in `tests/shot_strikes.rs` cover the TP A.19 `a = 0.25R`, `b = 0.25R`, `phi = 75°` source relation through API `height_offset = -0.25R`; assert the separate API `height_offset = +0.25R` result; validate a matching `B`/`A`/`R` setup; reject its old sign-reversed final direction; and assert the current nonzero-speed elevated shot is classified as jump-then-curve.

**Remaining limitation.** This is an explicit calibrated aiming helper, not a full speed/path solve: TP A.19 says speed controls where the curve completes, and the current API still does not choose speed or solve obstacle clearance automatically. DSL `.masse(...)` sugar remains intentionally absent; callers use `.tip(...)` plus `.elevation(...)` and/or the Rust helper.

### DONE P1 — Decide how ordinary English shots get realistic cue elevation/swerve

**Done 2026-07-06.** Policy: `Shot::new` remains the low-level idealized API default at `0°`, but
the scenario DSL now derives a conservative `1.384°` TP A.3 rail-clearance elevation for ordinary
side-English shots whenever `.elevation(...)`/`.jump(...)` is omitted. Center-ball shots remain
level by default, and `.elevation(0deg)` is the explicit opt-out for idealized side-English tests.

- Code: `src/lib.rs:3228-3234`, `src/dsl.rs:42-47`, `src/dsl.rs:2727-2846`
- Tests: `tests/dsl.rs:1276-1328`
- Docs: `DSL_SHOT_MINI_SPEC.md:190-194`, `examples/scenarios/README.md:37-39`

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

### DONE P3 — Make trace/rendered paths sample within-phase curvature

**Done 2026-07-06.** Rendered trace paths use phase-aware sampling instead of endpoint-only
event vertices. `BallPath::sampled_points` advances within each traced segment with the motion
solver, `GameState::add_rendered_ball_path_styled` samples each rendered segment through the same
phase-aware path, and `ScenarioBallTrace::sampled_points`/rendering route on-table traces through
that sampler while retaining `projected_points` as the explicit event-vertex diagnostic view.

- Code evidence: `src/lib.rs:2480-2538`, `src/lib.rs:15064-15103`, `src/lib.rs:15191-15223`, `src/dsl.rs:950-968`, `src/dsl.rs:1230-1327`.
- Test evidence: `tests/dsl.rs:532-632` asserts a rolling side-spin trace inserts within-segment samples, deviates from endpoint-only segment chords, and renders differently from an endpoint-only baseline.

### DONE P3 — Decide whether Kim object/table friction should be default physics

**Done 2026-07-06.** Policy is explicit opt-in: `BallBallCollisionConfig::ideal()`,
`Default`, and `human_tuned()` keep `object_table_static_friction_coefficient = 0` because
Kim's measured object/table static friction (`μ_s ≈ 0.2..0.4`) is a significant first-order
correction, but the current on-table collision state still omits the paired vertical hop.

- Source evidence: Kim notes object/table static friction is large and measured around `0.2 < μ_s < 0.4`, and derives the topspin-only first-order normal-speed/object-spin correction in Eqs. (52)-(54): `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf:80-150`, `:700-790`.
- Code evidence: `src/lib.rs:1241-1249` documents default-off as intentional, `src/lib.rs:1259-1278` keeps `new()` and `new_with_friction_model()` default-off, and `src/lib.rs:1301-1314` provides explicit coefficient and Kim-named opt-in config helpers.
- Test evidence: `tests/non_ideal_ball_collisions.rs:1179-1247` asserts `Default`/`ideal()`/`human_tuned()` are default-off and that human-tuned collision diagnostics apply no Kim normal correction; `tests/non_ideal_ball_collisions.rs:1249-1308` asserts the named Kim opt-in produces the expected first-order recoil, object speed reduction, and object-spin scale.
