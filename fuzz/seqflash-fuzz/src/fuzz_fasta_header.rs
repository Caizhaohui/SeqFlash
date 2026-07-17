//! Stdin harness: FASTA header parse must never panic.
use std::io::Read;

fn main() {
    let mut buf = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut buf);
    // Parse whole buffer and each line-like slice.
    let _ = seqflash_formats::parse_fasta_header(&buf);
    for line in buf.split(|&b| b == b'\n' || b == b'\r') {
        let _ = seqflash_formats::parse_fasta_header(line);
    }
}
