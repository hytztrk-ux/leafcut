//! User-interface strings in English and Turkish.

/// UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Tr,
}

impl Lang {
    /// Pick the language from the operating-system locale: Turkish for
    /// `tr*` locales, English otherwise.
    pub fn from_system() -> Self {
        match sys_locale::get_locale() {
            Some(l) if l.to_ascii_lowercase().starts_with("tr") => Lang::Tr,
            _ => Lang::En,
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Tr => "Türkçe",
        }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::En => &EN,
            Lang::Tr => &TR,
        }
    }
}

/// Every piece of text the GUI shows. Placeholders in braces are replaced at
/// run time.
pub struct Strings {
    pub tagline: &'static str,
    pub language: &'static str,
    pub files: &'static str,
    pub add_files: &'static str,
    pub clear_list: &'static str,
    pub drop_hint: &'static str,
    pub pages_suffix: &'static str,
    pub reading: &'static str,
    pub move_up: &'static str,
    pub move_down: &'static str,
    pub remove: &'static str,
    pub operation: &'static str,
    pub op_merge: &'static str,
    pub op_split: &'static str,
    pub op_extract: &'static str,
    pub op_delete: &'static str,
    pub op_rotate: &'static str,
    pub op_reorder: &'static str,
    pub op_compress: &'static str,
    pub op_info: &'static str,
    pub op_metadata: &'static str,
    pub help_merge: &'static str,
    pub help_split: &'static str,
    pub help_extract: &'static str,
    pub help_delete: &'static str,
    pub help_rotate: &'static str,
    pub help_reorder: &'static str,
    pub help_compress: &'static str,
    pub help_info: &'static str,
    pub help_metadata: &'static str,
    pub works_on_selected: &'static str,
    pub parameters: &'static str,
    pub split_every_page: &'static str,
    pub split_ranges: &'static str,
    pub split_chunks: &'static str,
    pub pages_per_file: &'static str,
    pub pages: &'static str,
    pub pages_hint: &'static str,
    pub pages_all_hint: &'static str,
    pub angle: &'static str,
    pub new_order: &'static str,
    pub reverse: &'static str,
    pub title: &'static str,
    pub author: &'static str,
    pub subject: &'static str,
    pub keywords: &'static str,
    pub load_current: &'static str,
    pub clear_all_metadata: &'static str,
    pub metadata_hint: &'static str,
    pub output: &'static str,
    pub output_file: &'static str,
    pub output_folder: &'static str,
    pub browse: &'static str,
    pub overwrite: &'static str,
    pub run: &'static str,
    pub working: &'static str,
    pub no_files: &'static str,
    pub need_two_files: &'static str,
    pub no_output: &'static str,
    pub output_is_input: &'static str,
    pub output_exists: &'static str,
    pub done_saved: &'static str,
    pub done_split: &'static str,
    pub done_compress: &'static str,
    pub error_prefix: &'static str,
    pub pdf_filter: &'static str,
    pub offline_note: &'static str,
    pub info_pages: &'static str,
    pub info_version: &'static str,
    pub info_size: &'static str,
    pub info_page_size: &'static str,
    pub info_metadata: &'static str,
    pub info_none: &'static str,
    pub encrypted: &'static str,
}

