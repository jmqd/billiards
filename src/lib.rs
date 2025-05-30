mod assets;

use crate::assets::diamond_to_pixel;
use assets::ideal_ball_size_px;
use image::imageops::{FilterType, resize};
use lazy_static::lazy_static;
use std::fs::File;
use std::io::Write;
use std::ops::{Add, Div, Mul, Sub, Neg};
use std::path::Path;
use std::str::FromStr;

use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;
use bigdecimal::ToPrimitive;

lazy_static! {
    pub static ref DIAMOND_SIGHT_NOSE_OFFSET: Inches = Inches {
        magnitude: BigDecimal::from_str("3.6875").unwrap()
    };
    pub static ref OFFICIAL_DIAMOND_SIGHT_NOSE_OFFSET: Inches = Inches {
        magnitude: BigDecimal::from_str("3.6875").unwrap()
    };
    pub static ref GC4_POCKET_DEPTH: Inches = Inches {
        magnitude: BigDecimal::from_str("1.4").unwrap()
    };
    pub static ref GC4_CORNER_POCKET_WIDTH: Inches = Inches {
        magnitude: BigDecimal::from_str("4.5").unwrap()
    };
    pub static ref GC4_SIDE_POCKET_WIDTH: Inches = Inches {
        magnitude: BigDecimal::from_str("5").unwrap()
    };
    pub static ref TYPICAL_BALL_RADIUS: Inches = Inches {
        magnitude: BigDecimal::from_str("1.125").unwrap()
    };
    pub static ref CENTER_SPOT: Position = Position {
        x: Diamond::from("2"),
        y: Diamond::from("4"),
        ..Default::default()
    };
    pub static ref TOP_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("8"),
        ..Default::default()
    };
    pub static ref SIDE_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("4"),
        ..Default::default()
    };
    pub static ref RACK_SPOT: Position = Position {
        x: Diamond::from("2"),
        y: Diamond::from("2"),
        ..Default::default()
    };
    pub static ref BOTTOM_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("0"),
        ..Default::default()
    };
    pub static ref BOTTOM_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("0"),
        ..Default::default()
    };
    pub static ref SIDE_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("4"),
        ..Default::default()
    };
    pub static ref TOP_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("8"),
        ..Default::default()
    };
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// Represents the unit of distance on a pool table of "a diamond".
/// Going left-to-right, a diamond is 25% of the pool tables width.
/// Going top-down, a diamond is 1/8 (12.5%) of the tables length.
pub struct Diamond {
    pub magnitude: BigDecimal,
}

impl Diamond {
    pub fn zero() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(0).unwrap(),
        }
    }

    pub fn one() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(1).unwrap(),
        }
    }

    pub fn two() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(2).unwrap(),
        }
    }

    pub fn three() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(3).unwrap(),
        }
    }

    pub fn four() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(4).unwrap(),
        }
    }

    pub fn five() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(5).unwrap(),
        }
    }

    pub fn six() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(6).unwrap(),
        }
    }

    pub fn seven() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(7).unwrap(),
        }
    }

    pub fn eight() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(8).unwrap(),
        }
    }

    pub fn double(self) -> Self {
        Self {
            magnitude: self.magnitude.double(),
        }
    }

    pub fn half(self) -> Self {
        Self {
            magnitude: self.magnitude.half(),
        }
    }

    pub fn inverse(self) -> Self {
        Self {
            magnitude: self.magnitude.inverse(),
        }
    }
}

impl Default for Diamond {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// Our representation for converting to inches.
pub struct Inches {
    pub magnitude: BigDecimal,
}

impl Default for Inches {
    fn default() -> Self {
        Self {
            magnitude: Default::default(),
        }
    }
}

impl Inches {
    pub fn double(self) -> Self {
        Self {
            magnitude: self.magnitude.double(),
        }
    }

    pub fn half(self) -> Self {
        Self {
            magnitude: self.magnitude.half(),
        }
    }

}

impl Neg for Inches {
    type Output = Inches;

