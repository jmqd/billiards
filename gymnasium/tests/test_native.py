from __future__ import annotations

from billiards_gymnasium import simulate_shot


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