pub const EN: Strings = Strings {
    tagline: "Offline PDF toolkit. Your files never leave this computer.",
    language: "Language",
    files: "Files",
    add_files: "Add PDFs…",
    clear_list: "Clear list",
    drop_hint: "Drop PDF files here, or click “Add PDFs…”.",
    pages_suffix: "pages",
    reading: "reading…",
    move_up: "Move up",
    move_down: "Move down",
    remove: "Remove from list",
    operation: "Operation",
    op_merge: "Merge",
    op_split: "Split",
    op_extract: "Extract pages",
    op_delete: "Delete pages",
    op_rotate: "Rotate",
    op_reorder: "Reorder",
    op_compress: "Compress",
    op_info: "Info",
    op_metadata: "Metadata",
    help_merge: "Combines all listed files into one PDF, in list order.",
    help_split: "Splits the selected file into several PDFs.",
    help_extract: "Writes only the chosen pages of the selected file to a new PDF.",
    help_delete: "Writes a copy of the selected file without the chosen pages.",
    help_rotate: "Rotates pages of the selected file clockwise.",
    help_reorder: "Writes the pages of the selected file in a new order.",
    help_compress: "Lossless optimisation: compresses streams and drops unused objects. Images are not re-encoded, so scans shrink little.",
    help_info: "Shows page count, page sizes and metadata of the selected file.",
    help_metadata: "Sets or removes title, author and other document properties.",
    works_on_selected: "Works on the selected file:",
    parameters: "Parameters",
    split_every_page: "Every page",
    split_ranges: "By ranges",
    split_chunks: "Every N pages",
    pages_per_file: "Pages per file",
    pages: "Pages",
    pages_hint: "e.g. 1-3,7,10-",
    pages_all_hint: "empty = all pages",
    angle: "Angle (clockwise)",
    new_order: "New order",
    reverse: "Reverse order",
    title: "Title",
    author: "Author",
    subject: "Subject",
    keywords: "Keywords",
    load_current: "Load current values",
    clear_all_metadata: "Remove all metadata",
    metadata_hint: "Empty fields are removed from the document.",
    output: "Output",
    output_file: "Output file",
    output_folder: "Output folder",
    browse: "Browse…",
    overwrite: "Overwrite existing files",
    run: "Run",
    working: "Working…",
    no_files: "Add at least one PDF first.",
    need_two_files: "Merging needs at least two files.",
    no_output: "Choose an output location.",
    output_is_input: "The output would overwrite an input file. Choose another name.",
    output_exists: "“{path}” already exists. Tick “Overwrite existing files” to replace it.",
    done_saved: "Done. Saved {pages} pages to {path}",
    done_split: "Done. Wrote {count} files to {path}",
    done_compress: "Done. {before} -> {after} ({change}). Saved to {path}",
    error_prefix: "Error",
    pdf_filter: "PDF files",
    offline_note: "100% offline · no upload",
    info_pages: "Pages",
    info_version: "PDF version",
    info_size: "File size",
    info_page_size: "Page size",
    info_metadata: "Metadata",
    info_none: "(none)",
    encrypted: "This PDF is password protected. Encrypted PDFs are not supported yet.",
};

