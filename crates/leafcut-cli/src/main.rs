//! `leafcut`: command-line front end for leafcut-core.

mod cli;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use leafcut_core::{format_pdf_date, Error, Metadata, Pdf, SplitMode};

use cli::{Cli, Command, OutputArgs};

/// Process exit codes (also documented in `--help`).
mod exit {
    pub const ERROR: u8 = 1;
    pub const USAGE: u8 = 2;
    pub const ENCRYPTED: u8 = 3;
    pub const OUTPUT_EXISTS: u8 = 4;
}

/// An error plus the exit code it maps to.
#[derive(Debug)]
struct Failure {
    code: u8,
    message: String,
}

impl Failure {
    fn usage(message: impl Into<String>) -> Self {
        Failure {
            code: exit::USAGE,
            message: message.into(),
        }
    }

    fn exists(path: &Path) -> Self {
        Failure {
            code: exit::OUTPUT_EXISTS,
            message: format!(
                "'{}' already exists; use --force to overwrite it",
                path.display()
            ),
        }
    }
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        let code = match &err {
            Error::Encrypted => exit::ENCRYPTED,
            Error::InvalidRange { .. }
            | Error::PageOutOfRange { .. }
            | Error::EmptySelection
            | Error::WouldDeleteAllPages
            | Error::InvalidOrder(_)
            | Error::InvalidRotation(_)
            | Error::NothingToMerge => exit::USAGE,
            _ => exit::ERROR,
        };
        Failure {
            code,
            message: err.to_string(),
        }
    }
}

type CliResult<T = ()> = Result<T, Failure>;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("leafcut: error: {}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}

/// Print a status line unless `--quiet`.
macro_rules! say {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet {
            println!($($arg)*);
        }
    };
}

