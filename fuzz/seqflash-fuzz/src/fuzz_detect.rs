//! Stdin harness: format detection must never panic.
use std::io::Read;

fn main() {
    let mut buf = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut buf);
    let _ = seqflash_formats::detect_format(&buf);
}
