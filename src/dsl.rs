use std::collections::HashMap;

use crate::{
    strike_resting_ball_on_table, Angle, Ball, BallSetPhysicsSpec, BallSpec, BallState, BallType,
    CueStrikeConfig, CueTipContact, Diamond, GameState, Inches, InchesPerSecond, OnTableBallState,
    Position, Rail, RestingOnTableBallState, Scale, Shot, ShotError, TableSpec,
    BOTTOM_LEFT_DIAMOND, BOTTOM_RIGHT_DIAMOND, CENTER_LEFT_DIAMOND, CENTER_RIGHT_DIAMOND,
    CENTER_SPOT, RACK_SPOT, TOP_LEFT_DIAMOND, TOP_RIGHT_DIAMOND,
};
use winnow::ascii::{float, line_ending, till_line_ending};
use winnow::combinator::{alt, cut_err, delimited, eof, opt, peek, preceded, repeat, terminated};
use winnow::error::{ErrMode, InputError};
use winnow::prelude::*;
use winnow::stream::{LocatingSlice, Location};
use winnow::token::take_while;

#[derive(Debug, Clone, PartialEq)]
pub struct DslDoc {
    pub table: Option<TableRef>,
    pub entries: Vec<DslEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DslEntry {
    Alias(AliasDef),
    Ball(BallPlacement),
    CueStrike(CueStrikeDef),
    Shot(ShotDef),
}

#[derive(Debug, Clone)]
pub struct DslScenario {
    pub game_state: GameState,
    pub shot: Option<ScenarioShot>,
}

impl DslScenario {
    pub fn strike_shot_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
    ) -> Result<Option<OnTableBallState>, DslBuildError> {
        let Some(shot) = &self.shot else {
            return Ok(None);
        };
        let ball = self
            .game_state
            .select_ball(shot.ball.clone())
            .ok_or(DslBuildError::ShotTargetBallNotPlaced(shot.ball_ref))?;
        let resting = RestingOnTableBallState::try_from(BallState::from_position(
            &ball.position,
            &self.game_state.table_spec,
        ))
        .expect("game-state ball placements should always correspond to resting on-table states");

        strike_resting_ball_on_table(&resting, &shot.shot, &shot.cue_strike, ball_set)
            .map(Some)
            .map_err(DslBuildError::InvalidShot)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioShot {
    pub ball_ref: BallRef,
    pub ball: BallType,
    pub shot: Shot,
    pub cue_strike: CueStrikeConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasDef {
    pub name: String,
    pub position: PositionExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BallPlacement {
    At {
        ball: BallRef,
        position: PositionExpr,
    },
    Frozen {
        ball: BallRef,
        rail: RailSide,
        coord: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CueStrikeDef {
    pub name: String,
    pub methods: Vec<CueStrikeMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CueStrikeMethodExpr {
    MassRatio(f64),
    EnergyLoss(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShotDef {
    pub ball: BallRef,
    pub methods: Vec<ShotMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShotMethodExpr {
    HeadingDegrees(f64),
    SpeedIps(f64),
    Tip { side: f64, height: f64 },
    Using(String),
}

#[derive(Debug, Clone, PartialEq)]
enum BallPlacementKind {
    At(PositionExpr),
    Frozen { rail: RailSide, coord: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRef {
    BrunswickGc4_9ft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BallRef {
    Cue,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PositionExpr {
    Diamond { x: f64, y: f64 },
    Named(NamedPosition),
    Alias(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedPosition {
    Center,
    Rack,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    CenterLeft,
    CenterRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailSide {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateAxis {
    X,
    Y,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DslParseError {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for DslParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for DslParseError {}

#[derive(Debug, Clone, PartialEq)]
pub enum DslBuildError {
    UnknownAlias(String),
    CoordinateOutOfRange {
        axis: CoordinateAxis,
        value: f64,
        min: f64,
        max: f64,
    },
    FrozenCoordinateOutOfRange {
        rail: RailSide,
        value: f64,
        min: f64,
        max: f64,
    },
    DuplicateCueStrike(String),
    DuplicateCueStrikeMethod {
        name: String,
        method: String,
    },
    MissingCueStrikeMethod {
        name: String,
        method: String,
    },
    DuplicateShotMethod {
        method: String,
    },
    MissingShotMethod {
        method: String,
    },
    UnknownCueStrike(String),
    MultipleShotsNotSupported {
        count: usize,
    },
    ShotTargetMustBeCueBall(BallRef),
    ShotTargetBallNotPlaced(BallRef),
    InvalidCueStrikeConfig {
        name: String,
        error: ShotError,
    },
    InvalidShot(ShotError),
}

impl std::fmt::Display for DslBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAlias(name) => write!(f, "unknown alias '{name}'"),
            Self::CoordinateOutOfRange {
                axis,
                value,
                min,
                max,
            } => write!(
                f,
                "{axis}-coordinate {value} is out of bounds; expected {min}..={max}"
            ),
            Self::FrozenCoordinateOutOfRange {
                rail,
                value,
                min,
                max,
            } => write!(
                f,
                "frozen {rail} coordinate {value} is out of bounds; expected {min}..={max}"
            ),
            Self::DuplicateCueStrike(name) => {
                write!(f, "cue_strike '{name}' was defined more than once")
            }
            Self::DuplicateCueStrikeMethod { name, method } => write!(
                f,
                "cue_strike '{name}' specified .{method}(...) more than once"
            ),
            Self::MissingCueStrikeMethod { name, method } => {
                write!(f, "cue_strike '{name}' is missing .{method}(...)")
            }
            Self::DuplicateShotMethod { method } => {
                write!(f, "shot specified .{method}(...) more than once")
            }
            Self::MissingShotMethod { method } => {
                write!(f, "shot is missing .{method}(...)")
            }
            Self::UnknownCueStrike(name) => write!(f, "unknown cue_strike '{name}'"),
            Self::MultipleShotsNotSupported { count } => write!(
                f,
                "the current DSL supports at most one shot statement, but found {count}"
            ),
            Self::ShotTargetMustBeCueBall(ball) => write!(
                f,
                "the current DSL only supports shot(cue), but found shot({ball})"
            ),
            Self::ShotTargetBallNotPlaced(ball) => {
                write!(f, "shot target ball '{ball}' is not present in the layout")
            }
            Self::InvalidCueStrikeConfig { name, error } => {
                write!(f, "cue_strike '{name}' is invalid: {error:?}")
            }
            Self::InvalidShot(error) => write!(f, "invalid shot: {error:?}"),
        }
    }
}

impl std::error::Error for DslBuildError {}

#[derive(Debug, Clone, PartialEq)]
pub enum DslError {
    Parse(DslParseError),
    Build(DslBuildError),
}

impl std::fmt::Display for DslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::Build(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DslError {}

type Stream<'i> = LocatingSlice<&'i str>;

type ParseError<'i> = ErrMode<InputError<Stream<'i>>>;

type ParseResult<'i, T> = Result<T, ParseError<'i>>;

fn parse_dsl_inner(input: &str) -> ParseResult<'_, DslDoc> {
    let mut stream = LocatingSlice::new(input);
    dsl_doc.parse_next(&mut stream)
}

pub fn parse_dsl(input: &str) -> Result<DslDoc, DslParseError> {
    parse_dsl_inner(input).map_err(parse_error)
}

pub fn parse_dsl_to_game_state(input: &str) -> Result<GameState, DslError> {
    let doc = parse_dsl(input).map_err(DslError::Parse)?;
    build_game_state(&doc).map_err(DslError::Build)
}

pub fn parse_dsl_to_scenario(input: &str) -> Result<DslScenario, DslError> {
    let doc = parse_dsl(input).map_err(DslError::Parse)?;
    build_scenario(&doc).map_err(DslError::Build)
}

pub fn build_game_state(doc: &DslDoc) -> Result<GameState, DslBuildError> {
    build_scenario(doc).map(|scenario| scenario.game_state)
}

pub fn build_scenario(doc: &DslDoc) -> Result<DslScenario, DslBuildError> {
    let table_spec = match doc.table.unwrap_or(TableRef::BrunswickGc4_9ft) {
        TableRef::BrunswickGc4_9ft => TableSpec::brunswick_gc4_9ft(),
    };

    let mut game_state = GameState::new(table_spec);
    let mut aliases = HashMap::new();
    let mut cue_strikes = HashMap::new();
    let mut shots = Vec::new();

    for entry in &doc.entries {
        match entry {
            DslEntry::Alias(alias) => {
                let resolved = resolve_position_expr(&aliases, &alias.position)?;
                aliases.insert(alias.name.clone(), resolved);
            }
            DslEntry::Ball(placement) => match placement {
                BallPlacement::At { ball, position } => {
                    let pos = resolve_position_expr(&aliases, position)?;
                    game_state.add_ball(Ball {
                        ty: ball.to_ball_type(),
                        position: pos,
                        spec: BallSpec::default(),
                    });
                }
                BallPlacement::Frozen { ball, rail, coord } => {
                    validate_frozen_coordinate(*rail, *coord)?;
                    let rail = rail.to_rail();
                    let diamond = Diamond::from(coord.to_string().as_str());
                    game_state.freeze_to_rail(
                        rail,
                        diamond,
                        Ball {
                            ty: ball.to_ball_type(),
                            ..Default::default()
                        },
                    );
                }
            },
            DslEntry::CueStrike(def) => {
                let name = def.name.clone();
                let cue_strike = build_cue_strike(def)?;
                if cue_strikes.insert(name.clone(), cue_strike).is_some() {
                    return Err(DslBuildError::DuplicateCueStrike(name));
                }
            }
            DslEntry::Shot(def) => shots.push(def.clone()),
        }
    }

    let shot = match shots.as_slice() {
        [] => None,
        [shot] => Some(build_shot(shot, &cue_strikes, &game_state)?),
        _ => {
            return Err(DslBuildError::MultipleShotsNotSupported { count: shots.len() });
        }
    };

    Ok(DslScenario { game_state, shot })
}

fn resolve_position_expr(
    aliases: &HashMap<String, Position>,
    position: &PositionExpr,
) -> Result<Position, DslBuildError> {
    match position {
        PositionExpr::Diamond { x, y } => {
            validate_coordinate(CoordinateAxis::X, *x, 0.0, 4.0)?;
            validate_coordinate(CoordinateAxis::Y, *y, 0.0, 8.0)?;
            Ok(Position::new(
                Diamond::from(x.to_string().as_str()),
                Diamond::from(y.to_string().as_str()),
            ))
        }
        PositionExpr::Named(named) => Ok(named.to_position()),
        PositionExpr::Alias(name) => aliases
            .get(name)
            .cloned()
            .ok_or_else(|| DslBuildError::UnknownAlias(name.clone())),
    }
}

fn build_cue_strike(def: &CueStrikeDef) -> Result<CueStrikeConfig, DslBuildError> {
    let mut cue_mass_ratio = None;
    let mut collision_energy_loss = None;

    for method in &def.methods {
        match method {
            CueStrikeMethodExpr::MassRatio(value) => {
                set_once(&mut cue_mass_ratio, Scale::from_f64(*value), || {
                    DslBuildError::DuplicateCueStrikeMethod {
                        name: def.name.clone(),
                        method: "mass_ratio".to_string(),
                    }
                })?;
            }
            CueStrikeMethodExpr::EnergyLoss(value) => {
                set_once(&mut collision_energy_loss, Scale::from_f64(*value), || {
                    DslBuildError::DuplicateCueStrikeMethod {
                        name: def.name.clone(),
                        method: "energy_loss".to_string(),
                    }
                })?;
            }
        }
    }

    let cue_mass_ratio = cue_mass_ratio.ok_or_else(|| DslBuildError::MissingCueStrikeMethod {
        name: def.name.clone(),
        method: "mass_ratio".to_string(),
    })?;
    let collision_energy_loss =
        collision_energy_loss.ok_or_else(|| DslBuildError::MissingCueStrikeMethod {
            name: def.name.clone(),
            method: "energy_loss".to_string(),
        })?;

    CueStrikeConfig::new(cue_mass_ratio, collision_energy_loss).map_err(|error| {
        DslBuildError::InvalidCueStrikeConfig {
            name: def.name.clone(),
            error,
        }
    })
}

fn build_shot(
    def: &ShotDef,
    cue_strikes: &HashMap<String, CueStrikeConfig>,
    game_state: &GameState,
) -> Result<ScenarioShot, DslBuildError> {
    if def.ball != BallRef::Cue {
        return Err(DslBuildError::ShotTargetMustBeCueBall(def.ball));
    }

    let ball = def.ball.to_ball_type();
    if game_state.select_ball(ball.clone()).is_none() {
        return Err(DslBuildError::ShotTargetBallNotPlaced(def.ball));
    }

    let mut heading_degrees = None;
    let mut speed_ips = None;
    let mut tip = None;
    let mut cue_strike_name = None;

    for method in &def.methods {
        match method {
            ShotMethodExpr::HeadingDegrees(value) => {
                set_once(&mut heading_degrees, *value, || {
                    DslBuildError::DuplicateShotMethod {
                        method: "heading".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::SpeedIps(value) => {
                set_once(&mut speed_ips, *value, || {
                    DslBuildError::DuplicateShotMethod {
                        method: "speed".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::Tip { side, height } => {
                set_once(&mut tip, (*side, *height), || {
                    DslBuildError::DuplicateShotMethod {
                        method: "tip".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::Using(name) => {
                set_once(&mut cue_strike_name, name.clone(), || {
                    DslBuildError::DuplicateShotMethod {
                        method: "using".to_string(),
                    }
                })?;
            }
        }
    }

    let heading_degrees = heading_degrees.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "heading".to_string(),
    })?;
    let speed_ips = speed_ips.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "speed".to_string(),
    })?;
    let (side, height) = tip.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "tip".to_string(),
    })?;
    let cue_strike_name = cue_strike_name.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "using".to_string(),
    })?;

    let cue_strike = cue_strikes
        .get(&cue_strike_name)
        .cloned()
        .ok_or(DslBuildError::UnknownCueStrike(cue_strike_name))?;
    let angle_radians = heading_degrees.to_radians();
    let tip_contact = CueTipContact::new(Scale::from_f64(side), Scale::from_f64(height))
        .map_err(DslBuildError::InvalidShot)?;
    let shot = Shot::new(
        Angle::from_north(angle_radians.sin(), angle_radians.cos()),
        InchesPerSecond::new(Inches::from_f64(speed_ips)),
        tip_contact,
    )
    .map_err(DslBuildError::InvalidShot)?;

    Ok(ScenarioShot {
        ball_ref: def.ball,
        ball,
        shot,
        cue_strike,
    })
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    duplicate: impl FnOnce() -> DslBuildError,
) -> Result<(), DslBuildError> {
    if slot.is_some() {
        Err(duplicate())
    } else {
        *slot = Some(value);
        Ok(())
    }
}

fn validate_coordinate(
    axis: CoordinateAxis,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), DslBuildError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(DslBuildError::CoordinateOutOfRange {
            axis,
            value,
            min,
            max,
        })
    }
}

fn validate_frozen_coordinate(rail: RailSide, value: f64) -> Result<(), DslBuildError> {
    let max = match rail {
        RailSide::Left | RailSide::Right => 8.0,
        RailSide::Top | RailSide::Bottom => 4.0,
    };

    if (0.0..=max).contains(&value) {
        Ok(())
    } else {
        Err(DslBuildError::FrozenCoordinateOutOfRange {
            rail,
            value,
            min: 0.0,
            max,
        })
    }
}

fn parse_error(err: ParseError<'_>) -> DslParseError {
    let offset = match err {
        ErrMode::Backtrack(error) | ErrMode::Cut(error) => {
            let input = error.input;
            Location::current_token_start(&input)
        }
        ErrMode::Incomplete(_) => 0,
    };
    let message = "invalid DSL".to_string();
    DslParseError { message, offset }
}

fn dsl_doc<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslDoc> {
    let mut doc = DslDoc {
        table: None,
        entries: Vec::new(),
    };

    repeat(0.., statement)
        .fold(
            || (),
            |(), entry| {
                match entry {
                    DslStatement::Table(table) => doc.table = Some(table),
                    DslStatement::Alias(alias) => doc.entries.push(DslEntry::Alias(alias)),
                    DslStatement::Ball(placement) => doc.entries.push(DslEntry::Ball(placement)),
                    DslStatement::CueStrike(def) => doc.entries.push(DslEntry::CueStrike(def)),
                    DslStatement::Shot(def) => doc.entries.push(DslEntry::Shot(def)),
                    DslStatement::Empty => {}
                }
                ()
            },
        )
        .parse_next(input)?;

    let _ = terminated(hws0, eof).parse_next(input)?;

    Ok(doc)
}

#[derive(Debug, Clone, PartialEq)]
enum DslStatement {
    Table(TableRef),
    Alias(AliasDef),
    Ball(BallPlacement),
    CueStrike(CueStrikeDef),
    Shot(ShotDef),
    Empty,
}

fn statement<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = hws0.parse_next(input)?;
    let stmt = alt((
        comment_line,
        blank_line,
        preceded(peek("table"), cut_err(table_stmt)),
        preceded(peek("pos"), cut_err(alias_stmt)),
        preceded(peek("ball"), cut_err(ball_stmt)),
        preceded(peek("cue_strike"), cut_err(cue_strike_stmt)),
        preceded(peek("shot"), cut_err(shot_stmt)),
    ))
    .parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    Ok(stmt)
}

fn comment_line<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = ('#', till_line_ending).parse_next(input)?;
    let _ = opt(line_ending).parse_next(input)?;
    Ok(DslStatement::Empty)
}

fn blank_line<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = line_ending.parse_next(input)?;
    Ok(DslStatement::Empty)
}

fn table_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "table".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let table = table_ref.parse_next(input)?;
    Ok(DslStatement::Table(table))
}

fn alias_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "pos".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let name = identifier.parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    let _ = '='.parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    let position = position_expr.parse_next(input)?;
    Ok(DslStatement::Alias(AliasDef {
        name: name.to_string(),
        position,
    }))
}

fn ball_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "ball".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let ball = ball_ref.parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let placement = alt((ball_frozen, ball_at)).parse_next(input)?;
    Ok(DslStatement::Ball(match placement {
        BallPlacementKind::At(position) => BallPlacement::At { ball, position },
        BallPlacementKind::Frozen { rail, coord } => BallPlacement::Frozen { ball, rail, coord },
    }))
}

fn cue_strike_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "cue_strike".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., cue_strike_method_segment).parse_next(input)?;
    Ok(DslStatement::CueStrike(CueStrikeDef {
        name: name.to_string(),
        methods,
    }))
}

fn shot_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "shot".parse_next(input)?;
    let ball = delimited('(', delimited(hws0, ball_ref, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., shot_method_segment).parse_next(input)?;
    Ok(DslStatement::Shot(ShotDef { ball, methods }))
}

fn hws0<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ()> {
    take_while(0.., |c: char| c == ' ' || c == '\t')
        .void()
        .parse_next(input)
}

fn chain_ws0<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ()> {
    take_while(0.., |c: char| {
        c == ' ' || c == '\t' || c == '\n' || c == '\r'
    })
    .void()
    .parse_next(input)
}

fn ws1<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ()> {
    take_while(1.., |c: char| c == ' ' || c == '\t')
        .void()
        .parse_next(input)
}

fn ball_at<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallPlacementKind> {
    let _ = "at".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let position = position_expr.parse_next(input)?;
    Ok(BallPlacementKind::At(position))
}

fn ball_frozen<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallPlacementKind> {
    let _ = "frozen".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let rail = rail_side.parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let coord = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(BallPlacementKind::Frozen { rail, coord })
}

fn cue_strike_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    cue_strike_method.parse_next(input)
}

fn cue_strike_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, CueStrikeMethodExpr> {
    alt((
        preceded(peek("mass_ratio"), cut_err(cue_strike_mass_ratio_method)),
        preceded(peek("energy_loss"), cut_err(cue_strike_energy_loss_method)),
    ))
    .parse_next(input)
}

fn cue_strike_mass_ratio_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = "mass_ratio".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(CueStrikeMethodExpr::MassRatio(value))
}

fn cue_strike_energy_loss_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = "energy_loss".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(CueStrikeMethodExpr::EnergyLoss(value))
}

fn shot_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    shot_method.parse_next(input)
}

fn shot_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    alt((
        preceded(peek("heading"), cut_err(shot_heading_method)),
        preceded(peek("speed"), cut_err(shot_speed_method)),
        preceded(peek("tip"), cut_err(shot_tip_method)),
        preceded(peek("using"), cut_err(shot_using_method)),
    ))
    .parse_next(input)
}

fn shot_heading_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "heading".parse_next(input)?;
    let value = delimited('(', delimited(hws0, degrees_literal, hws0), ')').parse_next(input)?;
    Ok(ShotMethodExpr::HeadingDegrees(value))
}

fn shot_speed_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "speed".parse_next(input)?;
    let value = delimited('(', delimited(hws0, speed_literal, hws0), ')').parse_next(input)?;
    Ok(ShotMethodExpr::SpeedIps(value))
}

fn shot_tip_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "tip".parse_next(input)?;
    let (side, height) = delimited(
        '(',
        delimited(
            hws0,
            (
                preceded(("side", hws0, ':', hws0), radius_scale_literal),
                preceded(
                    (hws0, ',', hws0, "height", hws0, ':', hws0),
                    radius_scale_literal,
                ),
            ),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(ShotMethodExpr::Tip { side, height })
}

fn shot_using_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "using".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(ShotMethodExpr::Using(name.to_string()))
}

fn degrees_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    terminated(float, "deg").parse_next(input)
}

fn speed_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    terminated(float, "ips").parse_next(input)
}

fn radius_scale_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    terminated(float, 'R').parse_next(input)
}

fn rail_side<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailSide> {
    alt((
        "left".map(|_| RailSide::Left),
        "right".map(|_| RailSide::Right),
        "top".map(|_| RailSide::Top),
        "bottom".map(|_| RailSide::Bottom),
    ))
    .parse_next(input)
}

fn position_expr<'a>(input: &mut Stream<'a>) -> ParseResult<'a, PositionExpr> {
    let _ = hws0.parse_next(input)?;
    let expr = alt((
        coordinate.map(|(x, y)| PositionExpr::Diamond { x, y }),
        named_position.map(PositionExpr::Named),
        identifier.map(|name| PositionExpr::Alias(name.to_string())),
    ))
    .parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    Ok(expr)
}

