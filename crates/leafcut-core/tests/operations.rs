mod common;

use common::*;
use leafcut_core::{
    merge, merge_files, plan_split, Error, Metadata, PageRanges, Pdf, Rotation, SplitMode,
};
use lopdf::{dictionary, Object};

// ---------------------------------------------------------------- open / info

#[test]
fn open_reports_pages_sizes_and_metadata() {
    let pdf = build_pdf("A", 5);
    assert_eq!(pdf.page_count(), 5);
    let info = pdf.info();
    assert_eq!(info.page_count, 5);
    assert_eq!(info.version, "1.5");
    assert!(!info.has_forms);
    assert!(!info.has_outlines);
    // MediaBox is inherited from the root /Pages node.
    for page in &info.pages {
        assert_eq!((page.width_pt, page.height_pt), (595.0, 842.0));
        assert_eq!(page.paper_name(), Some("A4"));
        assert_eq!(page.rotation, 0);
    }
    assert_eq!(info.metadata.title.as_deref(), Some("Test A"));
    assert_eq!(info.metadata.producer.as_deref(), Some("leafcut tests"));
    assert_eq!(info.metadata.author, None);
    assert_eq!(page_markers(&pdf), markers("A", &[1, 2, 3, 4, 5]));
}

#[test]
fn open_from_disk_and_save_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    std::fs::write(&input, build_bytes("D", 3)).unwrap();

    let mut pdf = Pdf::open(&input).unwrap();
    let output = dir.path().join("out.pdf");
    pdf.save(&output).unwrap();
    let again = Pdf::open(&output).unwrap();
    assert_eq!(page_markers(&again), markers("D", &[1, 2, 3]));

    // No temporary files are left behind.
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names.len(), 2, "unexpected files: {names:?}");
}

#[test]
fn missing_file_is_an_io_error() {
    let err = Pdf::open("/definitely/not/here.pdf").unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "{err:?}");
}

#[test]
fn garbage_is_rejected() {
    let err = Pdf::from_bytes(b"this is not a pdf at all").unwrap_err();
    assert!(matches!(err, Error::Pdf(_)), "{err:?}");
}

#[test]
fn encrypted_pdf_is_detected() {
    use lopdf::{EncryptionState, EncryptionVersion, Permissions};

    for user_password in ["secret", ""] {
        let mut doc = build_doc("E", 2);
        let id = Object::String(
            b"0123456789abcdef".to_vec(),
            lopdf::StringFormat::Hexadecimal,
        );
        doc.trailer.set("ID", vec![id.clone(), id]);
        let version = EncryptionVersion::V2 {
            document: &doc,
            owner_password: "owner",
            user_password,
            key_length: 128,
            permissions: Permissions::all(),
        };
        let state = EncryptionState::try_from(version).unwrap();
        doc.encrypt(&state).unwrap();
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();

        let err = Pdf::from_bytes(&bytes).unwrap_err();
        assert!(
            matches!(err, Error::Encrypted),
            "user password {user_password:?}: {err:?}"
        );
        assert!(err.to_string().contains("encrypted"));
    }
}

// ---------------------------------------------------------------- merge

#[test]
fn merge_keeps_order_and_all_pages() {
    let a = build_pdf("A", 3);
    let b = build_pdf("B", 2);
    let c = build_pdf("C", 1);
    let mut merged = merge(vec![a, b, c]).unwrap();
    let merged = reload(&mut merged);
    assert_eq!(merged.page_count(), 6);
    let mut expected = markers("A", &[1, 2, 3]);
    expected.extend(markers("B", &[1, 2]));
    expected.extend(markers("C", &[1]));
    assert_eq!(page_markers(&merged), expected);
    // Inherited MediaBox survived the merge.
    assert!(merged
        .info()
        .pages
        .iter()
        .all(|p| p.paper_name() == Some("A4")));
}

#[test]
fn merge_same_document_twice() {
    let a = build_pdf("A", 2);
    let mut merged = merge(vec![a.clone(), a]).unwrap();
    let merged = reload(&mut merged);
    assert_eq!(page_markers(&merged), markers("A", &[1, 2, 1, 2]));
}

#[test]
fn merge_files_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    std::fs::write(&a, build_bytes("A", 1)).unwrap();
    std::fs::write(&b, build_bytes("B", 2)).unwrap();
    let merged = merge_files(&[&a, &b]).unwrap();
    assert_eq!(page_markers(&merged), ["A-P1", "B-P1", "B-P2"]);
}

