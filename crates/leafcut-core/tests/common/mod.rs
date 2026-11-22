//! Test helpers: build small PDFs in memory with lopdf, so no binary
//! fixtures need to be checked in.

#![allow(dead_code)]

use leafcut_core::Pdf;
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};

/// Build a document with `pages` pages. Page `i` draws the text
/// `"{tag}-P{i}"`, so tests can tell pages apart.
///
/// The page tree is deliberately nested (pages are grouped in pairs under
/// intermediate `/Pages` nodes) and `MediaBox`, `Resources` and `Rotate` are
/// inherited from ancestors, to exercise the less common code paths.
/// Content streams are stored uncompressed and one unused object is
/// included, so compression has something to do.
pub fn build_doc(tag: &str, pages: u32) -> Document {
    let mut doc = Document::with_version("1.5");
    let root_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut groups: Vec<Object> = Vec::new();
    let mut page_no = 1;
    while page_no <= pages {
        let group_id = doc.new_object_id();
        let mut kids = Vec::new();
        for _ in 0..2 {
            if page_no > pages {
                break;
            }
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![72.into(), 720.into()]),
                    Operation::new("Tj", vec![Object::string_literal(marker(tag, page_no))]),
                    Operation::new("ET", vec![]),
                    // Padding so the stream is worth compressing.
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 10.into()]),
                    Operation::new("Td", vec![72.into(), 100.into()]),
                    Operation::new(
                        "Tj",
                        vec![Object::string_literal(
                            "lorem ipsum dolor sit amet ".repeat(20),
                        )],
                    ),
                    Operation::new("ET", vec![]),
                ],
            };
            let stream = Stream::new(dictionary! {}, content.encode().unwrap());
            let content_id = doc.add_object(stream);
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => group_id,
                "Contents" => content_id,
            });
            kids.push(Object::Reference(page_id));
            page_no += 1;
        }
        let count = kids.len() as i64;
        doc.objects.insert(
            group_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Parent" => root_id,
                "Kids" => kids,
                "Count" => count,
                "Resources" => resources_id,
            }),
        );
        groups.push(Object::Reference(group_id));
    }

    doc.objects.insert(
        root_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => groups,
            "Count" => pages as i64,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => root_id,
    });
    doc.trailer.set("Root", catalog_id);

    // An object nothing refers to.
    doc.add_object(Stream::new(dictionary! {}, b"orphan ".repeat(200)));

    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal(format!("Test {tag}")),
        "Producer" => Object::string_literal("leafcut tests"),
    });
    doc.trailer.set("Info", info_id);
    doc
}

pub fn build_bytes(tag: &str, pages: u32) -> Vec<u8> {
    let mut doc = build_doc(tag, pages);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

pub fn build_pdf(tag: &str, pages: u32) -> Pdf {
    Pdf::from_bytes(&build_bytes(tag, pages)).unwrap()
}

pub fn marker(tag: &str, page: u32) -> String {
    format!("{tag}-P{page}")
}

/// Save and re-open, to be sure the output is a valid PDF that lopdf can parse.
pub fn reload(pdf: &mut Pdf) -> Pdf {
    let bytes = pdf.to_bytes().unwrap();
    Pdf::from_bytes(&bytes).expect("output should be a readable PDF")
}

/// The marker text drawn on each page, in page order.
pub fn page_markers(pdf: &Pdf) -> Vec<String> {
    let doc = pdf.as_lopdf();
    doc.get_pages()
        .values()
        .map(|&id| page_marker(doc, id))
        .collect()
}

fn page_marker(doc: &Document, id: ObjectId) -> String {
    let content = doc.get_and_decode_page_content(id).unwrap();
    for op in content.operations {
        if op.operator == "Tj" {
            if let Some(Object::String(bytes, _)) = op.operands.first() {
                let text = String::from_utf8_lossy(bytes).into_owned();
                if text.contains("-P") {
                    return text;
                }
            }
        }
    }
    panic!("page has no marker");
}

/// `["A-P1", "A-P2", ...]`
pub fn markers(tag: &str, pages: &[u32]) -> Vec<String> {
    pages.iter().map(|&p| marker(tag, p)).collect()
}

/// Effective (inherited) /Rotate of every page.
pub fn rotations(pdf: &Pdf) -> Vec<i64> {
    pdf.info().pages.iter().map(|p| p.rotation).collect()
}
