# Preserve Physical Pocket Mouth Width with Rounded Jaw Arcs

Date: 2026-07-10

Severity: Medium-high  
Priority: P1 — correct before relying on rounded-jaw outcomes for calibration, event ordering, or 3-D pocket geometry

## Problem statement

`PocketSpec.width` is currently used as the physical pocket-mouth width by the TP 3.5–3.8 capture equations, but the same width is used as the center-to-center spacing of two rounded jaw circles. A nonzero jaw-nose radius therefore moves both solid surfaces into the configured opening. The jaw predictor sees a narrower pocket than the capture predictor even though both consume the same `TableSpec`.

The configured width must mean one thing everywhere: the physical surface-to-surface, point-to-point mouth opening. Rounded noses must be represented as bounded, exposed physical arcs derived from that measured opening, the rail/facing directions, and the nose radius. A mouth endpoint is a physical support point on an arc, not the arc's circle center.

## Current behavior and impact

### Observed implementation

- `PocketSpec.width` and `PocketShapeSpec::rounded_noses` are public configuration at `src/lib.rs:13899-13942`.
- The default Brunswick profile sets corner width to `4.500 in`, side width to `5.000 in`, and both rounded-nose radii to `0.125 in` at `src/lib.rs:54-62` and `src/lib.rs:14047-14145`.
- `pocket_jaw_reference_point_in_inches` places two jaw reference points exactly one configured width apart at `src/lib.rs:6013-6038`.
- `pocket_jaw_geometry_in_inches` reinterprets each reference point as a full-circle center and adds the configured `nose_radius` at `src/lib.rs:6040-6056`.
- `compute_next_ball_jaw_impact_on_table` tests the ball center against that full circle at radius `R + r_j` at `src/lib.rs:8397-8472`. It does not reject contacts on the circle's buried or facing-exterior sectors.
- `pocket_jaw_collision_basis` again uses the ball-center-to-circle-center radial direction during resolution at `src/lib.rs:12406-12416`.
- In contrast, `pocket_mouth_width_in_inches`, slow target geometry, fast target geometry, and capture gating continue to use the unmodified configured width as TP mouth width `p` at `src/lib.rs:5976-5980`, `src/lib.rs:6067-6795`, and `src/lib.rs:6799-6958`.
- The same jaw reference point is also used as the physical mouth plane by `pocket_mouth_plane_gap_raw` and `first_pocket_mouth_plane_crossing_time_during_current_phase_raw` at `src/lib.rs:7159-7217`. Its current name hides the fact that “mouth tip,” “sharp rail/facing intersection,” and “circle center” are different geometric concepts once a radius is nonzero.
- `tests/n_ball_pockets.rs:1071-1105` verifies only that changing the radius changes one oblique impact time. It does not verify that the configured physical aperture is invariant.

### Dimensional discrepancy

For two full circles whose centers are separated by configured width `p`, the clear surface gap is

\[
p_{\text{current}} = p - 2r_j.
\]

With the current default `r_j = 0.125 in`:

- side pocket: `5.000 in - 2(0.125 in) = 4.750 in`;
- corner pocket: `4.500 in - 2(0.125 in) = 4.250 in`.

The jaw geometry is therefore `0.250 in` narrower than the nominal mouth in both cases, while capture formulas still use `5.000 in` and `4.500 in`. This is not a tolerance issue. It is a disagreement about what the configured dimension measures.

### User-visible impact

- A ball with genuine straight-through clearance can receive a false `BallJawImpact`.
- Changing only nose curvature changes the effective straight-through aperture, so a table cannot be calibrated independently by measured mouth width and nose shape.
- Jaw collision normals can come from the back side of a full circle rather than the exposed cushion surface.
- Jaw and capture candidates can overlap in space. The separate causal-ordering defect is addressed by `plans/jaw-capture-event-causality.md`, but removing its time override does not correct the false jaw surface.
- All pocket-aware scheduler, advance, simulation, and DSL shot paths inherit the result through `PocketAwareEventCache::refresh_ball` (`src/lib.rs:1997-2064`), `PocketAwareEventCache::next_event`, and `resolve_n_ball_system_event_with_physics_and_pockets_on_table` (`src/lib.rs:10742-10885`).

