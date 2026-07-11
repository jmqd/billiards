# Clip Rail Solids at Pocket Mouths and Add Explicit Facings

Date: 2026-07-10

Severity: **High**  
Priority: **P1 — correct before treating pocket-aware simulation as physical at rejected entries**

## Problem statement

The pocket-aware solver currently predicts collisions against four uninterrupted, axis-aligned cushion planes. Pocket capture and two circular jaw candidates are superimposed on those planes, but no pocket aperture is subtracted from the rail solid. When the target-envelope calculation rejects an entry, the rail predictor can therefore rebound the ball from an ordinary cushion face that physically does not exist across the pocket mouth. The center-right 60° fast-entry fixture below reaches this phantom right-rail plane before it reaches the current upper jaw.

The implementation must make the table boundary a nonoverlapping set of finite physical surfaces:

1. ordinary cushion-face segments clipped at every pocket mouth;
2. physical jaw/nose arcs at the segment ends;
3. explicit pocket-facing and inner-wall segments from each mouth nose toward the throat/hole region; and
4. a separate terminal capture boundary.

Collision prediction must operate on the ball-radius Minkowski offset of those physical surfaces. Capture rejection must allow the ball to continue through the aperture until it meets a real nose/facing/wall or exits the supported geometry; it must never restore a full-rail collision at the mouth.

## Observed current behavior and impact

### Observed implementation

- `src/lib.rs:5834-5855`, `rail_collision_gap_quadratic_coefficients`, creates one complete plane for each `Rail::{Top,Right,Bottom,Left}`. For the default 50×100 in playfield and a pool-ball radius `R=1.125 in`, the right-ball-center plane is `x=50-R=48.875 in` for every `y`.
- `src/lib.rs:5865-5906`, `first_rail_collision_time_during_current_phase_raw`, finds a root against that plane. It has no pocket-mouth or finite-segment membership check.
- `src/lib.rs:5908-5966`, `compute_next_ball_rail_impact_on_table`, loops over the four planes and returns the earliest root. Its rustdoc explicitly describes “four ideal rail planes.”
- `src/lib.rs:5968-6056`, especially `pocket_jaw_reference_point_in_inches`, `pocket_jaw_geometry_in_inches`, and `pocket_jaw_associated_rail`, constructs pocket geometry independently of the rail planes.
- `src/lib.rs:8397-8472`, `compute_next_ball_jaw_impact_on_table`, tests two fixed circles per pocket, inflated to `R + nose_radius`, independently of rail prediction.
- `src/lib.rs:8288-8395` and `:8491-8647`, `pocket_capture_gap_during_current_phase_raw` and `compute_next_ball_pocket_capture_on_table`, independently apply the TP-derived target, mouth-plane, back-plane, and acceptance gaps.
- `src/lib.rs:1961-2064`, `PocketAwareEventCache::{build,refresh_ball}`, stores jaw, capture, and rail candidates in separate arrays. `src/lib.rs:2109-2257`, `PocketAwareEventCache::next_event`, then compares those candidates chronologically.
- `src/lib.rs:10742-10885`, `resolve_n_ball_system_event_with_physics_and_pockets_on_table`, resolves `BallRailImpact` with the full rail collision model, `BallJawImpact` with a nose-normal rail profile, and `BallPocketCapture` by changing the state to `Pocketed`.
- `src/lib.rs:13889-14145`, `PocketShapeSpec`, `PocketSpec`, and the GC4 constructors, store mouth width, one `depth`, and jaw nose shape/radius. They do not store a finite facing, throat setback/width, or inner-wall primitive. The current side/corner target formulas instead use type-level wall-angle and hole/shelf constants at `src/lib.rs:7061-7077`.

### Impact

A capture rejection is supposed to mean “this trajectory is not made by the selected TP target model,” not “replace the open mouth with an ordinary cushion.” The current geometry can:

- produce an early `BallRailImpact` at the center of an open side or corner aperture;
- apply the wrong contact normal, restitution/friction profile, tangential impulse, and spin response;
- prevent a later nose/facing/wall rattle that the pocket literature describes;
- change every downstream event because simulation advances and resolves the wrong first contact; and
- expose the wrong event in DSL shot traces as an ordinary rail impact.

This is a geometry defect, not a tie-breaking defect: in the reproducer the false rail event is genuinely earlier than the real jaw candidate because the complete plane lies in front of the pocket interior.

## Exact affected files, symbols, and callers

### Production code

- `src/lib.rs:1549-1582`
  - `PredictedBallRailImpact`
  - `PocketJaw`
  - `PredictedBallJawImpact`
  - `PredictedBallPocketCapture`
- `src/lib.rs:1739-1902`
  - `NBallPocketAwareSystemEventSource`
  - `NBallPocketAwareSystemEventCandidateRef`
  - candidate `source`, `time_seconds`, and `to_event` mappings
- `src/lib.rs:1961-2257`
  - `PocketAwareEventCache::{build,refresh_ball,next_event}`
  - `jaw_impacts`, `pocket_captures`, and `rail_impacts`
- `src/lib.rs:5834-5966`
  - `rail_collision_gap_quadratic_coefficients`
  - `first_rail_collision_time_during_current_phase_raw`
  - `compute_next_ball_rail_impact_on_table`
