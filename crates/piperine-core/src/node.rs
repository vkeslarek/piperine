use std::fmt;

/// A circuit node identifier.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Node {
    /// Ground node (SPICE node "0")
    Ground,
    /// Named node (e.g. "in", "out", "vcc")
    Named(String),
    /// Indexed node (integer > 0)
    Indexed(usize),
}

/// Ground constant for convenience.
pub const GND: Node = Node::Ground;

impl Node {
    pub fn is_ground(&self) -> bool {
        matches!(self, Node::Ground)
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Ground => write!(f, "0"),
            Node::Named(name) => write!(f, "{}", name),
            Node::Indexed(idx) => write!(f, "{}", idx),
        }
    }
}

impl From<&str> for Node {
    fn from(s: &str) -> Self {
        match s {
            "0" | "gnd" | "GND" | "Gnd" => Node::Ground,
            _ => Node::Named(s.to_string()),
        }
    }
}

impl From<String> for Node {
    fn from(s: String) -> Self {
        match s.as_str() {
            "0" | "gnd" | "GND" | "Gnd" => Node::Ground,
            _ => Node::Named(s),
        }
    }
}

impl From<usize> for Node {
    fn from(n: usize) -> Self {
        if n == 0 { Node::Ground } else { Node::Indexed(n) }
    }
}

impl From<&Node> for Node {
    fn from(n: &Node) -> Self {
        n.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_variants() {
        assert_eq!(Node::from("0"), Node::Ground);
        assert_eq!(Node::from("GND"), Node::Ground);
        assert_eq!(Node::from("gnd"), Node::Ground);
        assert_eq!(Node::from(0usize), Node::Ground);
    }

    #[test]
    fn display() {
        assert_eq!(GND.to_string(), "0");
        assert_eq!(Node::Named("out".into()).to_string(), "out");
        assert_eq!(Node::Indexed(5).to_string(), "5");
    }
}
