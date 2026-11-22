//! End-to-end tests that run the real `leafcut` binary on PDFs built in the test.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

fn leafcut(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_leafcut"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to run leafcut")
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "expected success, got {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// Write a simple PDF with `pages` pages whose text is `"{tag}-P{n}"`.
fn make_pdf(dir: &Path, name: &str, tag: &str, pages: u32) -> PathBuf {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let mut kids = Vec::new();
    for n in 1..=pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(format!("{tag}-P{n}"))]),
                Operation::new("Tj", vec![Object::string_literal("padding ".repeat(50))]),
                Operation::new("ET", vec![]),
            ],
        };
        let c = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => c,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => pages as i64, "Kids" => kids,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let path = dir.join(name);
    doc.save(&path).unwrap();
    path
}

fn markers(path: &Path) -> Vec<String> {
    let doc = Document::load(path).unwrap();
    doc.get_pages()
        .values()
        .map(|&id| {
            let content = doc.get_and_decode_page_content(id).unwrap();
            content
                .operations
                .iter()
                .find_map(|op| match op.operands.first() {
                    Some(Object::String(s, _)) if op.operator == "Tj" => {
                        Some(String::from_utf8_lossy(s).into_owned())
                    }
                    _ => None,
                })
                .unwrap()
        })
        .collect()
}

fn rotations(path: &Path) -> Vec<i64> {
    let doc = Document::load(path).unwrap();
    doc.get_pages()
        .values()
        .map(|&id| {
            doc.get_dictionary(id)
                .unwrap()
                .get(b"Rotate")
                .and_then(Object::as_i64)
                .unwrap_or(0)
        })
        .collect()
}

#[test]
fn merge_split_rotate_delete_compress_info() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    make_pdf(dir, "a.pdf", "A", 2);
    make_pdf(dir, "b.pdf", "B", 3);

    ok(&leafcut(&["merge", "a.pdf", "b.pdf", "-o", "ab.pdf"], dir));
    assert_eq!(
        markers(&dir.join("ab.pdf")),
        ["A-P1", "A-P2", "B-P1", "B-P2", "B-P3"]
    );

    let out = ok(&leafcut(&["split", "ab.pdf", "--ranges", "1-2,3-"], dir));
    assert!(out.contains("into 2 files"), "{out}");
    assert_eq!(markers(&dir.join("ab-1-2.pdf")), ["A-P1", "A-P2"]);
    assert_eq!(markers(&dir.join("ab-3-5.pdf")), ["B-P1", "B-P2", "B-P3"]);

    ok(&leafcut(&["split", "a.pdf", "-d", "pages"], dir));
    assert_eq!(markers(&dir.join("pages/a-1.pdf")), ["A-P1"]);
    assert_eq!(markers(&dir.join("pages/a-2.pdf")), ["A-P2"]);

    ok(&leafcut(
        &["rotate", "ab.pdf", "--angle", "90", "--pages", "1,3"],
        dir,
    ));
    assert_eq!(rotations(&dir.join("ab-rotated.pdf")), [90, 0, 90, 0, 0]);

    ok(&leafcut(
        &["delete", "ab.pdf", "--pages", "-2", "-o", "only-b.pdf"],
        dir,
    ));
    assert_eq!(markers(&dir.join("only-b.pdf")), ["B-P1", "B-P2", "B-P3"]);

    ok(&leafcut(
        &["extract", "ab.pdf", "-p", "5,1", "-o", "x.pdf"],
        dir,
    ));
    assert_eq!(markers(&dir.join("x.pdf")), ["B-P3", "A-P1"]);

    ok(&leafcut(
        &["reorder", "ab.pdf", "--reverse", "-o", "rev.pdf"],
        dir,
    ));
    assert_eq!(
        markers(&dir.join("rev.pdf")),
        ["B-P3", "B-P2", "B-P1", "A-P2", "A-P1"]
    );

    let out = ok(&leafcut(&["compress", "ab.pdf"], dir));
    assert!(out.contains('%'), "{out}");
    let before = std::fs::metadata(dir.join("ab.pdf")).unwrap().len();
    let after = std::fs::metadata(dir.join("ab-compressed.pdf"))
        .unwrap()
        .len();
    assert!(after < before, "{before} -> {after}");
    assert_eq!(markers(&dir.join("ab-compressed.pdf")).len(), 5);

    ok(&leafcut(
        &["meta", "ab.pdf", "--title", "Çok güzel", "-o", "m.pdf"],
        dir,
    ));
    let out = ok(&leafcut(&["info", "m.pdf"], dir));
    assert!(out.contains("Pages:     5"), "{out}");
    assert!(out.contains("Title:     Çok güzel"), "{out}");
    assert!(out.contains("A4"), "{out}");

    let out = ok(&leafcut(&["info", "--json", "m.pdf"], dir));
    assert!(out.contains("\"pages\":5"), "{out}");
    assert!(out.contains("\"Title\":\"Çok güzel\""), "{out}");
}