## Affected files, symbols, and callers

### `src/lib.rs`

| Current range | Symbol/caller | Required role after the change |
| --- | --- | --- |
| `54-62` | `GC4_CORNER_POCKET_WIDTH`, `GC4_SIDE_POCKET_WIDTH` | Rename/document as physical mouth widths, not center spacings. |
| `1557-1574` | `PocketJaw`, `PredictedBallJawImpact` | Keep the event API unless a stored contact feature is proven necessary; prediction and resolution must agree on the same finite arc closest point. |
| `1997-2064` | `PocketAwareEventCache::refresh_ball` | Continue consuming the corrected jaw predictor; no cache-local geometry fork. |
| `5976-5980` | `pocket_mouth_width_in_inches` | Read the renamed physical-mouth field. |
| `5991-6004` | `pocket_jaw_associated_rail` | Supply the rail side of each fillet and exposed-arc bounds. |
| `6006-6056` | `ResolvedPocketJawGeometry`, `pocket_jaw_reference_point_in_inches`, `pocket_jaw_geometry_in_inches` | Replace point-as-center geometry with distinct physical mouth tips, virtual sharp intersections, and canonical uninflated point/arc geometry. |
| `6067-6795` | slow side/corner target geometry | Continue using the physical mouth width; read the table-configured facing direction rather than a parallel type constant. |
| `6799-6958` | `FastPocketTargetGeometry` and TP 3.7/3.8 target helpers | Same mouth/facing migration; numeric defaults must remain unchanged. |
| `7061-7063` | `SIDE_POCKET_WALL_ANGLE_DEGREES`, `CORNER_POCKET_WALL_ANGLE_DEGREES` | Move table-specific facing calibration into `PocketSpec`; do not retain a second runtime convention. |
| `7159-7217` | mouth-plane gap and crossing helpers | Use the physical mouth-tip plane, never a virtual intersection or arc center. |
| `7983-8011` | `raw_state_on_pocket_mouth_with_local_offset` | Migrate internal fixtures to the physical tip/mouth-plane helper. |
| `8397-8472` | `compute_next_ball_jaw_impact_on_table` | Predict first contact with a finite point or exposed arc, including arc endpoints, not a full circle. |
| `10742-10885` | pocket-aware event resolver | Resolve only corrected jaw events and preserve the event's selected physical normal. |
| `12406-12416` | `pocket_jaw_collision_basis` | Derive the normal from the closest point on the bounded physical jaw feature. |
| `13159-13209` | `collide_ball_jaw_on_table_with_radius_and_profile` | Consume that corrected basis without changing rail response coefficients. |
| `13899-14183` | `PocketJawGeometry`, `PocketShapeSpec`, `PocketSpec`, `TableSpec` constructors and mutators | Perform the clean public data-model cutover described below. |

### Tests

- `tests/table_and_pocket_geometry.rs:21-76` and `102-156`: migrate nominal dimension/shape assertions and add surface-to-surface mouth invariants.
- `tests/n_ball_pockets.rs:446-458`: reuse `fast_rolling_side_pocket_state` for the exact `1.300 in` fixture.
- `tests/n_ball_pockets.rs:829-923`: migrate point/rounded touching and immediate-impact tests to physical tips/arcs.
- `tests/n_ball_pockets.rs:1071-1105`: retain the useful oblique-curvature behavior, but replace the unconditional “larger circle is earlier” premise with exposed-arc and fixed-aperture assertions.
- Other direct public-field uses at `tests/n_ball_pockets.rs:382-385`, `570-573`, `614-617`, `852-855`, and `880-885` must use `mouth_width` and must not reconstruct an arc center from that width.

