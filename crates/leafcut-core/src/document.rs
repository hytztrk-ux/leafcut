//! The [`Pdf`] document type and all single-document operations.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::{Error, Result};
use crate::info::{Metadata, PageInfo, PdfInfo};
use crate::pages::PageRanges;

/// Largest stream leafcut will inflate while optimising (protects against
/// decompression bombs).
const MAX_INFLATE: usize = 256 * 1024 * 1024;

/// Page attributes that a page may inherit from its ancestors in the page tree.
const INHERITABLE: [&[u8]; 4] = [b"MediaBox", b"CropBox", b"Resources", b"Rotate"];

/// A clockwise page rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rotation {
    /// 90 degrees clockwise.
    Cw90,
    /// 180 degrees.
    Cw180,
    /// 270 degrees clockwise (= 90 degrees counter-clockwise).
    Cw270,
}

impl Rotation {
    /// Build from degrees. Accepts 90, 180, 270 and their negative /
    /// over-full equivalents (e.g. -90 = 270, 450 = 90).
    pub fn from_degrees(degrees: i64) -> Result<Self> {
        if degrees % 90 != 0 {
            return Err(Error::InvalidRotation(degrees));
        }
        match degrees.rem_euclid(360) {
            90 => Ok(Rotation::Cw90),
            180 => Ok(Rotation::Cw180),
            270 => Ok(Rotation::Cw270),
            _ => Err(Error::InvalidRotation(degrees)),
        }
    }

    /// The rotation in degrees (90, 180 or 270).
    pub fn degrees(self) -> i64 {
        match self {
            Rotation::Cw90 => 90,
            Rotation::Cw180 => 180,
            Rotation::Cw270 => 270,
        }
    }
}

/// What [`Pdf::compress`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompressReport {
    /// Objects before optimising.
    pub objects_before: usize,
    /// Objects after optimising.
    pub objects_after: usize,
    /// Unreferenced objects that were dropped.
    pub objects_removed: usize,
    /// Uncompressed streams that were Flate-compressed.
    pub streams_compressed: usize,
    /// Already-Flate streams that got smaller by recompressing at maximum level.
    pub streams_recompressed: usize,
}

/// An open, unencrypted PDF document held in memory.
///
/// All editing operations work in memory; nothing touches the disk until
/// [`Pdf::save`] is called.
#[derive(Debug, Clone)]
pub struct Pdf {
    doc: Document,
    object_streams: bool,
}

