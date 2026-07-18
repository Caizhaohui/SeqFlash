//! Windows resource embedding: application icon and version info.
//!
//! On Windows, this build script uses `winres` to embed the `icon.ico` and
//! `VS_VERSION_INFO` so the .exe shows the SeqFlash icon in Explorer and
//! the file properties dialog displays version information.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("ProductName", "SeqFlash");
    res.set(
        "FileDescription",
        "SeqFlash — FASTA/FASTQ browser for large sequence files",
    );
    res.set("LegalCopyright", "MIT License");
    res.set("OriginalFilename", "SeqFlash.exe");
    res.set("InternalName", "SeqFlash");

    #[allow(clippy::print_stderr)]
    if let Err(e) = res.compile() {
        eprintln!("warning: winres compile failed (non-fatal): {e}");
    }
}
