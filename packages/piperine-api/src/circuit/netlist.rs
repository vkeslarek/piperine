use crate::devices::Component;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum NodeIdentifier {
    Named(String),
    Indexed(usize),
    Gnd,
}

pub trait IntoNodeIdentifier: Into<NodeIdentifier> {}
impl<T> IntoNodeIdentifier for T where T: Into<NodeIdentifier> {}

impl Into<NodeIdentifier> for usize {
    fn into(self) -> NodeIdentifier {
        if self == 0 {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Indexed(self)
        }
    }
}

impl From<&str> for NodeIdentifier {
    fn from(name: &str) -> Self {
        if name.to_uppercase() == "GND" {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Named(name.to_string())
        }
    }
}

impl Into<NodeIdentifier> for String {
    fn into(self) -> NodeIdentifier {
        if self == "GND" {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Named(self)
        }
    }
}

pub const GND: NodeIdentifier = NodeIdentifier::Gnd;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BranchIdentifier {
    pub component: String,
    pub name: Option<String>,
}

impl BranchIdentifier {
    pub fn new(component_name: impl Into<String>, branch_name: impl Into<String>) -> Self {
        Self {
            component: component_name.into(),
            name: Some(branch_name.into()),
        }
    }

    pub fn from_component(component_name: impl Into<String>) -> Self {
        Self {
            component: component_name.into(),
            name: None,
        }
    }
}

impl Into<BranchIdentifier> for String {
    fn into(self) -> BranchIdentifier {
        BranchIdentifier {
            component: self,
            name: None,
        }
    }
}

impl Into<BranchIdentifier> for &str {
    fn into(self) -> BranchIdentifier {
        BranchIdentifier {
            component: self.to_string(),
            name: None,
        }
    }
}

pub struct ComponentIdentifier(String);

impl From<&str> for ComponentIdentifier {
    fn from(value: &str) -> Self {
        ComponentIdentifier(value.to_string())
    }
}

impl From<String> for ComponentIdentifier {
    fn from(value: String) -> Self {
        ComponentIdentifier(value)
    }
}

impl<C: Component> From<C> for ComponentIdentifier {
    fn from(value: C) -> Self {
        value.name().clone().into()
    }
}