- `src/lib.rs:5968-6083`
  - `pocket_center_in_inches`
  - `pocket_mouth_width_in_inches`
  - `pocket_face_coordinates_in_inches`
  - `pocket_jaw_associated_rail`
  - `ResolvedPocketJawGeometry`
  - `pocket_jaw_reference_point_in_inches`
  - `pocket_jaw_geometry_in_inches`
- `src/lib.rs:6770-6957`
  - `pocket_slow_target_bounds_in_inches`
  - `FastPocketTargetGeometry`
  - `fast_pocket_target_geometry`
  - `fast_pocket_target_sleft`
  - `pocket_fast_target_bounds_in_inches`
  - `pocket_target_bounds_in_inches`
- `src/lib.rs:7061-7181`
  - side/corner wall, hole, shelf, and entry-angle constants
  - `pocket_entry_axis`
  - `pocket_acceptance_gap_raw`
  - `pocket_mouth_plane_gap_raw`
  - `pocket_back_plane_gap_raw`
- `src/lib.rs:8288-8647`
  - pocket capture scanning/refinement
  - `compute_next_ball_jaw_impact_on_table`
  - `compute_next_ball_pocket_capture_on_table`
- `src/lib.rs:10728-11221`
  - `compute_next_n_ball_system_event_with_rails_and_pockets_on_table`
  - `resolve_n_ball_system_event_with_physics_and_pockets_on_table`
  - every `advance_to_next_n_ball_system_event_with_*pockets*` wrapper
  - `simulate_n_ball_system_with_physics_and_pockets_on_table_until_{rest,event_limit}`
  - every `simulate_n_ball_system_with_*pockets*` and `simulate_n_balls_with_*pockets*` wrapper
- `src/lib.rs:13843-14183`
  - `Pocket`, `PocketJawGeometry`, `PocketShapeSpec`, `PocketSpec`
  - `TableSpec::{brunswick_gc4_9ft,brunswick_gc4_corner_pocket,brunswick_gc4_side_pocket,disabled_corner_pocket,disabled_side_pocket,with_pocket_shape}`
  - existing TP B.15 conversion helpers at `src/lib.rs:13944-13998`
- `src/dsl.rs:1032-1139` and `:1415-1523`
  - `ScenarioShotTraceEventKind`
  - `ScenarioShotTraceEventKind::format_human`
  - `scenario_event_involves_ball`
  - `scenario_event_kind_from_system_event`

### Direct rail-only callers that must receive an explicit compatibility decision

`compute_next_ball_rail_impact_on_table` is also called outside the pocket-aware cache:

- airborne planar scheduling in `PocketAwareEventCache::refresh_ball` at `src/lib.rs:2039-2041`;
- `compute_next_single_ball_event_with_rails_on_table` at `src/lib.rs:8685-8705`;
- `select_earliest_n_ball_event_from_states` at `src/lib.rs:8834-8895`;
- `PostContactContinuation::next_rail_impact` at `src/lib.rs:12311-12319`;
- direct scheduling regressions in `tests/rail_event_scheduling.rs`.

The direct rail predictor must become physically truthful: it returns only ordinary-cushion contacts and returns `None` when the first complete-plane root lies in a pocket aperture. Rail-only higher-level APIs continue to model only rails and motion transitions; their rustdoc must state that callers using a pocketed `TableSpec` need the pocket-aware APIs for continuation through an aperture. Do not retain a hidden “full rail” fallback for those callers.

### Tests and trace consumers

- `tests/n_ball_pockets.rs:638-1105` covers side target rejection, center capture, jaw impacts, rejected jaw paths, and jaw-shape injection but not rail cutouts.
- `tests/n_ball_pockets.rs:1550-1596` proves a successful center entry has no right-rail event; it does not cover a rejected entry.
- `tests/table_and_pocket_geometry.rs:21-178` covers nominal dimensions, injected jaw shape, and TP B.15 round trips; it lacks finite rail/facing topology assertions.
- `tests/rail_event_scheduling.rs:41-150` covers ordinary rail-plane timing and zero-time behavior away from pockets; these are controls that must remain unchanged.
- `tests/scenario_examples.rs:879-880` treats jaw/capture trace kinds as pocket interactions and must recognize the new generalized pocket-boundary feature.

## Whitepaper and corpus evidence

### TP 3.7: rejected fast side entries interact with a point/wall, not a rail spanning the hole

Primary source: `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf`.

- The stated assumption is a ball pocketed after three wall rattles, with equal approach/rebound angles (`pdf` extracted lines 12-15).
- The model defines `R=1.125 in`, mouth width `p=5.0625 in`, wall angle `α=14°`, hole radius `R_hole=3 in`, and shelf depth `b=0.1875 in` (`pdf` lines 36-40; generated corpus `agent_knowledge/whitepapers_formula_candidates.txt:2071-2074`).
- Its fast-entry boundary is the root of

  \[
  \operatorname{poly}(p,\alpha,b,r,\theta)
  =p\cos\alpha-R-B(p,\alpha,b,r,\theta)\sin(\theta+\alpha)-r\cos(\theta+\alpha)=0,
  \]

  with `r=R`, giving

  \[
  \theta_{\max}=50.688^\circ.
  \]

  See the PDF’s “Maximum angle” construction at extracted lines 68-102 and the generated anchors at `agent_knowledge/whitepapers_formula_candidates.txt:2084-2096`.
