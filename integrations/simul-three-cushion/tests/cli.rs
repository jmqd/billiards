use std::num::NonZeroUsize;
use std::process::Command;

use clap::Parser;
use simul::experiment::{RandomDomain, TrialKey, SEED_PROTOCOL};
use simul_three_cushion::{
    Bounds, CandidateReport, Cli, Controls, ExperimentReport, Mode, NoiseSigmas, OutcomeSummary,
    PerturbationWidths, Position, ReplayKey, Shooter, TrialDisposition, TrialReport, TrialStage,
};

fn parse(arguments: &[&str]) -> Result<simul_three_cushion::ExperimentConfig, String> {
    Cli::try_parse_from(std::iter::once("simul-three-cushion").chain(arguments.iter().copied()))
        .map_err(|error| error.to_string())?
        .into_config()
}

#[test]
fn new_defaults_and_typed_good_candidates_parse() {
    let config = parse(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "7",
        "--good",
        "26,150,-0.1,0.2,0",
    ])
    .expect("new defaults should validate");
    assert_eq!(config.mode, Mode::Sensitivity);
    assert_eq!(config.shooter, Shooter::White);
    assert_eq!(config.candidate_budget, 16);
    assert_eq!(config.screening_replication_budget, 32);
    assert_eq!(config.finalist_budget, 4);
    assert_eq!(config.validation_replication_budget, 256);
    assert_eq!(config.workers, NonZeroUsize::MIN);
    assert_eq!(config.additional_candidates.len(), 1);
    assert_eq!(config.additional_candidates[0].speed, 150.0);
    assert_eq!(config.shot_inaccuracy.as_array(), [0.0; 5]);

    let search = parse(&[
        "--fixture",
        "--shooter",
        "yellow",
        "--mode",
        "search",
        "--seed",
        "9",
        "--candidates",
        "1",
        "--finalists",
        "4",
    ])
    .expect("finalist budget may exceed candidate budget");
    assert_eq!(search.shooter, Shooter::Yellow);
    assert_eq!(search.candidate_budget, 1);
    assert_eq!(search.finalist_budget, 4);
}

#[test]
fn gaussian_sigma_flags_map_without_aliases() {
    let config = parse(&[
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "11",
        "--elevation",
        "3",
        "--heading-sigma",
        "0.1",
        "--speed-sigma",
        "0.2",
        "--tip-side-sigma",
        "0.003",
        "--tip-height-sigma",
        "0.004",
        "--elevation-sigma",
        "0.5",
    ])
    .expect("sigma flags should validate");
    assert_eq!(
        config.shot_inaccuracy.as_array(),
        [0.1, 0.2, 0.003, 0.004, 0.5]
    );

    for removed in [
        "--replications",
        "--heading-noise",
        "--speed-noise",
        "--tip-side-noise",
        "--tip-height-noise",
        "--elevation-noise",
    ] {
        let error = Cli::try_parse_from([
            "simul-three-cushion",
            "--fixture",
            "--mode",
            "sensitivity",
            "--seed",
            "1",
            removed,
            "1",
        ])
        .expect_err("removed flag must be rejected");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::UnknownArgument,
            "{removed}"
        );
    }
}

#[test]
fn tuple_arguments_are_finite_typed_and_whitespace_tolerant() {
    let config = parse(&[
        "--mode",
        "sensitivity",
        "--seed",
        "11",
        "--white",
        " 0.7 , 1.0 ",
        "--yellow",
        "1.2,2.1",
        "--red",
        "0.85,6.55",
        "--good",
        " 26 , 150 , -0.1 , 0.2 , 0 ",
        "--heading-bounds",
        " 10 , 40 ",
    ])
    .expect("finite tuple arguments should parse");
    assert_eq!((config.positions[0].x, config.positions[0].y), (0.7, 1.0));
    assert_eq!(
        config.additional_candidates[0],
        Controls {
            heading: 26.0,
            speed: 150.0,
            tip_side: -0.1,
            tip_height: 0.2,
            elevation: 0.0,
        }
    );
    assert_eq!(config.search_bounds[0], Bounds::new(10.0, 40.0));

    for (flag, value, expected) in [
        (
            "--white",
            "not-a-number",
            "wrong number of comma-separated values",
        ),
        ("--white", "NaN,2", "values must be finite"),
        (
            "--good",
            "1,2,3,4",
            "wrong number of comma-separated values",
        ),
        ("--good", "1,2,inf,4,5", "values must be finite"),
        (
            "--heading-bounds",
            "2,1",
            "bound minimum must not exceed maximum",
        ),
    ] {
        let mut arguments = vec![
            "simul-three-cushion",
            "--fixture",
            "--mode",
            "sensitivity",
            "--seed",
            "1",
        ];
        arguments.extend([flag, value]);
        let error = Cli::try_parse_from(arguments).expect_err("invalid tuple must fail parsing");
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        assert!(error.to_string().contains(expected), "{flag}={value}");
    }
}

