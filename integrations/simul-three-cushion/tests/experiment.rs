use clap::Parser;
use simul_three_cushion::{run, Cli, TrialDisposition};

fn config(arguments: &[&str]) -> simul_three_cushion::ExperimentConfig {
    Cli::try_parse_from(std::iter::once("simul-three-cushion").chain(arguments.iter().copied()))
        .expect("arguments should parse")
        .into_config()
        .expect("config should validate")
}

#[test]
fn sensitivity_requires_a_center_before_trial_execution() {
    let mut sensitivity = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--candidates",
        "1",
        "--replications",
        "1",
    ]);
    sensitivity.sensitivity_centers.clear();

    assert_eq!(
        run(&sensitivity).unwrap_err(),
        "sensitivity mode requires at least one center"
    );

    sensitivity.mode = simul_three_cushion::Mode::Search;
    sensitivity
        .validate()
        .expect("search mode does not require sensitivity centers");
}

#[test]
fn invalid_additional_sensitivity_centers_are_rejected_before_trials() {
    let baseline = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--candidates",
        "2",
        "--replications",
        "1",
    ]);
    let center = baseline.nominal;
    let cases = [
        (
            "negative speed",
            simul_three_cushion::Controls {
                speed: -1.0,
                ..center
            },
            "launch speed must be greater than zero",
        ),
        (
            "NaN speed",
            simul_three_cushion::Controls {
                speed: f64::NAN,
                ..center
            },
            "speed must be finite",
        ),
        (
            "infinite speed",
            simul_three_cushion::Controls {
                speed: f64::INFINITY,
                ..center
            },
            "speed must be finite",
        ),
        (
            "tip outside the unit circle",
            simul_three_cushion::Controls {
                tip_side: 0.8,
                tip_height: 0.8,
                ..center
            },
            "tip side/height must lie within one ball radius",
        ),
        (
            "NaN tip side",
            simul_three_cushion::Controls {
                tip_side: f64::NAN,
                ..center
            },
            "tip side must be finite",
        ),
        (
            "infinite tip height",
            simul_three_cushion::Controls {
                tip_height: f64::NEG_INFINITY,
                ..center
            },
            "tip height must be finite",
        ),
        (
            "negative elevation",
            simul_three_cushion::Controls {
                elevation: -f64::EPSILON,
                ..center
            },
            "elevation must be in [0, 90) degrees",
        ),
        (
            "excluded upper elevation boundary",
            simul_three_cushion::Controls {
                elevation: 90.0,
                ..center
            },
            "elevation must be in [0, 90) degrees",
        ),
        (
            "NaN elevation",
            simul_three_cushion::Controls {
                elevation: f64::NAN,
                ..center
            },
            "elevation must be finite",
        ),
        (
            "infinite elevation",
            simul_three_cushion::Controls {
                elevation: f64::INFINITY,
                ..center
            },
            "elevation must be finite",
        ),
    ];

    for (case, invalid_center, expected) in cases {
        let mut config = baseline.clone();
        config.sensitivity_centers.push(invalid_center);
        assert_eq!(run(&config).unwrap_err(), expected, "{case}");
    }
}

#[test]
fn valid_sensitivity_center_boundaries_validate_and_execute() {
    let mut config = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--candidates",
        "1",
        "--replications",
        "1",
        "--max-events",
        "1",
    ]);
    let boundary = simul_three_cushion::Controls {
        heading: 0.0,
        speed: f64::MIN_POSITIVE,
        tip_side: 1.0,
        tip_height: 0.0,
        elevation: 0.0,
    };
    config.sensitivity_centers = vec![boundary];
    config
        .validate()
        .expect("positive speed, unit-radius tip, and zero elevation are valid");

    let elevated = simul_three_cushion::Controls {
        heading: 0.0,
        speed: 150.0,
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 45.0,
    };
    config.sensitivity_centers = vec![elevated];
    let report = run(&config).expect("a supported elevated center should execute");
    assert_eq!(report.trials.len(), 1);
    assert_eq!(report.trials[0].applied, elevated);
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
fn trial_schedule_is_candidate_major_with_stable_replay_identity() {
    let report = run(&config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "42",
        "--candidates",
        "3",
        "--replications",
        "2",
        "--max-events",
        "1",
        "--workers",
        "1",
    ]))
    .expect("sensitivity schedule should execute");

    let scheduled = report
        .trials
        .iter()
        .map(|trial| {
            (
                trial.candidate_id,
                trial.replication_id,
                trial.replay_key.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scheduled,
        [
            (0, 0, "v1:42:0:0:0"),
            (0, 1, "v1:42:0:1:1"),
            (1, 0, "v1:42:1:0:0"),
            (1, 1, "v1:42:1:1:1"),
            (2, 0, "v1:42:2:0:0"),
            (2, 1, "v1:42:2:1:1"),
        ]
    );
    assert_eq!(
        report
            .candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.requested,
                    candidate.scored,
                    candidate.missed,
                    candidate.indeterminate,
                    candidate.failed,
                    candidate.eligible,
                )
            })
            .collect::<Vec<_>>(),
        [(2, 0, 0, 2, 0, false); 3]
    );
    assert!(report
        .trials
        .iter()
        .all(|trial| matches!(trial.disposition, TrialDisposition::Indeterminate(_))));
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
