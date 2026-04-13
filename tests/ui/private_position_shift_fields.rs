use bigdecimal::BigDecimal;
use billiards::{Inches, Position};

fn main() {
    let mut position = Position::new(2u8, 4u8);
    position.unresolved_x_shift = Some(Inches {
        magnitude: BigDecimal::from(1),
    });
}
