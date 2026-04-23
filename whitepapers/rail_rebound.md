# Rail rebound notes

Working notes for the current ball-rail / cushion model, with emphasis on TP 7.3 style
vertical-plane-spin realism.

## Primary local reference

- `whitepapers/tp_7_3_ball_rail_interaction_and_the_effects_on_vertical_plane_spin.pdf`

Useful extracted formulas / worked-example values also appear in:

- `agent_knowledge/whitepapers_formula_candidates.txt`
- `/tmp/billiards-whitepaper-text/tp_7_3_ball_rail_interaction_and_the_effects_on_vertical_plane_spin.txt`

## TP 7.3 summary

TP 7.3 models a rail impact with:

- rebound normal speed reduction `v' = e v`
- frictional angular impulse term `μ F' R`
- an additional cushion-height / geometry term `F' a`

The worked example in the note uses representative values:

- `e = 0.7`
- `μ = 0.17`
- `a = 0.08 R`

Qualitatively, the note says:

- a **rolling** entry rebounds with post-impact vertical-plane spin **close to zero**;
- a strong **overspin / follow** entry can leave the rail with **reverse** spin relative to the
  new travel direction;
- a **stun** entry can still pick up some forward roll because of the geometric `a` term even when
  the friction term is absent;
- a **draw** entry can leave with much less draw than it brought in, sometimes close to stun.

## Current code mapping

### Directly represented in `src/lib.rs`

- `RailCollisionConfig::normal_restitution` models the TP 7.3 `e` term.
- `RailCollisionConfig::tangential_friction_coefficient` models the tangential rail-friction term
  `μ`.
- `RailCollisionConfig::impact_cloth_friction_coefficient` now exposes the reduced simultaneous
  rail+cloth slip solve's impact-time cloth-friction term instead of hard-coding it.
- `RailCollisionConfig::effective_contact_height_ratio` now exposes the reduced TP 7.3-style
  geometric `a / R` term instead of hard-coding it.
- `spin_aware_ball_rail_collision_on_table(...)` now runs a reduced Mathavan-style impact solve in
  the local cushion frame, including:
  - along-rail slip from tangential velocity and `ωz`,
  - vertical slip from `ωx` / `ωy`, and
  - a configurable impact-time ball-cloth sliding-friction term during the rail contact interval.
- `tp73_geometric_vertical_plane_spin_delta(...)` remains the explicit TP 7.3-style geometric `a`
  contribution for vertical-plane-spin conversion.

### Intentional realism guards still layered on top

The reduced horizontal on-table model still keeps two small guardrails for ordinary exits:

- `rail_running_english_generation_scale(...)`
- `rail_rebound_horizontal_spin_blend(...)`
- `clamp_rail_rebound_horizontal_spin_to_slip_limit(...)`

These are not direct TP 7.3 or Mathavan formulas. They remain pragmatic controls because the state
model still does not include full cushion compression, vertical center-of-mass motion, or richer
post-rail contact history.

The rolling-settle controls are also now capped so they can pull a reverse-spin overspin rebound
back toward **stun**, but not all the way through stun into fresh forward roll; TP 7.3's slight-
overspin cases should still be able to leave the rail with reverse vertical-plane spin.

## Important current simplifications / limits

### 1. The effective `a` term is intentionally reduced

TP 7.3's worked example uses roughly `a = 0.08 R`.

The current default code path uses:

- `RailCollisionConfig::effective_contact_height_ratio = 0.04`

This is still intentionally smaller than the TP 7.3 worked-case `a ≈ 0.08R`, but it is no longer
as aggressively reduced as the earlier `0.02R` stopgap. The simultaneous rail+cloth solve now
carries more of the burden, while this geometric term restores some of the missing forward-roll
pickup for stun-like entries.

### 2. Cushion compliance is still only implicit

The current solver now includes simultaneous rail+cloth friction during impact, but it still does
not model explicit cushion compression / release. The main remaining cushion-compliance surrogate is
therefore still the fixed contact-height geometry:

- `THEORETICAL_CUSHION_CONTACT_HEIGHT_ABOVE_CENTER_RATIO = 2/5`
- `RailCollisionConfig::effective_contact_height_ratio = 0.04` by default

That is a useful reduced model, but it is not a full compliant cushion patch solve.

### 3. Rail-entry rolling states still get a small pragmatic nudge

Ordinary rolling entries without explicit side spin are still nudged toward more realistic exits by:

- suppressing some fresh running-english generation,
- blending horizontal spin somewhat back toward the post-rail rolling target, and
- clamping excessive outgoing cloth-slip mismatch.

This remains a pragmatic correction for the reduced horizontal state, not a paper-derived result.

### 4. The current state does not include airborne / vertical post-impact motion

The on-table rail state does not explicitly represent:

- post-impact vertical COM velocity,
- detailed cushion compression / release,
- speed-dependent penetration depth into the rail,
- separate static vs kinetic rail-friction regimes beyond the current limited no-slip clamp.

That means the current solver is still a **reduced horizontal slice** of the full cushion-impact
problem.

## What is already covered well enough

At a qualitative level, the current model now captures the most important local rail behaviors:

- simultaneous rail+cloth friction acts during impact rather than only after the fact;
- rolling entries rebound much closer to **stun** than a pure mirror rebound would;
- strong overspin / follow-style entries can leave with **reverse** vertical-plane spin;
- pure stun entries can pick up a small amount of forward vertical-plane roll;
- pure rolling rail rebounds generally leave the ball in a **sliding** phase, not rolling;
- explicit side spin and horizontal cloth-slip are kept from exploding after rail contact.

## What still looks missing or worth documenting better

### Missing / under-documented

1. **A single durable note mapping TP 7.3 to the code**
   - this file is intended to fill that gap.

2. **Canonical TP 7.3 / Mathavan regression tests**
   - especially the qualitative rolling / stun / overspin / draw entry cases.

3. **Explicit statement that some rail guards are heuristic, not paper-derived**
   - especially the rolling-entry running-english scale / blend / clamp helpers.

### Still-open physics questions

1. The fixed impact-time cloth-friction coefficient is now exposed via
   `RailCollisionConfig::impact_cloth_friction_coefficient`; it still needs measured calibration.
2. The effective `a` term is now exposed via
   `RailCollisionConfig::effective_contact_height_ratio`; it may still want a **speed-dependent**
   model rather than a fixed value.
3. Should cushion compliance / penetration depth vary with impact speed and rail?
4. Should the rail model expose an internal trace of:
   - tangential contact slip,
   - cloth-contact slip,
   - compression / restitution work,
   so scenario debugging can distinguish paper-backed response from guardrail clamps?

## Pointers into the code

- `src/lib.rs`
  - `tp73_geometric_vertical_plane_spin_delta(...)`
  - `spin_aware_ball_rail_collision_on_table(...)`
  - `rail_running_english_generation_scale(...)`
  - `rail_side_spin_retention_scale(...)`
  - `rail_rebound_horizontal_spin_blend(...)`
  - `clamp_rail_rebound_horizontal_spin_to_slip_limit(...)`
- `tests/rail_collisions.rs`
  - qualitative rail regression coverage for mirror / restitution / spin-aware behavior
