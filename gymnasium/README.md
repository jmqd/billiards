# billiards-gymnasium

Gymnasium environments backed by the Rust billiards physics engine.

This is an MVP integration layer. It intentionally keeps the Python/Rust boundary small:
Python sends a single-shot JSON payload to a PyO3 native module, Rust simulates the shot to rest,
and Python receives events, final ball states, pocketing results, and a `legal_nine_pocketed` flag.

## Install for local development

Python dependencies are owned by `pyproject.toml` and pinned in `uv.lock`. The repository flake
provides `uv`, Rust, and a Nix-managed Python; `uv` creates the project venv at `gymnasium/.venv`.

From the repository root:

```bash
nix develop
cd gymnasium
uv sync --dev
uv run pytest -q
```

For one-off commands without entering an interactive shell:

```bash
nix develop -c bash -c 'eval "$shellHook"; cd gymnasium; uv sync --locked --dev; uv run pytest -q'
```

To build a wheel:

```bash
cd gymnasium
uv build
```

## Native one-shot API

```python
from billiards_gymnasium import simulate_shot

balls = [
    {"ball": "cue", "x": 10.0, "y": 50.0},
    {"ball": "one", "x": 25.0, "y": 50.0},
    {"ball": "nine", "x": 37.5, "y": 50.0},
]
shot = {
    "heading_degrees": 90.0,
    "speed_ips": 180.0,
    "speed_semantics": "cue_ball_launch",
}
outcome = simulate_shot(balls, shot)

print(outcome["events"])
print(outcome["fouls"])        # e.g. scratch, no_object_contact, wrong_first_contact
print(outcome["game_events"])  # e.g. legal_nine_ball_win
print(outcome["legal_nine_pocketed"])
```

PNG rendering helpers are exposed through the same native module:

```python
from billiards_gymnasium import render_board_png, render_shot_trace_png, render_step_pngs

render_board_png(balls, path="before.png")
render_board_png(outcome["final_balls"], path="after.png")
render_shot_trace_png(balls, shot, path="action.png")

# Or render all three at once and keep the simulated outcome:
bundle = render_step_pngs(
    balls,
    shot,
    before_path="before.png",
    after_path="after.png",
    action_path="action.png",
)
```

For faster training rollouts, batch many layouts/actions into one native call:

```python
from billiards_gymnasium import layouts_and_shots_to_batch_arrays, simulate_shots_batch

layouts = [balls] * 128
shots = [shot] * 128
ball_ids, ball_xs, ball_ys, shot_values = layouts_and_shots_to_batch_arrays(layouts, shots)
batch = simulate_shots_batch(ball_ids, ball_xs, ball_ys, shot_values)

print(batch["pocketed_mask"].shape)   # (128, 10), columns cue + one..nine
print(batch["final_state"].shape)     # 0=absent, 1=on_table, 2=pocketed
```

Coordinates are table inches on the default 9ft Brunswick GC4 coordinate frame:
`x = 0..50`, `y = 0..100`.

Shot speed semantics:

- `cue_stick_at_impact`: `speed_ips` is the lower-level Rust `Shot` cue-stick speed.
- `cue_ball_launch`: `speed_ips` is inverted through the cue-strike model to target immediate
  cue-ball launch speed. This is the default in the Gymnasium env because it is easier for RL.

## Gymnasium environments

For a one-page shotmaking RL walkthrough, see [SHOTMAKING_RL_QUICKSTART.md](./SHOTMAKING_RL_QUICKSTART.md).

```python
import gymnasium as gym
import billiards_gymnasium

env = gym.make("BilliardsNineBall-v0")
obs, info = env.reset()
action = [0.25, 0.50]  # 90 degrees, mid speed

env.render_before_png("before.png")
env.render_action_png(action, "action.png")
obs, reward, terminated, truncated, info = env.step(action)
env.render_after_png("after.png")
```

Current envs are one-shot: `step(...)` simulates a single shot to rest and terminates.

- `BilliardsNineBall-v0`: reward is `1.0` only if the nine is legally pocketed.
- `BilliardsPocketBall-v0`: reward is based on pocketing a target/object ball.

Shared controls:

- action: `[heading_norm, speed_norm]`, both in `[0, 1]`
- heading is pure absolute table heading over `0..360°` rather than a cut-angle helper, so banks and
  kicks remain expressible
- observation: `(10, 4)` matrix for cue + one..nine: `present, x_norm, y_norm, pocketed`

`BilliardsNineBall-v0` legal-nine approximation: nine pocketed, cue not pocketed, and first
cue/object contact is the lowest-numbered object ball present at reset.

## Tiny training baseline

A NumPy-only Cross-Entropy Method baseline is included so training can run without torch/SB3:

```bash
cd gymnasium
uv run python examples/train_pocket_cem.py --iterations 20 --population 64
```

This trains a small linear policy for `BilliardsPocketBall-v0` and writes `pocket_cem_policy.npz`.
It is a smoke-testable baseline, not the final RL stack. Future iterations should add richer action
modes, randomized layouts, rendering, vectorized/batched simulation, and full nine-ball foul/rule
accounting.
