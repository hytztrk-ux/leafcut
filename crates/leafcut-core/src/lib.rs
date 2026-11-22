//! # leafcut-core
//!
//! The PDF engine behind [leafcut](https://github.com/hytztrk-ux/leafcut): a small,
//! fully offline PDF toolkit. Everything runs locally, in memory, in pure
//! Rust (built on [`lopdf`]).
//!
//! ```no_run
//! use leafcut_core::{merge_files, PageRanges, Pdf, Rotation};
//!
//! # fn main() -> leafcut_core::Result<()> {
//! // Merge two files.
//! let mut merged = merge_files(&["a.pdf", "b.pdf"])?;
//! merged.save("merged.pdf")?;
//!
//! // Rotate pages 1-3 and drop the last page.
//! let mut doc = Pdf::open("scan.pdf")?;
//! let pages = PageRanges::parse("1-3")?.resolve(doc.page_count())?;
//! doc.rotate(Some(&pages), Rotation::Cw90)?;
//! let last = doc.page_count();
//! doc.delete_pages(&[last])?;
//! doc.save("scan-fixed.pdf")?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod document;
mod error;
mod info;
mod pages;
mod split;

pub use document::{merge, merge_files, write_atomic, CompressReport, Pdf, Rotation};
pub use error::{Error, Result};
pub use info::{format_pdf_date, Metadata, PageInfo, PdfInfo};
pub use pages::{PageRange, PageRanges};
pub use split::{plan_split, SplitMode, SplitPart};

/// Re-export of the `lopdf` version this crate is built on.
pub use lopdf;
