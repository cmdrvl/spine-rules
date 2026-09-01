//! Shared composite-key conformance fixtures and assertions.
//!
//! This module is test infrastructure. Consumers adapt their internal key
//! constructor to [`assert_ordered_tuple_semantics`] without depending on a
//! particular runtime representation.

use std::fmt::Debug;

use serde_json::Value;

/// Raw ordered key components before per-component normalization.
pub type RawKeyTuple = Vec<Vec<u8>>;

/// Named fixtures covering the cross-tool composite-key contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositeKeyFixtures {
    /// Canonical tuple used as the target of ASCII-trim checks.
    pub canonical: RawKeyTuple,
    /// Same tuple with leading/trailing ASCII spaces and tabs.
    pub ascii_padded: RawKeyTuple,
    /// A tuple whose secondary component is empty after ASCII trimming.
    pub incomplete_secondary: RawKeyTuple,
    /// Missing-looking strings that remain literal key bytes.
    pub missing_tokens: [RawKeyTuple; 2],
    /// Distinct tuples sharing their first component.
    pub repeated_first: [RawKeyTuple; 2],
    /// Distinct tuples that collide if flattened with a `0xff` sentinel.
    pub sentinel_collision: [RawKeyTuple; 2],
    /// A valid tuple containing non-UTF-8 bytes.
    pub non_utf8: RawKeyTuple,
    /// The same full tuple twice, which must compare equal.
    pub duplicate_full_tuple: [RawKeyTuple; 2],
    /// Rows deliberately out of order.
    pub reordered_rows: Vec<RawKeyTuple>,
    /// Expected structural lexicographic order for `reordered_rows`.
    pub lexicographic_rows: Vec<RawKeyTuple>,
}

/// Return a fresh, reusable fixture corpus for consumer tests.
pub fn composite_key_fixtures() -> CompositeKeyFixtures {
    CompositeKeyFixtures {
        canonical: tuple(&[b"A", b"East"]),
        ascii_padded: tuple(&[b" \tA ", b"\tEast\t"]),
        incomplete_secondary: tuple(&[b"A", b" \t "]),
        missing_tokens: [tuple(&[b"NA", b"NULL"]), tuple(&[b"N/A", b"null"])],
        repeated_first: [tuple(&[b"A", b"East"]), tuple(&[b"A", b"West"])],
        sentinel_collision: [tuple(&[b"a\xff", b"b"]), tuple(&[b"a", b"\xffb"])],
        non_utf8: tuple(&[b"\xff", b"unit"]),
        duplicate_full_tuple: [tuple(&[b"D", b"1"]), tuple(&[b"D", b"1"])],
        reordered_rows: vec![
            tuple(&[b"B", b"1"]),
            tuple(&[b"A", b"2"]),
            tuple(&[b"A", b"1"]),
        ],
        lexicographic_rows: vec![
            tuple(&[b"A", b"1"]),
            tuple(&[b"A", b"2"]),
            tuple(&[b"B", b"1"]),
        ],
    }
}

/// Reference normalization: ASCII-trim every component and reject the whole
/// tuple when any normalized component is empty.
pub fn reference_key_tuple(components: &[Vec<u8>]) -> Option<RawKeyTuple> {
    components
        .iter()
        .map(|component| {
            let trimmed = ascii_trim(component);
            (!trimmed.is_empty()).then(|| trimmed.to_vec())
        })
        .collect()
}

