use billiards::svg_generator::{
    render_svg_report_from_dsl, serialize_scenario_playback_report, ScenarioPlaybackBallReport,
    ScenarioPlaybackBallVisual, ScenarioPlaybackEventReport, ScenarioPlaybackFrameReport,
    ScenarioPlaybackReport,
};
use serde_json::Value;

const SCRIPT_HOSTILE: &str = "quote\" slash\\ line\n<tag>&\u{2028}\u{2029}";
const SCRIPT_SAFE_JSON: &str = "quote\\\" slash\\\\ line\\n\\u003ctag\\u003e\\u0026\\u2028\\u2029";

fn assert_playback_tuple_schema(playback: &Value) {
    let playback = playback
        .as_object()
        .expect("playback report must be a JSON object");
    assert_eq!(playback.len(), 4, "playback object field count changed");
    assert!(playback["duration"].is_number());

    let events = playback["events"]
        .as_array()
        .expect("playback events must be an array");
    for event in events {
        let event = event
            .as_array()
            .expect("playback events must use positional tuples");
        assert_eq!(event.len(), 3, "event tuple field count changed");
        assert!(event[0].is_string());
        assert!(event[1].is_number());
        assert!(event[2].is_string());
    }

    let balls = playback["balls"]
        .as_array()
        .expect("playback ball visuals must be an array");
    for ball in balls {
        let ball = ball
            .as_array()
            .expect("playback ball visuals must use positional tuples");
        assert_eq!(ball.len(), 5, "ball visual tuple field count changed");
        assert!(ball[0].is_string());
        assert!(ball[1].is_string());
        assert!(ball[2].is_string() || ball[2].is_null());
        assert!(ball[3].is_number());
        assert!(ball[4].is_number());
    }

    let frames = playback["frames"]
        .as_array()
        .expect("playback frames must be an array");
    for frame in frames {
        let frame = frame
            .as_array()
            .expect("playback frames must use positional tuples");
        assert_eq!(frame.len(), 2, "frame tuple field count changed");
        assert!(frame[0].is_number());
        let frame_balls = frame[1].as_array().expect("frame balls must be an array");
        for ball in frame_balls {
            let ball = ball
                .as_array()
                .expect("frame balls must use positional tuples");
            assert_eq!(ball.len(), 10, "frame ball tuple field count changed");
            assert!(ball[0].is_string());
            assert!(ball[1..].iter().all(Value::is_number));
        }
    }
}

#[test]
fn canonical_playback_serializer_preserves_compact_schema_precision_and_escaping() {
    let report = ScenarioPlaybackReport {
        duration: 1.25,
        events: vec![ScenarioPlaybackEventReport {
            label: SCRIPT_HOSTILE.to_string(),
            time: 2.5,
            summary: SCRIPT_HOSTILE.to_string(),
        }],
        balls: vec![ScenarioPlaybackBallVisual {
            id: SCRIPT_HOSTILE,
            fill: "#aabbcc",
            label: Some(SCRIPT_HOSTILE),
            radius: 12.25,
            radius_inches: 1.125,
        }],
        frames: vec![ScenarioPlaybackFrameReport {
            time: 0.125,
            balls: vec![ScenarioPlaybackBallReport {
                id: SCRIPT_HOSTILE,
                x: 12.5,
                y: -0.25,
                height_inches: 0.125,
                vx_ips: 1.25,
                vy_ips: -2.5,
                vz_ips: 3.75,
                wx_rps: -4.125,
                wy_rps: 5.5,
                wz_rps: -6.625,
            }],
        }],
    };

    let mut json = String::new();
    serialize_scenario_playback_report(&mut json, &report);
    let expected = format!(
        "{{\"duration\":1.250000,\"events\":[[\"{0}\",2.500000,\"{0}\"]],\"balls\":[[\"{0}\",\"#aabbcc\",\"{0}\",12.250,1.125000]],\"frames\":[[0.125000,[[\"{0}\",12.500,-0.250,0.125000,1.250000,-2.500000,3.750000,-4.125000,5.500000,-6.625000]]]]}}",
        SCRIPT_SAFE_JSON
    );
    assert_eq!(json, expected);

    let parsed: Value = serde_json::from_str(&json).expect("compact playback must be valid JSON");
    assert_playback_tuple_schema(&parsed);

    let event = parsed["events"][0]
        .as_array()
        .expect("event must remain a tuple");
    assert_eq!(event[0].as_str(), Some(SCRIPT_HOSTILE));
    assert_eq!(event[1].as_f64(), Some(2.5));
    assert_eq!(event[2].as_str(), Some(SCRIPT_HOSTILE));

    let visual = parsed["balls"][0]
        .as_array()
        .expect("ball visual must remain a tuple");
    assert_eq!(visual[0].as_str(), Some(SCRIPT_HOSTILE));
    assert_eq!(visual[1].as_str(), Some("#aabbcc"));
    assert_eq!(visual[2].as_str(), Some(SCRIPT_HOSTILE));
    assert_eq!(visual[3].as_f64(), Some(12.25));
    assert_eq!(visual[4].as_f64(), Some(1.125));

    let ball = parsed["frames"][0][1][0]
        .as_array()
        .expect("frame ball must remain a tuple");
    assert_eq!(ball[0].as_str(), Some(SCRIPT_HOSTILE));
    assert_eq!(
        ball[1..]
            .iter()
            .map(|value| value.as_f64().expect("frame ball scalar"))
            .collect::<Vec<_>>(),
        [12.5, -0.25, 0.125, 1.25, -2.5, 3.75, -4.125, 5.5, -6.625]
    );
}

#[test]
fn svg_report_embeds_canonical_playback_tuple_schema() {
    let report = render_svg_report_from_dsl(
        "table brunswick_gc4_9ft\n\
         ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         trace(max_events: 1)\n\
         shot(cue).heading(0deg).speed(30ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("SVG playback report should render");
    let report: Value = serde_json::from_str(&report).expect("SVG report must be valid JSON");
    let playback = &report["playback"];

    assert_playback_tuple_schema(playback);
    assert!(!playback["events"].as_array().unwrap().is_empty());
    assert!(!playback["balls"].as_array().unwrap().is_empty());
    assert!(!playback["frames"].as_array().unwrap().is_empty());
}
