//! Every value the compiled backend reads from the environment, in one place (design
//! "Settings"). [`Settings::resolve`] is pure over a lookup, so tests pass a table and never
//! mutate the process environment; [`Settings::from_environment`] is the one reader.

use std::path::PathBuf;

use crate::error::Error;

/// The compiler named by the user, if any, and the cache directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `PIPERINE_RUSTC`; `None` means `rustc` on `PATH` (RS-03 AC 1).
    pub rustc: Option<PathBuf>,
    /// The cache directory (RS-04 AC 9).
    pub cache: PathBuf,
}

impl Settings {
    /// The settings of the process environment.
    pub fn from_environment() -> Result<Settings, Error> {
        Settings::resolve(&|name| std::env::var(name).ok())
    }

    /// The settings `lookup` gives; an empty value counts as unset. The cache directory is
    /// the first of: `PIPERINE_CACHE_DIR` itself, `$XDG_CACHE_HOME/piperine`,
    /// `$HOME/.cache/piperine`, `%LOCALAPPDATA%\piperine`; none of the four is an
    /// `Error::Setting`.
    pub fn resolve(lookup: &dyn Fn(&str) -> Option<String>) -> Result<Settings, Error> {
        let get = |name: &str| lookup(name).filter(|v| !v.is_empty());
        let rustc = get("PIPERINE_RUSTC").map(PathBuf::from);
        let cache = if let Some(dir) = get("PIPERINE_CACHE_DIR") {
            PathBuf::from(dir)
        } else if let Some(xdg) = get("XDG_CACHE_HOME") {
            PathBuf::from(xdg).join("piperine")
        } else if let Some(home) = get("HOME") {
            PathBuf::from(home).join(".cache").join("piperine")
        } else if let Some(local) = get("LOCALAPPDATA") {
            PathBuf::from(local).join("piperine")
        } else {
            return Err(Error::Setting {
                name: "PIPERINE_CACHE_DIR",
                value: String::new(),
                valid: "a directory; or set XDG_CACHE_HOME, HOME or LOCALAPPDATA for the default",
            });
        };
        Ok(Settings { rustc, cache })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::Settings;
    use crate::error::Error;

    /// Associated test helpers of `settings`, grouped on one fixture type.
    struct SettingsFixture;

    impl SettingsFixture {
        fn resolve(table: &[(&str, &str)]) -> Result<Settings, Error> {
            let map: HashMap<String, String> = table.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            Settings::resolve(&|name| map.get(name).cloned())
        }
    }

    // RS-04 AC 9: PIPERINE_CACHE_DIR, else $XDG_CACHE_HOME/piperine, else
    // $HOME/.cache/piperine, else %LOCALAPPDATA%\piperine.
    #[test]
    fn the_cache_directory_follows_the_four_rules() {
        let all = [
            ("PIPERINE_CACHE_DIR", "/pc"),
            ("XDG_CACHE_HOME", "/xdg"),
            ("HOME", "/home/u"),
            ("LOCALAPPDATA", "/local"),
        ];
        assert_eq!(SettingsFixture::resolve(&all).unwrap().cache, PathBuf::from("/pc"));
        assert_eq!(SettingsFixture::resolve(&all[1..]).unwrap().cache, PathBuf::from("/xdg/piperine"));
        assert_eq!(SettingsFixture::resolve(&all[2..]).unwrap().cache, PathBuf::from("/home/u/.cache/piperine"));
        assert_eq!(SettingsFixture::resolve(&all[3..]).unwrap().cache, PathBuf::from("/local/piperine"));
        // an empty value is unset
        assert_eq!(
            SettingsFixture::resolve(&[("PIPERINE_CACHE_DIR", ""), ("HOME", "/home/u")]).unwrap().cache,
            PathBuf::from("/home/u/.cache/piperine")
        );
        // none of the four: an error naming the setting
        assert!(matches!(SettingsFixture::resolve(&[]), Err(Error::Setting { name: "PIPERINE_CACHE_DIR", .. })));
    }

    // RS-03 AC 1: PIPERINE_RUSTC, else `rustc` on PATH (None here).
    #[test]
    fn the_compiler_is_the_variable_then_the_path() {
        let home = ("HOME", "/h");
        assert_eq!(SettingsFixture::resolve(&[("PIPERINE_RUSTC", "/opt/rustc"), home]).unwrap().rustc, Some(PathBuf::from("/opt/rustc")));
        assert_eq!(SettingsFixture::resolve(&[home]).unwrap().rustc, None);
        assert_eq!(SettingsFixture::resolve(&[("PIPERINE_RUSTC", ""), home]).unwrap().rustc, None);
    }
}
