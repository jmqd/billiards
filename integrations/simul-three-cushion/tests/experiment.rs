use clap::Parser;
use simul_three_cushion::{
    run, Bounds, Cli, Controls, ExperimentConfig, TrialDisposition, TrialStage,
};

fn config(arguments: &[&str]) -> ExperimentConfig {
    Cli::try_parse_from(std::iter::once("simul-three-cushion").chain(arguments.iter().copied()))
        .expect("arguments should parse")
        .into_config()
        .expect("config should validate")
}

fn fixed_search_arguments() -> [&'static str; 22] {
    [
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "1979",
        "--heading-bounds",
        "196.391792039,196.391792039",
        "--speed-bounds",
        "237.947968822,237.947968822",
        "--tip-side-bounds=-0.230661681,-0.230661681",
        "--tip-height-bounds",
        "0.365060077,0.365060077",
        "--elevation-bounds",
        "0,0",
        "--screening-replications",
        "2",
        "--validation-replications",
        "2",
        "--finalists",
        "4",
        "--max-events",
        "64",
    ]
}

#[test]
fn config_validation_checks_central_controls_bounds_and_full_noise_envelope() {
    let baseline = config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--candidates",
        "2",
    ]);

    let mut too_few = baseline.clone();
    too_few.additional_candidates.push(too_few.nominal);
    too_few.candidate_budget = 1;
    assert_eq!(
        too_few.validate().unwrap_err(),
        "candidate budget must cover the nominal shot and every supplied candidate"
    );

    for (name, controls, expected) in [
        (
            "heading upper endpoint",
            Controls {
                heading: 360.0,
                ..baseline.nominal
            },
            "heading must be in [0, 360)",
        ),
        (
            "nonpositive speed",
            Controls {
                speed: 0.0,
                ..baseline.nominal
            },
            "launch speed must be greater than zero",
        ),
        (
            "strict canonical tip limit",
            Controls {
                tip_side: 0.5,
                tip_height: 0.0,
                ..baseline.nominal
            },
            "tip side/height radius must not exceed",
        ),
        (
            "cue elevation maximum",
            Controls {
                elevation: 85.0,
                ..baseline.nominal
            },
            "elevation must be in",
        ),
    ] {
        let mut invalid = baseline.clone();
        invalid.additional_candidates = vec![controls];
        assert!(invalid.validate().unwrap_err().contains(expected), "{name}");
    }

    let mut speed_envelope = baseline.clone();
    speed_envelope.nominal = Controls {
        speed: 3.0,
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 3.0,
        ..baseline.nominal
    };
    speed_envelope.shot_inaccuracy.speed = 1.0;
    assert!(speed_envelope
        .validate()
        .unwrap_err()
        .contains("speed's lower 3σ endpoint must be greater than zero"));
    speed_envelope.nominal.speed = 3.000_000_000_001;
    speed_envelope
        .validate()
        .expect("strictly positive lower speed endpoint should validate");

    let mut elevation_envelope = speed_envelope;
    elevation_envelope.nominal.elevation = 82.0;
    elevation_envelope.shot_inaccuracy.elevation = 1.0;
    assert!(elevation_envelope
        .validate()
        .unwrap_err()
        .contains("elevation's 3σ endpoints"));

    let mut tip_envelope = baseline.clone();
    tip_envelope.nominal = Controls {
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 3.0,
        ..baseline.nominal
    };
    tip_envelope.shot_inaccuracy.tip_side = 1.0 / 6.0;
    assert!(tip_envelope
        .validate()
        .unwrap_err()
        .contains("every tip side/height 3σ corner"));

    let mut arithmetic = baseline.clone();
    arithmetic.shot_inaccuracy.heading = f64::MAX;
    assert!(arithmetic
        .validate()
        .unwrap_err()
        .contains("control ± 3σ arithmetic must remain finite"));

    let mut span = baseline;
    span.search_bounds[0] = Bounds::new(-f64::MAX, f64::MAX);
    assert_eq!(
        span.validate().unwrap_err(),
        "every search-bound span must be finite"
    );
}

