# Rail/cushion impacts and pocket acceptance audit — 2026-04-24

## Scope and result

Report-only audit of the current rail/cushion impact, pocket-jaw, and pocket-capture model against the local literature. I made no source or test changes: the issues below are mostly model/calibration gaps, and the one clear rail-slip correctness issue needs a small design choice rather than an opportunistic patch.

Focused checks passed:

```text
cargo test --test rail_collisions --test rail_event_scheduling --test rail_event_execution --test n_ball_pockets --test table_and_pocket_geometry --test bank_paths
```

## Source evidence used

- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf`
  - Models a 3-D ball/cushion impact with simultaneous ball-cloth contact.
  - Uses cushion contact height `h = 7R/5`, so the contact normal geometry gives `sin(theta) = 2/5`.
  - Uses impulse as the integration variable; compression ends when the cushion-normal relative velocity reaches zero; restitution is energetic, with `e_e^2` as work restitution.
  - Reports fitted snooker/pool-like values `e_e = 0.98`, ball-cushion sliding friction `mu_w = 0.14`, and ball-cloth sliding friction around `mu_s = 0.212`; rigid-cushion validity is reported for normal incident speed below about `2.5 m/s`.
  - Explicitly says that when slip speed at cushion/table contact becomes zero, the friction impulses at that contact become zero in that rolling/no-slip branch; it also shows slip direction changes during impact.
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf`
  - Confirms the same modeling pattern for billiards impacts: simultaneous 3-D frictional contact, table-contact terms during impact, no general closed-form solution, numerical impulse integration.
- `whitepapers/tp_7_3_ball_rail_interaction_and_the_effects_on_vertical_plane_spin.pdf`
  - Provides a simplified vertical-plane rail model: `v' = e v`, `F' = m(1+e)v`, and `mu F' R + F' a = I_o (omega' + omega)`.
  - Worked values: `e = 0.7`, `mu = 0.17`, `a = 0.08R`.
  - Qualitative cases match current test intent: rolling entry rebounds close to stun, overspin can leave with reverse vertical-plane spin, stun can pick up forward roll from geometry, and draw can be reduced toward stun.
- `whitepapers/the_art_of_billiards_play.html`
  - General collision mechanics: conservation of linear/angular momentum, Coulomb friction, rail collision as body 2 stationary, and an explicit adherence/no-slip branch when the friction-limited slip solution would cross through zero.
  - Cloth motion section gives sliding-to-rolling transition formulas and supports post-rail sliding when the no-slip condition is broken.
- `whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf`
  - Side-pocket slow-shot geometry uses `R = 1.125`, `p = 5.0625`, `alpha = 14 deg`, `Rhole = 3`, `b = 0.1875` and derives `sleft(theta)`, `sright(theta)`, target offset, and `theta_max = 68.292 deg`.
- `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf`
  - Side-pocket fast-shot model assumes rebound/rattle to at least the pocket-hole rim center after three wall rattles; derives `theta_max = 50.688 deg` plus target size and offset functions.
- `whitepapers/tp_3_6_effective_target_sizes_for_slow_shots_into_a_corner_pocket_at_different_angles.pdf`
  - Corner-pocket slow-shot model uses `p = 4.5875`, `alpha = 7 deg`, `Rhole = 2.75`, `b = 1.125` and derives multi-wall/rail-rattle target size and offset functions over the corner approach range.
- `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf`
  - Corner-pocket fast-shot model uses `p = 4.58125`, `alpha = 7 deg`, `Rhole = 2.75`, `b = 1.125`; derives `theta_max = 59.841 deg` and target size/offset functions.

I used `agent_knowledge/whitepapers_index.jsonl`, `agent_knowledge/whitepapers_formula_candidates.txt`, and `agent_knowledge/whitepapers_corpus.txt` for lookup/extraction; no generated files were edited.

## Code and tests audited

Main code paths in `src/lib.rs`:

- Rail scheduling: `compute_next_ball_rail_impact_on_table`, `rail_collision_gap_during_current_phase_raw`, `refine_ball_rail_collision_time_during_current_phase_raw`.
- Pocket geometry/capture: `pocket_center_in_inches`, `pocket_mouth_width_in_inches`, `pocket_jaw_reference_point_in_inches`, `pocket_jaw_geometry_in_inches`, `pocket_capture_radius_in_inches`, `pocket_entry_axis`, `pocket_entry_angle_degrees_raw`, `pocket_capture_max_entry_angle_degrees`, `pocket_acceptance_gap_raw`, `compute_next_ball_pocket_capture_on_table`.
- Jaw scheduling/response: `compute_next_ball_jaw_impact_on_table`, `pocket_jaw_gap_during_current_phase_raw`, `pocket_jaw_collision_basis`, `collide_ball_jaw_on_table_with_radius_and_profile`, `should_capture_after_jaw_impact`, `side_pocket_post_jaw_path_reaches_centerline_inside_capture_region`.
- Rail response: `cushion_collision_basis_from_normal`, `rail_collision_basis`, `restitution_aware_ball_cushion_collision_velocity_from_basis`, `solve_rail_impact_compression_phase`, `solve_rail_impact_restitution_phase`, `solve_spin_aware_rail_impact_in_frame`, `tp73_geometric_vertical_plane_spin_delta`, `spin_aware_ball_cushion_collision_on_table_from_basis`, `collide_ball_rail_*`.
- N-ball ordering/execution: `compute_next_n_ball_system_event_with_rails_and_pockets_on_table`, `prefer_explicit_jaw_over_nearby_capture`, `resolve_n_ball_system_event_with_physics_and_pockets_on_table`.
- Table/pocket specs: `TableSpec::brunswick_gc4_*`, `Pocket::aiming_center`, `PocketShapeSpec`.

