use billiards::dsl::{
    parse_dsl_to_scenario, DslScenario, ScenarioShotTrace, ScenarioTraceRenderOptions,
};
use billiards::{
    collide_ball_ball_detailed_on_table, write_png_to_file, Angle, Ball, BallSetPhysicsSpec,
    BallSpec, BallType, CollisionModel, CutAngle, DiagramBackground, DiagramRenderOptions, Inches,
    InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, NBallSystemEvent,
    NBallSystemState, OnTableBallState, OnTableMotionConfig, Pocket, Position, RadiansPerSecondSq,
    Rail, RailModel, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, TableSpec,
};
use clap::{Parser, ValueEnum};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const GRAVITY_IPS2: f64 = 386.0886;

fn motion_config(
    sliding_friction_accel_ips2: f64,
    spin_decay_radps2: f64,
    rolling_resistance_accel_ips2: f64,
) -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new(Inches::from_f64(
                sliding_friction_accel_ips2,
            )),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(spin_decay_radps2),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new(Inches::from_f64(
                rolling_resistance_accel_ips2,
            )),
        },
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ProbeStyleArg {
    ForceFollow,
    Stun,
    Draw,
}

impl ProbeStyleArg {
    fn label(self) -> &'static str {
        match self {
            Self::ForceFollow => "force-follow",
            Self::Stun => "stun",
            Self::Draw => "draw",
        }
    }

    fn tip_height_r(self) -> f64 {
        match self {
            Self::ForceFollow => 0.4,
            Self::Stun => 0.0,
            Self::Draw => -0.3,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Sweep follow / stun / draw cut-shot probes and emit summary artifacts"
)]
struct Args {
    /// Output directory for generated .billiards files, PNGs, logs, and summaries.
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Comma-separated cue-ball launch-speed inputs in inches per second.
    #[arg(long, default_value = "64,80,96,112,128")]
    speeds: String,

    /// Comma-separated cut-angle magnitudes in degrees.
    #[arg(long, default_value = "2,4,8,16,32")]
    cut_angles: String,

    /// Restrict the sweep to one or more shot styles. Defaults to force-follow, stun, and draw.
    #[arg(long, value_enum)]
    style: Vec<ProbeStyleArg>,

    /// Distance from the generated ghost-ball position back to the starting cue-ball center.
    #[arg(long, default_value_t = 18.0)]
    cue_distance_inches: f64,

    /// Fixed object-ball X coordinate in diamonds.
    #[arg(long, default_value_t = 2.6)]
    object_x: f64,

    /// Fixed object-ball Y coordinate in diamonds.
    #[arg(long, default_value_t = 4.0)]
    object_y: f64,

    /// Render PNGs for each generated probe case.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    render: bool,

    /// Write each generated probe as a .billiards scenario file.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    write_scenarios: bool,

    /// Write a per-case text log with summary metrics and event lines.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    write_logs: bool,

    /// Write a Markdown summary table.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    write_summary_md: bool,

    /// Write a CSV summary table.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    write_summary_csv: bool,

    /// Render onto a transparent background instead of the table image.
    #[arg(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    transparent_background: bool,

    /// Scale each rendered PNG by this positive integer factor.
    #[arg(long, default_value_t = 1)]
    scale_factor: u32,

    /// Cue-tip side offset in ball-radius units. Positive is right English in the shot frame.
    #[arg(long, default_value_t = 0.0)]
    side_offset_r: f64,

    /// Sliding-friction acceleration magnitude in inches / s^2.
    #[arg(long, default_value_t = 15.0)]
    sliding_friction_accel_ips2: f64,

    /// Rolling-resistance deceleration magnitude in inches / s^2.
    #[arg(long, default_value_t = 5.0)]
    rolling_resistance_accel_ips2: f64,

    /// Vertical-axis spin angular deceleration magnitude in rad / s^2.
    #[arg(long, default_value_t = 10.9)]
    spin_decay_radps2: f64,
}

