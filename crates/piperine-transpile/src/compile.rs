//! `Transpiler::compile`: print, key, look in the cache, build on a miss (the ABI `rlib` once,
//! then the design's `cdylib`), load (design "Architecture Overview"; RS-01 to RS-04, RS-08).
//! Nothing here runs during an analysis: a design is compiled once, before the first one.

use std::path::Path;
use std::time::Instant;

use crate::abi_source::AbiSource;
use crate::cache::{Cache, Entry, Key};
use crate::compiled::{Compiled, Report};
use crate::error::Error;
use crate::load::Loaded;
use crate::rt::Runtime;
use crate::rustc::Rustc;
use crate::settings::Settings;
use crate::Transpiler;

impl Transpiler {
    /// Compiles the design with the compiler `settings` names, through the cache at
    /// `settings.cache`, and loads it.
    ///
    /// The cache key is the SHA-256 of the generated source, the runtime source, the embedded
    /// ABI sources, the output of `rustc -vV` and the flags (RS-Q3). A hit invokes the compiler
    /// zero times. A cached library whose bytes fail the manifest hash is removed and rebuilt
    /// once; a second failure is the load error. A compiler error is `Error::CompileFailed`
    /// with the compiler's standard error, and the generated source stays on disk where the
    /// error names it.
    pub fn compile(&self, settings: &Settings) -> Result<Compiled, Error> {
        self.compile_announced(settings, &mut || {})
    }

    /// [`Transpiler::compile`], calling `building` once, before the compiler builds anything,
    /// when the cache does not hold the design: a host announces a compile while it runs.
    pub fn compile_announced(&self, settings: &Settings, building: &mut dyn FnMut()) -> Result<Compiled, Error> {
        let started = Instant::now();
        let source = self.source()?;
        let mut report = Report { transpile: started.elapsed(), ..Report::default() };

        let rustc = Rustc::locate(settings)?;
        let cache = Cache::at(&settings.cache)?;
        let runtime = Runtime::files(self.digital() || self.loops() || self.carried_params() || self.tables() || self.derives());
        let (key, abi_key) = Self::cache_keys(&source, &runtime, rustc.version_text(), AbiSource::FILES);

        // RS-04 AC 5: an entry that is incomplete or fails to open is not what it recorded; it
        // is removed and rebuilt. Only what was there at this look is removed: an entry is placed
        // complete in one rename, so a build that found nothing must not delete the entry a
        // concurrent build of the same design has placed since (RS-04 AC 4).
        let present = cache.rust_dir().join(key.as_str()).exists();
        let hit = cache.entry(&key).and_then(|entry| {
            let at = Instant::now();
            let opened = Loaded::open(&entry);
            report.load += at.elapsed();
            opened.ok().map(|loaded| (entry, loaded))
        });
        let (entry, loaded) = match hit {
            Some(found) => {
                report.cache_hit = true;
                found
            }
            None => {
                if present {
                    cache.remove(&key)?;
                }
                building();
                self.build_and_open(&cache, &key, &abi_key, &rustc, &source, &runtime, &mut report)?
            }
        };
        report.rustc_invocations = rustc.invocations();
        Compiled::bind(self.ir.clone(), &self.plans, entry, loaded, report)
    }

    /// The cache keys of a design: its own, then the ABI library's (RS-Q3). A design's key is the SHA-256 of
    /// the generated `source`, the `runtime` files, the compiler's `version_text`, the flags and the `abi_files`
    /// (the name and text of each file of the ABI crate); the library's own key leaves out the first two. The ABI
    /// contract is edited in place and stays major 1, so its text is in the key: an entry built against an earlier
    /// contract is never found by the current key.
    pub fn cache_keys(source: &str, runtime: &[(&str, &str)], version_text: &str, abi_files: &[(&str, &str)]) -> (Key, Key) {
        let flags = Rustc::FLAGS.join(" ");
        let mut abi_parts: Vec<&[u8]> = vec![version_text.as_bytes(), flags.as_bytes()];
        for (name, text) in abi_files {
            abi_parts.push(name.as_bytes());
            abi_parts.push(text.as_bytes());
        }
        let mut parts: Vec<&[u8]> = vec![source.as_bytes()];
        parts.extend(runtime.iter().map(|(_, text)| text.as_bytes()));
        parts.extend(abi_parts.iter().copied());
        (Key::of(&parts), Key::of(&abi_parts))
    }

    fn build_and_open(
        &self,
        cache: &Cache,
        key: &Key,
        abi_key: &Key,
        rustc: &Rustc,
        source: &str,
        runtime: &[(&str, &str)],
        report: &mut Report,
    ) -> Result<(Entry, Loaded), Error> {
        let at = Instant::now();
        let abi = cache.abi(abi_key, |staging| {
            let src = staging.join("src");
            AbiSource::write_to(&src)?;
            rustc.rlib(&src.join("lib.rs"), &staging.join("libpiperine_abi.rlib")).map(|_| ())
        })?;
        let mut warnings = String::new();
        let entry = cache.build(key, rustc.version_text(), Rustc::FLAGS, |staging| {
            let entry = Entry::at(staging);
            Self::write(&entry.source(), source)?;
            for (path, text) in runtime {
                let file = staging.join(path);
                if let Some(dir) = file.parent() {
                    std::fs::create_dir_all(dir)
                        .map_err(|e| Error::Cache { dir: dir.to_path_buf(), cause: e.to_string() })?;
                }
                Self::write(&file, text)?;
            }
            warnings = rustc.cdylib(&entry.source(), &abi, &entry.library())?;
            Ok(())
        })?;
        report.compile += at.elapsed();
        report.warnings = warnings;
        let at = Instant::now();
        let loaded = Loaded::open(&entry)?;
        report.load += at.elapsed();
        Ok((entry, loaded))
    }

    fn write(path: &Path, text: &str) -> Result<(), Error> {
        std::fs::write(path, text)
            .map_err(|e| Error::Cache { dir: path.parent().unwrap_or(path).to_path_buf(), cause: e.to_string() })
    }
}
