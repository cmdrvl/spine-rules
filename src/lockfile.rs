//! Shared `lock.v0` member verification, so every spine tool that accepts `--lock` agrees
//! on the same contract. Tools can call [`verify_member`] directly, or keep their own
//! implementation and prove it against [`conformance_cases`] with [`run_conformance`].
//!
//! Contract (`lock.v0`, as written by `vacuum | hashbytes | lock`):
//!
//! - `members[].bytes_hash` is `<algorithm>:<lowercase hex>` of the RAW file bytes.
//!   Supported algorithms: `blake3` (canonical across the spine) and `sha256` (external
//!   and legacy interoperability). Anything else is refused, never guessed.
//! - A member matches an input by exact path, otherwise by a unique basename. Real locks
//!   store paths relative to the vacuum root, so basename matching is the normal case.
//!   An ambiguous basename matches nothing (it cannot be verified safely).
//! - The input's bytes are hashed with the MEMBER'S algorithm, never the tool's favourite.
//!   Different bytes are drift. No member is "not locked".
//! - Not checked here: the lock's own `lock_hash` self-hash.

use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Algorithms a `bytes_hash` may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Blake3,
    Sha256,
}

impl HashAlgorithm {
    /// Algorithm named by the prefix of `hash` (`blake3:` or `sha256:`), if supported.
    pub fn of(hash: &str) -> Option<Self> {
        match hash.split_once(':')?.0 {
            "blake3" => Some(Self::Blake3),
            "sha256" => Some(Self::Sha256),
            _ => None,
        }
    }

    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Blake3 => "blake3",
            Self::Sha256 => "sha256",
        }
    }

    /// `<algorithm>:<lowercase hex>` of `bytes`.
    pub fn digest(self, bytes: &[u8]) -> String {
        match self {
            Self::Blake3 => format!("blake3:{}", blake3::hash(bytes).to_hex()),
            Self::Sha256 => format!("sha256:{:x}", Sha256::digest(bytes)),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LockMember {
    pub path: String,
    pub bytes_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LockV0 {
    pub version: String,
    #[serde(default)]
    pub lock_hash: Option<String>,
    pub members: Vec<LockMember>,
}

/// Why a lock file could not be used at all (as opposed to an input failing it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockRefusal {
    /// Not JSON, or not shaped like `lock.v0` (for example a member without `bytes_hash`).
    Malformed(String),
    UnsupportedVersion(String),
    /// A matched member names an algorithm other than `blake3` or `sha256`.
    UnknownAlgorithm {
        member: String,
        bytes_hash: String,
    },
}

/// Result of checking one input against one lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberVerdict {
    Verified {
        member: String,
        algorithm: HashAlgorithm,
    },
    Drift {
        member: String,
        expected: String,
        observed: String,
    },
    NotLocked,
}

pub fn parse(lock_json: &str) -> Result<LockV0, LockRefusal> {
    let lock: LockV0 = serde_json::from_str(lock_json)
        .map_err(|error| LockRefusal::Malformed(error.to_string()))?;
    if lock.version != "lock.v0" {
        return Err(LockRefusal::UnsupportedVersion(lock.version));
    }
    Ok(lock)
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or("")
}

/// Members that vouch for `input_path`: exact path match wins, else a unique basename.
pub fn candidates<'a>(lock: &'a LockV0, input_path: &str) -> Vec<&'a LockMember> {
    let normalized = input_path.replace('\\', "/");
    let normalized = normalized.strip_prefix("./").unwrap_or(&normalized);
    let exact: Vec<&LockMember> = lock
        .members
        .iter()
        .filter(|member| member.path == normalized)
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let wanted = basename(normalized);
    if wanted.is_empty() {
        return Vec::new();
    }
    let by_name: Vec<&LockMember> = lock
        .members
        .iter()
        .filter(|member| basename(&member.path) == wanted)
        .collect();
    let distinct: BTreeSet<&str> = by_name.iter().map(|member| member.path.as_str()).collect();
    if distinct.len() == 1 {
        by_name
    } else {
        Vec::new()
    }
}

/// Check `bytes` (the content of `input_path`) against `lock`.
pub fn verify_member(
    lock: &LockV0,
    input_path: &str,
    bytes: &[u8],
) -> Result<MemberVerdict, LockRefusal> {
    let matches = candidates(lock, input_path);
    if matches.is_empty() {
        return Ok(MemberVerdict::NotLocked);
    }
    let mut first_drift: Option<MemberVerdict> = None;
    for member in &matches {
        let Some(algorithm) = HashAlgorithm::of(&member.bytes_hash) else {
            return Err(LockRefusal::UnknownAlgorithm {
                member: member.path.clone(),
                bytes_hash: member.bytes_hash.clone(),
            });
        };
        let observed = algorithm.digest(bytes);
        if observed == member.bytes_hash {
            return Ok(MemberVerdict::Verified {
                member: member.path.clone(),
                algorithm,
            });
        }
        first_drift.get_or_insert(MemberVerdict::Drift {
            member: member.path.clone(),
            expected: member.bytes_hash.clone(),
            observed,
        });
    }
    Ok(first_drift.expect("a non-empty candidate set that never verified drifted"))
}