fn run(cli: Cli) -> CliResult {
    let q = cli.quiet;
    match cli.command {
        Command::Merge {
            inputs,
            output,
            force,
        } => {
            for input in &inputs {
                if same_file(input, &output) {
                    return Err(Failure::usage(format!(
                        "the output '{}' is also an input; choose another output file",
                        output.display()
                    )));
                }
            }
            check_writable(&output, force)?;
            let mut merged = leafcut_core::merge_files(&inputs)?;
            let pages = merged.page_count();
            merged.save(&output)?;
            say!(
                q,
                "Merged {} files ({} pages) into {}",
                inputs.len(),
                pages,
                output.display()
            );
        }

        Command::Split {
            input,
            ranges,
            every,
            out_dir,
            prefix,
            force,
        } => {
            let pdf = Pdf::open(&input)?;
            let mode = match (ranges, every) {
                (Some(r), _) => SplitMode::Ranges(r),
                (None, Some(n)) => SplitMode::Chunks(n),
                (None, None) => SplitMode::EveryPage,
            };
            let plan = leafcut_core::plan_split(&mode, pdf.page_count())?;
            let dir = out_dir.unwrap_or_else(|| parent_dir(&input));
            let prefix = prefix.unwrap_or_else(|| file_stem(&input));
            let targets: Vec<PathBuf> = plan
                .iter()
                .map(|part| dir.join(format!("{prefix}-{}.pdf", part.label)))
                .collect();
            for target in &targets {
                if same_file(target, &input) {
                    return Err(Failure::usage(format!(
                        "split would overwrite the input '{}'; use another --prefix or --out-dir",
                        input.display()
                    )));
                }
                check_writable(target, force)?;
            }
            fs::create_dir_all(&dir).map_err(|e| Error::Io {
                path: dir.clone(),
                source: e,
            })?;
            for (part, target) in plan.iter().zip(&targets) {
                let mut doc = pdf.extract(&part.pages)?;
                doc.save(target)?;
                say!(
                    q,
                    "{}  ({} page{})",
                    target.display(),
                    part.pages.len(),
                    plural(part.pages.len())
                );
            }
            say!(q, "Split {} into {} files", input.display(), targets.len());
        }

        Command::Extract { input, pages, out } => {
            let target = output_path(&input, &out, "extract")?;
            let pdf = Pdf::open(&input)?;
            let mut doc = pdf.extract_ranges(&pages)?;
            let n = doc.page_count();
            doc.save(&target)?;
            say!(
                q,
                "Wrote {} page{} to {}",
                n,
                plural(n as usize),
                target.display()
            );
        }

        Command::Delete { input, pages, out } => {
            let target = output_path(&input, &out, "deleted")?;
            let mut pdf = Pdf::open(&input)?;
            let before = pdf.page_count();
            let list = pages.resolve_set(before)?;
            pdf.delete_pages(&list)?;
            let after = pdf.page_count();
            pdf.save(&target)?;
            say!(
                q,
                "Deleted {} page{} ({} -> {}), wrote {}",
                before - after,
                plural((before - after) as usize),
                before,
                after,
                target.display()
            );
        }

        Command::Rotate {
            input,
            angle,
            pages,
            out,
        } => {
            let target = output_path(&input, &out, "rotated")?;
            let mut pdf = Pdf::open(&input)?;
            let list = match &pages {
                Some(r) => Some(r.resolve_set(pdf.page_count())?),
                None => None,
            };
            pdf.rotate(list.as_deref(), angle.rotation())?;
            let count = list.as_ref().map_or(pdf.page_count() as usize, Vec::len);
            pdf.save(&target)?;
            say!(
                q,
                "Rotated {} page{} by {} degrees, wrote {}",
                count,
                plural(count),
                angle.rotation().degrees(),
                target.display()
            );
        }

        Command::Reorder {
            input,
            order,
            reverse,
            out,
        } => {
            let target = output_path(&input, &out, "reordered")?;
            let mut pdf = Pdf::open(&input)?;
            let n = pdf.page_count();
            let list: Vec<u32> = match order {
                Some(order) if !reverse => order.resolve(n)?,
                _ => (1..=n).rev().collect(),
            };
            pdf.reorder(&list)?;
            pdf.save(&target)?;
            say!(q, "Reordered {} pages, wrote {}", n, target.display());
        }

        Command::Compress { input, out } => {
            let target = output_path(&input, &out, "compressed")?;
            let before = file_size(&input);
            let mut pdf = Pdf::open(&input)?;
            let report = pdf.compress();
            pdf.save(&target)?;
            let after = file_size(&target);
            say!(
                q,
                "{} -> {}: {} -> {} ({})",
                input.display(),
                target.display(),
                human_size(before),
                human_size(after),
                change(before, after)
            );
            say!(
                q,
                "  removed {} unused object{}, compressed {} stream{}, recompressed {}",
                report.objects_removed,
                plural(report.objects_removed),
                report.streams_compressed,
                plural(report.streams_compressed),
                report.streams_recompressed
            );
            if after >= before && !q {
                println!("  note: no size reduction was possible without touching images");
            }
        }

        Command::Info {
            inputs,
            pages,
            json,
        } => {
            let mut json_items = Vec::new();
            for (i, input) in inputs.iter().enumerate() {
                let pdf = Pdf::open(input)?;
                let size = file_size(input);
                if json {
                    json_items.push(info_json(input, size, &pdf));
                } else {
                    if i > 0 {
                        println!();
                    }
                    print!("{}", info_text(input, size, &pdf, pages));
                }
            }
            if json {
                if json_items.len() == 1 {
                    println!("{}", json_items.remove(0));
                } else {
                    println!("[{}]", json_items.join(","));
                }
            }
        }

        Command::Meta {
            input,
            fields,
            clear,
            out,
        } => {
            let update = Metadata {
                title: fields.title,
                author: fields.author,
                subject: fields.subject,
                keywords: fields.keywords,
                creator: fields.creator,
                producer: fields.producer,
                ..Default::default()
            };
            if !clear && update.is_empty() {
                return Err(Failure::usage(
                    "nothing to do: give at least one field (e.g. --title) or --clear; \
                     use 'leafcut info' to view metadata",
                ));
            }
            let target = output_path(&input, &out, "meta")?;
            let mut pdf = Pdf::open(&input)?;
            if clear {
                pdf.clear_metadata()?;
            } else {
                pdf.set_metadata(&update)?;
            }
            pdf.save(&target)?;
            say!(
                q,
                "{} metadata, wrote {}",
                if clear { "Cleared" } else { "Updated" },
                target.display()
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- paths

/// Decide where a single-output command writes, enforcing the
/// never-overwrite rules.
fn output_path(input: &Path, out: &OutputArgs, suffix: &str) -> CliResult<PathBuf> {
    if out.in_place {
        if !input.exists() {
            return Err(Error::Io {
                path: input.to_path_buf(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            }
            .into());
        }
        return Ok(input.to_path_buf());
    }
    let target = match &out.output {
        Some(path) => path.clone(),
        None => parent_dir(input).join(format!("{}-{suffix}.pdf", file_stem(input))),
    };
    if same_file(&target, input) {
        return Err(Failure::usage(format!(
            "refusing to overwrite the input file '{}'; use --in-place if you really mean it",
            input.display()
        )));
    }
    check_writable(&target, out.force)?;
    Ok(target)
}

fn check_writable(target: &Path, force: bool) -> CliResult {
    if target.exists() && !force {
        return Err(Failure::exists(target));
    }
    if target.is_dir() {
        return Err(Failure::usage(format!(
            "'{}' is a directory",
            target.display()
        )));
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Directory of `path`; empty (meaning the current directory) for bare file names.
fn parent_dir(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into())
}

// ---------------------------------------------------------------- formatting

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
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

fn change(before: u64, after: u64) -> String {
    if before == 0 {
        return "n/a".into();
    }
    let pct = (after as f64 - before as f64) / before as f64 * 100.0;
    format!("{pct:+.1}%")
}

fn info_text(path: &Path, size: u64, pdf: &Pdf, per_page: bool) -> String {
    let info = pdf.info();
    let mut s = String::new();
    let _ = writeln!(s, "File:      {}", path.display());
    let _ = writeln!(s, "Size:      {} ({} bytes)", human_size(size), size);
    let _ = writeln!(s, "Version:   PDF {}", info.version);
    let _ = writeln!(s, "Pages:     {}", info.page_count);

    let mut groups: Vec<(String, usize)> = Vec::new();
    for page in &info.pages {
        let label = size_label(page);
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, n)) => *n += 1,
            None => groups.push((label, 1)),
        }
    }
    for (i, (label, n)) in groups.iter().enumerate() {
        let head = if i == 0 { "Page size:" } else { "" };
        let _ = writeln!(s, "{head:<11}{label} x{n}");
    }
    let _ = writeln!(s, "Bookmarks: {}", yes_no(info.has_outlines));
    let _ = writeln!(s, "Forms:     {}", yes_no(info.has_forms));

    let m = &info.metadata;
    let fields = [
        ("Title", &m.title),
        ("Author", &m.author),
        ("Subject", &m.subject),
        ("Keywords", &m.keywords),
        ("Creator", &m.creator),
        ("Producer", &m.producer),
    ];
    for (name, value) in fields {
        if let Some(v) = value {
            let _ = writeln!(s, "{:<11}{}", format!("{name}:"), v);
        }
    }
    if let Some(d) = &m.creation_date {
        let _ = writeln!(s, "Created:   {}", format_pdf_date(d));
    }
    if let Some(d) = &m.mod_date {
        let _ = writeln!(s, "Modified:  {}", format_pdf_date(d));
    }
    if m.is_empty() {
        let _ = writeln!(s, "Metadata:  (none)");
    }

    if per_page {
        let _ = writeln!(s);
        for page in &info.pages {
            let _ = writeln!(s, "  page {:>4}: {}", page.number, size_label(page));
        }
    }
    s
}

fn size_label(page: &leafcut_core::PageInfo) -> String {
    let mut label = format!(
        "{:.0} x {:.0} pt ({:.0} x {:.0} mm",
        page.width_pt,
        page.height_pt,
        page.width_mm(),
        page.height_mm()
    );
    if let Some(name) = page.paper_name() {
        let _ = write!(label, ", {name}");
    }
    label.push(')');
    if page.rotation != 0 {
        let _ = write!(label, ", rotated {}°", page.rotation);
    }
    label
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn info_json(path: &Path, size: u64, pdf: &Pdf) -> String {
    let info = pdf.info();
    let mut s = String::from("{");
    let _ = write!(
        s,
        "\"file\":{},\"size\":{},\"version\":{},\"pages\":{},\"bookmarks\":{},\"forms\":{},",
        json_str(&path.display().to_string()),
        size,
        json_str(&info.version),
        info.page_count,
        info.has_outlines,
        info.has_forms
    );
    s.push_str("\"metadata\":{");
    let entries: Vec<String> = info
        .metadata
        .entries()
        .iter()
        .filter_map(|(k, v)| v.map(|v| format!("{}:{}", json_str(k), json_str(v))))
        .collect();
    s.push_str(&entries.join(","));
    s.push_str("},\"page_sizes\":[");
    let pages: Vec<String> = info
        .pages
        .iter()
        .map(|p| {
            format!(
                "{{\"page\":{},\"width_pt\":{},\"height_pt\":{},\"rotation\":{}}}",
                p.number, p.width_pt, p.height_pt, p.rotation
            )
        })
        .collect();
    s.push_str(&pages.join(","));
    s.push_str("]}");
    s
}

fn json_str(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KiB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MiB");
    }

    #[test]
    fn json_escaping() {
        assert_eq!(json_str("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        assert_eq!(json_str("Öztürk"), "\"Öztürk\"");
        assert_eq!(json_str("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn change_percent() {
        assert_eq!(change(200, 100), "-50.0%");
        assert_eq!(change(100, 100), "+0.0%");
    }
}
