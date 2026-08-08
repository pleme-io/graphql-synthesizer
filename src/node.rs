//! GraphQL executable-document AST — the irreducible primitives.
//!
//! The algebraic basis of an executable GraphQL document: 2 definitions
//! (operation, fragment) + 3 selections (field, spread, inline fragment) +
//! 9 values + 3 type refs. ANY valid executable document is a composition
//! of these.
//!
//! Scope is deliberately **executable documents**, not the type-system
//! (SDL) half. The receipt this crate exists for is a client composing a
//! query at runtime; SDL is a separate axis and is added when something
//! needs it, not speculatively.
//!
//! ## The one invariant everything else rests on
//!
//! [`Value`] has **no variant that holds source text.** An argument cannot
//! carry syntax, so a value cannot end a string literal and continue as
//! query source. That is not a check that runs — it is a constructor that
//! does not exist. Escaping happens once, in [`Value::emit`], on the way
//! out.
//!
//! This is the whole difference from the `format!()` it replaces:
//!
//! ```text
//! format!("repository(owner: \"{owner}\")")   owner is TEXT inside syntax
//! Argument::new("owner", Value::str(owner))   owner is a VALUE beside syntax
//! ```

use std::fmt::Write as _;

// ══════════════════════════════════════════════════════════════
// Values — the type that makes injection unrepresentable
// ══════════════════════════════════════════════════════════════

/// A GraphQL input value.
///
/// There is deliberately no `Raw(String)` variant. A caller with a string
/// in hand can only reach [`Value::String`], which is escaped on emit, or
/// [`Value::Enum`], which is validated as a name. Neither can produce
/// syntax.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `$name` — the preferred way to pass an operand at all.
    Variable(Name),
    Int(i64),
    Float(f64),
    /// Emitted with GraphQL string escaping. Arbitrary bytes are safe here.
    String(String),
    Boolean(bool),
    Null,
    /// An enum member — a bare name in the document, so it is a [`Name`].
    Enum(Name),
    List(Vec<Value>),
    Object(Vec<(Name, Value)>),
}

