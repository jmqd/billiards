# Example shot scenarios

These `.billiards` files are small end-to-end examples for the current shot DSL and full-table
simulation pipeline.

Run one with:

```bash
cargo run --bin billiards -- examples/scenarios/<name>.billiards
```

The CLI will:

- parse the layout and shot
- simulate the full table to rest
- print a typed-event-log rendering
- render the final layout with same-color-per-ball traces

Generate and preview every scenario diagram as a local validation gallery with:

```bash
cargo xtask validation-suite
xdg-open target/validation-suite/index.html
```

Or open it automatically:

```bash
cargo xtask validation-suite --open
```

The gallery writes fresh SVG diagrams plus `target/validation-suite/index.html`. The gallery embeds
each SVG inline with table/overlay layer toggles, zoom/pan controls, 2.5 ms playback frames for
smoother 1/16x slow motion, scenario comments, DSL shot line, simulation summary, event log,
cue-ball launch speed in km/h, and the nearest human-facing shot-speed label.

Scenario DSL shots with nonzero side English derive the conservative `1.384deg` TP A.3
rail-clearance cue elevation when `.elevation(...)` is omitted. Use `.elevation(0deg)` only when an
example is intentionally demonstrating the idealized level-cue model.

## Included scenarios

### Elevated side-spin / z-spin diagnostics

These cue-only examples use `.elevation(...)` plus near-miscue side tip offsets so the gallery
playback exposes airborne height, table bounces, and `ωz` spin glyphs without a scoring outcome
muddying the visual check.

### `elevated_right_english_swerve_showcase.billiards`
Expected flavor:
- positive/right `ωz` from a high-right elevated hit
- cue ball leaves the table, lands, then continues with visible side-spin glyphs
- swerve-style visual diagnostic rather than a pocketing route

### `elevated_left_english_masse_showcase.billiards`
Expected flavor:
- negative/left `ωz` from a steeper low-left elevated hit
- cue ball leaves the table and returns with a visible table-bounce event
- masse-style visual diagnostic for spin/height playback

### Three-cushion / carom examples

These scenarios use `table three_cushion_carom_10ft`, `game three_cushion`, the carom ball names
`cue`, `yellow`, and `red`, and `simulation(default).preset(three_cushion)`. The typed preset resolves
the production `PhysicsProfile::three_cushion_default()` baseline and defaults to `heated_carom`.
Three-cushion validation files may vary layouts, shots, event limits, and cue-strike transfer, but
must not embed independent ball-collision or rail coefficients. `cargo xtask validation-suite`
rejects any such drift before rendering.

### `three_cushion_opening_break.billiards`
Expected flavor:
- pocketless 10 ft carom table render with three unnumbered carom balls
- rail-first opening trace with heated-cloth speed retention

### `three_cushion_short_angle.billiards`
Expected flavor:
- compact short-angle rail-first path
- no pocket or jaw events because the table layout is pocketless

### `three_cushion_long_rail_natural.billiards`
Expected flavor:
- longer natural-angle rail-first path on the carom table
- carom ball and table scale rather than pool-ball physics

### Three-cushion scoring route examples

These layouts encode legal cue-ball routes through both object balls and the named cushions on a
pocketless carom table. Every `_score.billiards` fixture is engine-adjudicated under the UMB Article
83 contact order: at least three cue-ball cushion contacts must precede the second object contact.

### `three_cushion_right_top_left_score.billiards`
Expected flavor:
- cue -> yellow first
- cue rail sequence starts right, top, left before scoring on red

### `three_cushion_left_top_right_score.billiards`
Expected flavor:
- cue -> yellow first
- cue rail sequence starts left, top, right before scoring on red

### `three_cushion_bottom_left_top_score.billiards`
Expected flavor:
- cue -> yellow first
- cue rail sequence starts bottom, left, top before scoring on red

### `three_cushion_top_right_left_score.billiards`
Expected flavor:
- cue -> yellow first
- cue rail sequence starts top, right, left before scoring on red

### `three_cushion_left_bottom_right_score.billiards`
Expected flavor:
- cue -> yellow first
- cue rail sequence starts left, bottom, right before scoring on red

### `three_cushion_bottom_right_top_score.billiards`
Expected flavor:
- cue -> yellow first
- cue rail sequence starts bottom, right, top before scoring on red

### `jump_over_full_ball_showcase.billiards`
Expected flavor:
- cue uses `.jump()` as the default 45-degree jump-shot alias
- cue clears the blocking 1
- the trace reports the unsupported airborne cue -> 2 contact before a landing bounce

### `long_jump_over_blocker_showcase.billiards`
Expected flavor:
- cue uses `.jump(32deg)` for a lower, longer jump arc
- cue clears a farther blocking 1 than the 45-degree default example
- the trace reports the unsupported airborne cue -> 2 contact before a landing bounce

