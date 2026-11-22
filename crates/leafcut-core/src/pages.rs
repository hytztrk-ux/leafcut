//! Page-range expressions such as `"1-3,7,10-"`.
//!
//! Grammar (whitespace around tokens is ignored, page numbers are 1-based):
//!
//! ```text
//! ranges := part ("," part)*
//! part   := N        a single page
//!         | N-M      pages N through M (inclusive, N <= M)
//!         | N-       page N through the last page
//!         | -M       the first page through page M
//!         | "all"    every page (also "*")
//! ```
//!
//! Parsing does not need to know the page count; open-ended parts are resolved
//! later with [`PageRanges::resolve`].

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// One part of a page-range expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    /// First page (1-based, inclusive).
    pub start: u32,
    /// Last page (1-based, inclusive). `None` means "until the last page".
    pub end: Option<u32>,
}

impl PageRange {
    /// A range covering a single page.
    pub fn single(page: u32) -> Self {
        PageRange {
            start: page,
            end: Some(page),
        }
    }

    /// Expand this range into concrete page numbers for a document with
    /// `page_count` pages.
    pub fn resolve(&self, page_count: u32) -> Result<Vec<u32>> {
        let end = self.end.unwrap_or(page_count);
        if self.start > page_count {
            return Err(Error::PageOutOfRange {
                page: self.start,
                page_count,
            });
        }
        if end > page_count {
            return Err(Error::PageOutOfRange {
                page: end,
                page_count,
            });
        }
        Ok((self.start..=end).collect())
    }
}

impl fmt::Display for PageRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.end {
            Some(end) if end == self.start => write!(f, "{}", self.start),
            Some(end) => write!(f, "{}-{}", self.start, end),
            None => write!(f, "{}-", self.start),
        }
    }
}

/// A parsed, comma-separated list of [`PageRange`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRanges {
    parts: Vec<PageRange>,
}

impl PageRanges {
    /// Parse an expression such as `"1-3,7,10-"`.
    pub fn parse(input: &str) -> Result<Self> {
        let err = |reason: &str| Error::InvalidRange {
            input: input.to_string(),
            reason: reason.to_string(),
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(err("the expression is empty"));
        }

        let mut parts = Vec::new();
        for raw in trimmed.split(',') {
            let part = raw.trim();
            if part.is_empty() {
                return Err(err("empty item between commas"));
            }
            if part.eq_ignore_ascii_case("all") || part == "*" {
                parts.push(PageRange {
                    start: 1,
                    end: None,
                });
                continue;
            }
            let range = match part.split_once('-') {
                None => PageRange::single(parse_page(part).map_err(|r| err(&r))?),
                Some((a, b)) => {
                    let (a, b) = (a.trim(), b.trim());
                    if a.is_empty() && b.is_empty() {
                        return Err(err("'-' needs a page number on at least one side"));
                    }
                    let start = if a.is_empty() {
                        1
                    } else {
                        parse_page(a).map_err(|r| err(&r))?
                    };
                    let end = if b.is_empty() {
                        None
                    } else {
                        Some(parse_page(b).map_err(|r| err(&r))?)
                    };
                    if let Some(end) = end {
                        if end < start {
                            return Err(err(&format!(
                                "range {start}-{end} is backwards; write the smaller page first"
                            )));
                        }
                    }
                    PageRange { start, end }
                }
            };
            parts.push(range);
        }
        Ok(PageRanges { parts })
    }

    /// A selection covering every page.
    pub fn all() -> Self {
        PageRanges {
            parts: vec![PageRange {
                start: 1,
                end: None,
            }],
        }
    }

    /// The individual parts, in the order they were written.
    pub fn parts(&self) -> &[PageRange] {
        &self.parts
    }

    /// Expand into page numbers, in the written order. Duplicates are kept
    /// (`"1,1"` yields `[1, 1]`).
    pub fn resolve(&self, page_count: u32) -> Result<Vec<u32>> {
        let mut pages = Vec::new();
        for part in &self.parts {
            pages.extend(part.resolve(page_count)?);
        }
        if pages.is_empty() {
            return Err(Error::EmptySelection);
        }
        Ok(pages)
    }

    /// Expand into a sorted list of distinct page numbers.
    pub fn resolve_set(&self, page_count: u32) -> Result<Vec<u32>> {
        let mut pages = self.resolve(page_count)?;
        pages.sort_unstable();
        pages.dedup();
        Ok(pages)
    }
}

impl FromStr for PageRanges {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        PageRanges::parse(s)
    }
}

impl fmt::Display for PageRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

fn parse_page(s: &str) -> std::result::Result<u32, String> {
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("'{s}' is not a page number"));
    }
    match s.parse::<u32>() {
        Ok(0) => Err("page numbers start at 1".to_string()),
        Ok(n) => Ok(n),
        Err(_) => Err(format!("'{s}' is too large")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> PageRanges {
        PageRanges::parse(s).unwrap_or_else(|e| panic!("{s:?} should parse: {e}"))
    }

    #[test]
    fn single_pages() {
        assert_eq!(r("3").resolve(5).unwrap(), vec![3]);
        assert_eq!(r("1,3,5").resolve(5).unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn closed_and_open_ranges() {
        assert_eq!(
            r("1-3,7,10-").resolve(12).unwrap(),
            vec![1, 2, 3, 7, 10, 11, 12]
        );
        assert_eq!(r("-3").resolve(10).unwrap(), vec![1, 2, 3]);
        assert_eq!(r("8-").resolve(10).unwrap(), vec![8, 9, 10]);
        assert_eq!(r("4-4").resolve(10).unwrap(), vec![4]);
    }

    #[test]
    fn whitespace_is_ignored() {
        assert_eq!(r(" 1 - 2 , 4 ").resolve(5).unwrap(), vec![1, 2, 4]);
    }

    #[test]
    fn all_keyword() {
        assert_eq!(r("all").resolve(3).unwrap(), vec![1, 2, 3]);
        assert_eq!(r("ALL").resolve(2).unwrap(), vec![1, 2]);
        assert_eq!(r("*").resolve(2).unwrap(), vec![1, 2]);
        assert_eq!(PageRanges::all().resolve(4).unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn order_and_duplicates_are_kept_by_resolve() {
        assert_eq!(r("3,1,1").resolve(3).unwrap(), vec![3, 1, 1]);
        assert_eq!(r("3,1,1,2-3").resolve_set(3).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn display_round_trips() {
        for s in ["1-3,7,10-", "5", "1-", "2-9"] {
            assert_eq!(r(s).to_string(), s);
        }
        assert_eq!(r("-3").to_string(), "1-3");
        assert_eq!(r("all").to_string(), "1-");
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            " ",
            ",",
            "1,,2",
            "0",
            "0-3",
            "a",
            "1-b",
            "-",
            "3-1",
            "1.5",
            "+2",
            "1-2-3",
            "99999999999",
        ] {
            assert!(
                matches!(PageRanges::parse(bad), Err(Error::InvalidRange { .. })),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn out_of_range_is_reported_on_resolve() {
        let err = r("2-6").resolve(5).unwrap_err();
        assert!(matches!(
            err,
            Error::PageOutOfRange {
                page: 6,
                page_count: 5
            }
        ));
        let err = r("9-").resolve(5).unwrap_err();
        assert!(matches!(err, Error::PageOutOfRange { page: 9, .. }));
    }

    #[test]
    fn from_str_works() {
        let p: PageRanges = "1-2".parse().unwrap();
        assert_eq!(
            p.parts(),
            &[PageRange {
                start: 1,
                end: Some(2)
            }]
        );
    }
}
