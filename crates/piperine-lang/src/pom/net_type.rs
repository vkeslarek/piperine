//! POM type references — [`NetRef`], [`NetType`], [`ValueType`], [`TypeRef`].
//!
//! The value/net split (`docs/piperine-hdl-spec.md` §2): a [`NetType`] types a
//! port or wire (a discipline or net-capable bundle); a [`ValueType`] types a
//! param, var, or function result.

// ─────────────────────────────── Net reference ───────────────────────────────

/// Names that refer to the ground (reference) node.
pub const GROUND_NAMES: &[&str] = &["gnd", "GND", "vss", "VSS", "0"];

/// Whether a name refers to the ground (reference) node.
pub fn is_ground(name: &str) -> bool {
    GROUND_NAMES.contains(&name)
}

/// A reference to a net, optionally indexed into a bus — `node[i]`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

    /// The sentinel prefix every [`Self::unconnected`] net name starts
    /// with. A leading digit makes it unreachable by any real
    /// Verilog-AMS identifier (identifiers never start with a digit),
    /// so it can never collide with an authored net.
    const UNCONNECTED_PREFIX: &'static str = "0$unconnected";

    /// The sentinel connection for a port left unconnected at
    /// instantiation (`.p()` or a blank positional slot, §9.19,
    /// LRMC-C7-09, T50) — `$port_connected()` queries this.
    ///
    /// `disambiguator` (typically `"instance_label.port_name"`) makes
    /// each unconnected port its own distinct net: two *different*
    /// unconnected ports must never resolve to the identical net name,
    /// or the circuit builder would electrically short them together —
    /// a real bug this crate's own tests caught before this doc comment
    /// was written.
    pub fn unconnected(disambiguator: &str) -> Self {
        Self::simple(format!("{}.{disambiguator}", Self::UNCONNECTED_PREFIX))
    }

    /// True if this is one of [`Self::unconnected`]'s sentinels.
    pub fn is_unconnected(&self) -> bool {
        self.net.starts_with(Self::UNCONNECTED_PREFIX)
    }
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

/// One of the four IEEE 1364-2005 net-type keywords VAMS carries forward
/// verbatim (§3.7) — distinct from a *discipline* name even though both
/// are spelled as a bare leading identifier in a `net_declaration`
/// (`grammar/item.rs::parse_net_decl` captures either shape the same way;
/// the elaborator is what tells them apart, `elab/lower/vams.rs::
/// declarator_net_type`). `Wire`/`Tri`/`Reg` are ordinary 4-state digital
/// nets (Table 7-1's "bit" row); `Wreal` is real-valued (§3.7: "shall not
/// store its value... initial value of zero", never 4-state) — the two
/// kinds get different [`crate::pom::ValueType`]-adjacent treatment where
/// a continuous context reads one (T52, LRMW2-45).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DigitalNetKind {
    Wire,
    Tri,
    Reg,
    Wreal,
}

impl DigitalNetKind {
    /// Recognizes one of the four keywords by exact spelling — never a
    /// prefix/case-insensitive match, so a user's own identically-spelled
    /// discipline name (unusual but legal) is never silently reinterpreted
    /// as one of these built-ins by accident... except these four spellings
    /// *are* reserved (Annex B): a `discipline wire { ... }` declaration is
    /// itself illegal, so this exact-match is unambiguous in practice.
    pub fn from_keyword(s: &str) -> Option<Self> {
        match s {
            "wire" => Some(Self::Wire),
            "tri" => Some(Self::Tri),
            "reg" => Some(Self::Reg),
            "wreal" => Some(Self::Wreal),
            _ => None,
        }
    }
}

/// A port/wire type — a discipline, one of the four §3.7 1364 net-type
/// keywords, or a fixed-size array of either.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetType {
    Discipline(String),
    /// §3.7 — see [`DigitalNetKind`]'s own doc comment.
    Digital1364(DigitalNetKind),
    Array(Box<NetType>, u64),
}

impl NetType {
    /// Returns the innermost discipline name, unwinding any arrays. A
    /// [`Self::Digital1364`] net has no discipline at all (§3.7's own
    /// point — these are not disciplines) — returns its own keyword
    /// spelling instead, so a caller that only wants *some* diagnostic
    /// label still gets a meaningful one rather than a panic.
    pub fn discipline_name(&self) -> &str {
        match self {
            Self::Discipline(s) => s,
            Self::Digital1364(DigitalNetKind::Wire) => "wire",
            Self::Digital1364(DigitalNetKind::Tri) => "tri",
            Self::Digital1364(DigitalNetKind::Reg) => "reg",
            Self::Digital1364(DigitalNetKind::Wreal) => "wreal",
            Self::Array(inner, _) => inner.discipline_name(),
        }
    }
    /// Total flattened width (product of all array dimensions, 1 for a scalar).
    pub fn width(&self) -> u64 {
        match self {
            Self::Discipline(_) | Self::Digital1364(_) => 1,
            Self::Array(inner, n) => inner.width() * n,
        }
    }
}

// ─────────────────────────────── Value types ─────────────────────────────────

/// A param/var/function type — a primitive, enum, array, or function pointer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ValueType {
    Real, Natural, Integer, Complex, Boolean, Quad, Str,
    /// A four-state (`0`/`1`/`x`/`z`) bit vector of the given width —
    /// LRMC-C1-02. Backed by `crate::value::LogicVec`.
    Logic(u32),
    Enum(String),
    /// A value bundle (`param model : DiodeModel`, `fn f(m : DiodeModel)`).
    /// Consumers flatten it per-field; a context that cannot fails loud.
    Bundle(String),
    /// A tuple type `(T, U, ...)`.
    Tuple(Vec<ValueType>),
    Array(Box<ValueType>, u64),
    FnPtr(Vec<TypeRef>, Box<TypeRef>),
}

/// Either half of the value/net split — whatever a `Param`, function
/// argument, or return type resolves to.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