### Named high-speed / rail-first scoring examples

These expand the legal set with named carom routes: hako-dama / box-ball (`箱球`), teketeke/ticky,
double-rail/snake, and a three-rails-first bank/bricole. They are tuned as fast diagnostic examples,
not canonical tournament diagrams.

### `three_cushion_teketeke_corner_score.billiards`
Expected flavor:
- rail-first cue route starts left, clips yellow, then returns to the same left cushion
- cue rail sequence starts left, left, top before scoring on red

### `three_cushion_double_rail_return_score.billiards`
Expected flavor:
- cue -> yellow first
- controlled opposite side checks the cue ball back to its first cushion
- rail sequence starts bottom, top, bottom before scoring on red
- the third counted rail is a repeat of the first physical cushion, not a third distinct cushion

### `three_cushion_double_rail_top_return_score.billiards`
Expected flavor:
- cue -> yellow first
- vertical mirror of the double-rail return
- rail sequence starts top, bottom, top before scoring on red

### `three_cushion_double_rail_side_mirror_score.billiards`
Expected flavor:
- cue -> yellow first
- side mirror of the double-rail return
- rail sequence starts bottom, top, bottom from the opposite side

### `three_cushion_stun_check_long_rail_fast_score.billiards`
Expected flavor:
- cue -> yellow first with a stun-thick hit near the long rail
- tuned opposite/check side sends the cue bottom, top, bottom before scoring on red
- fastest of the three stun-check variants

### `three_cushion_stun_check_long_rail_hold_score.billiards`
Expected flavor:
- cue -> yellow first with the same near-rail full-hit family
- cue reaches bottom, top, then returns to bottom before scoring on red
- middle-speed member of the three stun-check variants

### `three_cushion_stun_check_long_rail_nip_score.billiards`
Expected flavor:
- cue -> yellow first with a slight below-center stun nip
- lower launch speed still checks back to the original long rail
- rail sequence starts bottom, top, bottom before scoring on red

### `three_cushion_three_rails_first_score.billiards`
Expected flavor:
- cue runs left, right, left, right before reaching either object ball
- cue then contacts red and yellow to complete the legal rails-first score

### `three_cushion_hako_dama_long_box_behind_score.billiards`
Expected flavor:
- cue -> yellow first
- high-speed hako-dama / box-ball route starts right, top, left and comes back behind red

### `three_cushion_hako_dama_short_side_check_score.billiards`
Expected flavor:
- cue -> yellow first
- opposite-spin hako-dama route starts left, bottom, right and checks into red from the short side

### Source-backed advanced scoring repertoire

The following nine fixtures are asserted as legal UMB scores: the cue ball contacts both object
balls and records at least three cushion contacts before the second object ball. Their source references
and exact expected event orders are embedded in each scenario.

### `three_cushion_natural_angle_standard_score.billiards`
Expected scoring order:
- cue -> yellow
- right, bottom, left cushions
- cue -> red

### `three_cushion_short_angle_running_score.billiards`
Expected scoring order:
- cue -> yellow
- right, top, left cushions with near-limit running English
- cue -> red

### `three_cushion_reverse_english_hold_score.billiards`
Expected scoring order:
- cue -> yellow
- left, bottom, right cushions with heavy reverse English
- cue -> red

### `three_cushion_five_cushion_double_around_score.billiards`
Expected scoring order:
- cue -> yellow
- right, top, left, bottom, right cushions
- cue -> red after the fifth cushion

### `three_cushion_two_rails_first_umbrella_score.billiards`
Expected scoring order:
- right and top cushions before the first object ball
- cue -> yellow
- left cushion
- cue -> red

### `three_cushion_ticky_repeated_rail_score.billiards`
Expected scoring order:
- left cushion, cue -> yellow, then the left cushion again
- bottom cushion
- cue -> red
- the two left-cushion impacts remain separate events and both count under UMB Article 83

### `three_cushion_reverse_the_corner_score.billiards`
Expected scoring order:
- cue -> yellow on a very thin hit
- right, top, then the right cushion again
- cue -> red

### `three_cushion_kiss_back_score.billiards`
Expected scoring order:
- cue -> yellow, yellow rebounds from the left cushion, then cue -> yellow again
- cue takes the right, top, and left cushions
- cue -> red

### `three_cushion_gather_control_score.billiards`
Expected scoring order and leave:
- cue -> yellow
- top, right, and left cushions
- cue -> red
- all three final ball centers remain within 16.3 inches pairwise

### Source-backed advanced nine-ball repertoire

These fixtures turn published instructional patterns into deterministic simulation contracts.
Each scenario embeds its primary source URL and asserts the intended contact order, rail route,
pocket, landing, or safety geometry in `tests/scenario_examples.rs`.

