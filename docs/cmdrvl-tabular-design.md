# cmdrvl-tabular — design

Status: design + v0 skeleton, pre-1.0, API may change. Implementation: <https://github.com/cmdrvl/tabular>
(crate `cmdrvl-tabular`, not on crates.io; 44 passing tests over real files in all five formats).
Throwaway spike: `spikes/tabular-spike/` (superseded by the skeleton, see Conformance).
Things the skeleton does not cover are marked **unproven**; the skeleton README lists them.
Crate versions were checked on crates.io on 2026-11-24.

## Problem

Spine tools read different formats (fingerprint: csv/xlsx; shape, rvl, profile: csv;
canon: csv/tsv/jsonl; verify, benchmark: csv/json/jsonl/parquet through bundled DuckDB).
An agent holding Parquet or an `.xlsx` loan tape cannot run `shape` or `rvl`. The fix is
one reader that yields the same cells for the same dataset regardless of container.

## Placement

Separate repo `cmdrvl/tabular`, crate `cmdrvl-tabular`. Not inside `spine-rules`:
spine-rules is a dev-dependency of every tool and is deliberately serde + serde_json only
(`AGENTS.md` rule 4); parquet and calamine would land in every tool's test build.
`spine-rules` keeps owning composite-key *semantics*; `cmdrvl-tabular` owns *cells*.
`cmdrvl-tabular` takes `spine-rules` as a dev-dependency only, for the conformance test.
No `evidence-machine` dependency (boundary ban in bd-2tw).

## Crates

| Need | Choice | Why |
|---|---|---|
| CSV/TSV | `csv` 1.4.0 (`ByteRecord`) | already used by shape, rvl, profile, canon, fingerprint; bytes-first |
| JSONL | `serde_json` 1.0.x **with `preserve_order`** | spike failure: without it keys sort alphabetically and JSONL headers stop matching CSV headers |
| Parquet | `parquet` 60.2.0, `default-features = false`, features `snap`, `flate2-zlib-rs`, `lz4`; **record API** (`SerializedFileReader::get_row_iter`) | no `arrow` feature, so no arrow types in the public API and no clash with rvl's arrow 57. MSRV 1.88 |
| XLSX/XLS | `calamine` 0.38.0, default features | `open_workbook_auto`, `sheet_names()`, `worksheet_range()`. fingerprint pins `=0.26.1`; it must move to 0.38 to share the crate (**unproven** that its `read_cell`/`read_range` code compiles unchanged) |

Rejected: DuckDB (bundled C++ build in every binary; stays inside verify/benchmark as their SQL engine),
`polars`, `arrow-csv`, parquet's `arrow` feature (pulls six arrow crates for nothing we need).

Parquet codecs: snappy, gzip (`flate2-zlib-rs`), lz4 and `zstd` are tested in the skeleton (pyarrow-written
files, multiple row groups). `zstd` (C dependency) is crate feature `zstd`, default on; the crate also builds
and tests with `--no-default-features`. Brotli is not compiled in.

## Cell model

```rust
pub enum Cell { Null, Text(Vec<u8>) }   // Text is never empty
```

Invariants, all enforced by the skeleton's cross-format tests:
1. Zero-length input is `Null` in every format (CSV `,,`, JSON `null`, Parquet null, XLSX empty cell,
   empty string anywhere). The null/empty-string distinction is intentionally not preserved:
   spine-rules treats both as an incomplete key component.
2. `NA`, `NULL`, `N/A`, `null` are ordinary `Text`. The reader never interprets missing-value tokens.
3. Whitespace is preserved (`" North "` stays). ASCII-trim is the key layer's job
   (`spine_rules::composite_key::reference_key_tuple`), so readers cannot disagree on it.
4. Bytes, not `String`. CSV non-UTF-8 bytes pass through untouched (needed by the `non_utf8` fixture).

### Canonical text per source type

Parity means: the *typed* source value renders to the same text a CSV writer would have to emit.

