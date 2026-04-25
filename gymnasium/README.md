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

outcome = simulate_shot(
    [
        {"ball": "cue", "x": 10.0, "y": 50.0},
        {"ball": "one", "x": 25.0, "y": 50.0},
        {"ball": "nine", "x": 37.5, "y": 50.0},
    ],
    {
        "heading_degrees": 90.0,
        "speed_ips": 180.0,
        "speed_semantics": "cue_ball_launch",
    },
)

print(outcome["events"])
print(outcome["legal_nine_pocketed"])
```

Coordinates are table inches on the default 9ft Brunswick GC4 coordinate frame:
`x = 0..50`, `y = 0..100`.

Shot speed semantics:

- `cue_stick_at_impact`: `speed_ips` is the lower-level Rust `Shot` cue-stick speed.
- `cue_ball_launch`: `speed_ips` is inverted through the cue-strike model to target immediate
  cue-ball launch speed. This is the default in the Gymnasium env because it is easier for RL.

## Gymnasium environment

```python
import gymnasium as gym
import billiards_gymnasium

env = gym.make("BilliardsNineBall-v0")
obs, info = env.reset()
obs, reward, terminated, truncated, info = env.step([0.25, 0.50])  # 90 degrees, mid speed
```

The MVP env is one-shot:

- action: `[heading_norm, speed_norm]`, both in `[0, 1]`
- observation: `(10, 4)` matrix for cue + one..nine: `present, x_norm, y_norm, pocketed`
- reward: `1.0` only if the nine is legally pocketed
- legal-nine approximation: nine pocketed, cue not pocketed, and first cue/object contact is the
  lowest-numbered object ball present at reset

Future iterations should add richer action modes, randomized layouts, rendering, vectorized/batched
simulation, and full nine-ball foul/rule accounting.