- The target is explicitly piecewise between a near-point branch, a point/inner-wall branch, and an inner-wall branch:

  \[
  s_{\text{left}}(\theta)=
  \begin{cases}
  s_{\text{left,point}}, & \theta\ge\alpha,\\
  s_{\text{left,point-wall}}, & \theta_c\le\theta<\alpha,\\
  s_{\text{left,wall}}, & \text{otherwise}.
  \end{cases}
  \]

  See `agent_knowledge/whitepapers_formula_candidates.txt:2089-2103`. The modeled solids after a rejected/near-boundary entry are a point and pocket walls; no term represents an ordinary cushion crossing the mouth.

TP 3.8 provides the mirrored corner control: `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf`, with generated anchors `agent_knowledge/whitepapers_formula_candidates.txt:2114-2147`, uses the same point/wall construction and gives `θmax=59.841°` for its stated corner parameters.

### TP B.15 and BU Part IV: mouth, throat, facing, and shelf are distinct measured geometry

Primary geometry source: `whitepapers/tp_b_15_pocket_geometry_calculations.pdf`.

TP B.15 defines the cushion wedge/facing relationships (`pdf` extracted lines 42-120; generated anchors `agent_knowledge/whitepapers_formula_candidates.txt:3291-3313`):

\[
\beta=180^\circ-\alpha,\qquad
\theta=\alpha-135^\circ,\qquad
\beta=45^\circ-\theta,
\]

\[
r=\frac{c}{\sin\beta},\qquad
\frac{m-t}{2}=r\sin\theta,
\]

and therefore

\[
\alpha=135^\circ+
\arctan\!\left(\frac{1}{1+2\sqrt 2\,c/(m-t)}\right).
\]

Here `m` is the mouth width, `t` is the throat width, and `c` is the reference setback used for the throat measurement. These equations justify deriving finite facing endpoints from measured mouth/throat geometry; they do not justify an infinite face plane.

`whitepapers/billiard_university_bu_part_iv_table_difficulty.pdf` distinguishes mouth size, throat size, facing angle, and shelf depth (`pdf` lines 21-33). It states that throat size is measured 2 in back from the cushion noses when the cushion is not 2 in thick, and shelf depth is measured from the pocket mouth line to the slate lip (`pdf` lines 27-33). Therefore `PocketSpec.depth` must not silently serve as both throat setback/facing length and shelf depth.

### Qualitative observed behavior

`whitepapers/just_how_big_are_the_pockets_anyway_part_i.pdf` is derivative explanatory evidence, not the numeric authority:

- Principle 8 defines pocket opening “from point to point across the mouth” (`pdf` lines 135-167).
- Principle 10 and Diagrams 5-6 say a fast ball contacting the near point receives sidespin and can rattle out; the listed high-speed clips also distinguish near-point, far-wall, and wall-rattle misses (`pdf` lines 174-227).

This supports the event classification and qualitative invariant: a rejected ball in the opening meets a nose or pocket wall, not a straight cushion spanning the opening.

## Source limits

- TP 3.7 and TP 3.8 are analytic target models. TP 3.7 explicitly assumes equal incidence/reflection angles and a three-wall sequence. They do **not** empirically calibrate restitution, friction, nose radius, cushion compliance, vertical extent, or a particular Brunswick GC IV.
- TP 3.7 uses `p=5.0625 in`, while the current `TableSpec::brunswick_gc4_9ft` side mouth is `5.000 in`. Preserve the configured table width for collision geometry; do not silently replace it with the TP example width.
- TP B.15 is two-dimensional measurement geometry. It determines a facing line from mouth/throat/setback values but does not specify impact response or curved-nose construction.
- BU Part IV’s generic “Brunswick Gold Crown” example does not establish factory GC IV dimensions. Any preset facing/throat values derived from the currently used 14° side and 7° corner analytic angles must be labeled model-derived, not measured GC IV calibration.
- “Just How Big … Part I” is qualitative/high-speed-video context. It must not replace TP 3.7/3.8 for equations or become evidence for an exact response coefficient.
- This plan remains a 2-D on-table boundary correction. Vertical geometry is owned by the airborne-boundary sibling plan.

## Numeric reproducer with units

Use the exact current default-table fixture below in `tests/n_ball_pockets.rs`.

- Playfield: `W=50 in`, `H=100 in`.
- Pocket: `Pocket::CenterRight`, physical face `x=50 in`, mouth center `(50,50) in`, configured mouth width `p=5.000 in`.
- Ball radius: `R=1.125 in`.
- Rolling deceleration: `a=5 in/s²`.
- Initial center:

  \[
  (x_0,y_0)=(46.375,45.6698729811)\ \mathrm{in}.
  \]

- Initial planar velocity, speed `v_0=80 in/s`, local angle `60°` from the right-pocket entry axis:

  \[
  (v_x,v_y)=80(\cos60^\circ,\sin60^\circ)
  =(40,69.2820323028)\ \mathrm{in/s}.
  \]

- Pure-rolling angular velocity:

  \[
  (\omega_x,\omega_y,\omega_z)
  =(-v_y/R,v_x/R,0)
  =(-61.5840287136,35.5555555556,0)\ \mathrm{rad/s}.
  \]

The complete right-rail center plane is `x=W-R=48.875 in`. The trajectory reaches that plane at the center of the open mouth, `(48.875,50) in`, after exactly `s=5.000 in` of path. Under constant rolling deceleration,

