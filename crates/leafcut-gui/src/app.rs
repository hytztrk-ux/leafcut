//! The leafcut window. This file only does UI: collecting input, starting
//! jobs and showing results. All PDF work happens in `leafcut-core` on a
//! worker thread (see `worker.rs`).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui::{self, Color32, RichText};
use leafcut_core::{format_pdf_date, Error, Metadata, PageRanges, Rotation, SplitMode};

use crate::i18n::{fill, Lang, Strings};
use crate::worker::{self, Job, JobError, Msg, Outcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Merge,
    Split,
    Extract,
    Delete,
    Rotate,
    Reorder,
    Compress,
    Info,
    Metadata,
}

impl Op {
    const ALL: [Op; 9] = [
        Op::Merge,
        Op::Split,
        Op::Extract,
        Op::Delete,
        Op::Rotate,
        Op::Reorder,
        Op::Compress,
        Op::Info,
        Op::Metadata,
    ];

    fn label(self, s: &Strings) -> &'static str {
        match self {
            Op::Merge => s.op_merge,
            Op::Split => s.op_split,
            Op::Extract => s.op_extract,
            Op::Delete => s.op_delete,
            Op::Rotate => s.op_rotate,
            Op::Reorder => s.op_reorder,
            Op::Compress => s.op_compress,
            Op::Info => s.op_info,
            Op::Metadata => s.op_metadata,
        }
    }

    fn help(self, s: &Strings) -> &'static str {
        match self {
            Op::Merge => s.help_merge,
            Op::Split => s.help_split,
            Op::Extract => s.help_extract,
            Op::Delete => s.help_delete,
            Op::Rotate => s.help_rotate,
            Op::Reorder => s.help_reorder,
            Op::Compress => s.help_compress,
            Op::Info => s.help_info,
            Op::Metadata => s.help_metadata,
        }
    }

    /// File-name suffix for the suggested output.
    fn suffix(self) -> &'static str {
        match self {
            Op::Merge => "merged",
            Op::Split => "split",
            Op::Extract => "extract",
            Op::Delete => "deleted",
            Op::Rotate => "rotated",
            Op::Reorder => "reordered",
            Op::Compress => "compressed",
            Op::Info => "",
            Op::Metadata => "meta",
        }
    }

    fn output_kind(self) -> OutputKind {
        match self {
            Op::Info => OutputKind::None,
            Op::Split => OutputKind::Folder,
            _ => OutputKind::File,
        }
    }
}

#[derive(PartialEq, Eq)]
enum OutputKind {
    None,
    File,
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitChoice {
    EveryPage,
    Ranges,
    Chunks,
}

struct FileEntry {
    path: PathBuf,
    pages: Option<Result<u32, String>>,
}

enum Status {
    Ok(String),
    Err(String),
}

#[derive(Default)]
struct MetaForm {
    title: String,
    author: String,
    subject: String,
    keywords: String,
}

pub struct LeafcutApp {
    lang: Lang,
    files: Vec<FileEntry>,
    selected: usize,
    op: Op,

    split_choice: SplitChoice,
    split_ranges: String,
    split_chunk: u32,
    pages: String,
    angle: i64,
    order: String,
    reverse: bool,
    meta: MetaForm,
    meta_clear: bool,

    output: String,
    output_auto: bool,
    overwrite: bool,

    running: bool,
    status: Option<Status>,
    info: Option<(Box<leafcut_core::PdfInfo>, u64)>,

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl LeafcutApp {
    /// Create the app; `initial_files` (e.g. from the command line or
    /// "Open with…") are added to the list right away.
    pub fn new(cc: &eframe::CreationContext<'_>, initial_files: Vec<PathBuf>) -> Self {
        let (tx, rx) = channel();
        let mut app = LeafcutApp {
            lang: Lang::from_system(),
            files: Vec::new(),
            selected: 0,
            op: Op::Merge,
            split_choice: SplitChoice::EveryPage,
            split_ranges: String::new(),
            split_chunk: 1,
            pages: String::new(),
            angle: 90,
            order: String::new(),
            reverse: false,
            meta: MetaForm::default(),
            meta_clear: false,
            output: String::new(),
            output_auto: true,
            overwrite: false,
            running: false,
            status: None,
            info: None,
            tx,
            rx,
        };
        app.add_files(&cc.egui_ctx, initial_files);
        app
    }

    fn s(&self) -> &'static Strings {
        self.lang.strings()
    }

