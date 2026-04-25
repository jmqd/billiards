"""Gymnasium integration for the Rust billiards physics engine."""

from __future__ import annotations

from gymnasium.envs.registration import register

from .core import simulate_shot
from .envs import BilliardsNineBallEnv, BilliardsPocketBallEnv

try:
    register(
        id="BilliardsNineBall-v0",
        entry_point="billiards_gymnasium.envs:BilliardsNineBallEnv",
    )
    register(
        id="BilliardsPocketBall-v0",
        entry_point="billiards_gymnasium.envs:BilliardsPocketBallEnv",
    )
except Exception:
    # Gymnasium raises if the id was already registered in this interpreter.
    pass

__all__ = ["BilliardsNineBallEnv", "BilliardsPocketBallEnv", "simulate_shot"]
