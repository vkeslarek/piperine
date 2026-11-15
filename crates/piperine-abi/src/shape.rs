//! An owned shape (abi.md §3, design `shape`): Rust code builds a shape from
//! plain specs, and the `ShapeBuf` keeps every string and list the raw
//! `Shape` points to. Each string and nested list lives in its own heap box,
//! and each top-level list's pointer is refreshed after every add, so the raw
//! view is valid at every moment; once frozen into a `ShapeRef` nothing is
//! added and nothing moves until the last clone is dropped.

use std::sync::Arc;

use crate::raw::{
    Bytes, Cell, Field, Function, Input, Layout, List, Partial, Pin, Reaction, Shape, Statement, Str, Term, Watch,
    kind,
};

/// A frozen shape, shared by every instance of equal structure.
pub type ShapeRef = Arc<ShapeBuf>;

/// One pin (abi.md §3.1). The layout is copied, names included.
#[derive(Clone, Copy, Debug)]
pub struct PinSpec<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub kind: u32,
    pub role: u32,
    pub global: u32,
    pub flags: u32,
    pub layout: &'a Layout,
    pub unit: &'a str,
    pub init: &'a [u8],
    pub check: u32,
    pub abstol: [f64; 2],
}

impl<'a> PinSpec<'a> {
    /// A continuous pin: one `F64` (`layout::F64`).
    pub fn continuous(name: &'a str, role: u32) -> Self {
        Self::with(name, kind::CONTINUOUS, role, &crate::layout::F64)
    }

    /// A discrete pin of the given layout.
    pub fn discrete(name: &'a str, role: u32, layout: &'a Layout) -> Self {
        Self::with(name, kind::DISCRETE, role, layout)
    }

    /// An event pin: no bytes (`layout::NONE`).
    pub fn event(name: &'a str, role: u32) -> Self {
        Self::with(name, kind::EVENT, role, &crate::layout::NONE)
    }

    fn with(name: &'a str, kind: u32, role: u32, layout: &'a Layout) -> Self {
        PinSpec {
            name,
            origin: "",
            kind,
            role,
            global: 0,
            flags: 0,
            layout,
            unit: "",
            init: &[],
            check: 0,
            abstol: [0.0, 0.0],
        }
    }
}

/// One term.
#[derive(Clone, Copy, Debug)]
pub struct TermSpec<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub op: u32,
    pub plus: u32,
    pub minus: u32,
    pub abstol: u32,
    pub max: u32,
}

/// One held input.
#[derive(Clone, Copy, Debug)]
pub struct InputSpec<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub kind: u32,
}

/// One watch. The layout is copied, names included.
#[derive(Clone, Copy, Debug)]
pub struct WatchSpec<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub kind: u32,
    pub layout: &'a Layout,
    pub event: u32,
    pub ttol: u32,
    pub etol: u32,
}

/// One function.
#[derive(Clone, Debug, Default)]
pub struct FunctionSpec<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub flags: u32,
    pub reads: Vec<Cell>,
    pub writes: Vec<Cell>,
    pub partials: Vec<Partial>,
}

/// One reaction.
#[derive(Clone, Debug, Default)]
pub struct ReactionSpec<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub flags: u32,
    pub on: Vec<u32>,
    pub reads: Vec<Cell>,
    pub writes: Vec<Statement>,
}

/// A shape and the storage of everything it points to.
#[derive(Debug)]
pub struct ShapeBuf {
    shape: Shape,
    pins: Vec<Pin>,
    terms: Vec<Term>,
    inputs: Vec<Input>,
    watches: Vec<Watch>,
    functions: Vec<Function>,
    reactions: Vec<Reaction>,
    strings: Vec<Box<str>>,
    bytes: Vec<Box<[u8]>>,
    fields: Vec<Box<[Field]>>,
    cells: Vec<Box<[Cell]>>,
    partials: Vec<Box<[Partial]>>,
    statements: Vec<Box<[Statement]>>,
    events: Vec<Box<[u32]>>,
}

impl Default for ShapeBuf {
    fn default() -> Self {
        Self::new()
    }
}

/// The index the next entry of a list of `len` entries gets.
fn index(len: usize) -> u32 {
    u32::try_from(len).expect("a shape list holds at most u32::MAX entries")
}

impl ShapeBuf {
    /// An empty shape.
    pub fn new() -> ShapeBuf {
        ShapeBuf {
            shape: Shape {
                size: size_of::<Shape>() as u32,
                pins: List::empty(),
                terms: List::empty(),
                inputs: List::empty(),
                watches: List::empty(),
                functions: List::empty(),
                reactions: List::empty(),
            },
            pins: Vec::new(),
            terms: Vec::new(),
            inputs: Vec::new(),
            watches: Vec::new(),
            functions: Vec::new(),
            reactions: Vec::new(),
            strings: Vec::new(),
            bytes: Vec::new(),
            fields: Vec::new(),
            cells: Vec::new(),
            partials: Vec::new(),
            statements: Vec::new(),
            events: Vec::new(),
        }
    }