impl Value {
    /// A string value. The escaping is applied at emit time, so any bytes
    /// are accepted here.
    pub fn str(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    /// Emit canonical GraphQL syntax for this value.
    pub fn emit(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Self::Variable(n) => {
                out.push('$');
                out.push_str(n.as_str());
            }
            Self::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Self::Float(f) => {
                let _ = write!(out, "{f}");
            }
            Self::String(s) => write_graphql_string(s, out),
            Self::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Null => out.push_str("null"),
            Self::Enum(n) => out.push_str(n.as_str()),
            Self::List(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Self::Object(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(k.as_str());
                    out.push_str(": ");
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Write `s` as a GraphQL string literal, escaped per the spec's
/// `StringCharacter` production.
///
/// This is the ONE place a caller-supplied string becomes syntax, which is
/// why it is the one place escaping can be got wrong — and therefore the
/// one place worth testing exhaustively.
fn write_graphql_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            // Remaining C0 controls have no short escape; GraphQL requires
            // \uXXXX. Printable characters (including non-ASCII) pass
            // through — GraphQL documents are UTF-8.
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ══════════════════════════════════════════════════════════════
// Names — a lexical class, checked once at construction
// ══════════════════════════════════════════════════════════════

/// A GraphQL `Name`: `/[_A-Za-z][_0-9A-Za-z]*/`.
///
/// Wrapping rather than using bare `String` is what stops a field name,
/// alias or directive from carrying syntax. The inner field is private, so
/// the only way in is [`Name::new`], which validates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(String);

/// Why a string is not a valid GraphQL name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    Empty,
    /// Byte offset and the offending character.
    BadChar(usize, char),
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("a GraphQL name cannot be empty"),
            Self::BadChar(i, c) => {
                write!(f, "invalid character {c:?} at {i} in a GraphQL name")
            }
        }
    }
}

impl std::error::Error for NameError {}

impl Name {
    /// Validate and wrap. Returns `Err` rather than panicking, so a caller
    /// building a document from external input has somewhere to put the
    /// failure other than the document.
    pub fn new(s: impl Into<String>) -> Result<Self, NameError> {
        let s = s.into();
        let mut chars = s.char_indices();
        let Some((_, first)) = chars.next() else {
            return Err(NameError::Empty);
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(NameError::BadChar(0, first));
        }
        for (i, c) in chars {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                return Err(NameError::BadChar(i, c));
            }
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ══════════════════════════════════════════════════════════════
// Type references — for variable definitions
// ══════════════════════════════════════════════════════════════

/// A type reference: `Name`, `[T]`, `T!`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Named(Name),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

impl TypeRef {
    pub fn emit(&self) -> String {
        match self {
            Self::Named(n) => n.as_str().to_string(),
            Self::List(inner) => {
                let mut s = String::from("[");
                s.push_str(&inner.emit());
                s.push(']');
                s
            }
            Self::NonNull(inner) => {
                let mut s = inner.emit();
                s.push('!');
                s
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════
// Arguments + directives
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub name: Name,
    pub value: Value,
}

impl Argument {
    pub fn new(name: Name, value: Value) -> Self {
        Self { name, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Directive {
    pub name: Name,
    pub arguments: Vec<Argument>,
}

// ══════════════════════════════════════════════════════════════
// Selections
// ══════════════════════════════════════════════════════════════

/// One selection inside a selection set.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Field(Field),
    /// `...FragmentName`
    FragmentSpread {
        name: Name,
        directives: Vec<Directive>,
    },
    /// `... on Type { … }`
    InlineFragment {
        type_condition: Option<Name>,
        directives: Vec<Directive>,
        selection_set: Vec<Selection>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// `alias: name` when present.
    pub alias: Option<Name>,
    pub name: Name,
    pub arguments: Vec<Argument>,
    pub directives: Vec<Directive>,
    pub selection_set: Vec<Selection>,
}

impl Field {
    /// A leaf field with no arguments and no sub-selection.
    pub fn leaf(name: Name) -> Self {
        Self {
            alias: None,
            name,
            arguments: Vec::new(),
            directives: Vec::new(),
            selection_set: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_alias(mut self, alias: Name) -> Self {
        self.alias = Some(alias);
        self
    }

    #[must_use]
    pub fn with_arg(mut self, name: Name, value: Value) -> Self {
        self.arguments.push(Argument::new(name, value));
        self
    }

    #[must_use]
    pub fn with_selections(mut self, selections: Vec<Selection>) -> Self {
        self.selection_set = selections;
        self
    }

    /// The key this field occupies in its parent's response object — the
    /// alias if there is one, else the name. Two fields sharing a response
    /// key must be mergeable; this is what a dedup or a merge keys on.
    #[must_use]
    pub fn response_key(&self) -> &str {
        self.alias.as_ref().unwrap_or(&self.name).as_str()
    }
}

// ══════════════════════════════════════════════════════════════
// Definitions + document
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}

impl OperationType {
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Mutation => "mutation",
            Self::Subscription => "subscription",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableDefinition {
    pub name: Name,
    pub ty: TypeRef,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub operation_type: OperationType,
    pub name: Option<Name>,
    pub variable_definitions: Vec<VariableDefinition>,
    pub directives: Vec<Directive>,
    pub selection_set: Vec<Selection>,
}

impl Operation {
    pub fn query() -> Self {
        Self {
            operation_type: OperationType::Query,
            name: None,
            variable_definitions: Vec::new(),
            directives: Vec::new(),
            selection_set: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_selections(mut self, selections: Vec<Selection>) -> Self {
        self.selection_set = selections;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FragmentDefinition {
    pub name: Name,
    pub type_condition: Name,
    pub directives: Vec<Directive>,
    pub selection_set: Vec<Selection>,
}

/// The node this crate's `SynthesizerNode` impl is written for.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphQlNode {
    Operation(Operation),
    Fragment(FragmentDefinition),
    /// A whole document: definitions in order, separated by a blank line.
    Document(Vec<GraphQlNode>),
}

// ══════════════════════════════════════════════════════════════
// The canonical printer — the ONE place a tree becomes text
// ══════════════════════════════════════════════════════════════

/// Two spaces, the GraphQL community default.
pub const INDENT_UNIT: &str = "  ";

impl GraphQlNode {
    /// Emit canonical GraphQL source at `indent` levels of nesting.
    ///
    /// One tree has exactly one spelling, which is what makes byte
    /// comparison of two documents meaningful.
    pub fn emit(&self, indent: usize) -> String {
        let mut out = String::new();
        match self {
            Self::Operation(op) => write_operation(op, indent, &mut out),
            Self::Fragment(frag) => write_fragment(frag, indent, &mut out),
            Self::Document(defs) => {
                for (i, d) in defs.iter().enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    out.push_str(&d.emit(indent));
                }
            }
        }
        out
    }
}

fn pad(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str(INDENT_UNIT);
    }
}

fn write_arguments(args: &[Argument], out: &mut String) {
    if args.is_empty() {
        return;
    }
    out.push('(');
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(a.name.as_str());
        out.push_str(": ");
        out.push_str(&a.value.emit());
    }
    out.push(')');
}

fn write_directives(dirs: &[Directive], out: &mut String) {
    for d in dirs {
        out.push_str(" @");
        out.push_str(d.name.as_str());
        write_arguments(&d.arguments, out);
    }
}

fn write_selection_set(sels: &[Selection], indent: usize, out: &mut String) {
    if sels.is_empty() {
        return;
    }
    out.push_str(" {\n");
    for s in sels {
        write_selection(s, indent + 1, out);
    }
    pad(indent, out);
    out.push('}');
}

fn write_selection(sel: &Selection, indent: usize, out: &mut String) {
    pad(indent, out);
    match sel {
        Selection::Field(f) => {
            if let Some(alias) = &f.alias {
                out.push_str(alias.as_str());
                out.push_str(": ");
            }
            out.push_str(f.name.as_str());
            write_arguments(&f.arguments, out);
            write_directives(&f.directives, out);
            write_selection_set(&f.selection_set, indent, out);
        }
        Selection::FragmentSpread { name, directives } => {
            out.push_str("...");
            out.push_str(name.as_str());
            write_directives(directives, out);
        }
        Selection::InlineFragment {
            type_condition,
            directives,
            selection_set,
        } => {
            out.push_str("...");
            if let Some(t) = type_condition {
                out.push_str(" on ");
                out.push_str(t.as_str());
            }
            write_directives(directives, out);
            write_selection_set(selection_set, indent, out);
        }
    }
    out.push('\n');
}

fn write_operation(op: &Operation, indent: usize, out: &mut String) {
    pad(indent, out);
    out.push_str(op.operation_type.keyword());
    if let Some(n) = &op.name {
        out.push(' ');
        out.push_str(n.as_str());
    }
    if !op.variable_definitions.is_empty() {
        out.push('(');
        for (i, v) in op.variable_definitions.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push('$');
            out.push_str(v.name.as_str());
            out.push_str(": ");
            out.push_str(&v.ty.emit());
            if let Some(d) = &v.default_value {
                out.push_str(" = ");
                out.push_str(&d.emit());
            }
        }
        out.push(')');
    }
    write_directives(&op.directives, out);
    write_selection_set(&op.selection_set, indent, out);
    out.push('\n');
}

fn write_fragment(frag: &FragmentDefinition, indent: usize, out: &mut String) {
    pad(indent, out);
    out.push_str("fragment ");
    out.push_str(frag.name.as_str());
    out.push_str(" on ");
    out.push_str(frag.type_condition.as_str());
    write_directives(&frag.directives, out);
    write_selection_set(&frag.selection_set, indent, out);
    out.push('\n');
}
