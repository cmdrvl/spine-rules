# Hashing policy and the shared `lock.v0` verifier

## Policy

- **`blake3:` is the canonical content hash inside the spine.** New tools and new internal
  artifacts hash bytes with BLAKE3 and write `blake3:<lowercase hex>`.
- **`sha256:` is supported for external and legacy interoperability** (published checksums,
  older locks, anything a counterparty already hashes with SHA-256). A tool must accept it
  wherever it accepts `blake3:`, and must never silently re-hash with the other algorithm.
- The algorithm is named by the prefix, `<algorithm>:<lowercase hex>`. **Verification always
  uses the algorithm the lock names**, not the verifying tool's favourite. Any other prefix is
  refused, never guessed or skipped.
- Hashes are over the **raw file bytes**.

`hashbytes --algorithm sha256|blake3` writes either form and `lock` records it verbatim in
`members[].bytes_hash`, so a real `lock.v0` file can carry either.

## Why this exists

Lock verification had drifted per tool: one verifier hardcoded `sha256:` and failed every
`blake3:` lock; another read a private `lock.input.v1` / `content_hash` format that `lock`
never writes. The shared helper and fixtures below make one contract.

## The shared verifier: `spine_rules::lockfile`

- `parse(lock_json)`: accepts `lock.v0` only; anything else is a typed `LockRefusal`.
- `verify_member(&lock, input_path, bytes)`: finds the member (exact path, else a unique
  basename; an ambiguous basename matches nothing) and returns `Verified`, `Drift` or
  `NotLocked`, or a `LockRefusal` (`UnknownAlgorithm`, `Malformed`, `UnsupportedVersion`).
- `conformance_cases()` and `run_conformance(check)`: golden cases for **both** algorithms
  (pass, drift, ambiguous basename) plus not-locked, unknown algorithm prefix, a member with
  no `bytes_hash`, and an unknown lock version. The fixtures are real output of
  `vacuum data | hashbytes --algorithm <alg> | lock`, not hand-written JSON.

A tool keeps its own `--lock` code if it wants and proves it against the cases:

```rust
#[test]
fn lock_check_conforms() {
    spine_rules::lockfile::run_conformance(|lock_json, input_path, bytes| {
        // Call the tool's own verifier on (lock_json, input_path, bytes) and return
        // Expect::Verified, Expect::Drift, Expect::NotLocked or Expect::Refused.
        my_tool::check_lock(lock_json, input_path, bytes)
    });
}
```

`run_conformance` names every case a verifier gets wrong. A SHA-256-only verifier fails the
`blake3` cases, and a verifier that guesses an ambiguous basename fails the ambiguous cases
(both are planted-negative tests in `tests/lockfile.rs`).

## What this does not cover

- The lock's own `lock_hash` self-hash is not checked here.
- Streaming large files: `verify_member` takes bytes. A tool that streams must hash with
  `HashAlgorithm::of(...)` equivalents itself and map the result for the conformance cases.
- No tool has been migrated to call this or `run_conformance` yet. `verify`'s own
  `lock_check.rs` implements the same contract independently and its tests cover the same
  situations separately; `benchmark`, `shape`, `rvl`, `profile` and `pack` are unchecked here.