No custom `TableSpec` literal exists outside the four constructors in `src/lib.rs`; the DSL currently selects named table presets via `TableRef::to_table_spec` at `src/dsl.rs:3823-3828`. The public nested fields and constructors are nevertheless a library API, so this is an intentional clean cutover rather than a compatibility alias.

## Whitepaper and corpus evidence

### Physical mouth-width definition

`whitepapers/just_how_big_are_the_pockets_anyway_part_i.pdf`, Principle 8 and Diagram 4 discussion (extracted PDF lines `135-168`), describes a side-pocket opening as measured “from point to point across the mouth of the pocket.” This is direct qualitative evidence for the measurement convention. The article is derivative explanatory material, so it establishes terminology rather than superseding the technical proofs.

### TP equations use `p` as that mouth width

`whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf`:

- extracted lines `12` and `56-60` label `p` as “width of pocket mouth” / “mouth width” and separately define ball radius `R` and wall angle `α`;
- extracted lines `94-150` use `p/2` in the left-target equations;
- the generated corpus anchor is `agent_knowledge/whitepapers_formula_candidates.txt:1985-2024`.

Representative TP 3.5 point-contact expression:

\[
s_{\text{left,point}}(\theta)
= \frac{p}{2}\cos\theta - R\sin\beta_l(\alpha,p,b,R_{\text{hole}},\theta).
\]

`whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf`:

- extracted lines `36-46` again define `R = 1.125 in`, `p` as mouth width, and use `p/2`;
- extracted lines `68-112` retain `p` in the near-point/wall branches;
- the generated corpus anchor is `agent_knowledge/whitepapers_formula_candidates.txt:2071-2100`.

Its straight near-point branch is

\[
s_{\text{left,point}}(p,r,\theta)=\frac{p}{2}\cos\theta-r,
\]

which at `θ = 0` and `r = R` gives the familiar center corridor half-width

\[
s_{\max}=\frac{p}{2}-R.
\]

These TPs are analytic models with stated assumptions, not empirical validation of the current Brunswick calibration. They do, however, unambiguously treat `p` and `R` as separate physical dimensions. Subtracting jaw radius from `p` only in the collision geometry is inconsistent with those equations.

### Facing geometry

`whitepapers/tp_b_15_pocket_geometry_calculations.pdf`, extracted lines `28-42`, defines the pocket mouth, throat, and facing at the intersections of the cushion-nose, facing, and depth edge lines. Lines `42-80` define facing angle `α`, wedge angle `β`, and local angle `θ`, including

\[
\beta = 180^\circ-\alpha,\qquad
\theta=\alpha-135^\circ,\qquad
\frac{m-t}{2}=r\sin\theta.
\]

Lines `110-144` and `agent_knowledge/whitepapers_formula_candidates.txt:3291-3311` provide the angle conversion and the `m=4.5 in`, `t=3.75 in`, `c=2 in` worked example. This supports making facing direction part of table geometry instead of deriving an unbounded circle from radius alone.

The implementation must document its angle convention explicitly. TP B.15's conventional included facing angle and TP 3.5–3.8's pocket-local wall angle are related inputs, not interchangeable numbers.

## Numeric reproducer

Use the current default right-side pocket and the existing fast rolling helper semantics:

- physical mouth width: `p = 5.000 in`;
- pool-ball radius: `R = 1.125 in`;
- rounded nose radius: `r_j = 0.125 in`;
- pocket mouth center: `(50.000, 50.000) in`;
- initial ball center: `(40.000, 51.300) in`, i.e. local lateral offset `s = +1.300 in`;
- velocity: `(200.000, 0.000) in/s`;
- rolling angular velocity: `(0, 200/1.125, 0) = (0, 177.777… , 0) rad/s`.

The physical straight-through limit is

\[
\frac{p}{2}-R=2.500-1.125=1.375\ \text{in}.
\]

The fixture therefore has genuine clearance

\[
1.375-1.300=0.075\ \text{in}.
\]

Current geometry puts the upper jaw's full-circle center at lateral offset `2.500 in`. The path-center separation from that center is only

\[
d=2.500-1.300=1.200\ \text{in},
\]