impl Pdf {
    /// Open a PDF file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
        Self::from_bytes(&bytes)
    }

    /// Parse a PDF from memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let doc = match Document::load_mem(bytes) {
            Ok(doc) => doc,
            Err(err) => {
                if looks_encrypted(bytes) {
                    return Err(Error::Encrypted);
                }
                return Err(Error::Pdf(err));
            }
        };
        Self::from_document(doc)
    }

    /// Wrap an existing `lopdf` document.
    pub fn from_document(doc: Document) -> Result<Self> {
        if doc.trailer.has(b"Encrypt") || doc.was_encrypted() {
            return Err(Error::Encrypted);
        }
        let pdf = Pdf {
            doc,
            object_streams: false,
        };
        if pdf.page_count() == 0 {
            return Err(Error::NoPages);
        }
        Ok(pdf)
    }

    /// Borrow the underlying `lopdf` document (read-only escape hatch).
    pub fn as_lopdf(&self) -> &Document {
        &self.doc
    }

    /// Consume and return the underlying `lopdf` document.
    pub fn into_lopdf(self) -> Document {
        self.doc
    }

    /// Number of pages.
    pub fn page_count(&self) -> u32 {
        self.doc.get_pages().len() as u32
    }

    /// Whether [`Pdf::save`] writes compressed object streams and a
    /// cross-reference stream (PDF 1.5+). Enabled by [`Pdf::compress`].
    pub fn set_object_streams(&mut self, enabled: bool) {
        self.object_streams = enabled;
    }

    /// Serialise to bytes.
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let written = if self.object_streams {
            self.doc.save_modern(&mut out)
        } else {
            self.doc.save_to(&mut out)
        };
        written.map_err(|e| Error::Pdf(lopdf::Error::IO(e)))?;
        Ok(out)
    }

    /// Write the document to `path`.
    ///
    /// The file is first written to a temporary file next to the target and
    /// then renamed over it, so a failure never leaves a half-written PDF
    /// behind. Whether overwriting an existing file is acceptable is the
    /// caller's decision.
    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.to_bytes()?;
        write_atomic(path, &bytes)
    }

    /// Page numbers as a sorted list of object ids, page 1 first.
    fn page_ids(&self) -> Vec<ObjectId> {
        self.doc.get_pages().into_values().collect()
    }

    fn check_pages(&self, pages: &[u32]) -> Result<()> {
        let page_count = self.page_count();
        if pages.is_empty() {
            return Err(Error::EmptySelection);
        }
        if let Some(&page) = pages.iter().find(|&&p| p == 0 || p > page_count) {
            return Err(Error::PageOutOfRange { page, page_count });
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Page operations
    // ------------------------------------------------------------------

    /// Rotate the given pages (1-based) clockwise. `None` rotates every page.
    /// Rotation is added to any rotation the page already has.
    pub fn rotate(&mut self, pages: Option<&[u32]>, rotation: Rotation) -> Result<()> {
        let ids = self.page_ids();
        let targets: Vec<u32> = match pages {
            Some(p) => {
                self.check_pages(p)?;
                let set: BTreeSet<u32> = p.iter().copied().collect();
                set.into_iter().collect()
            }
            None => (1..=ids.len() as u32).collect(),
        };
        for page in targets {
            let id = ids[(page - 1) as usize];
            let current = inherited(&self.doc, id, b"Rotate")
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0);
            let new = (current + rotation.degrees()).rem_euclid(360);
            self.doc.get_dictionary_mut(id)?.set("Rotate", new);
        }
        Ok(())
    }

    /// Keep only the given pages, in the given order. Pages may repeat.
    pub fn extract(&self, pages: &[u32]) -> Result<Pdf> {
        self.check_pages(pages)?;
        let mut out = self.clone();
        out.rebuild_page_tree(pages)?;
        Ok(out)
    }

    /// Like [`Pdf::extract`], taking a page-range expression.
    pub fn extract_ranges(&self, ranges: &PageRanges) -> Result<Pdf> {
        let pages = ranges.resolve(self.page_count())?;
        self.extract(&pages)
    }

    /// Delete the given pages (1-based). Duplicates are ignored. At least one
    /// page must remain.
    pub fn delete_pages(&mut self, pages: &[u32]) -> Result<()> {
        self.check_pages(pages)?;
        let remove: HashSet<u32> = pages.iter().copied().collect();
        let keep: Vec<u32> = (1..=self.page_count())
            .filter(|p| !remove.contains(p))
            .collect();
        if keep.is_empty() {
            return Err(Error::WouldDeleteAllPages);
        }
        self.rebuild_page_tree(&keep)
    }

    /// Put the pages in a new order. `order` must name every page exactly once.
    pub fn reorder(&mut self, order: &[u32]) -> Result<()> {
        let page_count = self.page_count();
        self.check_pages(order)?;
        let mut seen = vec![false; page_count as usize];
        for &p in order {
            let slot = &mut seen[(p - 1) as usize];
            if *slot {
                return Err(Error::InvalidOrder(format!(
                    "page {p} appears more than once"
                )));
            }
            *slot = true;
        }
        if let Some(missing) = seen.iter().position(|s| !s) {
            return Err(Error::InvalidOrder(format!(
                "page {} is missing (the order must list all {page_count} pages; use extract to keep only some)",
                missing + 1
            )));
        }
        self.rebuild_page_tree(order)
    }

    /// Replace the page tree with a flat one containing `pages` (1-based, in
    /// order, repeats allowed), then drop everything that is no longer used.
    fn rebuild_page_tree(&mut self, pages: &[u32]) -> Result<()> {
        let old_ids = self.page_ids();
        let root_pages = self.doc.catalog()?.get(b"Pages")?.as_reference()?;

        // Collect intermediate page-tree nodes so they can be dropped.
        let mut tree_nodes = Vec::new();
        collect_tree_nodes(&self.doc, root_pages, &mut tree_nodes, &mut HashSet::new());

        // Materialise inherited attributes on every page we keep, because
        // the old ancestors are about to disappear.
        let kept: HashSet<ObjectId> = pages.iter().map(|&p| old_ids[(p - 1) as usize]).collect();
        for &id in &kept {
            materialize_inherited(&mut self.doc, id)?;
        }

        let new_root = self.doc.new_object_id();
        let mut used = HashSet::new();
        let mut kids = Vec::with_capacity(pages.len());
        for &p in pages {
            let src = old_ids[(p - 1) as usize];
            let id = if used.insert(src) {
                src
            } else {
                // A page listed twice: duplicate the page dictionary. Content
                // streams and resources are shared, not copied.
                let copy = self.doc.get_dictionary(src)?.clone();
                self.doc.add_object(copy)
            };
            self.doc.get_dictionary_mut(id)?.set("Parent", new_root);
            kids.push(Object::Reference(id));
        }

        let mut root = Dictionary::new();
        root.set("Type", "Pages");
        root.set("Count", kids.len() as i64);
        root.set("Kids", kids);
        self.doc.objects.insert(new_root, Object::Dictionary(root));
        self.doc.catalog_mut()?.set("Pages", new_root);

        for id in tree_nodes {
            self.doc.objects.remove(&id);
        }
        for id in old_ids.into_iter().filter(|id| !kept.contains(id)) {
            self.doc.objects.remove(&id);
        }
        self.cleanup();
        Ok(())
    }

    /// Replace references to objects that no longer exist with `null`, then
    /// drop unreachable objects.
    fn cleanup(&mut self) {
        null_dangling_references(&mut self.doc);
        self.doc.prune_objects();
    }

    // ------------------------------------------------------------------
    // Optimisation
    // ------------------------------------------------------------------

    /// Optimise the document losslessly:
    ///
    /// * drop objects that nothing refers to,
    /// * Flate-compress streams that are stored uncompressed,
    /// * recompress existing Flate streams at maximum level when that is smaller,
    /// * renumber objects densely,
    /// * and make [`Pdf::save`] pack objects into compressed object streams.
    ///
    /// Images are **not** downsampled or re-encoded, so documents that are
    /// mostly scanned images usually shrink very little.
    pub fn compress(&mut self) -> CompressReport {
        let objects_before = self.doc.objects.len();
        self.cleanup();
        let after_prune = self.doc.objects.len();

        let mut streams_compressed = 0;
        let mut streams_recompressed = 0;
        for object in self.doc.objects.values_mut() {
            let Object::Stream(stream) = object else {
                continue;
            };
            let filter = stream.dict.get(b"Filter").ok().cloned();
            match filter {
                None => {
                    if stream.allows_compression {
                        let before = stream.content.len();
                        if stream.compress().is_ok() && stream.content.len() < before {
                            streams_compressed += 1;
                        }
                    }
                }
                Some(f) if is_plain_flate(&f) && !stream.dict.has(b"DecodeParms") => {
                    let Ok(raw) = stream.decompressed_content_with_limit(MAX_INFLATE) else {
                        continue;
                    };
                    let mut candidate = stream.clone();
                    candidate.dict.remove(b"Filter");
                    candidate.set_content(raw);
                    if candidate.compress().is_ok()
                        && candidate.dict.has(b"Filter")
                        && candidate.content.len() < stream.content.len()
                    {
                        *stream = candidate;
                        streams_recompressed += 1;
                    }
                }
                Some(_) => {}
            }
        }

        self.doc.renumber_objects();
        self.object_streams = true;
        let objects_after = self.doc.objects.len();
        CompressReport {
            objects_before,
            objects_after,
            objects_removed: objects_before - after_prune,
            streams_compressed,
            streams_recompressed,
        }
    }

    // ------------------------------------------------------------------
    // Metadata and info
    // ------------------------------------------------------------------

    /// Read the document information dictionary.
    pub fn metadata(&self) -> Metadata {
        let mut meta = Metadata::default();
        let Some(info) = self.info_dict() else {
            return meta;
        };
        let get = |key: &[u8]| -> Option<String> {
            let obj = info.get(key).ok()?;
            let obj = match obj {
                Object::Reference(id) => self.doc.get_object(*id).ok()?,
                other => other,
            };
            lopdf::decode_text_string(obj).ok().or_else(|| {
                obj.as_str()
                    .ok()
                    .map(|b| String::from_utf8_lossy(b).into_owned())
            })
        };
        meta.title = get(b"Title");
        meta.author = get(b"Author");
        meta.subject = get(b"Subject");
        meta.keywords = get(b"Keywords");
        meta.creator = get(b"Creator");
        meta.producer = get(b"Producer");
        meta.creation_date = get(b"CreationDate");
        meta.mod_date = get(b"ModDate");
        meta
    }

    fn info_dict(&self) -> Option<&Dictionary> {
        match self.doc.trailer.get(b"Info").ok()? {
            Object::Reference(id) => self.doc.get_dictionary(*id).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        }
    }

    /// Update metadata. Fields that are `None` are left as they are; fields
    /// set to an empty string are removed.
    ///
    /// Any XMP metadata stream attached to the catalog is removed, because it
    /// would otherwise keep showing the old values in many viewers.
    pub fn set_metadata(&mut self, update: &Metadata) -> Result<()> {
        let info_id = match self.doc.trailer.get(b"Info").ok() {
            Some(Object::Reference(id)) if self.doc.get_dictionary(*id).is_ok() => *id,
            Some(Object::Dictionary(d)) => {
                let d = d.clone();
                let id = self.doc.add_object(d);
                self.doc.trailer.set("Info", id);
                id
            }
            _ => {
                let id = self.doc.add_object(Dictionary::new());
                self.doc.trailer.set("Info", id);
                id
            }
        };
        let info = self.doc.get_dictionary_mut(info_id)?;
        for (key, value) in update.entries() {
            if let Some(value) = value {
                if value.is_empty() {
                    info.remove(key.as_bytes());
                } else {
                    info.set(key, lopdf::text_string(value));
                }
            }
        }
        self.remove_xmp();
        self.cleanup();
        Ok(())
    }

    /// Remove all document metadata: the information dictionary and the XMP
    /// metadata stream.
    pub fn clear_metadata(&mut self) -> Result<()> {
        self.doc.trailer.remove(b"Info");
        self.remove_xmp();
        self.cleanup();
        Ok(())
    }

    fn remove_xmp(&mut self) {
        if let Ok(catalog) = self.doc.catalog_mut() {
            catalog.remove(b"Metadata");
        }
    }

    /// Collect information about the document.
    pub fn info(&self) -> PdfInfo {
        let pages = self
            .page_ids()
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                let (width, height) = inherited(&self.doc, id, b"MediaBox")
                    .and_then(|b| box_size(&self.doc, &b))
                    .unwrap_or((612.0, 792.0));
                let rotation = inherited(&self.doc, id, b"Rotate")
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(0)
                    .rem_euclid(360);
                PageInfo {
                    number: i as u32 + 1,
                    width_pt: width,
                    height_pt: height,
                    rotation,
                }
            })
            .collect::<Vec<_>>();
        let catalog = self.doc.catalog().ok();
        PdfInfo {
            version: self.doc.version.clone(),
            page_count: pages.len() as u32,
            object_count: self.doc.objects.len(),
            has_outlines: catalog.is_some_and(|c| c.has(b"Outlines")),
            has_forms: catalog.is_some_and(|c| c.has(b"AcroForm")),
            metadata: self.metadata(),
            pages,
        }
    }
}

