use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt::Write as _;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

use billiards::dsl::{
    parse_dsl_to_scenario, DslScenario, ScenarioShotTrace, ScenarioTraceRenderOptions,
};
use billiards::svg_generator::{
    build_scenario_playback_report, serialize_prepared_svg_report,
    serialize_scenario_playback_report,
};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    human_tuned_preview_motion_config, CollisionModel, DiagramRenderOptions, RailModel, Seconds,
    TableSpec,
};

const THREE_BALL_PINBALL_DSL: &str = "ball cue at (1.0, 4.0)\nball one at (2.0, 4.2)\nball two at (3.0, 4.9)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(80deg).speed(120ips).tip(side: 0.0R, height: 0.0R).using(default)\n";
const NINE_BALL_BREAK_DSL: &str =
    include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards");

struct CountingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
}

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static TOTAL_REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);
static CURRENT_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

fn tracking_enabled() -> bool {
    TRACK_ALLOCATIONS.with(Cell::get)
}

fn update_peak(candidate: u64) {
    let mut peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    while candidate > peak {
        match PEAK_LIVE_BYTES.compare_exchange_weak(
            peak,
            candidate,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => peak = actual,
        }
    }
}

fn record_allocation(size: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    TOTAL_REQUESTED_BYTES.fetch_add(size as u64, Ordering::Relaxed);
    let current = CURRENT_LIVE_BYTES.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
    update_peak(current);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && tracking_enabled() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && tracking_enabled() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if tracking_enabled() {
            DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            CURRENT_LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() && tracking_enabled() {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            TOTAL_REQUESTED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            let current = if new_size >= layout.size() {
                CURRENT_LIVE_BYTES.fetch_add((new_size - layout.size()) as u64, Ordering::Relaxed)
                    + (new_size - layout.size()) as u64
            } else {
                CURRENT_LIVE_BYTES.fetch_sub((layout.size() - new_size) as u64, Ordering::Relaxed)
                    - (layout.size() - new_size) as u64
            };
            update_peak(current);
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
struct AllocationMetrics {
    allocations: u64,
    reallocations: u64,
    deallocations: u64,
    total_requested_bytes: u64,
    current_live_bytes: u64,
    peak_live_bytes: u64,
}

fn assert_consistent_metrics(metrics: AllocationMetrics) {
    assert!(metrics.allocations + metrics.reallocations > 0);
    assert!(metrics.current_live_bytes <= metrics.peak_live_bytes);
    assert!(metrics.peak_live_bytes <= metrics.total_requested_bytes);
    assert!(metrics.deallocations <= metrics.allocations + metrics.reallocations);
}

fn measure_allocations<T>(operation: impl FnOnce() -> T) -> (T, AllocationMetrics) {
    for counter in [
        &ALLOCATIONS,
        &REALLOCATIONS,
        &DEALLOCATIONS,
        &TOTAL_REQUESTED_BYTES,
        &CURRENT_LIVE_BYTES,
        &PEAK_LIVE_BYTES,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
    TRACK_ALLOCATIONS.with(|enabled| {
        assert!(
            !enabled.replace(true),
            "allocation measurements may not nest"
        );
    });
    let output = operation();
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    let metrics = AllocationMetrics {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
        total_requested_bytes: TOTAL_REQUESTED_BYTES.load(Ordering::Relaxed),
        current_live_bytes: CURRENT_LIVE_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
    };
    (output, metrics)
}

fn prepare_trace(source: &str, event_limit: usize) -> (ScenarioShotTrace, TableSpec, DslScenario) {
    let mut scenario = parse_dsl_to_scenario(source).expect("allocation fixture should parse");
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
        .expect("allocation fixture should simulate")
        .expect("allocation fixture should contain a shot");
    (trace, table_spec, scenario)
}

fn stream_checksum(trace: &ScenarioShotTrace, step: Seconds) -> (usize, usize, u64) {
    let mut frames = 0;
    let mut balls = 0;
    let mut checksum = 0_u64;
    for frame in trace.playback_frames_iter(step) {
        frames += 1;
        checksum = checksum.rotate_left(7) ^ frame.time.as_f64().to_bits();
        for ball in frame.balls {
            balls += 1;
            checksum = checksum.rotate_left(7) ^ ball.state.position.x().as_f64().to_bits();
            checksum = checksum.rotate_left(7) ^ ball.state.position.y().as_f64().to_bits();
            checksum = checksum.rotate_left(7) ^ ball.state.height.as_f64().to_bits();
        }
    }
    (frames, balls, checksum)
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            ch if ch.is_control() => {
                write!(out, "\\u{:04x}", ch as u32)
                    .expect("writing JSON to a string should not fail");
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

fn serialize_owned_svg_report(
    out: &mut String,
    svg: &str,
    trace: &ScenarioShotTrace,
    table_spec: &TableSpec,
    step: Seconds,
) {
    out.push_str("{\"svg\":");
    push_json_string(out, svg);
    out.push_str(",\"events\":[");
    for (index, event) in trace.event_log.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let label = format!("({})", index + 1);
        out.push('[');
        push_json_string(out, &label);
        write!(out, ",{:.6},", event.time.as_f64())
            .expect("writing JSON to a string should not fail");
        push_json_string(out, &event.kind.format_human());
        out.push(',');
        push_json_string(out, &event.format_human());
        out.push(']');
    }
    out.push_str("],\"playback\":");
    let playback = build_scenario_playback_report(trace, table_spec, step);
    serialize_scenario_playback_report(out, &playback);
    out.push('}');
}

#[test]
fn playback_allocation_and_peak_live_metrics() {
    let (three_ball_trace, three_ball_table, three_ball_scenario) =
        prepare_trace(THREE_BALL_PINBALL_DSL, 8);
    let (ten_ball_trace, ten_ball_table, ten_ball_scenario) =
        prepare_trace(NINE_BALL_BREAK_DSL, 32);
    let fixtures = [
        (
            "three_ball_event_limit_8",
            three_ball_trace,
            three_ball_table,
            three_ball_scenario,
        ),
        (
            "ten_ball_event_limit_32",
            ten_ball_trace,
            ten_ball_table,
            ten_ball_scenario,
        ),
    ];

    for (name, trace, table_spec, scenario) in fixtures {
        for step_seconds in [0.020, 0.0025] {
            let step = Seconds::new(step_seconds);
            let (owned, owned_metrics) =
                measure_allocations(|| trace.playback_frames(black_box(step)));
            let (streamed, stream_metrics) =
                measure_allocations(|| stream_checksum(&trace, black_box(step)));
            assert_consistent_metrics(owned_metrics);
            assert_consistent_metrics(stream_metrics);
            assert_eq!(streamed.0, owned.len());
            assert_eq!(
                streamed.1,
                owned.iter().map(|frame| frame.balls.len()).sum::<usize>()
            );
            assert!(
                stream_metrics.peak_live_bytes * 2 < owned_metrics.peak_live_bytes,
                "{name}/{step_seconds} streaming should use less than half the owned peak: stream={stream_metrics:?}, owned={owned_metrics:?}"
            );
            assert!(
                stream_metrics.allocations <= owned_metrics.allocations,
                "{name}/{step_seconds} streaming allocation count regressed: stream={stream_metrics:?}, owned={owned_metrics:?}"
            );

            let trace_options = ScenarioTraceRenderOptions {
                path_render: BallPathRenderOptions {
                    max_time_step: step,
                    ..ScenarioTraceRenderOptions::default().path_render
                },
                start_ghost_balls: true,
                event_markers: true,
                labels: false,
                spin_glyphs: true,
                path_color_mode: PathColorMode::Solid,
            };
            let rendered =
                trace.rendered_final_layout_with_trace_options(&scenario, &trace_options);
            let svg = rendered.draw_2d_svg_with_options(&DiagramRenderOptions::default());
            let (report, report_metrics) = measure_allocations(|| {
                let mut report = String::new();
                serialize_prepared_svg_report(
                    &mut report,
                    &svg,
                    Some(&trace),
                    &table_spec,
                    black_box(step),
                );
                report
            });
            assert_consistent_metrics(report_metrics);
            assert!(!report.is_empty());
            if step_seconds == 0.0025 {
                let (owned_report, owned_report_metrics) = measure_allocations(|| {
                    let mut report = String::new();
                    serialize_owned_svg_report(
                        &mut report,
                        &svg,
                        &trace,
                        &table_spec,
                        black_box(step),
                    );
                    report
                });
                assert_eq!(report, owned_report);
                assert!(
                    report_metrics.peak_live_bytes * 100
                        <= owned_report_metrics.peak_live_bytes * 85,
                    "{name}/{step_seconds} streamed report should reduce the owned-report peak by at least 15%: stream={report_metrics:?}, owned={owned_report_metrics:?}"
                );
            }
            println!(
                "fixture={name} actual_events={} step={step_seconds} owned={owned_metrics:?} stream={stream_metrics:?} report={report_metrics:?}",
                trace.event_log.len(),
            );
            black_box((&owned, streamed, &report));
        }
    }
}
