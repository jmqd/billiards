# Correct the Massé Height Sign at the TP A.19 Convention Boundary

**Date:** 2026-07-10  
**Severity / priority:** High / P1

## Problem statement

`CueTipContact` exposes a public, above-positive normalized height coordinate, but the Coriolis/BAR massé aiming helper currently substitutes that value directly for TP A.19's opposite, below-positive `b/R` coordinate. The resulting denominator has the wrong sign for every nonzero tip height.

The correction must happen only at the aiming-model convention boundary. The struck-state spin decomposition already converts above-positive height correctly and must not be inverted as part of this change.

## Observed current behavior and impact

### Public convention

`CueTipContact` documents and enforces cue-local ball-radius coordinates in `src/lib.rs:3183-3227`:

- `side_offset > 0` is to the striker's right;
- `height_offset > 0` is above center (follow/topspin);
- `height_offset < 0` is below center (draw).

The contact type, constructor, accessors, and their public convention are correct and remain the source of truth.

### Incorrect aiming calculation

`coriolis_masse_curve_angle_degrees` in `src/lib.rs:4176-4204` documents TP A.19's normalized relation as

$$
\theta_c = \operatorname{atan2}\!\left(a\sin\phi,\ \cos\phi-b\right),
$$

but calls the API's above-positive `height_offset` “`b`” and evaluates:

```text
denominator = cos(phi) - height_offset
```

TP A.19's `b` is positive below center, so this silently identifies two coordinates with opposite signs. For a normalized API height $h=\texttt{height_offset}$, the required conversion is $b/R=-h$, and the API-coordinate denominator is therefore $\cos\phi+h$.

The error propagates through every public aiming result derived from that helper:

- `coriolis_masse_final_heading` (`src/lib.rs:4206-4218`) adds the incorrect curve angle to the aim heading;
- `Shot::masse_aim_estimate` (`src/lib.rs:3343-3367`) exposes the incorrect value through both `MasseAimEstimate::signed_curve_angle_degrees` and `MasseAimEstimate::final_heading` (`src/lib.rs:3262-3269`);
- `validate_coriolis_masse_bar_relationship` (`src/lib.rs:4250-4307`) validates BAR geometry against the incorrect predicted heading and reports it in `MasseBarRelationship` (`src/lib.rs:3271-3286`).

Repository-wide structural call-site search finds no other production callers. Direct calls are:

- `Shot::masse_aim_estimate` to the curve and final-heading helpers at `src/lib.rs:3354-3357`;
- `coriolis_masse_final_heading` to the curve helper at `src/lib.rs:4215-4217`;
- the BAR validator to the final-heading and curve helpers at `src/lib.rs:4281-4284`;
- focused integration tests in `tests/shot_strikes.rs:219-313`.

Because the functions are public crate-root APIs, downstream callers outside this repository can also observe the wrong values. The correction is an intentional behavioral cutover: signatures and data types stay stable, while numeric results for nonzero height change. Center-height contacts are unaffected.

### Self-confirming tests and documentation

The regression `coriolis_masse_helper_predicts_tp_a19_bar_final_direction` at `tests/shot_strikes.rs:219-258` constructs API `height_offset=+0.25` and repeats the same incorrect `cos(phi)-0.25` expression as its oracle. `shot_masse_aim_estimate_reports_current_jump_then_curve_mode` at `tests/shot_strikes.rs:291-313` compares the estimate to the faulty helper instead of an independent source-derived expectation. `coriolis_masse_bar_validation_rejects_unsupported_or_mismatched_requests` at `tests/shot_strikes.rs:261-288` checks generic rejection behavior but does not distinguish corrected from sign-reversed BAR geometry.

`PHYSICS_TODOS.md:66-75` repeats the convention error by calling the API height TP A.19's `b` and describing the `a=0.25R`, `b=0.25R` source case as an above-positive API contact. The separate statement at `PHYSICS_TODOS.md:19-22` that the strike spin decomposition is aligned remains correct.

### Impact

Above-center and below-center contributions are swapped in the analytic final-direction prediction. A caller can receive a heading error exceeding 60° for a valid, moderate contact point and can have `validate_coriolis_masse_bar_relationship` accept the wrong reference line while rejecting the source-aligned one. This is a deterministic coordinate-conversion defect, not the smaller empirical difference TP A.19 attributes to squirt or cue-ball/tip/slate jamming.

The defect is confined to the analysis/aiming surface. It does not show that the actual struck-state initializer bends or spins the ball with the wrong sign.

## Whitepaper and corpus evidence

TP A.19 is the decisive authority:

