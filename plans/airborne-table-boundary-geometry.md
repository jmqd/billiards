# Stop Flattening Airborne Balls into Table-Boundary Events

Date: 2026-07-10

Severity: High  
Priority: P0

Audit evidence: `agent://RailPhysicsAudit` Finding 1 and `agent://PocketGeometryAudit` Finding 1.

## Problem statement

The pocket-aware system scheduler currently converts every `NBallSystemState::Airborne(BallState)` into a synthetic `OnTableBallState` containing only its XY position, XY velocity, and angular velocity. It then runs the ordinary on-table rail, jaw, and pocket-capture predictors against that projection. If a projected event occurs before the next ballistic table contact, the scheduler exposes it as an executable on-table event.

That conversion discards the two state components that determine whether an airborne sphere can touch table-level geometry:

- `BallState.height`, measured relative to the resting center plane; and
- `BallState.vertical_velocity`.

The corresponding rail and jaw resolvers then overwrite the ball with an `OnTable` state, while the capture resolver stores the projected on-table state in `Pocketed`. A projected XY crossing can therefore teleport a ball vertically, erase its vertical momentum, report a cushion or jaw hit while the ball is many inches above the table, or pocket a ball whose sphere has not reached the opening or rim.

The fix must make the type boundary real: executable `BallRailImpact`, `BallJawImpact`, and `BallPocketCapture` events remain on-table events, and no airborne state may be converted into their `OnTableBallState` prediction inputs. Airborne scheduling must use the full ballistic center trajectory against finite physical boundary primitives. Where the repository has no validated three-dimensional response model, the event must be an explicit terminal unsupported-contact or leaves-table diagnostic that preserves the full airborne state rather than invoking an on-table resolver.

## Current behavior and impact

### Observed implementation

`PocketAwareEventCache::refresh_ball` handles an airborne state by first scheduling its next table contact and then constructing:

```rust
OnTableBallState::try_new(BallState::on_table(
    state.position.clone(),
    state.velocity.clone(),
    state.angular_velocity.clone(),
))
```

It passes that projection to all three on-table predictors and retains each projected candidate when its time is no later than the ballistic table-contact time.

| Candidate | Current prediction | Current resolution | Result for an airborne source state |
| --- | --- | --- | --- |
| Rail | `compute_next_ball_rail_impact_on_table` returns `PredictedBallRailImpact { state_at_impact: OnTableBallState, .. }` | `collide_ball_rail_on_table_with_radius_and_profile`, then unconditional `NBallSystemState::OnTable` | Height and vertical velocity are erased; an on-table cushion model is applied outside its precondition. |
| Jaw | `compute_next_ball_jaw_impact_on_table` returns `PredictedBallJawImpact { state_at_impact: OnTableBallState, .. }` | `collide_ball_jaw_on_table_with_radius_and_profile`, followed by `should_capture_after_jaw_impact` | Height and vertical velocity are erased before either rebound or post-jaw capture. |
| Pocket capture | `compute_next_ball_pocket_capture_on_table` returns `PredictedBallPocketCapture { state_at_capture: OnTableBallState, .. }` | Unconditional `NBallSystemState::Pocketed` using the projected state | A high ball can become pocketed from XY entry alone; its actual airborne state is lost. |

The behavior also contradicts the public scheduler documentation, which says airborne balls do not collide with rails, jaws, or pockets until table contact.

`settle_airborne_ball_on_next_table_contact` independently assumes the ballistic `height == 0` root is a cloth contact. It does not classify the XY point as slate, pocket opening/rim, cushion, or exterior. Once projected boundary events are removed, this must also be corrected so a ball that has flown beyond the supported slate is not later bounced by an infinite table plane.

### User-visible impact

All pocket-aware advance and simulation APIs, cached simulation, replay, and DSL shot traces inherit the bad candidate. The trace can label an impossible rail/jaw/capture event, and replay then reproduces the same vertical discontinuity. Because the public event structs carry `OnTableBallState`, downstream exhaustive matches cannot tell that the event originated from an airborne state.

The defect is not a small tolerance problem. The existing fixtures put the ball 12 inches above its resting center plane—more than ten ball radii high—and intentionally assert that the projected rail, jaw, or capture event wins.

## Exact affected files, symbols, and callers

### `src/lib.rs`

