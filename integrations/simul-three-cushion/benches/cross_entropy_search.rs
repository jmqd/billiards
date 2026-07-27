use std::{hint::black_box, num::NonZeroUsize, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use simul_three_cushion::{
    known_carom_fixture, run, Bounds, Controls, ExperimentConfig, Mode, NoiseSigmas,
    PerturbationWidths, Shooter,
};

const CANDIDATES: usize = 24;
const SCREENING_REPLICATIONS: u32 = 2;
const FINALISTS: usize = 4;
const VALIDATION_REPLICATIONS: u32 = 4;
const TOTAL_PHYSICS_EVALUATIONS: u64 = CANDIDATES as u64 * SCREENING_REPLICATIONS as u64
    + FINALISTS as u64 * VALIDATION_REPLICATIONS as u64;
const MAX_EVENTS: usize = 64;

fn search_config(workers: NonZeroUsize) -> ExperimentConfig {
    let fixture = known_carom_fixture();
    let shot_inaccuracy = NoiseSigmas {
        heading: 0.25,
        speed: 1.5,
        tip_side: 0.008,
        tip_height: 0.008,
        elevation: 0.15,
    };
    let zero_perturbation = PerturbationWidths {
        heading: 0.0,
        speed: 0.0,
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 0.0,
    };
    let nominal = Controls {
        elevation: 3.0 * shot_inaccuracy.elevation,
        ..fixture.controls
    };

    ExperimentConfig {
        mode: Mode::Search,
        shooter: Shooter::White,
        positions: fixture.positions,
        nominal,
        additional_candidates: Vec::new(),
        perturbations: zero_perturbation,
        shot_inaccuracy,
        search_bounds: [
            Bounds::new(0.0, 360.0),
            Bounds::new(
                1.0 + 3.0 * shot_inaccuracy.speed,
                300.0 - 3.0 * shot_inaccuracy.speed,
            ),
            Bounds::new(-0.45, 0.45),
            Bounds::new(-0.45, 0.45),
            Bounds::new(
                3.0 * shot_inaccuracy.elevation,
                85.0 - 1.0e-12 - 3.0 * shot_inaccuracy.elevation,
            ),
        ],
        master_seed: 0x524f_4255_5354_0001,
        candidate_budget: CANDIDATES,
        screening_replication_budget: SCREENING_REPLICATIONS,
        finalist_budget: FINALISTS,
        validation_replication_budget: VALIDATION_REPLICATIONS,
        workers,
        max_events: MAX_EVENTS,
    }
}

fn validate_search(config: &ExperimentConfig) {
    config
        .validate()
        .expect("CEM search benchmark configuration should validate");
    let report = run(config).expect("CEM search benchmark should execute before timing");

    assert_eq!(report.candidates.len(), CANDIDATES);
    assert_eq!(report.trials.len() as u64, TOTAL_PHYSICS_EVALUATIONS);
    assert!(report.winner().is_some());
    assert!(report
        .winner()
        .and_then(|winner| winner.validation.as_ref())
        .is_some_and(|summary| summary.scored > 0));
}

fn benchmark_cross_entropy_search(criterion: &mut Criterion) {
    let serial = search_config(NonZeroUsize::MIN);
    let available = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let parallel_workers = NonZeroUsize::new(available.get().min(8)).unwrap_or(NonZeroUsize::MIN);
    let parallel = search_config(parallel_workers);

    // Validate both paths outside timed loops; timings include CEM, CRN replication,
    // physics execution, finalist selection, held-out validation, and report assembly.
    validate_search(&serial);
    validate_search(&parallel);

    let mut group = criterion.benchmark_group("three_cushion_cem_search");
    group.throughput(Throughput::Elements(TOTAL_PHYSICS_EVALUATIONS));
    group.bench_function(
        BenchmarkId::new("serial", TOTAL_PHYSICS_EVALUATIONS),
        |bench| {
            bench.iter(|| black_box(run(black_box(&serial))));
        },
    );
    group.bench_function(
        BenchmarkId::new(
            format!("parallel_{}_workers", parallel_workers.get()),
            TOTAL_PHYSICS_EVALUATIONS,
        ),
        |bench| {
            bench.iter(|| black_box(run(black_box(&parallel))));
        },
    );
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(15));
    targets = benchmark_cross_entropy_search
}
criterion_main!(benches);