### `nine_ball_two_rail_kick_side_pocket.billiards`
Expected outcome:
- cue clears the 6/8 blockers via the right and top cushions
- cue contacts the legal 5 first and pockets it center-left

### `nine_ball_three_rail_bank_side_pocket.billiards`
Expected outcome:
- cue contacts the legal 8 first
- 8 banks bottom, right, top and drops center-left

### `nine_ball_rail_first_hide_safety.billiards`
Expected outcome:
- cue contacts the right cushion before the legal 6
- all balls remain up, with the 8 occluding the final cue-to-6 line

### `nine_ball_two_rail_z_position.billiards`
Expected outcome:
- 8 drops top-right
- cue crosses from the right cushion to the left cushion and finishes on the 9-to-top-left line

### `nine_ball_jump_over_blocker_top_right.billiards`
Expected outcome:
- cue clears the 8 and contacts the legal 6 while airborne
- 6 drops top-right and the cue returns to the cloth

### `nine_ball_stun_carom_nine_top_right.billiards`
Expected outcome:
- cue contacts the legal 1, then caroms directly into the 9
- 9 drops top-right

### Professional / practice-book manual checks

These scenarios are source-grounded layouts for manual physics review. They intentionally favor
diagnostic trace visibility over exact tournament-table replication; use `--trace-labels true` and
`--trace-color-mode motion-phase` when reviewing.

### `corey_deuel_power_draw.billiards`
Source: `whitepapers/corey_deuel_s_famous_draw_shot.pdf`.

Expected flavor:
- heavy draw and slight outside/right spin after pocketing the 4 up-table
- cue ball bends after the first cushion toward the down-table shape marker

### `golden_break_cut_break.billiards`
Source: `whitepapers/golden_break.pdf`.

Expected flavor:
- non-square 1-ball hit reaches the tight 9-ball rack
- the trace reports the unsupported airborne 1 -> 2 contact instead of silently resolving it

### `frozen_proposition_kiss.billiards`
Source: `whitepapers/frozen_proposition_shot.pdf`.

Expected flavor:
- near-frozen 8/9 one ball off the foot rail
- fuller-than-obvious hit line with draw-assisted kissed 8-ball motion

### `magic_spot_three_rail_kick.billiards`
Source: `whitepapers/magic_spot_kicks.pdf`.

Expected flavor:
- running-spin cue-ball kick with multiple rail contacts
- cue path approaches the symmetric target-ball lane

### `bank_reference_track_one_rail.billiards`
Source: `whitepapers/bank_shot_reference_tracks.pdf`.

Expected flavor:
- object ball starts one diamond off the side rail with cue ball in hand up-table
- reference-track path pockets the object ball in the bottom-right corner

### `hustler_frozen_rail_bank.billiards`
Source: `whitepapers/billiards_on_the_big_screen_the_hustler.pdf`.

### `mirror_frozen_rail_bank_top_left.billiards`

### `frozen_rail_bank_bottom_right.billiards`

All three preserve a source-inspired, rail-frozen elevated bank layout. The current simulator
reports a terminal `UnsupportedAirborneBallBallContact` for their mixed ball/rail/table contact
rather than falsely executing or pocketing the bank. A coupled mixed-contact response is required
before any of the source pocket outcomes can be claimed.

### Lag-shot Dr. Dave speed ladder

The `lag_shot_00_touch.billiards` through `lag_shot_08_exceptional_power_break.billiards`
examples are cue-only lag shots. The cue starts at `(2.0, 2.0)` — centered left/right on the
second diamond from the bottom — and is hit with a very slight angle toward the right side of the
top rail via `heading(1deg)`, so overlapping rebounds are easier to inspect. They cover the built-in
Dr. Dave speed aliases from softest to hardest:

- `lag_shot_00_touch.billiards`
- `lag_shot_01_slow.billiards`
- `lag_shot_02_medium_soft.billiards`
- `lag_shot_03_medium.billiards`
- `lag_shot_04_medium_fast.billiards`
- `lag_shot_05_fast.billiards`
- `lag_shot_06_power.billiards`
- `lag_shot_07_typical_power_break.billiards`
- `lag_shot_08_exceptional_power_break.billiards`

### `straight_in_side_pocket.billiards`
Cue ball from center, straight into an object ball that goes to the right side pocket.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue continues and comes to rest

### `five_degree_side_pocket.billiards`
A slight cut to the right side pocket from center, roughly five degrees off the straight-in line,
with a little draw on the cue ball.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue stays on the table with a modest draw reaction

### `straight_follow_side_pocket.billiards`
A straight pot with topspin / follow.

Current modeled flavor:
- cue -> one collision
- one pocketed in center-right
- cue bounces, then pockets in center-right