while the expanded circle radius is

\[
R+r_j=1.125+0.125=1.250\ \text{in}.
\]

Consequently the current predictor finds a false circle entry

\[
\Delta x=\sqrt{1.250^2-1.200^2}=\sqrt{0.1225}=0.350\ \text{in}
\]

before the ball center reaches the jaw-circle center's `x=50.000 in` line, at `x=49.650 in`. The corrected bounded arc must return no jaw event for this path, while the unchanged TP target/capture calculation remains eligible.

Add a paired boundary fixture immediately inside and outside `|s|=1.375 in`. The inside case must have positive or tangent clearance according to the chosen numeric tolerance; the outside case must not pass cleanly. Do not encode `4.750 in` as the new mouth or widen the TP capture formula to hide the false collision.

## Root cause

1. **One point has three incompatible meanings.** `pocket_jaw_reference_point_in_inches` is simultaneously treated as a measured mouth endpoint, a mouth-plane anchor, and a rounded-circle center.
2. **The configuration name is ambiguous.** Public `PocketSpec.width` does not state that it is the physical point-to-point mouth opening used as TP `p`.
3. **Radius-only geometry is underdetermined for oblique contact.** `PocketShapeSpec` supplies a nose radius, but table-specific facing direction remains in private pocket-type constants and the resolved jaw has no exposed angular interval.
4. **The collision primitive is too broad.** A full fixed circle exposes solid-side and back-side sectors that are not physical contact surfaces.
5. **Capture and contact geometry are resolved independently.** Capture correctly keeps nominal `p`; jaw construction silently changes it.

## Proposed geometry and invariants

The following is proposed design, not a description of current code.

### Public table specification

Perform a clean cutover:

```rust
pub struct PocketSpec {
    pub ty: PocketType,
    pub depth: Diamond,
    /// Physical nearest-surface, point-to-point mouth opening; TP 3.5–3.8 `p`.
    pub mouth_width: Diamond,
    /// Conventional pocket-facing calibration, with documented conversion to pocket-local direction.
    pub facing_angle_degrees: f64,
    pub shape: PocketShapeSpec,
}
```

- Rename `PocketSpec.width` to `mouth_width`; do not retain a deprecated alias or a second center-spacing field.
- Rename `GC4_CORNER_POCKET_WIDTH` and `GC4_SIDE_POCKET_WIDTH` to `GC4_CORNER_POCKET_MOUTH_WIDTH` and `GC4_SIDE_POCKET_MOUTH_WIDTH`.
- Keep `nose_radius` in `PocketJawGeometry::RoundedNoses`.
- Move the table-specific facing calibration into each `PocketSpec`; the resolver derives the pocket-local rail/facing directions from `Pocket`, `PocketType`, and that angle.
- Initialize defaults so the derived local wall angles remain numerically `14°` for side pockets and `7°` for corner pockets, preserving current TP target curves. For a B.15-style included-angle representation this corresponds to the existing local-angle offsets (for example, the current corner `7°` maps to `142° = 135° + 7°`). Document the side/corner conversion functions and test them; never pass the conventional included angle directly where TP 3.5–3.8 expect local `α`.
- Validate finite angle, finite/nonnegative mouth width, and finite/nonnegative nose radius at table-geometry resolution. A disabled carom pocket may retain zero dimensions but must not enter pocket resolution.

### Canonical uninflated jaw feature

Replace the current center/radius struct with one canonical physical feature in raw inches:

```rust
enum ResolvedPocketJawGeometry {
    Point {
        mouth_tip: RawPoint2,
    },
    RoundedArc {
        mouth_tip: RawPoint2,
        center: RawPoint2,
        nose_radius: f64,
        rail_boundary_normal: RawUnitVector2,
        facing_boundary_normal: RawUnitVector2,
        sweep: ArcSweep,
    },
}
```

`ArcSweep` must encode an oriented bounded interval without an `atan2` wraparound ambiguity. The geometry is uninflated: it represents the physical table solid only. Ball radius is applied by the collision query, allowing the same arc to be reused later by 3-D sphere-distance geometry.

