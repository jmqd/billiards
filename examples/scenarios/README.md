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

The gallery writes fresh SVG diagrams by default plus `target/validation-suite/index.html`; use
`--format png` or `--format both` when raster exports are needed. The gallery includes the scenario
comments, DSL shot line, simulation summary, event log, cue-ball launch speed in mph, and the nearest
human-facing shot-speed label.

## Included scenarios

### Professional / practice-book manual checks

These scenarios are source-grounded layouts for manual physics review. They intentionally favor
diagnostic trace visibility over exact tournament-table replication; use `--trace-labels true` and
`--trace-color-mode motion-phase` when reviewing.

### `corey_deuel_power_draw.billiards`
Source: `whitepapers/corey_deuel_s_famous_draw_shot.pdf`.

Expected flavor:
- heavy draw and slight outside/right spin after potting the 4 up-table
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
- object ball starts one diamond off the side rail
- one-rail bank track can be compared against the reference lane to the bottom-right corner

### `hustler_frozen_rail_bank.billiards`
Source: `whitepapers/billiards_on_the_big_screen_the_hustler.pdf`.

Expected flavor:
- cue ball starts near-frozen to a rail-frozen 8
- firm top-right-English hit shows transferred spin/throw and bank response

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

### `long_cut_top_right_rail.billiards`
A longer cut where the object ball runs up the right rail toward the top-right corner.

Expected flavor:
- cue -> one collision
- object ball runs up the rail and now falls in top-right under the slightly more generous corner capture
- cue continues with later rail contacts

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
Cue-only multi-rail bank path that used to scratch in the opposite side pocket.

Expected flavor:
- right rail
- top rail
- current jaw-aware pocket gate keeps this one on the table as a near-miss instead of a scratch

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
- the default preview trace reaches the wing-ball pocket and follows multiple balls to rails
- many balls begin moving quickly, with outcomes depending on the current break tuning

### `nine_ball_break_left_side_rail.billiards`
A fuller nine-ball cut break from the left side rail near the second diamond from the top, with a
slight-draw hit that drives the wing ball toward the corner.

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
