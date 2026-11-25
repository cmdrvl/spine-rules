# Spine fix swarm — order of attack

Source: operator master plan, 2026-11-24. Sequenced by critical path and fan-out leverage so no repo is touched twice.
Rule: fix first-try traps and the flagship shape → rvl seam first, then land the two spine-rules upstream contracts before fanning out across individual tools.

Swarm rules: targeted tests only (no full sweeps); no needless rebuilds; no Agent Mail (dropped by the operator 2026-11-25: one agent per repo, coordination by disjoint ownership, evidence in `br` comments and commit messages); honest-credit floor per `/just-say-no-to-process-porn-and-ceremony`; 10-minute operator check-in.

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
- `cmdrvl/cmd-w51` (`cmd-3w7`..`cmd-nal`, repo `github.com/cmdrvl/cmdrvl`) → `homebrew-tap/bd-qg4` — open-source `cmdrvl` umbrella porcelain CLI (with git-style `cmdrvl-<subcommand>` plugin dispatch and cross-repo E2E CI harness `cmd-nal`); `homebrew-tap/bd-2rl` superseded by `cmdrvl/cmd-w51`, `homebrew-tap/bd-qg4` packages the formula once published.
- `canon/bd-jefv` — split core canon runtime from canon-workbench; after `canon/bd-c98n` and canon CI fix `bd-vjgi`.

## First 5 pickups

1. `homebrew-tap/bd-yy5` + `.github/bd-xk6`
2. `rvl/bd-16z`
3. `veil/bd-2ll` + `veil/bd-dzu`
4. `spine-rules/bd-m1p`, `bd-yes`, `bd-6g4`
5. `spine-rules/bd-e0g`

## Overnight operating mode (operator decision, 2026-11-25)

- **P0 and P1 beads ONLY for the rest of the session (operator, 2026-11-25).** Every P2, P3 and P4 bead is out of the queue: do not start one, do not file one, do not "tidy" one. A genuinely new defect found while working is filed only if it is P0/P1; anything lower goes in a comment on the bead you are on.
- **4 panes max, slow and steady.** The machine overloaded at 6 panes plus verifier builds (load ~230 on 12 cores). Each pane runs ONE cargo command at a time with `CARGO_BUILD_JOBS=3` (and `CMAKE_BUILD_PARALLEL_LEVEL=3` for DuckDB); no clippy and tests concurrently. When a pane finishes its list it stops; the orchestrator assigns the next queue item.
- **Verify before close, one verifier at a time, no full-sweep builds.** A built-binary check that needs a heavy build (DuckDB in verify and benchmark) waits for a quiet machine. A bead that changes a manifest, schema or output shape is verified with the repo's whole `--lib` suite as well as its integration tests (the pack bd-pom miss).
- **Clean `target/` when done.** When a repo's last bead is verified and no pane is working in it, run `cargo clean --manifest-path <repo>/Cargo.toml` (scoped to that repo's `target/`, no `rm`). Never in a repo with a running build or uncommitted work. Never in verify or benchmark while DuckDB is being built there (it forces a full rebuild).
- **Off limits:** veil and airlock (finished after in-flight work; no new beads), canon and naic (other owners).

### Lanes (one pane per lane; work the items IN ORDER, one at a time) — P0/P1 only
Each item: `br show` first (acceptance + blockers), claim, real change + planted-negative test, targeted tests, commit + push, evidence comment, never `br close`. An item whose bead says BLOCKED or whose dependency is not closed is SKIPPED and reported, and the lane CONTINUES with the next item (do not stop at a blocked bead). Items marked (wait) need the dependency named. If a P1 bead turns out to be an epic that spans other owners' work, say so in a comment and skip it.

