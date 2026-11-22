//! Command-line definition (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use leafcut_core::PageRanges;

const AFTER_HELP: &str = "\
Page ranges are 1-based and comma separated: \"1-3,7,10-\" means pages 1 to 3,
page 7, and page 10 to the end. \"-3\" means pages 1 to 3, \"all\" means every page.

leafcut never overwrites an input file unless you pass --in-place, and never
overwrites an existing output file unless you pass --force.

Exit codes: 0 success, 1 error, 2 invalid usage, 3 encrypted PDF (unsupported),
4 output file already exists.

Everything happens locally on your machine; no file is ever uploaded.";

/// leafcut: a small, fully offline PDF toolkit.
#[derive(Debug, Parser)]
#[command(
    name = "leafcut",
    version,
    about = "A small, fully offline PDF toolkit: merge, split, rotate, reorder, compress and more.",
    long_about = None,
    after_help = AFTER_HELP,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Only print errors.
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Merge several PDFs into one, in the order given.
    #[command(
        after_help = "Example:\n  leafcut merge cover.pdf report.pdf appendix.pdf -o full.pdf"
    )]
    Merge {
        /// Input PDF files (two or more).
        #[arg(required = true, num_args = 2.., value_name = "INPUT")]
        inputs: Vec<PathBuf>,
        /// Output file.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        /// Overwrite the output file if it already exists.
        #[arg(short, long)]
        force: bool,
    },

    /// Split a PDF into several files (one per page by default).
    #[command(after_help = "Examples:\n  \
        leafcut split book.pdf                      # book-01.pdf, book-02.pdf, ...\n  \
        leafcut split book.pdf --ranges 1-3,7,10-   # book-01-03.pdf, book-07.pdf, book-10-12.pdf\n  \
        leafcut split book.pdf --every 10 -d parts/ # 10 pages per file")]
    Split {
        /// Input PDF.
        input: PathBuf,
        /// One output file per comma-separated range.
        #[arg(short, long, value_name = "RANGES", value_parser = parse_ranges, allow_hyphen_values = true, conflicts_with = "every")]
        ranges: Option<PageRanges>,
        /// Put N consecutive pages in each output file.
        #[arg(short, long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
        every: Option<u32>,
        /// Directory for the output files [default: next to the input].
        #[arg(short = 'd', long, value_name = "DIR")]
        out_dir: Option<PathBuf>,
        /// File name prefix [default: the input file name without extension].
        #[arg(short, long)]
        prefix: Option<String>,
        /// Overwrite output files that already exist.
        #[arg(short, long)]
        force: bool,
    },

    /// Write the selected pages to a new PDF (in the order given).
    #[command(
        after_help = "Example:\n  leafcut extract contract.pdf --pages 1,4-6 -o signed-pages.pdf"
    )]
    Extract {
        /// Input PDF.
        input: PathBuf,
        /// Pages to keep, e.g. "1,4-6". Order is kept; pages may repeat.
        #[arg(short, long, value_parser = parse_ranges, allow_hyphen_values = true)]
        pages: PageRanges,
        #[command(flatten)]
        out: OutputArgs,
    },

    /// Remove pages from a PDF.
    #[command(after_help = "Example:\n  leafcut delete scan.pdf --pages 2,9- -o scan-trimmed.pdf")]
    Delete {
        /// Input PDF.
        input: PathBuf,
        /// Pages to remove, e.g. "2,9-".
        #[arg(short, long, value_parser = parse_ranges, allow_hyphen_values = true)]
        pages: PageRanges,
        #[command(flatten)]
        out: OutputArgs,
    },

    /// Rotate pages clockwise by 90, 180 or 270 degrees.
    #[command(after_help = "Examples:\n  \
        leafcut rotate scan.pdf --angle 90                 # every page\n  \
        leafcut rotate scan.pdf --angle 180 --pages 2,4-5 -o fixed.pdf")]
    Rotate {
        /// Input PDF.
        input: PathBuf,
        /// Clockwise angle.
        #[arg(short, long, value_enum)]
        angle: Angle,
        /// Pages to rotate [default: all pages].
        #[arg(short, long, value_parser = parse_ranges, allow_hyphen_values = true)]
        pages: Option<PageRanges>,
        #[command(flatten)]
        out: OutputArgs,
    },

    /// Put the pages in a new order.
    #[command(after_help = "Examples:\n  \
        leafcut reorder slides.pdf --order 3,1,2,4-   # move page 3 to the front\n  \
        leafcut reorder slides.pdf --reverse")]
    Reorder {
        /// Input PDF.
        input: PathBuf,
        /// New order; must mention every page exactly once (open ranges like "4-" help).
        #[arg(short = 'r', long, value_parser = parse_ranges, allow_hyphen_values = true, required_unless_present = "reverse")]
        order: Option<PageRanges>,
        /// Reverse the page order.
        #[arg(long, conflicts_with = "order")]
        reverse: bool,
        #[command(flatten)]
        out: OutputArgs,
    },

    /// Losslessly optimise a PDF (stream compression, unused-object removal, object streams).
    #[command(after_help = "\
