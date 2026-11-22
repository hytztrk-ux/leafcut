//! Background jobs. Every PDF operation runs on a worker thread so the window
//! never freezes; results come back over a channel.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use leafcut_core::{Error, Metadata, PageRanges, Pdf, PdfInfo, Rotation, SplitMode};

/// A unit of work for the worker thread.
#[derive(Debug, Clone)]
pub enum Job {
    Merge {
        inputs: Vec<PathBuf>,
        output: PathBuf,
    },
    Split {
        input: PathBuf,
        mode: SplitMode,
        dir: PathBuf,
        overwrite: bool,
    },
    Extract {
        input: PathBuf,
        pages: PageRanges,
        output: PathBuf,
    },
    Delete {
        input: PathBuf,
        pages: PageRanges,
        output: PathBuf,
    },
    Rotate {
        input: PathBuf,
        pages: Option<PageRanges>,
        rotation: Rotation,
        output: PathBuf,
    },
    /// `order: None` reverses the pages.
    Reorder {
        input: PathBuf,
        order: Option<PageRanges>,
        output: PathBuf,
    },
    Compress {
        input: PathBuf,
        output: PathBuf,
    },
    Info {
        input: PathBuf,
    },
    /// `update: None` clears all metadata.
    SetMetadata {
        input: PathBuf,
        update: Option<Metadata>,
        output: PathBuf,
    },
    ReadMetadata {
        input: PathBuf,
    },
}

/// Successful result of a [`Job`].
#[derive(Debug, Clone)]
pub enum Outcome {
    Saved {
        path: PathBuf,
        pages: u32,
    },
    Split {
        dir: PathBuf,
        count: usize,
    },
    Compressed {
        path: PathBuf,
        before: u64,
        after: u64,
    },
    Info {
        info: Box<PdfInfo>,
        size: u64,
    },
    Metadata(Metadata),
}

/// Why a job failed.
#[derive(Debug)]
pub enum JobError {
    Core(Error),
    OutputExists(PathBuf),
    OutputIsInput,
}

impl From<Error> for JobError {
    fn from(e: Error) -> Self {
        JobError::Core(e)
    }
}

/// Messages sent from worker threads to the UI.
pub enum Msg {
    /// Page count (or error text) for a file in the list.
    PageCount(PathBuf, Result<u32, String>),
    /// A [`Job`] finished.
    Finished(Result<Outcome, JobError>),
}

/// Run `job` on a new thread; `notify` is called after the result is sent
/// (used to wake up the UI).
pub fn spawn(job: Job, tx: Sender<Msg>, notify: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        let result = run(job);
        let _ = tx.send(Msg::Finished(result));
        notify();
    });
}

/// Read the page count of `path` on a new thread.
pub fn spawn_page_count(path: PathBuf, tx: Sender<Msg>, notify: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        let result = Pdf::open(&path)
            .map(|p| p.page_count())
            .map_err(|e| e.to_string());
        let _ = tx.send(Msg::PageCount(path, result));
        notify();
    });
}

fn save(mut pdf: Pdf, output: &Path) -> Result<Outcome, JobError> {
    let pages = pdf.page_count();
    pdf.save(output)?;
    Ok(Outcome::Saved {
        path: output.to_path_buf(),
        pages,
    })
}

fn run(job: Job) -> Result<Outcome, JobError> {
    match job {
        Job::Merge { inputs, output } => save(leafcut_core::merge_files(&inputs)?, &output),
        Job::Split {
            input,
            mode,
            dir,
            overwrite,
        } => {
            let pdf = Pdf::open(&input)?;
            let plan = leafcut_core::plan_split(&mode, pdf.page_count())?;
            let prefix = input
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "output".into());
            let targets: Vec<PathBuf> = plan
                .iter()
                .map(|p| dir.join(format!("{prefix}-{}.pdf", p.label)))
                .collect();
            for target in &targets {
                if crate::app::same_file(target, &input) {
                    return Err(JobError::OutputIsInput);
                }
                if target.exists() && !overwrite {
                    return Err(JobError::OutputExists(target.clone()));
                }
            }
            fs::create_dir_all(&dir).map_err(|e| Error::Io {
                path: dir.clone(),
                source: e,
            })?;
            for (part, target) in plan.iter().zip(&targets) {
                pdf.extract(&part.pages)?.save(target)?;
            }
            Ok(Outcome::Split {
                dir,
                count: targets.len(),
            })
        }
        Job::Extract {
            input,
            pages,
            output,
        } => {
            let pdf = Pdf::open(&input)?;
            save(pdf.extract_ranges(&pages)?, &output)
        }
        Job::Delete {
            input,
            pages,
            output,
        } => {
            let mut pdf = Pdf::open(&input)?;
            let list = pages.resolve_set(pdf.page_count())?;
            pdf.delete_pages(&list)?;
            save(pdf, &output)
        }
        Job::Rotate {
            input,
            pages,
            rotation,
            output,
        } => {
            let mut pdf = Pdf::open(&input)?;
            let list = match pages {
                Some(r) => Some(r.resolve_set(pdf.page_count())?),
                None => None,
            };
            pdf.rotate(list.as_deref(), rotation)?;
            save(pdf, &output)
        }
        Job::Reorder {
            input,
            order,
            output,
        } => {
            let mut pdf = Pdf::open(&input)?;
            let n = pdf.page_count();
            let list = match order {
                Some(r) => r.resolve(n)?,
                None => (1..=n).rev().collect(),
            };
            pdf.reorder(&list)?;
            save(pdf, &output)
        }
        Job::Compress { input, output } => {
            let before = fs::metadata(&input).map(|m| m.len()).unwrap_or(0);
            let mut pdf = Pdf::open(&input)?;
            pdf.compress();
            pdf.save(&output)?;
            let after = fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
            Ok(Outcome::Compressed {
                path: output,
                before,
                after,
            })
        }
        Job::Info { input } => {
            let size = fs::metadata(&input).map(|m| m.len()).unwrap_or(0);
            let pdf = Pdf::open(&input)?;
            Ok(Outcome::Info {
                info: Box::new(pdf.info()),
                size,
            })
        }
        Job::SetMetadata {
            input,
            update,
            output,
        } => {
            let mut pdf = Pdf::open(&input)?;
            match update {
                Some(update) => pdf.set_metadata(&update)?,
                None => pdf.clear_metadata()?,
            }
            save(pdf, &output)
        }
        Job::ReadMetadata { input } => Ok(Outcome::Metadata(Pdf::open(&input)?.metadata())),
    }
}