\[
s=v_0t-\tfrac12at^2,
\qquad
t=\frac{v_0-\sqrt{v_0^2-2as}}{a},
\]

so the false rail candidate is

\[
t_{\text{phantom rail}}=0.0626225495\ \mathrm{s}.
\]

At `80 in/s`, the current target interpolation is fully in its fast branch (`80>60 in/s`), and TP 3.7/current capture rejects the entry because `60°>50.688°`.

For comparison, the current upper-jaw center is `(50,52.5) in`; the default physical jaw radius is `0.125 in`, so the current ball-center contact radius is `R+r_j=1.250 in`. The same path first reaches that circle after approximately `6.508325 in`, at

\[
t_{\text{jaw}}=0.0815619502\ \mathrm{s}.
\]

The observed event scheduler therefore selects `BallRailImpact { rail: Right }` at the open mouth before a real nose/facing contact. After this plan, the `0.0626225495 s` plane root must be discarded by aperture membership; capture remains rejected; the first boundary event must be a later explicit pocket nose/facing/wall contact, with no intervening ordinary right-rail impulse.

## Root cause

The implementation composes three independently predicted abstractions—complete rail planes, jaw circles, and a target/capture region—rather than deriving events from one partitioned physical boundary. A complete rail plane has no representation of mouth intervals. The jaw predictor adds solids but never subtracts the rail under them, and capture rejection only removes the capture candidate. Chronological selection is behaving consistently with the candidates it is given; the candidate geometry is inconsistent.

A secondary data-model cause is that `PocketShapeSpec` has no facing/throat geometry. Even after filtering rail roots inside mouth intervals, a rejected entry would have only two full jaw circles and no explicit line/arc representing the inside facing/wall.

## Proposed geometry and invariants

Everything in this section is proposed design, not observed code.

### Store physical surfaces, derive center-contact geometry per ball

The geometry source of truth must be **uninflated physical geometry in inches**. Do not cache one pool-ball-radius offset in `TableSpec`, because `BallSetPhysicsSpec.radius` is variable.

Resolve each table profile once into finite primitives such as:

```rust
struct ResolvedTableBoundaryGeometry {
    rail_faces: Vec<ResolvedRailFaceSegment>,
    pocket_boundaries: [ResolvedPocketBoundaryGeometry; 6],
}

enum ResolvedPocketJawGeometry {
    Point {
        mouth_tip: Point2,
    },
    RoundedArc {
        mouth_tip: Point2,
        center: Point2,
        nose_radius: f64,
        rail_boundary_normal: Vector2,
        facing_boundary_normal: Vector2,
        sweep: ArcSweep,
    },
}

struct ResolvedPocketBoundaryGeometry {
    jaws: [ResolvedPocketJawGeometry; 2],
    facings: [LineSegment2; 2],
    inner_walls: [LineSegment2; 2],
    capture_boundary: ResolvedPocketCaptureBoundary,
}
```

The exact internal container may use fixed-size arrays rather than heap-backed `Vec`s; the essential contract is finite physical segments/arcs with stable feature identity.

For a physical segment

\[
S=\{a+s(b-a)\mid 0\le s\le1\},
\]

and ball-center position `q`, contact is the boundary of the Minkowski sum

\[
S\oplus B_R=\{x+y\mid x\in S,\ \lVert y\rVert\le R\},
\]

or equivalently `distance(q,S)=R` with a closing normal velocity. For a flat face with inward unit normal `n`, the interior center-contact locus is

\[
S_R=\{a+s(b-a)+Rn\mid0<s<1\}.
\]

For the right rail, `n=(-1,0)`, which retains `x=50-R`, but only over retained finite face segments. A physical circular jaw arc of radius `r_j` becomes a center-contact arc of radius `r_j+R`. A line facing/wall is offset by `R` along its playable-side normal.

Clip the **physical** rail segments at mouth-tip/tangent boundaries first, then form the Minkowski offset of the union. Do not implement a guessed `mouth_width ± 2R` interval on the center plane: clipping and Minkowski inflation do not commute at endpoints, and the endpoint cap belongs to the explicit jaw/nose arc. At a shared rail/nose/facing seam, use a half-open ownership convention or feature-priority rule so the union is continuous but no contact is emitted twice.

Required topology invariants:

1. Every ordinary rail face terminates at a physical mouth boundary; no ordinary face exists across the aperture.
2. Every retained rail endpoint joins exactly one jaw/nose arc, which joins exactly one facing.
3. Every facing joins the configured throat/inner wall without a positive gap or overlap larger than geometric tolerance.
4. The ball-center forbidden set is the Minkowski sum of that physical union for the queried `R`.
5. A root is executable only if its closest physical feature and parameter lie in that feature’s finite exposed domain.
6. A capture candidate and a solid-boundary candidate are selected from noncontradictory geometry and compared by time; capture rejection never mutates the solid set.

### Pocket-facing data model

Extend `PocketShapeSpec` with explicit measured/model geometry instead of reusing `PocketSpec.depth`:

```rust
pub struct PocketFacingSpec {
    pub wall_angle_degrees: f64,
    pub throat_setback: Inches,
}

pub struct PocketShapeSpec {
    pub jaw_geometry: PocketJawGeometry,
    pub facing: PocketFacingSpec,
}
```

