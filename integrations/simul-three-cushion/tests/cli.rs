use clap::Parser;
use simul_three_cushion::{
    CandidateReport, Cli, Controls, ExperimentReport, Mode, Shooter, TrialDisposition, TrialReport,
};

#[test]
fn fixture_config_supports_both_modes_and_shooters() {
    let sensitivity = Cli::try_parse_from([
        "simul-three-cushion",
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "7",
        "--candidates",
        "3",
        "--replications",
        "2",
    ])
    .expect("fixture sensitivity arguments should parse")
    .into_config()
    .expect("fixture sensitivity config should validate");
    assert_eq!(sensitivity.mode, Mode::Sensitivity);
    assert_eq!(sensitivity.shooter, Shooter::White);
    assert_eq!(sensitivity.candidate_budget, 3);
    assert_eq!(sensitivity.replication_budget, 2);
    assert_eq!(sensitivity.workers.get(), 1);

    let search = Cli::try_parse_from([
        "simul-three-cushion",
        "--fixture",
        "--shooter",
        "yellow",
        "--mode",
        "search",
        "--seed",
        "9",
        "--heading-bounds",
        "10,40",
        "--speed-bounds",
        "100,180",
    ])
    .expect("fixture search arguments should parse")
    .into_config()
    .expect("fixture search config should validate");
    assert_eq!(search.mode, Mode::Search);
    assert_eq!(search.shooter, Shooter::Yellow);
    assert_eq!(search.search_bounds[0].minimum, 10.0);
    assert_eq!(search.search_bounds[1].maximum, 180.0);
}

#[test]
fn explicit_layout_and_additional_good_candidate_are_typed() {
    let config = Cli::try_parse_from([
        "simul-three-cushion",
        "--mode",
        "sensitivity",
        "--seed",
        "11",
        "--white",
        "0.7,1.0",
        "--yellow",
        "1.2,2.1",
        "--red",
        "0.85,6.55",
        "--good",
        "26,150,-0.1,0.2,0",
    ])
    .expect("explicit layout should parse")
    .into_config()
    .expect("explicit layout should validate");
    assert_eq!(config.positions[2].y, 6.55);
    assert_eq!(config.sensitivity_centers.len(), 2);
    assert_eq!(config.sensitivity_centers[1].speed, 150.0);
}

#[test]
fn rejects_zero_workers_during_argument_parsing() {
    let error = Cli::try_parse_from([
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
    assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    assert!(error.to_string().contains("--workers"));
}

#[test]
fn rejects_zero_budgets_and_invalid_tip_offsets() {
    let zero_budget = Cli::try_parse_from([
        "simul-three-cushion",
        "--fixture",
        "--mode",
        "search",
        "--seed",
        "1",
        "--candidates",
        "0",
    ])
    .expect("syntax should parse")
    .into_config();
    assert_eq!(
        zero_budget.unwrap_err(),
        "candidate budget must be greater than zero"
    );

    let invalid_tip = Cli::try_parse_from([
        "simul-three-cushion",
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--tip-side",
        "0.9",
        "--tip-height",
        "0.9",
    ])
    .expect("syntax should parse")
    .into_config();
    assert_eq!(
        invalid_tip.unwrap_err(),
        "tip side/height must lie within one ball radius"
    );
}

#[test]
fn rejects_negative_speed_in_additional_good_center() {
    let config = Cli::try_parse_from([
        "simul-three-cushion",
        "--fixture",
        "--mode",
        "sensitivity",
        "--seed",
        "1",
        "--candidates",
        "2",
        "--good",
        "0,-1,0,0,0",
    ])
    .expect("finite additional center syntax should parse")
    .into_config();

    assert_eq!(
        config.unwrap_err(),
        "launch speed must be greater than zero"
    );
}

#[test]
fn report_is_stable_and_distinguishes_non_misses() {
    let controls = Controls {
        heading: 25.0,
        speed: 150.0,
        tip_side: 0.0,
        tip_height: 0.0,
        elevation: 0.0,
    };
    let report = ExperimentReport {
        mode: Mode::Sensitivity,
        shooter: Shooter::Yellow,
        master_seed: 42,
        seed_protocol: "v1",
        candidates: vec![CandidateReport {
            rank: None,
            candidate_id: 3,
            controls,
            requested: 2,
            scored: 0,
            missed: 0,
            indeterminate: 1,
            failed: 1,
            success_rate: None,
            confidence_low: None,
            confidence_high: None,
            eligible: false,
        }],
        trials: vec![
            TrialReport {
                candidate_id: 3,
                replication_id: 0,
                replay_key: "v1:3:0".into(),
                applied: controls,
                disposition: TrialDisposition::Indeterminate("event limit".into()),
            },
            TrialReport {
                candidate_id: 3,
                replication_id: 1,
                replay_key: "v1:3:1".into(),
                applied: controls,
                disposition: TrialDisposition::Failed("invalid shot".into()),
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
    assert!(text.contains("META,mode=sensitivity,shooter=yellow,"));
    assert!(text.contains("outcome,detail"));
    assert!(text.contains(",indeterminate,event limit"));
    assert!(text.contains(",failed,invalid shot"));
}
