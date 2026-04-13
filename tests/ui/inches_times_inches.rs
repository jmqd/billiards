use bigdecimal::BigDecimal;
use billiards::Inches;

fn main() {
    let inches = Inches {
        magnitude: BigDecimal::from(1),
    };

    let _ = inches.clone() * inches;
}