Construction in the pocket-local frame:

1. Let `e` be `pocket_entry_axis(pocket)` and `t = (-e_y, e_x)` the mouth tangent.
2. Place desired physical mouth support points `T₁` and `T₂` at the mouth center plus/minus `(p/2)t`. Thus `|T₂-T₁|=p` by construction for side and corner pockets.
3. Derive the two oriented rail-nose and facing lines from `pocket_jaw_associated_rail`, pocket handedness, and the configured facing angle.
4. Treat the old sharp rail/facing-line crossing as a **virtual intersection**, not a measured tip and not a circle center. Construct the radius-`r_j` fillet tangent to the two oriented lines on the solid side.
5. Compute the fillet's support point toward the opposite jaw over the exposed sweep. Translate/solve the virtual intersection so that this support point is exactly `T_j`. In support-function form, for opening direction `u_j` from one jaw toward the other,

   \[
   T_j=\operatorname*{arg\,max}_{Q\in A_j} Q\cdot u_j,
   \]

   where `A_j` is the exposed physical arc after placement. This absorbs the radius- and facing-angle-dependent support offset into the virtual intersection rather than into physical `p`.
6. Store only the minor exposed sweep between rail and facing tangencies, with the opening support point inside that sweep. Do not expose the rest of the source circle.
7. For `PointNoses`, return the physical `T_j` directly and preserve the current swept-point behavior.

Required construction invariants:

\[
|T_2-T_1|=p
\]

for every radius and every mirrored pocket, and

\[
\min_{Q_1\in A_1,Q_2\in A_2}|Q_2-Q_1|=p
\]

along the configured point-to-point mouth measurement direction. The exact circle-center spacing is derived data and must not be reused as `p`.

### Finite-arc collision query

Add a dedicated closest-point/first-contact query for `ResolvedPocketJawGeometry`.

For an interior arc feature and ball center `B`, the radial candidate satisfies

\[
|B-C|=R+r_j,
\qquad
n=\frac{B-C}{|B-C|},
\qquad
Q=C+r_j n.
\]

Accept this candidate only when `n` lies within the oriented exposed sweep. For directions outside the sweep, use the closest exposed endpoint; the distance to a finite arc is piecewise

\[
d(B,A)=
\begin{cases}
\bigl||B-C|-r_j\bigr|, & \text{if the radial projection lies on the exposed sweep},\\
\min(|B-Q_{rail}|,|B-Q_{facing}|), & \text{otherwise}.
\end{cases}
\]

First contact occurs when `d(B(t),A)=R` and the relative motion is closing. Generate and validate candidates for the radial arc and both endpoints over the current motion-phase horizon, including `t=0`; select the earliest valid finite-feature contact. Do not call the current single full-circle entry helper and merely discard an out-of-sweep first root, because a later valid feature or endpoint root could then be missed.

At resolution, recompute the same closest feature at `state_at_impact` and use `(B-Q)/|B-Q|` as the cushion normal. This avoids a public `PredictedBallJawImpact` migration while ensuring prediction and response use the same physical feature. Apply one shared geometric tolerance to arc membership, root validation, and resolution; do not expand the physical mouth by that tolerance.

## Explicit non-goals

- Do not recalibrate whether the named Brunswick GC IV profile's `5.000 in`, `4.500 in`, `1.400 in`, or `0.125 in` values match a particular manufactured or shimmed table. This plan fixes dimensional semantics for whatever values are configured.
- Do not add rail mouth cutouts, full pocket facings, throat walls, shelf solids, or hole/drop volumes. Those belong to `plans/pocket-rail-cutouts.md`; this plan supplies the canonical mouth tips, facing directions, and arc boundaries it must reuse.
- Do not change the jaw/capture event comparator or its `5 ms` override. That belongs to `plans/jaw-capture-event-causality.md` and depends on this plan's corrected physical mouth.
- Do not add airborne vertical gating or 3-D collision volumes. The airborne plan may consume this plan's uninflated physical arc rather than creating another jaw definition.
- Do not retune TP 3.5–3.8 target curves, speed interpolation, maximum entry angles, capture radii, rail restitution, or spin response.
- Do not rename events or change pocketed-state semantics.
- Do not preserve `PocketSpec.width` as an alias; dual width meanings are the defect.

