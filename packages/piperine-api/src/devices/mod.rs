use crate::expression::Expr;
use crate::num::Scalar;

pub mod components;
pub mod generated;

pub use components::*;
pub use generated::DEVICE_METADATA;

pub trait Component {
    fn name(&self) -> &str;
}

pub trait Model {
    type ComponentType: Component;
}

#[derive(Debug, Clone)]
pub enum Dynamic<T: Scalar> {
    Literal(T),
    Expression(Expr),
}

impl<T: Scalar> From<T> for Dynamic<T> {
    fn from(val: T) -> Self {
        Dynamic::Literal(val)
    }
}

impl<T: Scalar> From<Expr> for Dynamic<T> {
    fn from(expr: Expr) -> Self {
        Dynamic::Expression(expr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Real,
    Integer,
    Complex,
    Flag,
    String,
    Node,
    InstanceRef,
    Parsetree,
    RealVector,
    IntVector,
    StringVector,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Instance,
    Model,
    Output,
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub keyword: &'static str,
    pub value_type: ValueType,
    pub description: &'static str,
    pub kind: ParameterKind,
}

#[derive(Debug, Clone)]
pub struct DeviceMetadata {
    pub key: &'static str,
    pub prefix: &'static str,
    pub nodes: &'static [&'static str],
    pub instance_params: &'static [ParameterMeta],
    pub model_params: &'static [ParameterMeta],
    pub output_params: &'static [ParameterMeta],
}

impl DeviceMetadata {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn find_parameter(&self, keyword: &str) -> Option<&'static ParameterMeta> {
        self.instance_params
            .iter()
            .chain(self.model_params.iter())
            .chain(self.output_params.iter())
            .find(|meta| meta.keyword.eq_ignore_ascii_case(keyword))
    }
}

pub fn device_keys() -> impl Iterator<Item = &'static str> {
    DEVICE_METADATA.iter().map(|d| d.key)
}

pub fn find_device(key: &str) -> Option<&'static DeviceMetadata> {
    DEVICE_METADATA.iter().find(|d| d.key == key)
}
