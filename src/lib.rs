//! graphql-synthesizer — typed AST for structurally correct GraphQL
//! executable documents.
//!
//! The 19th member of the pleme-io synthesizer family (`nix-` `go-` `hcl-`
//! `helm-` `yaml-` `sql-` `rust-` `ruby-` `python-` `shell-` `packer-`
//! `kustomize-` `dockerfile-` `github-actions-` `go-tool-` `terraform-`
//! `arch-` `abstract-`), over the shared `synthesizer-core` contract.
//!
//! GraphQL's executable-document basis: 2 definitions (operation,
//! fragment), 3 selections (field, spread, inline fragment), 9 values and
//! 3 type refs. Any valid executable document composes from these.
//!
//! ## Why this exists
//!
//! [`theory/GRAPHQL-AST.md`](https://github.com/pleme-io/theory/blob/main/GRAPHQL-AST.md)
//! carries the doctrine and the receipt. The short version: `tend` built
//! GitHub's batch query with `format!()`, fabricated a ref that does not
//! exist (`refs/heads/HEAD`), GitHub answered `null` instead of erroring,
//! and the *default* code path silently returned nothing for most of the
//! fleet. The fix comment left in that file makes the argument:
//!
//! > *"it is a different **selection**, not a different string, which is
//! > why a string-building fix could not work."*
//!
//! A wrong selection is invisible to a string builder, because to a string
//! builder a selection is not a thing.
//!
//! ## Scope
//!
//! **Executable documents only.** The type-system (SDL) half is a separate
//! axis, added when something needs it rather than speculatively. For
//! *parsing* and schema *validation*, use `apollo-compiler` — a parser we
//! do not own is fine; an emitter we do not own is what this doctrine
//! exists to prevent.
//!
//! Statically-known client queries are `cynic`'s job and remain so. This
//! covers the case cynic structurally cannot: a document composed at
//! runtime.
//!
//! ## Example — the shape `tend` needed
//!
//! ```
//! use graphql_synthesizer::*;
//!
//! let n = |s: &str| Name::new(s).unwrap();
//!
//! // One repository(owner:, name:) selection per target, aliased r0..rN.
//! let targets = [("pleme-io", "escriba"), ("pleme-io", "tend")];
//! let fields: Vec<Field> = targets
//!     .iter()
//!     .map(|(owner, repo)| {
//!         Field::leaf(n("repository"))
//!             .with_arg(n("owner"), Value::str(*owner))
//!             .with_arg(n("name"), Value::str(*repo))
//!             .with_selections(vec![Selection::Field(Field::leaf(n("id")))])
//!     })
//!     .collect();
//!
//! let doc = GraphQlNode::Operation(Operation::query().with_selections(aliased_batch(fields)));
//! let text = doc.emit(0);
//! assert!(text.contains("r0: repository(owner: \"pleme-io\", name: \"escriba\")"));
//! assert!(text.contains("r1: repository(owner: \"pleme-io\", name: \"tend\")"));
//! ```

mod node;
mod synthesizer_core_impl;

pub use node::*;

/// Batch field selections under generated aliases `r0`, `r1`, … `rN`.
///
/// This is the shape that has no `cynic` answer — N instances of the same
/// field, count known only at runtime — and therefore the shape every
/// hand-roller reaches for `format!()` to express.
///
/// A field that already carries an alias keeps it; only unaliased fields
/// are given one, so a caller can mix.
///
/// The generated aliases are `r{index}`, which is always a valid [`Name`]
/// by construction (`r` is an alphabetic first character and the rest are
/// digits) — so this cannot fail, and does not return a `Result` that a
/// caller would have to pretend to handle.
#[must_use]
pub fn aliased_batch(fields: impl IntoIterator<Item = Field>) -> Vec<Selection> {
    fields
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            let f = if f.alias.is_some() {
                f
            } else {
                let alias = Name::new(format!("r{i}"))
                    .expect("r<digits> is a valid GraphQL name by construction");
                f.with_alias(alias)
            };
            Selection::Field(f)
        })
        .collect()
}