- `whitepapers/tp_a_19_masse_shot_aiming_method_and_curved_cue_ball_paths.pdf:30-32` labels $a$ as offset right of center and $b$ as offset **below** center.
- Lines `54-66` define the initial forward velocity from cue elevation.
- Lines `68-98` derive the initial horizontal spin components

  $$
  \omega_{x0}=\frac{5v_e}{2R^2}b,
  \qquad
  \omega_{y0}=\frac{5v_e}{2R^2}a\sin\phi.
  $$

- Lines `98-112` give Eq. 7's final-direction ratio with denominator $R\cos\phi-b$.
- Lines `114-150` independently derive the BAR geometry. Eqs. 9 and 10 state

  $$
  R\cos\phi=b+\overline{RA}\cos\theta_c\sin\phi,
  \qquad
  \cos\theta_c=\frac{R\cos\phi-b}{\overline{RA}\sin\phi},
  $$

  which reproduces the same denominator when combined with Eq. 8.

After normalizing by $R$, define $a_R=a/R$, $b_R=b/R$, and retain the API's $h_R=\texttt{height_offset}$. Since below-positive $b_R=-h_R$,

$$
\theta_c
=\operatorname{atan2}\!\left(a_R\sin\phi,\ \cos\phi-b_R\right)
=\operatorname{atan2}\!\left(a_R\sin\phi,\ \cos\phi+h_R\right).
$$

The broader Coriolis/Alciatore discussion at `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf:120-154` identifies `RA` as the final direction and points to TP A.19 for the detailed proof; it is supporting orientation, not a replacement for TP A.19's explicit coordinate labels and equations. Generated corpus cross-references at `agent_knowledge/whitepapers_corpus.txt:16819-16830`, `:37870-37875`, and `:50205-50210` likewise identify TP A.19 as the detailed BAR authority.

## Numeric reproducer

Use a standard 2.25 in pool ball, so $R=1.125\ \mathrm{in}$, with:

- right tip offset $a_R=+0.25$ ($a=0.28125\ \mathrm{in}=0.25R$);
- public API height $h_R=+0.25$ ($0.28125\ \mathrm{in}$ above center);
- cue elevation $\phi=75^\circ$;
- aim heading $0^\circ$.

The API contact corresponds to TP A.19 $b_R=-0.25$, not $+0.25$.

Current result:

$$
\operatorname{atan2}\!\left(0.25\sin75^\circ,\ \cos75^\circ-0.25\right)
=87.9084539054^\circ\approx87.91^\circ.
$$

Source-aligned result:

$$
\operatorname{atan2}\!\left(0.25\sin75^\circ,\ \cos75^\circ+0.25\right)
=25.3886425866^\circ\approx25.39^\circ.
$$

The present error is $62.5198113188^\circ\approx62.52^\circ$. The below-center API counterpart $h_R=-0.25$ must produce approximately $87.91^\circ$; current code instead gives approximately $25.39^\circ$. Thus the counterexample also exposes the exact above/below swap.

## Root cause

The implementation copied TP A.19's algebraic symbol `b` without copying its coordinate definition. `CueTipContact::height_offset` was treated as a name-compatible substitute even though it is above-positive and dimensionless, while TP A.19's `b` is below-positive and dimensional before normalization. The rustdoc then encoded that conflation, and the original test duplicated the implementation expression, allowing the semantic error to pass.

This is a convention-boundary defect, not a derivation defect in TP A.19 and not a general sign error in cue impact.

## Explicit non-goals

- Do not change the public `CueTipContact` height convention, constructor, accessors, fields, serialization expectations, or DSL `.tip(..., height: ...)` meaning.
- Do not add a second public below-positive contact type or expose TP A.19's `b` as a new API parameter.
- Do not invert, rewrite, or otherwise “fix” `compute_post_strike_planar_state`'s `local_angular_right = -spin_scale * height_offset` mapping at `src/lib.rs:4123-4159`; it already performs the correct conversion for the physical spin state.
- Do not change squirt, cue transfer, table-contact impulse, swerve integration, curve-mode classification, speed dependence, cue elevation validation, or BAR tolerances.
- Do not add DSL `.masse(...)` syntax or a full speed/path/obstacle-clearance solver.
- Do not alter the checked-in TP A.19 PDF, qualitative `whitepapers/swerve.md`, or generated `agent_knowledge` files.

## Implementation plan

### Phase 1 — Replace self-confirming tests with source-derived failures

