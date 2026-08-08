//! Integration tests proving `GraphQlNode` conforms to `synthesizer_core`.
//!
//! Every test calls one of `synthesizer_core::node::laws::*` on a real
//! `GraphQlNode`, compounding proof surface: the same laws prove properties
//! of every synthesizer in the family, so a new member inherits them rather
//! than restating them.
//!
//! Unlike SQL DDL — which is flat at the statement level and whose trait
//! impl therefore ignores `indent` — **GraphQL is genuinely nested**, so
//! this member satisfies `honors_indent_unit` and `indent_monotone_len` by
//! the non-trivial disjunct: output really does grow with indent.

use graphql_synthesizer::*;
use synthesizer_core::node::laws;
use synthesizer_core::{NoRawAttestation, SynthesizerNode};

fn n(s: &str) -> Name {
    Name::new(s).expect("valid name")
}

/// One of each variant, for the laws that quantify over variants.
fn samples() -> Vec<GraphQlNode> {
    let op = Operation::query().with_selections(vec![Selection::Field(
        Field::leaf(n("outer")).with_selections(vec![Selection::Field(Field::leaf(n("inner")))]),
    )]);
    let frag = FragmentDefinition {
        name: n("F"),
        type_condition: n("T"),
        directives: vec![],
        selection_set: vec![Selection::Field(Field::leaf(n("x")))],
    };
    vec![
        GraphQlNode::Operation(op.clone()),
        GraphQlNode::Fragment(frag.clone()),
        GraphQlNode::Document(vec![
            GraphQlNode::Operation(op),
            GraphQlNode::Fragment(frag),
        ]),
    ]
}

// ─── Trait shape ────────────────────────────────────────────────────

#[test]
fn indent_unit_is_two_spaces() {
    assert_eq!(<GraphQlNode as SynthesizerNode>::indent_unit(), "  ");
}

#[test]
fn variant_ids_distinct_across_all_variants() {
    let ids: Vec<u8> = samples().iter().map(SynthesizerNode::variant_id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "variant ids collide: {ids:?}");
}

#[test]
fn attestation_states_what_is_actually_guaranteed() {
    let a = <GraphQlNode as NoRawAttestation>::attestation();
    assert!(!a.is_empty());
    // The claim must name the mechanism, not just assert safety — an
    // attestation that says "is safe" is a slogan, not a receipt.
    assert!(a.contains("Value"), "{a}");
    assert!(a.contains("Name"), "{a}");
}

// ─── The family's laws ──────────────────────────────────────────────

#[test]
fn law_determinism_holds_on_every_variant() {
    for node in samples() {
        for indent in 0..3 {
            assert!(
                laws::is_deterministic(&node, indent),
                "emit is not pure for {node:?} at {indent}"
            );
        }
    }
}

#[test]
fn law_honors_indent_unit_on_every_variant() {
    for node in samples() {
        for base in 0..3 {
            assert!(laws::honors_indent_unit(&node, base), "{node:?} at {base}");
        }
    }
}

#[test]
fn law_indent_monotone_on_every_variant() {
    for node in samples() {
        for base in 0..3 {
            assert!(laws::indent_monotone_len(&node, base), "{node:?} at {base}");
        }
    }
}

#[test]
fn law_variant_id_in_range_on_every_variant() {
    for node in samples() {
        assert!(laws::variant_id_is_valid(&node), "{node:?}");
    }
}

/// GraphQL nests, so the monotonicity law holds *strictly* here rather
/// than by the equal-length escape hatch SQL uses. Worth asserting
/// separately: a member that accidentally ignored `indent` would still
/// pass the law above.
#[test]
fn indenting_actually_indents() {
    let node = &samples()[0];
    let a = SynthesizerNode::emit(node, 0);
    let b = SynthesizerNode::emit(node, 1);
    assert!(
        b.len() > a.len(),
        "GraphQL is nested — deeper indent must produce more bytes\n{a}\n---\n{b}"
    );
    assert!(b.starts_with("  query"), "{b}");
}
