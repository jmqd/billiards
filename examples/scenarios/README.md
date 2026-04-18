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
- cue takes a couple of post-contact rails from the right spin

### `long_cut_top_right_rail.billiards`
A longer cut where the object ball runs up the right rail into the top-right corner pocket.

Expected flavor:
- cue -> one collision
- one pocketed in top-right
- cue continues with later rail contacts

### `spot_shot_bottom_right.billiards`
Object ball on the rack / spot region, cut toward the bottom-right corner pocket.

Expected flavor:
- cue -> one collision
- one pocketed in bottom-right
- cue scratches later in bottom-left

### `force_follow_scratch.billiards`
A force-follow shot where the cue follows the object into the same side pocket.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue scratches in center-right after following through

### `double_rail_kick_side_pocket.billiards`
A two-rail kick into an object ball that later falls in the right side pocket.

Expected flavor:
- cue rail impact: right
- cue rail impact: top
- cue -> one collision
- one pocketed in center-right

### `two_rail_bank_scratch.billiards`
Cue-only multi-rail bank path that eventually scratches in the opposite side pocket.

Expected flavor:
- right rail
- top rail
- cue pocketed in center-left

### `mini_break_cluster.billiards`
A compact break-style shot into a six-ball cluster near the rack spot.

Expected flavor:
- several nearly immediate collisions through the cluster
- at least one later pocketed ball
- a busy multi-event spread with several balls remaining on the table

### `three_ball_pinball.billiards`
A deliberately busy three-ball chain-reaction layout.

Expected flavor:
- cue -> one collision
- one -> two collision
- two pockets in center-right
- cue and one both stop on the table shortly after