const MIN_MEANINGFUL_BEND_SPEED_IPS: f64 = 1.0;

#[derive(Clone, Debug)]
struct ProbeCase {
    style: ProbeStyleArg,
    side_offset_r: f64,
    shot_speed_ips: f64,
    requested_cut_deg: f64,
    shot_heading_deg: f64,
    cue_position: Position,
    object_position: Position,
    dsl: String,
    stem: String,
}

#[derive(Clone, Debug)]
struct ProbeResult {
    style: ProbeStyleArg,
    shot_speed_ips: f64,
    cue_launch_speed_ips: f64,
    requested_cut_deg: f64,
    actual_cut_deg: f64,
    shot_heading_deg: f64,
    first_collision_time_s: f64,
    cue_impact_speed_ips: f64,
    simulation_elapsed_s: f64,
    cue_post_contact_speed_ips: f64,
    cue_post_contact_heading_deg: f64,
    throw_angle_deg: Option<f64>,
    cue_bend_deg: Option<f64>,
    cue_bend_duration_s: Option<f64>,
    cue_heading_after_bend_deg: Option<f64>,
    next_rail: Option<Rail>,
    time_to_next_rail_s: Option<f64>,
    cue_rail_hits: usize,
    cue_path_length_inches: f64,
    object_final: String,
    cue_final: String,
    scenario_filename: Option<String>,
    image_filename: Option<String>,
    log_filename: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let speeds = parse_number_list(&args.speeds)?;
    let cut_angles = parse_number_list(&args.cut_angles)?;
    validate_speeds(&speeds)?;
    validate_cut_angles(&cut_angles)?;

    let styles = if args.style.is_empty() {
        vec![
            ProbeStyleArg::ForceFollow,
            ProbeStyleArg::Stun,
            ProbeStyleArg::Draw,
        ]
    } else {
        args.style.clone()
    };

    let output_dir = args.output_dir.clone().unwrap_or_else(default_output_dir);
    fs::create_dir_all(&output_dir)?;

    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config(
        args.sliding_friction_accel_ips2,
        args.spin_decay_radps2,
        args.rolling_resistance_accel_ips2,
    );
    let mut results = Vec::new();

    println!(
        "Generating {} probe cases into {}",
        styles.len() * speeds.len() * cut_angles.len(),
        output_dir.display()
    );
    println!(
        "motion: slide={} ips^2 (mu_s≈{}), roll={} ips^2 (mu_r≈{}), spin={} rad/s^2, side={}R",
        format_decimal(args.sliding_friction_accel_ips2),
        format_decimal(args.sliding_friction_accel_ips2 / GRAVITY_IPS2),
        format_decimal(args.rolling_resistance_accel_ips2),
        format_decimal(args.rolling_resistance_accel_ips2 / GRAVITY_IPS2),
        format_decimal(args.spin_decay_radps2),
        format_decimal(args.side_offset_r),
    );

    for style in styles {
        for &speed in &speeds {
            for &cut_angle in &cut_angles {
                let probe = build_probe_case(
                    &table,
                    style,
                    args.side_offset_r,
                    speed,
                    cut_angle,
                    args.cue_distance_inches,
                    args.object_x,
                    args.object_y,
                );
                let result = run_probe_case(&probe, &output_dir, &ball_set, &motion, &args)?;
                println!(
                    "{:>12}  speed={:>6.1}  cut={:>5.1}  impact={:>5.2}  bend={:>7}  cue={}  obj={}",
                    result.style.label(),
                    result.shot_speed_ips,
                    result.requested_cut_deg,
                    result.actual_cut_deg,
                    format_option(result.cue_bend_deg, 2),
                    result.cue_final,
                    result.object_final,
                );
                results.push(result);
            }
        }
    }

    if args.write_summary_csv {
        fs::write(output_dir.join("summary.csv"), summary_csv(&results, &args))?;
    }
    if args.write_summary_md {
        fs::write(
            output_dir.join("summary.md"),
            summary_markdown(&results, &args, &output_dir),
        )?;
    }

