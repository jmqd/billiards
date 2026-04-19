# Physics Audit 2026-04-14

This note captures the post-gallery audit for the remaining realism issues reported in:

- `examples/scenarios/double_rail_kick_side_pocket.billiards`
- `examples/scenarios/mini_break_cluster.billiards`

It also records why a hive swarm was not used for this run: the local `hive` runtime currently fails at `hive up` because the Docker daemon is not running, so the investigation was performed manually in a clean detached worktree at current `HEAD`.

## References consulted

Primary local references:

- `whitepapers/art_of_billiards_play_files/bil_praa.html`
  - §7.1 collision / cushion friction formulation (`WCa`, `(C10)`, `(C11)`, `(C13)`)
  - §§7.3–7.5 on cloth sliding / rolling / spin decay
- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf`
- `whitepapers/tp_a_8_the_effects_of_english_on_the_30_degree_rule.pdf`
- `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf`
- `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf`
- `whitepapers/tp_7_3_ball_rail_interaction_and_the_effects_on_vertical_plane_spin.pdf`
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf`
- `whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf`
- `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf`
- `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf`
- `agent_knowledge/whitepapers_formula_candidates.txt`

## Confirmed findings

### 1. The cloth side-spin curve heuristic was too strong

Code path:

- `src/lib.rs`
  - `side_spin_curve_from_speed_window(...)`
  - `sliding_side_spin_curve_during(...)`
  - `rolling_side_spin_curve_during(...)`

The previous heuristic effectively mapped:

- `slip_ratio = R * ωz / v`
- directly into radians of heading change over the phase window

via:

```rust
(slip_ratio * duration_fraction).to_degrees()
```

That is much too aggressive for believable billiards cloth-turn. The local whitepapers support that residual spin can bend the path, but they repeatedly frame the effect as a **small** additional angle (`Δθ`) rather than tens of degrees of immediate curve:

- TP A.24 explicitly describes the trajectory curve as a small amount `Δθ`
- TP B.2's ball-turn discussion also points toward small turn angles from cloth effects

### 2. The double-rail example's unrealistic look was dominated by cloth-turn, not by missing trace data

Instrumenting the current shot showed:

- cue starts with `ωz = 0`
- the first right-rail impact seeds substantial running spin
- the old cloth-turn heuristic then amplified that into a very large visible bend

This means the cue-ball path *was* being bent by real simulated states, but the path bend was exaggerated mainly by the cloth-turn mapping from spin to heading change.

### 3. The break-shot trace system was not dropping balls

Trace diagnostics showed all seven balls in `mini_break_cluster` already had non-empty traces/segments under the checked-in code path. The user-visible issue was that only a few balls moved **very far** in the old symmetric/frozen cluster setup.

So the problem was not:

- missing trace reconstruction
- missing rendered overlays
- or a CLI/preset seam silently disabling tracing

### 4. The old mini-break geometry was too symmetric for a convincing gallery shot

The old cluster did technically propagate through the solver, but several balls only moved fractions of an inch. That made the example look broken even though events/traces existed.

## Candidate issues still worth future investigation

### Likely / medium priority

1. **Rail-induced z-spin still needs measured calibration**
   - The current `SpinAware` rail model is now more conservative than the earlier direct tangential-slip -> `ωz` seed.
   - The follow-up pass added:
     - a reduced TP 7.3-style contact-height term,
     - a smaller effective torque lever for compliant cushion contact, and
     - extra damping for ordinary rolling / no-english entries.
   - Even with those improvements, the simplified on-table model still does not solve the full simultaneous rail+cloth contact problem, so a future audit should compare measured post-rail running spin against TP 7.3 / cushion-impact examples more directly.

2. **Dense simultaneous multi-ball contact remains approximate**
   - `src/lib.rs` still explicitly documents deterministic tie-breaking instead of true simultaneous multi-contact resolution.
   - For rack/break-like frozen clusters, this can bias how momentum distributes through a contact graph.
   - The loosened mini-break example avoids overstating solver quality in that regime.

3. **Scenario gallery should prefer staggered or slightly loose contact chains over perfectly frozen cluster demonstrations**
   - The current event model is strongest for event-driven chains, not full rigid multi-contact rack solves.

### Lower priority / likely no immediate issue

4. **CLI / preferred simulation preset plumbing**
   - Re-audit during this run found no remaining evidence that the checked-in example path was silently falling back to ideal/legacy physics.
   - The preferred named-simulation path added earlier appears to be the correct seam.

5. **Trace reconstruction for pocketed balls / multi-ball traces**
   - The current trace layer already preserves multi-ball segments and extends pocketed balls into the pocket center.
   - No additional bug was found here during this audit.

## Practical conclusion

For the current reported regressions, the most justified changes were:

1. reduce the cloth side-spin curve heuristic to a conservative magnitude
2. make the rail model more conservative with:
   - a reduced TP 7.3-style contact-height term,
   - a compliant effective torque lever, and
   - extra damping for fresh running english from ordinary rolling entries
3. add a first-pass jaw-aware pocket acceptance gate based on the local effective-target-size notes
4. retune the double-rail and mini-break gallery examples for the calmer physics
5. add regression checks around the updated gallery expectations

A future follow-up can revisit measured rail-spin calibration, explicit jaw/rattle collision handling, and simultaneous-contact handling once hive/subagents are available or Docker is running.
