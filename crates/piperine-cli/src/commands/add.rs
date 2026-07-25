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
        deps_table.insert(&name, Item::Value(Value::InlineTable(table)));
        resolve_or_revert(&name, &project_root, &toml_path, &toml_content, doc);
        return;
    }
    // Go-style source resolution (plugin-interface v2, PLG-22 / D9): with
    // no `--git` flag the positional argument IS the source — a bare
    // `owner/repo` resolves to GitHub, a full git URL is used verbatim,
    // and the package name is derived from the URL.
    let (name, git_url) = match git {
        Some(g) => (name, g),
        None => match piperine_project::git::GitSource::parse(&name) {
            Ok(src) => (src.name().to_string(), src.url().to_string()),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
    };
    table.insert("git", git_url.into());
    if let Some(v) = version {
        table.insert("version", v.into());
    } else if let Some(b) = branch {
        table.insert("branch", b.into());
    } else if let Some(r) = rev {
        table.insert("rev", r.into());
    }

    deps_table.insert(&name, Item::Value(Value::InlineTable(table)));
    resolve_or_revert(&name, &project_root, &toml_path, &toml_content, doc);
}

/// Write the updated manifest, resolve the new dependency, and revert the
/// manifest on failure — a failed `add` leaves the project untouched.
fn resolve_or_revert(
    name: &str,
    project_root: &std::path::Path,
    toml_path: &std::path::Path,
    toml_content: &str,
    doc: DocumentMut,
) {
    // Try resolving to see if the dependency exists
    println!("Resolving dependency '{}'...", name);
    // Write temporarily to check
    let new_content = doc.to_string();
    fs::write(toml_path, &new_content).expect("Failed to write Piperine.toml");

    let piperine_toml = match PiperineToml::load(toml_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to parse updated Piperine.toml: {}", e);
            // Revert
            fs::write(toml_path, toml_content).ok();
            std::process::exit(1);
        }
    };

    let mut resolver = Resolver::new(project_root, false);

    match resolver.resolve(&piperine_toml) {
        Ok(_) => {
            println!("Successfully added '{}' to dependencies.", name);
        }
        Err(e) => {
            eprintln!("Error: Failed to fetch dependency: {}", e);
            // Revert changes
            fs::write(toml_path, toml_content).expect("Failed to revert Piperine.toml");
            std::process::exit(1);
        }
    }
}