Images are not downsampled or re-encoded, so scanned documents usually shrink
very little. Files written by tools that do not compress (or that leave
unused objects behind) can shrink a lot.

Example:\n  leafcut compress report.pdf -o report-small.pdf")]
    Compress {
        /// Input PDF.
        input: PathBuf,
        #[command(flatten)]
        out: OutputArgs,
    },

    /// Show page count, page sizes and metadata.
    Info {
        /// Input PDF(s).
        #[arg(required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
        /// List the size of every page.
        #[arg(long)]
        pages: bool,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Set or clear document metadata (title, author, ...).
    #[command(
        visible_alias = "metadata",
        after_help = "An empty value removes a field: --author \"\".\n\n\
Examples:\n  \
leafcut meta thesis.pdf --title \"Yüksek Lisans Tezi\" --author \"Ayşe Yılmaz\"\n  \
leafcut meta leaked.pdf --clear -o clean.pdf"
    )]
    Meta {
        /// Input PDF.
        input: PathBuf,
        #[command(flatten)]
        fields: MetaFields,
        /// Remove all metadata (information dictionary and XMP).
        #[arg(long, conflicts_with_all = ["title", "author", "subject", "keywords", "creator", "producer"])]
        clear: bool,
        #[command(flatten)]
        out: OutputArgs,
    },
}

#[derive(Debug, Args)]
pub struct MetaFields {
    /// Document title.
    #[arg(long)]
    pub title: Option<String>,
    /// Author.
    #[arg(long)]
    pub author: Option<String>,
    /// Subject.
    #[arg(long)]
    pub subject: Option<String>,
    /// Keywords.
    #[arg(long)]
    pub keywords: Option<String>,
    /// Creating application.
    #[arg(long)]
    pub creator: Option<String>,
    /// Producing application.
    #[arg(long)]
    pub producer: Option<String>,
}

#[derive(Debug, Args)]
pub struct OutputArgs {
    /// Output file [default: INPUT-<operation>.pdf next to the input].
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Overwrite the input file instead of writing a new one.
    #[arg(long, conflicts_with = "output")]
    pub in_place: bool,
    /// Overwrite the output file if it already exists.
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Angle {
    #[value(name = "90")]
    A90,
    #[value(name = "180")]
    A180,
    #[value(name = "270")]
    A270,
}

impl Angle {
    pub fn rotation(self) -> leafcut_core::Rotation {
        match self {
            Angle::A90 => leafcut_core::Rotation::Cw90,
            Angle::A180 => leafcut_core::Rotation::Cw180,
            Angle::A270 => leafcut_core::Rotation::Cw270,
        }
    }
}

fn parse_ranges(s: &str) -> Result<PageRanges, String> {
    PageRanges::parse(s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