`throat_setback` is the mouth-to-throat reference distance; `PocketSpec.depth` remains shelf depth and should be renamed `shelf_depth` in the same clean cutover if its public meaning is confirmed. Derive throat endpoints from physical mouth tips, pocket-local entry/tangent axes, `wall_angle_degrees`, and `throat_setback`. Expose constructors that accept either `(wall_angle_degrees, throat_setback)` or measured `(mouth_width, throat_width, throat_setback)` and use the existing TP B.15 helpers to validate/round-trip the resulting angle where applicable.

For current presets, use the already selected analytic wall angles—14° side and 7° corner—and the BU/TP B.15 2.0 in throat-measurement setback to build a deterministic model profile. Document these as model-derived defaults, not measured Brunswick GC IV values. Disabled carom pockets produce no cutouts, jaws, facings, walls, or capture boundary.

The physical mouth-tip and rounded-arc construction is shared with `plans/pocket-rounded-jaw-mouth-width.md`. That plan owns preserving point-to-point mouth width and locating rounded-nose arc centers; this plan consumes those physical tips/arcs to terminate rails and begin facings. There must be one resolver, not parallel “rail mouth endpoint” and “jaw mouth endpoint” conventions.

### Event/API cutover

Generalize jaw-only impact identity to cover every pocket solid:

```rust
pub enum PocketBoundaryFeature {
    Nose(PocketJaw),
    Facing(PocketJaw),
    InnerWall(PocketJaw),
}

pub struct PredictedBallPocketBoundaryImpact {
    pub pocket: Pocket,
    pub feature: PocketBoundaryFeature,
    pub time_until_impact: Seconds,
    pub state_at_impact: OnTableBallState,
    // Internal resolved contact normal/feature parameter, if needed by response.
}
```

Replace, without aliases or deprecated compatibility variants:

- `PredictedBallJawImpact` → `PredictedBallPocketBoundaryImpact`;
- `compute_next_ball_jaw_impact_on_table` → `compute_next_ball_pocket_boundary_impact_on_table`;
- `NBallSystemEvent::BallJawImpact` → `BallPocketBoundaryImpact`;
- `NBallPocketAwareSystemEventSource::BallJawImpact` and candidate-ref variant likewise;
- `PocketAwareEventCache::jaw_impacts` → `pocket_boundary_impacts`;
- `collide_ball_jaw_on_table_with_radius_and_profile` → a boundary-feature resolver that uses the predicted physical normal and the selected pocket/rail response profile;
- DSL `ScenarioShotTraceEventKind::BallJawImpact` → `BallPocketBoundaryImpact { pocket, feature }`, with human labels that distinguish “nose,” “facing,” and “inner wall.”

`PredictedBallRailImpact` and `NBallSystemEvent::BallRailImpact` remain for retained ordinary cushion faces. `compute_next_ball_rail_impact_on_table` must enumerate finite rail segments or validate each plane root against them and return only the earliest valid ordinary-rail contact.

The pocket-aware cache must independently cache the earliest true ordinary-rail impact, earliest pocket-boundary impact, capture, transition, and other current event classes, then choose the minimum physical time. For the 60° fixture:

1. the right-plane root at `0.0626225495 s` is rejected as lying in the center-right aperture;
2. capture has no candidate because `60°>50.688°` in the fast target;
3. the later nose/facing/wall root is retained; and
4. that solid contact is resolved with its actual normal.

Truly coincident seam roots within `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` must collapse to one physical feature by the geometry ownership rule. Do not introduce a new macroscopic time preference. Removal of the existing 5 ms jaw/capture override belongs to the sibling causality plan; this plan must be compatible with strict chronological selection.

## Explicit non-goals

- Do not recalibrate GC IV mouth, throat, shelf, hole, jaw radius, restitution, or friction from the generic BU table row.
- Do not replace or retune the TP 3.5-3.8 target formulas, the 60 in/s slow/fast interpolation, or capture/drop dynamics except where needed to consume the same resolved mouth/facing geometry.
- Do not implement a deformable cushion, speed-dependent pocket restitution, empirical rattle success model, or arbitrary multi-bounce pocket integrator.
- Do not solve airborne rail/jaw/pocket geometry; that work must consume these physical primitives in the dedicated 3-D plan.
- Do not fix rounded-nose mouth-width shrinkage independently of `plans/pocket-rounded-jaw-mouth-width.md`; consume its physical tip/arc contract.
- Do not solve the 5 ms jaw-over-capture causality defect here; only avoid introducing additional ordering exceptions.
- Do not change ordinary rail response on retained rail segments away from pocket mouths.
- Do not edit whitepaper PDFs or hand-edit generated `agent_knowledge` artifacts.

## Phased implementation plan

### Phase 1 — Lock the defect with failing end-to-end tests

