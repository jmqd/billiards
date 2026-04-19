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
- `RailCollisionConfig::tangential_friction_coefficient` models the tangential friction term `μ`.
- `spin_aware_ball_rail_collision_on_table(...)` uses the full rail-face contact-slip vector,
  combining:
  - along-rail slip from tangential velocity and `ωz`, and
  - vertical slip from `ωx` / `ωy`.
- `tp73_geometric_vertical_plane_spin_delta(...)` is the explicit TP 7.3-style geometric `a`
  contribution for vertical-plane-spin conversion.

### Intentional realism guards currently layered on top

The current reduced horizontal on-table model also adds guards to keep ordinary rail exits in
believable bounds:

- `rail_running_english_generation_scale(...)`
- `rail_side_spin_retention_scale(...)`
- `rail_rebound_horizontal_spin_blend(...)`
- `clamp_rail_rebound_horizontal_spin_to_slip_limit(...)`

These are not direct TP 7.3 formulas. They are pragmatic controls added because the state model does
not include full cushion compression, vertical center-of-mass motion, or richer post-rail contact
history.

## Important current simplifications / limits

### 1. The effective `a` term is intentionally reduced

TP 7.3's worked example uses roughly `a = 0.08 R`.

The current code uses:

- `TP73_EFFECTIVE_CONTACT_HEIGHT_RATIO = 0.02`

This is intentionally smaller to avoid double-counting cushion effects already partly captured by
other reduced-model terms.

Likely consequence:

- the model is less likely to over-flip ordinary rolling entries,
- but it probably also **under-predicts how much fresh forward vertical-plane roll a pure stun rail
  impact should gain** relative to the TP 7.3 worked example.

### 2. Cushion compliance is represented only as an effective torque lever

The code uses:

- `THEORETICAL_CUSHION_CONTACT_HEIGHT_ABOVE_CENTER_RATIO = 2/5`
- `CUSHION_COMPLIANCE_EFFECTIVE_TORQUE_RATIO = 0.65`

This is a reduced surrogate for a compliant cushion patch. It is useful, but it is not a full
cushion deformation model.

### 3. Rail-entry rolling states are deliberately damped

Ordinary rolling entries without explicit side spin are currently nudged toward more realistic exits
by suppressing some:

- fresh running-english generation,
- carried side-spin retention,
- post-rail horizontal-spin mismatch.

This is motivated by observed over-curve after rail contact, but it is not directly derived from TP
7.3.

### 4. The current state does not include airborne / vertical post-impact motion

The on-table rail state does not explicitly represent:

- post-impact vertical COM velocity,
- detailed cushion compression / release,
- speed-dependent penetration depth into the rail,
- separate static vs kinetic rail-friction regimes beyond the current limited no-slip clamp.

That means the current solver is still a **reduced horizontal slice** of the full cushion-impact
problem.

## What is already covered well enough

At a qualitative level, the current model does capture the most important TP 7.3 behaviors:

- rolling entries rebound much closer to **stun** than a pure mirror rebound would;
- strong overspin / follow-style entries can leave with **reverse** vertical-plane spin;
- pure rolling rail rebounds generally leave the ball in a **sliding** phase, not rolling;
- explicit side spin and horizontal cloth-slip are kept from exploding after rail contact.

## What still looks missing or worth documenting better

### Missing / under-documented

1. **A single durable note mapping TP 7.3 to the code**
   - this file is intended to fill that gap.

2. **Canonical TP 7.3 regression tests**
   - especially the qualitative rolling / stun / overspin / draw entry cases.

3. **Explicit statement that some rail guards are heuristic, not paper-derived**
   - especially the rolling-entry side-spin scrub / blend / clamp helpers.

### Still-open physics questions

1. Should the effective `a` term be made **speed-dependent** rather than fixed?
2. Should cushion compliance / penetration depth vary with impact speed and rail?
3. Should the rail model expose an internal trace of:
   - tangential contact slip,
   - vertical contact slip,
   - friction-limited vs no-slip regime,
   so scenario debugging can distinguish paper-backed response from guardrail clamps?
4. Do we want a stricter TP 7.3 calibration mode for isolated rail studies, separate from the more
   conservative whole-table realism defaults?

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
