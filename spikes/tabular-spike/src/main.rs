//! Minimal proof for `cmdrvl-tabular` (docs/cmdrvl-tabular-design.md): one real
//! file per format read into the same canonical grid, with the same refusals.

use std::fs::File;
use std::path::Path;

use calamine::{Data, Reader, open_workbook_auto};
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::Field;

/// A cell. `Text` is never empty: zero-length input is `Null` in every format.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Cell {
    Null,
    Text(Vec<u8>),
}

#[derive(Debug)]
pub struct Refusal {
    pub code: &'static str,
    pub detail: String,
}

fn refuse<T>(code: &'static str, detail: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal {
        code,
        detail: detail.into(),
    })
}

#[derive(Default)]
pub struct OpenOptions {
    pub sheet: Option<String>,
    pub max_rows: Option<u64>,
}

pub struct Table {
    pub headers: Vec<Vec<u8>>,
    pub rows: Vec<Vec<Cell>>,
}

fn text(bytes: impl Into<Vec<u8>>) -> Cell {
    let bytes = bytes.into();
    if bytes.is_empty() {
        Cell::Null
    } else {
        Cell::Text(bytes)
    }
}

/// Canonical number text: integral values in +-1e15 print without a fraction,
/// everything else uses the shortest round-trip form.
fn canon_f64(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

pub fn read(path: &Path, opts: &OpenOptions) -> Result<Table, Refusal> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let table = match ext.as_str() {
        "csv" => read_delimited(path, b','),
        "tsv" => read_delimited(path, b'\t'),
        "jsonl" | "ndjson" => read_jsonl(path),
        "parquet" => read_parquet(path),
        "xlsx" | "xls" => read_xlsx(path, opts),
        other => refuse("E_FORMAT", format!("unsupported extension '{other}'")),
    }?;
    if let Some(max) = opts.max_rows
        && table.rows.len() as u64 > max
    {
        return refuse(
            "E_TOO_LARGE",
            format!("{} rows exceeds --max-rows {max}", table.rows.len()),
        );
    }
    Ok(table)
}

fn read_delimited(path: &Path, delimiter: u8) -> Result<Table, Refusal> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(false)
        .from_path(path)
        .map_err(|e| Refusal {
            code: "E_IO",
            detail: e.to_string(),
        })?;
    let mut records = rdr.byte_records();
    let headers = match records.next() {
        Some(Ok(h)) => h.iter().map(<[u8]>::to_vec).collect(),
        Some(Err(e)) => return refuse("E_CSV_PARSE", e.to_string()),
        None => return refuse("E_EMPTY", "no header row"),
    };
    let mut rows = Vec::new();
    for rec in records {
        let rec = rec.map_err(|e| Refusal {
            code: "E_CSV_PARSE",
            detail: e.to_string(),
        })?;
        rows.push(rec.iter().map(text).collect());
    }
    Ok(Table { headers, rows })
}

fn json_cell(v: &serde_json::Value, key: &str) -> Result<Cell, Refusal> {
    use serde_json::Value::*;
    Ok(match v {
        Null => Cell::Null,
        Bool(b) => text(b.to_string()),
        String(s) => text(s.as_bytes()),
        Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => text(i.to_string()),
            (_, Some(u), _) => text(u.to_string()),
            (_, _, Some(f)) => text(canon_f64(f)),
            _ => return refuse("E_UNSUPPORTED_VALUE", format!("number in '{key}'")),
        },
        Array(_) | Object(_) => {
            return refuse("E_UNSUPPORTED_VALUE", format!("nested value in '{key}'"));
        }
    })
}

fn read_jsonl(path: &Path) -> Result<Table, Refusal> {
    let body = std::fs::read_to_string(path).map_err(|e| Refusal {
        code: "E_IO",
        detail: e.to_string(),
    })?;
    let mut headers: Vec<Vec<u8>> = Vec::new();
    let mut rows = Vec::new();
    for (n, line) in body.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| Refusal {
            code: "E_JSONL_PARSE",
            detail: format!("line {}: {e}", n + 1),
        })?;
        let serde_json::Value::Object(map) = v else {
            return refuse("E_JSONL_PARSE", format!("line {}: not an object", n + 1));
        };
        // Header order = first-seen key order; requires serde_json `preserve_order`
        // (without it keys come back sorted and CSV/JSONL header parity breaks).
        for k in map.keys() {
            if !headers.iter().any(|h| h == k.as_bytes()) {
                headers.push(k.as_bytes().to_vec());
            }
        }
        let mut row = vec![Cell::Null; headers.len()];
        for (k, val) in &map {
            let idx = headers.iter().position(|h| h == k.as_bytes()).unwrap();
            row[idx] = json_cell(val, k)?;
        }
        rows.push(row);
    }
    // Rows seen before later keys appeared are short: pad them.
    for r in &mut rows {
        r.resize(headers.len(), Cell::Null);
    }
    if headers.is_empty() {
        return refuse("E_EMPTY", "no records");
    }
    Ok(Table { headers, rows })
}