| Source | Rendering |
|---|---|
| CSV/TSV field | bytes verbatim |
| JSON string / Parquet string / XLSX string | UTF-8 bytes verbatim |
| integer (JSON, Parquet any width, XLSX int) | base-10, no `+`, no leading zeros |
| float (JSON non-integer, Parquet f32/f64, XLSX float) | integral and `abs < 1e15` → integer text (`2000.0` → `2000`); else Rust shortest round-trip `{}` |
| bool | `true` / `false` |
| JSON nested object/array, Parquet struct/list/map | refuse `E_UNSUPPORTED_VALUE` |

v1 (tabular `91aec2d`, github.com/cmdrvl/tabular) renders what v0 refused. Dates are `YYYY-MM-DD`; timestamps
`YYYY-MM-DDTHH:MM:SS.fff` / `.ffffff` / `.fffffffff` by source unit (a trailing `Z` only when the column is
UTC-adjusted, otherwise a naive wall clock); times of day `HH:MM:SS.fff`; decimals are exact scale-preserving
text (`12.50`, `-0.05`). Column types come from the Parquet schema (the record API hands nanosecond and non-UTC
timestamps over as bare integers). XLSX datetimes: whole-day serial → date, otherwise a naive millisecond
timestamp, time-only serial → time of day, 1900 and 1904 systems; refused: serial 60 (Excel's nonexistent
1900-02-29), serials outside 0–9999-12-31, a time-only serial in a 1904 workbook (ambiguous), durations and error
cells. Still refused: Parquet binary and nested. Excel cannot tell a date from a midnight datetime, so neither
can the reader. Expected text is produced by Python `datetime`/`decimal` on pyarrow/openpyxl files, not by the
crate. The `error_cells = Literal` opt-in sketched earlier was not built.

Known limit: JSON numbers beyond f64/i64 precision lose digits (`serde_json` without
`arbitrary_precision`). Spine tools that need exact decimals from JSONL must carry them as strings.

`classify(&Cell) -> CellClass { Null, Integer, Decimal, Bool, Text }` is a pure function of the
canonical bytes, so type classification is identical across formats by construction. (Implemented and tested.)

## API surface

```rust
pub struct OpenOptions {
    pub format: Option<Format>,        // override extension sniff
    pub sheet: Option<SheetSel>,       // Name(String) | Index(usize); xlsx/xls only
    pub max_rows: Option<u64>,         // data rows, excluding header
    pub max_bytes: Option<u64>,        // input file size on disk
    pub csv: CsvOptions,               // delimiter: Option<u8> (None = auto-detect), skip_rows (physical lines before the header)
}
pub enum Format { Csv, Tsv, Jsonl, Parquet, Xlsx }   // .ndjson => Jsonl, .xls => Xlsx

pub fn open(path: &Path, opts: &OpenOptions) -> Result<Table, Refusal>;

pub struct Table { /* owns reader state */ }
impl Table {
    pub fn headers(&self) -> &[Vec<u8>];            // raw bytes; normalization stays tool-side
    pub fn format(&self) -> Format;
    pub fn source(&self) -> &SourceInfo;            // resolved delimiter, sheet name, bytes_read
    pub fn rows(&mut self) -> impl Iterator<Item = Result<Row, Refusal>>;   // streaming
}
pub struct Row { pub index: u64, pub cells: Vec<Cell> }   // index is 1-based data row

pub struct Refusal { pub code: RefusalCode, pub detail: String, pub context: BTreeMap<String, Value> }
```

Skeleton v0 differences, stated plainly: `open` materializes the table, so `rows()` is a single-pass
iterator over already-read rows and never yields `Err`; all refusals surface from `open`. The signatures
are the streaming-shaped contract; real streaming is not implemented. `header_row` was dropped as redundant
with `skip_rows`. `SourceInfo { delimiter, sheet, bytes_read }` where `bytes_read` is the file size
(v0 readers consume the whole file). `Row::key_components(&[usize]) -> Option<Vec<Vec<u8>>>` is the
composite-key hand-off (`Null` becomes an empty component). `Cell::classify` is implemented.

