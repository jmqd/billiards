from __future__ import annotations

import numpy as np

from billiards_gymnasium.envs import BilliardsNineBallEnv, BilliardsPocketBallEnv


def test_default_combo_rewards_legal_nine_when_shot_straight_right():
    env = BilliardsNineBallEnv()
    obs, info = env.reset()

    assert obs.shape == (10, 4)
    assert info == {}

    obs, reward, terminated, truncated, info = env.step(np.array([0.25, 0.50], dtype=np.float32))

    assert obs.shape == (10, 4)
    assert terminated
    assert not truncated
    assert reward == 1.0
    assert info["legal_nine_pocketed"] is True
    assert info["first_cue_contact"] == "one"


def test_pocket_ball_env_rewards_target_pocket_on_direct_shot():
    env = BilliardsPocketBallEnv(layout="side_pocket_one_ball")
    obs, reset_info = env.reset(seed=1)

    assert obs.shape == (10, 4)
    assert reset_info["balls"][1]["ball"] == "one"

    obs, reward, terminated, truncated, info = env.step(np.array([0.25, 0.50], dtype=np.float32))

    assert obs.shape == (10, 4)
    assert terminated
    assert not truncated
    assert reward == 1.0
    assert info["target_pocketed"] is True
    assert info["any_object_pocketed"] is True
    assert info["cue_pocketed"] is False