/// Merge documents in order into a new document.
///
/// Page content, resources and annotations are kept. Document-level
/// structures of the inputs (bookmarks, form fields, named destinations) are
/// not carried over.
pub fn merge(documents: Vec<Pdf>) -> Result<Pdf> {
    if documents.is_empty() {
        return Err(Error::NothingToMerge);
    }
    let mut version = String::from("1.4");
    let mut out = Document::with_version("1.4");
    let mut next_id = 1;
    let mut kids = Vec::new();

    for pdf in documents {
        let mut doc = pdf.doc;
        if doc.version > version {
            version = doc.version.clone();
        }
        for id in doc.get_pages().into_values() {
            materialize_inherited(&mut doc, id)?;
        }
        null_dangling_references(&mut doc);
        doc.prune_objects();
        doc.renumber_objects_with(next_id);
        next_id = doc.max_id + 1;
        kids.extend(doc.get_pages().into_values().map(Object::Reference));
        out.objects.extend(doc.objects);
    }

    out.version = version;
    out.max_id = next_id - 1;
    let pages_id = out.new_object_id();
    for kid in &kids {
        if let Object::Reference(id) = kid {
            out.get_dictionary_mut(*id)?.set("Parent", pages_id);
        }
    }
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", kids.len() as i64);
    pages.set("Kids", kids);
    out.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = out.add_object(catalog);
    out.trailer.set("Root", catalog_id);

    out.prune_objects();
    Pdf::from_document(out)
}

