pub mod netlist;

use crate::devices::Component;
use std::sync::Arc;

pub struct Circuit {
    title: String,
    components: Vec<Arc<dyn Component>>,
}
