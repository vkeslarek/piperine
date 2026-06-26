use std::path::Path;

pub fn execute(file: String) {
    let path = Path::new(&file);
    println!("Checking file: {}", path.display());
    
    match piperine_parser::parse_file(path) {
        Ok(doc) => {
            println!("Parse successful! Captured models:");
            println!("  Modules: {}", doc.modules.len());
            for m in &doc.modules {
                println!("    - {} (ports: {}, parameters: {}, variables: {}, analog_blocks: {}, instances: {}, tasks: {}, continuous_assigns: {})", 
                    m.name, m.ports.len(), m.parameters.len(), m.variables.len(), m.analog_blocks.len(), m.instances.len(), m.tasks.len(), m.continuous_assigns.len());
            }
            println!("  Disciplines: {}", doc.disciplines.len());
            println!("  Natures: {}", doc.natures.len());
            println!("  Paramsets: {}", doc.paramsets.len());
            println!("  Connectrules: {}", doc.connectrules.len());
            println!("  Configs: {}", doc.configs.len());
            println!("  Primitives: {}", doc.primitives.len());
        }
        Err(e) => {
            eprintln!("Parse failed: {}", e);
            std::process::exit(1);
        }
    }
}