- `PredictedBallRailImpact`, `PredictedBallJawImpact`, and `PredictedBallPocketCapture` (`1550-1582`): their state payloads are correctly on-table for their documented predictors, but cannot represent an airborne interaction.
- `NBallPocketAwareSystemEventSource`, `NBallPocketAwareSystemEventCandidateRef::{BallJawImpact,BallPocketCapture,BallRailImpact}`, its `source`, `time_seconds`, and `to_event` mappings (`1740-1900`): these currently have no distinct airborne boundary/exit candidate.
- `PocketAwareEventCache::{build,refresh_ball,next_event}` (`1961-2255`), especially the synthetic projection and all three calls at `2024-2041`.
- `BallState::{height,vertical_velocity}` and its documented height reference (`2770-2853`).
- `advance_airborne_ball`, `time_until_airborne_ball_reaches_table`, `AirborneTableContact`, and `settle_airborne_ball_on_next_table_contact` (`4433-4553`).
- `rail_collision_gap_quadratic_coefficients`, `first_rail_collision_time_during_current_phase_raw`, and `compute_next_ball_rail_impact_on_table` (`5834-5966`): on-table four-plane geometry only.
- `ResolvedPocketJawGeometry`, `pocket_jaw_reference_point_in_inches`, and `pocket_jaw_geometry_in_inches` (`6006-6056`): current XY jaw geometry; the canonical finite arc will come from the sibling jaw plan.
- `pocket_capture_gap_during_current_phase_raw`, `compute_next_ball_jaw_impact_on_table`, and `compute_next_ball_pocket_capture_on_table` (`8288-8647`): on-table XY target/contact predicates only.
- `NBallSystemState`, especially `Airborne` and `Pocketed`, and `as_ball_state` (`2307-2357`). No new nonterminal out-of-table state is needed for the diagnostic cutover.
- `NBallSystemEvent`, `time`, `primary_ball`, and `is_terminal_diagnostic` (`2359-2438`): public event API requiring clean addition of airborne-boundary and leaves-table diagnostics.
- `advance_n_ball_system_without_event` (`10691-10715`): already advances airborne states ballistically and should remain the single source of state-at-event advancement.
- The contradictory rustdoc on `compute_next_n_ball_system_event_with_rails_and_pockets_on_table` (`10722-10735`).
- `resolve_n_ball_system_event_with_physics_and_pockets_on_table` (`10737-10886`), especially the jaw, capture, table-bounce, and rail branches at `10835-10883`.
- All richer-system wrappers and loops at `10888-11224`:
  - `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table`;
  - `advance_to_next_n_ball_system_event_with_rail_profile_and_pockets_on_table`;
  - `advance_to_next_n_ball_system_event_with_rail_config_and_pockets_on_table`;
  - `advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table`;
  - `simulate_n_ball_system_with_physics_and_pockets_on_table_until_rest`;
  - `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`;
  - the rail-profile/config/default system wrappers; and
  - the on-table compatibility wrappers `simulate_n_balls_with_*pockets*_on_table_until_rest`.
- `PocketShapeSpec`, `PocketSpec`, and `TableSpec` (`13900-14199`): current geometry has XY mouth, depth, and jaw-radius data but no explicit vertical boundary profile.

The pure on-table `TwoBallOnTableEvent`, `NBallOnTableEvent`, and single-ball rail APIs are not affected and must not acquire airborne variants.

### `src/dsl.rs`

- `Scenario::ball_traces_from_simulation` (`601-695`), which advances airborne timeline segments and replays each system event through the shared resolver.
- `ScenarioShotTraceEventKind` and `format_human` (`1032-1139`).
- `scenario_event_involves_ball` and `scenario_event_kind_from_system_event` (`1415-1523`).

These exhaustive matches must expose the new terminal diagnostics by ball label and boundary identity rather than mislabel them as ordinary rail/jaw/capture events.

### Tests and documentation

- `tests/n_ball_pockets.rs:267-295`, `airborne_ball_rail_impact_is_scheduled_before_later_table_contact`, currently asserts the wrong executable rail event.
- `tests/n_ball_pockets.rs:297-328`, `airborne_ball_pocket_capture_is_scheduled_before_later_table_contact`, currently asserts the wrong terminal capture.
- `tests/n_ball_pockets.rs:377-407`, `airborne_ball_jaw_impact_is_scheduled_before_later_table_contact`, currently asserts the wrong executable jaw event.
- `tests/n_ball_pockets.rs:330-375` is the existing pattern for a full-state terminal airborne diagnostic and should be reused, not copied into a second convention.
- `tests/dsl.rs` must cover event conversion and human-readable formatting for each new diagnostic category.
- `PHYSICS_TODOS.md:44-54` records the completed airborne ball-ball diagnostic but still describes pre-landing rail/jaw/pocket scheduling as normal current behavior.
- `whitepapers/rail_rebound.md:109-119` correctly documents that the rail solver is a reduced on-table slice; it should explicitly state that airborne boundary contacts are diagnosed rather than passed through that solver.

