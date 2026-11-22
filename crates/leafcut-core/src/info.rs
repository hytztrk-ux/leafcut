//! Document information and metadata types.

/// Document information dictionary fields.
///
/// When used as an update (see [`crate::Pdf::set_metadata`]), `None` means
/// "leave unchanged" and `Some("")` means "remove".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    /// `/Title`
    pub title: Option<String>,
    /// `/Author`
    pub author: Option<String>,
    /// `/Subject`
    pub subject: Option<String>,
    /// `/Keywords`
    pub keywords: Option<String>,
    /// `/Creator` (the application that created the original document)
    pub creator: Option<String>,
    /// `/Producer` (the application that produced the PDF)
    pub producer: Option<String>,
    /// `/CreationDate`, raw PDF date string such as `D:20240131120000+03'00'`
    pub creation_date: Option<String>,
    /// `/ModDate`, raw PDF date string
    pub mod_date: Option<String>,
}

impl Metadata {
    /// `(PDF key, value)` pairs for every field.
    pub fn entries(&self) -> [(&'static str, Option<&str>); 8] {
        [
            ("Title", self.title.as_deref()),
            ("Author", self.author.as_deref()),
            ("Subject", self.subject.as_deref()),
            ("Keywords", self.keywords.as_deref()),
            ("Creator", self.creator.as_deref()),
            ("Producer", self.producer.as_deref()),
            ("CreationDate", self.creation_date.as_deref()),
            ("ModDate", self.mod_date.as_deref()),
        ]
    }

    /// True when no field is set.
    pub fn is_empty(&self) -> bool {
        self.entries().iter().all(|(_, v)| v.is_none())
    }
}

/// Size and rotation of one page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageInfo {
    /// 1-based page number.
    pub number: u32,
    /// MediaBox width in points (1/72 inch), before rotation.
    pub width_pt: f64,
    /// MediaBox height in points, before rotation.
    pub height_pt: f64,
    /// Rotation in degrees (0, 90, 180 or 270).
    pub rotation: i64,
}

impl PageInfo {
    /// Width in millimetres, before rotation.
    pub fn width_mm(&self) -> f64 {
        pt_to_mm(self.width_pt)
    }

    /// Height in millimetres, before rotation.
    pub fn height_mm(&self) -> f64 {
        pt_to_mm(self.height_pt)
    }

    /// Name of a common paper size that matches (within 2 mm), if any.
    pub fn paper_name(&self) -> Option<&'static str> {
        const SIZES: [(&str, f64, f64); 7] = [
            ("A3", 297.0, 420.0),
            ("A4", 210.0, 297.0),
            ("A5", 148.0, 210.0),
            ("B5", 176.0, 250.0),
            ("Letter", 215.9, 279.4),
            ("Legal", 215.9, 355.6),
            ("Tabloid", 279.4, 431.8),
        ];
        let (w, h) = (self.width_mm(), self.height_mm());
        let (short, long) = if w <= h { (w, h) } else { (h, w) };
        SIZES
            .iter()
            .find(|(_, sw, sh)| (short - sw).abs() < 2.0 && (long - sh).abs() < 2.0)
            .map(|(name, _, _)| *name)
    }
}

fn pt_to_mm(pt: f64) -> f64 {
    pt * 25.4 / 72.0
}

/// Summary of a document, as returned by [`crate::Pdf::info`].
#[derive(Debug, Clone, PartialEq)]
pub struct PdfInfo {
    /// PDF version from the header, e.g. `"1.7"`.
    pub version: String,
    /// Number of pages.
    pub page_count: u32,
    /// Number of indirect objects.
    pub object_count: usize,
    /// Whether the document has bookmarks (an outline).
    pub has_outlines: bool,
    /// Whether the document has interactive form fields.
    pub has_forms: bool,
    /// Document information dictionary.
    pub metadata: Metadata,
    /// Per-page sizes.
    pub pages: Vec<PageInfo>,
}

/// Turn a PDF date (`D:YYYYMMDDHHmmSSOHH'mm'`) into `YYYY-MM-DD HH:MM:SS +HH:MM`.
/// Returns the input unchanged if it does not look like a PDF date.
pub fn format_pdf_date(raw: &str) -> String {
    let s = raw.trim();
    let s = s.strip_prefix("D:").unwrap_or(s);
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return raw.to_string();
    }
    let field = |from: usize, len: usize, default: &str| -> String {
        digits
            .get(from..from + len)
            .map(str::to_string)
            .unwrap_or_else(|| default.to_string())
    };
    let mut out = format!(
        "{}-{}-{} {}:{}:{}",
        field(0, 4, "0000"),
        field(4, 2, "01"),
        field(6, 2, "01"),
        field(8, 2, "00"),
        field(10, 2, "00"),
        field(12, 2, "00"),
    );
    let rest = &s[digits.len()..];
    match rest.chars().next() {
        Some('Z') => out.push_str(" UTC"),
        Some(sign @ ('+' | '-')) => {
            let tz: String = rest[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            let hh = tz.get(0..2).unwrap_or("00");
            let mm = tz.get(2..4).unwrap_or("00");
            out.push_str(&format!(" {sign}{hh}:{mm}"));
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(
            format_pdf_date("D:20240131123456+03'00'"),
            "2024-01-31 12:34:56 +03:00"
        );
        assert_eq!(format_pdf_date("D:20240131"), "2024-01-31 00:00:00");
        assert_eq!(
            format_pdf_date("D:20240131123456Z"),
            "2024-01-31 12:34:56 UTC"
        );
        assert_eq!(format_pdf_date("yesterday"), "yesterday");
    }

    #[test]
    fn paper_names() {
        let a4 = PageInfo {
            number: 1,
            width_pt: 595.0,
            height_pt: 842.0,
            rotation: 0,
        };
        assert_eq!(a4.paper_name(), Some("A4"));
        let letter_landscape = PageInfo {
            number: 1,
            width_pt: 792.0,
            height_pt: 612.0,
            rotation: 0,
        };
        assert_eq!(letter_landscape.paper_name(), Some("Letter"));
        let odd = PageInfo {
            number: 1,
            width_pt: 100.0,
            height_pt: 100.0,
            rotation: 0,
        };
        assert_eq!(odd.paper_name(), None);
    }
}
