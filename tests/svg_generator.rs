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

#[derive(Clone, Copy)]
struct PoolPaint {
    paint: &'static str,
    gradient: &'static str,
}

#[derive(Clone, Copy)]
struct BallVisual {
    id: &'static str,
    playback_fill: &'static str,
    label: Option<&'static str>,
    shell: PoolPaint,
    stripe: Option<PoolPaint>,
}

const IVORY: PoolPaint = PoolPaint {
    paint: "ivory",
    gradient: "pool-ball-ivory",
};
const YELLOW: PoolPaint = PoolPaint {
    paint: "yellow",
    gradient: "pool-ball-yellow",
};
const BLUE: PoolPaint = PoolPaint {
    paint: "blue",
    gradient: "pool-ball-blue",
};
const RED: PoolPaint = PoolPaint {
    paint: "red",
    gradient: "pool-ball-red",
};
const PURPLE: PoolPaint = PoolPaint {
    paint: "purple",
    gradient: "pool-ball-purple",
};
const ORANGE: PoolPaint = PoolPaint {
    paint: "orange",
    gradient: "pool-ball-orange",
};
const GREEN: PoolPaint = PoolPaint {
    paint: "green",
    gradient: "pool-ball-green",
};
const MAROON: PoolPaint = PoolPaint {
    paint: "maroon",
    gradient: "pool-ball-maroon",
};
const BLACK: PoolPaint = PoolPaint {
    paint: "black",
    gradient: "pool-ball-black",
};

const BALL_VISUALS: [BallVisual; 12] = [
    BallVisual {
        id: "cue",
        playback_fill: "#f8f4e8",
        label: None,
        shell: IVORY,
        stripe: None,
    },
    BallVisual {
        id: "one",
        playback_fill: "#f1c232",
        label: Some("1"),
        shell: YELLOW,
        stripe: None,
    },
    BallVisual {
        id: "two",
        playback_fill: "#2458c8",
        label: Some("2"),
        shell: BLUE,
        stripe: None,
    },
    BallVisual {
        id: "three",
        playback_fill: "#c82828",
        label: Some("3"),
        shell: RED,
        stripe: None,
    },
    BallVisual {
        id: "four",
        playback_fill: "#6f3fa8",
        label: Some("4"),
        shell: PURPLE,
        stripe: None,
    },
    BallVisual {
        id: "five",
        playback_fill: "#e27a22",
        label: Some("5"),
        shell: ORANGE,
        stripe: None,
    },
    BallVisual {
        id: "six",
        playback_fill: "#25834b",
        label: Some("6"),
        shell: GREEN,
        stripe: None,
    },
    BallVisual {
        id: "seven",
        playback_fill: "#8f2d20",
        label: Some("7"),
        shell: MAROON,
        stripe: None,
    },
    BallVisual {
        id: "eight",
        playback_fill: "#111111",
        label: Some("8"),
        shell: BLACK,
        stripe: None,
    },
    BallVisual {
        id: "nine",
        playback_fill: "#f1c232",
        label: Some("9"),
        shell: IVORY,
        stripe: Some(YELLOW),
    },
    BallVisual {
        id: "yellow",
        playback_fill: "#f1c232",
        label: None,
        shell: YELLOW,
        stripe: None,
    },
    BallVisual {
        id: "red",
        playback_fill: "#c82828",
        label: None,
        shell: RED,
        stripe: None,
    },
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

fn svg_element_with_class<'a>(group: &'a str, class: &str) -> &'a str {
    let marker = format!("class=\"{class}\"");
    let start = group
        .find(&marker)
        .unwrap_or_else(|| panic!("missing static SVG element with class {class}"));
    let element = &group[start..];
    let end = element
        .find("/>")
        .unwrap_or_else(|| panic!("unterminated static SVG element with class {class}"));
    &element[..end]
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

    for visual in BALL_VISUALS {
        let group = svg_ball_group(&static_svg, visual.id);
        let shell = svg_element_with_class(group, "ball-shell");
        assert!(
            shell.contains(&format!("data-fill=\"{}\"", visual.shell.paint)),
            "static SVG ball {} should preserve shell paint {}",
            visual.id,
            visual.shell.paint
        );
        assert!(
            shell.contains(&format!("fill=\"url(#{})\"", visual.shell.gradient)),
            "static SVG ball {} should preserve shell gradient {}",
            visual.id,
            visual.shell.gradient
        );

        if let Some(stripe) = visual.stripe {
            let stripe_band = svg_element_with_class(group, "ball-stripe-band");
            assert!(
                stripe_band.contains(&format!("data-fill=\"{}\"", stripe.paint)),
                "static SVG ball {} should preserve stripe paint {}",
                visual.id,
                stripe.paint
            );
            assert!(
                stripe_band.contains(&format!("fill=\"url(#{})\"", stripe.gradient)),
                "static SVG ball {} should preserve stripe gradient {}",
                visual.id,
                stripe.gradient
            );
        } else {
            assert!(
                !group.contains("class=\"ball-stripe-band\""),
                "static SVG ball {} should not have a stripe",
                visual.id
            );
        }

        assert_eq!(
            group.matches("class=\"ball-label").count(),
            usize::from(visual.label.is_some()),
            "static SVG ball {} label presence changed",
            visual.id
        );
        if let Some(label) = visual.label {
            assert!(
                group.contains(&format!(">{label}</text>")),
                "static SVG ball {} should preserve label {label}",
                visual.id
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

    let expected_playback: Vec<_> = BALL_VISUALS
        .iter()
        .map(|visual| (visual.id, visual.playback_fill, visual.label))
        .collect();
    assert_eq!(playback_ball_metadata(&report), expected_playback);
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