fn coordinate<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (f64, f64)> {
    delimited(
        '(',
        delimited(hws0, (terminated(float, (hws0, ',', hws0)), float), hws0),
        ')',
    )
    .parse_next(input)
}

fn named_position<'a>(input: &mut Stream<'a>) -> ParseResult<'a, NamedPosition> {
    alt((
        "center".map(|_| NamedPosition::Center),
        "rack".map(|_| NamedPosition::Rack),
        "top-left".map(|_| NamedPosition::TopLeft),
        "top-right".map(|_| NamedPosition::TopRight),
        "bottom-left".map(|_| NamedPosition::BottomLeft),
        "bottom-right".map(|_| NamedPosition::BottomRight),
        "center-left".map(|_| NamedPosition::CenterLeft),
        "center-right".map(|_| NamedPosition::CenterRight),
    ))
    .parse_next(input)
}

fn table_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, TableRef> {
    alt(("brunswick_gc4_9ft".map(|_| TableRef::BrunswickGc4_9ft),)).parse_next(input)
}

fn ball_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallRef> {
    alt((
        "cue".map(|_| BallRef::Cue),
        "one".map(|_| BallRef::One),
        "two".map(|_| BallRef::Two),
        "three".map(|_| BallRef::Three),
        "four".map(|_| BallRef::Four),
        "five".map(|_| BallRef::Five),
        "six".map(|_| BallRef::Six),
        "seven".map(|_| BallRef::Seven),
        "eight".map(|_| BallRef::Eight),
        "nine".map(|_| BallRef::Nine),
    ))
    .parse_next(input)
}

