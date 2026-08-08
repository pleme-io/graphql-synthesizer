//! Conformance to [`synthesizer_core`] traits.
//!
//! GraphQL rendering IS nested — unlike SQL DDL, a selection set sits
//! inside its parent — so the trait's `indent` argument is honoured rather
//! than ignored, and `emit` delegates straight to the inherent emitter
//! which already takes one.

use crate::node::{GraphQlNode, INDENT_UNIT};
use synthesizer_core::{NoRawAttestation, SynthesizerNode};

impl SynthesizerNode for GraphQlNode {
    fn emit(&self, indent: usize) -> String {
        // Inherent `GraphQlNode::emit` already takes an indent level;
        // inherent methods win UFCS lookup, so name it explicitly.
        GraphQlNode::emit(self, indent)
    }

    fn indent_unit() -> &'static str {
        INDENT_UNIT
    }

    fn variant_id(&self) -> u8 {
        match self {
            Self::Operation(_) => 0,
            Self::Fragment(_) => 1,
            Self::Document(_) => 2,
        }
    }
}

impl NoRawAttestation for GraphQlNode {
    fn attestation() -> &'static str {
        // What this crate actually guarantees, stated so it cannot quietly
        // grow: `Value` has no variant holding source text, so an operand
        // cannot become syntax. Escaping happens once, in the printer.
        "GraphQlNode carries no raw GraphQL source: `Value` has no text-bearing \
         variant and `Name` is validated at construction, so every operand is \
         emitted as a value and escaped by the canonical printer."
    }
}