    println!("Done: {}", output_dir.display());
    if args.write_summary_md {
        println!(
            "Markdown summary: {}",
            output_dir.join("summary.md").display()
        );
    }
    if args.write_summary_csv {
        println!("CSV summary: {}", output_dir.join("summary.csv").display());
    }

    Ok(())
}

fn run_probe_case(
    probe: &ProbeCase,
    output_dir: &Path,
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    args: &Args,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    if args.write_scenarios {
        fs::write(
            output_dir.join(format!("{}.billiards", probe.stem)),
            &probe.dsl,
        )?;
    }

    let scenario = parse_dsl_to_scenario(&probe.dsl)?;
    let launch = scenario
        .validate_shot_human_speed()?
        .expect("generated probe should always include a shot");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            ball_set,
            motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )?
        .expect("generated probe should simulate a shot trace");

    let cue_trace = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Cue)
        .expect("probe trace should include the cue ball");
    let object_trace = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::One)
        .expect("probe trace should include the object ball");
    let cue_rail_hits = trace
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                billiards::dsl::ScenarioShotTraceEventKind::BallRailImpact {
                    ball: BallType::Cue,
                    ..
                }
            )
        })
        .count();
    let cue_path_length_inches = trace_path_length_inches(cue_trace);

    let collision = first_cue_object_collision(&scenario, &trace)
        .expect("generated probe should contain an initial cue/object collision");
    let cue_impact_speed_ips = collision.cue_impact.as_ball_state().speed().as_f64();
    let cue_impact_heading = collision
        .cue_impact
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("cue should be moving at impact");
    let object_line = angle_between_states(
        collision.cue_impact.as_ball_state(),
        collision.object_impact.as_ball_state(),
    );
    let actual_cut_deg = CutAngle::from_headings(cue_impact_heading, object_line).as_degrees();
    let outcome = collide_ball_ball_detailed_on_table(
        &collision.cue_impact,
        &collision.object_impact,
        CollisionModel::ThrowAware,
    );
    let cue_post_contact_speed_ips = outcome.a_after.as_ball_state().speed().as_f64();
    let cue_post_contact_heading_deg = outcome
        .a_after
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("cue should still be moving after collision")
        .as_degrees();
    let raw_cue_bend = outcome.estimate_post_contact_cue_ball_bend(ball_set, motion);
    let cue_bend = if cue_post_contact_speed_ips >= MIN_MEANINGFUL_BEND_SPEED_IPS {
        raw_cue_bend.clone()
    } else {
        None
    };
    let cue_heading_after_bend_deg = cue_bend.as_ref().and_then(|bend| {
        bend.state_after_bend
            .as_ball_state()
            .velocity
            .angle_from_north()
            .map(|angle| angle.as_degrees())
    });
    let next_rail = outcome.cue_ball_continuation().next_rail_impact(
        ball_set,
        &scenario.game_state.table_spec,
        motion,
    );

    let image_filename = if args.render {
        let render_path = output_dir.join(format!("{}.png", probe.stem));
        let render_state = trace.rendered_final_layout_with_trace_options(
            &scenario,
            &ScenarioTraceRenderOptions {
                path_color_mode: billiards::visualization::PathColorMode::MotionPhase,
                ..ScenarioTraceRenderOptions::rich_defaults()
            },
            ball_set,
            motion,
        );
        let image = render_state.draw_2d_diagram_with_options(&DiagramRenderOptions {
            scale_factor: args.scale_factor.max(1),
            background: if args.transparent_background {
                DiagramBackground::Transparent
            } else {
                DiagramBackground::Table
            },
        });
        write_png_to_file(&image, Some(&render_path));
        Some(file_name_string(&render_path))
    } else {
        None
    };

    let mut result = ProbeResult {
        style: probe.style,
        shot_speed_ips: probe.shot_speed_ips,
        cue_launch_speed_ips: launch.estimated_cue_ball_speed_after_impact.as_f64(),
        requested_cut_deg: probe.requested_cut_deg,
        actual_cut_deg,
        shot_heading_deg: probe.shot_heading_deg,
        first_collision_time_s: collision.time_s,
        cue_impact_speed_ips,
        simulation_elapsed_s: trace.simulation.elapsed.as_f64(),
        cue_post_contact_speed_ips,
        cue_post_contact_heading_deg,
        throw_angle_deg: outcome.throw_angle_degrees,
        cue_bend_deg: cue_bend.as_ref().map(|bend| bend.bend_angle_degrees),
        cue_bend_duration_s: cue_bend
            .as_ref()
            .map(|bend| bend.time_until_bend_completes.as_f64()),
        cue_heading_after_bend_deg,
        next_rail: next_rail.as_ref().map(|impact| impact.rail),
        time_to_next_rail_s: next_rail
            .as_ref()
            .map(|impact| impact.time_until_impact.as_f64()),
        cue_rail_hits,
        cue_path_length_inches,
        object_final: final_state_label(&object_trace.final_state),
        cue_final: final_state_label(&cue_trace.final_state),
        scenario_filename: args
            .write_scenarios
            .then(|| format!("{}.billiards", probe.stem)),
        image_filename,
        log_filename: args.write_logs.then(|| format!("{}.log", probe.stem)),
    };

    if args.write_logs {
        let log_path = output_dir.join(format!("{}.log", probe.stem));
        fs::write(
            &log_path,
            per_case_log(
                probe,
                &result,
                &scenario,
                &trace,
                raw_cue_bend.as_ref(),
                args,
            ),
        )?;
        result.log_filename = Some(file_name_string(&log_path));
    }

    Ok(result)
}