#[test]
fn search_requires_explicit_candidates_inside_all_five_bounds() {
    let mut search = config(&fixed_search_arguments());
    search.additional_candidates.push(Controls {
        heading: search.nominal.heading + 1.0,
        ..search.nominal
    });
    search.candidate_budget = 2;
    assert!(search
        .validate()
        .unwrap_err()
        .contains("additional candidate 0 must lie within every search bound"));

    search.additional_candidates[0] = search.nominal;
    search
        .validate()
        .expect("a repeated explicit candidate inside fixed bounds is valid");
}

#[test]
fn built_in_fixture_scores_in_screening_without_sensitivity_ranking() {
    let report = run(&config(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "7",
        "--candidates",
        "1",
        "--screening-replications",
        "1",
        "--max-events",
        "64",
    ]))
    .expect("fixture should execute");
    let candidate = &report.candidates[0];
    assert_eq!(candidate.screening.scored, 1);
    assert_eq!(candidate.screening.success_rate, Some(1.0));
    assert!(candidate.screening.eligible);
    assert!(candidate.validation.is_none());
    assert_eq!(candidate.rank, None);
    assert!(report.winner().is_none());
    assert_eq!(report.winner_id, None);
    assert_eq!(report.trials.len(), 1);
    assert_eq!(report.trials[0].stage, TrialStage::Screening);
    assert!(matches!(
        report.trials[0].disposition,
        TrialDisposition::Scored
    ));
    assert!(report.trials[0]
        .replay_key
        .to_string()
        .contains(":53454e5349540002:"));
}

#[test]
fn sensitivity_candidate_order_noise_and_schedule_are_reproducible() {
    let fixture = simul_three_cushion::known_carom_fixture();
    let good = format!(
        "{},{},{},{},{}",
        fixture.controls.heading,
        fixture.controls.speed,
        fixture.controls.tip_side,
        fixture.controls.tip_height,
        fixture.controls.elevation
    );
    let arguments = [
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "918273",
        "--candidates",
        "3",
        "--screening-replications",
        "2",
        "--good",
        good.as_str(),
        "--heading-perturb",
        "0.25",
        "--speed-perturb",
        "1.5",
        "--tip-side-perturb",
        "0.01",
        "--heading-sigma",
        "0.1",
        "--speed-sigma",
        "0.5",
        "--tip-height-sigma",
        "0.005",
    ];
    let config = config(&arguments);
    let first = run(&config).expect("first sensitivity run should execute");
    let second = run(&config).expect("second sensitivity run should execute");
    assert_eq!(first, second);
    assert_eq!(
        first
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(first.candidates[0].controls, fixture.controls);
    assert_eq!(first.candidates[1].controls, fixture.controls);
    assert_eq!(first.trials.len(), 6);
    assert!(first
        .trials
        .iter()
        .all(|trial| trial.stage == TrialStage::Screening));
    assert_eq!(
        first
            .trials
            .iter()
            .map(|trial| (trial.candidate_id, trial.replication_id))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)]
    );
    assert_eq!(first.trials[0].applied, first.trials[2].applied);
    assert_ne!(first.trials[0].applied, first.trials[1].applied);
}

#[test]
fn screening_and_validation_use_distinct_domains_with_common_random_groups() {
    let fixture = simul_three_cushion::known_carom_fixture();
    let good = format!(
        "{},{},{},{},{}",
        fixture.controls.heading,
        fixture.controls.speed,
        fixture.controls.tip_side,
        fixture.controls.tip_height,
        fixture.controls.elevation
    );
    let search = config(&[
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "918273",
        "--candidates",
        "2",
        "--good",
        good.as_str(),
        "--screening-replications",
        "2",
        "--finalists",
        "2",
        "--validation-replications",
        "2",
        "--heading-sigma",
        "0.01",
        "--speed-sigma",
        "0.05",
        "--tip-side-sigma",
        "0.0001",
        "--tip-height-sigma",
        "0.0001",
    ]);
    let report = run(&search).expect("two-stage noisy search should execute");
    assert_eq!(report.trials.len(), 10);
    let screening = &report.trials[..4];
    let validation = &report.trials[4..8];
    let nominal = &report.trials[8..];
    assert_eq!(screening[0].applied, screening[2].applied);
    assert_eq!(screening[1].applied, screening[3].applied);
    assert_eq!(validation[0].applied, validation[2].applied);
    assert_eq!(validation[1].applied, validation[3].applied);
    assert_ne!(screening[0].applied, validation[0].applied);
    assert!(screening
        .iter()
        .all(|trial| trial.replay_key.to_string().contains(":5345415243480002:")));
    assert!(validation
        .iter()
        .all(|trial| trial.replay_key.to_string().contains(":5345415243480003:")));
    assert!(nominal.iter().all(|trial| {
        trial.stage == TrialStage::Nominal
            && trial.replay_key.to_string().contains(":5345415243480004:")
    }));
}

