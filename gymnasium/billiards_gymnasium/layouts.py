"""Small layout helpers for MVP Gymnasium billiards tasks."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

# A deliberately simple legal-nine combo: cue -> one -> nine -> center-right pocket.
DEFAULT_NINE_BALL_COMBO: list[dict[str, Any]] = [
    {"ball": "cue", "x": 10.0, "y": 50.0},
    {"ball": "one", "x": 25.0, "y": 50.0},
    {"ball": "nine", "x": 37.5, "y": 50.0},
]


def default_nine_ball_combo() -> list[dict[str, Any]]:
    return deepcopy(DEFAULT_NINE_BALL_COMBO)


def normalize_layout(layout: str | list[dict[str, Any]] | None) -> list[dict[str, Any]]:
    if layout is None or layout == "default_nine_ball_combo":
        return default_nine_ball_combo()
    if isinstance(layout, list):
        return deepcopy(layout)
    raise ValueError(f"unknown billiards layout {layout!r}")