struct CueObjectCollision {
    time_s: f64,
    cue_impact: OnTableBallState,
    object_impact: OnTableBallState,
}

fn first_cue_object_collision(
    scenario: &DslScenario,
    trace: &ScenarioShotTrace,
) -> Option<CueObjectCollision> {
    let ball_types: Vec<BallType> = scenario
        .game_state
        .balls()
        .iter()
        .map(|ball| ball.ty.clone())
        .collect();
    let mut elapsed = 0.0;

    for event in &trace.simulation.events {
        elapsed += event.time().as_f64();
        let NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } = event
        else {
            continue;
        };

        let first_type = &ball_types[*first_ball_index];
        let second_type = &ball_types[*second_ball_index];
        match (first_type, second_type) {
            (BallType::Cue, BallType::One) => {
                return Some(CueObjectCollision {
                    time_s: elapsed,
                    cue_impact: collision.a_at_impact.clone(),
                    object_impact: collision.b_at_impact.clone(),
                });
            }
            (BallType::One, BallType::Cue) => {
                return Some(CueObjectCollision {
                    time_s: elapsed,
                    cue_impact: collision.b_at_impact.clone(),
                    object_impact: collision.a_at_impact.clone(),
                });
            }
            _ => continue,
        }
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn build_probe_case(
    table: &TableSpec,
    style: ProbeStyleArg,
    side_offset_r: f64,
    shot_speed_ips: f64,
    requested_cut_deg: f64,
    cue_distance_inches: f64,
    object_x: f64,
    object_y: f64,
) -> ProbeCase {
    let object_position = position_from_diamonds(object_x, object_y);
    let object_ball = Ball {
        ty: BallType::One,
        position: object_position.clone(),
        spec: BallSpec::default(),
    };
    let ghost_ball = object_ball.ghost_ball_to_pocket(Pocket::CenterRight, table);
    let shot_heading_deg = 90.0 - requested_cut_deg;
    let heading_radians = shot_heading_deg.to_radians();
    let shot_heading = Angle::from_north(heading_radians.sin(), heading_radians.cos());
    let mut cue_position = ghost_ball.translate_inches(
        Inches::from_f64(cue_distance_inches),
        shot_heading.flipped(),
    );
    cue_position.resolve_shifts(table);

    let stem = format!(
        "{}_side{}_v{}_cut{}",
        style.label().replace('-', "_"),
        slug_number(side_offset_r),
        slug_number(shot_speed_ips),
        slug_number(requested_cut_deg)
    );
    let dsl = format!(
        "# Generated follow/draw/stun cut-shot probe\n# style: {}\n# requested cut angle: {} deg\n# cue-ball launch-speed input: {} ips\n# side offset: {}R\nball cue at ({}, {})\nball one at ({}, {})\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading({}deg).speed({}ips).tip(side: {}R, height: {}R).using(default)\n",
        style.label(),
        format_decimal(requested_cut_deg),
        format_decimal(shot_speed_ips),
        format_decimal(side_offset_r),
        diamond_string(&cue_position.x),
        diamond_string(&cue_position.y),
        diamond_string(&object_position.x),
        diamond_string(&object_position.y),
        format_decimal(shot_heading_deg),
        format_decimal(shot_speed_ips),
        format_decimal(side_offset_r),
        format_decimal(style.tip_height_r()),
    );

    ProbeCase {
        style,
        side_offset_r,
        shot_speed_ips,
        requested_cut_deg,
        shot_heading_deg,
        cue_position,
        object_position,
        dsl,
        stem,
    }
}

fn per_case_log(
    probe: &ProbeCase,
    result: &ProbeResult,
    _scenario: &DslScenario,
    trace: &ScenarioShotTrace,
    raw_cue_bend: Option<&billiards::PostContactCueBallBend>,
    args: &Args,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "style: {}", result.style.label());
    let _ = writeln!(
        out,
        "side_offset_r: {}",
        format_decimal(probe.side_offset_r)
    );
    let _ = writeln!(
        out,
        "sliding_friction_accel_ips2: {}",
        format_decimal(args.sliding_friction_accel_ips2)
    );
    let _ = writeln!(
        out,
        "rolling_resistance_accel_ips2: {}",
        format_decimal(args.rolling_resistance_accel_ips2)
    );
    let _ = writeln!(
        out,
        "spin_decay_radps2: {}",
        format_decimal(args.spin_decay_radps2)
    );
    let _ = writeln!(
        out,
        "effective_mu_s: {}",
        format_decimal(args.sliding_friction_accel_ips2 / GRAVITY_IPS2)
    );
    let _ = writeln!(
        out,
        "effective_mu_r: {}",
        format_decimal(args.rolling_resistance_accel_ips2 / GRAVITY_IPS2)
    );
    let _ = writeln!(
        out,
        "requested_cut_deg: {}",
        format_decimal(result.requested_cut_deg)
    );
    let _ = writeln!(
        out,
        "actual_cut_deg: {}",
        format_decimal(result.actual_cut_deg)
    );
    let _ = writeln!(
        out,
        "shot_heading_deg: {}",
        format_decimal(result.shot_heading_deg)
    );
    let _ = writeln!(
        out,
        "shot_launch_speed_input_ips: {}",
        format_decimal(result.shot_speed_ips)
    );
    let _ = writeln!(
        out,
        "cue_launch_speed_ips: {}",
        format_decimal(result.cue_launch_speed_ips)
    );
    let _ = writeln!(
        out,
        "first_collision_time_s: {}",
        format_decimal(result.first_collision_time_s)
    );
    let _ = writeln!(
        out,
        "cue_impact_speed_ips: {}",
        format_decimal(result.cue_impact_speed_ips)
    );
    let _ = writeln!(
        out,
        "simulation_elapsed_s: {}",
        format_decimal(result.simulation_elapsed_s)
    );
    let _ = writeln!(
        out,
        "cue_post_contact_speed_ips: {}",
        format_decimal(result.cue_post_contact_speed_ips)
    );
    let _ = writeln!(
        out,
        "cue_post_contact_heading_deg: {}",
        format_decimal(result.cue_post_contact_heading_deg)
    );
    let _ = writeln!(
        out,
        "throw_angle_deg: {}",
        format_option(result.throw_angle_deg, 4)
    );
    let _ = writeln!(
        out,
        "cue_bend_deg: {}",
        format_option(result.cue_bend_deg, 4)
    );
    let _ = writeln!(
        out,
        "cue_bend_duration_s: {}",
        format_option(result.cue_bend_duration_s, 4)
    );
    let _ = writeln!(
        out,
        "cue_heading_after_bend_deg: {}",
        format_option(result.cue_heading_after_bend_deg, 4)
    );
    let _ = writeln!(
        out,
        "next_rail: {}",
        result.next_rail.map(rail_name).unwrap_or("-")
    );
    let _ = writeln!(
        out,
        "time_to_next_rail_s: {}",
        format_option(result.time_to_next_rail_s, 4)
    );
    let _ = writeln!(out, "cue_rail_hits: {}", result.cue_rail_hits);
    let _ = writeln!(
        out,
        "cue_path_length_inches: {}",
        format_decimal(result.cue_path_length_inches)
    );
    let _ = writeln!(out, "cue_final: {}", result.cue_final);
    let _ = writeln!(out, "object_final: {}", result.object_final);
    let _ = writeln!(
        out,
        "cue_position: ({}, {})",
        diamond_string(&probe.cue_position.x),
        diamond_string(&probe.cue_position.y)
    );
    let _ = writeln!(
        out,
        "object_position: ({}, {})",
        diamond_string(&probe.object_position.x),
        diamond_string(&probe.object_position.y)
    );
    if let Some(bend) = raw_cue_bend {
        let final_heading = bend
            .state_after_bend
            .as_ball_state()
            .velocity
            .angle_from_north()
            .map(|angle| angle.as_degrees())
            .map(format_decimal)
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(out, "raw_bend_state_heading_deg: {}", final_heading);
        if result.cue_post_contact_speed_ips < MIN_MEANINGFUL_BEND_SPEED_IPS {
            let _ = writeln!(
                out,
                "bend_note: suppressed in summary because post-contact cue speed fell below {} ips",
                format_decimal(MIN_MEANINGFUL_BEND_SPEED_IPS)
            );
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "events:");
    for line in trace.event_lines() {
        let _ = writeln!(out, "  {line}");
    }
    out
}

fn trace_path_length_inches(ball_trace: &billiards::dsl::ScenarioBallTrace) -> f64 {
    ball_trace
        .segments
        .iter()
        .map(|segment| {
            let start = segment.start.as_ball_state();
            let end = segment.end.as_ball_state();
            let dx = end.position.x().as_f64() - start.position.x().as_f64();
            let dy = end.position.y().as_f64() - start.position.y().as_f64();
            dx.hypot(dy)
        })
        .sum()
}

fn final_state_label(state: &NBallSystemState) -> String {
    match state {
        NBallSystemState::OnTable(on_table) => format!(
            "on-table@({:.2},{:.2})",
            on_table.as_ball_state().position.x().as_f64(),
            on_table.as_ball_state().position.y().as_f64()
        ),
        NBallSystemState::Pocketed { pocket, .. } => format!("pocketed:{}", pocket_name(*pocket)),
    }
}

fn angle_between_states(a: &billiards::BallState, b: &billiards::BallState) -> Angle {
    let dx = b.position.x().as_f64() - a.position.x().as_f64();
    let dy = b.position.y().as_f64() - a.position.y().as_f64();
    Angle::from_north(dx, dy)
}

fn summary_csv(results: &[ProbeResult], args: &Args) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "style,side_offset_r,sliding_friction_accel_ips2,rolling_resistance_accel_ips2,spin_decay_radps2,effective_mu_s,effective_mu_r,shot_launch_speed_input_ips,cue_launch_speed_ips,requested_cut_deg,actual_cut_deg,shot_heading_deg,first_collision_time_s,cue_impact_speed_ips,simulation_elapsed_s,cue_post_contact_speed_ips,cue_post_contact_heading_deg,throw_angle_deg,cue_bend_deg,cue_bend_duration_s,cue_heading_after_bend_deg,next_rail,time_to_next_rail_s,cue_rail_hits,cue_path_length_inches,object_final,cue_final,scenario_file,image_file,log_file"
    );
    for result in results {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            result.style.label(),
            format_decimal(args.side_offset_r),
            format_decimal(args.sliding_friction_accel_ips2),
            format_decimal(args.rolling_resistance_accel_ips2),
            format_decimal(args.spin_decay_radps2),
            format_decimal(args.sliding_friction_accel_ips2 / GRAVITY_IPS2),
            format_decimal(args.rolling_resistance_accel_ips2 / GRAVITY_IPS2),
            format_decimal(result.shot_speed_ips),
            format_decimal(result.cue_launch_speed_ips),
            format_decimal(result.requested_cut_deg),
            format_decimal(result.actual_cut_deg),
            format_decimal(result.shot_heading_deg),
            format_decimal(result.first_collision_time_s),
            format_decimal(result.cue_impact_speed_ips),
            format_decimal(result.simulation_elapsed_s),
            format_decimal(result.cue_post_contact_speed_ips),
            format_decimal(result.cue_post_contact_heading_deg),
            format_option(result.throw_angle_deg, 6),
            format_option(result.cue_bend_deg, 6),
            format_option(result.cue_bend_duration_s, 6),
            format_option(result.cue_heading_after_bend_deg, 6),
            result.next_rail.map(rail_name).unwrap_or("-"),
            format_option(result.time_to_next_rail_s, 6),
            result.cue_rail_hits,
            format_decimal(result.cue_path_length_inches),
            result.object_final,
            result.cue_final,
            result.scenario_filename.as_deref().unwrap_or("-"),
            result.image_filename.as_deref().unwrap_or("-"),
            result.log_filename.as_deref().unwrap_or("-"),
        );
    }
    out
}

