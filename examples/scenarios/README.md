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
each SVG inline with table/overlay layer toggles, zoom/pan controls, scenario comments, DSL shot line,
simulation summary, event log, cue-ball launch speed in mph, and the nearest human-facing shot-speed
label.

## Included scenarios

### Three-cushion / carom examples

These scenarios use `table three_cushion_carom_10ft`, `game three_cushion`, the carom ball names
`cue`, `yellow`, and `red`, and the `heated_carom` condition preset for lower cloth drag and
livelier rails.

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

### Legal three-cushion scoring examples

These layouts are tuned scoring paths: cue ball contacts `yellow` before the final `red`, with at
least three cue-ball cushion contacts before `red`; some legal examples put one or more cushions
before `yellow`, and all run on a pocketless carom table.

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
- cue rail sequence starts bottom, top, bottom, returning to the first cushion before scoring

### `three_cushion_three_rails_first_score.billiards`
Expected flavor:
- cue takes three cushions before yellow
- cue rail sequence starts left, right, left before yellow -> red

### `three_cushion_hako_dama_long_box_behind_score.billiards`
Expected flavor:
- cue -> yellow first
- high-speed hako-dama / box-ball route starts right, top, left and comes back behind red

### `three_cushion_hako_dama_short_side_check_score.billiards`
Expected flavor:
- cue -> yellow first
- opposite-spin hako-dama route starts left, bottom, right and checks into red from the short side

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
- non-square 1-ball hit opens the tight 9-ball rack
- cue ball routes toward the side rail and back into the rack/9-ball region

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

Expected flavor:
- cue ball is frozen directly behind a rail-frozen 8 on the right side rail
- firm elevated top-right-English hit banks the 8 into the top-right corner

### `mirror_frozen_rail_bank_top_left.billiards`
A mirror-image frozen-rail bank from the left rail.

Expected flavor:
- cue and 6 are frozen together on the left side rail
- elevated outside-English hit banks the 6 into the top-left corner

### `frozen_rail_bank_bottom_right.billiards`
A second frozen-rail bank, aimed down-table into the bottom-right corner.

Expected flavor:
- cue and 7 are frozen together on the right side rail
- the 7 uses the rail contact and transferred spin to fall in bottom-right

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

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue follows through and scratches in center-right

### `straight_draw_side_pocket.billiards`
A straight pot with draw.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue draws back and scratches in center-left

### `stop_shot_side_pocket.billiards`
A short straight stop shot into the right side pocket.

Expected flavor:
- cue -> one collision
- cue comes nearly dead to rest near contact
- one pocketed in center-right

### `right_spin_stun_side_pocket.billiards`
A slight cut to the right side pocket with lots of right spin and a near-stun hit.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue stays on the table with a visible but bounded post-contact spin effect


### `low_left_spin_throw_transfer.billiards`
A full-contact low-left-English diagnostic: the cue is aimed perpendicular to the top cushion,
with its starting x-position offset just enough that squirt still produces a square hit on the 1.

Expected flavor:
- cue -> one collision on a near-full hit
- one travels mostly straight toward the top cushion, with spin-induced throw to the right
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
A two-rail kick into an object ball that later falls in the left side pocket.

Expected flavor:
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