Focused tests reviewed:

- `tests/rail_collisions.rs`
- `tests/rail_event_scheduling.rs`
- `tests/rail_event_execution.rs`
- `tests/n_ball_pockets.rs`
- `tests/table_and_pocket_geometry.rs`
- `tests/bank_paths.rs`

## Confirmed bugs / inaccuracies, prioritized

### 1. High: rail impact lacks a no-slip/adherence branch at cushion/table contacts

`rail_impact_frame_slip_angles(...)` returns only angles, and `advance_rail_impact_frame_by_impulse_step(...)` applies kinetic Coulomb terms from `sin(angle)`/`cos(angle)` every step. If either contact's slip speed is exactly zero or crosses through zero, `atan2(0, 0)` effectively selects an arbitrary slip direction rather than switching to the rolling/no-slip branch.

That conflicts with both local references:

- Mathavan cushion paper: when cushion slip `s = 0`, the cushion friction impulses are zero in that branch; when table slip `s' = 0`, table friction impulses are zero.
- Petit: if the friction-limited solution would overrun no-slip (`u < 0`), use an adherence/no-slip collision branch rather than continuing kinetic friction through zero.

Risk: geared/no-slip rail-contact states and near-zero-slip states can receive spurious tangential impulses or spin changes, and the response can be discontinuous around the no-slip crossing. Current tests cover qualitative friction/spin behavior but do not pin an exact zero-slip or slip-reversal case.

Suggested fix: carry contact slip speeds as well as angles. For a minimal first pass, zero the kinetic friction term at a contact when its slip speed is below epsilon; better, add a static/adherence branch that clamps impulse to the no-slip condition when the friction limit allows it. Add regression tests for cushion-zero-slip and table-zero-slip entries.

### 2. Medium-high: fast corner pocket acceptance is looser than TP 3.8

`CORNER_POCKET_CONSERVATIVE_MAX_ENTRY_ANGLE_DEGREES` is `61.5`, while TP 3.8's fast corner proof gives `theta_max = 59.841 deg` for the stated corner geometry. Larger accepted angle is less conservative, so the constant name and behavior do not match the fast-shot reference.

Risk: a fast corner approach that should be outside TP 3.8's effective target envelope can pass the current angle gate if it also enters the circular capture disk.

Suggested fix: at minimum cap fast corner entries at `59.841 deg` and add a test analogous to `a_fast_side_pocket_entry_beyond_the_effective_target_angle_is_rejected`. Better: make corner acceptance speed-dependent and derive it from TP 3.6/3.8 target-size data rather than one constant.

### 3. Medium: pocket acceptance is a disk-plus-angle gate, not the effective-target model in TP 3.5–3.8

`compute_next_ball_pocket_capture_on_table(...)` accepts when the ball crosses a circular capture radius around `Pocket::aiming_center()` and the velocity lies within a pocket-type angle threshold. The TP sources define pocketability by a direction-dependent effective target interval:

- `sleft(theta)` / `sright(theta)` target sizes,
- target-center offset,
- mouth width `p`, wall angle `alpha`, hole radius `Rhole`, shelf depth `b`,
- different slow/fast and side/corner rattle assumptions.

The current model ignores the lateral target offset and most of the wall/shelf/rattle geometry. This is acceptable as a coarse first pass, but it is not a faithful implementation of TP 3.5–3.8.

Suggested fix: replace or augment `pocket_acceptance_gap_raw(...)` with a signed gap to the TP effective target interval for the relevant pocket type and speed class. A tabulated/interpolated `sleft/sright/offset` curve would be much easier to validate than re-solving every Mathcad-style root expression at runtime.

### 4. Medium: pocket-jaw/wall response is much simpler than the cited pocket geometry

`pocket_jaw_reference_point_in_inches(...)` creates point/rounded jaw noses at mouth endpoints, and `collide_ball_jaw_on_table_with_radius_and_profile(...)` reuses a rail-cushion collision model at the local nose normal. The TP pocket papers model contact with pocket points, inside walls, shelf depth, hole rim, and multi-rattle paths. The current `side_pocket_post_jaw_path_reaches_centerline_inside_capture_region(...)` is a useful heuristic, but it is not the same as the explicit wall/rattle acceptance model.