#[test]
fn finalist_budget_is_capped_and_validation_ranking_is_deterministic() {
    let mut arguments = fixed_search_arguments().to_vec();
    arguments[19] = "1";
    arguments.extend(["--candidates", "3"]);
    let report = run(&config(&arguments)).expect("fixed search should execute");
    assert_eq!(report.trials.len(), 9);
    assert_eq!(
        report
            .trials
            .iter()
            .filter(|trial| trial.stage == TrialStage::Screening)
            .count(),
        6
    );
    assert_eq!(
        report
            .trials
            .iter()
            .filter(|trial| trial.stage == TrialStage::Validation)
            .count(),
        2
    );
    assert_eq!(
        report
            .trials
            .iter()
            .filter(|trial| trial.stage == TrialStage::Nominal)
            .count(),
        1
    );
    assert_eq!(report.candidates[0].candidate_id, 0);
    assert_eq!(report.candidates[0].rank, Some(1));
    assert!(report.candidates[0].validation.is_some());
    assert_eq!(report.candidates[0].nominal_scored, Some(true));
    assert!(report.candidates[1..].iter().all(|candidate| {
        candidate.rank.is_none()
            && candidate.validation.is_none()
            && candidate.nominal_scored.is_none()
    }));
    assert_eq!(report.winner_id, Some(0));
    assert_eq!(
        report.winner().map(|candidate| candidate.candidate_id),
        Some(0)
    );

    let mut capped_arguments = fixed_search_arguments().to_vec();
    capped_arguments.extend(["--candidates", "1"]);
    let capped =
        run(&config(&capped_arguments)).expect("finalist budget should cap to eligibility");
    assert_eq!(capped.trials.len(), 5);
    assert_eq!(capped.winner_id, Some(0));
}

#[test]
fn ineligible_screening_returns_complete_report_without_validation_or_winner() {
    let report = run(&config(&[
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "42",
        "--candidates",
        "2",
        "--screening-replications",
        "2",
        "--finalists",
        "2",
        "--validation-replications",
        "2",
        "--max-events",
        "1",
    ]))
    .expect("eligibility failure should still return a complete report");
    assert_eq!(report.trials.len(), 4);
    assert!(report
        .trials
        .iter()
        .all(|trial| trial.stage == TrialStage::Screening));
    assert!(report.candidates.iter().all(|candidate| {
        !candidate.screening.eligible
            && candidate.screening.indeterminate == 2
            && candidate.validation.is_none()
            && candidate.rank.is_none()
    }));
    assert_eq!(report.winner_id, None);
    assert!(report.winner().is_none());
}

#[test]
fn serial_and_parallel_stage_reports_are_semantically_identical() {
    let mut serial = config(&[
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "918273",
        "--candidates",
        "3",
        "--screening-replications",
        "2",
        "--finalists",
        "2",
        "--validation-replications",
        "2",
        "--heading-sigma",
        "0.05",
        "--speed-sigma",
        "0.1",
        "--workers",
        "1",
    ]);
    let mut parallel = serial.clone();
    parallel.workers = std::num::NonZeroUsize::new(4).unwrap_or(std::num::NonZeroUsize::MIN);
    let serial_report = run(&serial).expect("serial search should execute");
    let parallel_report = run(&parallel).expect("parallel search should execute");
    assert_eq!(serial_report.candidates, parallel_report.candidates);
    assert_eq!(serial_report.trials, parallel_report.trials);
    assert_eq!(serial_report.winner_id, parallel_report.winner_id);

    serial.shooter = simul_three_cushion::Shooter::Yellow;
    let yellow = run(&serial).expect("yellow turn should execute without role inference");
    assert!(!yellow.trials.is_empty());
}