1. In `tests/shot_strikes.rs`, replace or split `coriolis_masse_helper_predicts_tp_a19_bar_final_direction` so its expected values are independent of the production helper.
2. Construct `CueTipContact(side=+0.25, height=+0.25)` at $75^\circ$ and assert `coriolis_masse_curve_angle_degrees` is `25.3886425866°` within the existing numeric tolerance, computed in the test from `atan2(0.25*sin(75°), cos(75°)+0.25)` or recorded with that equation beside the assertion.
3. Construct the matching below-center contact with `height=-0.25` and assert `87.9084539054°`, using `atan2(0.25*sin(75°), cos(75°)-0.25)`. Assert the below-center angle is greater than the above-center angle for this case. This second case is also TP A.19's published $b_R=+0.25$ convention expressed through the public API.
4. For an aim heading of `0°`, assert `coriolis_masse_final_heading` independently yields the same two corrected headings; do not use `coriolis_masse_curve_angle_degrees` as the only oracle for the caller.
5. Strengthen `shot_masse_aim_estimate_reports_current_jump_then_curve_mode` so `MasseAimEstimate::signed_curve_angle_degrees` and `final_heading` are asserted against the independent `25.3886425866°` expectation. Preserve its existing `JumpThenCurve` and zero-speed `ContinuousOnClothSwerve` assertions to prove the sign fix does not alter curve-mode classification.
6. Build `final_reference_point` from the corrected `25.3886425866°` `RA` heading and assert `validate_coriolis_masse_bar_relationship` accepts it. Also build the old sign-reversed `87.9084539054°` reference and assert the validator returns `ShotError::MasseAimRelationshipMismatch` at a tight tolerance. Retain coverage for no-side-spin rejection and generic mismatched requests.
7. Add a separate struck-state regression using two otherwise identical level, north-heading shots with `side=+0.25` and `height=±0.25`, routed through `strike_resting_ball_on_table`. Assert the above-center shot has negative shot-right/horizontal-axis spin, the below-center shot has positive spin, their magnitudes are equal and opposite, and their vertical-axis side-spin components are equal. This observable public-API test locks the already-correct `-height_offset` decomposition without reaching into the private helper.
8. Run only the new/changed test filters and confirm the aiming expectations fail under the old denominator while the strike-state protection already passes. That failure/pass split proves the planned production edit is scoped to the actual defect.

### Phase 2 — Make the convention conversion explicit at the source

1. In `coriolis_masse_curve_angle_degrees` (`src/lib.rs:4183-4204`), introduce a clearly named local conversion:

   ```text
   b_over_r = -tip_contact.height_offset().as_f64()
   denominator = cos(phi) - b_over_r
   ```

   This is algebraically equivalent to `cos(phi) + height_offset`, but the explicit `b_over_r` assignment makes the source-paper boundary auditable and prevents a future maintainer from “correcting” the plus sign back to the broken form.
2. Keep the existing side-offset numerator, `atan2` call, angle units, side sign, elevation/no-side validation, and return type unchanged. Do not clamp or special-case a zero/negative denominator: `atan2` must retain the source relation's quadrant behavior.
3. Do not touch `compute_post_strike_planar_state` (`src/lib.rs:4123-4159`) or its call from `strike_resting_ball` (`src/lib.rs:4350-4405`). The new struck-state regression is a guard against accidental collateral edits there.
4. Run the focused helper, heading, estimate, BAR, and strike-state tests before changing documentation.

### Phase 3 — Cut over every in-repository public caller

No signature or data-model migration is required. All production callers already delegate to `coriolis_masse_curve_angle_degrees`, so the one convention conversion propagates atomically:

- `coriolis_masse_final_heading` receives the corrected angle;
- `Shot::masse_aim_estimate` receives corrected `signed_curve_angle_degrees` and `final_heading` while preserving `curve_mode`;
- `validate_coriolis_masse_bar_relationship` compares against and reports the corrected heading/angle.

Update all direct tests at `tests/shot_strikes.rs:219-313` to the corrected expectations in the same change. Do not add compatibility flags or preserve the old numeric behavior: it has no valid interpretation under the documented public convention. Release notes, if maintained outside this repository, should call this a public behavioral correction for nonzero `height_offset`, not an API rename.

### Phase 4 — Correct source-facing documentation after the focused smoke test passes

1. Rewrite the rustdoc on `coriolis_masse_curve_angle_degrees` at `src/lib.rs:4176-4182` to name both conventions and dimensions explicitly:
   - TP A.19: $a_R=a/R$, below-positive $b_R=b/R$, and denominator $\cos\phi-b_R$;
   - API: above-positive $h_R=\texttt{height_offset}=-b_R$, giving denominator $\cos\phi+h_R$.
2. Add one concise convention sentence to `Shot::masse_aim_estimate` rustdoc at `src/lib.rs:3343-3348` or link its height wording directly to `CueTipContact`, so callers do not reinterpret the estimate as accepting TP's below-positive `b`.
3. Correct `PHYSICS_TODOS.md:66-75` to record that the completed aiming surface translates above-positive API height into below-positive TP A.19 `b`, and update its test summary to distinguish API `height=+0.25R` from source `b=+0.25R` (API `height=-0.25R`). Preserve the aligned strike-spin statement at `PHYSICS_TODOS.md:19-22`.
4. Leave `whitepapers/swerve.md` and `DSL_SHOT_MINI_SPEC.md` unchanged: neither states the incorrect equation or height sign. Leave the whitepaper and generated corpus/index artifacts unchanged because no source document, extraction, or authority classification changes. In particular, do not hand-edit `agent_knowledge/*`.

