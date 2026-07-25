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
    // A denied/failed add restores the lockfile too — nothing installed.
    let lock_path = project_root.join("Piperine.lock");
    let lock_content = fs::read_to_string(&lock_path).ok();
    let revert = |toml_path: &std::path::Path, toml_content: &str| {
        fs::write(toml_path, toml_content).expect("Failed to revert Piperine.toml");
        match &lock_content {
            Some(content) => fs::write(&lock_path, content).expect("Failed to revert Piperine.lock"),
            None => {
                if lock_path.exists() {
                    fs::remove_file(&lock_path).expect("Failed to revert Piperine.lock")
                }
            }
        }
    };

    let piperine_toml = match PiperineToml::load(toml_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to parse updated Piperine.toml: {}", e);
            revert(toml_path, toml_content);
            std::process::exit(1);
        }
    };

    let mut resolver = Resolver::new(project_root, false);

    let resolved = match resolver.resolve(&piperine_toml) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Error: Failed to fetch dependency: {}", e);
            revert(toml_path, toml_content);
            std::process::exit(1);
        }
    };

    // Permissions consent (plugin-interface v2, PLG-23 / D11): a
    // dependency declaring `[plugin.permissions]` is a plugin — print
    // them and require an explicit accept/deny. A deny aborts the
    // install; there is no silent-accept default.
    if let Some(dir) = resolved.get(name) {
        let dir = if dir.is_absolute() { dir.clone() } else { project_root.join(dir) };
        if dir.join("piperine-plugin.toml").exists() {
            let consent = piperine_plugin::Manifest::load(name, &dir).and_then(|manifest| {
                let mode = match std::env::var("PIPERINE_PLUGIN_TRUST").as_deref() {
                    Ok("accept") => piperine_plugin::TrustMode::AcceptAll,
                    Ok("reject") => piperine_plugin::TrustMode::RejectUntrusted,
                    _ => piperine_plugin::TrustMode::Interactive,
                };
                piperine_plugin::ensure_permissions_consented(&manifest, mode)
            });
            if let Err(e) = consent {
                eprintln!("Error: {e}");
                revert(toml_path, toml_content);
                std::process::exit(1);
            }
        }
    }

    println!("Successfully added '{}' to dependencies.", name);
}