## Whitepaper and source evidence

### Ballistic height is a real state dimension

The engine itself defines height relative to the resting center plane and advances an airborne ball with

$$
z(t)=z_0+w_0t-\frac{1}{2}gt^2,\qquad
w(t)=w_0-gt,
$$

where `g = 386.08858267716535 in/s²` (`src/lib.rs:2606-2607`, `4433-4469`). Dropping `z_0` and `w_0` before a boundary query changes the trajectory; it is not a coordinate conversion.

The jump-shot sources independently establish the observable consequence:

- `whitepapers/veps_gems_part_xv_the_jump_shot.pdf:53-77`, Diagrams 1 and 2: greater elevation makes the cue ball gain height sooner, and greater speed makes it jump higher and longer.
- `whitepapers/jump_shots_made_simple.pdf:49-59`, Diagrams 1 and 2: a cue or object ball can remain airborne long enough to hop off the table.
- `whitepapers/draw_shot_physics_part_iv_cue_elevation_effects.pdf:29-40`: the ball remains airborne without cloth friction; longer hops can reach another ball while still airborne, changing the cut angle.

These are qualitative/instructional sources rather than table-profile calibrations, but they directly refute treating an airborne XY projection as a table-level sphere.

### The current rail response has an on-table contact precondition

Mathavan, Jackson, and Parkin model a real cushion contact, not an arbitrary XY plane crossing:

- `agent_knowledge/whitepapers_corpus.txt:2260-2274`: the cushion has a sloped, fixed cross-section and the analysis assumes point contact with the sphere.
- `agent_knowledge/whitepapers_corpus.txt:2297-2305`: the cushion slope constrains vertical center motion in the analyzed on-table impact, so the derivation sets $\dot z_G=0$.
- `agent_knowledge/whitepapers_corpus.txt:2312-2319`: for pool/snooker, the contact-point height above the cloth is

$$
h_{nose}=\frac{7R}{5},\qquad \sin\theta=\frac{2}{5}.
$$

For `R = 1.125 in`, the modeled nose contact is `1.575 in` above the cloth, or `0.450 in` above the resting center plane. Applying that response to a sphere whose bottom is roughly 10–12 inches above the cloth violates the source geometry and the $\dot z_G=0$ precondition.

### Pocket acceptance is tied to walls and a hole rim, not an unbounded XY target

`whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf:14-15` defines success through rebounds reaching the center of the pocket-hole rim after three wall rattles. Its geometry explicitly includes ball radius, mouth width, wall angle, hole radius, and shelf depth (`:36-40`), and gives the source model's `50.688°` maximum fast side-entry angle (`:68-102`). It is a planar target model and does not supply airborne vertical dimensions; therefore it may continue to classify the XY target for on-table capture, but it cannot justify capture solely because a high sphere's projection enters that target. The finite facings/rim/opening must first be respected in 3D.

No whitepaper in the cited audit supplies a validated airborne cushion/jaw impulse response or a complete Brunswick GC IV vertical cross-section. The safe source-aligned cutover is therefore to predict finite 3D contact/entry and emit a terminal diagnostic while preserving the state, rather than inventing a rebound.

## Numeric reproducer with units

All three current wrong fixtures use `R = 1.125 in`, `z_0 = 12 in`, `w_0 = 0`, `g = 386.08858267716535 in/s²`, and rolling-projection deceleration `a = 5 in/s²`. Their ballistic table-plane time is

$$
t_{table}=\sqrt{\frac{2z_0}{g}}
=\sqrt{\frac{24\ \mathrm{in}}{386.0885827\ \mathrm{in/s^2}}}
=0.2493229\ \mathrm{s}.
$$

For a projected rolling distance $s$ and initial planar speed $v$, the current on-table projection reaches the target at

$$
t_{xy}=\frac{v-\sqrt{v^2-2as}}{a}.
$$

1. **Rail fixture (`tests/n_ball_pockets.rs:267-295`).** The ball starts `10 in` before the top center-contact plane at `200 in/s`. The projected rail time is `0.0500313 s`. The actual ballistic state then is

   $$z=11.5167851\ \mathrm{in},\qquad w=-19.3165095\ \mathrm{in/s}.$$

   Because `height` is measured above the resting center plane, the bottom of the sphere is still `11.5168 in` above the cloth. Relative to Mathavan's `1.575 in` nose height, it clears the modeled nose by about `9.9418 in`. The event cannot be an executable ordinary top-rail impact.

