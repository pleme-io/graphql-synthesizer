# graphql-synthesizer

> **★★★ CSE / Knowable Construction.** This repo operates under
> **Constructive Substrate Engineering** — canonical specification at
> [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md).
> The Compounding Directive is in the org-level `pleme-io/CLAUDE.md` ★★★
> section. Read both before non-trivial changes.

Typed AST for structurally correct GraphQL **executable-document**
generation. The 19th member of the pleme-io synthesizer family, over the
shared `synthesizer-core` contract.

**Doctrine:** [`theory/GRAPHQL-AST.md`](https://github.com/pleme-io/theory/blob/main/GRAPHQL-AST.md) ·
**parent rule:** [`theory/TYPED-EMISSION.md`](https://github.com/pleme-io/theory/blob/main/TYPED-EMISSION.md).

## Why it exists — the receipt, not a principle

`tend/src/operator/discovery.rs` built GitHub's batch query with
`format!()`. It fabricated `refs/heads/HEAD` — a ref that does not exist —
GitHub answered `null` rather than erroring, and the **default** code path
silently returned no head for most of the fleet while the REST fallback it
was built to replace kept working. The fix comment left in that file is the
argument for this crate:

> *"`defaultBranchRef` is the query form that actually has a default
> branch; it is a **different selection, not a different string**, which is
> why **a string-building fix could not work**."*

A wrong *selection* is invisible to a string builder, because to a string
builder a selection is not a thing; it is characters.

## The one invariant everything rests on

**`Value` has no variant that holds source text.** There is no
`Value::Raw(String)`. A caller with a string reaches `Value::String`, which
is escaped by the printer, or `Value::Enum`/`Name`, which are validated as
names. None of them can produce syntax. That is not a check that runs — it
is a constructor that does not exist.

```rust
format!("repository(owner: \"{owner}\")")   // owner is TEXT inside syntax
Field::leaf(n("repository"))                 // owner is a VALUE beside syntax
    .with_arg(n("owner"), Value::str(owner))
```

`Name` is `/[_A-Za-z][_0-9A-Za-z]*/`, validated in `Name::new`, private
inner field. Escaping happens exactly once, in `write_graphql_string`, on
the way out — which is why that function is the one worth testing
exhaustively.

## Scope, stated honestly

- **Executable documents only.** SDL (the type-system half) is a separate
  axis, added when something needs it — not speculatively.
- **Parsing and schema validation are `apollo-compiler`'s job.** A parser we
  do not own is fine; an emitter we do not own is what this doctrine exists
  to prevent.
- **Statically-known client queries stay `cynic`'s.** This covers the case
  cynic structurally cannot: a document composed at runtime. A second client
  stack would be the duplication the fleet forbids.
- **Not yet built:** structural diff and patch emission (`GRAPHQL-AST.md`
  §V, M3). The crate today is the AST + canonical printer.

## Layout

| file | contents |
|---|---|
| `src/node.rs` | the AST + the canonical printer (the one place a tree becomes text) |
| `src/lib.rs` | re-exports + `aliased_batch`, the N-aliased-fields shape that has no cynic answer |
| `src/synthesizer_core_impl.rs` | `SynthesizerNode` + `NoRawAttestation` conformance |
| `tests/document.rs` | asserts on emitted TEXT, including the receipt and the injection case |
| `tests/synthesizer_core_conformance.rs` | calls `synthesizer_core::node::laws::*` to inherit family proof surface |

## Conventions

- Edition 2024, Rust 1.89.0+, MIT.
- `cargo clippy --all-targets` must be warning-free.
- **Tests assert on emitted bytes, not on the tree.** A test that asks the
  builder what it thinks will agree with a printer that is wrong.
- `format!()` inside the printer is correct and expected — the canonical
  serializer is precisely where syntax becomes text. The ban is on
  *constructing* syntax by interpolation upstream of the AST.