#[test]
fn merge_nothing_is_an_error() {
    assert!(matches!(merge(vec![]), Err(Error::NothingToMerge)));
}

// ---------------------------------------------------------------- split / extract

#[test]
fn split_every_page() {
    let pdf = build_pdf("S", 4);
    let parts = pdf.split(&SplitMode::EveryPage).unwrap();
    assert_eq!(parts.len(), 4);
    for (i, (part, mut doc)) in parts.into_iter().enumerate() {
        let page = i as u32 + 1;
        assert_eq!(part.pages, vec![page]);
        let doc = reload(&mut doc);
        assert_eq!(page_markers(&doc), markers("S", &[page]));
        assert_eq!(doc.info().pages[0].paper_name(), Some("A4"));
    }
}

#[test]
fn split_by_ranges() {
    let pdf = build_pdf("S", 12);
    let mode = SplitMode::Ranges(PageRanges::parse("1-3,7,10-").unwrap());
    let parts = pdf.split(&mode).unwrap();
    let got: Vec<_> = parts
        .into_iter()
        .map(|(part, mut doc)| (part.label, page_markers(&reload(&mut doc))))
        .collect();
    assert_eq!(
        got,
        vec![
            ("01-03".to_string(), markers("S", &[1, 2, 3])),
            ("07".to_string(), markers("S", &[7])),
            ("10-12".to_string(), markers("S", &[10, 11, 12])),
        ]
    );
}

#[test]
fn split_plan_matches_split() {
    let pdf = build_pdf("S", 5);
    let plan = plan_split(&SplitMode::Chunks(2), pdf.page_count()).unwrap();
    let parts = pdf.split(&SplitMode::Chunks(2)).unwrap();
    assert_eq!(plan.len(), 3);
    assert_eq!(parts.len(), 3);
    assert_eq!(page_markers(&parts[2].1), markers("S", &[5]));
}

#[test]
fn extract_respects_order_and_duplicates() {
    let pdf = build_pdf("X", 5);
    let mut out = pdf.extract(&[5, 1, 1, 3]).unwrap();
    let out = reload(&mut out);
    assert_eq!(page_markers(&out), markers("X", &[5, 1, 1, 3]));
    // The source is untouched.
    assert_eq!(pdf.page_count(), 5);
}

#[test]
fn extract_ranges_expression() {
    let pdf = build_pdf("X", 6);
    let out = pdf
        .extract_ranges(&PageRanges::parse("2-3,6").unwrap())
        .unwrap();
    assert_eq!(page_markers(&out), markers("X", &[2, 3, 6]));
}

#[test]
fn extract_drops_content_of_other_pages() {
    let pdf = build_pdf("X", 4);
    let mut out = pdf.extract(&[2]).unwrap();
    let bytes = out.to_bytes().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("X-P2"));
    for gone in ["X-P1", "X-P3", "X-P4", "orphan"] {
        assert!(!text.contains(gone), "{gone} should have been dropped");
    }
}

#[test]
fn extract_out_of_range() {
    let pdf = build_pdf("X", 3);
    assert!(matches!(
        pdf.extract(&[4]),
        Err(Error::PageOutOfRange {
            page: 4,
            page_count: 3
        })
    ));
    assert!(matches!(
        pdf.extract(&[0]),
        Err(Error::PageOutOfRange { .. })
    ));
    assert!(matches!(pdf.extract(&[]), Err(Error::EmptySelection)));
}

// ---------------------------------------------------------------- delete

#[test]
fn delete_pages_works() {
    let mut pdf = build_pdf("D", 5);
    pdf.delete_pages(&[2, 4, 4]).unwrap();
    let pdf = reload(&mut pdf);
    assert_eq!(page_markers(&pdf), markers("D", &[1, 3, 5]));
}

#[test]
fn delete_all_pages_is_refused() {
    let mut pdf = build_pdf("D", 2);
    assert!(matches!(
        pdf.delete_pages(&[1, 2]),
        Err(Error::WouldDeleteAllPages)
    ));
    assert_eq!(pdf.page_count(), 2);
}

#[test]
fn delete_out_of_range_page() {
    let mut pdf = build_pdf("D", 2);
    assert!(matches!(
        pdf.delete_pages(&[3]),
        Err(Error::PageOutOfRange { .. })
    ));
}

// ---------------------------------------------------------------- rotate

