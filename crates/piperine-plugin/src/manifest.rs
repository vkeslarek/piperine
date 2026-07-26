//! `piperine-plugin.toml` — the plugin manifest (plugin-interface v2,
//! PLG-21). Intentionally minimal: identity, contribution shape, and
//! permissions. The shape is **inferred from which keys are present** —
//! `python` → scripted, `device` → device, neither → pure-PHDL code —
//! with no `abi` field anywhere (D1); a manifest still carrying
//! `abi = "wasm"|"process"` fails with a targeted removed-backend error
//! (PLG-02), never a generic unknown-field one.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::PluginError;

/// The contribution shape a manifest declares (PLG-21), inferred by
/// [`Manifest::shape`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginShape {
    /// `[plugin]` only — a code library: its `pub` items resolve via
    /// `use`, nothing else runs.
    Pure,
    /// `python = "…"` — scripts/hooks in the embedded-Python host.
    Scripted,
    /// `device = { … }` — a compiled binary through the native backend,
    /// bound to the plugin's `@device` mods.
    Device,
}

impl PluginShape {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginShape::Pure => "pure",
            PluginShape::Scripted => "scripted",
            PluginShape::Device => "device",
        }
    }
}

/// Where a device plugin's binary comes from (design §4). Exactly one
/// source: `path` is a prebuilt binary relative to the plugin root
/// (fixtures, the release-fetch cache); `release` names a
/// `github:owner/repo@tag` release fetched + triple-matched at install
/// time. `verify` pins the fetched asset's hash up front.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSource {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub verify: Option<String>,
}

/// Capability declarations — deny-by-default (SPEC Part VI §3.3).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    /// Glob patterns the plugin may read/write (`"read *.cir"`,
    /// `"write *.ppr"`), relative to the project root.
    #[serde(default)]
    pub filesystem: Vec<String>,
    #[serde(default)]
    pub network: bool,
    /// Whitelist of executables the plugin may spawn; empty = none.
    #[serde(default)]
    pub process_spawn: Vec<String>,
}

impl Default for Permissions {
    fn default() -> Self {
        Self { filesystem: Vec::new(), network: false, process_spawn: Vec::new() }
    }
}

impl Permissions {
    /// Nothing declared — no consent to ask for at `add` time (PLG-23).
    pub fn is_default(&self) -> bool {
        self.filesystem.is_empty() && !self.network && self.process_spawn.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    plugin: PluginSection,
    #[serde(default)]
    permissions: Option<Permissions>,
    /// Removed surface: the in-language bench (and its plugin bench tasks)
    /// no longer exists — a manifest declaring one gets a clear error, not
    /// an "unknown field".
    #[serde(default)]
    bench_tasks: Option<toml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginSection {
    name: String,
    /// Removed in plugin-interface v2 (the backend is inferred from the
    /// contribution keys, never declared). Captured — not denied — so the
    /// removed backends fail with the targeted `RemovedBackend` (PLG-02)
    /// instead of a generic unknown-field error.
    #[serde(default)]
    abi: Option<String>,
    /// Removed alongside `abi` (the artifact is declared via `device` /
    /// `python` now). Captured for the same targeted-error reason: legacy
    /// manifests pair `abi` + `entry`, and a deny-unknown-fields error on
    /// `entry` would mask the `RemovedBackend` the author needs to see.
    #[serde(default)]
    entry: Option<toml::Value>,
    #[serde(default)]
    description: Option<String>,
    /// Scripted shape: the Python script/hook entry, relative to the
    /// plugin root.
    #[serde(default)]
    python: Option<PathBuf>,
    /// Device shape: where the compiled binary comes from.
    #[serde(default)]
    device: Option<DeviceSource>,
    /// Same removed surface as [`ManifestFile::bench_tasks`], in the section
    /// an author would naturally put it.
    #[serde(default)]
    bench_tasks: Option<toml::Value>,
}

/// The parsed, validated manifest the host carries for a plugin's lifetime.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub description: Option<String>,
    /// Python script/hook entry, relative to the plugin root.
    pub python: Option<PathBuf>,
    /// Device binary source.
    pub device: Option<DeviceSource>,
    pub permissions: Permissions,
}

impl Manifest {
    /// The inferred contribution shape (PLG-21): `device` → device,
    /// `python` → scripted, neither → pure-PHDL. A manifest may carry both
    /// `device` and `python` (both load — spec Edge Cases); the shape then
    /// classifies by the load-bearing binary and the loader consults the
    /// fields directly.
    pub fn shape(&self) -> PluginShape {
        if self.device.is_some() {
            PluginShape::Device
        } else if self.python.is_some() {
            PluginShape::Scripted
        } else {
            PluginShape::Pure
        }
    }

