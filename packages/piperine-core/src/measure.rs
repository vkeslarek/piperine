use crate::circuit::{BranchIdentifier, NodeIdentifier};

pub enum Measure {
    Voltage(NodeIdentifier),
    Current(BranchIdentifier),
}
