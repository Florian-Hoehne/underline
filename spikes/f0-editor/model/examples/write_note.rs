//! Writes the test note to the path given (default `/tmp/f0-note.md`).
fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/f0-note.md".into());
    std::fs::write(&path, f0_model::note::generate()).expect("the note is written");
    println!("{path}");
}