    /// Parse a manifest from TOML text. Malformed or incomplete manifests
    /// are `P0006 BadManifest` — validated before any plugin code runs.
    pub fn parse(name_hint: &str, text: &str) -> Result<Self, PluginError> {
        let bad = |reason: String| PluginError::BadManifest { plugin: name_hint.to_string(), reason };
        let file: ManifestFile = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
        if file.bench_tasks.is_some() || file.plugin.bench_tasks.is_some() {
            return Err(bad(
                "`bench_tasks`: the in-language bench was removed — plugin bench tasks no longer \
                 exist; write python testbenches (`*_tb.py`, run by `piperine test`) instead"
                    .into(),
            ));
        }
        if let Some(abi) = &file.plugin.abi {
            return Err(match abi.as_str() {
                "wasm" | "process" => PluginError::RemovedBackend {
                    plugin: name_hint.to_string(),
                    backend: abi.clone(),
                },
                other => bad(format!(
                    "`plugin.abi = \"{other}\"`: the `abi` field no longer exists — the backend \
                     is inferred from the contribution keys (`device` / `python` / neither)"
                )),
            });
        }
        if file.plugin.entry.is_some() {
            return Err(bad(
                "`plugin.entry` no longer exists — a device binary is declared as \
                 `device = { path = \"…\" }`, a script entry as `python = \"…\"`"
                    .into(),
            ));
        }
        if file.plugin.name.is_empty() {
            return Err(bad("`plugin.name` must not be empty".into()));
        }
        if let Some(device) = &file.plugin.device {
            match (&device.path, &device.release) {
                (Some(_), None) | (None, Some(_)) => {}
                _ => {
                    return Err(bad(
                        "`plugin.device` needs exactly one source: a local `path` or a `release` \
                         coordinate"
                            .into(),
                    ));
                }
            }
        }
        Ok(Self {
            name: file.plugin.name,
            description: file.plugin.description,
            python: file.plugin.python,
            device: file.plugin.device,
            permissions: file.permissions.unwrap_or_default(),
        })
    }

    /// Load a plugin's manifest from a resolved plugin directory. Two
    /// spellings, one schema (D9 — a plugin is a contributing dependency):
    /// a dedicated `piperine-plugin.toml`, or the `[plugin]` section of the
    /// package's own `Piperine.toml` (with `[plugin.permissions]`), so
    /// `piperine add <git>` needs no second manifest file. The dedicated
    /// file wins when both exist.
    pub fn load(name_hint: &str, plugin_root: &Path) -> Result<Self, PluginError> {
        let path = plugin_root.join("piperine-plugin.toml");
        if path.is_file() {
            let text = std::fs::read_to_string(&path).map_err(|e| PluginError::BadManifest {
                plugin: name_hint.to_string(),
                reason: format!("{}: {e}", path.display()),
            })?;
            return Self::parse(name_hint, &text);
        }
        Self::from_project_manifest(name_hint, &plugin_root.join("Piperine.toml"))
    }

    /// The inline spelling: the `[plugin]` section of a package's own
    /// `Piperine.toml`, lifted into the same schema [`Self::parse`] reads.
    /// `plugin.name` defaults to `[project].name`, and
    /// `[plugin.permissions]` becomes the manifest's permissions.
    fn from_project_manifest(name_hint: &str, path: &Path) -> Result<Self, PluginError> {
        let bad = |reason: String| PluginError::BadManifest { plugin: name_hint.to_string(), reason };
        let text = std::fs::read_to_string(path)
            .map_err(|e| bad(format!("{}: {e}", path.display())))?;
        let doc: toml::Table = toml::from_str(&text).map_err(|e| bad(e.to_string()))?;
        let Some(toml::Value::Table(mut plugin)) = doc.get("plugin").cloned() else {
            return Err(bad(format!(
                "{}: no `[plugin]` section and no `piperine-plugin.toml` — a plugin declares \
                 its contributions in one of the two",
                path.display()
            )));
        };
        let permissions = plugin.remove("permissions");
        if !plugin.contains_key("name") {
            let project_name = doc
                .get("project")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .ok_or_else(|| {
                    bad(format!("{}: neither `plugin.name` nor `project.name`", path.display()))
                })?;
            plugin.insert("name".into(), toml::Value::String(project_name.to_string()));
        }
        let mut lifted = toml::Table::new();
        lifted.insert("plugin".into(), toml::Value::Table(plugin));
        if let Some(permissions) = permissions {
            lifted.insert("permissions".into(), permissions);
        }
        let lifted = toml::to_string(&toml::Value::Table(lifted)).map_err(|e| bad(e.to_string()))?;
        Self::parse(name_hint, &lifted)
    }
}

#[cfg(test)]
mod tests {
    //! Manifest parsing, shape inference, and removed-field errors
    //! (plugin-interface v2: PLG-02, PLG-21). Moved inline from
    //! `tests/manifest.rs` by P6/CLN-03: these exercise this module's own
    //! `parse`/`load` behaviour, nothing else.