    fn str(&mut self, s: &str) -> Str {
        if s.is_empty() {
            return Str::EMPTY;
        }
        let owned: Box<str> = s.into();
        let raw = Str { ptr: owned.as_ptr(), len: owned.len() };
        self.strings.push(owned);
        raw
    }

    fn bytes(&mut self, b: &[u8]) -> Bytes {
        if b.is_empty() {
            return Bytes::EMPTY;
        }
        let owned: Box<[u8]> = b.into();
        let raw = Bytes { ptr: owned.as_ptr(), len: owned.len() };
        self.bytes.push(owned);
        raw
    }

    fn layout(&mut self, l: &Layout) -> Layout {
        let fields: Vec<Field> = l
            .fields
            .iter()
            .map(|f| Field { name: self.str(f.name.as_str()), ..*f })
            .collect();
        let owned: Box<[Field]> = fields.into();
        let list = List::from_slice(&owned);
        self.fields.push(owned);
        Layout { size: l.size, align: l.align, fields: list }
    }

    fn list<T: Copy>(store: &mut Vec<Box<[T]>>, items: &[T]) -> List<T> {
        let owned: Box<[T]> = items.into();
        let list = List::from_slice(&owned);
        store.push(owned);
        list
    }

    /// Adds a pin; returns its index.
    pub fn pin(&mut self, p: PinSpec<'_>) -> u32 {
        let pin = Pin {
            name: self.str(p.name),
            origin: self.str(p.origin),
            kind: p.kind,
            role: p.role,
            global: p.global,
            flags: p.flags,
            layout: self.layout(p.layout),
            unit: self.str(p.unit),
            init: self.bytes(p.init),
            check: p.check,
            abstol: p.abstol,
        };
        let i = index(self.pins.len());
        self.pins.push(pin);
        self.shape.pins = List::from_slice(&self.pins);
        i
    }

    /// Adds a term; returns its index.
    pub fn term(&mut self, t: TermSpec<'_>) -> u32 {
        let term = Term {
            name: self.str(t.name),
            origin: self.str(t.origin),
            op: t.op,
            plus: t.plus,
            minus: t.minus,
            abstol: t.abstol,
            max: t.max,
        };
        let i = index(self.terms.len());
        self.terms.push(term);
        self.shape.terms = List::from_slice(&self.terms);
        i
    }

    /// Adds a held input; returns its index.
    pub fn input(&mut self, x: InputSpec<'_>) -> u32 {
        let input = Input { name: self.str(x.name), origin: self.str(x.origin), kind: x.kind };
        let i = index(self.inputs.len());
        self.inputs.push(input);
        self.shape.inputs = List::from_slice(&self.inputs);
        i
    }

    /// Adds a watch; returns its index.
    pub fn watch(&mut self, w: WatchSpec<'_>) -> u32 {
        let watch = Watch {
            name: self.str(w.name),
            origin: self.str(w.origin),
            kind: w.kind,
            layout: self.layout(w.layout),
            event: w.event,
            ttol: w.ttol,
            etol: w.etol,
        };
        let i = index(self.watches.len());
        self.watches.push(watch);
        self.shape.watches = List::from_slice(&self.watches);
        i
    }

    /// Adds a function; returns its index.
    pub fn function(&mut self, f: FunctionSpec<'_>) -> u32 {
        let function = Function {
            name: self.str(f.name),
            origin: self.str(f.origin),
            flags: f.flags,
            reads: Self::list(&mut self.cells, &f.reads),
            writes: Self::list(&mut self.cells, &f.writes),
            partials: Self::list(&mut self.partials, &f.partials),
        };
        let i = index(self.functions.len());
        self.functions.push(function);
        self.shape.functions = List::from_slice(&self.functions);
        i
    }

    /// Adds a reaction; returns its index.
    pub fn reaction(&mut self, r: ReactionSpec<'_>) -> u32 {
        let reaction = Reaction {
            name: self.str(r.name),
            origin: self.str(r.origin),
            flags: r.flags,
            on: Self::list(&mut self.events, &r.on),
            reads: Self::list(&mut self.cells, &r.reads),
            writes: Self::list(&mut self.statements, &r.writes),
        };
        let i = index(self.reactions.len());
        self.reactions.push(reaction);
        self.shape.reactions = List::from_slice(&self.reactions);
        i
    }

    /// Freezes the shape: nothing is added after this, and the raw view stays
    /// valid until the last clone of the returned `ShapeRef` is dropped.
    pub fn freeze(self) -> ShapeRef {
        Arc::new(self)
    }

    /// The raw shape. Valid while this `ShapeBuf` neither moves nor changes;
    /// behind a `ShapeRef`, until its last clone is dropped.
    pub fn as_raw(&self) -> *const Shape {
        &self.shape
    }

    /// The raw shape, borrowed.
    pub fn shape(&self) -> &Shape {
        &self.shape
    }
}