- **Lane A (contracts / tabular), Sonnet:** spine-rules bd-q2e (P1, tabular v1: temporal types done in phase 1; streaming, zip-bomb guard and fixtures pushed: finish the remaining acceptance, then report); shape bd-o7y2 (P1, adopt tabular); profile bd-1t0 then bd-47o (P1, adopt tabular, zero-retention inspect); fingerprint bd-jxd (P1, adopt tabular; prove calamine 0.38 compiles first); then the fingerprint tabular-family P1s: bd-hra (extend zero-retention `peek` to more formats), bd-yq3 (native xml/json/jsonl/parquet document formats), bd-h5o (zero-ceremony direct file/directory mode), bd-uai (`fingerprint materialize`), bd-41y (fingerprint v1 universal engine: likely an epic, last). rvl bd-jf7 is pushed and awaits verification.
- **Lane B (verify / spine-rules), Sonnet:** verify bd-dbr (P1, pushed: awaiting verification), bd-16h (P0: lock down DuckDB external I/O and reject non-read-only SQL in `query_zero_rows`; hostile-SQL negatives refused with no side effect), bd-q4d (P0: default redaction of failing-row output + `--explicit`), bd-pnr (P0: `--lock` for canonical `lock.v0` `bytes_hash` with `blake3:`/`sha256:`; test against a real lock from the installed `lock`), bd-ec5 (P1: wire verify into spine-rules via `golden_rules_suite!`); then spine-rules bd-u3f (P1: `assert_no_raw_cell_leak` helper, OPT-IN per tool via a fixture argument so existing suites keep passing), bd-ejf (P1: blake3-first hashing policy + shared `lock.v0` dual-algorithm verifier; coordinate with Lane C's lock work), bd-2tw (P1: conformance over `evidence-protocol` objects + cargo-deny bound); then (wait: tabular v1 temporal types) verify bd-l2l and benchmark bd-526 (P1, XLSX input).
- **Lane C (pack / lock / profile), Luna:** pack bd-pom (P1, REOPENED: two stale unit tests in `src/operator.rs` after the operator.v1 migration; fix first, whole `--lib` suite green); lock bd-rpf (P1, finish the preserved partial edits against the current spine-rules pin); lock bd-edy (P1, pushed: awaiting verification); pack bd-80p (P1, `pack render`: offline self-contained HTML/Markdown dossier over a seal); then the profile v1 P1s, read each bead first and say if one is an epic: bd-3om (universal L2 dataset contract), bd-87o (bounded-memory composite key discovery), bd-cgd (optional per-column type/nullable/unit/sensitivity in the L2 contract), bd-tjc (`profile evolve`).
- **Lane D (decoding / benchmark / shape / vacuum / rvl), Luna:** benchmark bd-w6e (P1, pushed: awaiting verification), then benchmark bd-c6a (P1, composite keys/profile support, ASCII-trim parity, default `--explicit` redaction), vacuum bd-3yk (P1, symlink root-containment by default), decoding bd-1pv (P1, operator decision 2026-11-25, option 1: replace the hash markers in redacted output with deterministic ordinal refs v1, v2, ... by default, plus an opt-in keyed HMAC via `DECODING_REDACTION_KEY`; a random per-run salt was rejected because it breaks reproducible output), benchmark bd-u61 (P1, uncommitted files in the tree: review and finish, do not redo; wire into golden_rules_suite!), shape bd-pitd (P1: default redacted output leaks column names through `reasons`; deliberate golden changes, say so), vacuum bd-8bv (P1, BLOCKED: find the blocker; skip if still blocked), rvl bd-u1k (P1, domain-agnostic non-numeric comparison mode; use `cargo +stable`), decoding bd-ou1 (P1, BDC claims convergence contract; large, last).

### P0 / P1 Security & Integrity Priority Insertions (2026-11-25 source audit)
Slot ahead of lower-priority polish within their existing lanes (zero new panes, respects off-limits rule on veil/airlock/canon/naic):
- **Lane B (`verify` P0s + conformance):** `verify/bd-16h` (P0 lock down DuckDB external I/O + read-only SQL in `query_zero_rows`), `verify/bd-q4d` (P0 default redaction + `--explicit`), `verify/bd-pnr` (P0 fix `--lock` for canonical `lock.v0` `bytes_hash` with `blake3:`/`sha256:`), then `verify/bd-ec5` (P1 wire `verify` into `spine-rules` after `bd-dbr`).
- **Lane C (`pack` P0s/P1s + `lock` symlink):** `pack/bd-iwr` (P0 fix fail-open `pack verify` on unreadable members/dirs), `pack/bd-5pf` (P0 block unsafe-path hashing oracle + symlink-following extra walk), `pack/bd-ok6` (P1 reject `CurDir` `./manifest.json` overwrite + bound tar extraction), `pack/bd-69n` (P1 fix `output_parent` for bare relative paths), `lock/bd-edy` (P1 symlink root-containment in `lock verify`).
- **Lane D (`shape`/`benchmark`/`vacuum` P1s):** `shape/bd-1vap` (P1 fail on 0% key overlap + `--min-coverage`), `benchmark/bd-w6e` (P1 `blake3:` in `--lock`), `benchmark/bd-c6a` (P1 composite keys + default `--explicit` redaction), `vacuum/bd-3yk` (P1 symlink root-containment by default).
- **Shared crate ready (for next veil/airlock cycle):** `github.com/cmdrvl/boundary-detectors` v0.1.0 is live, public, and CI-green at `0d4dd36fc07836dc67becf0647322c5ddd7dc6e8`.

### Orchestrator-side queue (P0/P1 only)
- **Verification backlog (one verifier at a time, light; whole `--lib` suite when a manifest/schema/output shape changed):** pack bd-iwr/bd-5pf/bd-ok6/bd-69n (closed 2026-11-25), pack bd-pom (after its fix), lock bd-edy, lock bd-rpf, shape bd-1vap and bd-k3eu, benchmark bd-w6e and bd-dk9 (built-binary checks wait for a quiet machine), verify bd-dbr and the verify P0s once they land, rvl bd-jf7, spine-rules bd-q2e, decoding bd-1pv.
- **Closed when its children are done:** spine-rules bd-9m0 (umbrella: closes when every tool has adopted the contract).
- **Tap releases:** per tool, once all of that tool's new P0/P1 beads are closed and verified; state exactly what would be published and wait for the operator.
- **Dropped by the P0/P1-only rule (do not start):** verify bd-5oi, bd-zoe; benchmark bd-22v; pack bd-kfu; fingerprint bd-1d9, bd-27o; the generalized typo-recovery beads (shape bd-tqea, profile bd-375, lock bd-1mv, pack bd-i2l); assess bd-84u; homebrew-tap bd-2rl, bd-qg4, bd-5ov; the "Doc Audit: add agent-specific harness docs" beads (benchmark bd-2l4, decoding bd-130, lock bd-396, pack bd-2zw); hash bd-174 (repo rename: needs the operator); `cmdrvl/cmd-w51` umbrella CLI (other repo and owner).
- **Not mine to assign:** pack bd-3ue epic and bd-3ue.4 (other owner), fingerprint bd-3hf and bd-lxy (in progress elsewhere), fingerprint bd-2lg (tombstone), lock bd-36t and verify bd-286 (deferred), veil, airlock, canon and naic repos.