Rules:
- `rows()` yields exactly `headers().len()` cells per row. Short JSONL rows are `Null`-padded,
  and headers are the union of keys in first-seen order (JSONL needs a first pass, or a documented
  first-record-wins mode). Decided in the skeleton: union only, first-seen key order; no first-record-wins mode.
- All maps in `context` are `BTreeMap` (R-005).
- Header handling (`__shape_col_N` for empty, duplicate refusal) stays in the tools. Moving it here
  would silently change shape output; revisit once all tools consume the crate.

## Format detection

Extension first (`csv tsv jsonl ndjson parquet xlsx xls`), then confirm with magic bytes:
`PAR1` head and tail (Parquet), `PK\x03\x04` (xlsx), `D0 CF 11 E0` (xls). Mismatch or unknown →
`E_FORMAT`. Implemented and tested (unknown extension, content/extension mismatch, override still checks
magic); `.xls` and `.xlsm` have no fixture, so those paths are untested.

## CSV dialect

Re-implemented from `shape/src/csv/dialect.rs` as the single implementation (candidates `, \t ; | ^`, 64 KiB
sample, record-count/mode scoring, `E_DIALECT` when ambiguous). `.tsv` pins `\t`. BOM stripped;
UTF-16/32 BOM or NUL bytes → `E_ENCODING` (same as shape today). `profile slice` preamble handling
is passed in as `skip_rows` (physical lines); slice's own encoding directives stay in profile.
The skeleton implements this and tests every candidate delimiter, pinning, BOM, CRLF, quoted newlines and
non-UTF-8 bytes. **Reconciled against shape (tabular `11ce182`):** run against shape 0.9.0's real input
pipeline on its 16 CSV fixtures plus 3,000 generated files, the first pass disagreed on 443 of 3,016. The causes
were an Excel `sep=<byte>` first line (shape honors it), shape's retry over the whole file when the 64 KiB
prefix does not decide, the backslash-escape retry for a pinned or `sep=` delimiter, and the NUL scan window
(8 KiB after the BOM, not 64 KiB). After fixing all four: 0 disagreements. `tests/fixtures/shape/` plus
`shape.expected.json` (shape's own outcome per file, produced by running shape) keep that checkable. Two
scoring quirks are shape's own and so also tabular's (ragged record in the sample refuses `E_DIALECT`; tiny
files with quoted newlines can pick the wrong delimiter). Kept on purpose: `skip_rows` exists only in tabular,
and a header-only file is `E_EMPTY` from `open` where shape's input layer reports it OK and decides later.

## Excel sheet selection

- One sheet → used. More than one and no `sheet` → `E_AMBIGUOUS_SHEET`, `context.sheets` = names in
  workbook order. (Spike: tested.)
- `Name` exact match wins; otherwise a base-10 `Index` (0-based). A name that parses as an integer
  and also matches a sheet name is the name. Not found → `E_SHEET_NOT_FOUND` with the same list.
  (Spike: tested, including index selection.)
- Hidden sheets count as sheets (calamine lists them; **hidden-sheet behavior unproven**).
- Header row = first row of the sheet's *used range* as calamine reports it, so a sheet whose data
  starts at C5 yields header from C5. Merged cells, formulas without cached values, and password-protected
  workbooks (`E_ENCRYPTED`) are **unproven**.
- CLI flag in every tool: `--sheet <name|index>`.

## Refusal taxonomy

Reuse shape's existing codes unchanged so shape's refusal schema does not move:
`E_IO`, `E_ENCODING`, `E_CSV_PARSE`, `E_EMPTY`, `E_HEADERS`, `E_DIALECT`, `E_TOO_LARGE`.
Semantics (as in shape): `E_HEADERS` = no header row at all (`context.issue = "missing"`); `E_EMPTY` =
header present, zero data rows, in every format.
(`E_INPUT_NOT_LOCKED`, `E_INPUT_DRIFT`, `E_AMBIGUOUS_PROFILE` are tool-level, not reader-level.)

New:

| Code | When | `context` |
|---|---|---|
| `E_FORMAT` | unknown extension or magic/extension mismatch | `extension`, `detected` |
| `E_JSONL_PARSE` | invalid JSON line or line not an object | `line` |
| `E_PARQUET_PARSE` | not a Parquet file / corrupt footer / unreadable page | none |
| `E_XLSX_PARSE` | not a workbook / corrupt zip | none |
| `E_AMBIGUOUS_SHEET` | multi-sheet workbook, no `--sheet` | `sheets` |
| `E_SHEET_NOT_FOUND` | `--sheet` matches nothing | `requested`, `sheets` |
| `E_UNSUPPORTED_VALUE` | nested, or a type v0 refuses | `kind`, plus `column`+`row` (JSONL, Parquet) or absolute 1-based `row`+`col` (XLSX) |
| `E_ENCRYPTED` | password-protected workbook | none |

Refusal `detail` and `context` never contain cell values (see below). Exit code mapping is the
tool's business and follows bd-6g4; the crate returns the typed `Refusal` and prints nothing.

## Zero retention

The reader is a pure function from file bytes to cells:
- no files written, no temp files, no cache, no network, no env reads;
- no logging of cell content; `Refusal.detail`/`context` carry codes, type names, row/col
  coordinates, sheet names and header names only. Skeleton tests
  (`assert_no_leak` over parquet, xlsx, JSONL value and parse refusals, and `E_IO`) pin this; the first spike
  draft printed `{:?}` of the value, which leaked. Library error text is never forwarded. Header/sheet names are metadata, not cell values;
  a tool with a stricter policy can redact `context.column`.
- tools that need SQL (verify, benchmark) feed rows into DuckDB through an in-memory appender,
  never a temp CSV. **Unproven**; if the DuckDB appender path is not workable, XLSX in
  verify/benchmark stays out of scope rather than adding a temp file.
- the reader does not hash. Tools hash the raw input as they do today; reading the file twice
  leaves the TOCTOU window that `E_INPUT_DRIFT` already covers.

## Memory bounds

- `max_bytes`: compared with file size from `metadata()` before opening → `E_TOO_LARGE`. For xlsx the
  uncompressed zip size is the real cost; checking central-directory sizes is planned and **unproven**
  (zip-bomb exposure until done).
- `max_rows`: the first row past the limit → `E_TOO_LARGE`; readers stop at that row, and Parquet refuses on
  the footer row count before decoding. Tested in every format at the boundary.
- Streaming: **not implemented in v0** (see API). The format readers could stream (csv records, JSONL lines,
  Parquet row groups); XLSX uses `worksheet_range`, which materializes the sheet, and whether calamine's cell
  reader can stream xlsx at 0.38 is **unproven**. XLSX is O(sheet) memory until shown otherwise.

## Composite-key parity

`cmdrvl-tabular` hands over `Cell`s. A tool's key builder takes `[Cell]` components,
treats `Null` as an empty component, and applies the shared rules (ASCII-trim each, any empty →
incomplete tuple, tuples ordered lexicographically by component bytes, never flattened with a
sentinel). That is exactly what `spine_rules::composite_key::assert_ordered_tuple_semantics`
checks; the skeleton runs it over an adapter that consumes `Cell`s, and also compares `Row::key_components`
tuples across all five formats. The claim proven is "Cell bytes survive the shared contract", not "every
tool's key builder passes".

## Conformance plan

Implemented in the skeleton (`tests/conformance.rs`, `tests/refusals.rs`, fixtures from independent writers via
`tests/fixtures/make_fixtures.py`):

1. Dataset: 3 rows x `id,region,amount,active,note` (padded text, `NA`/`NULL` literals, a null, integral and
   fractional float, bool) as csv, tsv, jsonl, parquet, xlsx (sheets `loans`, `notes`). Every format is compared
   to a hard-coded golden grid, with classification and composite-key tuples identical across formats and the
   shared `assert_ordered_tuple_semantics` run over cells.
2. Parquet: zstd, gzip, lz4 codecs across row groups of 2; float32 renders in f32 shortest form.
3. Refusals with planted inputs: `E_AMBIGUOUS_SHEET` (incl. hidden sheet), `E_SHEET_NOT_FOUND`, name-beats-index,
   off-A1 used range, `E_ENCRYPTED` (real msoffcrypto file), truncated xlsx, `E_UNSUPPORTED_VALUE` (parquet date and
   list, xlsx datetime, JSONL nested) each asserted to carry no cell value, `E_TOO_LARGE` for `max_rows` (every
   format, boundary) and `max_bytes`, `E_FORMAT` (unknown extension, content mismatch, override still checks magic),
   corrupt Parquet footer, `E_ENCODING`, `E_HEADERS`, `E_EMPTY`, `E_CSV_PARSE` with counts, `E_DIALECT`,
   `E_JSONL_PARSE` (no content), `E_IO` (kind only), every candidate delimiter, skip_rows, BOM, CRLF, quoted
   newline, non-UTF-8 bytes, JSONL ragged keys.
4. Spike supersession: the spike's six tests (identical grids, literal tokens/null, ambiguous sheet + index
   select, max_rows, no-leak refusal, composite-key contract) all have equivalents in the skeleton. The spike
   directory is left in place; deleting it needs the operator's say-so.

