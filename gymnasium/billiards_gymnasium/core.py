"""Thin Python wrapper around the Rust/PyO3 billiards simulator."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping, Sequence

import numpy as np

from . import _native
from .spaces import BALL_INDEX, BALL_ORDER

BallInput = Mapping[str, Any]
ShotInput = Mapping[str, Any]
PathLike = str | bytes | Path
ABSENT_BALL_ID = 255


def _rule_events(outcome: dict[str, Any]) -> dict[str, Any]:
    """Attach explicit one-shot pool rule events inferred from native outcome flags."""

    fouls: list[dict[str, Any]] = []
    lowest_object_ball = outcome.get("lowest_object_ball")
    first_cue_contact = outcome.get("first_cue_contact")

    if bool(outcome.get("cue_pocketed")):
        fouls.append({"kind": "scratch"})

    if lowest_object_ball is not None:
        if first_cue_contact is None:
            fouls.append(
                {
                    "kind": "no_object_contact",
                    "expected_first_contact": lowest_object_ball,
                }
            )
        elif outcome.get("first_contact_lowest_object_ball") is False:
            fouls.append(
                {
                    "kind": "wrong_first_contact",
                    "first_contact": first_cue_contact,
                    "expected_first_contact": lowest_object_ball,
                }
            )

    game_events: list[dict[str, Any]] = []
    if bool(outcome.get("legal_nine_pocketed")):
        game_events.append({"kind": "legal_nine_ball_win", "ball": "nine"})

    outcome["fouls"] = fouls
    outcome["game_events"] = game_events
    return outcome


def _write_optional(path: PathLike | None, data: bytes) -> None:
    if path is not None:
        Path(path).write_bytes(data)


def _render_options(
    *,
    scale_factor: int = 1,
    transparent_background: bool = False,
    trace_sample_step_seconds: float = 0.02,
    trace_color_mode: str = "motion_phase",
    start_ghosts: bool = True,
    event_markers: bool = True,
    labels: bool = False,
) -> dict[str, Any]:
    return {
        "scale_factor": scale_factor,
        "transparent_background": transparent_background,
        "trace_sample_step_seconds": trace_sample_step_seconds,
        "trace_color_mode": trace_color_mode,
        "start_ghosts": start_ghosts,
        "event_markers": event_markers,
        "labels": labels,
    }


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
    return _rule_events(json.loads(_native.simulate_shot_json(json.dumps(payload))))


def layouts_and_shots_to_batch_arrays(
    layouts: Sequence[Sequence[BallInput]],
    shots: Sequence[ShotInput],
    *,
    max_balls: int = 10,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    """Pack Python layouts/shots into compact arrays for `simulate_shots_batch`.

    Ball ids are `cue=0, one=1, ..., nine=9`; absent slots are `255`. `shot_values` columns are
    `[heading_degrees, speed_ips, tip_side_r, tip_height_r]`.
    """

    if len(layouts) != len(shots):
        raise ValueError("layouts and shots must have the same batch length")
    if max_balls <= 0:
        raise ValueError("max_balls must be positive")

    batch_size = len(layouts)
    ball_ids = np.full((batch_size, max_balls), ABSENT_BALL_ID, dtype=np.uint8)
    ball_xs = np.zeros((batch_size, max_balls), dtype=np.float64)
    ball_ys = np.zeros((batch_size, max_balls), dtype=np.float64)
    shot_values = np.zeros((batch_size, 4), dtype=np.float64)

    for batch_index, (layout, shot) in enumerate(zip(layouts, shots, strict=True)):
        if len(layout) > max_balls:
            raise ValueError(f"layout {batch_index} has {len(layout)} balls, max_balls={max_balls}")
        for slot, ball in enumerate(layout):
            name = str(ball["ball"])
            if name not in BALL_INDEX:
                raise ValueError(f"unknown ball name {name!r}")
            ball_ids[batch_index, slot] = BALL_INDEX[name]
            ball_xs[batch_index, slot] = float(ball["x"])
            ball_ys[batch_index, slot] = float(ball["y"])

        shot_values[batch_index, 0] = float(shot["heading_degrees"])
        shot_values[batch_index, 1] = float(shot["speed_ips"])
        shot_values[batch_index, 2] = float(shot.get("tip_side_r", 0.0))
        shot_values[batch_index, 3] = float(shot.get("tip_height_r", 0.0))

    return ball_ids, ball_xs, ball_ys, shot_values


def simulate_shots_batch(
    ball_ids: Any,
    ball_xs: Any,
    ball_ys: Any,
    shot_values: Any,
    *,
    speed_semantics: str = "cue_ball_launch",
    config: Mapping[str, Any] | None = None,
) -> dict[str, np.ndarray]:
    """Simulate a batch of shots through the compact native array API.

    Inputs are array-like:

    - `ball_ids`: `(batch, max_balls)` uint8, using `cue=0, one=1, ..., nine=9`, `255=absent`
    - `ball_xs`, `ball_ys`: `(batch, max_balls)` float64 table-inch coordinates
    - `shot_values`: `(batch, 2..4)` float64 columns
      `[heading_degrees, speed_ips, optional tip_side_r, optional tip_height_r]`

    Returns a dict of NumPy arrays with batch-major flags and final states. `final_state` uses
    `0=absent, 1=on_table, 2=pocketed`; ball columns use `billiards_gymnasium.spaces.BALL_ORDER`.
    """

    config = dict(config or {})
    packed_ball_ids = np.ascontiguousarray(ball_ids, dtype=np.uint8)
    packed_ball_xs = np.ascontiguousarray(ball_xs, dtype=np.float64)
    packed_ball_ys = np.ascontiguousarray(ball_ys, dtype=np.float64)
    packed_shot_values = np.ascontiguousarray(shot_values, dtype=np.float64)
    return _native.simulate_shots_batch(
        packed_ball_ids.tolist(),
        packed_ball_xs.tolist(),
        packed_ball_ys.tolist(),
        packed_shot_values.tolist(),
        speed_semantics,
        float(config.get("cue_mass_ratio", 1.0)),
        float(config.get("collision_energy_loss", 0.1)),
    )


def simulate_shots(
    layouts: Sequence[Sequence[BallInput]],
    shots: Sequence[ShotInput],
    *,
    speed_semantics: str | None = None,
    config: Mapping[str, Any] | None = None,
    max_balls: int = 10,
) -> dict[str, np.ndarray]:
    """Convenience wrapper: pack Python layouts/shots, then call `simulate_shots_batch`."""

    if speed_semantics is None:
        speed_semantics = str(shots[0].get("speed_semantics", "cue_ball_launch")) if shots else "cue_ball_launch"
    ball_ids, ball_xs, ball_ys, shot_values = layouts_and_shots_to_batch_arrays(
        layouts,
        shots,
        max_balls=max_balls,
    )
    return simulate_shots_batch(
        ball_ids,
        ball_xs,
        ball_ys,
        shot_values,
        speed_semantics=speed_semantics,
        config=config,
    )


def render_board_png(
    balls: Sequence[BallInput],
    *,
    path: PathLike | None = None,
    scale_factor: int = 1,
    transparent_background: bool = False,
) -> bytes:
    """Render a table layout to PNG bytes, optionally writing `path`.

    Accepts either initial ball dictionaries or `outcome["final_balls"]` from `simulate_shot`.
    Pocketed final balls are omitted from the rendered table.
    """

    payload = {
        "balls": list(balls),
        "render": _render_options(
            scale_factor=scale_factor,
            transparent_background=transparent_background,
        ),
    }
    png = bytes(_native.render_board_png_json(json.dumps(payload)))
    _write_optional(path, png)
    return png


def render_shot_trace_png(
    balls: Sequence[BallInput],
    shot: ShotInput,
    *,
    path: PathLike | None = None,
    config: Mapping[str, Any] | None = None,
    scale_factor: int = 1,
    transparent_background: bool = False,
    trace_sample_step_seconds: float = 0.02,
    trace_color_mode: str = "motion_phase",
    start_ghosts: bool = True,
    event_markers: bool = True,
    labels: bool = False,
) -> bytes:
    """Render one action as a PNG with simulated ball traces and event markers."""

    payload: dict[str, Any] = {
        "balls": list(balls),
        "shot": dict(shot),
        "render": _render_options(
            scale_factor=scale_factor,
            transparent_background=transparent_background,
            trace_sample_step_seconds=trace_sample_step_seconds,
            trace_color_mode=trace_color_mode,
            start_ghosts=start_ghosts,
            event_markers=event_markers,
            labels=labels,
        ),
    }
    if config is not None:
        payload["config"] = dict(config)
    png = bytes(_native.render_shot_trace_png_json(json.dumps(payload)))
    _write_optional(path, png)
    return png


def render_step_pngs(
    balls: Sequence[BallInput],
    shot: ShotInput,
    *,
    before_path: PathLike | None = None,
    after_path: PathLike | None = None,
    action_path: PathLike | None = None,
    config: Mapping[str, Any] | None = None,
    scale_factor: int = 1,
    transparent_background: bool = False,
    trace_sample_step_seconds: float = 0.02,
    trace_color_mode: str = "motion_phase",
    start_ghosts: bool = True,
    event_markers: bool = True,
    labels: bool = False,
) -> dict[str, Any]:
    """Render before/after/action PNGs for a single shot.

    Returns `{before_png, after_png, action_png, outcome}`. Each PNG is returned as `bytes`; pass
    paths to also write files.
    """

    before_png = render_board_png(
        balls,
        path=before_path,
        scale_factor=scale_factor,
        transparent_background=transparent_background,
    )
    outcome = simulate_shot(balls, shot, config=config)
    after_png = render_board_png(
        outcome["final_balls"],
        path=after_path,
        scale_factor=scale_factor,
        transparent_background=transparent_background,
    )
    action_png = render_shot_trace_png(
        balls,
        shot,
        path=action_path,
        config=config,
        scale_factor=scale_factor,
        transparent_background=transparent_background,
        trace_sample_step_seconds=trace_sample_step_seconds,
        trace_color_mode=trace_color_mode,
        start_ghosts=start_ghosts,
        event_markers=event_markers,
        labels=labels,
    )
    return {
        "before_png": before_png,
        "after_png": after_png,
        "action_png": action_png,
        "outcome": outcome,
    }
