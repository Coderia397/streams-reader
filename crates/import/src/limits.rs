//! Size limits for untrusted input. They are far above any real book or article and exist so a hostile or broken file
//! fails fast with [`crate::ImportError::TooLarge`] instead of exhausting the browser tab's memory.

/// Largest text, Markdown or HTML input, in bytes.
pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;

/// Largest EPUB file, in bytes (compressed).
pub const MAX_EPUB_BYTES: usize = 200 * 1024 * 1024;

/// Total uncompressed bytes read from one EPUB archive.
pub const MAX_EPUB_UNCOMPRESSED: u64 = 200 * 1024 * 1024;

/// Largest compression ratio accepted for one archive entry once it is bigger than [`RATIO_FREE_BYTES`]. Real XHTML
/// compresses about 3–6x; a deflate bomb reaches 1000x.
pub const MAX_ENTRY_RATIO: u64 = 100;

/// Entries up to this size are not ratio-checked (a tiny file full of spaces can legitimately compress very well).
pub const RATIO_FREE_BYTES: u64 = 1024 * 1024;

/// Largest number of HTML elements handed to readability.
pub const MAX_HTML_ELEMENTS: usize = 500_000;

/// Largest number of PDF pages.
pub const MAX_PDF_PAGES: usize = 20_000;

/// Deepest element nesting kept as structure; anything deeper is flattened to its text. Bounds recursion depth
/// (the wasm32 stack is small) without losing words.
pub const MAX_TREE_DEPTH: usize = 128;
