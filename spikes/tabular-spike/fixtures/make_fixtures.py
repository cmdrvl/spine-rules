# Regenerates fixtures/*. Independent writers (pyarrow, openpyxl, csv, json) on purpose:
# the readers under test must not be validated against their own output.
# uv run --with pyarrow --with openpyxl python fixtures/make_fixtures.py fixtures
import sys, json, csv, datetime
import pyarrow as pa, pyarrow.parquet as pq, openpyxl
out = sys.argv[1]
rows = [
  (1, "East",  100.5,  True,  "NA"),
  (2, "West",  2000.0, False, "NULL"),
  (3, " North ", 0.1,  True,  None),
]
hdr = ["id","region","amount","active","note"]
def canon(v):
    if v is None: return ""
    if isinstance(v,bool): return "true" if v else "false"
    if isinstance(v,float) and v==int(v): return str(int(v))
    return str(v)
with open(f"{out}/sample.csv","w",newline="") as f:
    w=csv.writer(f,lineterminator="\n"); w.writerow(hdr)
    for r in rows: w.writerow([canon(x) for x in r])
with open(f"{out}/sample.tsv","w",newline="") as f:
    w=csv.writer(f,delimiter="\t",lineterminator="\n"); w.writerow(hdr)
    for r in rows: w.writerow([canon(x) for x in r])
with open(f"{out}/sample.jsonl","w") as f:
    for r in rows: f.write(json.dumps(dict(zip(hdr,r)))+"\n")
t = pa.table({h:[r[i] for r in rows] for i,h in enumerate(hdr)},
  schema=pa.schema([("id",pa.int64()),("region",pa.string()),("amount",pa.float64()),("active",pa.bool_()),("note",pa.string())]))
pq.write_table(t, f"{out}/sample.parquet", compression="snappy")
wb=openpyxl.Workbook(); ws=wb.active; ws.title="loans"; ws.append(hdr)
for r in rows: ws.append(list(r))
wb.create_sheet("notes").append(["scratch"])
wb.save(f"{out}/sample.xlsx")
import datetime as _dt
pq.write_table(pa.table({"id":[1],"closed":pa.array([_dt.date(2024,5,17)],pa.date32())}), f"{out}/unsupported_date.parquet")
