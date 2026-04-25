"""Gymnasium integration for the Rust billiards physics engine."""

from __future__ import annotations

from gymnasium.envs.registration import register

from .core import simulate_shot

try:
    register(
        id="BilliardsNineBall-v0",
        entry_point="billiards_gymnasium.envs:BilliardsNineBallEnv",
    )
except Exception:
    # Gymnasium raises if the id was already registered in this interpreter.
    pass

__all__ = ["simulate_shot"]
