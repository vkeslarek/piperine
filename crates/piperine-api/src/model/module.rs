//! [`Module`] — a navigable view of one named module in a shared
//! [`Design`](crate::model::Design) (CLA-17).
//!
//! The view stores `(Rc<Design>, name)` and re-resolves the module on each
//! call, so no accessor holds a POM borrow open for the view's lifetime.

use std::rc::Rc;

use crate::error::Error;
use crate::model::Instance;

/// A navigable view of a named module.
///
/// **Navigation walks the authored hierarchy** (MD-25): [`Module::instances`]
/// yields the instances the author wrote — one entry per authored instance, not
/// the leaf-only splice codegen's flattened side artifact carries. Descending is
/// `instance.module()` → [`Design::module`](crate::model::Design::module), and
/// the tree stays walkable to any depth.
#[derive(Clone)]
pub struct Module {
    design: Rc<piperine_lang::Design>,
    name: String,
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module").field("name", &self.name).finish()
    }
}

impl Module {
    /// Build a view of `name` in `design`. Constructed by
    /// [`Design::module`](crate::model::Design::module)/`top`/`modules`.
    pub(crate) fn new(design: Rc<piperine_lang::Design>, name: String) -> Self {
        Self { design, name }
    }

    /// Re-resolve the live module from the shared POM — the **authored** map,
    /// never the flattened one (MD-25).
    fn pom(&self) -> Result<&piperine_lang::Module, Error> {
        self.design
            .module(&self.name)
            .ok_or_else(|| Error::NotFound(format!("module `{}` is no longer present", self.name)))
    }

    /// The module's declared name (re-resolved against the live POM).
    pub fn name(&self) -> Result<&str, Error> {
        Ok(self.pom()?.name())
    }

    /// The module's submodule instances, as the author wrote them (MD-25).
    pub fn instances(&self) -> Result<Vec<Instance>, Error> {
        Ok(self.pom()?.instances().iter().map(Instance::of).collect())
    }
}