fn summary_markdown(results: &[ProbeResult], args: &Args, output_dir: &Path) -> String {
    let mut out = String::new();
    let styles = if args.style.is_empty() {
        "force-follow, stun, draw".to_string()
    } else {
        args.style
            .iter()
            .map(|style| style.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let _ = writeln!(out, "# Cut-shot probe sweep");
    let _ = writeln!(out);
    let _ = writeln!(out, "Generated into `{}`.", output_dir.display());
    let _ = writeln!(out);
    let _ = writeln!(out, "## Geometry");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "- object ball fixed at `({}, {})` diamonds",
        format_decimal(args.object_x),
        format_decimal(args.object_y)
    );
    let _ = writeln!(out, "- object-ball target: `center-right` side pocket");
    let _ = writeln!(
        out,
        "- cue-ball start distance from ghost-ball target: `{}` in",
        format_decimal(args.cue_distance_inches)
    );
    let _ = writeln!(out, "- style sweep: `{styles}`");
    let _ = writeln!(
        out,
        "- tip heights: force-follow `+0.4R`, stun `0.0R`, draw `-0.3R`"
    );
    let _ = writeln!(
        out,
        "- side offset: `{}`R",
        format_decimal(args.side_offset_r)
    );
    let _ = writeln!(out, "- shot heading convention: `90° - cut_angle`, so the cue approaches from the lower-left side of the line");
    let _ = writeln!(out, "- bend columns are suppressed when the cue leaves contact below `1 ips`, because near-stop headings are not visually meaningful");
    let _ = writeln!(out);
    let _ = writeln!(out, "## Motion config");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "- sliding friction accel: `{}` ips² (`mu_s ≈ {}`)",
        format_decimal(args.sliding_friction_accel_ips2),
        format_decimal(args.sliding_friction_accel_ips2 / GRAVITY_IPS2)
    );
    let _ = writeln!(
        out,
        "- rolling resistance accel: `{}` ips² (`mu_r ≈ {}`)",
        format_decimal(args.rolling_resistance_accel_ips2),
        format_decimal(args.rolling_resistance_accel_ips2 / GRAVITY_IPS2)
    );
    let _ = writeln!(
        out,
        "- z-spin decay: `{}` rad/s²",
        format_decimal(args.spin_decay_radps2)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "## Results");
    let _ = writeln!(out);
    let _ = writeln!(out, "| style | speed | cut req | cut act | impact | elapsed | throw | bend | next rail | cue rails | object | cue | png | log |");
    let _ = writeln!(
        out,
        "|---|---:|---:|---:|---:|---:|---:|---:|---|---:|---|---|---|---|"
    );
    for result in results {
        let png = result
            .image_filename
            .as_ref()
            .map(|name| format!("[png]({name})"))
            .unwrap_or_else(|| "-".to_string());
        let log = result
            .log_filename
            .as_ref()
            .map(|name| format!("[log]({name})"))
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            result.style.label(),
            format_decimal(result.shot_speed_ips),
            format_decimal(result.requested_cut_deg),
            format_decimal(result.actual_cut_deg),
            format_decimal(result.cue_impact_speed_ips),
            format_decimal(result.simulation_elapsed_s),
            format_option(result.throw_angle_deg, 2),
            format_option(result.cue_bend_deg, 2),
            result.next_rail.map(rail_name).unwrap_or("-"),
            result.cue_rail_hits,
            result.object_final,
            result.cue_final,
            png,
            log,
        );
    }
    out
}