fn identifier<'a>(input: &mut Stream<'a>) -> ParseResult<'a, &'a str> {
    take_while(1.., |c: char| {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    })
    .parse_next(input)
}

impl std::fmt::Display for CoordinateAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinateAxis::X => write!(f, "x"),
            CoordinateAxis::Y => write!(f, "y"),
        }
    }
}

impl std::fmt::Display for RailSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RailSide::Left => write!(f, "left"),
            RailSide::Right => write!(f, "right"),
            RailSide::Top => write!(f, "top"),
            RailSide::Bottom => write!(f, "bottom"),
        }
    }
}

impl std::fmt::Display for BallRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BallRef::Cue => write!(f, "cue"),
            BallRef::One => write!(f, "one"),
            BallRef::Two => write!(f, "two"),
            BallRef::Three => write!(f, "three"),
            BallRef::Four => write!(f, "four"),
            BallRef::Five => write!(f, "five"),
            BallRef::Six => write!(f, "six"),
            BallRef::Seven => write!(f, "seven"),
            BallRef::Eight => write!(f, "eight"),
            BallRef::Nine => write!(f, "nine"),
        }
    }
}

impl NamedPosition {
    fn to_position(self) -> Position {
        match self {
            NamedPosition::Center => CENTER_SPOT.clone(),
            NamedPosition::Rack => RACK_SPOT.clone(),
            NamedPosition::TopLeft => TOP_LEFT_DIAMOND.clone(),
            NamedPosition::TopRight => TOP_RIGHT_DIAMOND.clone(),
            NamedPosition::BottomLeft => BOTTOM_LEFT_DIAMOND.clone(),
            NamedPosition::BottomRight => BOTTOM_RIGHT_DIAMOND.clone(),
            NamedPosition::CenterLeft => CENTER_LEFT_DIAMOND.clone(),
            NamedPosition::CenterRight => CENTER_RIGHT_DIAMOND.clone(),
        }
    }
}

