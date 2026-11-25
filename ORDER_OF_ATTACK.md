# Spine fix swarm — order of attack

Source: operator master plan, 2026-11-24. Sequenced by critical path and fan-out leverage so no repo is touched twice.
Rule: fix first-try traps and the flagship shape → rvl seam first, then land the two spine-rules upstream contracts before fanning out across individual tools.

Swarm rules: targeted tests only (no full sweeps); no needless rebuilds; coordinate via Agent Mail (reserve exact files, `[bd-xxx]` subjects, bead id as thread_id); honest-credit floor per `/just-say-no-to-process-porn-and-ceremony`; 10-minute operator check-in.

Bead ids are `<repo>/<id>`.

## Wave 0 — Stop the bleeding (today, ~1–2 hrs, zero risk)

Fixes every broken first-try experience an agent or human hits right now.

1. `homebrew-tap/bd-yy5` + `.github/bd-xk6` → `.github/bd-yn3`
   - Fix broken `brew install` command (cmdrvl-hash, cmdrvl-benchmark), correct benchmark binary name, `~/.epistemic` → `~/.cmdrvl`, document `--explicit`, add `--robot-triage` / `capabilities --json` / robot-docs guide to SYSTEM_PROMPT.md and AGENT_PROMPT.md.
2. `shape/bd-j95p`, `shape/bd-c5ro`, `rvl/bd-qyf` (parallel)
   - Sync `shape/operator.json` (0.8.0 → 0.9.0), resolve its deferred `--lock` stub, document default `--explicit` zero-retention redaction on both flagship READMEs.
3. Quick hygiene sweeps (parallel, ~15 min each): `benchmark/bd-r57`, `vacuum/bd-5o0`, `canon/bd-bqzc`, `decoding/bd-ani`.

## Wave 1 — Highest-ROI single-tool unlocks (this week; no upstream blockers)

4. `rvl/bd-16z` — overlap / added-removed key alignment mode. Closes shape (COMPATIBLE) → rvl (E_KEY_MISMATCH) dead end on added/removed rows.
5. `veil/bd-2ll` + `veil/bd-dzu` — add verify/assess/benchmark/airlock to `authorized_tools`; decouple veil from Claude Code + dcg.

## Wave 2 — Two upstream contracts (once, before fan-out)

Hold all per-tool spine-rules v2 and tabular-format beads until these land.

6. spine-rules contract trio (parallel, unblocks `spine-rules/bd-9m0`):
   - `spine-rules/bd-m1p` — unify operator.json (`--describe`) and `capabilities --json`.
   - `spine-rules/bd-yes` — unify doctor schema, unhealthy exit code (1), flag aliases (`--out`, `--capsule-dir`), NO_COLOR / SOURCE_DATE_EPOCH.
   - `spine-rules/bd-6g4` — resolve exit-code overload (assess BLOCK vs REFUSAL, empty witness query = 0, rvl `set -e` guidance).
7. `spine-rules/bd-e0g` — design shared `cmdrvl-tabular` crate (CSV, TSV, JSONL, Parquet, XLSX).

## Wave 3 — Parallel fan-out (after Wave 2)

Track A — format parity and zero-retention inspection:
- `shape/bd-o7y2` + `rvl/bd-jf7` (Parquet/JSONL/XLSX on the flagship pair)
- `profile/bd-1t0` → `profile/bd-47o`, plus `fingerprint/bd-jxd`
- `verify/bd-l2l`, `benchmark/bd-526`, `canon/bd-ko97`

Track B — governance and boundary contract alignment:
- `assess/bd-fa1` → `assess/bd-wtu`
- `verify/bd-dbr` (doctor exit 2→1, empty witness query 1→0)
- `airlock/al-hd5` → `airlock/al-n9f`
- Close existing `benchmark/bd-dk9`, `veil/bd-t94`, `decoding/bd-199` → unblocks `benchmark/bd-u61`, `veil/bd-bf2`, `decoding/bd-n5s`
- `canon/bd-c98n`
- Light alignment sweep: `rvl/bd-59w` + `rvl/bd-k3c`, `shape/bd-k3eu`, `profile/bd-ps0`, `fingerprint/bd-zbn`, `vacuum/bd-8bv`, `hash/bd-a3x`, `lock/bd-rpf` (after bd-16f), `pack/bd-pom`

## Wave 4 — Porcelain and structural polish (after contracts stabilize)

- `verify/bd-zoe` — zero-ceremony inline/ad-hoc checks.
- `.github/bd-63g` → `homebrew-tap/bd-qg4` — open-source `cmdrvl` umbrella CLI; build last so it wraps the unified contracts.
- `canon/bd-jefv` — split core canon runtime from canon-workbench; after `canon/bd-c98n` and canon CI fix `bd-vjgi`.

## First 5 pickups

1. `homebrew-tap/bd-yy5` + `.github/bd-xk6`
2. `rvl/bd-16z`
3. `veil/bd-2ll` + `veil/bd-dzu`
4. `spine-rules/bd-m1p`, `bd-yes`, `bd-6g4`
5. `spine-rules/bd-e0g`
