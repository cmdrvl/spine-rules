use serde_json::json;
use spine_rules::composite_key::{
    assert_authoritative_key_output, assert_ordered_tuple_semantics, composite_key_fixtures,
    reference_key_tuple,
};

#[test]
fn reference_key_passes_shared_composite_conformance() {
    assert_ordered_tuple_semantics(reference_key_tuple);
}

#[test]
#[should_panic(expected = "identity includes every component")]
fn shared_conformance_rejects_first_component_only_keys() {
    assert_ordered_tuple_semantics(|components| {
        reference_key_tuple(components).and_then(|tuple| tuple.into_iter().next())
    });
}

#[test]
#[should_panic(expected = "component boundaries must remain structural")]
fn shared_conformance_rejects_sentinel_flattened_keys() {
    assert_ordered_tuple_semantics(|components| {
        reference_key_tuple(components).map(|tuple| {
            let mut flattened = Vec::new();
            for (index, component) in tuple.iter().enumerate() {
                if index > 0 {
                    flattened.push(0xff);
                }
                flattened.extend_from_slice(component);
            }
            flattened
        })
    });
}

#[test]
fn fixture_corpus_covers_required_edge_cases() {
    let fixtures = composite_key_fixtures();
    assert_eq!(fixtures.repeated_first[0][0], fixtures.repeated_first[1][0]);
    assert_eq!(
        fixtures.duplicate_full_tuple[0],
        fixtures.duplicate_full_tuple[1]
    );
    assert!(reference_key_tuple(&fixtures.incomplete_secondary).is_none());
    assert!(fixtures.non_utf8.iter().flatten().any(|byte| *byte == 0xff));
    assert_ne!(fixtures.reordered_rows, fixtures.lexicographic_rows);
}

#[test]
fn json_assertion_requires_component_array_and_derived_display_label() {
    let output = json!({
        "row_key": ["u8:A", "hex:ff"],
        "row_id": "u8:A + hex:ff"
    });
    assert_authoritative_key_output(
        &output,
        "/row_key",
        "/row_id",
        &[b"A".to_vec(), b"\xff".to_vec()],
    );
}
