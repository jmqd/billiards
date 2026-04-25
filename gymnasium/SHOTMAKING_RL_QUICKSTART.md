# Shotmaking RL quickstart

This is the shortest path to training against the billiards physics engine. The current tasks are
**one-shot** environments: each episode resets the table, your policy chooses one cue-ball shot, Rust
simulates until everything stops, then the episode terminates.

## 1. Install/run from the repo

```bash
nix develop
cd gymnasium
uv sync --dev
```

Smoke test:

```bash
uv run pytest -q tests
```

## 2. The environment in 30 seconds

```python
import gymnasium as gym
import numpy as np

import billiards_gymnasium  # registers BilliardsPocketBall-v0 / BilliardsNineBall-v0

# Easiest starter task: pocket the one ball without scratching.
env = gym.make(
    "BilliardsPocketBall-v0",
    layout="random_direct_side_pocket",
    target_ball="one",
    reward_mode="target_pocketed_no_scratch",
)

obs, info = env.reset(seed=1)
print(obs.shape)      # (10, 4): cue + one..nine, columns present/x/y/pocketed
print(info["balls"])  # starting layout in table inches

# action = [heading_norm, speed_norm], both in [0, 1]
# heading_norm maps to 0..360 degrees. 0=north/up-table, .25=east/right,
# .5=south/down-table, .75=west/left.
action = np.array([0.25, 0.50], dtype=np.float32)  # shoot right at medium speed
obs, reward, terminated, truncated, info = env.step(action)

print(reward, terminated)          # one-shot env: terminated is always True after step
print(info["target_pocketed"])     # did the target ball drop?
print(info["cue_pocketed"])        # did we scratch?
print(info["pocketed"])            # list of pocketed balls
print(info["events"][:3])          # collision/rail/pocket event trace for debugging
```

To write visual debug PNGs from the same env:

```python
obs, info = env.reset(seed=1)
action = np.array([0.25, 0.50], dtype=np.float32)
env.render_before_png("before.png")          # board before the step
env.render_action_png(action, "action.png")  # simulated trace/markers for this action
env.step(action)
env.render_after_png("after.png")            # board after the step
```

Coordinate frame: table inches on the default 9ft table, roughly `x=0..50`, `y=0..100`.
Shot speed defaults to cue-ball launch speed in inches/sec, interpolated from `min_speed_ips` to
`max_speed_ips` by `speed_norm`.

## 3. Fixed layout for debugging

Use a fixed layout when developing a learner, then randomize once it works.

```python
import gymnasium as gym
import numpy as np
import billiards_gymnasium

layout = [
    {"ball": "cue", "x": 15.0, "y": 50.0},
    {"ball": "one", "x": 30.0, "y": 50.0},
]

env = gym.make("BilliardsPocketBall-v0", layout=layout, target_ball="one")
obs, _ = env.reset()

# cue -> one is straight to the right, so heading 90 degrees = 90/360 = 0.25
action = np.array([90.0 / 360.0, 0.45], dtype=np.float32)
_, reward, _, _, info = env.step(action)
print("made it:", bool(reward), "pocketed:", info["pocketed"])
```

## 4. Minimal learner loop

Because each episode has exactly one action, this is closer to a contextual bandit than long-horizon
RL. Any optimizer can fit `policy(obs) -> [heading_norm, speed_norm]`.

```python
import gymnasium as gym
import billiards_gymnasium

env = gym.make("BilliardsPocketBall-v0", layout="random_direct_side_pocket")

for episode in range(1000):
    obs, info = env.reset(seed=episode)
    action = env.action_space.sample()      # replace with your policy(obs)
    next_obs, reward, done, truncated, info = env.step(action)

    # update your policy from (obs, action, reward)
    # useful debug fields: info["target_pocketed"], info["cue_pocketed"], info["events"]
```

## 5. Batched rollouts

For throughput, pack many independent one-shot episodes into one native call:

```python
from billiards_gymnasium import layouts_and_shots_to_batch_arrays, simulate_shots_batch

layouts = [info["balls"] for _ in range(128)]
shots = [{"heading_degrees": 90.0, "speed_ips": 128.0, "speed_semantics": "cue_ball_launch"} for _ in layouts]
ball_ids, ball_xs, ball_ys, shot_values = layouts_and_shots_to_batch_arrays(layouts, shots)
out = simulate_shots_batch(ball_ids, ball_xs, ball_ys, shot_values)
rewards = out["pocketed_mask"][:, 1].astype(float)  # one ball pocketed
```

## 6. Included baseline: NumPy CEM

A dependency-light Cross-Entropy Method baseline is included:

```bash
uv run python examples/train_pocket_cem.py \
  --layout random_direct_side_pocket \
  --iterations 20 \
  --population 64 \
  --episodes-per-candidate 4 \
  --output pocket_cem_policy.npz
```

For a fast smoke test:

```bash
uv run python examples/train_pocket_cem.py --iterations 1 --population 4 --episodes-per-candidate 1
```

## What to try next

- Start with `BilliardsPocketBall-v0`; move to `BilliardsNineBall-v0` after the learner can pocket a
  single object ball.
- Keep the base action as absolute heading/speed so banks and kicks remain possible.
- Add curriculum wrappers later for friendlier actions like ghost-ball aim or cut angle.
- Randomize layouts gradually; sparse rewards get hard quickly.