2. **Jaw fixture (`tests/n_ball_pockets.rs:377-407`).** The side-pocket path is aligned with the current rounded-jaw center. The start-to-center distance is `10 in`; the current contact radius is `R + 0.125 in = 1.250 in`, so the projected travel is `8.75 in`. At `200 in/s`, current projected contact is `0.0437740 s`, while the true state is

   $$z=11.6300965\ \mathrm{in},\qquad w=-16.9006231\ \mathrm{in/s}.$$

   It is vertically clear of a table-level jaw by roughly ten inches.

3. **Capture fixture (`tests/n_ball_pockets.rs:297-328`).** The corner trajectory begins `10 in` from the top-right aiming center at `120 in/s`. The current radial analytic gate uses `0.5 × 4.5 in × 1.08 = 2.43 in`, reached after `7.57 in` of projected travel, at `0.0631665 s`; even there, `z = 11.2297530 in`. At the audit's conservative `0.08 s` planar-capture bound, the ball is still at

   $$z=10.7645165\ \mathrm{in},\qquad w=-30.8870866\ \mathrm{in/s}.$$

   Its center is `11.8895 in` above the cloth and its bottom is `10.7645 in` above the cloth. An XY target entry cannot mean rim intersection or terminal capture.

The implementation tests must calculate these states from the public equations rather than hard-code only event-type expectations.

## Root cause

1. **An invalid type escape.** The three predictors require `&OnTableBallState`, but `PocketAwareEventCache::refresh_ball` manufactures one from an airborne state instead of respecting that precondition.
2. **Geometry without a Z domain.** Current rails are four complete 2D planes, jaws are 2D circles, and capture is a 2D target/mouth/back-plane predicate. None represents a finite cushion, facing, jaw, rim, opening, or exterior volume.
3. **Resolvers trust the projected payload.** Rail and jaw resolution unconditionally install `OnTable` results; capture stores the projected `OnTableBallState`. No branch checks the source state's variant or continuity with the ballistically advanced state.
4. **The landing plane is infinite.** Ballistic table contact is scheduled from `height == 0` without classifying whether the XY point is slate, pocket opening/rim, cushion, or outside the table.

## Proposed boundary/contact model

This section is proposed design, not a description of current behavior.

### One uninflated physical geometry source

Consume the canonical uninflated XY primitives introduced by:

- `plans/pocket-rail-cutouts.md`, **Clip Rail Solids at Pocket Mouths and Add Explicit Facings**; and
- `plans/pocket-rounded-jaw-mouth-width.md`, which provides physical mouth tips and bounded exposed jaw arcs.

Those primitives must remain independent of ball radius. This plan lifts them into 3D and performs one sphere-radius query at prediction time; it must not apply a second 2D Minkowski offset.

Represent stable physical primitive identities for:

- each finite ordinary cushion segment, with pocket-mouth intervals removed;
- each bounded jaw arc, facing, and inner-wall segment;
- each finite pocket-rim/slate patch and its complementary opening;
- the playable slate support region; and
- the exterior/escape side of the complete boundary.

For the source-backed diagnostic envelope, express physical heights above the cloth in the `BallState` coordinate system with

$$
z_{model}=h_{above\ cloth}-R.
$$

Use the cloth/slate top at `z_model = -R`. Use Mathavan's modeled cushion/jaw nose height at `z_model = 2R/5`. Until a measured table profile supplies a richer cross-section, the conservative diagnostic cushion/facing envelope is the finite extrusion from cloth/slate level through the known nose height. Document this as an unsupported-contact detection envelope, not as a validated impulse surface. Do not invent GC IV top-rail, rubber-thickness, or pocket-drop dimensions.

### Full sphere-to-finite-primitive eligibility

Let the ballistic center be

$$
\mathbf c(t)=
\begin{bmatrix}
x_0+v_xt\\y_0+v_yt\\z_0+w_0t-gt^2/2
\end{bmatrix}.
$$

For a finite uninflated solid/contact primitive $S$, define

$$
F_S(t)=d(\mathbf c(t),S)^2-R^2.
$$