#[test]
fn budgets_widths_and_worker_limits_fail_before_running() {
    for (flag, expected) in [
        ("--candidates", "candidate budget must be greater than zero"),
        (
            "--screening-replications",
            "screening replication budget must be greater than zero",
        ),
        ("--finalists", "finalist budget must be greater than zero"),
        (
            "--validation-replications",
            "validation replication budget must be greater than zero",
        ),
        ("--max-events", "max events must be greater than zero"),
    ] {
        let error = parse(&["--fixture", "--mode", "search", "--seed", "1", flag, "0"])
            .expect_err("zero budget must fail config validation");
        assert_eq!(error, expected, "{flag}");
    }

    let worker_error = Cli::try_parse_from([
        "simul-three-cushion",
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--workers",
        "0",
    ])
    .expect_err("zero workers must not parse");
    assert_eq!(worker_error.kind(), clap::error::ErrorKind::ValueValidation);

    let overflow = parse(&[
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "1",
        "--candidates",
        "18446744073709551615",
    ])
    .expect_err("proposal sample IDs must be checked before allocation");
    assert_eq!(overflow, "candidate proposal sample ID would overflow u64");

    for width in ["-1", "NaN", "inf"] {
        let width_argument = format!("--heading-perturb={width}");
        let error = parse(&[
            "--fixture",
            "--mode",
            "search",
            "--seed",
            "1",
            width_argument.as_str(),
        ])
        .expect_err("invalid perturbation width must fail before bound derivation");
        assert_eq!(error, "perturbation widths must be finite and non-negative");
    }
}

#[test]
fn tagged_report_is_stage_aware_self_describing_and_csv_safe() {
    let controls = Controls {
        heading: 25.0,
        speed: 150.0,
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 0.0,
    };
    let screening = OutcomeSummary {
        requested: 2,
        scored: 0,
        scored_object_first: 0,
        missed: 0,
        indeterminate: 1,
        failed: 1,
        success_rate: None,
        confidence_low: None,
        confidence_high: None,
        eligible: false,
    };
    let domain = RandomDomain::new(0x5345_4e53_4954_0002);
    let trial_key = |replication_id| TrialKey {
        random_domain: domain,
        candidate_id: 3,
        replication_id,
        common_random_group: u64::from(replication_id),
    };
    let report = ExperimentReport {
        mode: Mode::Sensitivity,
        shooter: Shooter::Yellow,
        positions: [
            Position { x: 0.7, y: 1.0 },
            Position { x: 1.2, y: 2.1 },
            Position { x: 0.85, y: 6.55 },
        ],
        perturbations: PerturbationWidths {
            heading: 1.0,
            speed: 2.0,
            tip_side: 0.01,
            tip_height: 0.02,
            elevation: 3.0,
        },
        shot_inaccuracy: NoiseSigmas {
            heading: 0.1,
            speed: 0.2,
            tip_side: 0.003,
            tip_height: 0.004,
            elevation: 0.5,
        },
        search_bounds: [
            Bounds::new(0.0, 360.0),
            Bounds::new(1.0, 300.0),
            Bounds::new(-0.3, 0.3),
            Bounds::new(-0.05, 0.4),
            Bounds::new(0.0, 45.0),
        ],
        candidate_budget: 1,
        screening_replication_budget: 2,
        finalist_budget: 4,
        validation_replication_budget: 256,
        requested_workers: NonZeroUsize::MIN,
        max_events: 64,
        master_seed: 42,
        seed_protocol: SEED_PROTOCOL,
        physics_profile: "three-cushion-default",
        noise_model: "independent-truncated-normal",
        noise_parent_sigma_limit: 3.0,
        selection_policy: "none",
        winner_id: None,
        candidates: vec![CandidateReport {
            rank: None,
            candidate_id: 3,
            controls,
            nominal_scored: None,
            screening,
            validation: None,
        }],
        trials: vec![
            TrialReport {
                stage: TrialStage::Screening,
                candidate_id: 3,
                replication_id: 0,
                replay_key: ReplayKey::new(42, trial_key(0)),
                applied: None,
                disposition: TrialDisposition::Failed("sampling, \"exhausted\"".into()),
            },
            TrialReport {
                stage: TrialStage::Screening,
                candidate_id: 3,
                replication_id: 1,
                replay_key: ReplayKey::new(42, trial_key(1)),
                applied: Some(controls),
                disposition: TrialDisposition::Indeterminate(
                    "event, \"limit\"\r\ncontinued".into(),
                ),
            },
        ],
    };

    let mut first = Vec::new();
    let mut second = Vec::new();
    report.write_to(&mut first).expect("report should write");
    report
        .write_to(&mut second)
        .expect("report should write twice");
    assert_eq!(first, second);
    let text = String::from_utf8(first).expect("report is UTF-8");
    assert!(text.contains(concat!(
        "META,mode=sensitivity,shooter=yellow,physics_profile=three-cushion-default,",
        "master_seed=42,seed_protocol=simul-v1-splitmix64-box-muller,"
    )));
    assert!(text.contains("selection_policy=none,winner_id=,candidate_count=1,trial_count=2\n"));
    assert!(text.contains("CONFIG,white_diamonds=0.700000000:1.000000000"));
    assert!(text.contains("CANDIDATE,,3,25.000000000,150.000000000"));
    assert!(text.contains("false,,,,,,,,,,\n"));
    assert!(text.contains(concat!(
        "TRIAL,screening,3,0,simul-v1:42:53454e5349540002:3:0:0,",
        ",,,,,failed,\"sampling, \"\"exhausted\"\"\"\n"
    )));
    assert!(text.contains(concat!(
        "TRIAL,screening,3,1,simul-v1:42:53454e5349540002:3:1:1,",
        "25.000000000,150.000000000,0.000000000,0.000000000,0.000000000,",
        "indeterminate,\"event, \"\"limit\"\"\r\ncontinued\"\n"
    )));
    assert!(report.winner().is_none());
}