1. Add an exact fixture helper in `tests/n_ball_pockets.rs` using the 60° state and units above, including rolling spin and `5 in/s²` rolling deceleration.
2. Assert the fixture’s preconditions: speed `80 in/s`, signed center-right entry angle `60°`, capture predictor returns `None`, and the geometric mouth-plane point is `(48.875,50) in` at `0.0626225495 s` under the phase motion equation.
3. Add the failing event assertion: `compute_next_n_ball_system_event_with_rails_and_pockets_on_table` must not return `BallRailImpact(Right)` at the mouth. Once explicit facings exist, require the first event to be `BallPocketBoundaryImpact` for `CenterRight`, with time greater than the discarded mouth-plane time, feature `Nose(_)` or `Facing(_)` as dictated by the resolved geometry, and no capture.
4. Advance/resolve that event and assert the output velocity is separated from the actual feature normal, not mirrored about the right-rail normal. This catches an implementation that merely relabels the phantom event.
5. Preserve a centered admissible fast side entry control: it is captured and has no ordinary right-rail or pocket-boundary event first.

### Phase 2 — Specify physical boundary topology and Minkowski tests

1. Add focused `tests/table_and_pocket_geometry.rs` assertions for the default table’s finite physical topology: three right-rail face segments separated by bottom-corner, center-side, and top-corner apertures; corresponding segments for all four rails; each rail endpoint joined to the same physical mouth tip used by a nose/facing.
2. Test the physical-to-center-locus transform at `R=1.125 in` and a second nondefault radius. Assert right-face interior contacts stay at `x=50-R`; nose arc contact radius is `r_j+R`; facing offsets are exactly `R` along their inward normals.
3. Add seam tests at each rail/nose and nose/facing junction. A closing trajectory at the seam produces one candidate, never zero and never two.
4. Validate geometry inputs: finite positive throat setback, finite wall angle in the supported open interval, no inverted throat, and no crossing facings. Disabled pockets resolve to a continuous full rail with no aperture.

### Phase 3 — Introduce one resolved physical geometry model

1. Add the uninflated resolved boundary primitives and a `TableSpec` resolver in `src/lib.rs`. Use fixed-size arrays/small enums where cardinality is known; avoid allocating per event query.
2. Build ordinary cushion-face segments by subtracting all physical mouth intervals from each rail. Side pockets split one rail around a centered aperture; corner pockets clip both adjoining rails at their respective mouth tips.
3. Derive facing endpoints from mouth tips plus `PocketFacingSpec`; derive inner-wall segments only through the capture/throat region supported by configured geometry. Store stable feature identity and playable-side normal orientation.
4. Reuse the mouth-tip/nose-arc resolver from `plans/pocket-rounded-jaw-mouth-width.md`. Delete duplicate endpoint derivations after all callers migrate.
5. Add debug/test-only topology validation for finite coordinates, connected seams, nonnegative lengths/radii, no ordinary rail inside an aperture, and consistent inward normals.

### Phase 4 — Replace complete-plane rail roots with finite-feature roots

1. Keep the existing quadratic plane root as a fast broad-phase for each axis-aligned ordinary face, but accept a root only when the state-at-root projects into a retained segment’s exposed interior.
2. Add analytic closest-feature roots for offset line segments and exposed arcs. Reject roots outside the segment/arc parameter range, roots beyond the current motion-phase horizon, and grazing/separating zero-time contacts under the existing closing-contact policy.
3. At endpoints, route ownership to the connected nose/facing feature rather than emitting an artificial full-circle cap for an ordinary rail segment.
4. Update `compute_next_ball_rail_impact_on_table` to return the earliest retained ordinary-face contact only.
5. Implement `compute_next_ball_pocket_boundary_impact_on_table` over nose, facing, and inner-wall primitives, preserving curved within-phase trajectories and current transition horizons.

### Phase 5 — Cut over events, resolution, and all callsites

1. Introduce `PocketBoundaryFeature`, `PredictedBallPocketBoundaryImpact`, and `NBallSystemEvent::BallPocketBoundaryImpact`; migrate all `time`, `source`, `primary_ball`, clone/equality, and event-candidate mappings.
2. Replace `PocketAwareEventCache::jaw_impacts` with `pocket_boundary_impacts`; schedule the new predictor for on-table states wherever jaw impacts are currently scheduled. Coordinate the airborne branch with the 3-D sibling plan rather than flattening new boundary primitives.
3. Resolve a pocket-boundary impact using its actual physical normal. Preserve current rail-profile selection by mapping the feature to its associated cushion/pocket profile; do not infer every facing normal from `pocket_jaw_associated_rail`.
4. Migrate all `advance_*with_*pockets*`, `simulate_*with_*pockets*`, event-limit, and until-rest paths through the changed event variant. Confirm cache rebuild after resolution sees the updated boundary contact and does not loop at zero time.
5. Migrate `src/dsl.rs` trace involvement, event conversion, and human formatting. Update `tests/scenario_examples.rs` so traces visibly distinguish ordinary rails from nose/facing/wall impacts.
6. Update direct rail-only caller rustdocs and tests to state the clipped-predictor contract. Do not add an unexported full-plane fallback.
7. Remove obsolete jaw-only types, variants, functions, fields, comments, and imports in the same cutover; leave no aliases or deprecated paths.

### Phase 6 — Complete symmetry, boundary, and integration coverage

1. Parameterize the 60° side fixture under exact table symmetries:
   - horizontal reflection: `CenterRight → CenterLeft`, `x→50-x`, `v_x→-v_x`, with angular velocity recomputed from the reflected rolling velocity;
   - vertical reflection: upper/far side of `CenterRight` to lower/near side, `y→100-y`, `v_y→-v_y`;
   - combined reflection for both sides of both side pockets.
   Every transform must reject the phantom ordinary rail and select the correspondingly mirrored pocket-boundary feature at the same time within numeric tolerance.