## Regression and acceptance tests

The implementation is accepted when all of these observable invariants hold:

1. For $a_R=+0.25$, public $h_R=+0.25$, and $\phi=75^\circ$, the curve angle and `0°`-aim final heading are approximately `25.39°`, not `87.91°`.
2. For the same side/elevation and public $h_R=-0.25$ (TP A.19 $b_R=+0.25$), they are approximately `87.91°`, not `25.39°`.
3. The below-center result is greater than the above-center result in this reproducer, and their difference is approximately `62.52°`.
4. `Shot::masse_aim_estimate` exposes the corrected angle and heading while returning the same `MasseCurveMode` as before.
5. BAR geometry constructed from the corrected `RA` heading is accepted; geometry constructed from the old sign-reversed heading is rejected with `MasseAimRelationshipMismatch` under a tolerance much smaller than `62.52°`.
6. Existing `MasseRequiresSideSpin`, cue-elevation validation, coincident-point, and tolerance validation behavior remains unchanged.
7. A zero-height contact retains the pre-change formula because $h_R=b_R=0$.
8. For matched north-heading level strikes, above- and below-center contacts retain equal-and-opposite horizontal-axis spin with above center negative and below center positive; same-side vertical-axis spin is unchanged. This proves the aiming correction did not invert the physical strike decomposition.

## Risks and edge cases

- **Quadrant changes are intentional.** When $\cos\phi+h_R$ crosses zero, `atan2` can return a magnitude at or above `90°`. Replacing it with one-argument `atan`, an absolute value, or a clamp would introduce a new physics error.
- **Negative side offsets:** the numerator remains signed. The change must not take absolute values or otherwise disturb left/right curve direction.
- **Zero height:** behavior must be bit-for-bit algebraically equivalent except for harmless local-variable evaluation; it is a useful no-regression boundary.
- **Near-zero side/elevation:** existing domain errors must remain authoritative; the sign conversion does not justify weakening validation.
- **External callers:** code continues to compile, but corrected values can materially change stored aim solutions. The rustdoc and historical physics note must make the behavioral correction explicit.
- **Test-oracle coupling:** caller tests must use the whitepaper-derived equation/numeric result, not merely compare one public helper to another.
- **Accidental overcorrection:** searching for `height_offset` and globally flipping signs would break valid follow/draw and spin behavior. Restrict the production edit to the Coriolis/BAR conversion seam.

## Dependencies and overlap

There is no implementation dependency on another planned physics correction: this change is a local semantic conversion in the cue aiming surface plus focused tests and documentation. It intentionally excludes event scheduling, curved-motion integration, spin lifetime/decay, airborne geometry, contacts, rails, and pockets.

Named sibling plans `plans/curved-rolling-event-detection.md` (“Use canonical curved rolling trajectories for continuous event detection”) and `plans/rolling-side-spin-lifetime.md` (“Restore TP B.2 rolling turn while side spin outlasts translation”) are adjacent only in that they concern curved cue-ball motion. They operate on TP B.2 rolling integration/event roots after state initialization; this plan operates on the separate TP A.19 analytic aiming seam. Neither sibling should change `CueTipContact` coordinates or absorb this sign fix, and this plan should not alter their trajectory or spin-decay work.

The only direct historical overlap is the already-completed massé entry in `PHYSICS_TODOS.md:66-75`, which this plan corrects. Its separate “Elevated side-tip spin decomposition is directionally consistent” entry at `PHYSICS_TODOS.md:19-22` is not superseded and should be protected by the new strike-state regression rather than rewritten as a defect.

## Verification commands

Run in this order so failures identify the smallest contract first:

```sh
cargo test --test shot_strikes coriolis_masse_helper
cargo test --test shot_strikes shot_masse_aim_estimate
cargo test --test shot_strikes coriolis_masse_bar_validation
cargo test --test shot_strikes strike_spin
cargo test --test shot_strikes
cargo test
```

Name the new strike regression so the `strike_spin` filter is stable. The first four commands are the focused behavioral proof; `cargo test --test shot_strikes` checks the surrounding cue-impact surface; the final workspace-default `cargo test` is the relevant aggregate regression suite.

## Candidate commit message

```text
Correct TP A.19 massé height sign conversion

Translate CueTipContact's above-positive height to TP A.19's below-positive b before evaluating Coriolis/BAR headings. Add independent above/below aiming and strike-spin regressions, and correct the physics documentation.
```