## Phased implementation plan

### Phase 1 — Add failing contract tests first

1. In `tests/table_and_pocket_geometry.rs`, add a default-table test that resolves both jaw surfaces for every pocket and asserts point-to-point surface gaps of exactly `5.000 in` for side pockets and `4.500 in` for corner pockets, within a scale-aware tolerance.
2. Add the same invariant after replacing a pocket's rounded radius with at least `0.125 in` and `0.750 in`: the centers/oblique curvature may move, but the measured surface opening may not.
3. In the private `src/lib.rs` test module, add mirrored arc-construction tests for all six pockets:
   - physical mouth tips are `p` apart;
   - the tip/support normal is inside the sweep;
   - rail and facing tangent normals are accepted at the boundary;
   - a normal just beyond either boundary and the circle's buried backside are rejected;
   - corner and side mirrors produce equal distances and opposite handedness without angle-wrap failures.
4. In `tests/n_ball_pockets.rs`, add `fast_rolling_side_pocket_state(1.300)` as the explicit numeric regression. Assert direct jaw prediction is `None` and direct capture prediction is `Some(CenterRight)` for the same state.
5. Add paired `1.375 in ± ε` straight-entry tests. Just inside the physical limit must not produce a jaw hit; just outside must not pass cleanly. Choose `ε` comfortably above root tolerance and below calibration significance, and state it in inches.
6. Add an oblique path whose closest contact lies inside the exposed arc and a geometrically similar path intersecting only the discarded full-circle sector. The first must produce a jaw event; the second must not.
7. Preserve a zero-time closing-contact test on an actual exposed arc point and add its separating-motion counterpart to prevent immediate re-collision loops.

The new tests should fail against the current full-circle geometry for the stated physical reason, not because they inspect private source text.

### Phase 2 — Cut over `TableSpec` and every caller

1. Rename `PocketSpec.width` to `mouth_width` and rename the two public default-width constants to `*_MOUTH_WIDTH`.
2. Add the documented facing-angle field/conversion to `PocketSpec`. Populate all six pool pocket specs and the disabled carom specs explicitly.
3. Update `brunswick_gc4_corner_pocket`, `brunswick_gc4_side_pocket`, `disabled_corner_pocket`, and `disabled_side_pocket`; do not add old-field compatibility constructors.
4. Migrate `pocket_mouth_width_in_inches`, slow/fast target geometry, capture radius, mouth-plane code, internal tests, integration tests, and all direct public-field accesses listed above.
5. Replace runtime use of `SIDE_POCKET_WALL_ANGLE_DEGREES` and `CORNER_POCKET_WALL_ANGLE_DEGREES` with the per-pocket derived local angle. Default target-bound tests must remain numerically unchanged.
6. Keep `PocketShapeSpec::rounded_noses(nose_radius)` and `with_pocket_shape` focused on curvature. Facing direction belongs to the pocket/table calibration, not to a per-call collision workaround.

### Phase 3 — Resolve physical tips and exposed arcs

1. Split the current helper into explicit operations, with names that encode semantics:
   - physical mouth-tip pair;
   - virtual sharp rail/facing intersections;
   - canonical uninflated point/arc jaw features;
   - physical mouth-plane projection.
2. Implement the pocket-local support-offset construction above. Avoid heap allocation; each pocket has exactly two jaws and can use fixed arrays/value structs.
3. Validate the constructed radius, unit normals, tangencies, sweep, and support points once per resolved table geometry. If the configured radius/facing combination cannot form the requested fillet, reject the table specification explicitly; never fall back to a full circle.
4. Ensure slow/fast capture formulas receive `mouth_width` directly. They must not consume virtual intersection spacing, arc-center spacing, or `mouth_width - 2r_j`.
5. Make the canonical geometry reusable by the rail-cutout and airborne work; do not create a second draw-only or scheduler-only arc representation.