2. Add representative corner-aperture controls for all four corners. A centered diagonal entry reaches the simultaneous complete-plane point—for `TopRight`, `(50-R,100-R)=(48.875,98.875) in`—inside the corner opening and must never emit `BallRailImpact(Top|Right)` there. Mirror the state to the other three corners and require equal event times/features under symmetry.
3. Add corner rejected-entry controls outside TP 3.8’s `59.841°` fast target, requiring a later explicit corner nose/facing/wall event rather than either adjoining ordinary rail.
4. Add an ordinary-rail control clearly outside each mouth, not at an ambiguous Minkowski seam. For example, a rightward trajectory at `y=54.0 in` must still return `BallRailImpact(Right)` at `x=48.875 in` with the existing timing and response.
5. Add just-inside/just-outside aperture boundary pairs using a scale-aware geometric tolerance. The inside case cannot hit the ordinary face; the outside case does. Perturbations must not create gaps, double events, negative times, or zero-time loops.
6. Run an until-rest rejected-entry simulation and assert observable invariants: event times are nondecreasing, no event places the ball beyond a solid without contact, no ordinary rail event occurs in any aperture, capture remains absent for the 60° fixture, and the ball remains a live on-table state after the first rejected pocket-boundary response.

## Regression and acceptance tests

The implementation is accepted only if all of the following observable contracts hold:

1. **Exact center-right reproducer:** the specified 60°/80 in/s rolling fixture rejects capture; the complete-plane point `(48.875,50) in` at `0.0626225495 s` is not an executable `BallRailImpact(Right)`; a later explicit pocket-boundary event occurs and resolves with its own normal.
2. **Admissible center entry:** a centered entry within the TP target is captured, with no phantom rail or premature facing event.
3. **Outside-mouth rail:** a trajectory whose physical closest point lies clearly on retained cushion remains an ordinary rail impact with unchanged timing/response.
4. **Minkowski radius:** changing ball radius moves every flat contact by exactly the radius change along the normal and changes nose radius from `r_j+R_1` to `r_j+R_2`; physical mouth-tip coordinates do not move.
5. **Finite-domain enforcement:** a line/arc root outside its exposed parameter interval is not accepted.
6. **Seam uniqueness:** rail/nose/facing junction contacts produce exactly one event under closing motion.
7. **Mirrored sides:** CenterRight/CenterLeft and upper/lower approach reflections produce mirrored feature identity, position, normal, and equal event time.
8. **Mirrored corners:** all four corner-aperture controls suppress both phantom adjoining rails and preserve reflection symmetry.
9. **Chronology:** capture, pocket-boundary, rail, ball-ball, table-contact, and transition candidates remain ordered by physical time; this change adds no time override.
10. **No regression away from pockets:** current top/right/bottom/left rail scheduling tests away from apertures retain their times, zero-time closing policy, and collision response.
11. **Trace fidelity:** DSL/scenario traces report “nose,” “facing,” or “inner wall” for pocket-solid impacts and never call the center of an aperture an ordinary rail.
12. **Carom continuity:** `TableSpec::three_cushion_carom_10ft` retains four continuous rails and emits no pocket-boundary/capture candidates.

Tests must assert event variants, physical positions, times, and post-impact normal separation. A source-text assertion, snapshot of implementation details, or mere “is_some” check is insufficient.

## Risks and edge cases

- **Minkowski seam gaps/overlap:** independently inflating clipped rails and nose/facing primitives can create a tiny gap or duplicate forbidden region. Build and offset one connected physical union with explicit seam ownership.
- **Tangency and zero-time loops:** a ball at a shared seam can be reported by two features or repeatedly collide without state change. Require closing normal velocity, deterministic feature ownership, and existing zero-time progress guards.
- **Rounded-nose ownership:** a full jaw circle is not an exposed physical arc. Accepting its hidden half can create new phantom impacts. Restrict roots to the resolved exposed arc from rail tangent to facing tangent.
- **Curved within-phase motion:** sliding/rolling acceleration can make a trajectory intersect a finite feature after an earlier broad-phase plane root was rejected. Search every relevant feature through the full current phase horizon; do not stop globally at the first invalid complete-plane root.
- **Large/nonstandard ball radii:** an inflated ball can bridge an aperture or contact both sides. Geometry must remain valid and event collection must not assume a pool-ball radius. True simultaneous multi-surface contacts overlap the shared-contact solver plan.
- **Corner double ownership:** a corner aperture touches two rails. The centered diagonal can yield equal roots on both complete planes; both must be discarded before candidate tie-breaking.
- **Uncalibrated defaults:** 14°/7° plus a 2 in setback is a deterministic analytic profile, not measured GC IV truth. Surface this in rustdoc and keep future profile calibration possible without changing event semantics.
- **Public API break:** replacing jaw-only event/type names affects Rust consumers and DSL trace tests. Perform a repository-wide clean cutover; no alias can safely preserve the old semantic claim that every pocket solid is a jaw circle.
- **Rail-only API behavior:** a clipped direct rail predictor may return `None` through an aperture. Document and test that callers wanting pocket continuation must use pocket-aware APIs rather than resurrecting full rails.
- **Capture overlap:** if capture and a facing occupy overlapping geometry, event conflicts will persist. Validate topology and rely on strict chronology; coordinate with the causality plan rather than adding a new tolerance.
- **Airborne extrusion:** the 2-D primitives need physical vertical extents before sphere contact is complete. Keep the 2-D source geometry reusable and leave vertical scheduling to the airborne plan.

