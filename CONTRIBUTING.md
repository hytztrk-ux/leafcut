# Contributing to leafcut

Thanks for your interest! Issues and pull requests can be written in **English or Turkish**
(Türkçe ya da İngilizce yazabilirsiniz).

## Ground rules

- **Offline only.** leafcut must never open a network connection. Pull requests that add
  telemetry, update checks or any networking will not be merged.
- **Pure Rust.** Please do not add dependencies that need a C toolchain or system libraries
  (beyond what egui/eframe already need on Linux). If a feature truly needs one, open an issue
  first so we can discuss it.
- **Logic lives in `leafcut-core`.** The CLI and GUI should stay thin: parse input, call the core,
  show the result. New operations go into `leafcut-core` with tests first.
- **Never destroy user data.** Do not overwrite inputs by default, write outputs atomically, and
  return a clear error rather than a silently broken file.
- **Be honest in docs.** If something is limited (for example, how much `compress` achieves), say
  so in the README.

## Development setup

You need Rust 1.88 or newer (`rustup update stable`).

On Linux the GUI needs the usual X11/Wayland development headers to build in some environments,
for example on Debian/Ubuntu:

```sh
sudo apt-get install libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libxkbcommon-dev libwayland-dev libgl1-mesa-dev
```

Then:

```sh
cargo build --workspace
cargo run -p leafcut-cli -- --help
cargo run -p leafcut-gui
```

## Before you open a pull request

Run the same checks as CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Tests

- Unit tests live next to the code (`#[cfg(test)]`).
- `crates/leafcut-core/tests/` holds integration tests. **Do not commit binary PDF fixtures**:
  build the PDFs you need in code with `lopdf` (see `tests/common/mod.rs`), so every test is
  readable and reviewable.
- `crates/leafcut-cli/tests/` runs the real `leafcut` binary end to end.

### Translations

GUI strings live in `crates/leafcut-gui/src/i18n.rs`, with one `Strings` table per language. To add
a language, add a `Lang` variant and a new table; the compiler will tell you if a string is
missing. Natural phrasing is preferred over word-for-word translation.

## Commit messages and changelog

Keep commits focused and describe *why* in the message body when it is not obvious. User-visible
changes should get a line in `CHANGELOG.md` under "Unreleased".

## Releasing (maintainers)

1. Update the version in the root `Cargo.toml` and move "Unreleased" entries in `CHANGELOG.md`
   under the new version.
2. Tag the commit: `git tag v0.2.0 && git push --tags`.
3. The `release` workflow builds Windows, macOS (Intel and Apple Silicon) and Linux binaries and
   attaches them to a GitHub release.

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
