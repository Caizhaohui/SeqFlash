//! Stdin harness: FASTQ parser must never panic.
use std::io::Read;

fn main() {
    let mut buf = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut buf);
    let mut pos = 0usize;
    let mut n = 0u64;
    while pos < buf.len() {
        match seqflash_formats::parse_single_record(&buf, pos, n) {
            Ok((entry, next)) => {
                if next <= pos {
                    break;
                }
                pos = next;
                n = entry.record_number.saturating_add(1);
            }
            Err(_) => break,
        }
    }
}