Done since the skeleton (bd-q2e): detector run against shape's fixtures; `.xls`/`.xlsm`/formula/merged-cell/brotli
fixtures; temporal and decimal rendering; streaming; zip-bomb guard; fingerprint compiles on calamine 0.38. Still
open: publishing fixtures with the crate (tools must not copy them). Each Wave 3 pane adds one test in its repo: open all five fixtures through its adapter, assert grid equality plus
`assert_ordered_tuple_semantics`.

## Wave 3 adoption

| Pane | Replaces | Notes |
|---|---|---|
| shape (bd-o7y2), rvl (bd-jf7) | their `csv` readers | add `--sheet`; rvl's arrow/polars stay dev-only for benchmarks |
| profile (bd-1t0, bd-47o) | csv reader | keep slice preamble/encoding logic in profile; pass `skip_rows`/`header_row` |
| fingerprint (bd-jxd) | `calamine` 0.26.1 + csv | cell-level APIs (`read_cell`, `read_range`) stay in fingerprint on top of calamine 0.38; use tabular only for tabular-shaped input |
| verify (bd-l2l), benchmark (bd-526) | nothing today for xlsx | keep DuckDB for csv/json/parquet; xlsx via tabular → in-memory appender |
| canon (bd-ko97) | csv/tsv/jsonl readers | gains parquet, xlsx |