    fn neg(self) -> Self {
        Self {
            magnitude: self.magnitude.neg()
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// A point on the table, interepreted as follows:
///   - Top-down view of the table, headstring at the top and rack spot at the bottom.
///   - The diamond that would exist at the bottom-left pocket is x=0, y=0.
///   - The diamond that would exist at the top-right pocket is x=4, y=8.
///   - The headstring is the imaginary line from (0, 6) <-> (4, 6).
///   - The rack spot is the point (2, 2).
///   - The center of the table is the point (2, 4).
///   - The kitchen is the rectangle from (0, 8) <-> (4, 6).
#[derive(Default)]
pub struct Position {
    pub x: Diamond,
    pub y: Diamond,
    pub unresolved_x_shift: Option<Inches>,
    pub unresolved_y_shift: Option<Inches>,
}

impl Position {
    /// Calculates the displacement from this position to another.
    pub fn displacement(&self, to: &Self) -> Displacement {
        Displacement {
            dx: to.x.clone() - self.x.clone(),
            dy: to.y.clone() - self.y.clone(),
        }
    }

    pub fn merge_unset_component(mut self, diamond: Diamond) -> Self {
        if self.x == Diamond::zero() {
            self.x = diamond;
            self
        } else if self.y == Diamond::zero() {
            self.y = diamond;
            self
        } else {
            unreachable!();
        }
    }

    pub fn zeroed() -> Self {
        Self {
            x: Diamond::zero(),
            y: Diamond::zero(),
            ..Default::default()
        }
    }

    pub fn shift_horizontally(&mut self, distance: Diamond) -> &mut Self {
        self.x = self.x.clone() + distance.clone();
        self
    }

    pub fn shift_vertically(&mut self, distance: Diamond) -> &mut Self {
        self.y = self.y.clone() + distance.clone();
        self
    }

    pub fn shift_horizontally_inches(&mut self, distance: Inches) -> &mut Self {
        self.unresolved_x_shift =
            Some(self.unresolved_x_shift.clone().unwrap_or_default() + distance);
        self
    }

    pub fn shift_vertically_inches(&mut self, distance: Inches) -> &mut Self {
        self.unresolved_y_shift =
            Some(self.unresolved_y_shift.clone().unwrap_or_default() + distance);
        self
    }
}

/// A displacement indicating a direction and distance.
#[derive(Clone, Debug)]
pub struct Displacement {
    /// The delta x component of the displacement.
    pub dx: Diamond,

    /// The delta y component of the displacement.
    pub dy: Diamond,
}

impl Displacement {
    pub fn absolute_distance(&self) -> Diamond {
        let dx = self.dx.magnitude.to_f64().unwrap();
        let dy = self.dy.magnitude.to_f64().unwrap();

        // dx² + dy² = dist²
        let dist = (dx * dx + dy * dy).sqrt();

        Diamond {
            magnitude: bigdecimal::BigDecimal::from_f64(dist).unwrap(),
        }
    }
}

impl Sub for Diamond {
    type Output = Diamond;

    fn sub(self, rhs: Diamond) -> Self::Output {
        Self {
            magnitude: self.magnitude - rhs.magnitude,
        }
    }
}

impl Neg for Diamond {
    type Output = Diamond;

    fn neg(self) -> Self {
        Self {
            magnitude: self.magnitude.neg()
        }
    }
}

impl Add for Diamond {
    type Output = Diamond;

    fn add(self, rhs: Diamond) -> Self::Output {
        Self {
            magnitude: self.magnitude + rhs.magnitude,
        }
    }
}

impl Add for Inches {
    type Output = Inches;

    fn add(self, rhs: Inches) -> Self::Output {
        Self {
            magnitude: self.magnitude + rhs.magnitude,
        }
    }
}

impl Sub for Inches {
    type Output = Inches;

    fn sub(self, rhs: Inches) -> Self::Output {
        Self {
            magnitude: self.magnitude - rhs.magnitude,
        }
    }
}

impl Div<BigDecimal> for Inches {
    type Output = Inches;

    fn div(self, rhs: BigDecimal) -> Self::Output {
        Inches {
            magnitude: self.magnitude / rhs,
        }
    }
}

impl Mul<BigDecimal> for Diamond {
    type Output = Diamond;

    fn mul(self, rhs: BigDecimal) -> Self::Output {
        Diamond {
            magnitude: self.magnitude * rhs,
        }
    }
}

impl From<u8> for Diamond {
    fn from(value: u8) -> Self {
        Self {
            magnitude: BigDecimal::from_u8(value).unwrap(),
        }
    }
}

impl From<&str> for Diamond {
    fn from(value: &str) -> Self {
        Self {
            magnitude: BigDecimal::from_str(value).unwrap(),
        }
    }
}

#[derive(Clone, Debug)]
/// A type of pocket.
pub enum PocketType {
    /// One of the four corner pockets.
    Corner,

    /// One of the two side pockets.
    Side,
}

#[derive(Clone, Debug)]
/// Physical specifications of a pocket.
pub struct PocketSpec {
    pub ty: PocketType,
    pub depth: Diamond,
    pub width: Diamond,
}

#[derive(Clone, Debug)]
/// Physical specifications of a pool table.
pub struct TableSpec {
    pub pockets: [PocketSpec; 6],
    pub cushion_diamond_buffer: Diamond,
    pub diamond_length: Inches,
}

impl Default for TableSpec {
    fn default() -> Self {
        Self::brunswick_gc4_9ft()
    }
}

#[derive(Clone, Debug)]
/// Physical specifications of a pool ball.
pub struct BallSpec {
    pub radius: Inches,
}

impl Default for BallSpec {
    fn default() -> Self {
        Self {
            radius: Inches {
                magnitude: BigDecimal::from_str("1.125").unwrap(),
            },
        }
    }
}

impl TableSpec {
    /// A typical 9ft Brunswick Gold Crown IV specification.
    pub fn brunswick_gc4_9ft() -> Self {
        let diamond_length = Inches {
            magnitude: BigDecimal::from_str("12.5").unwrap(),
        };
        Self {
            diamond_length: diamond_length.clone(),
            cushion_diamond_buffer: Diamond {
                magnitude: DIAMOND_SIGHT_NOSE_OFFSET.magnitude.clone()
                    / diamond_length.magnitude.clone(),
            },
            pockets: [
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
                Self::brunswick_gc4_side_pocket(diamond_length.clone()),
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
                Self::brunswick_gc4_side_pocket(diamond_length.clone()),
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
            ],
        }
    }

    /// A typical Brunswick GC IV corner pocket specification.
    pub fn brunswick_gc4_corner_pocket(diamond_length: Inches) -> PocketSpec {
        PocketSpec {
            ty: PocketType::Corner,
            depth: Diamond {
                magnitude: GC4_POCKET_DEPTH.magnitude.clone() / diamond_length.magnitude.clone(),
            },
            width: Diamond {
                magnitude: GC4_CORNER_POCKET_WIDTH.magnitude.clone()
                    / diamond_length.magnitude.clone(),
            },
        }
    }

    /// A typical Brunswick GC IV side pocket specification.
    pub fn brunswick_gc4_side_pocket(diamond_length: Inches) -> PocketSpec {
        PocketSpec {
            ty: PocketType::Side,
            depth: Diamond {
                magnitude: GC4_POCKET_DEPTH.magnitude.clone() / diamond_length.magnitude.clone(),
            },
            width: Diamond {
                magnitude: GC4_SIDE_POCKET_WIDTH.magnitude.clone()
                    / diamond_length.magnitude.clone(),
            },
        }
    }

    /// For a given table, convert Diamond Units into Inches.
    /// On a typical 9ft table, 1 Diamond is equal to 12.5 inches.
    pub fn diamond_to_inches(&self, val: Diamond) -> Inches {
        Inches {
            magnitude: val.magnitude * self.diamond_length.magnitude.clone(),
        }
    }

    /// For a given table, convert inches into Diamond Units.
    pub fn inches_to_diamond(&self, val: Inches) -> Diamond {
        Diamond {
            magnitude: val.magnitude / self.diamond_length.magnitude.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// A type of ball, for example, Cue ball, the eight ball, etc.
pub enum BallType {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Cue,
}

#[derive(Clone, Debug)]
/// Represents a ball on the table, incl. its position, physical spec, type.
pub struct Ball {
    pub ty: BallType,
    pub position: Position,
    pub spec: BallSpec,
}

impl Ball {
    /// Calculates the displacement between two balls. (Distance w/ direction.)
    pub fn displacement(&self, to: &Self) -> Displacement {
        self.position.displacement(&to.position)
    }

    /// Calculates the absolute distance between two balls.
    pub fn distance(&self, to: &Self) -> Diamond {
        self.displacement(to).absolute_distance()
    }
}

#[derive(Clone, Debug)]
/// The type of game, e.g. Nineball, EightBall, OnePocket, etc.
pub enum GameType {
    NineBall,
    EightBall,
    TenBall,
    OnePocket,
    Banks,
}

impl Default for GameType {
    fn default() -> Self {
        Self::NineBall
    }
}

#[derive(Clone, Debug)]
/// A modifier being applied to the Cueball, for example ball in hand.
pub enum CueballModifier {
    AsItLays,
    BreakPlacement,
    BallInHand,
    KitchenPlacement,
}

impl Default for CueballModifier {
    fn default() -> Self {
        Self::AsItLays
    }
}

/// The rails on a pool table.
#[derive(Debug, Clone)]
pub enum Rail {
    Top,
    Bottom,
    Left,
    Right,
}

impl Rail {
    pub fn rail_origin(&self) -> Position {
        match *self {
            Rail::Top => Position {
                x: Diamond::zero(),
                y: Diamond::eight(),
                ..Default::default()
            },
            Rail::Right => Position {
                x: Diamond::four(),
                y: Diamond::zero(),
                ..Default::default()
            },
            Rail::Bottom => Position {
                x: Diamond::zero(),
                y: Diamond::zero(),
                ..Default::default()
            },
            Rail::Left => Position {
                x: Diamond::zero(),
                y: Diamond::zero(),
                ..Default::default()
            },
        }
    }

    pub fn is_vertical(&self) -> bool {
        matches!(*self, Rail::Left | Rail::Right)
    }

    pub fn is_horizontal(&self) -> bool {
        matches!(*self, Rail::Top | Rail::Bottom)
    }
}

#[derive(Clone, Debug)]
/// The full and compelete data structure to describe the state of a game.
#[derive(Default)]
pub struct GameState {
    pub table_spec: TableSpec,
    pub ball_positions: Vec<Ball>,
    pub ty: GameType,
    pub cueball_modifier: CueballModifier,
}

impl GameState {
    // TODO: We're assuming for now all BallTypes are unique. This may change.
    pub fn select_ball(&self, ball_type: BallType) -> Option<&Ball> {
        self.ball_positions.iter().find(|b| b.ty == ball_type)
    }

    /// This is mildly hacky, but works for now to resolve all the unresolved
    /// inches adjustments.
    pub fn resolve_positions(&mut self) {
        for ball in self.ball_positions.iter_mut() {
            if let Some(shift) = &ball.position.unresolved_x_shift {
                ball.position.shift_horizontally(self.table_spec.inches_to_diamond(shift.clone()));
                ball.position.unresolved_x_shift = None;
            }

            if let Some(shift) = &ball.position.unresolved_y_shift {
                ball.position.shift_vertically(self.table_spec.inches_to_diamond(shift.clone()));
                ball.position.unresolved_y_shift = None;
            }
        }
    }

    pub fn freeze_to_rail(&mut self, rail: Rail, diamond: Diamond, mut ball: Ball) {
        match rail {
            Rail::Top => {
                ball.position.y =
                    Diamond::eight() - self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.x = diamond;
            }
            Rail::Right => {
                ball.position.x =
                    Diamond::four() - self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.y = diamond;
            }
            Rail::Bottom => {
                ball.position.y =
                    Diamond::zero() + self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.x = diamond;
            }
            Rail::Left => {
                ball.position.x =
                    Diamond::zero() + self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.y = diamond;
            }
        };

        self.ball_positions.push(ball);
    }

    /// Draws a 2D diagram of the current GameState, placing the balls in the
    /// appropriate positions on the diagram.
    pub fn draw_2d_diagram(&self) -> Vec<u8> {
        use image::codecs::png::PngEncoder;
        use image::imageops::overlay;
        use image::{ImageEncoder, ImageFormat, RgbaImage};

        let ball_radius_px = ideal_ball_size_px();

        let mut table: RgbaImage =
            image::load_from_memory_with_format(assets::TABLE_DIAGRAM, ImageFormat::Png)
                .expect("broken table asset")
                .into_rgba8();

        let (tw, th) = table.dimensions();

        for ball in &self.ball_positions {
            let ball_png = assets::ball_img(ball.ty.clone());
            let mut ball_img: RgbaImage =
                image::load_from_memory_with_format(&ball_png, ImageFormat::Png)
                    .expect("bad ball image")
                    .into_rgba8();
            ball_img = resize(
                &ball_img,
                ball_radius_px,
                ball_radius_px,
                FilterType::CatmullRom,
            );
            let (bw, bh) = ball_img.dimensions();

            // Compute where the ball's *centre* should go
            let (px, py) = diamond_to_pixel(&ball.position);

            // Compute where to begin drawing the ball.
            // We have to account for the width and height of the ball.
            // Overlaying a png starts drawing at the top-left corner of the
            // ball, so we need to start drawing at px - bw/2, py - bh/2
            let mut px_shifted = px - (bw as i32 / 2);
            let mut py_shifted = py - (bh as i32 / 2);

            // Prevent any out of bounds weirdness (shouldn't happen).
            px_shifted = px_shifted.clamp(0, (tw - bw / 2) as i32);
            py_shifted = py_shifted.clamp(0, (th - bh / 2) as i32);

            overlay(&mut table, &ball_img, px_shifted.into(), py_shifted.into());
        }

        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(&table, tw, th, image::ColorType::Rgba8.into())
            .expect("PNG encode failed");
        buf
    }
}

// TODO: Return result, swap unwraps to ?.
pub fn write_png_to_file(png_bytes: &[u8], path: Option<&Path>) {
    let out_path = path.unwrap_or_else(|| Path::new("output.png"));
    let mut file = File::create(out_path).unwrap();
    file.write_all(png_bytes).unwrap();
    file.flush().unwrap();
}

pub fn rack_9_ball() -> Vec<Ball> {
    let ball_types = vec![
        BallType::One,
        BallType::Two,
        BallType::Three,
        BallType::Four,
        BallType::Nine,
        BallType::Five,
        BallType::Six,
        BallType::Seven,
        BallType::Eight,
    ];

    racked_ball_positions()
        .into_iter()
        .enumerate()
        .map(|(idx, pos)| Ball {
            ty: ball_types[idx].clone(),
            position: pos,
            spec: Default::default(),
        })
        .collect()
}

pub fn racked_ball_positions() -> Vec<Position> {
    let head_ball_position = RACK_SPOT.clone();
    let mut second_row_left = head_ball_position.clone();

    second_row_left
        .shift_vertically_inches(TYPICAL_BALL_RADIUS.clone().double().neg())
        .shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().neg());

    let mut second_row_right = second_row_left.clone();
    second_row_right.shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().double());

    vec![head_ball_position, second_row_left, second_row_right]
}
