//! POM type references — [`NetRef`], [`NetType`], [`ValueType`], [`TypeRef`].
//!
//! The value/net split (`docs/piperine-hdl-spec.md` §2): a [`NetType`] types a
//! port or wire (a discipline or net-capable bundle); a [`ValueType`] types a
//! param, var, or function result.

// ─────────────────────────────── Net reference ───────────────────────────────

/// A reference to a net, optionally indexed into a bus — `node[i]`.
#[derive(Debug, Clone, PartialEq)]
pub struct NetRef {
    pub net: String,
    pub index: Option<u64>,
}

impl NetRef {
    /// Create a simple (non-indexed) net reference.
    pub fn simple(net: impl Into<String>) -> Self {
        Self { net: net.into(), index: None }
    }
    /// Create an indexed net reference (e.g. `bus[3]`).
    pub fn indexed(net: impl Into<String>, index: u64) -> Self {
        Self { net: net.into(), index: Some(index) }
    }
    /// The net name.
    pub fn net(&self) -> &str { &self.net }
    /// The bus index, if this is an indexed reference.
    pub fn index(&self) -> Option<u64> { self.index }
}

impl std::fmt::Display for NetRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.index {
            None => write!(f, "{}", self.net),
            Some(i) => write!(f, "{}[{}]", self.net, i),
        }
    }
}

// ─────────────────────────────── Net types ───────────────────────────────────

/// A port/wire type — a discipline, or a fixed-size array of one.
#[derive(Debug, Clone, PartialEq)]
pub enum NetType {
    Discipline(String),
    Array(Box<NetType>, u64),
}

impl NetType {
    /// Returns the innermost discipline name, unwinding any arrays.
    pub fn discipline_name(&self) -> &str {
        match self { Self::Discipline(s) => s, Self::Array(inner, _) => inner.discipline_name() }
    }
    /// Total flattened width (product of all array dimensions, 1 for a scalar).
    pub fn width(&self) -> u64 {
        match self { Self::Discipline(_) => 1, Self::Array(inner, n) => inner.width() * n }
    }
}

// ─────────────────────────────── Value types ─────────────────────────────────

/// A param/var/function type — a primitive, enum, array, or function pointer.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Real, Natural, Integer, Complex, Boolean, Quad, Str,
    Enum(String),
    Array(Box<ValueType>, u64),
    FnPtr(Vec<TypeRef>, Box<TypeRef>),
}

/// Either half of the value/net split — whatever a `Param`, function
/// argument, or return type resolves to.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Net(NetType),
    Value(ValueType),
}

impl TypeRef {
    /// Extract the net type, if this is a `Net` variant.
    pub fn as_net(&self) -> Option<&NetType> {
        match self { TypeRef::Net(n) => Some(n), _ => None }
    }
    /// Extract the value type, if this is a `Value` variant.
    pub fn as_value(&self) -> Option<&ValueType> {
        match self { TypeRef::Value(v) => Some(v), _ => None }
    }
}
