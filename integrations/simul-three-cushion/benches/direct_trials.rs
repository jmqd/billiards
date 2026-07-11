use std::{hint::black_box, num::NonZeroUsize};

use billiards::shot_simulation::{
    execute_three_cushion_compact, PhysicsProfile, ShotControls, ShotLayout, ShotLimit,
    ShotSimulationError, ThreeCushionAdjudication, ThreeCushionShooter, ThreeCushionShot,
};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use simul_three_cushion::{
    known_carom_fixture, run, Bounds, Controls, ExperimentConfig, KnownFixture, Mode, NoiseWidths,
    Shooter, TrialDisposition,
};

const MAX_EVENTS: usize = 64;
const SERIAL_BATCH_SIZE: u32 = 32;

struct PreparedFixture {
    physics: PhysicsProfile,
    layout: ShotLayout,
    shot: ThreeCushionShot,
}

fn prepare_fixture(fixture: &KnownFixture) -> Result<PreparedFixture, ShotSimulationError> {
    let [white, yellow, red] = fixture.positions;
    let physics = PhysicsProfile::three_cushion_default();
    let layout = ShotLayout::three_cushion_from_diamonds(
        (white.x, white.y),
        (yellow.x, yellow.y),
        (red.x, red.y),
    )?;
    let Controls {
        heading,
        speed,
        tip_side,
        tip_height,
        elevation,
    } = fixture.controls;
    let controls = ShotControls::new(heading, speed, tip_side, tip_height, elevation)?;
    let shot = ThreeCushionShot::new(ThreeCushionShooter::Cue, controls);

    Ok(PreparedFixture {
        physics,
        layout,
        shot,
    })
}

fn serial_batch_config(fixture: KnownFixture) -> ExperimentConfig {
    let zero_noise = NoiseWidths {
        heading: 0.0,
        speed: 0.0,
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 0.0,
    };
    let Controls {
        heading,
        speed,
        tip_side,
        tip_height,
        elevation,
    } = fixture.controls;

    ExperimentConfig {
        mode: Mode::Sensitivity,
        shooter: Shooter::White,
        positions: fixture.positions,
        nominal: fixture.controls,
        sensitivity_centers: vec![fixture.controls],
        perturbations: zero_noise,
        execution_noise: zero_noise,
        search_bounds: [
            Bounds::new(heading, heading),
            Bounds::new(speed, speed),
            Bounds::new(tip_side, tip_side),
            Bounds::new(tip_height, tip_height),
            Bounds::new(elevation, elevation),
        ],
        master_seed: 7,
        candidate_budget: 1,
        replication_budget: SERIAL_BATCH_SIZE,
        workers: NonZeroUsize::MIN,
        max_events: MAX_EVENTS,
    }
}

fn validate_scored_fixture(prepared: &PreparedFixture) {
    let result = execute_three_cushion_compact(
        &prepared.physics,
        &prepared.layout,
        &prepared.shot,
        ShotLimit::EventCount(MAX_EVENTS),
    )
    .expect("known fixture should execute before benchmarking");

    let ThreeCushionAdjudication::Scored(facts) = &result.completion.summary else {
        panic!("known fixture must adjudicate as scored before benchmarking");
    };
    assert!(facts.completion.is_some());
    assert!(facts.cushion_contacts_before_completion >= 3);
    assert!(facts
        .first_three_qualifying_cushions
        .iter()
        .all(Option::is_some));
    assert_eq!(result.final_states.len(), 3);
}

fn validate_serial_batch(config: &ExperimentConfig) {
    config
        .validate()
        .expect("serial benchmark configuration should validate");
    let report = run(config).expect("known fixture serial batch should execute before timing");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.trials.len(), SERIAL_BATCH_SIZE as usize);
    let candidate = &report.candidates[0];
    assert_eq!(candidate.requested, SERIAL_BATCH_SIZE);
    assert_eq!(candidate.scored, SERIAL_BATCH_SIZE);
    assert_eq!(candidate.missed, 0);
    assert_eq!(candidate.indeterminate, 0);
    assert_eq!(candidate.failed, 0);
    assert_eq!(candidate.success_rate, Some(1.0));
    assert!(report
        .trials
        .iter()
        .all(|trial| trial.disposition == TrialDisposition::Scored));
}

fn benchmark_direct_trials(c: &mut Criterion) {
    let known = known_carom_fixture();
    let prepared = prepare_fixture(&known).expect("known fixture should prepare");
    let batch_config = serial_batch_config(known);

    // These checks stay outside every timed loop so timings cannot hide a semantic regression.
    validate_scored_fixture(&prepared);
    validate_serial_batch(&batch_config);

    // Measures only construction and validation of the immutable physics/layout/shot fixture.
    let mut preparation = c.benchmark_group("carom_fixture");
    preparation.bench_function("prepare_immutable", |b| {
        b.iter(|| black_box(prepare_fixture(black_box(&known))));
    });
    preparation.finish();

    let mut trials = c.benchmark_group("carom_trial");
    trials.throughput(Throughput::Elements(1));
    // The physics profile, layout, shot controls, and event limit are all outside this loop.
    trials.bench_function("direct_compact", |b| {
        b.iter(|| {
            let output = execute_three_cushion_compact(
                black_box(&prepared.physics),
                black_box(&prepared.layout),
                black_box(&prepared.shot),
                ShotLimit::EventCount(MAX_EVENTS),
            );
            black_box(output)
        });
    });

    trials.throughput(Throughput::Elements(u64::from(SERIAL_BATCH_SIZE)));
    // Measures a fixed serial simul batch plus typed report assembly; no CLI, DSL, I/O, or formatting.
    trials.bench_function("serial_batch_32", |b| {
        b.iter(|| {
            let output = run(black_box(&batch_config));
            black_box(output)
        });
    });
    trials.finish();
}

criterion_group!(benches, benchmark_direct_trials);
criterion_main!(benches);
