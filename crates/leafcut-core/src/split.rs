//! Splitting one document into several.

use crate::document::Pdf;
use crate::error::Result;
use crate::pages::PageRanges;

/// How to split a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitMode {
    /// One output document per page.
    EveryPage,
    /// One output document per comma-separated part, e.g. `"1-3,7,10-"`
    /// gives three documents.
    Ranges(PageRanges),
    /// Consecutive chunks of at most N pages.
    Chunks(u32),
}

/// One planned output of a split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPart {
    /// Short label suitable for a file name, e.g. `"3"` or `"1-3"`.
    pub label: String,
    /// Pages (1-based) that go into this part.
    pub pages: Vec<u32>,
}

/// Work out which pages go into which output, without touching any document.
pub fn plan_split(mode: &SplitMode, page_count: u32) -> Result<Vec<SplitPart>> {
    let width = page_count.to_string().len();
    let label_for = |pages: &[u32]| -> String {
        match pages {
            [one] => format!("{one:0width$}"),
            [first, .., last] => format!("{first:0width$}-{last:0width$}"),
            [] => String::new(),
        }
    };
    let parts = match mode {
        SplitMode::EveryPage => (1..=page_count)
            .map(|p| SplitPart {
                label: label_for(&[p]),
                pages: vec![p],
            })
            .collect(),
        SplitMode::Ranges(ranges) => ranges
            .parts()
            .iter()
            .map(|r| {
                let pages = r.resolve(page_count)?;
                Ok(SplitPart {
                    label: label_for(&pages),
                    pages,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        SplitMode::Chunks(n) => {
            let n = (*n).max(1);
            (1..=page_count)
                .step_by(n as usize)
                .map(|start| {
                    let end = (start + n - 1).min(page_count);
                    let pages: Vec<u32> = (start..=end).collect();
                    SplitPart {
                        label: label_for(&pages),
                        pages,
                    }
                })
                .collect()
        }
    };
    Ok(parts)
}

impl Pdf {
    /// Split into several documents. Each item is the plan entry together
    /// with the resulting document.
    ///
    /// For large documents prefer [`plan_split`] plus [`Pdf::extract`] in a
    /// loop, saving each part as you go, to keep memory use low.
    pub fn split(&self, mode: &SplitMode) -> Result<Vec<(SplitPart, Pdf)>> {
        plan_split(mode, self.page_count())?
            .into_iter()
            .map(|part| {
                let doc = self.extract(&part.pages)?;
                Ok((part, doc))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_every_page_pads_labels() {
        let parts = plan_split(&SplitMode::EveryPage, 12).unwrap();
        assert_eq!(parts.len(), 12);
        assert_eq!(parts[0].label, "01");
        assert_eq!(parts[11].label, "12");
        assert_eq!(parts[4].pages, vec![5]);
    }

    #[test]
    fn plan_ranges() {
        let r = PageRanges::parse("1-3,7,10-").unwrap();
        let parts = plan_split(&SplitMode::Ranges(r), 12).unwrap();
        let labels: Vec<_> = parts.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, ["01-03", "07", "10-12"]);
        assert_eq!(parts[2].pages, vec![10, 11, 12]);
    }

    #[test]
    fn plan_chunks() {
        let parts = plan_split(&SplitMode::Chunks(4), 10).unwrap();
        let pages: Vec<_> = parts.iter().map(|p| p.pages.clone()).collect();
        assert_eq!(pages, vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8], vec![9, 10]]);
    }

    #[test]
    fn plan_ranges_out_of_bounds() {
        let r = PageRanges::parse("1-20").unwrap();
        assert!(plan_split(&SplitMode::Ranges(r), 5).is_err());
    }
}