### `straight_draw_side_pocket.billiards`
A straight pot with draw.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue draws back and scratches in center-left

### `stop_shot_side_pocket.billiards`
A straight contact diagnostic with a centered tip.

Current modeled flavor:
- cue -> one collision
- one pocketed in center-right
- cue remains on the table

### `right_spin_stun_side_pocket.billiards`
A slight cut with lots of right spin and a near-stun hit.

Current modeled flavor:
- one pocketed in center-right
- cue remains on the table

### `low_left_spin_throw_transfer.billiards`
A low-left-English diagnostic: the cue and 1-ball start vertically aligned,
with a zero-deflection cue configuration so massé drift is the only pre-impact lateral effect.

Expected flavor:
- cue -> one collision on a nearly vertical line of centers
- one travels almost straight toward the top cushion; any small lateral motion is spin-induced
- one carries a small amount of transferred right spin from the low-left cue-ball spin

### `long_cut_top_right_rail.billiards`
A longer cut where the object ball runs up the right rail toward the top-right corner.

Expected flavor:
- cue -> one collision
- object ball runs up the rail and now falls in top-right under the slightly more generous corner capture
- cue continues with later rail contacts

### `long_cut_bottom_left_rail.billiards`
A mirror-image long cut where the object ball runs down the left rail toward the bottom-left corner.

Expected flavor:
- cue -> three collision
- 3 brushes the left rail before falling in bottom-left
- cue stays on the table

### `spot_shot_bottom_right.billiards`
Object ball on the rack / spot region, cut toward the bottom-right corner pocket.

Expected flavor:
- cue -> one collision
- one pocketed in bottom-right
- cue scratches later in bottom-left

### `routine_nine_ball_corner_cut.billiards`
A routine-looking cut on the 9-ball: cue from center, 9-ball near the top-right rail, cut into the top-right corner.

Expected flavor:
- cue -> nine collision
- nine pocketed in top-right
- cue brushes the right rail and comes to rest on the table

### `thin_cut_top_left_corner.billiards`
A thin cut on the 7 near the left rail into the top-left corner.

Expected flavor:
- cue -> seven collision
- seven pocketed in top-left
- cue brushes the left rail after contact

### `one_nine_corner_combo.billiards`
A compact 1-9 combination into the top-right corner.

Expected flavor:
- cue -> one collision
- one -> nine collision
- nine pocketed in top-right

### `seven_ball_force_follow_breakout.billiards`
A hard force-follow cut: the 7 is pocketed in the top-right corner, then the cue ball
drives into a frozen 8-9 cluster to open position on the 8.

Expected flavor:
- cue -> seven collision on a slightly off-angle cut
- seven pocketed in top-right
- cue follows into the frozen 8-9 cluster and leaves all remaining balls on the table

### `force_follow_scratch.billiards`
A force-follow shot where the cue follows the object into the same side pocket.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue scratches in center-right after following through

### `double_rail_kick_side_pocket.billiards`
A two-rail kick into an object-ball contact diagnostic.

Current modeled flavor:
- cue rail impact: right
- cue rail impact: top
- cue -> one collision
- one pocketed in center-left
- cue stays on the table after the kick

### `two_rail_bank_scratch.billiards`
Cue-only multi-rail bank path that scratches in the opposite side pocket.

Expected flavor:
- right rail
- top rail
- cue scratches in center-left after the two-rail path

### `mini_break_cluster.billiards`
A compact break-style shot into a slightly loosened six-ball cluster near the rack spot.

Expected flavor:
- several nearly immediate collisions through the cluster
- at least six balls take clearly visible paths in the current tuned setup
- a busy multi-event spread with several balls remaining on the table
- no pocketing in the current tuned setup

### `nine_ball_break_head_rail.billiards`
A fuller nine-ball break from the head-rail side, with the cue ball four inches off the rail,
a square hit on the 1-ball, and slight draw.

Expected flavor:
- cue -> one collision opens the rack
- several early ball-ball collisions through the triangle
- the default preview trace follows multiple object balls to rails without relying on a tuned make
- many balls begin moving quickly from the frozen rack, with outcomes depending on break tuning

### `nine_ball_break_left_side_rail.billiards`
A fuller nine-ball cut break from the left side rail near the second diamond from the top, with a
slight-draw hit into a frozen rack.

Expected flavor:
- cue -> one collision opens from a more off-axis approach
- several early collisions spread through the rack
- the default preview trace follows the longer cut-break spread past the first rail contacts
- typically less symmetric motion than the head-rail break

### `three_ball_pinball.billiards`
A deliberately busy three-ball chain-reaction layout.

Expected flavor:
- cue -> one collision
- one -> two collision
- later multi-rail motion from the object balls
- no pocketing is required; this one is meant to look busy rather than cleanly finished
