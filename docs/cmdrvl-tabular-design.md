# cmdrvl-tabular — design (bd-e0g)

Status: design + spike. Spike: `spikes/tabular-spike/` (6 passing tests, real files
in all five formats). Everything not covered by the spike is marked **unproven**.
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

Parquet codecs: spike compiled and read a Snappy file only. gzip and lz4 are compiled in but
**untested**. `zstd` (C dependency, needed for DuckDB-written files) is a crate feature `zstd`,
default on in the real crate, **not built or tested in the spike**.

## Cell model

```rust
pub enum Cell { Null, Text(Vec<u8>) }   // Text is never empty
```

Invariants, all enforced by the spike's cross-format test:
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

Refused in v0 (spike refuses them, with type name and coordinates only): Parquet decimal, date,
timestamp, binary, float16; XLSX datetime/duration/error cells. Planned v1 renderings, **not
implemented**: dates as `YYYY-MM-DD`, timestamps as RFC 3339 UTC with fixed 3/6-digit fraction,
decimals as exact scale-preserving text, XLSX datetimes via calamine's `dates` feature only when
the cell is a true Excel date (serial-number ambiguity is the risk), XLSX error cells as the
literal `#N/A` text behind an opt-in `error_cells = Literal`. Refusing is the right v0 behavior:
silently guessing a date format is exactly the divergence this crate exists to remove.

Known limit: JSON numbers beyond f64/i64 precision lose digits (`serde_json` without
`arbitrary_precision`). Spine tools that need exact decimals from JSONL must carry them as strings.

`classify(&Cell) -> CellClass { Null, Integer, Decimal, Bool, Text }` is a pure function of the
canonical bytes, so type classification is identical across formats by construction. (Not in spike.)

## API surface

