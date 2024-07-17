mod common;

fn main() {
    let file = std::env::args().nth(1).unwrap_or_else(|| "il-test.html".into());
    common::Rules::new().parse(&file);
}