### Phase 4 — Replace full-circle prediction and response

1. Implement the finite-feature closest-point and first-contact helpers for point noses and rounded arcs.
2. Replace the jaw loop's `first_fixed_circle_entry_time_for_raw_motion` usage with candidate collection/validation against the exposed arc and endpoints over the current phase horizon.
3. Preserve current earliest-pocket/jaw selection across the twelve jaw features, but compare only physically valid candidates.
4. Replace center-radial `pocket_jaw_collision_basis` with the closest physical feature normal at `state_at_impact`.
5. Re-run the immediate-contact, closing/separating, oblique-curvature, discarded-sector, and mirror tests before touching any event-ordering behavior.

### Phase 5 — Complete callsite, source, and artifact cleanup

1. Remove obsolete ambiguous helpers and the pocket-type wall-angle runtime constants after all callers use the new model. Do not leave aliases, comments describing the old behavior, or parallel geometry.
2. Update rustdoc on `PocketSpec`, `PocketJawGeometry::RoundedNoses`, `PocketShapeSpec`, the default table constructors, and jaw prediction to state surface-to-surface mouth semantics and bounded-arc behavior.
3. Update test fixture names/messages from “jaw circle” or center-derived mouth to “mouth tip,” “jaw arc,” or “physical mouth,” as appropriate.
4. Do not hand-edit `agent_knowledge/whitepapers_formula_candidates.txt` or other generated `agent_knowledge` artifacts. The cited extraction remains source evidence; regenerate it only through its owning pipeline if that pipeline is intentionally run for a separate source change.
5. No whitepaper PDF needs modification. No DSL syntax migration is required because the DSL selects named table presets rather than serializing `PocketSpec` fields.

## Regression and acceptance tests

The implementation is accepted only if all of the following observable contracts hold:

1. **Nominal default gaps:** resolved surface-to-surface openings are `5.000 in`, not `4.750 in`, for both side pockets and `4.500 in`, not `4.250 in`, for all four corners.
2. **Radius invariance:** changing rounded-nose radius while holding `mouth_width` fixed does not move either straight-through aperture boundary. It may change valid oblique contact time and normal.
3. **Exact reproducer:** the `p=5.000 in`, `R=1.125 in`, `r_j=0.125 in`, `s=1.300 in` right-side fixture has `0.075 in` physical clearance, produces no direct jaw event, and remains directly capture-eligible.
4. **Boundary pair:** a straight ball-center path just inside `|s|=1.375 in` clears; a path just outside does not pass cleanly.
5. **Capture consistency:** TP straight target half-width remains `p/2-R`; no target formula substitutes center spacing or `p-2r_j`.
6. **Exposed-sector filtering:** a trajectory touching a valid interior arc sector schedules a jaw; a trajectory intersecting only the discarded full-circle backside does not.
7. **Endpoint correctness:** valid rail/facing arc-endpoint grazes are detected without tunneling, and the response normal comes from the endpoint closest point rather than the source-circle center.
8. **Immediate-contact stability:** an on-arc, inward-closing state produces one zero-time jaw event; an equal outward/separating state does not immediately re-impact.
9. **Mirror symmetry:** equivalent local states on left/right side pockets and all four corners produce equal times/distances and mirrored normals.
10. **Point-nose behavior:** `PointNoses` remains a swept point at the physical tip and retains its existing valid impacts.
11. **Target calibration stability:** existing TP 3.5–3.8 numeric target-bound tests remain unchanged after facing-angle data moves into `TableSpec`.
12. **Public cutover:** no production or test caller references `PocketSpec.width`, the old width constants, or reconstructs rounded centers from physical mouth tips.

## Risks and edge cases