A candidate is the earliest finite $t\ge 0$ where $F_S(t)=0$, the pre-root gap is positive, and the sphere is closing against the primitive. Closest-point evaluation must clamp both the finite XY segment/arc parameter and the finite vertical interval. This automatically rejects a 12-inch-high XY crossing, includes segment/arc endpoints, and prevents an infinite plane from extending across a pocket mouth.

Do not rely on a fixed time-step scan that can tunnel at high speed. Partition at closest-feature changes and solve/bracket the resulting continuous polynomial pieces; ballistic Z makes squared-distance pieces at most quartic. Refine the earliest bracket to the existing event-time tolerance and verify the closing derivative. Stable primitive IDs provide deterministic tie-breaking at seams.

### Classify the ballistic table-plane root

At the next downward `height == 0` root, classify the XY sphere support against the same canonical geometry before creating `BallTableBounce`:

1. **Slate support:** schedule the existing `AirborneTableContact` and table-bounce resolver.
2. **Pocket opening/entry region:** schedule an explicit terminal `UnsupportedAirbornePocketEntry` diagnostic carrying the pocket and the full `BallState` at entry. The TP planar target may identify the associated pocket, but it must not turn this unsupported airborne entry into `BallPocketCapture`.
3. **Rim, jaw, facing, or cushion solid:** the earlier sphere-to-solid root must win as an unsupported airborne boundary contact.
4. **Exterior or an overflight that clears all finite solids:** schedule `AirborneBallLeavesTable` at the first irreversible support-domain exit. Do not schedule a later bounce against the infinite `height == 0` plane.

An airborne state at `height == 0` with upward vertical velocity remains airborne and must not be captured merely because its XY lies in an opening. Pocket entry requires a downward crossing.

### Resolver policy

The current repository has no source-validated full-3D cushion or jaw response. Therefore:

- ordinary `BallRailImpact`, `BallJawImpact`, and `BallPocketCapture` remain executable only for `NBallSystemState::OnTable` inputs;
- `UnsupportedAirborneRailContact`, `UnsupportedAirborneJawOrRimContact`, and `UnsupportedAirbornePocketEntry` are terminal diagnostics;
- `AirborneBallLeavesTable` is also a terminal diagnostic; and
- resolution advances every state by the event time through `advance_n_ball_system_without_event` and performs no state replacement for the affected airborne ball.

The affected state after diagnostic resolution must remain `NBallSystemState::Airborne` and equal the event's full `state_at_interaction`. This gives callers an exact state for fouls, visualization, or a future 3D solver without silently fabricating a rebound. Adding gameplay foul rules or continuing exterior flight is intentionally outside this plan.

## Event and data-model cutover

Use a clean public cutover; do not add aliases or encode diagnostics as ordinary rail/jaw/capture events.

1. Add public full-state prediction records whose payload is `BallState`, for example:
   - `PredictedUnsupportedAirborneRailContact { rail, time_until_contact, state_at_contact }`;
   - `PredictedUnsupportedAirborneJawOrRimContact { pocket, primitive, time_until_contact, state_at_contact }`;
   - `PredictedUnsupportedAirbornePocketEntry { pocket, time_until_entry, state_at_entry }`; and
   - `PredictedAirborneBallLeavesTable { boundary, time_until_exit, state_at_exit }`.

   A shared internal record is acceptable, but the public boundary identity must remain exhaustive and typed; string labels are not an API.

2. Add corresponding `NBallPocketAwareSystemEventSource` and `NBallPocketAwareSystemEventCandidateRef` cases, plus cache slots dedicated to airborne boundary contact, pocket entry, and exit. Never put an airborne prediction into `jaw_impacts`, `pocket_captures`, or `rail_impacts`.
3. Add public `NBallSystemEvent` variants for the diagnostic records. Update `time`, `primary_ball` (`Some(ball_index)`), and `is_terminal_diagnostic` (`true`).
4. In `PocketAwareEventCache::refresh_ball`:
   - clear all three executable on-table candidate slots for `Airborne`;
   - remove the synthetic `BallState::on_table` projection entirely;
   - schedule finite 3D contact/entry/exit and a geometry-classified table contact from the real `BallState`; and
   - clear airborne-only slots for `OnTable` and `Pocketed` states.
