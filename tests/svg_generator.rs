use billiards::{
    render_svg_from_dsl, render_svg_from_dsl_with_options,
    svg_generator::render_svg_report_from_dsl, SvgGeneratorOptions,
};

#[test]
fn render_svg_from_dsl_renders_static_ball_layout() {
    let svg = render_svg_from_dsl(
        "table brunswick_gc4_9ft\nball cue at center\nball nine at (2.0, 6.0)\n",
    )
    .expect("static SVG should render");

    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("data-layer=\"table\""));
    assert!(svg.contains("class=\"ball ball-cue\""));
    assert!(svg.contains("class=\"ball ball-nine\""));
}

const ALL_BALLS_DSL: &str = "\
table brunswick_gc4_9ft
ball cue at (0.5, 0.5)
ball one at (1.2, 0.5)
ball two at (1.9, 0.5)
ball three at (2.6, 0.5)
ball four at (3.3, 0.5)
ball five at (0.5, 1.2)
ball six at (1.2, 1.2)
ball seven at (1.9, 1.2)
ball eight at (2.6, 1.2)
ball nine at (3.3, 1.2)
ball yellow at (0.5, 1.9)
ball red at (1.2, 1.9)
";

const BALL_VISUALS: [(&str, &str, Option<&str>); 12] = [
    ("cue", "#f8f4e8", None),
    ("one", "#f1c232", Some("1")),
    ("two", "#2458c8", Some("2")),
    ("three", "#c82828", Some("3")),
    ("four", "#6f3fa8", Some("4")),
    ("five", "#e27a22", Some("5")),
    ("six", "#25834b", Some("6")),
    ("seven", "#8f2d20", Some("7")),
    ("eight", "#111111", Some("8")),
    ("nine", "#f1c232", Some("9")),
    ("yellow", "#f1c232", None),
    ("red", "#c82828", None),
];

fn svg_ball_group<'a>(svg: &'a str, id: &str) -> &'a str {
    let marker = format!("class=\"ball ball-{id}\" data-ball=\"{id}\"");
    let start = svg
        .find(&marker)
        .unwrap_or_else(|| panic!("missing static SVG ball {id}"));
    let group = &svg[start..];
    let end = group
        .find("</g>")
        .unwrap_or_else(|| panic!("unterminated static SVG ball {id}"));
    &group[..end]
}

fn unquoted_json_string(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("expected JSON string, got {value}"))
}

fn playback_ball_metadata(report: &str) -> Vec<(&str, &str, Option<&str>)> {
    let playback = report
        .split_once("\"playback\":")
        .expect("SVG report should contain playback JSON")
        .1;
    let balls = playback
        .split_once("\"balls\":[")
        .expect("playback JSON should contain ball metadata")
        .1
        .split_once("],\"frames\":")
        .expect("ball metadata should precede playback frames")
        .0;

    balls
        .split("],[")
        .map(|entry| {
            let fields: Vec<_> = entry
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .take(3)
                .collect();
            assert_eq!(fields.len(), 3, "incomplete playback ball metadata");
            let label = (fields[2] != "null").then(|| unquoted_json_string(fields[2]));
            (
                unquoted_json_string(fields[0]),
                unquoted_json_string(fields[1]),
                label,
            )
        })
        .collect()
}

#[test]
fn static_and_playback_ball_visuals_preserve_every_id_fill_and_optional_label() {
    let static_svg = render_svg_from_dsl(ALL_BALLS_DSL).expect("static SVG should render");

    for (id, fill, label) in BALL_VISUALS {
        let group = svg_ball_group(&static_svg, id);
        assert!(
            group.contains(&format!(
                "fill=\"{fill}\" stroke=\"#111\" stroke-width=\"1.5\""
            )),
            "static SVG ball {id} should preserve fill {fill}"
        );
        assert_eq!(
            group.matches("class=\"ball-label\"").count(),
            usize::from(label.is_some()),
            "static SVG ball {id} label presence changed"
        );
        if let Some(label) = label {
            assert!(
                group.contains(&format!(">{label}</text>")),
                "static SVG ball {id} should preserve label {label}"
            );
        }
    }

    let report = render_svg_report_from_dsl(&format!(
        "{ALL_BALLS_DSL}\
cue_strike(default).mass_ratio(1.0).energy_loss(0.1)
trace(max_events: 1)
shot(cue).heading(90deg).speed(30ips).tip(side: 0.0R, height: 0.0R).using(default)
"
    ))
    .expect("SVG playback report should render");

    assert_eq!(playback_ball_metadata(&report), BALL_VISUALS);
}

#[test]
fn render_svg_from_dsl_renders_shot_trace_without_visible_event_numbers() {
    let svg = render_svg_from_dsl(
        "table brunswick_gc4_9ft\n\
         ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         trace(max_events: 2)\n\
         shot(cue).heading(0deg).speed(30ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("shot trace SVG should render");

    assert!(svg.contains("class=\"overlay ghost-ball\""));
    assert!(svg.contains("class=\"overlay event-marker\""));
    assert!(svg.contains("data-event-label=\"(1)\""));
    assert!(svg.contains("<title>(1) t="));
    assert!(!svg.contains(">1</text>"));
}

#[test]
fn render_svg_from_dsl_rejects_invalid_trace_sample_step() {
    let options = SvgGeneratorOptions {
        trace_sample_step_seconds: 0.0,
        ..SvgGeneratorOptions::default()
    };

    let error = render_svg_from_dsl_with_options("ball cue at center\n", &options)
        .expect_err("zero sample step should be invalid");

    assert_eq!(
        error,
        "trace sample step seconds must be positive and finite"
    );
}
