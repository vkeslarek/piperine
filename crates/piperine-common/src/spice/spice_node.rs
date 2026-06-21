use std::fmt;

use serde::{Deserialize, Serialize};

use super::node::Node;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiceNode(String);

impl fmt::Display for SpiceNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Node> for SpiceNode {
    fn from(n: Node) -> Self {
        match n {
            Node::Named(s) => SpiceNode(s),
            Node::Indexed(i) => SpiceNode(i.to_string()),
            Node::Ground => SpiceNode("0".into()),
        }
    }
}

impl From<&str> for SpiceNode {
    fn from(s: &str) -> Self { SpiceNode(s.to_string()) }
}

impl From<String> for SpiceNode {
    fn from(s: String) -> Self { SpiceNode(s) }
}

impl From<usize> for SpiceNode {
    fn from(i: usize) -> Self { SpiceNode(i.to_string()) }
}

impl From<i32> for SpiceNode {
    fn from(i: i32) -> Self { SpiceNode(i.to_string()) }
}
