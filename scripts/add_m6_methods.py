import re
path = 'apps/seqflash-app/src/app/mod.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Fix import
content = content.replace(
    'use seqflash_ops::{count_bases, gc_percent, phred33_quality_stats, BaseCounts, QualityStats};',
    'use seqflash_ops::{\n    count_bases, export_fasta_records, export_fastq_records, gc_percent,\n    phred33_quality_stats, BaseCounts, FastaExportRecord, FastqExportRecord, QualityStats,\n    Transform,\n};'
)

# Add export/copy methods after take_pending_clipboard
old = (
    '    pub(crate) fn take_pending_clipboard(&mut self) -> Option<String> {\n'
    '        self.pending_clipboard.take()\n'
    '    }'
)

new_methods = '''    pub(crate) fn take_pending_clipboard(&mut self) -> Option<String> {
        self.pending_clipboard.take()
    }

    pub(crate) fn export_current_record(
        &self, rec: u64, path: &std::path::Path, transform: Transform,
    ) -> Result<(), String> {
        use seqflash_types::SequenceFormat;
        let id = self.active_document.ok_or("No document.")?;
        let doc = self.documents.get(id).ok_or("No doc.")?;
        let bytes = doc.bytes();
        match doc.format() {
            SequenceFormat::Fasta => {
                let idx = self.fasta_indexes.get(&id).ok_or("No index.")?;
                let e = idx.entries().get(usize::try_from(rec).unwrap_or(usize::MAX)).ok_or("Bad idx")?;
                let hs = usize::try_from(e.header_range.start).unwrap_or(0);
                let he = usize::try_from(e.header_range.end).unwrap_or(0);
                let es = usize::try_from(e.end_offset).unwrap_or(bytes.len()).min(bytes.len());
                let hdr = slice_header(&bytes[hs..he]);
                export_fasta_records(&[FastaExportRecord { header: hdr, sequence: &bytes[he..es] }], path, transform).map_err(|e| e.to_string())
            }
            SequenceFormat::Fastq => {
                let idx = self.fastq_indexes.get(&id).ok_or("No index.")?;
                let e = idx.entries().get(usize::try_from(rec).unwrap_or(usize::MAX)).ok_or("Bad idx")?;
                let hs = usize::try_from(e.header_range.start).unwrap_or(0);
                let he = usize::try_from(e.header_range.end).unwrap_or(0);
                let ss = usize::try_from(e.sequence_range.start).unwrap_or(0);
                let se = usize::try_from(e.sequence_range.end).unwrap_or(bytes.len()).min(bytes.len());
                let qs = usize::try_from(e.quality_range.start).unwrap_or(0);
                let qe = usize::try_from(e.quality_range.end).unwrap_or(bytes.len()).min(bytes.len());
                let hdr = slice_header(&bytes[hs..he]);
                export_fastq_records(&[FastqExportRecord { header: hdr, sequence: &bytes[ss..se], quality: &bytes[qs..qe] }], path, transform).map_err(|e| e.to_string())
            }
            SequenceFormat::Unknown => Err("Unsupported format".to_string()),
        }
    }

    pub(crate) fn copy_current_header(&mut self) {
        if let Some(t) = self.get_field(true, false, false) { self.pending_clipboard = Some(t); }
    }
    pub(crate) fn copy_current_sequence(&mut self) {
        if let Some(t) = self.get_field(false, true, false) { self.pending_clipboard = Some(t); }
    }
    pub(crate) fn copy_current_quality(&mut self) {
        if let Some(t) = self.get_field(false, false, true) { self.pending_clipboard = Some(t); }
    }

    fn get_field(&self, hdr: bool, seq: bool, qual: bool) -> Option<String> {
        let id = self.active_document?;
        let doc = self.documents.get(id)?;
        let bytes = doc.bytes();
        let rec = self.current_record_number?;
        match doc.format() {
            seqflash_types::SequenceFormat::Fasta => {
                let idx = self.fasta_indexes.get(&id)?;
                let e = idx.entries().get(usize::try_from(rec).ok()?)?;
                if hdr {
                    let a = usize::try_from(e.header_range.start).ok()?;
                    let b = usize::try_from(e.header_range.end).ok()?;
                    return Some(String::from_utf8_lossy(slice_header(&bytes[a..b])).into_owned());
                }
                if seq {
                    let a = usize::try_from(e.header_range.end).ok()?;
                    let b = usize::try_from(e.end_offset).ok()?.min(bytes.len());
                    return Some(String::from_utf8_lossy(&bytes[a..b]).into_owned());
                }
                None
            }
            seqflash_types::SequenceFormat::Fastq => {
                let idx = self.fastq_indexes.get(&id)?;
                let e = idx.entries().get(usize::try_from(rec).ok()?)?;
                if hdr {
                    let a = usize::try_from(e.header_range.start).ok()?;
                    let b = usize::try_from(e.header_range.end).ok()?;
                    return Some(String::from_utf8_lossy(slice_header(&bytes[a..b])).into_owned());
                }
                if seq {
                    let a = usize::try_from(e.sequence_range.start).ok()?;
                    let b = usize::try_from(e.sequence_range.end).ok()?.min(bytes.len());
                    return Some(String::from_utf8_lossy(&bytes[a..b]).into_owned());
                }
                if qual {
                    let a = usize::try_from(e.quality_range.start).ok()?;
                    let b = usize::try_from(e.quality_range.end).ok()?.min(bytes.len());
                    return Some(String::from_utf8_lossy(&bytes[a..b]).into_owned());
                }
                None
            }
            seqflash_types::SequenceFormat::Unknown => None,
        }
    }
}

fn slice_header(hdr: &[u8]) -> &[u8] {
    let s = if hdr.first() == Some(&b'>') || hdr.first() == Some(&b'@') { &hdr[1..] } else { hdr };
    let t = s.iter().rposition(|&b| b != b'\n' && b != b'\r').map_or(s.len(), |p| p + 1);
    &s[..t]
}
'''

content = content.replace(old, new_methods)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print("mod.rs updated")