    use super::*;

    #[test]
    fn device_manifest_infers_device_shape() {
        let m = Manifest::parse(
            "avr-cosim",
            r#"
            [plugin]
            name        = "avr-cosim"
            description = "AVR co-simulation"
            device      = { path = "libavr_cosim.so" }

            [permissions]
            filesystem     = ["read *.hex"]
            network        = false
            process_spawn  = ["simavr"]
            "#,
        )
        .expect("parse");
        assert_eq!(m.name, "avr-cosim");
        assert_eq!(m.shape(), PluginShape::Device);
        let device = m.device.as_ref().expect("device source present");
        assert_eq!(device.path.as_deref(), Some(std::path::Path::new("libavr_cosim.so")));
        assert_eq!(m.permissions.filesystem, vec!["read *.hex"]);
        assert_eq!(m.permissions.process_spawn, vec!["simavr"]);
        assert!(!m.permissions.network);
    }

    #[test]
    fn python_manifest_infers_scripted_shape() {
        let m = Manifest::parse(
            "lint",
            r#"
            [plugin]
            name   = "lint"
            python = "plugin.py"
            "#,
        )
        .expect("parse");
        assert_eq!(m.shape(), PluginShape::Scripted);
        assert_eq!(m.python.as_deref(), Some(std::path::Path::new("plugin.py")));
        assert!(m.device.is_none());
    }

    #[test]
    fn bare_plugin_section_infers_pure_phdl_shape() {
        let m = Manifest::parse(
            "models",
            r#"
            [plugin]
            name = "models"
            "#,
        )
        .expect("parse");
        assert_eq!(m.shape(), PluginShape::Pure);
        assert!(m.python.is_none());
        assert!(m.device.is_none());
    }

    /// A plugin may carry BOTH a device binary and a python entry (spec Edge
    /// Cases — both load); the declared shape classifies by the load-bearing
    /// binary, and both keys stay visible on the manifest.
    #[test]
    fn device_and_python_both_parse() {
        let m = Manifest::parse(
            "combo",
            r#"
            [plugin]
            name   = "combo"
            python = "glue.py"
            device = { path = "libcombo.so" }
            "#,
        )
        .expect("parse");
        assert_eq!(m.shape(), PluginShape::Device);
        assert!(m.device.is_some());
        assert_eq!(m.python.as_deref(), Some(std::path::Path::new("glue.py")));
    }

    /// PLG-02: a manifest declaring a removed backend fails with a targeted
    /// `RemovedBackend` naming the backend — never a generic unknown-field or
    /// unknown-value error.
    #[test]
    fn removed_backends_are_a_targeted_error() {
        for backend in ["wasm", "process"] {
            let src = format!("[plugin]\nname = \"x\"\nabi = \"{backend}\"\nentry = \"x.bin\"\n");
            let err = Manifest::parse("x", &src).expect_err(&src);
            match &err {
                PluginError::RemovedBackend { backend: named, .. } => {
                    assert_eq!(named, backend, "the error names the removed backend");
                }
                other => panic!("{backend}: expected RemovedBackend, got {other}"),
            }
            let msg = err.to_string();
            assert!(msg.contains(backend), "message names the removed backend: {msg}");
            assert!(msg.contains("removed"), "message says it was removed: {msg}");
        }
    }

    /// Any other `abi` value is not a removed-backend case but still not a v2
    /// field: a bad-manifest error that names the field, never silently parsed.
    #[test]
    fn any_other_abi_field_is_rejected() {
        for src in [
            "[plugin]\nname = \"x\"\nabi = \"native\"\nentry = \"x.so\"\n",
            "[plugin]\nname = \"x\"\nabi = \"exe\"\n",
        ] {
            let err = Manifest::parse("x", src).expect_err(src);
            assert!(matches!(err, PluginError::BadManifest { .. }), "{src}: {err}");
            assert!(!matches!(err, PluginError::RemovedBackend { .. }), "{src}: {err}");
            assert!(err.to_string().contains("abi"), "{src}: {err}");
        }
    }

