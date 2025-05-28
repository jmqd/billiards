mod assets;

use crate::assets::diamond_to_frac;
use image::imageops::{FilterType, resize};
use lazy_static::lazy_static;
use std::fs::File;
use std::io::Write;
use std::ops::{Add, Div, Mul};
use std::path::Path;
use std::str::FromStr;

use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;

lazy_static! {
    pub static ref DIAMOND_SIGHT_NOSE_OFFSET: Inches = Inches {
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
    pub static ref CENTER_SPOT: Position = Position {
        x: Diamond::from("2"),
        y: Diamond::from("4")
    };
    pub static ref TOP_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("8")
    };
    pub static ref SIDE_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("4")
    };
    pub static ref BOTTOM_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("0")
    };
    pub static ref BOTTOM_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("0")
    };
    pub static ref SIDE_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("4")
    };
    pub static ref TOP_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("8")
    };
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// Represents the unit of distance on a pool table of "a diamond".
/// Going left-to-right, a diamond is 25% of the pool tables width.
/// Going top-down, a diamond is 1/8 (12.5%) of the tables length.
pub struct Diamond {
    pub magnitude: BigDecimal,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// Our representation for converting to inches.
pub struct Inches {
    pub magnitude: BigDecimal,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// A point on the table, interepreted as follows:
///     - Top-down view of the table, rack spot at the top and the headstring at the bottom.
///     - The diamond that would exist at the bottom-left pocket is x=0, y=0.
///     - The diamond that would exist at the top-right pocket is x=4, y=8.
///     - The headstring is the imaginary line from (0, 2) <-> (4, 2).
///     - The rack spot is the point (2, 6).
///     - The center of the table is the point (2, 4).
///     - The kitchen is the rectangle from (0, 0) <-> (4, 2).
pub struct Position {
    pub x: Diamond,
    pub y: Diamond,
}

impl Add for Inches {
    type Output = Inches;

    fn add(self, rhs: Inches) -> Self::Output {
        Inches {
            magnitude: self.magnitude + rhs.magnitude,
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
    pub fn new_9ft_brunswick_gc4() -> Self {
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
/// The type of game, e.g. Nineball, EightBall, OnePocket, etc.
pub enum GameType {
    NineBall,
    EightBall,
    TenBall,
    OnePocket,
    Banks,
}

#[derive(Clone, Debug)]
/// A modifier being applied to the Cueball, for example ball in hand.
pub enum CueballModifier {
    AsItLays,
    BreakPlacement,
    BallInHand,
    KitchenPlacement,
}

#[derive(Clone, Debug)]
/// The full and compelete data structure to describe the state of a game.
pub struct GameState {
    pub table_spec: TableSpec,
    pub ball_positions: Vec<Ball>,
    pub ty: GameType,
    pub cueball_modifier: CueballModifier,
}

impl GameState {
    /// Draws a 2D diagram of the current GameState, placing the balls in the
    /// appropriate positions on the diagram.
    pub fn draw_2d_diagram(&self) -> Vec<u8> {
        use image::codecs::png::PngEncoder;
        use image::imageops::overlay;
        use image::{ImageEncoder, ImageFormat, RgbaImage};

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
            const BALL_SIZE: u32 = 44;
            ball_img = resize(&ball_img, BALL_SIZE, BALL_SIZE, FilterType::CatmullRom);
            let (bw, bh) = ball_img.dimensions();

            // Compute where the ball's *centre* should go
            let (fx, fy) = diamond_to_frac(&ball.position);
            let x_epsilon = (fx * 11.0) as i32;
            let px = (fx * (tw - 48 * 2) as f32) as i32 + 48 - x_epsilon;
            let py = (fy * (th - 48 * 2) as f32) as i32 + 48;

            // Compute where to begin drawing the ball.
            // We have to account for the width and height of the ball.
            // Overlaying a png starts drawing at the top-left corner of the
            // ball, so we need to start drawing at px - bw/2, py - bh/2
            let mut px_shifted = px - (BALL_SIZE as i32 / 2);
            let mut py_shifted = py - (BALL_SIZE as i32 / 2);

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