5. Update `next_event` to compare the new candidates chronologically with ball-ball diagnostics and table contact. Use only `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` for true ties. The separate `plans/jaw-capture-event-causality.md` owns removal of the current 5 ms jaw/capture reversal; this plan must not copy or extend that override to airborne diagnostics.
6. In `resolve_n_ball_system_event_with_physics_and_pockets_on_table`, make every new diagnostic branch a deliberate no-op after ballistic advancement and assert the event state matches the advanced airborne state. Add debug assertions to the existing rail/jaw/capture branches that the affected advanced state is `OnTable`; these branches must never normalize an airborne state.
7. Extend `ScenarioShotTraceEventKind` with typed boundary-contact, pocket-entry, and leaves-table diagnostics. Migrate formatting, ball-involvement checks, event-log conversion, timeline replay, and exhaustive tests in one change.
8. Keep `NBallSystemState::Pocketed { state_at_capture: OnTableBallState }` unchanged in this diagnostic cutover because executable capture remains on-table only. A future source-backed airborne drop model may separately promote it to `BallState`; this plan must not broaden that type without an executable capture contract.

## Explicit non-goals

- Airborne ball-ball response. The existing unsupported diagnostic is already implemented; generation and response of vertically moving collision states belongs to `plans/ball-collision-vertical-impulse.md`.
- Inventing or calibrating an airborne cushion/jaw impulse law. Existing Mathavan/rail models remain on-table only.
- Reworking the XY rail cutouts/facings, mouth-tip interpretation, or jaw-arc centers. Those belong to `plans/pocket-rail-cutouts.md` and `plans/pocket-rounded-jaw-mouth-width.md`.
- Fixing the 5 ms jaw-over-capture event-order override, owned by `plans/jaw-capture-event-causality.md`.
- Changing TP 3.5–3.8 target formulas, speed interpolation, pocket dimensions, on-table capture policy, or on-table rail response coefficients.
- Modeling post-capture pocket-drop motion, ball return systems, cushion-top rolling, exterior floor impacts, gameplay foul adjudication, or simulation after a leaves-table diagnostic.
- Adding a second radius-inflated geometry cache. Physical primitives remain uninflated; each ball query applies its own radius once.
- Editing whitepaper PDFs or generated `agent_knowledge/*` artifacts. No source corpus changes are required.

## Phased implementation

### Phase 1 — Replace the three wrong contracts with failing behavioral tests

1. Invert the existing 12-inch rail, capture, and jaw tests so they reject executable `BallRailImpact`, `BallPocketCapture`, and `BallJawImpact` at the old planar times.
2. Assert the analytic `height` and `vertical_velocity` at the old crossing for each fixture, with inches and seconds shown above.
3. Add resolver-continuity assertions: advancing to a diagnostic preserves `NBallSystemState::Airborne`; event payload and resulting state agree with the ballistic equations; neither component is reset to zero.
4. Add table-driven low/high pairs for rail, jaw/facing/rim, and pocket entry. The same XY path must be rejected while vertically clear and become a finite-contact/entry diagnostic only after descending into the modeled vertical envelope.
5. Add a support-classification trio at the downward table-plane root: slate yields `BallTableBounce`, pocket opening yields unsupported pocket entry, and exterior yields leaves-table.

These tests should fail on the projection code before production changes.

### Phase 2 — Introduce typed diagnostic events and remove the unsafe projection

1. Add the full-state prediction records and public `NBallSystemEvent` variants.
2. Add separate cache slots and candidate/source mappings.
3. Delete the `BallState::on_table` construction in the airborne branch. Immediately clear ordinary jaw/capture/rail candidates for every airborne state.
4. Implement the minimal safe fallback first: compare the real ballistic table-contact root with finite support-domain exit and emit a full-state unsupported pocket-entry or leaves-table diagnostic rather than any executable projected event.
5. Migrate `time`, `primary_ball`, terminal-diagnostic behavior, resolver exhaustiveness, and DSL event conversion in the same cutover. There is no compatibility shim.

At the end of this phase, the three impossible events and height erasure are gone even if the richer finite-contact predictor is not yet enabled.

### Phase 3 — Lift canonical finite boundary geometry into 3D

1. Consume uninflated finite rail/facing/opening primitives from `plans/pocket-rail-cutouts.md` and exposed physical jaw arcs from `plans/pocket-rounded-jaw-mouth-width.md`.
2. Add a raw, per-table/per-ball query view that combines those XY primitives with the source-backed cloth-to-nose diagnostic height envelope and rim/slate plane. Keep units and height reference explicit.
3. Implement exact closest points for bounded segments, exposed arcs, vertical endcaps, and rim/slate patches.
4. Implement earliest closing sphere-contact roots over the airborne horizon without fixed-step tunneling.
5. Classify each root as rail, jaw/facing/rim, opening entry, slate contact, or exterior exit and preserve stable primitive identity in diagnostics.
6. Remove the Phase 2 coarse fallback once every branch is covered by the canonical finite geometry. Do not retain two geometry conventions.