pub const TR: Strings = Strings {
    tagline: "Çevrimdışı PDF araç takımı. Dosyalarınız bu bilgisayardan hiç çıkmaz.",
    language: "Dil",
    files: "Dosyalar",
    add_files: "PDF ekle…",
    clear_list: "Listeyi temizle",
    drop_hint: "PDF dosyalarını buraya sürükleyin ya da “PDF ekle…” düğmesine tıklayın.",
    pages_suffix: "sayfa",
    reading: "okunuyor…",
    move_up: "Yukarı taşı",
    move_down: "Aşağı taşı",
    remove: "Listeden çıkar",
    operation: "İşlem",
    op_merge: "Birleştir",
    op_split: "Böl",
    op_extract: "Sayfa çıkar",
    op_delete: "Sayfa sil",
    op_rotate: "Döndür",
    op_reorder: "Sırala",
    op_compress: "Sıkıştır",
    op_info: "Bilgi",
    op_metadata: "Üst veri",
    help_merge: "Listedeki tüm dosyaları, listedeki sırayla tek bir PDF'te birleştirir.",
    help_split: "Seçili dosyayı birden fazla PDF'e böler.",
    help_extract: "Seçili dosyanın yalnızca belirtilen sayfalarını yeni bir PDF'e yazar.",
    help_delete: "Seçili dosyanın, belirtilen sayfalar çıkarılmış bir kopyasını yazar.",
    help_rotate: "Seçili dosyanın sayfalarını saat yönünde döndürür.",
    help_reorder: "Seçili dosyanın sayfalarını yeni bir sırayla yazar.",
    help_compress: "Kayıpsız iyileştirme: akışları sıkıştırır, kullanılmayan nesneleri atar. Görseller yeniden kodlanmaz; taranmış belgeler pek küçülmez.",
    help_info: "Seçili dosyanın sayfa sayısını, sayfa boyutlarını ve üst verisini gösterir.",
    help_metadata: "Başlık, yazar ve diğer belge özelliklerini ayarlar ya da siler.",
    works_on_selected: "Seçili dosya üzerinde çalışır:",
    parameters: "Ayarlar",
    split_every_page: "Her sayfa ayrı",
    split_ranges: "Aralıklara göre",
    split_chunks: "Her N sayfada bir",
    pages_per_file: "Dosya başına sayfa",
    pages: "Sayfalar",
    pages_hint: "örn. 1-3,7,10-",
    pages_all_hint: "boş = tüm sayfalar",
    angle: "Açı (saat yönünde)",
    new_order: "Yeni sıra",
    reverse: "Sırayı ters çevir",
    title: "Başlık",
    author: "Yazar",
    subject: "Konu",
    keywords: "Anahtar kelimeler",
    load_current: "Mevcut değerleri yükle",
    clear_all_metadata: "Tüm üst veriyi sil",
    metadata_hint: "Boş bırakılan alanlar belgeden silinir.",
    output: "Çıktı",
    output_file: "Çıktı dosyası",
    output_folder: "Çıktı klasörü",
    browse: "Gözat…",
    overwrite: "Var olan dosyaların üzerine yaz",
    run: "Çalıştır",
    working: "Çalışıyor…",
    no_files: "Önce en az bir PDF ekleyin.",
    need_two_files: "Birleştirmek için en az iki dosya gerekir.",
    no_output: "Bir çıktı konumu seçin.",
    output_is_input: "Çıktı, girdi dosyalarından birinin üzerine yazacak. Başka bir ad seçin.",
    output_exists: "“{path}” zaten var. Değiştirmek için “Var olan dosyaların üzerine yaz” kutusunu işaretleyin.",
    done_saved: "Tamamlandı. {pages} sayfa kaydedildi: {path}",
    done_split: "Tamamlandı. {count} dosya yazıldı: {path}",
    done_compress: "Tamamlandı. {before} -> {after} ({change}). Kaydedildi: {path}",
    error_prefix: "Hata",
    pdf_filter: "PDF dosyaları",
    offline_note: "%100 çevrimdışı · yükleme yok",
    info_pages: "Sayfa sayısı",
    info_version: "PDF sürümü",
    info_size: "Dosya boyutu",
    info_page_size: "Sayfa boyutu",
    info_metadata: "Üst veri",
    info_none: "(yok)",
    encrypted: "Bu PDF parola korumalı. Şifreli PDF'ler henüz desteklenmiyor.",
};

/// Replace `{key}` placeholders.
pub fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in pairs {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_replaces_placeholders() {
        assert_eq!(
            fill(
                "{count} files in {path}",
                &[("count", "3"), ("path", "/tmp")]
            ),
            "3 files in /tmp"
        );
        assert_eq!(fill("no placeholders", &[("x", "y")]), "no placeholders");
    }

    #[test]
    fn templates_keep_their_placeholders() {
        for s in [&EN, &TR] {
            assert!(s.output_exists.contains("{path}"));
            assert!(s.done_saved.contains("{pages}") && s.done_saved.contains("{path}"));
            assert!(s.done_split.contains("{count}") && s.done_split.contains("{path}"));
            for key in ["{before}", "{after}", "{change}", "{path}"] {
                assert!(s.done_compress.contains(key), "{key}");
            }
        }
    }

    #[test]
    fn languages_differ() {
        assert_ne!(Lang::En.strings().run, Lang::Tr.strings().run);
        assert_eq!(Lang::Tr.native_name(), "Türkçe");
    }
}