Suggested fix: introduce explicit pocket-wall segments with pocket-specific restitution/friction and use TP wall/rattle paths for acceptance/rejection after jaw impacts. At minimum, document jaw-nose response as heuristic and add tests for fast corner/side jaw rattles that should reject.

### 5. Medium: rail calibration mixes different references without measured table fitting

`RailCollisionConfig::human_tuned()` defaults are currently `normal_restitution = 0.70`, `tangential_friction_coefficient = 0.17`, `impact_cloth_friction_coefficient = 0.20`, and `effective_contact_height_ratio = 0.04R`.

Evidence:

- TP 7.3 worked example uses `e = 0.7`, `mu = 0.17`, `a = 0.08R`.
- Mathavan cushion paper fitted `e_e = 0.98`, `mu_w = 0.14`, and `mu_s ~= 0.212`, with a rigid-cushion validity limit near `2.5 m/s` normal incident speed.
- Current docs note that `a/R` is intentionally reduced and that guardrails are heuristic.

This is not a bug, but the default profile should be treated as a qualitative/human-tuned profile, not a paper-calibrated physical profile. Suggested fix: expose named profiles such as `tp73_example`, `mathavan_snooker_low_speed`, and `human_tuned`, and add calibration tests/plots in SI or clearly converted inch units.

## Likely calibration gaps / model limits

- Rail restitution/friction should probably vary with normal speed, rail/cushion condition, and pocket/jaw material. Mathavan explicitly warns the rigid-cushion assumption breaks down above about `2.5 m/s` normal speed.
- The spin-aware rail solve is still a reduced horizontal slice: no explicit vertical COM velocity, cushion deformation/penetration, contact patch, or speed-dependent contact height.
- Guardrails in `rail_running_english_generation_scale(...)`, `rail_rebound_horizontal_spin_blend(...)`, and `clamp_rail_rebound_horizontal_spin_to_slip_limit(...)` are pragmatic, not directly paper-derived.
- Rail/jaw/pocket event finders use fixed scan counts and sign-change bracketing. They are fine for ordinary crossings, but exact tangencies/grazes can be missed unless a sample lands on the zero.
- Default pocket dimensions (`4.5 in` corner, `5.0 in` side, `1.4 in` depth) differ from the TP proof parameters (`4.58125/4.5875 in` corner, `5.0625 in` side, specified hole radii and shelf depths). This may be intentional GC4 modeling, but it prevents one-to-one comparison with TP figures.
- `Pocket::aiming_center()` uses fixed centers/inward corner offsets; TP target centers are angle-dependent through `offset(theta)`.

## False alarms / things that look acceptable

- `THEORETICAL_CUSHION_CONTACT_HEIGHT_ABOVE_CENTER_RATIO = 2/5` matches Mathavan's `h = 7R/5` cushion-contact geometry.
- `RestitutionOnly` and `Mirror` rail models are valid limiting/simple models as long as callers do not treat them as the spin/friction literature model.
- A rolling ball leaving a mirror rail impact in `Sliding` phase is physically reasonable: reversing the translational normal component breaks the pre-impact no-slip relation.
- The side-pocket angle constants `68.292` and `50.688` match TP 3.5 and TP 3.7 maxima.
- Phase-limited single-event helpers are acceptable in the full event scheduler because motion transitions are scheduled and the system is re-evaluated after each transition.
- The nearby jaw-over-capture preference is a reasonable safety valve for the current coarse capture gate.

## Concrete suggested code/test changes

1. Add no-slip/adherence handling to the rail impact solver.
   - Return cushion/table slip speeds from `rail_impact_frame_slip_angles(...)`.
   - Suppress kinetic friction at zero slip as a minimal patch, or implement a static branch that solves for the impulse needed to preserve no-slip subject to `|J_t| <= mu J_n`.
   - Tests: zero cushion-slip entry should not gain arbitrary running english from `atan2(0,0)`; zero table-slip branch should not apply kinetic cloth friction.
2. Tighten fast corner acceptance.
   - Change the fast corner cap to TP 3.8's `59.841 deg` or introduce a speed interpolation between TP 3.6/3.8 corner data.
   - Test: a fast `~60.5 deg` corner entry is rejected.
3. Add TP-derived pocket-target tests before replacing the coarse gate.
   - Side slow: near `68.292 deg` boundary.
   - Side fast: near `50.688 deg` boundary.
   - Corner fast: near `59.841 deg` boundary.
   - Offset cases where the same angle is accepted/rejected depending on lateral target-center offset.
4. Add an optional diagnostic trace for spin-aware rail impact.
   - Compression/restitution impulse count, slip speeds at both contacts, zero-slip/adherence branch hits, work at compression/restitution, and whether guardrails clamped the result.
5. Split default rail profiles by intent.
   - Keep `human_tuned()` for gameplay feel.
   - Add explicit reference profiles for TP 7.3 and Mathavan-style low-speed cushion data to avoid accidental claims of physical calibration.

## Manual self-review

- Re-read this report after writing for source-path coverage, code-path coverage, required check command, and generated-file safety.
- No files under `agent_knowledge/` were edited.
- No source/test behavior was changed in this audit.
