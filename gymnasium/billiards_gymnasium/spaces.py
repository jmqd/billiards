"""Observation/action-space helpers for billiards Gymnasium environments."""

from __future__ import annotations

from typing import Any, Iterable, Mapping

import numpy as np

BALL_ORDER = ["cue", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"]
BALL_INDEX = {name: i for i, name in enumerate(BALL_ORDER)}

TABLE_WIDTH_INCHES = 50.0
TABLE_HEIGHT_INCHES = 100.0


def ball_matrix_observation(balls: Iterable[Mapping[str, Any]]) -> np.ndarray:
    """Return a `(10, 4)` matrix: present, x_norm, y_norm, pocketed."""

    obs = np.zeros((len(BALL_ORDER), 4), dtype=np.float32)
    for ball in balls:
        name = str(ball["ball"])
        if name not in BALL_INDEX:
            continue
        row = BALL_INDEX[name]
        obs[row, 0] = 1.0
        obs[row, 1] = np.clip(float(ball.get("x", 0.0)) / TABLE_WIDTH_INCHES, 0.0, 1.0)
        obs[row, 2] = np.clip(float(ball.get("y", 0.0)) / TABLE_HEIGHT_INCHES, 0.0, 1.0)
        obs[row, 3] = 1.0 if ball.get("state") == "pocketed" else 0.0
    return obs
