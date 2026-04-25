"""Thin Python wrapper around the Rust/PyO3 billiards simulator."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping, Sequence

from . import _native

BallInput = Mapping[str, Any]
ShotInput = Mapping[str, Any]
PathLike = str | bytes | Path


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
    return json.loads(_native.simulate_shot_json(json.dumps(payload)))


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