Order: tabular v0.1 (CSV, TSV, JSONL, Parquet, XLSX with the v0 type set) → shape + rvl first (flagship
pair) → the rest. Date/timestamp/decimal rendering (v1) should land before verify/benchmark/canon,
whose real inputs are typed.

## Not proven (after v1)

Streaming: CSV/TSV, JSONL (two passes for the header union) and Parquet (one row group at a time) decode lazily
and refuse mid-stream; a 171 MB, 3,000,000-row CSV read to the end peaked at 2.9 MB resident (measured once, not
a test). XLSX and `.xls` stay O(sheet) because calamine loads the sheet; Parquet's peak depends on row-group
size and was not measured. Zip-bomb guard is the declared-size sum (default 2 GiB); a lying header is stopped by
the `zip` 9.x crate, so upgrading it should re-run `zip_bomb_lying.xlsx`. Files from writers other than pyarrow,
openpyxl, xlwt, xlsxwriter (Spark, DuckDB, Excel, LibreOffice) have not been through the readers; INT96
timestamps have no fixture. The DuckDB in-memory path for verify/benchmark; fingerprint on calamine 0.38 compiles
and passes its 297 lib tests plus the xlsx-touching integration suites in a scratch copy (fingerprint itself is
unchanged and still pins =0.26.1, adoption is bd-jxd); release-binary size and build time of adding parquet (cold
build of the spike was about 4 minutes).