### Phase 4 — Integrate scheduling and resolution end to end

1. Schedule the earliest of airborne ball-ball diagnostic, finite boundary diagnostic, pocket-entry diagnostic, leaves-table diagnostic, and valid slate contact.
2. Make actual time order authoritative, with deterministic source ordering only inside the simultaneous-event epsilon.
3. Assert executable rail/jaw/capture resolvers receive an `OnTable` affected state and terminal airborne diagnostics preserve the advanced `Airborne` state.
4. Exercise cached simulation, manual compute-plus-resolve replay, event-limit simulation, and DSL trace replay to prove they produce the same event time and final full state.
5. Integrate airborne states generated by `plans/ball-collision-vertical-impulse.md`; no special route may bypass the same cache and geometry.

### Phase 5 — Smoke test, then update source documentation

After the focused behavioral tests pass:

1. Rewrite the public scheduler rustdoc to state the actual policy: on-table boundary events are executable; airborne finite boundary/entry/exit interactions are terminal diagnostics; slate contact alone enters the table-bounce resolver.
2. Update `PHYSICS_TODOS.md:44-54` so its historical airborne ball-ball entry no longer implies projected rail/jaw/pocket execution is valid.
3. Update `whitepapers/rail_rebound.md:109-119` to say the reduced solver rejects airborne inputs and identify the diagnostic event used instead.
4. Keep `agent_knowledge/*` untouched because it is generated evidence and no source document changes.

## Regression and acceptance tests

### Required observable invariants

1. **No airborne flattening:** no path from `NBallSystemState::Airborne` calls any `compute_next_ball_*_on_table` predictor by manufacturing an `OnTableBallState`.
2. **All three old event kinds are excluded:** the 12-inch fixtures produce no executable rail impact, jaw impact, or pocket capture at their planar crossing.
3. **Height continuity:** for every airborne diagnostic at time $t$,

   $$
   z_{event}=z_0+w_0t-gt^2/2,\qquad
   w_{event}=w_0-gt,
   $$

   and the resolver's affected state is byte-for-byte/equality-equivalent to that full event state within the unit types' existing equality contract.
4. **Finite vertical eligibility:** a sphere whose swept vertical interval does not intersect the boundary envelope has no contact candidate even when its XY projection crosses; a descending sphere on the same XY path does become eligible when its 3D distance reaches `R`.
5. **Finite XY eligibility:** an airborne query uses clipped rail segments and exposed jaw/facing/rim primitives. It cannot hit an infinite rail through a pocket opening or the hidden half of a jaw circle. A just-outside-mouth control still identifies the ordinary rail segment.
6. **Radius applied once:** repeat a contact query with pool and carom radii; each root uses the physical primitive plus that query radius, with no persisted pool-ball inflation.
7. **Landing classification:** only a downward root over slate schedules `BallTableBounce`. Opening, solid boundary, and exterior points select their respective diagnostic.
8. **No false pocketing:** a high XY crossing never produces `Pocketed`; unsupported airborne pocket entry remains `Airborne` at the diagnostic state and terminates simulation.
9. **Resolver separation:** existing on-table rail, jaw, and capture fixtures continue to resolve exactly through their existing event variants. A deliberately forged mismatch is rejected by an assertion in debug/test builds rather than silently normalized.
10. **Chronology:** event times are finite, nonnegative, and nondecreasing across manual and cached simulation. A later table contact cannot beat an earlier finite boundary/exit diagnostic, and no macroscopic jaw/capture tolerance is used for the new candidates.
11. **Zero-time stability:** initial overlap reports a diagnostic only when closing/penetrating under the chosen policy; separating or merely tangent states do not generate an endless zero-time loop.
12. **Symmetry and seams:** table-driven cases cover all four rails, both jaws of representative side/corner pockets, mirrored pockets, primitive endpoints, and equal-time seam contacts with deterministic identity.
13. **Terminal behavior:** `is_terminal_diagnostic()` is true, `primary_ball()` returns the affected index, simulation stops after recording one unsupported/exit event, and replay produces the same state.
14. **DSL visibility:** event logs and human strings identify the ball and distinguish rail contact, jaw/rim contact, pocket entry, and leaves-table. Timeline geometry ends at the preserved airborne state, not at a flattened point.

### Focused test placement

