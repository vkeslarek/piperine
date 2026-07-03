#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    /// True if the path starts from the context root (e.g. `/` or `//` at the start)
    pub absolute: bool,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// True if this step follows a `//` operator, meaning any descendant.
    pub is_descendant: bool,
    pub axis: Axis,
    pub test: NodeTest,
    pub predicates: Vec<Predicate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Inst,
    Net,
    Port,
    Param,
    Attr,
    Behavior,
    Driver,
    Load,
    Parent,
    Ancestor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeTest {
    Any,
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    Index(usize),
    Last,
    Expr(PredExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PredExpr {
    Or(Box<PredExpr>, Box<PredExpr>),
    And(Box<PredExpr>, Box<PredExpr>),
    Not(Box<PredExpr>),
    Compare(Compare),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Compare {
    pub lhs: Operand,
    pub rhs: Option<(CmpOp, Operand)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Glob, // ~
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    AttrRef(String),
    AxisRef(Axis, NodeTest),
    FuncOf(String),
    FuncCount(Axis, NodeTest),
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    String(String),
    Bool(bool),
    Ident(String),
}
