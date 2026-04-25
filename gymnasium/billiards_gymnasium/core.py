"""Thin Python wrapper around the Rust/PyO3 billiards simulator."""

from __future__ import annotations

import json
from typing import Any, Mapping, Sequence

from . import _native

BallInput = Mapping[str, Any]
ShotInput = Mapping[str, Any]


def simulate_shot(
    balls: Sequence[BallInput],
    shot: ShotInput,
    *,
    config: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Simulate one shot to rest and return event/pocketing outcome data.

    Parameters are intentionally simple dictionaries for the MVP native boundary.

    `balls` entries use table inches on a 9ft Brunswick GC4 coordinate frame:
    `{ "ball": "cue"|"one"|...|"nine", "x": float, "y": float }`.

    `shot` requires `heading_degrees` and `speed_ips`; optional fields are
    `speed_semantics`, `tip_side_r`, and `tip_height_r`.
    """

    payload: dict[str, Any] = {"balls": list(balls), "shot": dict(shot)}
    if config is not None:
        payload["config"] = dict(config)
    return json.loads(_native.simulate_shot_json(json.dumps(payload)))