/// Assert a consumer key constructor follows the shared ordered-tuple rules.
///
/// The adapter returns `None` for an incomplete tuple and `Some(K)` otherwise.
/// `K` remains consumer-defined so private structural key types can be tested.
pub fn assert_ordered_tuple_semantics<K, F>(mut normalize: F)
where
    K: Debug + Eq + Ord,
    F: FnMut(&[Vec<u8>]) -> Option<K>,
{
    let fixtures = composite_key_fixtures();

    assert_eq!(
        normalize(&fixtures.ascii_padded),
        normalize(&fixtures.canonical),
        "composite-key contract: ASCII-trim each component independently"
    );
    assert!(
        normalize(&fixtures.incomplete_secondary).is_none(),
        "composite-key contract: any empty component makes the tuple incomplete"
    );

    let missing_a = require_key(&mut normalize, &fixtures.missing_tokens[0]);
    let missing_b = require_key(&mut normalize, &fixtures.missing_tokens[1]);
    assert_ne!(
        missing_a, missing_b,
        "composite-key contract: missing-token strings remain literal key bytes"
    );

    assert_ne!(
        require_key(&mut normalize, &fixtures.repeated_first[0]),
        require_key(&mut normalize, &fixtures.repeated_first[1]),
        "composite-key contract: identity includes every component"
    );
    assert_ne!(
        require_key(&mut normalize, &fixtures.sentinel_collision[0]),
        require_key(&mut normalize, &fixtures.sentinel_collision[1]),
        "composite-key contract: component boundaries must remain structural"
    );
    let _ = require_key(&mut normalize, &fixtures.non_utf8);
    assert_eq!(
        require_key(&mut normalize, &fixtures.duplicate_full_tuple[0]),
        require_key(&mut normalize, &fixtures.duplicate_full_tuple[1]),
        "composite-key contract: duplicate full tuples compare equal"
    );

    let mut actual = fixtures
        .reordered_rows
        .iter()
        .map(|tuple| require_key(&mut normalize, tuple))
        .collect::<Vec<_>>();
    actual.sort();
    let expected = fixtures
        .lexicographic_rows
        .iter()
        .map(|tuple| require_key(&mut normalize, tuple))
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "composite-key contract: tuple ordering is lexicographic by component bytes"
    );

    assert_ne!(
        require_key(&mut normalize, &tuple(&[b"left", b"right"])),
        require_key(&mut normalize, &tuple(&[b"right", b"left"])),
        "composite-key contract: component order is semantic"
    );
}

/// Assert JSON exposes an authoritative encoded component array plus a display
/// label derived from that array. JSON Pointer paths identify the two fields.
pub fn assert_authoritative_key_output(
    output: &Value,
    components_pointer: &str,
    display_pointer: &str,
    expected_raw_components: &[Vec<u8>],
) {
    let expected = reference_key_tuple(expected_raw_components)
        .expect("expected output key fixture must be complete");
    let encoded = expected
        .iter()
        .map(|component| encode_key_component(component))
        .collect::<Vec<_>>();
    let expected_components = Value::Array(encoded.iter().cloned().map(Value::String).collect());
    let actual_components = output.pointer(components_pointer).unwrap_or_else(|| {
        panic!("missing authoritative key component array at JSON pointer {components_pointer}")
    });
    assert_eq!(
        actual_components, &expected_components,
        "machine key components at {components_pointer} must be authoritative and ordered"
    );

    let expected_display = encoded.join(" + ");
    let actual_display = output.pointer(display_pointer).unwrap_or_else(|| {
        panic!("missing display-only key label at JSON pointer {display_pointer}")
    });
    assert_eq!(
        actual_display.as_str(),
        Some(expected_display.as_str()),
        "display key label at {display_pointer} must be derived from the component array"
    );
}

/// Encode one component using the shared `u8:` / `hex:` machine convention.
pub fn encode_key_component(component: &[u8]) -> String {
    match std::str::from_utf8(component) {
        Ok(text) => format!("u8:{text}"),
        Err(_) => {
            let mut encoded = String::with_capacity(4 + component.len() * 2);
            encoded.push_str("hex:");
            for byte in component {
                use std::fmt::Write as _;
                write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
            }
            encoded
        }
    }
}

fn require_key<K, F>(normalize: &mut F, raw: &[Vec<u8>]) -> K
where
    F: FnMut(&[Vec<u8>]) -> Option<K>,
{
    normalize(raw).expect("composite-key contract: complete fixture must produce a key")
}

fn tuple(components: &[&[u8]]) -> RawKeyTuple {
    components
        .iter()
        .map(|component| component.to_vec())
        .collect()
}

fn ascii_trim(input: &[u8]) -> &[u8] {
    let start = input
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'))
        .unwrap_or(input.len());
    let end = input
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(start, |index| index + 1);
    &input[start..end]
}