fn parse_number_list(input: &str) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    let values = input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<f64>())
        .collect::<Result<Vec<_>, _>>()?;
    if values.is_empty() {
        return Err("expected at least one numeric value".into());
    }
    Ok(values)
}

fn validate_speeds(speeds: &[f64]) -> Result<(), Box<dyn std::error::Error>> {
    if speeds
        .iter()
        .any(|speed| !speed.is_finite() || *speed <= 0.0)
    {
        return Err("all speeds must be finite positive values".into());
    }
    Ok(())
}

fn validate_cut_angles(cut_angles: &[f64]) -> Result<(), Box<dyn std::error::Error>> {
    if cut_angles
        .iter()
        .any(|angle| !angle.is_finite() || *angle < 0.0 || *angle > 89.999)
    {
        return Err("all cut angles must be finite values in [0, 90) degrees".into());
    }
    Ok(())
}

fn position_from_diamonds(x: f64, y: f64) -> Position {
    let x = format_decimal(x);
    let y = format_decimal(y);
    Position::new(x.as_str(), y.as_str())
}

fn pocket_name(pocket: Pocket) -> &'static str {
    match pocket {
        Pocket::TopLeft => "top-left",
        Pocket::TopRight => "top-right",
        Pocket::CenterRight => "center-right",
        Pocket::BottomRight => "bottom-right",
        Pocket::BottomLeft => "bottom-left",
        Pocket::CenterLeft => "center-left",
    }
}

fn rail_name(rail: Rail) -> &'static str {
    match rail {
        Rail::Top => "top",
        Rail::Right => "right",
        Rail::Bottom => "bottom",
        Rail::Left => "left",
    }
}

fn diamond_string(value: &billiards::Diamond) -> String {
    value.magnitude.to_string()
}

fn file_name_string(path: &Path) -> String {
    path.file_name()
        .expect("generated path should have a file name")
        .to_string_lossy()
        .into_owned()
}

fn default_output_dir() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_secs();
    PathBuf::from(format!("/tmp/billiards-cut-probes-{timestamp}"))
}

fn slug_number(value: f64) -> String {
    format_decimal(value).replace('-', "m").replace('.', "p")
}

fn format_decimal(value: f64) -> String {
    let mut s = format!("{value:.6}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn format_option(value: Option<f64>, precision: usize) -> String {
    match value {
        Some(value) => {
            let mut s = format!("{value:.precision$}", precision = precision);
            while s.contains('.') && s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
            s
        }
        None => "-".to_string(),
    }
}