fn parquet_cell(f: &Field, col: &str) -> Result<Cell, Refusal> {
    Ok(match f {
        Field::Null => Cell::Null,
        Field::Bool(b) => text(b.to_string()),
        Field::Byte(v) => text(v.to_string()),
        Field::Short(v) => text(v.to_string()),
        Field::Int(v) => text(v.to_string()),
        Field::Long(v) => text(v.to_string()),
        Field::UByte(v) => text(v.to_string()),
        Field::UShort(v) => text(v.to_string()),
        Field::UInt(v) => text(v.to_string()),
        Field::ULong(v) => text(v.to_string()),
        Field::Float(v) => text(canon_f64(f64::from(*v))),
        Field::Double(v) => text(canon_f64(*v)),
        Field::Str(s) => text(s.as_bytes()),
        other => {
            return refuse(
                "E_UNSUPPORTED_VALUE",
                format!("parquet {} in column '{col}'", parquet_kind(other)),
            );
        }
    })
}

/// Type name only: refusal details must never carry cell values.
fn parquet_kind(f: &Field) -> &'static str {
    match f {
        Field::Decimal(_) => "decimal",
        Field::Bytes(_) => "binary",
        Field::Date(_) => "date",
        Field::TimestampMillis(_) | Field::TimestampMicros(_) => "timestamp",
        Field::Float16(_) => "float16",
        Field::Group(_) => "struct",
        Field::ListInternal(_) => "list",
        Field::MapInternal(_) => "map",
        _ => "unsupported type",
    }
}

fn read_parquet(path: &Path) -> Result<Table, Refusal> {
    let file = File::open(path).map_err(|e| Refusal {
        code: "E_IO",
        detail: e.to_string(),
    })?;
    let reader = SerializedFileReader::new(file).map_err(|e| Refusal {
        code: "E_PARQUET_PARSE",
        detail: e.to_string(),
    })?;
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    for row in reader.get_row_iter(None).map_err(|e| Refusal {
        code: "E_PARQUET_PARSE",
        detail: e.to_string(),
    })? {
        let row = row.map_err(|e| Refusal {
            code: "E_PARQUET_PARSE",
            detail: e.to_string(),
        })?;
        let first = headers.is_empty();
        let mut cells = Vec::new();
        for (name, field) in row.get_column_iter() {
            if first {
                headers.push(name.as_bytes().to_vec());
            }
            cells.push(parquet_cell(field, name)?);
        }
        rows.push(cells);
    }
    if headers.is_empty() {
        return refuse("E_EMPTY", "no rows");
    }
    Ok(Table { headers, rows })
}

fn xlsx_cell(d: &Data, at: (usize, usize)) -> Result<Cell, Refusal> {
    Ok(match d {
        Data::Empty => Cell::Null,
        Data::String(s) => text(s.as_bytes()),
        Data::Int(i) => text(i.to_string()),
        Data::Float(f) => text(canon_f64(*f)),
        Data::Bool(b) => text(b.to_string()),
        other => {
            let kind = match other {
                Data::DateTime(_) | Data::DateTimeIso(_) => "datetime",
                Data::DurationIso(_) => "duration",
                Data::Error(_) => "error cell",
                _ => "unsupported type",
            };
            return refuse(
                "E_UNSUPPORTED_VALUE",
                format!("xlsx {kind} at row {} col {}", at.0 + 1, at.1 + 1),
            );
        }
    })
}

