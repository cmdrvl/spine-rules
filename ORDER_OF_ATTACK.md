# Spine fix swarm — order of attack

Source: operator master plan, 2026-11-24. Sequenced by critical path and fan-out leverage so no repo is touched twice.
Rule: fix first-try traps and the flagship shape → rvl seam first, then land the two spine-rules upstream contracts before fanning out across individual tools.

Swarm rules: targeted tests only (no full sweeps); no needless rebuilds; coordinate via Agent Mail (reserve exact files, `[bd-xxx]` subjects, bead id as thread_id); honest-credit floor per `/just-say-no-to-process-porn-and-ceremony`; 10-minute operator check-in.

Bead ids are `<repo>/<id>`.

## Wave 0 — Stop the bleeding (today, ~1–2 hrs, zero risk)

Fixes every broken first-try experience an agent or human hits right now.

1. `homebrew-tap/bd-yy5` + `.github/bd-xk6` → `.github/bd-yn3` (all closed; the `.github` bead tracker was removed 2026-11-24)
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
- `homebrew-tap/bd-2rl` (was `.github/bd-63g`; moved 2026-11-24 when the `.github` tracker was removed) → `homebrew-tap/bd-qg4` — open-source `cmdrvl` umbrella CLI; build last so it wraps the unified contracts.
- `canon/bd-jefv` — split core canon runtime from canon-workbench; after `canon/bd-c98n` and canon CI fix `bd-vjgi`.

## First 5 pickups

1. `homebrew-tap/bd-yy5` + `.github/bd-xk6`
2. `rvl/bd-16z`
3. `veil/bd-2ll` + `veil/bd-dzu`
4. `spine-rules/bd-m1p`, `bd-yes`, `bd-6g4`
5. `spine-rules/bd-e0g`

## Overnight operating mode (operator decision, 2026-11-25)

- **4 panes max, slow and steady.** The machine overloaded at 6 panes plus verifier builds (load ~230 on 12 cores). Each pane runs ONE cargo command at a time with `CARGO_BUILD_JOBS=3`; no clippy and tests concurrently. When a pane finishes its list it stops; the orchestrator assigns the next queue item.
- **Verify before close, one verifier at a time, no full-sweep builds.** A built-binary check that needs a heavy build (benchmark's DuckDB) waits for a quiet machine.
- **Clean `target/` when done.** When a repo's last bead is verified and no pane is working in it, run `cargo clean --manifest-path <repo>/Cargo.toml` (scoped to that repo's `target/`, no `rm`). Never in a repo with a running build or uncommitted work.
- **Off limits:** veil and airlock (finished after in-flight work; no new beads), canon and naic (other owners).

### Lanes (one pane per lane; work the items IN ORDER, one at a time)
Each item: `br show` first (acceptance + blockers), claim, real change + planted-negative test, targeted tests, commit + push, evidence comment, never `br close`. An item whose bead says BLOCKED or whose dependency is not closed is skipped and reported, not forced. Items marked (wait) need the dependency named.

- **Lane A (assess / contracts / tabular), Sonnet:** assess bd-zka; spine-rules bd-q2e (tabular v1: temporal+decimal types, streaming, zip-bomb guard, dialect parity; repo github.com/cmdrvl/tabular, public); shape bd-o7y2 (adopt tabular, needs q2e Phase 1 at minimum for v0 types); rvl bd-jf7 (adopt tabular); profile bd-1t0 then bd-47o (adopt tabular, zero-retention inspect); fingerprint bd-jxd (adopt tabular; prove calamine 0.38 compiles first); rvl bd-1gj, shape bd-tqea, profile bd-375, lock bd-1mv, pack bd-i2l (generalized CLI typo recovery: one consistent behavior across tools).
- **Lane B (verify), Sonnet:** verify bd-dbr (in flight); verify bd-5oi (nullity-only rules over temporal columns need no temporal protocol value); verify bd-zoe (zero-ceremony inline rule check mode); verify bd-l2l (XLSX input, wait tabular v1 temporal); benchmark bd-526 (XLSX candidate input, wait tabular v1 temporal); benchmark bd-22v (claim-slot benchmark fixture family).
- **Lane C (sweep, Luna):** the review fixes on pack bd-pom, hash bd-a3x, fingerprint bd-zbn, profile bd-ps0 (stale pin, v0 manifests, dropped assertions); lock bd-16f then bd-rpf; fingerprint bd-1d9 (behavior-preserving CLI/library entrypoint); fingerprint bd-27o (harden peek streaming and shape vocabulary, wait tabular); pack bd-80p (`pack render`: offline self-contained HTML/Markdown dossier over a seal); pack bd-kfu (`pack index` and `pack query`).
- **Lane D (decoding / benchmark / shape / rvl), Luna:** decoding bd-n5s (in flight); decoding bd-4p4 (default redaction and --explicit for sensitive claim values); vacuum bd-8bv (BLOCKED: find the blocker first, ask before proceeding); benchmark bd-u61 (17 files of uncommitted work in the tree: review and finish, do not redo); shape bd-pitd (P1: default redacted output leaks column names through `reasons`; deliberate golden changes, say so); rvl bd-u1k (domain-agnostic non-numeric comparison mode); assess bd-84u (consume rvl non-numeric artifacts, wait rvl bd-u1k); decoding bd-ou1 (BDC claims convergence contract; large, last).

### Orchestrator-side queue
- **Verification backlog (one verifier at a time, light):** shape bd-k3eu, fingerprint bd-zbn, profile bd-ps0, pack bd-pom, hash bd-a3x, benchmark bd-dk9 (built-binary checks: --describe parity, clap parity; wait for a quiet machine), verify bd-dbr, decoding bd-n5s, lock bd-16f/bd-rpf, assess bd-zka.
- **Wave 4 / later:** homebrew-tap bd-2rl then bd-qg4 (umbrella CLI), bd-5ov (org audit, re-check stale findings); spine-rules bd-2tw (conformance over evidence-protocol objects + cargo-deny bound); tap releases per tool once all its new beads are closed and verified.
- **Not scheduled, on purpose:** the "Doc Audit: add agent-specific harness docs" P3 beads (benchmark bd-2l4, decoding bd-130, lock bd-396, pack bd-2zw): process artifacts, same call as the `.github` doc beads; hash bd-174 (renames a GitHub repo: needs the operator); pack bd-3ue epic and children and fingerprint bd-3hf/bd-lxy (other owners / in progress elsewhere); lock bd-36t, verify bd-286 (deferred); spine-rules bd-9m0 (umbrella: closes when every tool has adopted the contract).