    /// A `device` table needs exactly one source: a local `path` or a
    /// `release` coordinate — never neither, never both.
    #[test]
    fn device_source_needs_exactly_one_of_path_or_release() {
        for src in [
            "[plugin]\nname = \"x\"\ndevice = {}\n",
            "[plugin]\nname = \"x\"\ndevice = { path = \"x.so\", release = \"github:a/b@v1\" }\n",
        ] {
            let err = Manifest::parse("x", src).expect_err(src);
            assert!(matches!(err, PluginError::BadManifest { .. }), "{src}: {err}");
        }
        let m = Manifest::parse(
            "x",
            "[plugin]\nname = \"x\"\ndevice = { release = \"github:a/b@v1\", verify = \"sha256:ab\" }\n",
        )
        .expect("a release-only device source parses");
        assert_eq!(m.shape(), PluginShape::Device);
    }

    #[test]
    fn minimal_manifest_gets_default_permissions() {
        let m = Manifest::parse("x", "[plugin]\nname = \"x\"\n").expect("parse");
        assert!(m.permissions.filesystem.is_empty());
        assert!(!m.permissions.network);
        assert!(m.permissions.process_spawn.is_empty());
    }

    #[test]
    fn empty_name_and_malformed_toml_are_bad_manifest() {
        for src in ["[plugin]\nname = \"\"\n", "not toml at all ["] {
            let err = Manifest::parse("x", src).expect_err(src);
            assert!(matches!(err, PluginError::BadManifest { .. }), "{src}: {err}");
        }
    }

    #[test]
    fn unknown_permission_field_is_rejected() {
        let err = Manifest::parse(
            "x",
            r#"
            [plugin]
            name = "x"

            [permissions]
            sudo = true
            "#,
        )
        .expect_err("unknown permission must not parse");
        assert!(matches!(err, PluginError::BadManifest { .. }));
    }

    /// A manifest declaring `bench_tasks` fails loud with the removal notice —
    /// the in-language bench (and its plugin extension point) no longer exists;
    /// the generic "unknown field" would send authors hunting the wrong trail.
    #[test]
    fn bench_tasks_manifest_is_a_clear_removal_error() {
        let err = Manifest::parse(
            "x",
            r#"
            [plugin]
            name = "x"

            bench_tasks = ["gain"]
            "#,
        )
        .expect_err("bench_tasks must be rejected");
        let msg = err.to_string();
        assert!(msg.contains("bench"), "names the removed surface: {msg}");
        assert!(msg.contains("removed"), "says it is removed: {msg}");
        assert!(msg.contains("*_tb.py"), "points at python testbenches: {msg}");
    }

    /// D9 — a contributing dependency declares `[plugin]` inline in its own
    /// `Piperine.toml` (no second manifest file): `plugin.name` defaults to
    /// `project.name` and `[plugin.permissions]` becomes the permissions.
    #[test]
    fn an_inline_plugin_section_is_a_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Piperine.toml"),
            r#"
    [project]
    name = "acme-bjt"
    version = "0.1.0"
    authors = []
    edition = "2024"

    [plugin]
    python = "plugin.py"

    [plugin.permissions]
    filesystem = ["read *.model"]
    "#,
        )
        .unwrap();

        let m = Manifest::load("acme-bjt", dir.path()).expect("inline [plugin] manifest");
        assert_eq!(m.name, "acme-bjt");
        assert_eq!(m.shape(), PluginShape::Scripted);
        assert_eq!(m.permissions.filesystem, vec!["read *.model".to_string()]);
        assert!(!m.permissions.is_default());
    }

    /// A dedicated `piperine-plugin.toml` wins over the inline section — an
    /// artifact-only plugin vendored next to a package keeps its own manifest.
    #[test]
    fn a_dedicated_manifest_wins_over_the_inline_section() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Piperine.toml"),
            "[project]\nname = \"pkg\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
             [plugin]\npython = \"inline.py\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("piperine-plugin.toml"),
            "[plugin]\nname = \"dedicated\"\npython = \"dedicated.py\"\n",
        )
        .unwrap();

        let m = Manifest::load("pkg", dir.path()).expect("dedicated manifest");
        assert_eq!(m.name, "dedicated");
        assert_eq!(m.python.as_deref(), Some(std::path::Path::new("dedicated.py")));
    }

    /// A package with neither spelling is not a plugin — a loud manifest error
    /// naming both options, never a silent empty manifest.
    #[test]
    fn a_package_declaring_no_contributions_fails_loud() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Piperine.toml"),
            "[project]\nname = \"plain\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
        )
        .unwrap();

        let err = Manifest::load("plain", dir.path()).expect_err("no [plugin] section");
        let msg = err.to_string();
        assert!(msg.contains("[plugin]") && msg.contains("piperine-plugin.toml"), "{msg}");
    }
}
