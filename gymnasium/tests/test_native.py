from __future__ import annotations

from billiards_gymnasium import (
    layouts_and_shots_to_batch_arrays,
    render_board_png,
    render_shot_trace_png,
    render_step_pngs,
    simulate_shot,
    simulate_shots,
    simulate_shots_batch,
)

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def test_simulate_shot_reports_events_pockets_and_legal_nine():
    outcome = simulate_shot(
        [
            {"ball": "cue", "x": 10.0, "y": 50.0},
            {"ball": "one", "x": 25.0, "y": 50.0},
            {"ball": "nine", "x": 37.5, "y": 50.0},
        ],
        {
            "heading_degrees": 90.0,
            "speed_ips": 180.0,
            "speed_semantics": "cue_ball_launch",
        },
    )

    assert outcome["legal_nine_pocketed"] is True
    assert outcome["nine_pocketed"] is True
    assert outcome["cue_pocketed"] is False
    assert outcome["first_cue_contact"] == "one"
    assert any(event["kind"] == "ball_pocket_capture" for event in outcome["events"])


def test_render_helpers_return_and_write_pngs(tmp_path):
    balls = [
        {"ball": "cue", "x": 10.0, "y": 50.0},
        {"ball": "one", "x": 25.0, "y": 50.0},
    ]
    shot = {
        "heading_degrees": 90.0,
        "speed_ips": 128.0,
        "speed_semantics": "cue_ball_launch",
    }

    before_path = tmp_path / "before.png"
    action_path = tmp_path / "action.png"
    before = render_board_png(balls, path=before_path)
    action = render_shot_trace_png(balls, shot, path=action_path, trace_color_mode="motion_phase")
    bundle = render_step_pngs(
        balls,
        shot,
        before_path=tmp_path / "step-before.png",
        after_path=tmp_path / "step-after.png",
        action_path=tmp_path / "step-action.png",
    )

    assert before.startswith(PNG_SIGNATURE)
    assert action.startswith(PNG_SIGNATURE)
    assert before_path.read_bytes() == before
    assert action_path.read_bytes() == action
    assert bundle["before_png"].startswith(PNG_SIGNATURE)
    assert bundle["after_png"].startswith(PNG_SIGNATURE)
    assert bundle["action_png"].startswith(PNG_SIGNATURE)
    assert bundle["outcome"]["events"]
    assert (tmp_path / "step-before.png").exists()
    assert (tmp_path / "step-after.png").exists()
    assert (tmp_path / "step-action.png").exists()


def test_batched_array_simulation_matches_scalar_outcomes():
    layouts = [
        [
            {"ball": "cue", "x": 10.0, "y": 50.0},
            {"ball": "one", "x": 25.0, "y": 50.0},
        ],
        [
            {"ball": "cue", "x": 10.0, "y": 50.0},
            {"ball": "one", "x": 25.0, "y": 50.0},
            {"ball": "nine", "x": 37.5, "y": 50.0},
        ],
    ]
    shots = [
        {
            "heading_degrees": 90.0,
            "speed_ips": 128.0,
            "speed_semantics": "cue_ball_launch",
        },
        {
            "heading_degrees": 90.0,
            "speed_ips": 180.0,
            "speed_semantics": "cue_ball_launch",
        },
    ]
    ball_ids, ball_xs, ball_ys, shot_values = layouts_and_shots_to_batch_arrays(layouts, shots)

    batched = simulate_shots_batch(ball_ids, ball_xs, ball_ys, shot_values)
    convenience = simulate_shots(layouts, shots)
    scalar = [simulate_shot(layout, shot) for layout, shot in zip(layouts, shots, strict=True)]

    assert batched["elapsed_seconds"].shape == (2,)
    assert batched["final_state"].shape == (2, 10)
    assert batched["pocketed_mask"].shape == (2, 10)
    assert batched["event_count"].tolist() == [len(outcome["events"]) for outcome in scalar]
    assert batched["legal_nine_pocketed"].tolist() == [
        outcome["legal_nine_pocketed"] for outcome in scalar
    ]
    assert bool(batched["pocketed_mask"][0, 1]) is True
    assert bool(batched["pocketed_mask"][1, 9]) is True
    assert convenience["event_count"].tolist() == batched["event_count"].tolist()


def test_render_board_accepts_final_balls_from_simulation():
    outcome = simulate_shot(
        [
            {"ball": "cue", "x": 10.0, "y": 50.0},
            {"ball": "one", "x": 25.0, "y": 50.0},
        ],
        {
            "heading_degrees": 90.0,
            "speed_ips": 128.0,
            "speed_semantics": "cue_ball_launch",
        },
    )

    png = render_board_png(outcome["final_balls"])

    assert png.startswith(PNG_SIGNATURE)