impl BallRef {
    fn to_ball_type(self) -> BallType {
        match self {
            BallRef::Cue => BallType::Cue,
            BallRef::One => BallType::One,
            BallRef::Two => BallType::Two,
            BallRef::Three => BallType::Three,
            BallRef::Four => BallType::Four,
            BallRef::Five => BallType::Five,
            BallRef::Six => BallType::Six,
            BallRef::Seven => BallType::Seven,
            BallRef::Eight => BallType::Eight,
            BallRef::Nine => BallType::Nine,
        }
    }
}

impl RailSide {
    fn to_rail(self) -> Rail {
        match self {
            RailSide::Left => Rail::Left,
            RailSide::Right => Rail::Right,
            RailSide::Top => Rail::Top,
            RailSide::Bottom => Rail::Bottom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MotionPhase;
    use crate::TYPICAL_BALL_RADIUS;

    #[test]
    fn parse_ball_at_coordinate() {
        let dsl = "ball cue at (2, 4)";
        let doc = parse_dsl(dsl).expect("parse");
        assert_eq!(doc.entries.len(), 1);
    }

    #[test]
    fn parse_alias_then_ball() {
        let dsl = "pos spot = (1, 2)\nball eight at spot";
        let doc = parse_dsl(dsl).expect("parse");
        let game_state = build_game_state(&doc).expect("build");
        assert_eq!(game_state.balls().len(), 1);
    }

    #[test]
    fn parse_named_position() {
        let dsl = "ball nine at center";
        let doc = parse_dsl(dsl).expect("parse");
        let game_state = build_game_state(&doc).expect("build");
        assert_eq!(game_state.balls().len(), 1);
    }

    #[test]
    fn parse_frozen_ball() {
        let dsl = "ball cue frozen left (6)";
        let doc = parse_dsl(dsl).expect("parse");
        let game_state = build_game_state(&doc).expect("build");
        assert_eq!(game_state.balls().len(), 1);
    }

    #[test]
    fn parse_shot_scenario() {
        let dsl = "ball cue at center\n\
                   cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
                   shot(cue)\n\
                     .heading(30deg)\n\
                     .speed(128ips)\n\
                     .tip(side: 0.0R, height: 0.4R)\n\
                     .using(default)";
        let scenario = parse_dsl_to_scenario(dsl).expect("build scenario");
        let seeded = scenario
            .strike_shot_on_table(&BallSetPhysicsSpec::default())
            .expect("strike shot")
            .expect("shot present");

        assert_eq!(scenario.game_state.balls().len(), 1);
        assert_eq!(scenario.shot.as_ref().expect("shot").ball_ref, BallRef::Cue);
        assert_eq!(
            seeded
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rolling
        );
    }

    #[test]
    fn rejects_unknown_alias() {
        let dsl = "ball eight at spot";
        let doc = parse_dsl(dsl).expect("parse");
        let err = build_game_state(&doc).expect_err("build");
        assert!(matches!(err, DslBuildError::UnknownAlias(_)));
    }
}
