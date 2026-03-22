/// Trait for types that produce SPICE netlist lines.
///
/// Netlist lines are circuit topology definitions: element instances,
/// .model, .subckt/.ends, .param, .ic, .include, .lib.
/// These go between the title line and .end.
pub trait ToNetlist {
    fn to_netlist_lines(&self) -> Vec<String>;
}
