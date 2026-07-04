use piperine_project::{PiperineToml, get_current_project_root, resolver::Resolver};
use std::env;
use std::fs;
use toml_edit::{DocumentMut, InlineTable, Item, Value};

pub fn execute(
    name: String,
    git: Option<String>,
    version: Option<String>,
    branch: Option<String>,
    rev: Option<String>,
    path: Option<String>,
) {
    let project_root = get_current_project_root().unwrap_or_else(|| env::current_dir().unwrap());
    let toml_path = project_root.join("Piperine.toml");

    if !toml_path.exists() {
        eprintln!("Error: Piperine.toml not found in the current directory.");
        std::process::exit(1);
    }

    let toml_content = fs::read_to_string(&toml_path).expect("Failed to read Piperine.toml");
    let mut doc = toml_content
        .parse::<DocumentMut>()
        .expect("Failed to parse Piperine.toml");

    let deps = doc["dependencies"].or_insert(Item::Table(toml_edit::Table::new()));
    let deps_table = deps.as_table_mut().expect("dependencies must be a table");

    let mut table = InlineTable::new();
    if let Some(p) = path {
        table.insert("path", p.into());
    } else if let Some(g) = git {
        table.insert("git", g.into());
        if let Some(v) = version {
            table.insert("version", v.into());
        } else if let Some(b) = branch {
            table.insert("branch", b.into());
        } else if let Some(r) = rev {
            table.insert("rev", r.into());
        }
    } else {
        eprintln!("Error: Must specify either --git or --path");
        std::process::exit(1);
    }

    deps_table.insert(&name, Item::Value(Value::InlineTable(table)));

    // Try resolving to see if the dependency exists
    println!("Resolving dependency '{}'...", name);
    // Write temporarily to check
    let new_content = doc.to_string();
    fs::write(&toml_path, &new_content).expect("Failed to write Piperine.toml");

    let piperine_toml = match PiperineToml::load(&toml_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to parse updated Piperine.toml: {}", e);
            // Revert
            fs::write(&toml_path, toml_content).ok();
            std::process::exit(1);
        }
    };

    let mut resolver = Resolver::new(&project_root, false);

    match resolver.resolve(&piperine_toml) {
        Ok(_) => {
            println!("Successfully added '{}' to dependencies.", name);
        }
        Err(e) => {
            eprintln!("Error: Failed to fetch dependency: {}", e);
            // Revert changes
            fs::write(&toml_path, toml_content).expect("Failed to revert Piperine.toml");
            std::process::exit(1);
        }
    }
}
