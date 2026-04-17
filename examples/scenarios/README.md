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
- cue follows on toward the right rail

### `straight_draw_side_pocket.billiards`
A straight pot with draw.

Expected flavor:
- cue -> one collision
- one pocketed in center-right
- cue draws back toward the starting end instead of following through

### `spot_shot_bottom_right.billiards`
Object ball on the rack / spot region, cut toward the bottom-right corner pocket.

Expected flavor:
- cue -> one collision
- one pocketed in bottom-right
- cue scratches later in bottom-left

### `two_rail_bank_scratch.billiards`
Cue-only multi-rail bank path that eventually scratches in the opposite side pocket.

Expected flavor:
- right rail
- top rail
- cue pocketed in center-left

### `three_ball_pinball.billiards`
A deliberately busy three-ball chain-reaction layout.

Expected flavor:
- cue -> one collision
- one -> two collision
- several later rail impacts
- no pocketing in the current tuned model
- all three balls eventually stop on the table