#[test]
fn never_overwrites_without_permission() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let input = make_pdf(dir, "in.pdf", "I", 2);
    let original = std::fs::read(&input).unwrap();

    // Output == input is refused.
    let out = leafcut(&["rotate", "in.pdf", "-a", "90", "-o", "in.pdf"], dir);
    assert_eq!(code(&out), 2);
    assert!(String::from_utf8_lossy(&out.stderr).contains("--in-place"));
    assert_eq!(std::fs::read(&input).unwrap(), original);

    // Existing output is refused without --force.
    ok(&leafcut(&["rotate", "in.pdf", "-a", "90"], dir));
    let out = leafcut(&["rotate", "in.pdf", "-a", "180"], dir);
    assert_eq!(code(&out), 4);
    assert_eq!(rotations(&dir.join("in-rotated.pdf")), [90, 90]);
    ok(&leafcut(&["rotate", "in.pdf", "-a", "180", "--force"], dir));
    assert_eq!(rotations(&dir.join("in-rotated.pdf")), [180, 180]);

    // Merge output that is also an input is refused.
    let out = leafcut(
        &["merge", "in.pdf", "in-rotated.pdf", "-o", "in.pdf", "-f"],
        dir,
    );
    assert_eq!(code(&out), 2);
    assert_eq!(std::fs::read(&input).unwrap(), original);

    // --in-place is honoured.
    ok(&leafcut(
        &["rotate", "in.pdf", "-a", "270", "--in-place"],
        dir,
    ));
    assert_eq!(rotations(&input), [270, 270]);
}

#[test]
fn exit_codes_for_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    make_pdf(dir, "in.pdf", "I", 3);
    std::fs::write(dir.join("junk.pdf"), b"not a pdf").unwrap();

    assert_eq!(code(&leafcut(&["delete", "in.pdf", "-p", "9"], dir)), 2);
    assert_eq!(code(&leafcut(&["delete", "in.pdf", "-p", "1-3"], dir)), 2);
    assert_eq!(code(&leafcut(&["delete", "in.pdf", "-p", "3-1"], dir)), 2);
    assert_eq!(code(&leafcut(&["rotate", "in.pdf", "-a", "45"], dir)), 2);
    assert_eq!(code(&leafcut(&["reorder", "in.pdf", "-r", "1,2"], dir)), 2);
    assert_eq!(code(&leafcut(&["meta", "in.pdf"], dir)), 2);
    assert_eq!(code(&leafcut(&["info", "junk.pdf"], dir)), 1);
    assert_eq!(code(&leafcut(&["info", "missing.pdf"], dir)), 1);
}

#[test]
fn encrypted_input_exit_code() {
    use lopdf::{EncryptionState, EncryptionVersion, Permissions};
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let path = make_pdf(dir, "plain.pdf", "E", 1);
    let mut doc = Document::load(&path).unwrap();
    let id = Object::String(
        b"0123456789abcdef".to_vec(),
        lopdf::StringFormat::Hexadecimal,
    );
    doc.trailer.set("ID", vec![id.clone(), id]);
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &doc,
        owner_password: "o",
        user_password: "u",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    doc.save(dir.join("locked.pdf")).unwrap();

    let out = leafcut(&["info", "locked.pdf"], dir);
    assert_eq!(code(&out), 3);
    assert!(String::from_utf8_lossy(&out.stderr).contains("encrypted"));
}