## Source, documentation, and generated-artifact updates

- Update rustdoc on `compute_next_ball_rail_impact_on_table` from “four ideal rail planes” to finite ordinary cushion faces with pocket apertures.
- Document `PocketFacingSpec` units, angle convention, throat setback, validation, and the distinction between model-derived presets and measured table calibration.
- Update `NBallSystemEvent`, predicted-impact, pocket-aware advance/simulate, and DSL trace docs to distinguish ordinary rail, pocket nose, facing, inner wall, and capture.
- Update comments near TP 3.7/3.8 constants so target formulas and physical collision geometry use the same angle convention without claiming the analytic target paper calibrated impact response.
- Update existing scenario-event human strings and any user-facing trace expectations affected by the clean event rename.
- Do not modify any whitepaper PDF. No source-corpus membership changes are required.
- Do not hand-edit `agent_knowledge/whitepapers_formula_candidates.txt` or other generated artifacts. Because no whitepaper source changes, regeneration is not required; if the project’s normal source-index generator includes rustdoc metadata, run that generator rather than editing its output and verify no unrelated corpus churn.

## Dependencies and overlap

- **`plans/pocket-rounded-jaw-mouth-width.md` — “Preserve Physical Pocket Mouth Width with Rounded Jaw Arcs”:** direct geometry dependency/overlap. That plan owns mouth-tip semantics and the canonical uninflated `ResolvedPocketJawGeometry::{Point { mouth_tip }, RoundedArc { mouth_tip, center, nose_radius, rail_boundary_normal, facing_boundary_normal, sweep }}` contract. This plan consumes that enum’s tip, boundary normals, and exposed sweep to terminate rails and begin facings; it owns explicit facings/walls, Minkowski feature domains, and event migration. Implement one shared resolver and no parallel nose geometry.
- **`plans/jaw-capture-event-causality.md` — “Restore Causal Jaw/Capture Event Ordering”:** the strict chronological comparator/removal of the 5 ms jaw-over-capture override is complementary. It must consume this plan’s `ResolvedTableBoundaryGeometry` and each pocket’s capture boundary rather than invent a second surface model. This plan supplies nonoverlapping solid/capture geometry and must not duplicate selector/tie semantics or defer the cutout fix to comparator changes.
- **`plans/airborne-table-boundary-geometry.md` — “Stop Flattening Airborne Balls into Table-Boundary Events”:** it consumes this plan’s canonical uninflated finite XY rail/nose/facing/wall primitives, extrudes them with physical vertical extents, and owns full-state airborne contact/exit diagnostics. This plan does not flatten or solve airborne contacts and must not persist an `R`-inflated geometry that the 3-D predictor cannot reuse.
- **`plans/coupled-nonideal-shared-contacts.md` — “Couple non-Ideal shared ball contacts without creating energy”:** this sibling owns coupled non-ideal shared ball-ball impulse resolution and has no direct cutout dependency. A future ball-plus-pocket-boundary contact solver may consume this plan’s actual feature normals, but this plan must not expand that sibling’s scope or duplicate seam candidates.
- `PHYSICS_ENGINE_PLAN.md:779-811`, Phase 6, broadly names `Made`, `Jawed`, `Rejected`, and `CrossedFace` outcomes but does not identify or solve the phantom full-rail geometry.
- `physics_audits/2026-04-24-rail-pocket.md:97-101` notes missing explicit walls and simplified jaw response. It is supporting historical context, not a substitute for this cutout plan.
- `plans/performance_engineering.md:125-178` proposes cached raw table/pocket geometry. If implemented concurrently, the finite uninflated primitives should be resolved once into that raw geometry rather than rebuilt per predictor call; correctness and feature ownership take precedence over the cache layout.

## Verification commands

Run focused regressions first, using the final test names introduced by the implementation:

```bash
cargo test --test n_ball_pockets rejected_fast_side_entry_crossing_mouth_uses_facing_not_phantom_rail
cargo test --test n_ball_pockets mirrored_side_and_corner_apertures_never_emit_phantom_rails
cargo test --test table_and_pocket_geometry finite_rail_cutouts_join_pocket_noses_and_facings
cargo test --test table_and_pocket_geometry pocket_boundary_minkowski_offsets_follow_ball_radius
cargo test --test rail_event_scheduling a_rolling_ball_predicts_a_top_rail_impact_before_it_stops
cargo test --test scenario_examples
```

Then run the complete affected integration suites:

```bash
cargo test --test n_ball_pockets
cargo test --test table_and_pocket_geometry --test rail_event_scheduling --test scenario_examples
```

Finally run the relevant aggregate suite and full repository tests:

```bash
cargo test --lib pocket_mouth_tests
cargo test
```

## Candidate commit message

```text
Clip rails at pocket mouths and model explicit facings

Represent rail, nose, facing, and inner-wall surfaces as one finite physical
boundary and derive ball-center contacts with radius-aware Minkowski offsets.
Reject ordinary rail roots inside side and corner apertures so failed capture
entries continue to real pocket solids instead of phantom full cushions.
```
