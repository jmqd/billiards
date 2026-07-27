use billiards::Inches;

fn main() {
    let inches = Inches::from(1i64);

    let _ = inches.clone() * inches;
}
