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

## Included scenarios

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
- many balls begin moving quickly, with outcomes depending on the current break tuning

### `nine_ball_break_left_side_rail.billiards`
A fuller nine-ball break from the left side rail near the second diamond from the top, again with a
slight-draw hit and a direct line through the 1-ball.

Expected flavor:
- cue -> one collision opens from a more off-axis approach
- several early collisions spread through the rack
- typically less symmetric motion than the head-rail break

### `three_ball_pinball.billiards`
A deliberately busy three-ball chain-reaction layout.

Expected flavor:
- cue -> one collision
- one -> two collision
- later multi-rail motion from the object balls
- no pocketing is required; this one is meant to look busy rather than cleanly finished
