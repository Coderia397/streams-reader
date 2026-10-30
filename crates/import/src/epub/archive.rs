//! Bounded reads from an EPUB's zip container. Every entry read counts against one uncompressed budget for the whole
//! book, and entries with an implausible compression ratio are refused before they are inflated, so a zip bomb fails
//! with [`ImportError::TooLarge`] instead of exhausting memory. Declared sizes can lie, so the actual inflated length
//! is bounded too.

use std::io::{Cursor, Read};

use zip::ZipArchive;
use zip::result::ZipError;

use crate::ImportError;
use crate::limits::{MAX_ENTRY_RATIO, MAX_EPUB_UNCOMPRESSED, RATIO_FREE_BYTES};

pub(crate) struct Archive<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
    /// Uncompressed bytes still allowed.
    budget: u64,
}

impl<'a> Archive<'a> {
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Self, ImportError> {
        Self::with_budget(bytes, MAX_EPUB_UNCOMPRESSED)
    }

    pub(crate) fn with_budget(bytes: &'a [u8], budget: u64) -> Result<Self, ImportError> {
        let zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| map_zip_error(&e))?;
        Ok(Self { zip, budget })
    }

    /// Index of `name`, matched exactly, then ignoring ASCII case (some books disagree with their own manifest).
    fn index_of(&self, name: &str) -> Option<usize> {
        self.zip
            .index_for_name(name)
            .or_else(|| self.zip.file_names().position(|n| n.is_ok_and(|n| n.eq_ignore_ascii_case(name))))
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.index_of(name).is_some()
    }

    /// Names of every entry, in central directory order.
    pub(crate) fn names(&self) -> Vec<String> {
        self.zip.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect()
    }

    /// The entry's bytes, or `None` when there is no such file.
    pub(crate) fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, ImportError> {
        let Some(index) = self.index_of(name) else { return Ok(None) };
        let file = self.zip.by_index(index).map_err(|e| map_zip_error(&e))?;
        if file.is_dir() {
            return Ok(None);
        }
        let declared = file.size();
        let compressed = file.compressed_size().max(1);
        if declared > self.budget {
            return Err(ImportError::TooLarge("the book is larger than 200 MB uncompressed"));
        }
        if declared > RATIO_FREE_BYTES && declared / compressed > MAX_ENTRY_RATIO {
            return Err(ImportError::TooLarge("a file in the book is compressed suspiciously well (zip bomb)"));
        }
        // Never inflate more than the budget, nor more than the ratio allows, whatever the header claims.
        let ratio_cap = compressed.saturating_mul(MAX_ENTRY_RATIO).max(RATIO_FREE_BYTES);
        let limit = self.budget.min(ratio_cap);
        let mut out = Vec::with_capacity(usize::try_from(declared.min(limit)).unwrap_or(0));
        file.take(limit.saturating_add(1))
            .read_to_end(&mut out)
            .map_err(|e| ImportError::Malformed(format!("{name}: {e}")))?;
        let read = out.len() as u64;
        if read > limit {
            return Err(ImportError::TooLarge("a file in the book inflates beyond its limits (zip bomb)"));
        }
        self.budget -= read;
        Ok(Some(out))
    }
}

fn map_zip_error(e: &ZipError) -> ImportError {
    match e {
        ZipError::UnsupportedArchive(msg) => {
            ImportError::Unsupported(format!("this EPUB uses an unsupported zip feature ({msg})"))
        }
        other => ImportError::Malformed(format!("not a valid EPUB (zip): {other}")),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use zip::CompressionMethod;
    use zip::write::{SimpleFileOptions, ZipWriter};

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A deflated zip of `entries`; shared with the EPUB tests.
    pub(crate) fn zip_of(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut w = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            // EPUB requires `mimetype` stored first and uncompressed.
            let method = if *name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
            w.start_file(*name, SimpleFileOptions::default().compression_method(method))?;
            w.write_all(data)?;
        }
        Ok(w.finish()?.into_inner())
    }

    #[test]
    fn reads_entries_and_case_insensitive_names() -> TestResult {
        let bytes = zip_of(&[("OEBPS/Chapter1.xhtml", b"hello")])?;
        let mut a = Archive::open(&bytes)?;
        assert_eq!(a.read("OEBPS/Chapter1.xhtml")?, Some(b"hello".to_vec()));
        assert_eq!(a.read("oebps/chapter1.XHTML")?, Some(b"hello".to_vec()));
        assert_eq!(a.read("missing")?, None);
        assert!(a.contains("OEBPS/Chapter1.xhtml"));
        assert_eq!(a.names(), vec!["OEBPS/Chapter1.xhtml".to_owned()]);
        Ok(())
    }

    #[test]
    fn refuses_high_ratio_entries() -> TestResult {
        // 8 MiB of zeros deflates to a few KiB: a ratio far above the cap.
        let zeros = vec![0u8; 8 * 1024 * 1024];
        let bytes = zip_of(&[("bomb.xhtml", &zeros)])?;
        let mut a = Archive::open(&bytes)?;
        assert!(matches!(a.read("bomb.xhtml"), Err(ImportError::TooLarge(_))));
        Ok(())
    }

    #[test]
    fn enforces_the_total_budget() -> TestResult {
        let data = vec![b'a'; 1000];
        let bytes = zip_of(&[("a", &data), ("b", &data)])?;
        let mut a = Archive::with_budget(&bytes, 1500)?;
        assert!(a.read("a")?.is_some());
        assert!(matches!(a.read("b"), Err(ImportError::TooLarge(_))));
        Ok(())
    }

    #[test]
    fn garbage_is_malformed() {
        assert!(matches!(Archive::open(b"not a zip at all"), Err(ImportError::Malformed(_))));
    }
}