/// Merge PDF files from disk, in order.
pub fn merge_files<P: AsRef<Path>>(paths: &[P]) -> Result<Pdf> {
    let docs = paths
        .iter()
        .map(|p| Pdf::open(p.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    merge(docs)
}

/// Write `bytes` to `path` via a temporary file in the same directory.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output.pdf".into());
    let tmp = dir.join(format!(".{name}.{}.leafcut-tmp", std::process::id()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(Error::io(path, e));
    }
    Ok(())
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn looks_encrypted(bytes: &[u8]) -> bool {
    bytes.windows(8).any(|w| w == b"/Encrypt")
}

fn is_plain_flate(filter: &Object) -> bool {
    match filter {
        Object::Name(n) => n == b"FlateDecode",
        Object::Array(a) => a.len() == 1 && matches!(&a[0], Object::Name(n) if n == b"FlateDecode"),
        _ => false,
    }
}

/// Look up an inheritable page attribute, walking up `/Parent` links.
fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = page;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let dict = doc.get_dictionary(current).ok()?;
        if let Ok(value) = dict.get(key) {
            match value {
                Object::Reference(id) => match doc.get_object(*id) {
                    Ok(Object::Null) | Err(_) => {}
                    Ok(_) => return Some(value.clone()),
                },
                Object::Null => {}
                _ => return Some(value.clone()),
            }
        }
        current = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

/// Copy inherited attributes onto the page itself.
fn materialize_inherited(doc: &mut Document, page: ObjectId) -> Result<()> {
    let values: Vec<(&[u8], Object)> = INHERITABLE
        .iter()
        .filter_map(|key| inherited(doc, page, key).map(|v| (*key, v)))
        .collect();
    let dict = doc.get_dictionary_mut(page)?;
    for (key, value) in values {
        if !dict.has(key) {
            dict.set(key.to_vec(), value);
        }
    }
    Ok(())
}

fn collect_tree_nodes(
    doc: &Document,
    node: ObjectId,
    out: &mut Vec<ObjectId>,
    seen: &mut HashSet<ObjectId>,
) {
    if !seen.insert(node) {
        return;
    }
    let Ok(dict) = doc.get_dictionary(node) else {
        return;
    };
    let is_pages =
        matches!(dict.get(b"Type"), Ok(Object::Name(n)) if n == b"Pages") || dict.has(b"Kids");
    if !is_pages {
        return;
    }
    out.push(node);
    if let Ok(kids) = dict.get(b"Kids").and_then(Object::as_array) {
        for kid in kids {
            if let Ok(id) = kid.as_reference() {
                collect_tree_nodes(doc, id, out, seen);
            }
        }
    }
}

fn null_dangling_references(doc: &mut Document) {
    let existing: HashSet<ObjectId> = doc.objects.keys().copied().collect();
    doc.traverse_objects(|obj| {
        if let Object::Reference(id) = obj {
            if !existing.contains(id) {
                *obj = Object::Null;
            }
        }
    });
}

fn number(doc: &Document, obj: &Object) -> Option<f64> {
    let obj = match obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

fn box_size(doc: &Document, obj: &Object) -> Option<(f64, f64)> {
    let obj = match obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    let arr = obj.as_array().ok()?;
    if arr.len() != 4 {
        return None;
    }
    let v: Vec<f64> = arr.iter().filter_map(|o| number(doc, o)).collect();
    if v.len() != 4 {
        return None;
    }
    // PDF reals are often stored as f32; round away the float noise.
    let round = |x: f64| (x * 1000.0).round() / 1000.0;
    Some((round((v[2] - v[0]).abs()), round((v[3] - v[1]).abs())))
}
