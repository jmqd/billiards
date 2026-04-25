"""Small layout helpers for MVP Gymnasium billiards tasks."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

import numpy as np

TABLE_WIDTH_INCHES = 50.0
TABLE_HEIGHT_INCHES = 100.0
BALL_RADIUS_INCHES = 1.125
MIN_BALL_SEPARATION_INCHES = 2.0 * BALL_RADIUS_INCHES + 0.25

# A deliberately simple legal-nine combo: cue -> one -> nine -> center-right pocket.
DEFAULT_NINE_BALL_COMBO: list[dict[str, Any]] = [
    {"ball": "cue", "x": 10.0, "y": 50.0},
    {"ball": "one", "x": 25.0, "y": 50.0},
    {"ball": "nine", "x": 37.5, "y": 50.0},
]

# A simple one-ball side-pocket layout: cue -> one -> center-right side pocket.
DEFAULT_SIDE_POCKET_ONE_BALL: list[dict[str, Any]] = [
    {"ball": "cue", "x": 10.0, "y": 50.0},
    {"ball": "one", "x": 25.0, "y": 50.0},
]


def default_nine_ball_combo() -> list[dict[str, Any]]:
    return deepcopy(DEFAULT_NINE_BALL_COMBO)


def default_side_pocket_one_ball() -> list[dict[str, Any]]:
    return deepcopy(DEFAULT_SIDE_POCKET_ONE_BALL)


def random_direct_side_pocket(
    rng: np.random.Generator,
    *,
    target_ball: str = "one",
) -> list[dict[str, Any]]:
    """Generate a makeable-looking cue/object layout aimed at the right side pocket.

    The object ball is placed left of the center-right pocket. The cue ball is placed behind the
    object along the reverse object→pocket line, with small perpendicular jitter so training still
    has to learn heading rather than one fixed answer.
    """

    pocket = np.array([TABLE_WIDTH_INCHES, TABLE_HEIGHT_INCHES / 2.0], dtype=np.float64)
    for _ in range(100):
        object_pos = np.array(
            [rng.uniform(27.0, 39.0), rng.uniform(38.0, 62.0)],
            dtype=np.float64,
        )
        to_pocket = pocket - object_pos
        distance_to_pocket = np.linalg.norm(to_pocket)
        if distance_to_pocket <= 1e-9:
            continue
        away_from_pocket = -to_pocket / distance_to_pocket
        perpendicular = np.array([-away_from_pocket[1], away_from_pocket[0]], dtype=np.float64)
        cue_distance = rng.uniform(14.0, 32.0)
        cue_jitter = rng.uniform(-4.0, 4.0)
        cue_pos = object_pos + away_from_pocket * cue_distance + perpendicular * cue_jitter
        if (
            BALL_RADIUS_INCHES <= cue_pos[0] <= TABLE_WIDTH_INCHES - BALL_RADIUS_INCHES
            and BALL_RADIUS_INCHES <= cue_pos[1] <= TABLE_HEIGHT_INCHES - BALL_RADIUS_INCHES
            and np.linalg.norm(cue_pos - object_pos) >= MIN_BALL_SEPARATION_INCHES
        ):
            return [
                {"ball": "cue", "x": float(cue_pos[0]), "y": float(cue_pos[1])},
                {"ball": target_ball, "x": float(object_pos[0]), "y": float(object_pos[1])},
            ]

    # Extremely unlikely fallback that keeps reset infallible.
    return default_side_pocket_one_ball()


def normalize_layout(
    layout: str | list[dict[str, Any]] | None,
    *,
    rng: np.random.Generator | None = None,
    target_ball: str = "one",
) -> list[dict[str, Any]]:
    if layout is None or layout == "default_nine_ball_combo":
        return default_nine_ball_combo()
    if layout == "side_pocket_one_ball":
        return default_side_pocket_one_ball()
    if layout == "random_direct_side_pocket":
        if rng is None:
            rng = np.random.default_rng()
        return random_direct_side_pocket(rng, target_ball=target_ball)
    if isinstance(layout, list):
        return deepcopy(layout)
    raise ValueError(f"unknown billiards layout {layout!r}")
