use clap::Parser;
use simul_three_cushion::{run, Cli, TrialDisposition};

fn config(arguments: &[&str]) -> simul_three_cushion::ExperimentConfig {
    Cli::try_parse_from(std::iter::once("simul-three-cushion").chain(arguments.iter().copied()))
        .expect("arguments should parse")
        .into_config()
        .expect("config should validate")
}

#[test]
fn built_in_fixture_is_a_known_scored_carom() {
    let report = run(&config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "7",
        "--candidates",
        "1",
        "--replications",
        "1",
        "--max-events",
        "64",
    ]))
    .expect("fixture should execute");
    assert_eq!(report.candidates[0].scored, 1);
    assert_eq!(report.candidates[0].success_rate, Some(1.0));
    assert!(matches!(
        report.trials[0].disposition,
        TrialDisposition::Scored
    ));
}

#[test]
fn noisy_sensitivity_is_exactly_reproducible() {
    let config = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "918273",
        "--candidates",
        "3",
        "--replications",
        "2",
        "--heading-perturb",
        "0.25",
        "--speed-perturb",
        "1.5",
        "--tip-side-perturb",
        "0.01",
        "--heading-noise",
        "0.1",
        "--speed-noise",
        "0.5",
        "--tip-height-noise",
        "0.005",
    ]);
    let first = run(&config).expect("first sensitivity run should execute");
    let second = run(&config).expect("second sensitivity run should execute");
    assert_eq!(first, second);
    assert_eq!(first.trials.len(), 6);
    assert!(first
        .trials
        .iter()
        .all(|trial| !trial.replay_key.is_empty()));
}

#[test]
fn serial_and_parallel_runs_have_identical_semantic_records() {
    let serial_config = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "918273",
        "--candidates",
        "3",
        "--replications",
        "2",
        "--heading-perturb",
        "0.25",
        "--speed-perturb",
        "1.5",
        "--tip-side-perturb",
        "0.01",
        "--heading-noise",
        "0.1",
        "--speed-noise",
        "0.5",
        "--tip-height-noise",
        "0.005",
        "--workers",
        "1",
    ]);
    let parallel_config = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "918273",
        "--candidates",
        "3",
        "--replications",
        "2",
        "--heading-perturb",
        "0.25",
        "--speed-perturb",
        "1.5",
        "--tip-side-perturb",
        "0.01",
        "--heading-noise",
        "0.1",
        "--speed-noise",
        "0.5",
        "--tip-height-noise",
        "0.005",
        "--workers",
        "4",
    ]);

    let serial = run(&serial_config).expect("serial sensitivity run should execute");
    let parallel = run(&parallel_config).expect("parallel sensitivity run should execute");
    assert_eq!(serial.candidates, parallel.candidates);
    assert_eq!(serial.trials, parallel.trials);
}

#[test]
fn seeded_search_is_ranked_and_reproducible() {
    let config = config(&[
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "1979",
        "--candidates",
        "2",
        "--replications",
        "1",
        "--heading-bounds",
        "196.391792039,196.391792039",
        "--speed-bounds",
        "237.947968822,237.947968822",
        "--tip-side-bounds=-0.230661681,-0.230661681",
        "--tip-height-bounds",
        "0.365060077,0.365060077",
        "--elevation-bounds",
        "0,0",
    ]);
    let first = run(&config).expect("first search should execute");
    let second = run(&config).expect("second search should execute");
    assert_eq!(first, second);
    assert_eq!(first.candidates[0].rank, Some(1));
    assert_eq!(first.candidates[0].candidate_id, 0);
    assert_eq!(first.candidates[0].scored, 1);
    assert_eq!(first.candidates[1].rank, Some(2));
}

#[test]
fn yellow_turn_executes_without_role_inference_in_the_integration() {
    let report = run(&config(&[
        "--fixture",
        "--shooter",
        "yellow",
        "--mode",
        "sensitivity",
        "--seed",
        "5",
        "--candidates",
        "1",
        "--replications",
        "1",
    ]))
    .expect("yellow turn should execute");
    assert_eq!(report.trials.len(), 1);
    let accounted = report.candidates[0].scored
        + report.candidates[0].missed
        + report.candidates[0].indeterminate
        + report.candidates[0].failed;
    assert_eq!(accounted, 1);
}