fn read_xlsx(path: &Path, opts: &OpenOptions) -> Result<Table, Refusal> {
    let mut wb = open_workbook_auto(path).map_err(|e| Refusal {
        code: "E_XLSX_PARSE",
        detail: e.to_string(),
    })?;
    let names = wb.sheet_names().to_vec();
    let sheet = match (&opts.sheet, names.len()) {
        (Some(s), _) => match s.parse::<usize>() {
            Ok(i) if i < names.len() && !names.contains(s) => names[i].clone(),
            _ if names.contains(s) => s.clone(),
            _ => return refuse("E_SHEET_NOT_FOUND", format!("'{s}' not in {names:?}")),
        },
        (None, 1) => names[0].clone(),
        (None, _) => {
            return refuse(
                "E_AMBIGUOUS_SHEET",
                format!("workbook has sheets {names:?}; pass --sheet"),
            );
        }
    };
    let range = wb.worksheet_range(&sheet).map_err(|e| Refusal {
        code: "E_XLSX_PARSE",
        detail: e.to_string(),
    })?;
    let mut it = range.rows();
    let Some(hdr) = it.next() else {
        return refuse("E_EMPTY", "no header row");
    };
    let headers = hdr.iter().map(|c| c.to_string().into_bytes()).collect();
    let mut rows = Vec::new();
    for (r, row) in it.enumerate() {
        rows.push(
            row.iter()
                .enumerate()
                .map(|(c, d)| xlsx_cell(d, (r + 1, c)))
                .collect::<Result<_, _>>()?,
        );
    }
    Ok(Table { headers, rows })
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: tabular-spike <file> [sheet]");
    let opts = OpenOptions {
        sheet: std::env::args().nth(2),
        max_rows: None,
    };
    match read(Path::new(&path), &opts) {
        Ok(t) => {
            println!("{} cols, {} rows", t.headers.len(), t.rows.len());
            for r in t.rows {
                println!("{r:?}");
            }
        }
        Err(r) => {
            eprintln!(
                "{{\"outcome\":\"REFUSAL\",\"code\":\"{}\",\"detail\":{:?}}}",
                r.code, r.detail
            );
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name)
    }
    fn xlsx_opts() -> OpenOptions {
        OpenOptions {
            sheet: Some("loans".into()),
            max_rows: None,
        }
    }

    #[test]
    fn all_five_formats_yield_identical_grids() {
        let base = read(&fx("sample.csv"), &OpenOptions::default()).unwrap();
        assert_eq!(base.rows.len(), 3);
        for (file, opts) in [
            ("sample.tsv", OpenOptions::default()),
            ("sample.jsonl", OpenOptions::default()),
            ("sample.parquet", OpenOptions::default()),
            ("sample.xlsx", xlsx_opts()),
        ] {
            let t = read(&fx(file), &opts);
            assert!(t.is_ok(), "{file}: {:?}", t.as_ref().err());
            let t = t.unwrap();
            assert_eq!(t.headers, base.headers, "{file} headers");
            assert_eq!(t.rows, base.rows, "{file} rows");
        }
    }

    #[test]
    fn literal_missing_tokens_stay_text_and_empty_is_null() {
        let t = read(&fx("sample.parquet"), &OpenOptions::default()).unwrap();
        assert_eq!(t.rows[0][4], Cell::Text(b"NA".to_vec()));
        assert_eq!(t.rows[1][4], Cell::Text(b"NULL".to_vec()));
        assert_eq!(t.rows[2][4], Cell::Null);
        // Padding is preserved by the reader; trimming is the key layer's job.
        assert_eq!(t.rows[2][1], Cell::Text(b" North ".to_vec()));
    }

    #[test]
    fn multi_sheet_workbook_without_sheet_is_refused() {
        let e = read(&fx("sample.xlsx"), &OpenOptions::default())
            .err()
            .unwrap();
        assert_eq!(e.code, "E_AMBIGUOUS_SHEET");
        let e = read(
            &fx("sample.xlsx"),
            &OpenOptions {
                sheet: Some("nope".into()),
                max_rows: None,
            },
        )
        .err()
        .unwrap();
        assert_eq!(e.code, "E_SHEET_NOT_FOUND");
        let by_index = read(
            &fx("sample.xlsx"),
            &OpenOptions {
                sheet: Some("0".into()),
                max_rows: None,
            },
        )
        .unwrap();
        assert_eq!(by_index.rows.len(), 3);
    }

    #[test]
    fn unsupported_value_refusal_carries_no_cell_content() {
        let e = read(&fx("unsupported_date.parquet"), &OpenOptions::default())
            .err()
            .unwrap();
        assert_eq!(e.code, "E_UNSUPPORTED_VALUE");
        assert!(e.detail.contains("date") && e.detail.contains("closed"));
        assert!(!e.detail.contains("2024"), "leaked value: {}", e.detail);
    }

    #[test]
    fn max_rows_refuses() {
        let e = read(
            &fx("sample.csv"),
            &OpenOptions {
                sheet: None,
                max_rows: Some(2),
            },
        )
        .err()
        .unwrap();
        assert_eq!(e.code, "E_TOO_LARGE");
    }

    #[test]
    fn cells_feed_spine_rules_composite_key_contract() {
        // Same adapter shape a consumer would write: Cell tuples -> spine-rules key checks.
        spine_rules::composite_key::assert_ordered_tuple_semantics(|raw| {
            raw.iter()
                .map(|c| {
                    let trimmed =
                        spine_rules::composite_key::reference_key_tuple(std::slice::from_ref(c))?;
                    Some(Cell::Text(trimmed.into_iter().next()?))
                })
                .collect::<Option<Vec<_>>>()
        });
    }
}
