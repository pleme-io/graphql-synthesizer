//! What this crate must get right, asserted on emitted TEXT.
//!
//! The tests assert on the bytes the printer produces, not on the tree the
//! builder returns — a test that asks the model what it thinks agrees with
//! a printer that is wrong.

use graphql_synthesizer::*;

fn n(s: &str) -> Name {
    Name::new(s).expect("valid name")
}

// ══════════════════════════════════════════════════════════════
// The receipt — the defect this crate exists to make impossible
// ══════════════════════════════════════════════════════════════

/// tend's batch, built as a tree.
///
/// The `format!()` version of this is quoted in `theory/GRAPHQL-AST.md`.
#[test]
fn the_batch_shape_that_needed_format_is_a_tree() {
    // Built bottom-up with names, because hand-balancing five levels of
    // nesting inline is how the first draft of this test failed to compile.
    let commit_fields = || Selection::InlineFragment {
        type_condition: Some(n("Commit")),
        directives: vec![],
        selection_set: vec![
            Selection::Field(Field::leaf(n("oid"))),
            Selection::Field(Field::leaf(n("committedDate"))),
        ],
    };
    let target = || Field::leaf(n("target")).with_selections(vec![commit_fields()]);
    let default_branch_ref =
        || Field::leaf(n("defaultBranchRef")).with_selections(vec![Selection::Field(target())]);

    let targets = [("pleme-io", "escriba"), ("pleme-io", "tend")];
    let fields: Vec<Field> = targets
        .iter()
        .map(|(owner, repo)| {
            Field::leaf(n("repository"))
                .with_arg(n("owner"), Value::str(*owner))
                .with_arg(n("name"), Value::str(*repo))
                .with_selections(vec![Selection::Field(default_branch_ref())])
        })
        .collect();

    let doc = GraphQlNode::Operation(Operation::query().with_selections(aliased_batch(fields)));
    let text = doc.emit(0);

    assert!(
        text.contains(r#"r0: repository(owner: "pleme-io", name: "escriba")"#),
        "{text}"
    );
    assert!(
        text.contains(r#"r1: repository(owner: "pleme-io", name: "tend")"#),
        "{text}"
    );
    assert!(text.contains("... on Commit"), "{text}");
    assert!(text.starts_with("query {"), "{text}");
}

/// THE regression, stated as what it actually was.
///
/// The bug was not a bad string — it was picking `ref(qualifiedName:)` when
/// the target had no explicit ref, which fabricated `refs/heads/HEAD`. As a
/// tree, the two cases are visibly *different selections*, which is the
/// whole point: a reviewer sees a branch, not a format string.
#[test]
fn the_two_ref_cases_are_different_selections_not_different_strings() {
    let commit = || {
        vec![Selection::Field(Field::leaf(n("target")).with_selections(
            vec![Selection::InlineFragment {
                type_condition: Some(n("Commit")),
                directives: vec![],
                selection_set: vec![Selection::Field(Field::leaf(n("oid")))],
            }],
        ))]
    };

    // No explicit ref → defaultBranchRef, which takes NO argument.
    let default_ref = Field::leaf(n("defaultBranchRef")).with_selections(commit());
    // An explicit ref → ref(qualifiedName: "…").
    let explicit = Field::leaf(n("ref"))
        .with_arg(n("qualifiedName"), Value::str("refs/heads/main"))
        .with_selections(commit());

    let a = GraphQlNode::Operation(
        Operation::query().with_selections(vec![Selection::Field(default_ref)]),
    );
    let b = GraphQlNode::Operation(
        Operation::query().with_selections(vec![Selection::Field(explicit)]),
    );

    let (ta, tb) = (a.emit(0), b.emit(0));
    assert!(ta.contains("defaultBranchRef"), "{ta}");
    assert!(
        !ta.contains("qualifiedName"),
        "the default case takes no ref argument at all: {ta}"
    );
    assert!(
        tb.contains(r#"ref(qualifiedName: "refs/heads/main")"#),
        "{tb}"
    );
    assert_ne!(
        a, b,
        "different selections are different TREES, not strings"
    );
}

// ══════════════════════════════════════════════════════════════
// The invariant: an operand cannot become syntax
// ══════════════════════════════════════════════════════════════

/// The injection case, which `format!("…\"{owner}\"…")` cannot survive.
///
/// A value that closes the string literal and continues as query source is
/// exactly what interpolation permits. Here the quote is escaped and the
/// payload stays inside the literal.
#[test]
fn a_value_that_tries_to_close_the_literal_is_escaped() {
    let hostile = r#"a") { viewer { email } } x(y: ""#;
    let f = Field::leaf(n("repository")).with_arg(n("owner"), Value::str(hostile));
    let text =
        GraphQlNode::Operation(Operation::query().with_selections(vec![Selection::Field(f)]))
            .emit(0);

    // Golden, because the interesting property is structural and a
    // `contains` check cannot express it. The payload's own braces and
    // parens appear in the output — as DATA inside the literal — so
    // asserting they are absent would be asserting the wrong thing (the
    // first draft of this test did exactly that and failed correctly).
    //
    // What must hold is that the document's SHAPE is untouched: one
    // operation, one field, one argument, and every quote from the payload
    // escaped so it cannot end the literal.
    assert_eq!(
        text,
        concat!(
            "query {\n",
            "  repository(owner: \"a\\\") { viewer { email } } x(y: \\\"\")\n",
            "}\n",
        ),
        "actual:\n{text}"
    );

    // And the same tree with a benign operand differs ONLY inside the
    // quotes — the skeleton is byte-identical once the literal is removed.
    let benign = Field::leaf(n("repository")).with_arg(n("owner"), Value::str("benign"));
    let benign_text =
        GraphQlNode::Operation(Operation::query().with_selections(vec![Selection::Field(benign)]))
            .emit(0);
    let strip_literal = |t: &str| {
        let a = t.find('"').expect("open quote");
        let b = t.rfind('"').expect("close quote");
        format!("{}{}", &t[..a], &t[b + 1..])
    };
    assert_eq!(
        strip_literal(&text),
        strip_literal(&benign_text),
        "a hostile operand changed the document's structure"
    );
}

#[test]
fn control_characters_and_backslashes_are_escaped() {
    let v = Value::str("line\nnext\ttab\\slash\"quote\u{1}ctrl");
    let e = v.emit();
    assert!(e.starts_with('"') && e.ends_with('"'), "{e}");
    assert!(e.contains("\\n"), "{e}");
    assert!(e.contains("\\t"), "{e}");
    assert!(e.contains("\\\\"), "{e}");
    assert!(e.contains("\\\""), "{e}");
    assert!(e.contains("\\u0001"), "C0 control gets \\uXXXX: {e}");
    assert!(
        !e[1..e.len() - 1].contains('\n'),
        "no raw newline survives inside the literal: {e:?}"
    );
}

#[test]
fn non_ascii_passes_through_unescaped() {
    // GraphQL documents are UTF-8; escaping these would be wrong, not safe.
    assert_eq!(Value::str("café 日本 🦀").emit(), "\"café 日本 🦀\"");
}

/// A name is a lexical class, checked once, at construction.
#[test]
fn a_name_cannot_carry_syntax() {
    assert!(Name::new("validName_1").is_ok());
    assert!(Name::new("_leading").is_ok());
    for bad in [
        "",
        "1leading",
        "has space",
        "has-dash",
        "close) { evil",
        "quote\"",
    ] {
        assert!(Name::new(bad).is_err(), "{bad:?} must be rejected");
    }
}

// ══════════════════════════════════════════════════════════════
// Printer canonicality
// ══════════════════════════════════════════════════════════════

#[test]
fn one_tree_has_exactly_one_spelling() {
    let build = || {
        GraphQlNode::Operation(Operation::query().with_selections(vec![Selection::Field(
            Field::leaf(n("a")).with_selections(vec![Selection::Field(Field::leaf(n("b")))]),
        )]))
    };
    assert_eq!(build().emit(0), build().emit(0));
}

#[test]
fn nesting_indents_by_the_unit() {
    let doc = GraphQlNode::Operation(Operation::query().with_selections(vec![Selection::Field(
        Field::leaf(n("outer")).with_selections(vec![Selection::Field(Field::leaf(n("inner")))]),
    )]));
    let text = doc.emit(0);
    assert!(text.contains("\n  outer"), "{text}");
    assert!(text.contains("\n    inner"), "{text}");
}

#[test]
fn a_leaf_field_emits_no_empty_braces() {
    let doc = GraphQlNode::Operation(
        Operation::query().with_selections(vec![Selection::Field(Field::leaf(n("id")))]),
    );
    let text = doc.emit(0);
    assert!(
        !text.contains("{}"),
        "an empty selection set is omitted: {text}"
    );
}

#[test]
fn variables_and_types_emit() {
    let op = Operation {
        operation_type: OperationType::Mutation,
        name: Some(n("SetThing")),
        variable_definitions: vec![VariableDefinition {
            name: n("ids"),
            ty: TypeRef::NonNull(Box::new(TypeRef::List(Box::new(TypeRef::NonNull(
                Box::new(TypeRef::Named(n("ID"))),
            ))))),
            default_value: None,
        }],
        directives: vec![],
        selection_set: vec![Selection::Field(
            Field::leaf(n("setThing")).with_arg(n("ids"), Value::Variable(n("ids"))),
        )],
    };
    let text = GraphQlNode::Operation(op).emit(0);
    assert!(
        text.starts_with("mutation SetThing($ids: [ID!]!)"),
        "{text}"
    );
    assert!(text.contains("setThing(ids: $ids)"), "{text}");
}

#[test]
fn a_document_separates_definitions() {
    let frag = GraphQlNode::Fragment(FragmentDefinition {
        name: n("F"),
        type_condition: n("T"),
        directives: vec![],
        selection_set: vec![Selection::Field(Field::leaf(n("x")))],
    });
    let op = GraphQlNode::Operation(Operation::query().with_selections(vec![
        Selection::FragmentSpread {
            name: n("F"),
            directives: vec![],
        },
    ]));
    let text = GraphQlNode::Document(vec![op, frag]).emit(0);
    assert!(text.contains("...F"), "{text}");
    assert!(text.contains("fragment F on T"), "{text}");
}

/// An already-aliased field keeps its alias, so a caller can mix.
#[test]
fn aliased_batch_respects_an_existing_alias() {
    let fields = vec![
        Field::leaf(n("a")).with_alias(n("mine")),
        Field::leaf(n("b")),
    ];
    let sels = aliased_batch(fields);
    let text = GraphQlNode::Operation(Operation::query().with_selections(sels)).emit(0);
    assert!(text.contains("mine: a"), "{text}");
    assert!(text.contains("r1: b"), "{text}");
}
