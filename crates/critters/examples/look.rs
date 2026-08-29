//! Print one crossing to stdout, for a human to look at.
//! `cargo run -p robco-critters --example look -- whale 30`
fn main() {
    let mut args = std::env::args().skip(1);
    let want = args.next().unwrap_or_else(|| "whale".into());
    let at: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    let (cols, rows) = (80usize, 24usize);
    let art = critters::ART
        .iter()
        .find(|a| a.name == want)
        .expect("no such piece");
    let facing_left = art.right.is_empty();
    let crossing = critters::Crossing {
        art,
        facing_left,
        top: 8,
        step: at,
    };
    let mut cells = Vec::new();
    crossing.paint(cols, rows, &mut cells);
    let mut screen = vec![vec!['%'; cols]; rows];
    for (r, c, ch) in cells {
        screen[r][c] = ch;
    }
    println!(
        "{want} step {at} facing {}",
        if facing_left { "left" } else { "right" }
    );
    for row in screen.iter().skip(6).take(12) {
        println!("{}", row.iter().collect::<String>());
    }
}