```rust
pub struct OpenOptions {
    pub format: Option<Format>,        // override extension sniff
    pub sheet: Option<SheetSel>,       // Name(String) | Index(usize); xlsx/xls only
    pub max_rows: Option<u64>,         // data rows, excluding header
    pub max_bytes: Option<u64>,        // input file size on disk
    pub csv: CsvOptions,               // delimiter: Option<u8> (None = auto-detect), skip_rows, header_row
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

Spike shape differs deliberately: it materializes `Table { headers, rows }` to keep the proof small.
The streaming iterator is the contract; the spike does not prove it.

Rules:
- `rows()` yields exactly `headers().len()` cells per row. Short JSONL rows are `Null`-padded,
  and headers are the union of keys in first-seen order (JSONL needs a first pass, or a documented
  first-record-wins mode: **decision for Wave 3 owners, default union**).
- All maps in `context` are `BTreeMap` (R-005).
- Header handling (`__shape_col_N` for empty, duplicate refusal) stays in the tools. Moving it here
  would silently change shape output; revisit once all tools consume the crate.

## Format detection

Extension first (`csv tsv jsonl ndjson parquet xlsx xls`), then confirm with magic bytes:
`PAR1` head and tail (Parquet), `PK\x03\x04` (xlsx), `D0 CF 11 E0` (xls). Mismatch or unknown →
`E_FORMAT`. **Unproven**: magic-byte check is not in the spike (extension only).

## CSV dialect

Port `shape/src/csv/dialect.rs` as the single implementation (candidates `, \t ; | ^`, 64 KiB
sample, record-count/mode scoring, `E_DIALECT` when ambiguous). `.tsv` pins `\t`. BOM stripped;
UTF-16/32 BOM or NUL bytes → `E_ENCODING` (same as shape today). `profile slice` preamble handling
is passed in as `skip_rows` / `header_row`; slice's own encoding directives stay in profile.
**Unproven**: spike uses a fixed delimiter by extension; dialect port is untested here.

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
| `E_UNSUPPORTED_VALUE` | nested, or a type v0 refuses | `kind`, `column` or `row`+`col` |
| `E_ENCRYPTED` | password-protected workbook | none |

Refusal `detail` and `context` never contain cell values (see below). Exit code mapping is the
tool's business and follows bd-6g4; the spike prints `{"outcome":"REFUSAL",...}` and exits 2 only
because that is today's convention.

## Zero retention

The reader is a pure function from file bytes to cells:
- no files written, no temp files, no cache, no network, no env reads;
- no logging of cell content; `Refusal.detail`/`context` carry codes, type names, row/col
  coordinates, sheet names and header names only. Spike test
  `unsupported_value_refusal_carries_no_cell_content` pins this (the first spike draft printed `{:?}`
  of the value, which leaked; fixed). Header/sheet names are metadata, not cell values;
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
- `max_rows`: counted while streaming; the first row past the limit → `E_TOO_LARGE`. (Spike checks
  after full read; real impl must stop early.)
- Streaming: CSV, JSONL and Parquet (per row group) stream. The spike's XLSX path uses
  `worksheet_range`, which materializes the whole sheet; whether calamine's cell reader can stream
  xlsx at 0.38 is **unproven**. Document XLSX as O(sheet) memory until shown otherwise.

## Composite-key parity

`cmdrvl-tabular` hands over `Cell`s. A tool's key builder takes `[Cell]` components,
treats `Null` as an empty component, and applies the shared rules (ASCII-trim each, any empty →
incomplete tuple, tuples ordered lexicographically by component bytes, never flattened with a
sentinel). That is exactly what `spine_rules::composite_key::assert_ordered_tuple_semantics`
checks; the spike runs it over an adapter that consumes `Cell`s. The spike adapter is thin; the
claim proven is "Cell bytes survive the shared contract", not "every tool's key builder passes".

## Conformance plan

1. Fixture dataset (spike): 3 rows × `id,region,amount,active,note` covering padded text, `NA`/`NULL`
   literals, a null, integral float, fractional float, bool. Provided as csv, tsv, jsonl, parquet
   (Snappy), xlsx (two sheets: `loans`, `notes`), all produced by independent writers via
   `fixtures/make_fixtures.py` (csv, json, pyarrow, openpyxl).
2. Test: every format yields identical headers and identical `Vec<Vec<Cell>>`. (Spike: passing.)
3. Refusals: `E_AMBIGUOUS_SHEET`, `E_SHEET_NOT_FOUND`, `E_TOO_LARGE`, `E_UNSUPPORTED_VALUE` (spike:
   passing). To add in the real crate: `E_FORMAT` (extension/magic mismatch), `E_EMPTY`, `E_ENCODING`,
   `E_DIALECT`, `E_CSV_PARSE` ragged row, `E_JSONL_PARSE`, corrupt Parquet footer, truncated xlsx.
4. Extend fixtures before Wave 3 signs off: non-UTF-8 CSV bytes, a ≥1 row-group-boundary Parquet file,
   zstd Parquet from DuckDB, xlsx with sheet data starting off A1, xlsx with formula and date cells,
   CRLF and quoted-newline CSV, JSONL with ragged keys.
5. Each Wave 3 pane adds one test in its repo: open all five fixtures through its own adapter and
   assert grid equality plus `assert_ordered_tuple_semantics`. Fixtures are published by
   `cmdrvl-tabular` (a `fixtures` dir versioned with the crate); tools do not copy them.

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

## Not proven by the spike

Streaming iterator API; CSV dialect auto-detection and BOM/encoding refusals; magic-byte format
detection; `.xls` (BIFF) and `.ndjson`; Parquet gzip/lz4/zstd and multi-row-group files; Parquet
decimal/date/timestamp/nested; XLSX dates, formulas, hidden sheets, encrypted files, off-A1 used
ranges, streaming; zip-bomb guard; the DuckDB in-memory path; calamine 0.38 compatibility with
fingerprint's existing code; release-binary size and build-time cost of adding parquet (the spike
cold build took ~4 minutes, `cargo check` ~7s warm; binary size not measured).