#[test]
fn fixed_search_cli_is_stage_aware_and_reproducible() {
    let arguments = [
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "1979",
        "--candidates",
        "2",
        "--screening-replications",
        "2",
        "--finalists",
        "4",
        "--validation-replications",
        "2",
        "--workers",
        "2",
        "--heading-bounds",
        "196.391792039,196.391792039",
        "--speed-bounds",
        "237.947968822,237.947968822",
        "--tip-side-bounds=-0.230661681,-0.230661681",
        "--tip-height-bounds",
        "0.365060077,0.365060077",
        "--elevation-bounds",
        "0,0",
        "--heading-sigma",
        "0",
        "--speed-sigma",
        "0",
        "--tip-side-sigma",
        "0",
        "--tip-height-sigma",
        "0",
        "--elevation-sigma",
        "0",
    ];
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_simul-three-cushion"))
            .args(arguments)
            .output()
            .expect("binary should spawn")
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);

    let text = String::from_utf8(first.stdout).expect("stdout should be UTF-8");
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("META,"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("CONFIG,"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("CANDIDATE,rank,"))
            .count(),
        1
    );
    let candidates: Vec<_> = lines
        .iter()
        .filter(|line| line.starts_with("CANDIDATE,") && !line.starts_with("CANDIDATE,rank,"))
        .collect();
    assert_eq!(candidates.len(), 2);
    for (rank, id, row) in [("1", "0", candidates[0]), ("2", "1", candidates[1])] {
        let fields: Vec<_> = row.split(',').collect();
        assert_eq!((fields[1], fields[2]), (rank, id));
        assert_eq!(
            &fields[3..8],
            [
                "196.391792039",
                "237.947968822",
                "-0.230661681",
                "0.365060077",
                "0.000000000",
            ]
        );
        assert_eq!(fields[8], "true");
        assert_eq!(&fields[9..12], ["2", "2", "0"]);
        assert_eq!(&fields[12..15], ["0", "0", "0"]);
        assert_eq!(fields[18], "true");
        assert_eq!(&fields[19..22], ["2", "2", "0"]);
        assert_eq!(&fields[22..25], ["0", "0", "0"]);
        assert_eq!(fields[28], "true");
    }

    let meta = lines
        .iter()
        .find(|line| line.starts_with("META,"))
        .expect("META row");
    assert!(meta.contains(concat!(
        "selection_policy=nominal-score-required-then-wilson95-lower-then-rate-",
        "then-object-first-then-id"
    )));

    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("TRIAL,stage,"))
            .count(),
        1
    );
    let trials: Vec<_> = lines
        .iter()
        .filter(|line| line.starts_with("TRIAL,") && !line.starts_with("TRIAL,stage,"))
        .collect();
    assert_eq!(trials.len(), 10);
    assert_eq!(
        trials
            .iter()
            .filter(|row| row.starts_with("TRIAL,screening,"))
            .count(),
        4
    );
    assert_eq!(
        trials
            .iter()
            .filter(|row| row.starts_with("TRIAL,validation,"))
            .count(),
        4
    );
    assert_eq!(
        trials
            .iter()
            .filter(|row| row.starts_with("TRIAL,nominal,"))
            .count(),
        2
    );
    for row in trials {
        let fields: Vec<_> = row.split(',').collect();
        assert_eq!(
            &fields[5..10],
            [
                "196.391792039",
                "237.947968822",
                "-0.230661681",
                "0.365060077",
                "0.000000000",
            ]
        );
        assert_eq!(fields[10], "scored");
        match fields[1] {
            "screening" => assert!(fields[4].contains(":5345415243480002:")),
            "validation" => assert!(fields[4].contains(":5345415243480003:")),
            "nominal" => assert!(fields[4].contains(":5345415243480004:")),
            stage => panic!("unexpected stage {stage}"),
        }
    }
}