// ---------------------------------------------------------------------------
// Conformance fixtures: REAL output of `vacuum data | hashbytes --algorithm <alg> | lock`
// over `LOANS_CSV` (lock 0.6.0, hashbytes/vacuum 0.5.0), not hand-written JSON.
// ---------------------------------------------------------------------------

pub const LOANS_CSV: &str = include_str!("../fixtures/lock_v0/loans.csv");
pub const REAL_SHA256_LOCK: &str = include_str!("../fixtures/lock_v0/sha256.lock.json");
pub const REAL_BLAKE3_LOCK: &str = include_str!("../fixtures/lock_v0/blake3.lock.json");

/// What a tool's `--lock` check must conclude for a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    Verified,
    Drift,
    NotLocked,
    /// The lock itself is unusable (unknown algorithm, malformed, unknown version).
    Refused,
}

#[derive(Debug, Clone)]
pub struct ConformanceCase {
    pub name: String,
    pub lock_json: String,
    /// Path of the input as the tool sees it.
    pub input_path: &'static str,
    pub input_bytes: Vec<u8>,
    pub expect: Expect,
}

/// The cases every `--lock` implementation must agree on: pass, drift, ambiguous basename
/// for both algorithms, plus not-locked, unknown algorithm, malformed, and unknown version.
pub fn conformance_cases() -> Vec<ConformanceCase> {
    let good = LOANS_CSV.as_bytes().to_vec();
    let mut tampered = good.clone();
    let last = tampered.len() - 2;
    tampered[last] = if tampered[last] == b'9' { b'8' } else { b'9' };

    let mut cases = Vec::new();
    for (algorithm, lock) in [
        (HashAlgorithm::Sha256, REAL_SHA256_LOCK),
        (HashAlgorithm::Blake3, REAL_BLAKE3_LOCK),
    ] {
        let prefix = algorithm.prefix();
        let hash = algorithm.digest(&good);
        let two_members = format!(
            r#"{{"version":"lock.v0","members":[{{"path":"a/loans.csv","bytes_hash":"{hash}"}},{{"path":"b/loans.csv","bytes_hash":"{hash}"}}]}}"#
        );
        cases.push(ConformanceCase {
            name: format!("{prefix} real lock: matching bytes verify"),
            lock_json: lock.to_owned(),
            input_path: "loans.csv",
            input_bytes: good.clone(),
            expect: Expect::Verified,
        });
        cases.push(ConformanceCase {
            name: format!("{prefix} real lock: a changed byte is drift"),
            lock_json: lock.to_owned(),
            input_path: "loans.csv",
            input_bytes: tampered.clone(),
            expect: Expect::Drift,
        });
        cases.push(ConformanceCase {
            name: format!("{prefix}: two members with one basename are not locked"),
            lock_json: two_members,
            input_path: "loans.csv",
            input_bytes: good.clone(),
            expect: Expect::NotLocked,
        });
    }
    cases.push(ConformanceCase {
        name: "real lock, input not in it: not locked".to_owned(),
        lock_json: REAL_BLAKE3_LOCK.to_owned(),
        input_path: "other.csv",
        input_bytes: good.clone(),
        expect: Expect::NotLocked,
    });
    cases.push(ConformanceCase {
        name: "unknown algorithm prefix is refused, not guessed".to_owned(),
        lock_json: REAL_SHA256_LOCK.replace("sha256:973ef89c", "md5:973ef89c"),
        input_path: "loans.csv",
        input_bytes: good.clone(),
        expect: Expect::Refused,
    });
    cases.push(ConformanceCase {
        name: "member without bytes_hash is refused".to_owned(),
        lock_json: r#"{"version":"lock.v0","members":[{"path":"loans.csv"}]}"#.to_owned(),
        input_path: "loans.csv",
        input_bytes: good.clone(),
        expect: Expect::Refused,
    });
    cases.push(ConformanceCase {
        name: "unknown lock version is refused".to_owned(),
        lock_json: r#"{"version":"lock.v9","members":[]}"#.to_owned(),
        input_path: "loans.csv",
        input_bytes: good,
        expect: Expect::Refused,
    });
    cases
}

/// Run `check` over every conformance case and assert it reaches the expected conclusion.
/// `check(lock_json, input_path, input_bytes)` is the tool's own `--lock` logic reduced to
/// an [`Expect`]. Panics naming every case that disagrees.
pub fn run_conformance(check: impl Fn(&str, &str, &[u8]) -> Expect) {
    let failures: Vec<String> = conformance_cases()
        .into_iter()
        .filter_map(|case| {
            let got = check(&case.lock_json, case.input_path, &case.input_bytes);
            (got != case.expect)
                .then(|| format!("{}: expected {:?}, got {got:?}", case.name, case.expect))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "lock.v0 conformance failures:\n  - {}",
        failures.join("\n  - ")
    );
}

/// The reference implementation as a `check` function, so the fixtures prove themselves.
pub fn reference_check(lock_json: &str, input_path: &str, bytes: &[u8]) -> Expect {
    match parse(lock_json).and_then(|lock| verify_member(&lock, input_path, bytes)) {
        Ok(MemberVerdict::Verified { .. }) => Expect::Verified,
        Ok(MemberVerdict::Drift { .. }) => Expect::Drift,
        Ok(MemberVerdict::NotLocked) => Expect::NotLocked,
        Err(_) => Expect::Refused,
    }
}
