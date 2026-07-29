use billiards::diagram::BallStyle;
use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace};
use billiards::svg_generator::{
    build_scenario_playback_report, render_svg_report_from_dsl,
    render_svg_report_from_dsl_with_options, serialize_scenario_playback_report,
    ScenarioPlaybackBallReport, ScenarioPlaybackBallVisual, ScenarioPlaybackEventReport,
    ScenarioPlaybackFrameReport, ScenarioPlaybackReport,
};
use billiards::{
    human_tuned_preview_motion_config, CollisionModel, RailModel, Seconds, SvgGeneratorOptions,
    TableSpec,
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
        assert_eq!(ball.len(), 8, "ball visual tuple field count changed");
        assert!(ball[0].is_string());
        assert!(ball[1].is_string());
        assert!(ball[2].is_string() || ball[2].is_null());
        assert!(ball[3].is_number());
        assert!(ball[4].is_number());
        assert!(ball[5].is_string());
        assert!(ball[6].is_string() || ball[6].is_null());
        assert!(ball[7].is_string() || ball[7].is_null());
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

fn contact_number(contact: &Value, field: &str) -> f64 {
    contact[field]
        .as_f64()
        .unwrap_or_else(|| panic!("first-object contact `{field}` must be numeric"))
}

fn assert_contact_centers_are_tangent(contact: &Value) {
    let lateral = contact_number(contact, "lateralOffsetDiameters");
    let forward = contact_number(contact, "forwardOffsetDiameters");
    let vertical = contact_number(contact, "verticalOffsetDiameters");
    let normalized_distance_squared = lateral * lateral + forward * forward + vertical * vertical;
    assert!(
        (normalized_distance_squared - 1.0).abs() <= 1e-5,
        "contact centers must be one ball diameter apart, got {normalized_distance_squared}"
    );
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
            style: BallStyle::Stripe,
            paint: Some(SCRIPT_HOSTILE),
            gradient: Some(SCRIPT_HOSTILE),
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
        "{{\"duration\":1.250000,\"events\":[[\"{0}\",2.500000,\"{0}\"]],\"balls\":[[\"{0}\",\"#aabbcc\",\"{0}\",12.250,1.125000,\"stripe\",\"{0}\",\"{0}\"]],\"frames\":[[0.125000,[[\"{0}\",12.500,-0.250,0.125000,1.250000,-2.500000,3.750000,-4.125000,5.500000,-6.625000]]]]}}",
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
    assert_eq!(visual[5].as_str(), Some("stripe"));
    assert_eq!(visual[6].as_str(), Some(SCRIPT_HOSTILE));
    assert_eq!(visual[7].as_str(), Some(SCRIPT_HOSTILE));

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
    assert!(
        report["firstObjectContact"].is_null(),
        "a shot without a ball-ball collision must not report object contact"
    );

    assert_playback_tuple_schema(playback);
    assert!(!playback["events"].as_array().unwrap().is_empty());
    assert!(!playback["balls"].as_array().unwrap().is_empty());
    assert!(!playback["frames"].as_array().unwrap().is_empty());
}

#[test]
fn svg_report_first_object_contact_uses_exact_on_table_impact_geometry() {
    let report = render_svg_report_from_dsl(include_str!(
        "../examples/scenarios/three_cushion_left_top_right_score.billiards"
    ))
    .expect("carom SVG report should render");
    let report: Value = serde_json::from_str(&report).expect("SVG report must be valid JSON");
    let contact = &report["firstObjectContact"];
    let contact_object = contact
        .as_object()
        .expect("scoring shot must report its first object contact");

    assert_eq!(
        contact_object.len(),
        9,
        "first-object contact field count changed"
    );
    assert_eq!(contact["cueBall"][0].as_str(), Some("cue"));
    assert_eq!(contact["objectBall"][0].as_str(), Some("yellow"));
    assert_eq!(contact["airborne"].as_bool(), Some(false));
    assert_eq!(contact_number(contact, "verticalOffsetDiameters"), 0.0);
    assert_contact_centers_are_tangent(contact);

    let cut_angle = contact_number(contact, "cutAngleDegrees");
    let hit_fraction = contact_number(contact, "hitFraction");
    assert!(
        (30.0..31.0).contains(&cut_angle),
        "canonical carom cut angle changed: {cut_angle}"
    );
    assert!(
        (hit_fraction - (1.0 - cut_angle.to_radians().sin())).abs() <= 2e-6,
        "hit fullness must follow TP A.23"
    );
}

#[test]
fn svg_report_first_object_contact_preserves_airborne_contact_height() {
    let report = render_svg_report_from_dsl(include_str!(
        "../examples/scenarios/jump_over_full_ball_showcase.billiards"
    ))
    .expect("jump SVG report should render");
    let report: Value = serde_json::from_str(&report).expect("SVG report must be valid JSON");
    let contact = &report["firstObjectContact"];

    assert_eq!(contact["cueBall"][0].as_str(), Some("cue"));
    assert_eq!(contact["objectBall"][0].as_str(), Some("two"));
    assert_eq!(contact["airborne"].as_bool(), Some(true));
    assert!(
        contact_number(contact, "verticalOffsetDiameters").abs() > 0.01,
        "airborne contact must retain visible vertical separation"
    );
    assert_contact_centers_are_tangent(contact);

    let cut_angle = contact_number(contact, "cutAngleDegrees");
    let hit_fraction = contact_number(contact, "hitFraction");
    assert!(
        (hit_fraction - (1.0 - cut_angle.to_radians().sin())).abs() <= 2e-6,
        "airborne hit fullness must use the same cut-angle definition"
    );
}

#[test]
fn svg_report_playback_preserves_plain_solid_and_stripe_artwork_metadata() {
    let report = render_svg_report_from_dsl(
        "table brunswick_gc4_9ft\n\
         ball cue at (0.5, 0.5)\n\
         ball one at (1.2, 0.5)\n\
         ball nine at (1.9, 0.5)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         trace(max_events: 1)\n\
         shot(cue).heading(90deg).speed(30ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("SVG playback report should render");
    let report: Value = serde_json::from_str(&report).expect("SVG report must be valid JSON");
    assert_playback_tuple_schema(&report["playback"]);
    let balls = report["playback"]["balls"]
        .as_array()
        .expect("playback ball visuals must be an array");
    let visual = |id: &str| {
        balls
            .iter()
            .find(|ball| ball[0].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing playback visual for {id}"))
    };

    let expected = [
        ("cue", "#f8f4e8", None, "plain", "ivory", "pool-ball-ivory"),
        (
            "one",
            "#f1c232",
            Some("1"),
            "solid",
            "yellow",
            "pool-ball-yellow",
        ),
        (
            "nine",
            "#f1c232",
            Some("9"),
            "stripe",
            "yellow",
            "pool-ball-yellow",
        ),
    ];
    for (id, fill, label, style, paint, gradient) in expected {
        let visual = visual(id)
            .as_array()
            .unwrap_or_else(|| panic!("playback visual for {id} must remain a tuple"));
        assert_eq!(visual[0].as_str(), Some(id), "playback {id} id changed");
        assert_eq!(
            visual[1].as_str(),
            Some(fill),
            "playback {id} fallback fill changed"
        );
        assert_eq!(
            visual[2].as_str(),
            label,
            "playback {id} number label changed"
        );
        assert_eq!(
            visual[5].as_str(),
            Some(style),
            "playback {id} artwork style changed"
        );
        assert_eq!(
            visual[6].as_str(),
            Some(paint),
            "playback {id} paint identity changed"
        );
        assert_eq!(
            visual[7].as_str(),
            Some(gradient),
            "playback {id} gradient identity changed"
        );
    }
}

fn prepared_report_trace(source: &str, event_limit: usize) -> (ScenarioShotTrace, TableSpec) {
    let mut scenario = parse_dsl_to_scenario(source).expect("report fixture should parse");
    scenario.game_state.resolve_positions();
    let table_spec = scenario.game_state.table_spec.clone();
    let ball_set = scenario.ball_set_physics_spec();
    let motion = human_tuned_preview_motion_config();
    let trace = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
            event_limit,
        )
        .expect("report fixture should simulate")
        .expect("report fixture should contain a shot");
    (trace, table_spec)
}

#[test]
fn streamed_svg_playback_matches_the_owned_canonical_serializer_byte_for_byte() {
    const TWO_BALL: &str = "\
table brunswick_gc4_9ft
ball cue at (2.0, 6.0)
ball one at (2.0, 3.0)
cue_strike(default).mass_ratio(1.0).energy_loss(0.1)
shot(cue).heading(180deg).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)
";
    let fixtures = [
        ("two_ball", TWO_BALL, 8),
        (
            "pocket_capture",
            include_str!("../examples/scenarios/straight_in_side_pocket.billiards"),
            32,
        ),
        (
            "airborne",
            include_str!("../examples/scenarios/jump_over_full_ball_showcase.billiards"),
            32,
        ),
        (
            "nine_ball_break",
            include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards"),
            32,
        ),
    ];

    for (name, source, event_limit) in fixtures {
        let options = SvgGeneratorOptions {
            trace_sample_step_seconds: 0.02,
            trace_max_events: Some(event_limit),
            ..SvgGeneratorOptions::default()
        };
        let report = render_svg_report_from_dsl_with_options(source, &options)
            .unwrap_or_else(|error| panic!("{name} report should render: {error}"));
        let playback_marker = ",\"playback\":";
        let playback_start = report
            .rfind(playback_marker)
            .unwrap_or_else(|| panic!("{name} report should contain playback"))
            + playback_marker.len();
        let streamed = report[playback_start..]
            .strip_suffix('}')
            .expect("top-level report should end after playback");

        let (trace, table_spec) = prepared_report_trace(source, event_limit);
        let owned = build_scenario_playback_report(
            &trace,
            &table_spec,
            Seconds::new(options.trace_sample_step_seconds),
        );
        let mut expected = String::new();
        serialize_scenario_playback_report(&mut expected, &owned);

        assert_eq!(streamed, expected, "{name} streamed playback changed bytes");
        let parsed: Value =
            serde_json::from_str(streamed).expect("streamed playback should remain valid JSON");
        assert_playback_tuple_schema(&parsed);
    }
}
