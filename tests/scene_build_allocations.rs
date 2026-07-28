use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

use billiards::dsl::{parse_dsl_to_scenario, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, LabelOverlayStyle, PathColorMode};
use billiards::{
    BallSetPhysicsSpec, CollisionModel, DiagramBackground, DiagramRenderOptions, Diamond,
    GameState, InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, OnTableMotionConfig,
    Position, RadiansPerSecondSq, RailModel, RollingResistanceModel, Seconds, SlidingFrictionModel,
    SpinDecayModel, TableSpec,
};
use image::Rgba;

const TWO_BALL_LAYOUT_DSL: &str = "ball cue at center\nball nine at (2, 4.75)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n";

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

fn assert_consistent(metrics: AllocationMetrics) {
    assert!(metrics.allocations + metrics.reallocations > 0);
    assert!(metrics.current_live_bytes <= metrics.peak_live_bytes);
    assert!(metrics.peak_live_bytes <= metrics.total_requested_bytes);
    assert!(metrics.deallocations <= metrics.allocations + metrics.reallocations);
}

fn motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("5"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(2.0),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn long_polyline_points() -> Vec<Position> {
    (0..1_000)
        .map(|index| {
            let t = index as f64 / 999.0;
            let x = 0.1 + 3.8 * t;
            let y = 4.0 + 3.0 * (t * std::f64::consts::TAU * 8.0).sin();
            Position::new(
                Diamond::from(x.to_string().as_str()),
                Diamond::from(y.to_string().as_str()),
            )
        })
        .collect()
}

fn overlays_only_state() -> GameState {
    let mut state = GameState::new(TableSpec::default());
    state.add_smooth_polyline(&long_polyline_points(), Rgba([0x09, 0x6b, 0xd8, 0xff]));
    let label_style = LabelOverlayStyle::enabled(Rgba([0x20, 0x20, 0x20, 0xff]));
    state.add_text_label_styled(
        &Position::new("0.5", "0.75"),
        "(allocation event)",
        label_style.clone(),
    );
    state.add_text_label_styled(
        &Position::new("3.5", "7.25"),
        "t=1.000 allocation event title",
        label_style,
    );
    state
}

fn rich_trace_state() -> GameState {
    let scenario =
        parse_dsl_to_scenario(TWO_BALL_LAYOUT_DSL).expect("allocation scenario should parse");
    let trace = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("allocation scenario should simulate")
        .expect("allocation scenario should contain a shot");
    let trace_options = ScenarioTraceRenderOptions {
        path_render: BallPathRenderOptions {
            max_time_step: Seconds::new(0.005),
            ..ScenarioTraceRenderOptions::default().path_render
        },
        start_ghost_balls: true,
        event_markers: true,
        labels: false,
        spin_glyphs: true,
        path_color_mode: PathColorMode::MotionPhase,
    };
    trace.rendered_final_layout_with_trace_options(&scenario, &trace_options)
}

#[test]
fn scene_build_allocation_and_peak_live_metrics() {
    let options = DiagramRenderOptions {
        background: DiagramBackground::Transparent,
        ..DiagramRenderOptions::default()
    };
    for (name, state) in [
        ("overlays_only_1000_points", overlays_only_state()),
        ("rich_trace", rich_trace_state()),
    ] {
        let (scene, metrics) = measure_allocations(|| {
            black_box(black_box(&state).to_diagram_scene(black_box(&options)))
        });
        assert_consistent(metrics);
        assert!(!scene.elements.is_empty());
        println!("fixture={name} metrics={metrics:?}");
        black_box(scene);
    }
}