- Put scheduler, resolver, numeric fixture, finite-envelope, support-classification, and symmetry tests in `tests/n_ball_pockets.rs` alongside the current wrong tests.
- Add DSL conversion/format/replay assertions in `tests/dsl.rs`.
- Add narrow internal unit tests beside raw finite-boundary closest-point/root helpers only for feature-clamp and root-selection invariants that are not observable through the public scheduler.
- Reuse the existing on-table pocket and rail fixtures as unchanged-behavior controls; do not duplicate their broad target-formula assertions.

## Risks and edge cases

- **Feature-switch roots:** closest points can switch between segment interior, endpoints, arc interior, and vertical caps. Root search must partition or conservatively bracket those switches; a single smooth-plane formula is insufficient.
- **High-speed tunneling:** fixed sampling can miss a thin envelope. Tests must include fast crossings and a near-tangent case.
- **Coordinate reference mistakes:** `BallState.height == 0` is the resting center plane, not the cloth plane. Every physical table height must convert by subtracting `R` exactly once.
- **Pocket seams and duplicate candidates:** rail endpoint, jaw arc, facing, and rim may meet at one configuration. Stable primitive IDs and the simultaneous-event epsilon must select deterministically without a 5 ms preference.
- **Ascent at `height == 0`:** an upward post-bounce state is airborne, not a landing or pocket entry. Require a downward crossing for table-plane classification.
- **Outside landing:** `time_until_airborne_ball_reaches_table` remains a kinematic root, not proof of cloth contact. Geometry classification must occur before constructing `AirborneTableContact`.
- **Carom tables:** no pocket primitives exist, but finite rail contact/overflight diagnostics still apply with the carom ball radius.
- **Incomplete measured cross-sections:** the cloth-to-`7R/5` envelope is a conservative unsupported-contact detector, not a validated response surface. Do not turn it into an executable impulse or claim manufacturer calibration.
- **Dependency ordering:** the safety cutover can land before sibling XY plans; the final finite geometry phase must consume their canonical primitives and delete any temporary duplicate geometry.
- **Public enum exhaustiveness:** adding event variants is intentionally breaking for exhaustive downstream matches. Migrate every in-repository match in one commit and document the new terminal semantics in rustdoc.

## Verification commands

Run focused contracts first:

```text
cargo test --test n_ball_pockets airborne_ball_ -- --nocapture
cargo test --test n_ball_pockets airborne_boundary_ -- --nocapture
cargo test --test n_ball_pockets airborne_landing_region_ -- --nocapture
cargo test --test dsl airborne_table_boundary -- --nocapture
```

Then run the relevant file and internal geometry coverage:

```text
cargo test --test n_ball_pockets
cargo test --test dsl
cargo test --lib pocket_mouth_tests
```

Finally run the relevant aggregate suite after the focused checks are green:

```text
cargo test --tests
cargo test --lib
```

A verification report must record the three old planar event times/heights, the selected replacement diagnostic and time, and the post-resolution `NBallSystemState` for each 12-inch fixture.

## Dependencies and sibling-plan overlap

- **`plans/pocket-rail-cutouts.md` — Clip Rail Solids at Pocket Mouths and Add Explicit Facings.** Owns uninflated finite XY rail/facing/opening geometry. This plan owns lifting that geometry into Z, ballistic eligibility, and airborne routing. Do not duplicate rail clipping here.
- **`plans/pocket-rounded-jaw-mouth-width.md`.** Owns physical mouth-tip placement and bounded exposed jaw arcs. This plan consumes those arcs for sphere-distance/height queries; it does not recalibrate mouth width.
- **`plans/jaw-capture-event-causality.md`.** Owns removal of the 5 ms jaw-over-capture reversal and strict earliest-event semantics. This plan adds new candidates to that chronological framework without reproducing the tolerance fix.
- **`plans/ball-collision-vertical-impulse.md`.** Owns ball-ball-generated vertical COM motion and routing upward/downward post-collision states into `NBallSystemState`. Those states must use this plan's boundary scheduler; this plan does not change ball-ball impulses.

The safety cutover and diagnostic API can precede the first two geometry plans. Completion of Phase 3 depends on their canonical uninflated primitives. Integration with the vertical-impulse plan is required before aggregate verification so newly generated airborne states cannot regress through another route.

## Candidate commit message

```text
Prevent airborne balls from flattening into table-boundary events

Schedule finite 3D rail, jaw, pocket-entry, and table-exit diagnostics from the full ballistic state, preserve height through resolution, and keep executable boundary events on-table only.
```
