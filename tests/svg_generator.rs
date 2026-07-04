use billiards::{render_svg_from_dsl, render_svg_from_dsl_with_options, SvgGeneratorOptions};

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
