# Command-line reference

[Türkçe](cli.tr.md) · [Back to README](../README.md)

Every command has its own help: `leafcut --help`, `leafcut split --help`, and so on.

| Command    | What it does |
|------------|--------------|
| `merge`    | Combine several PDFs into one, in the order given |
| `split`    | One file per page, one file per range (`1-3,7,10-`), or every N pages |
| `extract`  | Write selected pages to a new file (any order, repeats allowed) |
| `delete`   | Remove pages |
| `rotate`   | Rotate all or selected pages by 90/180/270 degrees clockwise |
| `reorder`  | Put pages in a new order, or reverse them |
| `compress` | Lossless optimisation (see [Limitations](../README.md#limitations)) |
| `info`     | Page count, page sizes, PDF version and metadata (text or JSON) |
| `meta`     | Set or clear title, author, subject, keywords, creator, producer |

## Examples

```sh
# Merge (the output must be given explicitly)
leafcut merge cover.pdf report.pdf appendix.pdf -o full.pdf

# Split into single pages: book-01.pdf, book-02.pdf, ...
leafcut split book.pdf

# Split by ranges: book-01-03.pdf, book-07.pdf, book-10-12.pdf
leafcut split book.pdf --ranges 1-3,7,10- --out-dir parts/

# Ten pages per file
leafcut split book.pdf --every 10

# Keep pages 1 and 4-6 (written to contract-extract.pdf by default)
leafcut extract contract.pdf --pages 1,4-6

# Delete the first two pages and everything from page 9 on
leafcut delete scan.pdf --pages -2,9- -o scan-trimmed.pdf

# Rotate every page, or only some pages
leafcut rotate scan.pdf --angle 90
leafcut rotate scan.pdf --angle 180 --pages 2,4-5 -o fixed.pdf

# Move page 3 to the front, or reverse the document
leafcut reorder slides.pdf --order 3,1,2,4-
leafcut reorder slides.pdf --reverse

# Optimise
leafcut compress report.pdf -o report-small.pdf

# Inspect
leafcut info report.pdf
leafcut info report.pdf --pages        # list every page size
leafcut info *.pdf --json

# Metadata (an empty value removes a field)
leafcut meta thesis.pdf --title "Yüksek Lisans Tezi" --author "Ayşe Yılmaz"
leafcut meta thesis.pdf --clear -o thesis-clean.pdf
```

## Page ranges

Page numbers are 1-based and comma separated: `5`, `1-3`, `10-` (to the end), `-3` (from the
start), `all`. Example: `1-3,7,10-`.

## Output files

Commands that produce one file write `INPUT-<operation>.pdf` next to the input unless you pass
`-o FILE`. leafcut refuses to:

- overwrite the input (pass `--in-place` if you really want that),
- overwrite an existing output (pass `--force`).

Outputs are written to a temporary file in the target directory and then renamed, so an
interrupted run does not leave a half-written PDF behind.

## Exit codes

| Code | Meaning |
|------|---------|
| `0`  | success |
| `1`  | error: unreadable file, broken PDF, I/O |
| `2`  | invalid usage: bad arguments, page out of range |
| `3`  | encrypted PDF (not supported) |
| `4`  | output file already exists |