#[test]
fn rotate_selected_pages() {
    let mut pdf = build_pdf("R", 4);
    pdf.rotate(Some(&[2, 4]), Rotation::Cw90).unwrap();
    let mut pdf = reload(&mut pdf);
    assert_eq!(rotations(&pdf), vec![0, 90, 0, 90]);
    // Rotation accumulates and wraps.
    pdf.rotate(Some(&[2]), Rotation::Cw270).unwrap();
    pdf.rotate(None, Rotation::Cw180).unwrap();
    let pdf = reload(&mut pdf);
    assert_eq!(rotations(&pdf), vec![180, 180, 180, 270]);
    assert_eq!(page_markers(&pdf), markers("R", &[1, 2, 3, 4]));
}

#[test]
fn rotate_respects_inherited_rotation() {
    let mut doc = build_doc("R", 2);
    // Put /Rotate 90 on the intermediate node holding pages 1 and 2.
    let first_page = *doc.get_pages().get(&1).unwrap();
    let parent = doc
        .get_dictionary(first_page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(parent).unwrap().set("Rotate", 90);
    let mut pdf = Pdf::from_document(doc).unwrap();
    assert_eq!(rotations(&pdf), vec![90, 90]);

    pdf.rotate(Some(&[1]), Rotation::Cw90).unwrap();
    assert_eq!(rotations(&pdf), vec![180, 90]);

    // Extracting page 2 alone must keep its inherited rotation.
    let mut only_two = pdf.extract(&[2]).unwrap();
    assert_eq!(rotations(&reload(&mut only_two)), vec![90]);
}

#[test]
fn rotation_parsing() {
    assert_eq!(Rotation::from_degrees(90).unwrap(), Rotation::Cw90);
    assert_eq!(Rotation::from_degrees(-90).unwrap(), Rotation::Cw270);
    assert_eq!(Rotation::from_degrees(540).unwrap(), Rotation::Cw180);
    assert!(matches!(
        Rotation::from_degrees(45),
        Err(Error::InvalidRotation(45))
    ));
    assert!(Rotation::from_degrees(0).is_err());
    assert!(Rotation::from_degrees(360).is_err());
}

// ---------------------------------------------------------------- reorder

#[test]
fn reorder_pages() {
    let mut pdf = build_pdf("O", 4);
    pdf.reorder(&[4, 2, 3, 1]).unwrap();
    let pdf = reload(&mut pdf);
    assert_eq!(page_markers(&pdf), markers("O", &[4, 2, 3, 1]));
}

#[test]
fn reorder_with_range_expression() {
    let mut pdf = build_pdf("O", 6);
    let order = PageRanges::parse("5-6,1-4")
        .unwrap()
        .resolve(pdf.page_count())
        .unwrap();
    pdf.reorder(&order).unwrap();
    assert_eq!(page_markers(&pdf), markers("O", &[5, 6, 1, 2, 3, 4]));
}

#[test]
fn reorder_rejects_incomplete_or_duplicate_orders() {
    let mut pdf = build_pdf("O", 3);
    assert!(matches!(pdf.reorder(&[1, 2]), Err(Error::InvalidOrder(_))));
    assert!(matches!(
        pdf.reorder(&[1, 2, 2]),
        Err(Error::InvalidOrder(_))
    ));
    assert!(matches!(
        pdf.reorder(&[1, 2, 3, 4]),
        Err(Error::PageOutOfRange { .. })
    ));
    assert_eq!(page_markers(&pdf), markers("O", &[1, 2, 3]));
}

// ---------------------------------------------------------------- compress

#[test]
fn compress_shrinks_and_keeps_content() {
    let original = build_bytes("C", 8);
    let mut pdf = Pdf::from_bytes(&original).unwrap();
    let report = pdf.compress();
    assert!(report.objects_removed >= 1, "{report:?}");
    assert!(report.streams_compressed >= 8, "{report:?}");
    assert!(report.objects_after < report.objects_before);

    let compressed = pdf.to_bytes().unwrap();
    assert!(
        compressed.len() * 2 < original.len(),
        "expected at least 50% smaller: {} -> {}",
        original.len(),
        compressed.len()
    );
    let text = String::from_utf8_lossy(&compressed);
    assert!(text.contains("/ObjStm"), "object streams expected");

    let again = Pdf::from_bytes(&compressed).unwrap();
    assert_eq!(
        page_markers(&again),
        markers("C", &[1, 2, 3, 4, 5, 6, 7, 8])
    );
    assert_eq!(again.metadata().title.as_deref(), Some("Test C"));
}

#[test]
fn compress_twice_is_stable() {
    let mut pdf = build_pdf("C", 3);
    pdf.compress();
    let first = pdf.to_bytes().unwrap();
    let mut pdf = Pdf::from_bytes(&first).unwrap();
    let report = pdf.compress();
    assert_eq!(report.streams_compressed, 0);
    let second = pdf.to_bytes().unwrap();
    assert!(second.len() <= first.len() + 64);
    assert_eq!(page_markers(&Pdf::from_bytes(&second).unwrap()).len(), 3);
}

#[test]
fn compress_recompresses_weak_flate_streams() {
    use lopdf::Stream;
    let mut doc = build_doc("W", 1);
    // A Flate stream stored with no real compression (zlib "stored" blocks).
    let raw = b"0 0 m 100 100 l S\n".repeat(400);
    let weak = stored_zlib(&raw);
    let id = doc.add_object(Stream::new(
        dictionary! { "Filter" => "FlateDecode" },
        weak.clone(),
    ));
    let page = *doc.get_pages().get(&1).unwrap();
    let contents = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .clone();
    doc.get_dictionary_mut(page)
        .unwrap()
        .set("Contents", vec![contents, Object::Reference(id)]);

    let mut pdf = Pdf::from_document(doc).unwrap();
    let report = pdf.compress();
    assert!(report.streams_recompressed >= 1, "{report:?}");
    let again = reload(&mut pdf);
    let page = *again.as_lopdf().get_pages().get(&1).unwrap();
    let content = again.as_lopdf().get_page_content(page);
    assert!(content.windows(raw.len()).any(|w| w == raw.as_slice()));
}

/// zlib container with uncompressed ("stored") deflate blocks.
fn stored_zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        out.push(u8::from(i + 1 == chunks.len()));
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

// ---------------------------------------------------------------- metadata

#[test]
fn set_metadata_with_unicode() {
    let mut pdf = build_pdf("M", 1);
    let update = Metadata {
        title: Some("Yıllık Rapor — Çalışma Ağı".into()),
        author: Some("Şükrü Öztürk".into()),
        keywords: Some("pdf, leafcut".into()),
        producer: Some(String::new()), // remove
        ..Default::default()
    };
    pdf.set_metadata(&update).unwrap();
    let pdf = reload(&mut pdf);
    let meta = pdf.metadata();
    assert_eq!(meta.title.as_deref(), Some("Yıllık Rapor — Çalışma Ağı"));
    assert_eq!(meta.author.as_deref(), Some("Şükrü Öztürk"));
    assert_eq!(meta.keywords.as_deref(), Some("pdf, leafcut"));
    assert_eq!(meta.producer, None);
    assert_eq!(meta.subject, None);
}

#[test]
fn set_metadata_creates_info_when_missing() {
    let mut doc = build_doc("M", 1);
    doc.trailer.remove(b"Info");
    let mut pdf = Pdf::from_document(doc).unwrap();
    assert!(pdf.metadata().is_empty());
    pdf.set_metadata(&Metadata {
        subject: Some("Konu".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(reload(&mut pdf).metadata().subject.as_deref(), Some("Konu"));
}

#[test]
fn clear_metadata_removes_info_and_xmp() {
    use lopdf::Stream;
    let mut doc = build_doc("M", 1);
    let xmp = doc.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        b"<x:xmpmeta>Test M</x:xmpmeta>".to_vec(),
    ));
    doc.catalog_mut().unwrap().set("Metadata", xmp);

    let mut pdf = Pdf::from_document(doc).unwrap();
    pdf.clear_metadata().unwrap();
    let bytes = pdf.to_bytes().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("xmpmeta"));
    assert!(!text.contains("Test M"));
    let pdf = Pdf::from_bytes(&bytes).unwrap();
    assert!(pdf.metadata().is_empty());
    assert_eq!(pdf.page_count(), 1);
}

// ---------------------------------------------------------------- combined

#[test]
fn chained_operations() {
    let a = build_pdf("A", 3);
    let b = build_pdf("B", 3);
    let mut doc = merge(vec![a, b]).unwrap();
    doc.delete_pages(&[1, 6]).unwrap(); // A2 A3 B1 B2
    doc.reorder(&[4, 3, 2, 1]).unwrap(); // B2 B1 A3 A2
    doc.rotate(Some(&[1]), Rotation::Cw90).unwrap();
    doc.compress();
    let doc = reload(&mut doc);
    assert_eq!(page_markers(&doc), ["B-P2", "B-P1", "A-P3", "A-P2"]);
    assert_eq!(rotations(&doc), vec![90, 0, 0, 0]);
}
