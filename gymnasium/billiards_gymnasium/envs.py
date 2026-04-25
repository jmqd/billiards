"""Gymnasium environments backed by the Rust billiards simulator."""

from __future__ import annotations

from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces

from .core import simulate_shot
from .layouts import normalize_layout
from .spaces import ball_matrix_observation


def _action_to_shot(
    action: np.ndarray,
    *,
    min_speed_ips: float,
    max_speed_ips: float,
    speed_semantics: str,
) -> dict[str, float | str]:
    heading_degrees = float(action[0]) * 360.0
    speed_ips = min_speed_ips + float(action[1]) * (max_speed_ips - min_speed_ips)
    return {
        "heading_degrees": heading_degrees,
        "speed_ips": speed_ips,
        "speed_semantics": speed_semantics,
    }


def _normalized_heading_speed_action(action: Any) -> np.ndarray:
    action = np.asarray(action, dtype=np.float32)
    if action.shape != (2,):
        raise ValueError(f"expected action shape (2,), got {action.shape}")
    return np.clip(action, 0.0, 1.0)


def _any_object_pocketed(outcome: dict[str, Any]) -> bool:
    return any(pocketed["ball"] != "cue" for pocketed in outcome.get("pocketed", []))


def _ball_pocketed(outcome: dict[str, Any], ball: str) -> bool:
    return any(pocketed["ball"] == ball for pocketed in outcome.get("pocketed", []))


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
        self._initial_balls = normalize_layout(layout, rng=self.np_random)
        self._last_outcome = None
        self._last_observation = ball_matrix_observation(self._initial_balls)
        return self._last_observation.copy(), {}

    def step(self, action):
        action = _normalized_heading_speed_action(action)
        outcome = simulate_shot(
            self._initial_balls,
            _action_to_shot(
                action,
                min_speed_ips=self.min_speed_ips,
                max_speed_ips=self.max_speed_ips,
                speed_semantics=self.speed_semantics,
            ),
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


class BilliardsPocketBallEnv(gym.Env):
    """One-shot object-ball pocketing task with pure heading/speed control.

    The action is deliberately the low-level table heading rather than an aim-helper or cut angle:
    `[heading_norm, speed_norm]`, where heading spans the full 0..360 degrees. This keeps banks,
    kicks, and non-obvious routes expressible. Higher-level cut-angle/ghost-ball action wrappers can
    be layered on later as curriculum environments.
    """

    metadata = {"render_modes": ["human"], "render_fps": 1}

    def __init__(
        self,
        *,
        layout: str | list[dict[str, Any]] | None = "random_direct_side_pocket",
        target_ball: str = "one",
        render_mode: str | None = None,
        reward_mode: str = "target_pocketed_no_scratch",
        speed_semantics: str = "cue_ball_launch",
        min_speed_ips: float = 26.4,
        max_speed_ips: float = 352.0,
    ) -> None:
        super().__init__()
        if render_mode not in (None, "human"):
            raise ValueError("BilliardsPocketBallEnv currently supports render_mode None or 'human'")
        if max_speed_ips <= min_speed_ips:
            raise ValueError("max_speed_ips must be greater than min_speed_ips")
        if reward_mode not in (
            "target_pocketed",
            "target_pocketed_no_scratch",
            "any_object_pocketed",
        ):
            raise ValueError(
                "reward_mode must be 'target_pocketed', 'target_pocketed_no_scratch', "
                "or 'any_object_pocketed'"
            )

        self.layout = layout
        self.target_ball = target_ball
        self.render_mode = render_mode
        self.reward_mode = reward_mode
        self.speed_semantics = speed_semantics
        self.min_speed_ips = float(min_speed_ips)
        self.max_speed_ips = float(max_speed_ips)

        self.action_space = spaces.Box(
            low=np.array([0.0, 0.0], dtype=np.float32),
            high=np.array([1.0, 1.0], dtype=np.float32),
            dtype=np.float32,
        )
        self.observation_space = spaces.Box(low=0.0, high=1.0, shape=(10, 4), dtype=np.float32)

        self._initial_balls: list[dict[str, Any]] = []
        self._last_observation = np.zeros((10, 4), dtype=np.float32)
        self._last_outcome: dict[str, Any] | None = None

    def reset(self, *, seed: int | None = None, options: dict[str, Any] | None = None):
        super().reset(seed=seed)
        layout = options.get("balls") if options and "balls" in options else self.layout
        self._initial_balls = normalize_layout(
            layout,
            rng=self.np_random,
            target_ball=self.target_ball,
        )
        self._last_outcome = None
        self._last_observation = ball_matrix_observation(self._initial_balls)
        return self._last_observation.copy(), {"balls": [dict(ball) for ball in self._initial_balls]}

    def step(self, action):
        action = _normalized_heading_speed_action(action)
        outcome = simulate_shot(
            self._initial_balls,
            _action_to_shot(
                action,
                min_speed_ips=self.min_speed_ips,
                max_speed_ips=self.max_speed_ips,
                speed_semantics=self.speed_semantics,
            ),
        )
        self._last_outcome = outcome
        self._last_observation = ball_matrix_observation(outcome["final_balls"])

        target_pocketed = _ball_pocketed(outcome, self.target_ball)
        any_object_pocketed = _any_object_pocketed(outcome)
        cue_pocketed = bool(outcome.get("cue_pocketed"))
        if self.reward_mode == "any_object_pocketed":
            reward = 1.0 if any_object_pocketed else 0.0
        elif self.reward_mode == "target_pocketed_no_scratch":
            reward = 1.0 if target_pocketed and not cue_pocketed else 0.0
        else:
            reward = 1.0 if target_pocketed else 0.0

        info = {
            **outcome,
            "target_ball": self.target_ball,
            "target_pocketed": target_pocketed,
            "any_object_pocketed": any_object_pocketed,
        }

        if self.render_mode == "human":
            self.render()

        return self._last_observation.copy(), reward, True, False, info

    def render(self):
        if self._last_outcome is None:
            print("BilliardsPocketBallEnv: no shot has been simulated yet")
            return None
        for event in self._last_outcome.get("events", []):
            print(event)
        print(f"target_pocketed={_ball_pocketed(self._last_outcome, self.target_ball)}")
        return None
