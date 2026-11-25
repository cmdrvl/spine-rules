//! `lock.v0` shared verifier: the fixtures prove the reference implementation, and
//! planted wrong implementations (the drift seen in the spine) must FAIL conformance.

use std::panic::{AssertUnwindSafe, catch_unwind};

use spine_rules::lockfile::{
    Expect, HashAlgorithm, LockRefusal, MemberVerdict, REAL_BLAKE3_LOCK, REAL_SHA256_LOCK,
    conformance_cases, parse, reference_check, run_conformance, verify_member,
};

const LOANS: &[u8] = spine_rules::lockfile::LOANS_CSV.as_bytes();

#[test]
fn the_reference_implementation_passes_every_case() {
    run_conformance(reference_check);
}

#[test]
fn both_algorithms_verify_with_the_members_own_algorithm() {
    for (lock, expected) in [
        (REAL_SHA256_LOCK, HashAlgorithm::Sha256),
        (REAL_BLAKE3_LOCK, HashAlgorithm::Blake3),
    ] {
        let lock = parse(lock).expect("real lock parses");
        let verdict = verify_member(&lock, "loans.csv", LOANS).expect("usable lock");
        assert!(
            matches!(&verdict, MemberVerdict::Verified { algorithm, .. } if *algorithm == expected),
            "expected Verified with {expected:?}, got {verdict:?}"
        );
    }
}

#[test]
fn drift_reports_the_locks_prefix_on_both_sides() {
    let mut tampered = LOANS.to_vec();
    tampered[0] ^= 1;
    for (lock, prefix) in [(REAL_SHA256_LOCK, "sha256:"), (REAL_BLAKE3_LOCK, "blake3:")] {
        let lock = parse(lock).expect("real lock parses");
        let verdict = verify_member(&lock, "loans.csv", &tampered).expect("usable lock");
        let drifted = matches!(
            &verdict,
            MemberVerdict::Drift { expected, observed, .. }
                if expected.starts_with(prefix) && observed.starts_with(prefix) && expected != observed
        );
        assert!(
            drifted,
            "expected Drift with {prefix} on both sides, got {verdict:?}"
        );
    }
}

#[test]
fn refusals_are_typed() {
    assert!(matches!(parse("{"), Err(LockRefusal::Malformed(_))));
    assert!(matches!(
        parse(r#"{"version":"lock.v1","members":[]}"#),
        Err(LockRefusal::UnsupportedVersion(_))
    ));
    let lock =
        parse(r#"{"version":"lock.v0","members":[{"path":"loans.csv","bytes_hash":"md5:00"}]}"#)
            .expect("parses");
    assert!(matches!(
        verify_member(&lock, "loans.csv", LOANS),
        Err(LockRefusal::UnknownAlgorithm { .. })
    ));
}

/// The drift this bead exists for: a verifier that hardcodes sha256 (like benchmark's did).
fn sha256_only(lock_json: &str, input_path: &str, bytes: &[u8]) -> Expect {
    let Ok(lock) = parse(lock_json) else {
        return Expect::Refused;
    };
    let matches = spine_rules::lockfile::candidates(&lock, input_path);
    let Some(member) = matches.first() else {
        return Expect::NotLocked;
    };
    if member.bytes_hash == HashAlgorithm::Sha256.digest(bytes) {
        Expect::Verified
    } else {
        Expect::Drift
    }
}

/// A verifier that picks the first basename match even when it is ambiguous.
fn guesses_ambiguous_basenames(lock_json: &str, input_path: &str, bytes: &[u8]) -> Expect {
    let Ok(lock) = parse(lock_json) else {
        return Expect::Refused;
    };
    let name = input_path.rsplit('/').next().unwrap_or(input_path);
    let Some(member) = lock.members.iter().find(|m| m.path.ends_with(name)) else {
        return Expect::NotLocked;
    };
    match HashAlgorithm::of(&member.bytes_hash) {
        None => Expect::Refused,
        Some(alg) if alg.digest(bytes) == member.bytes_hash => Expect::Verified,
        Some(_) => Expect::Drift,
    }
}

fn conformance_failure(check: fn(&str, &str, &[u8]) -> Expect) -> String {
    let payload = catch_unwind(AssertUnwindSafe(|| run_conformance(check)))
        .expect_err("a wrong implementation must fail conformance");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

#[test]
fn a_sha256_only_verifier_is_caught_on_the_blake3_cases() {
    let message = conformance_failure(sha256_only);
    assert!(
        message.contains("blake3 real lock: matching bytes verify"),
        "{message}"
    );
    assert!(
        !message.contains("sha256 real lock: matching bytes verify"),
        "the sha256 case should have passed: {message}"
    );
    assert!(message.contains("unknown algorithm"), "{message}");
}

#[test]
fn a_verifier_that_guesses_ambiguous_basenames_is_caught() {
    let message = conformance_failure(guesses_ambiguous_basenames);
    assert!(
        message.contains("two members with one basename"),
        "{message}"
    );
}

#[test]
fn the_case_list_covers_pass_drift_and_unknown_for_both_algorithms() {
    let names: Vec<String> = conformance_cases().into_iter().map(|c| c.name).collect();
    for needed in [
        "sha256 real lock: matching bytes verify",
        "sha256 real lock: a changed byte is drift",
        "blake3 real lock: matching bytes verify",
        "blake3 real lock: a changed byte is drift",
        "unknown algorithm prefix is refused, not guessed",
    ] {
        assert!(
            names.iter().any(|name| name == needed),
            "missing case: {needed}"
        );
    }
}
