# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-22

First public release.

### Added

- `leafcut-core` library with a typed API (`Pdf`, `PageRanges`, `Rotation`, `SplitMode`,
  `Metadata`) and a single `Error` enum.
- Operations: merge, split (every page, by ranges, every N pages), extract, delete, rotate,
  reorder, lossless compress/optimise, info, set/clear metadata.
- Page-range syntax `1-3,7,10-`, `-3`, `all`.
- Detection of encrypted PDFs with a clear error.
- `leafcut` command-line tool with per-command help, safe output handling (`--in-place`,
  `--force`), JSON output for `info`, and documented exit codes.
- `leafcut-gui` desktop app (egui/eframe) with drag and drop, file list reordering, background
  processing, and English/Turkish interface.
- CI for Linux, Windows and macOS and a tag-triggered release workflow.

[Unreleased]: https://github.com/hytztrk-ux/leafcut/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/hytztrk-ux/leafcut/releases/tag/v0.1.0
