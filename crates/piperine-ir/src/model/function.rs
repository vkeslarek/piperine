//! Functions: their data and the generic instances they are made from.

use serde::{Deserialize, Serialize};

use crate::ids::ExprId;
use crate::model::{Local, Origin, ValueType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionData {
    pub(crate) name: String,
    pub(crate) origin: Origin,
    pub(crate) params: Vec<Local>,
    pub(crate) ret: ValueType,
    pub(crate) locals: Vec<Local>,
    pub(crate) body: ExprId,
    /// The generic item this function is one instantiation of; `None` for a function as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instance: Option<GenericInstance>,
}

/// One instantiation of a generic item: the item and the arguments, each printed as the source
/// spells it (`swap::<f64>` is item `swap`, arguments `["f64"]`). The IR name of the instance is
/// [`GenericInstance::name`], which `Ir::check` holds the function or type to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericInstance {
    /// The generic item's name as written.
    pub item: String,
    /// The generic arguments in order, a type, a const value or a function, each as printed.
    pub arguments: Vec<String>,
}

impl GenericInstance {
    /// The name of the instance, `item::<a, b>` (design D8): what its IR function carries.
    pub fn name(&self) -> String {
        format!("{}::<{}>", self.item, self.arguments.join(", "))
    }

    /// The name of the instance of a generic type, `item<a, b>` (design D8).
    pub fn type_name(&self) -> String {
        format!("{}<{}>", self.item, self.arguments.join(", "))
    }

    /// Whether `name` is the name of a body that lowers this instance of a generic module: the instance's name, then the
    /// values of the module's key params when it has any (`Ladder::<3>~r=1k`).
    pub fn names_body(&self, name: &str) -> bool {
        let own = self.name();
        name.strip_prefix(&own).is_some_and(|rest| rest.is_empty() || rest.starts_with('~'))
    }

    /// Whether `name` is this instance's name, or that name followed by the half of an `Option`
    /// result the function computes (`.is` or `.val`, `elab/variants.rs`).
    pub fn names(&self, name: &str) -> bool {
        let own = self.name();
        name.strip_prefix(&own).is_some_and(|rest| matches!(rest, "" | ".is" | ".val"))
    }
}