- **Angle convention mismatch:** B.15 conventional facing angle and TP 3.5–3.8 local wall angle use different reference axes. Centralize and test the conversion for side and corner pockets; do not duplicate arithmetic at callsites.
- **Wrong fillet branch:** two line-offset intersections can satisfy unsigned tangency. Select the solid-side center using pocket entry axis, associated rail normal, and jaw handedness, then assert the physical mouth support point faces the opposite jaw.
- **Arc wraparound:** start/end angles near `-π/π` can invert a sweep. Store boundary unit vectors plus explicit orientation and test all mirrors with cross/dot predicates.
- **Endpoint feature changes:** the closest feature switches between radial arc and endpoints. Use one closest-point implementation for root validation and response so feature-boundary contacts cannot get different normals.
- **Grazing roots and `t=0`:** tangency may not create a sign-changing gap. Include analytic tangent candidates and closing/acceleration checks; use a scale-aware geometric tolerance rather than a macroscopic time preference.
- **Large radius or shallow facing:** a fillet may overlap another feature or have no valid exposed sweep. Reject invalid table geometry explicitly instead of clamping radius or silently reverting to a circle.
- **Temporary overlap with uncut rails/facings:** until `plans/pocket-rail-cutouts.md` is implemented, rail and arc solids can still overlap near a pocket. This plan owns the correct nose feature; the cutout plan owns subtraction and adjoining facing segments.
- **Event-ordering interaction:** correcting the `1.300 in` false jaw removes one reproducer for the `5 ms` override, but it does not prove causal ordering fixed. Keep `plans/jaw-capture-event-causality.md` acceptance tests independent.
- **Performance:** candidate checks cover at most twelve fixed jaw features. Use fixed-size values and analytic/bracketed phase roots; do not allocate a vector per predictor call.
- **Public API break:** `PocketSpec.width` is public. The clean rename is intentional; compiler errors are the migration inventory. Do not mask them with a duplicate field.

## Dependencies and overlap

- **`plans/pocket-rail-cutouts.md` — “Clip Rail Solids at Pocket Mouths and Add Explicit Facings.”** Shared geometry dependency. This plan owns physical mouth-width semantics, table facing calibration, mouth tips, and canonical uninflated nose arcs. The cutout plan must consume the arc tangency boundaries/endpoints for adjoining rail/facing solids and must not redefine mouth points or nose circles.
- **`plans/jaw-capture-event-causality.md` — “Restore Causal Jaw/Capture Event Ordering.”** That plan removes the noncausal jaw-over-capture time preference. It depends on this plan to eliminate the false `1.300 in` jaw candidate; neither plan substitutes for the other.
- **Airborne pocket geometry plan.** It should consume the same uninflated bounded arcs for 3-D sphere-distance and vertical gating. This plan does not add height extents or ballistic event logic.

Recommended ordering: establish this plan's table/mouth/arc contract first; the rail-cutout and airborne implementations can then reuse it. The causality comparator can be changed independently, but its end-to-end geometry tests should be run again after this plan lands.

## Verification commands

Run focused tests first:

```sh
nix develop -c cargo test --test table_and_pocket_geometry physical_pocket_mouth_width
nix develop -c cargo test --lib pocket_jaw_arc
nix develop -c cargo test --test n_ball_pockets straight_side_entry_with_1_300_in_offset
nix develop -c cargo test --test n_ball_pockets rounded_jaw
```

Then run the affected integration targets together:

```sh
nix develop -c cargo test --test table_and_pocket_geometry --test n_ball_pockets
```

Run relevant geometry/aiming coverage after the field and facing-angle cutover:

```sh
nix develop -c cargo test --test aiming_geometry --test table_and_pocket_geometry --test n_ball_pockets
```

Finally run the aggregate suite:

```sh
nix develop -c cargo test
```

The plan-only change does not run these commands; they are the required implementation verification sequence.

## Candidate commit message

```text
Preserve physical pocket widths with bounded jaw arcs

Treat configured mouth width as the point-to-point surface opening, derive
rounded noses from table facing geometry, and reject contacts outside each
exposed arc.
```