    fn add_files(&mut self, ctx: &egui::Context, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            let path = std::path::absolute(&path).unwrap_or(path);
            let is_pdf = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
            if !is_pdf || self.files.iter().any(|f| f.path == path) {
                continue;
            }
            let ctx = ctx.clone();
            worker::spawn_page_count(path.clone(), self.tx.clone(), move || ctx.request_repaint());
            self.files.push(FileEntry { path, pages: None });
        }
    }

    fn selected_path(&self) -> Option<&Path> {
        self.files.get(self.selected).map(|f| f.path.as_path())
    }

    /// Suggested output for the current operation and selection.
    fn suggested_output(&self) -> String {
        let Some(first) = self.files.get(if self.op == Op::Merge {
            0
        } else {
            self.selected
        }) else {
            return String::new();
        };
        let dir = match first.path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let stem = first
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".into());
        match self.op.output_kind() {
            OutputKind::None => String::new(),
            OutputKind::Folder => dir.join(format!("{stem}-parts")).display().to_string(),
            OutputKind::File if self.op == Op::Merge => {
                dir.join("merged.pdf").display().to_string()
            }
            OutputKind::File => dir
                .join(format!("{stem}-{}.pdf", self.op.suffix()))
                .display()
                .to_string(),
        }
    }

    fn poll_worker(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::PageCount(path, result) => {
                    if let Some(f) = self.files.iter_mut().find(|f| f.path == path) {
                        f.pages = Some(result);
                    }
                }
                Msg::Finished(result) => {
                    self.running = false;
                    self.handle_result(result);
                }
            }
        }
    }

    fn handle_result(&mut self, result: Result<Outcome, JobError>) {
        let s = self.s();
        match result {
            Ok(Outcome::Saved { path, pages }) => {
                self.status = Some(Status::Ok(fill(
                    s.done_saved,
                    &[
                        ("pages", &pages.to_string()),
                        ("path", &path.display().to_string()),
                    ],
                )));
            }
            Ok(Outcome::Split { dir, count }) => {
                self.status = Some(Status::Ok(fill(
                    s.done_split,
                    &[
                        ("count", &count.to_string()),
                        ("path", &dir.display().to_string()),
                    ],
                )));
            }
            Ok(Outcome::Compressed {
                path,
                before,
                after,
            }) => {
                let change = if before > 0 {
                    format!(
                        "{:+.1}%",
                        (after as f64 - before as f64) / before as f64 * 100.0
                    )
                } else {
                    "n/a".into()
                };
                self.status = Some(Status::Ok(fill(
                    s.done_compress,
                    &[
                        ("before", &human_size(before)),
                        ("after", &human_size(after)),
                        ("change", &change),
                        ("path", &path.display().to_string()),
                    ],
                )));
            }
            Ok(Outcome::Info { info, size }) => {
                self.status = None;
                self.info = Some((info, size));
            }
            Ok(Outcome::Metadata(m)) => {
                self.meta = MetaForm {
                    title: m.title.unwrap_or_default(),
                    author: m.author.unwrap_or_default(),
                    subject: m.subject.unwrap_or_default(),
                    keywords: m.keywords.unwrap_or_default(),
                };
                self.status = None;
            }
            Err(e) => {
                let text = match e {
                    JobError::Core(Error::Encrypted) => s.encrypted.to_string(),
                    JobError::Core(e) => e.to_string(),
                    JobError::OutputExists(p) => {
                        fill(s.output_exists, &[("path", &p.display().to_string())])
                    }
                    JobError::OutputIsInput => s.output_is_input.to_string(),
                };
                self.status = Some(Status::Err(format!("{}: {text}", s.error_prefix)));
            }
        }
    }

    fn start(&mut self, ctx: &egui::Context, job: Job) {
        self.running = true;
        self.status = None;
        let ctx = ctx.clone();
        worker::spawn(job, self.tx.clone(), move || ctx.request_repaint());
    }

    /// Validate the form and build a job. Errors are user-facing messages.
    fn build_job(&self) -> Result<Job, String> {
        let s = self.s();
        let input = self.selected_path().ok_or(s.no_files)?.to_path_buf();
        let inputs: Vec<PathBuf> = self.files.iter().map(|f| f.path.clone()).collect();
        let ranges = |text: &str| PageRanges::parse(text).map_err(|e| e.to_string());

        let output = PathBuf::from(self.output.trim());
        if self.op.output_kind() != OutputKind::None {
            if self.output.trim().is_empty() {
                return Err(s.no_output.into());
            }
            if inputs.iter().any(|i| same_file(i, &output)) {
                return Err(s.output_is_input.into());
            }
            if self.op.output_kind() == OutputKind::File && output.exists() && !self.overwrite {
                return Err(fill(
                    s.output_exists,
                    &[("path", &output.display().to_string())],
                ));
            }
        }

        let job = match self.op {
            Op::Merge => {
                if inputs.len() < 2 {
                    return Err(s.need_two_files.into());
                }
                Job::Merge { inputs, output }
            }
            Op::Split => Job::Split {
                input,
                mode: match self.split_choice {
                    SplitChoice::EveryPage => SplitMode::EveryPage,
                    SplitChoice::Ranges => SplitMode::Ranges(ranges(&self.split_ranges)?),
                    SplitChoice::Chunks => SplitMode::Chunks(self.split_chunk.max(1)),
                },
                dir: output,
                overwrite: self.overwrite,
            },
            Op::Extract => Job::Extract {
                input,
                pages: ranges(&self.pages)?,
                output,
            },
            Op::Delete => Job::Delete {
                input,
                pages: ranges(&self.pages)?,
                output,
            },
            Op::Rotate => Job::Rotate {
                input,
                pages: if self.pages.trim().is_empty() {
                    None
                } else {
                    Some(ranges(&self.pages)?)
                },
                rotation: Rotation::from_degrees(self.angle).map_err(|e| e.to_string())?,
                output,
            },
            Op::Reorder => Job::Reorder {
                input,
                order: if self.reverse {
                    None
                } else {
                    Some(ranges(&self.order)?)
                },
                output,
            },
            Op::Compress => Job::Compress { input, output },
            Op::Info => Job::Info { input },
            Op::Metadata => {
                let update = if self.meta_clear {
                    None
                } else {
                    Some(Metadata {
                        title: Some(self.meta.title.trim().to_string()),
                        author: Some(self.meta.author.trim().to_string()),
                        subject: Some(self.meta.subject.trim().to_string()),
                        keywords: Some(self.meta.keywords.trim().to_string()),
                        ..Default::default()
                    })
                };
                Job::SetMetadata {
                    input,
                    update,
                    output,
                }
            }
        };
        Ok(job)
    }

    // ------------------------------------------------------------ UI parts

    fn ui_top(&mut self, ui: &mut egui::Ui) {
        let s = self.s();
        ui.horizontal(|ui| {
            ui.heading(RichText::new("leafcut").strong());
            ui.label(RichText::new(s.tagline).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("lang")
                    .selected_text(self.lang.native_name())
                    .show_ui(ui, |ui| {
                        for lang in [Lang::En, Lang::Tr] {
                            ui.selectable_value(&mut self.lang, lang, lang.native_name());
                        }
                    });
                ui.label(format!("{}:", s.language));
            });
        });
    }

    fn ui_files(&mut self, ui: &mut egui::Ui) {
        let s = self.s();
        let ctx = ui.ctx().clone();
        ui.heading(s.files);
        ui.horizontal(|ui| {
            if ui.button(s.add_files).clicked() {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter(s.pdf_filter, &["pdf", "PDF"])
                    .pick_files()
                {
                    self.add_files(&ctx, paths);
                }
            }
            if ui
                .add_enabled(!self.files.is_empty(), egui::Button::new(s.clear_list))
                .clicked()
            {
                self.files.clear();
                self.selected = 0;
                self.info = None;
            }
        });
        ui.separator();

        if self.files.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(s.drop_hint).weak());
            });
            return;
        }

        let mut action: Option<(usize, i32)> = None; // (index, -1 up / +1 down / 0 remove)
        let count = self.files.len();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, file) in self.files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let name = file
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let detail = match &file.pages {
                            None => s.reading.to_string(),
                            Some(Ok(n)) => format!("{n} {}", s.pages_suffix),
                            Some(Err(_)) => "!".to_string(),
                        };
                        if ui
                            .add_enabled(
                                i > 0,
                                egui::Button::new(RichText::new("↑").monospace()).small(),
                            )
                            .on_hover_text(s.move_up)
                            .clicked()
                        {
                            action = Some((i, -1));
                        }
                        if ui
                            .add_enabled(
                                i + 1 < count,
                                egui::Button::new(RichText::new("↓").monospace()).small(),
                            )
                            .on_hover_text(s.move_down)
                            .clicked()
                        {
                            action = Some((i, 1));
                        }
                        if ui
                            .add(egui::Button::new("×").small())
                            .on_hover_text(s.remove)
                            .clicked()
                        {
                            action = Some((i, 0));
                        }
                        let label = format!("{}. {name}  ({detail})", i + 1);
                        let mut resp = ui.selectable_label(self.selected == i, label);
                        resp = resp.on_hover_text(match &file.pages {
                            Some(Err(e)) => format!("{}\n{e}", file.path.display()),
                            _ => file.path.display().to_string(),
                        });
                        if resp.clicked() {
                            self.selected = i;
                            self.info = None;
                        }
                    });
                }
            });

        if let Some((i, dir)) = action {
            match dir {
                0 => {
                    self.files.remove(i);
                    if self.selected >= self.files.len() {
                        self.selected = self.files.len().saturating_sub(1);
                    }
                }
                -1 => {
                    self.files.swap(i, i - 1);
                    if self.selected == i {
                        self.selected = i - 1;
                    } else if self.selected == i - 1 {
                        self.selected = i;
                    }
                }
                _ => {
                    self.files.swap(i, i + 1);
                    if self.selected == i {
                        self.selected = i + 1;
                    } else if self.selected == i + 1 {
                        self.selected = i;
                    }
                }
            }
            self.info = None;
        }
    }

    fn ui_operation(&mut self, ui: &mut egui::Ui) {
        let s = self.s();
        let ctx = ui.ctx().clone();
        ui.heading(s.operation);
        ui.horizontal_wrapped(|ui| {
            for op in Op::ALL {
                if ui.selectable_value(&mut self.op, op, op.label(s)).changed() {
                    self.status = None;
                    self.info = None;
                }
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new(self.op.help(s)).weak());
        if self.op != Op::Merge {
            if let Some(path) = self.selected_path() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ui.label(format!("{} {name}", s.works_on_selected));
            }
        }
        ui.separator();

        let hint_width = 260.0;
        if !matches!(self.op, Op::Merge | Op::Compress | Op::Info) {
            ui.label(RichText::new(s.parameters).strong());
        }
        match self.op {
            Op::Merge | Op::Compress | Op::Info => {}
            Op::Split => {
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(
                        &mut self.split_choice,
                        SplitChoice::EveryPage,
                        s.split_every_page,
                    );
                    ui.radio_value(&mut self.split_choice, SplitChoice::Ranges, s.split_ranges);
                    ui.radio_value(&mut self.split_choice, SplitChoice::Chunks, s.split_chunks);
                });
                match self.split_choice {
                    SplitChoice::EveryPage => {}
                    SplitChoice::Ranges => {
                        ui.horizontal(|ui| {
                            ui.label(s.pages);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.split_ranges)
                                    .hint_text(s.pages_hint)
                                    .desired_width(hint_width),
                            );
                        });
                    }
                    SplitChoice::Chunks => {
                        ui.horizontal(|ui| {
                            ui.label(s.pages_per_file);
                            ui.add(egui::DragValue::new(&mut self.split_chunk).range(1..=100_000));
                        });
                    }
                }
            }
            Op::Extract | Op::Delete => {
                ui.horizontal(|ui| {
                    ui.label(s.pages);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.pages)
                            .hint_text(s.pages_hint)
                            .desired_width(hint_width),
                    );
                });
            }
            Op::Rotate => {
                ui.horizontal(|ui| {
                    ui.label(s.angle);
                    for a in [90, 180, 270] {
                        ui.radio_value(&mut self.angle, a, format!("{a}°"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s.pages);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.pages)
                            .hint_text(format!("{} ({})", s.pages_hint, s.pages_all_hint))
                            .desired_width(hint_width),
                    );
                });
            }
            Op::Reorder => {
                ui.checkbox(&mut self.reverse, s.reverse);
                ui.add_enabled_ui(!self.reverse, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(s.new_order);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.order)
                                .hint_text("3,1,2,4-")
                                .desired_width(hint_width),
                        );
                    });
                });
            }
            Op::Metadata => {
                ui.checkbox(&mut self.meta_clear, s.clear_all_metadata);
                ui.add_enabled_ui(!self.meta_clear, |ui| {
                    egui::Grid::new("meta")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            for (label, value) in [
                                (s.title, &mut self.meta.title),
                                (s.author, &mut self.meta.author),
                                (s.subject, &mut self.meta.subject),
                                (s.keywords, &mut self.meta.keywords),
                            ] {
                                ui.label(label);
                                ui.add(egui::TextEdit::singleline(value).desired_width(320.0));
                                ui.end_row();
                            }
                        });
                    ui.horizontal(|ui| {
                        let can_load = self.selected_path().is_some() && !self.running;
                        if ui
                            .add_enabled(can_load, egui::Button::new(s.load_current))
                            .clicked()
                        {
                            if let Some(input) = self.selected_path().map(Path::to_path_buf) {
                                self.start(&ctx, Job::ReadMetadata { input });
                            }
                        }
                        ui.label(RichText::new(s.metadata_hint).weak());
                    });
                });
            }
        }
    }

    fn ui_output(&mut self, ui: &mut egui::Ui) {
        let s = self.s();
        let kind = self.op.output_kind();
        if kind == OutputKind::None {
            return;
        }
        if self.output_auto {
            self.output = self.suggested_output();
        }
        ui.separator();
        ui.heading(s.output);
        ui.horizontal(|ui| {
            ui.label(if kind == OutputKind::Folder {
                s.output_folder
            } else {
                s.output_file
            });
            let edit = ui.add(egui::TextEdit::singleline(&mut self.output).desired_width(360.0));
            if edit.changed() {
                self.output_auto = self.output.is_empty();
            }
            if ui.button(s.browse).clicked() {
                let start = PathBuf::from(&self.output);
                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = start.parent().filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                let picked = if kind == OutputKind::Folder {
                    dialog.pick_folder()
                } else {
                    if let Some(name) = start.file_name() {
                        dialog = dialog.set_file_name(name.to_string_lossy());
                    }
                    dialog.add_filter(s.pdf_filter, &["pdf"]).save_file()
                };
                if let Some(p) = picked {
                    self.output = p.display().to_string();
                    self.output_auto = false;
                }
            }
        });
        ui.checkbox(&mut self.overwrite, s.overwrite);
    }

    fn ui_run(&mut self, ui: &mut egui::Ui) {
        let s = self.s();
        let ctx = ui.ctx().clone();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let run = egui::Button::new(RichText::new(s.run).size(16.0))
                .min_size(egui::vec2(120.0, 32.0));
            if ui.add_enabled(!self.running, run).clicked() {
                match self.build_job() {
                    Ok(job) => {
                        self.info = None;
                        self.start(&ctx, job);
                    }
                    Err(msg) => {
                        self.status = Some(Status::Err(format!("{}: {msg}", s.error_prefix)))
                    }
                }
            }
            if self.running {
                ui.spinner();
                ui.label(s.working);
            }
        });
        ui.add_space(6.0);
        match &self.status {
            Some(Status::Ok(msg)) => {
                ui.add(
                    egui::Label::new(
                        RichText::new(msg.as_str()).color(Color32::from_rgb(0x2e, 0x9e, 0x4f)),
                    )
                    .wrap(),
                );
            }
            Some(Status::Err(msg)) => {
                ui.add(
                    egui::Label::new(
                        RichText::new(msg.as_str()).color(Color32::from_rgb(0xd0, 0x3b, 0x3b)),
                    )
                    .wrap(),
                );
            }
            None => {}
        }
    }

    fn ui_info(&self, ui: &mut egui::Ui) {
        let Some((info, size)) = &self.info else {
            return;
        };
        let s = self.s();
        ui.separator();
        egui::Grid::new("info")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(s.info_pages);
                ui.label(info.page_count.to_string());
                ui.end_row();
                ui.label(s.info_version);
                ui.label(&info.version);
                ui.end_row();
                ui.label(s.info_size);
                ui.label(human_size(*size));
                ui.end_row();

                let mut sizes: Vec<(String, usize)> = Vec::new();
                for p in &info.pages {
                    let mut label = format!("{:.0} × {:.0} mm", p.width_mm(), p.height_mm());
                    if let Some(name) = p.paper_name() {
                        label.push_str(&format!(" ({name})"));
                    }
                    if p.rotation != 0 {
                        label.push_str(&format!(", {}°", p.rotation));
                    }
                    match sizes.iter_mut().find(|(l, _)| *l == label) {
                        Some((_, n)) => *n += 1,
                        None => sizes.push((label, 1)),
                    }
                }
                for (i, (label, n)) in sizes.iter().enumerate() {
                    ui.label(if i == 0 { s.info_page_size } else { "" });
                    ui.label(format!("{label} ×{n}"));
                    ui.end_row();
                }

                let m = &info.metadata;
                let fields = [
                    (s.title, m.title.clone()),
                    (s.author, m.author.clone()),
                    (s.subject, m.subject.clone()),
                    (s.keywords, m.keywords.clone()),
                    ("Creator", m.creator.clone()),
                    ("Producer", m.producer.clone()),
                    (
                        "CreationDate",
                        m.creation_date.as_deref().map(format_pdf_date),
                    ),
                    ("ModDate", m.mod_date.as_deref().map(format_pdf_date)),
                ];
                if m.is_empty() {
                    ui.label(s.info_metadata);
                    ui.label(s.info_none);
                    ui.end_row();
                }
                for (label, value) in fields {
                    if let Some(v) = value {
                        ui.label(label);
                        ui.label(v);
                        ui.end_row();
                    }
                }
            });
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_files(ctx, dropped);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovering {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("drop_overlay"),
            ));
            let rect = ctx.content_rect();
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(160));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.s().drop_hint,
                egui::FontId::proportional(22.0),
                Color32::WHITE,
            );
        }
    }
}

impl eframe::App for LeafcutApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_worker();
        self.handle_drops(&ctx);

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.ui_top(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("bottom").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.s().offline_note).small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("leafcut v{}", env!("CARGO_PKG_VERSION")))
                            .small()
                            .weak(),
                    );
                });
            });
        });
        egui::Panel::left("files")
            .resizable(true)
            .default_size(340.0)
            .min_size(240.0)
            .show(ui, |ui| {
                ui.add_space(6.0);
                self.ui_files(ui);
            });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    self.ui_operation(ui);
                    self.ui_output(ui);
                    self.ui_run(ui);
                    self.ui_info(ui);
                });
        });
    }
}

/// True if both paths point at the same file (or are equal as written).
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
