//! Error type used across `leafcut-core`.

use std::path::PathBuf;

/// Convenient result alias for this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong while working with a PDF.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading or writing a file failed.
    #[error("I/O error on '{path}': {source}")]
    Io {
        /// The file that was being read or written.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The file could not be parsed as a PDF, or its structure is broken.
    #[error("invalid or unsupported PDF: {0}")]
    Pdf(#[from] lopdf::Error),

    /// The PDF is encrypted (password protected). leafcut v0.1 does not handle encryption.
    #[error("the PDF is encrypted (password protected); encrypted PDFs are not supported yet")]
    Encrypted,

    /// The document has no pages.
    #[error("the PDF has no pages")]
    NoPages,

    /// A page range expression could not be parsed.
    #[error("invalid page range '{input}': {reason}")]
    InvalidRange {
        /// The text that was being parsed.
        input: String,
        /// Human-readable explanation.
        reason: String,
    },

    /// A page number lies outside the document.
    #[error("page {page} is out of range (the document has {page_count} page(s))")]
    PageOutOfRange {
        /// The offending page number (1-based).
        page: u32,
        /// Number of pages in the document.
        page_count: u32,
    },

    /// A page selection matched no pages.
    #[error("the page selection is empty")]
    EmptySelection,

    /// The requested operation would remove every page of the document.
    #[error("cannot delete every page; at least one page must remain")]
    WouldDeleteAllPages,

    /// A reorder list does not mention every page exactly once.
    #[error("invalid page order: {0}")]
    InvalidOrder(String),

    /// Rotation angle is not one of 90, 180 or 270 degrees.
    #[error("invalid rotation {0}; use 90, 180 or 270 degrees")]
    InvalidRotation(i64),

    /// Merge was called with no input documents.
    #[error("nothing to merge: no input documents were given")]
    NothingToMerge,
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
