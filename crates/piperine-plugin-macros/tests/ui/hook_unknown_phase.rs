fn main() {}

#[piperine_plugin_macros::hook(on_startup)]
fn f(_ctx: &piperine_plugin::Ctx) -> Result<(), String> {
    Ok(())
}
