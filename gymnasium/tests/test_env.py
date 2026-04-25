from __future__ import annotations

import numpy as np

from billiards_gymnasium.envs import BilliardsNineBallEnv, BilliardsPocketBallEnv

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


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


def test_envs_expose_before_after_and_action_png_helpers(tmp_path):
    env = BilliardsPocketBallEnv(layout="side_pocket_one_ball")
    env.reset(seed=1)
    action = np.array([0.25, 0.50], dtype=np.float32)

    before = env.render_before_png(path=tmp_path / "before.png")
    proposed_action = env.render_action_png(action, path=tmp_path / "proposed-action.png")
    env.step(action)
    after = env.render_after_png(path=tmp_path / "after.png")
    latest_action = env.render_action_png(path=tmp_path / "latest-action.png")
    bundle = env.render_step_pngs(
        before_path=tmp_path / "bundle-before.png",
        after_path=tmp_path / "bundle-after.png",
        action_path=tmp_path / "bundle-action.png",
    )

    assert before.startswith(PNG_SIGNATURE)
    assert proposed_action.startswith(PNG_SIGNATURE)
    assert after.startswith(PNG_SIGNATURE)
    assert latest_action.startswith(PNG_SIGNATURE)
    assert bundle["before_png"].startswith(PNG_SIGNATURE)
    assert bundle["after_png"].startswith(PNG_SIGNATURE)
    assert bundle["action_png"].startswith(PNG_SIGNATURE)
    assert (tmp_path / "before.png").read_bytes() == before
