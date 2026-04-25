"""Gymnasium environments backed by the Rust billiards simulator."""

from __future__ import annotations

from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces

from .core import simulate_shot
from .layouts import normalize_layout
from .spaces import ball_matrix_observation


class BilliardsNineBallEnv(gym.Env):
    """One-shot nine-ball task.

    The MVP episode is a single cue shot. `step(action)` simulates to rest, returns terminal=True,
    and rewards legal nine-ball pocketing according to the native outcome:

    - nine is pocketed,
    - cue ball is not pocketed,
    - first cue/object contact is the lowest numbered object ball present at reset.
    """

    metadata = {"render_modes": ["human"], "render_fps": 1}

    def __init__(
        self,
        *,
        layout: str | list[dict[str, Any]] | None = None,
        render_mode: str | None = None,
        reward_mode: str = "legal_nine",
        speed_semantics: str = "cue_ball_launch",
        min_speed_ips: float = 26.4,
        max_speed_ips: float = 352.0,
    ) -> None:
        super().__init__()
        if render_mode not in (None, "human"):
            raise ValueError("BilliardsNineBallEnv currently supports render_mode None or 'human'")
        if max_speed_ips <= min_speed_ips:
            raise ValueError("max_speed_ips must be greater than min_speed_ips")
        if reward_mode != "legal_nine":
            raise ValueError("the MVP environment currently supports reward_mode='legal_nine'")

        self.layout = layout
        self.render_mode = render_mode
        self.reward_mode = reward_mode
        self.speed_semantics = speed_semantics
        self.min_speed_ips = float(min_speed_ips)
        self.max_speed_ips = float(max_speed_ips)

        # [heading_norm, speed_norm], both in [0, 1]. Heading maps to 0..360 degrees.
        self.action_space = spaces.Box(
            low=np.array([0.0, 0.0], dtype=np.float32),
            high=np.array([1.0, 1.0], dtype=np.float32),
            dtype=np.float32,
        )
        # Ten rows: cue + one..nine. Columns: present, x_norm, y_norm, pocketed.
        self.observation_space = spaces.Box(low=0.0, high=1.0, shape=(10, 4), dtype=np.float32)

        self._initial_balls: list[dict[str, Any]] = []
        self._last_observation = np.zeros((10, 4), dtype=np.float32)
        self._last_outcome: dict[str, Any] | None = None

    def reset(self, *, seed: int | None = None, options: dict[str, Any] | None = None):
        super().reset(seed=seed)
        layout = options.get("balls") if options and "balls" in options else self.layout
        self._initial_balls = normalize_layout(layout)
        self._last_outcome = None
        self._last_observation = ball_matrix_observation(self._initial_balls)
        return self._last_observation.copy(), {}

    def step(self, action):
        action = np.asarray(action, dtype=np.float32)
        if action.shape != (2,):
            raise ValueError(f"expected action shape (2,), got {action.shape}")
        action = np.clip(action, self.action_space.low, self.action_space.high)

        heading_degrees = float(action[0]) * 360.0
        speed_ips = self.min_speed_ips + float(action[1]) * (self.max_speed_ips - self.min_speed_ips)
        outcome = simulate_shot(
            self._initial_balls,
            {
                "heading_degrees": heading_degrees,
                "speed_ips": speed_ips,
                "speed_semantics": self.speed_semantics,
            },
        )
        self._last_outcome = outcome
        self._last_observation = ball_matrix_observation(outcome["final_balls"])

        reward = 1.0 if outcome.get("legal_nine_pocketed") else 0.0
        terminated = True
        truncated = False
        info = outcome

        if self.render_mode == "human":
            self.render()

        return self._last_observation.copy(), reward, terminated, truncated, info

    def render(self):
        if self._last_outcome is None:
            print("BilliardsNineBallEnv: no shot has been simulated yet")
            return None
        for event in self._last_outcome.get("events", []):
            print(event)
        print(f"legal_nine_pocketed={self._last_outcome.get('legal_nine_pocketed')}")
        return None
