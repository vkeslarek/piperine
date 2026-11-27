//! The transpiler of a checked `Ir`: each body that runs is planned, and the design is printed as Rust and
//! compiled: print, key, look in the cache, build on a miss (the ABI `rlib` once, then the design's `cdylib`),
//! load (design "Architecture Overview"; RS-01 to RS-04, RS-08). Nothing in `compile` runs during an analysis: a
//! design is compiled once, before the first one.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use piperine_ir::{Body, BodyId, Broken, Expr, Ir, ValueType};
use piperine_plan::BodyPlan;

use crate::abi_source::AbiSource;
use crate::cache::{Cache, Entry, Key};
use crate::compiled::{Compiled, Report};
use crate::error::Error;
use crate::load::Loaded;
use crate::print::{self, LibraryPrinter};
use crate::rt::{Runtime, RuntimeForms};
use crate::rustc::Rustc;
use crate::settings::Settings;

/// A checked `Ir`, each body that runs planned.
#[derive(Debug)]
pub struct Transpiler {
    pub(crate) ir: Arc<Ir>,
    /// One plan per body, by body index; `None` for a body with nothing to run.
    pub(crate) plans: Vec<Option<BodyPlan>>,
}

impl Transpiler {
    /// Accepts an `Ir` whose `check()` is empty, by the interpreter's rule; otherwise returns
    /// `Error::Rejected` with the entries. A body runs when it has an `analog` block, a node
    /// with an initial potential or a digital form.
    pub fn new(ir: Arc<Ir>) -> Result<Transpiler, Error> {
        let entries = ir.check();
        if !entries.is_empty() {
            return Err(Error::Rejected(entries));
        }
        let mut plans = Vec::new();
        for body in ir.bodies() {
            let runs = body.analog().is_some() || body.nodes().iter().any(|n| n.init.is_some()) || Transpiler::has_digital_form(body);
            if !runs {
                plans.push(None);
                continue;
            }
            let plan = BodyPlan::build(body).map_err(|refused| {
                Error::Rejected(vec![Broken { item: format!("body {}", body.name()), origin: body.origin().clone(), what: refused.to_string() }])
            })?;
            plans.push(Some(plan));
        }
        Ok(Transpiler { ir, plans })
    }

    /// The design this transpiler prints.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }

    /// The plan of `body`; `None` for a body with nothing to run, or an id out of range.
    pub fn plan(&self, body: BodyId) -> Option<&BodyPlan> {
        self.plans.get(body.index()).and_then(Option::as_ref)
    }

    /// Whether `body` holds a digital form: a carrier, a drive, an event, a reaction or a record
    /// stream. A body with reactions and no carrier (a monitor that only emits) is one.
    fn has_digital_form(body: Body<'_>) -> bool {
        !(body.carriers().is_empty()
            && body.drives().is_empty()
            && body.events().is_empty()
            && body.reactions().is_empty()
            && body.streams().is_empty())
    }

    /// Whether the design holds a digital form in any body, so its runtime is the three files
    /// of `rt` and not `real.rs` alone.
    pub fn digital(&self) -> bool {
        self.ir.bodies().any(Transpiler::has_digital_form)
    }

    /// Whether the design holds a loop or a recursive function, whose printed forms count with
    /// `rt::Trips` and `rt::Depth` (`rt/control.rs`): its runtime is the files of `rt` and not
    /// `real.rs` alone.
    pub fn loops(&self) -> bool {
        self.ir.expressions().any(|e| matches!(e, Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. }))
            || self.ir.functions().any(|f| print::ExpressionPrinter::is_recursive(&self.ir, f.id()))
    }

    /// Whether a param of the design holds a type the pin carries beyond `f64` and `bool`: an
    /// integer, an enum or an `Option` (PI-01 to PI-03). Its generated reads spell `rt::UInt` and
    /// `rt::Carried`, so its runtime is the files of `rt` and not `real.rs` alone.
    pub fn carried_params(&self) -> bool {
        self.ir.bodies().any(|body| body.params().iter().any(|param| !matches!(param.ty, ValueType::F64 | ValueType::Bool)))
    }

    /// Whether the design holds an integer value: a constant, a cast or an operation of `rt::UInt` or `rt::Int`, which
    /// an analog term spells too (`k as f64` of a loop variable), so its runtime is the files of `rt` and not
    /// `real.rs` alone.
    pub fn integers(&self) -> bool {
        self.ir.expression_types().any(|ty| matches!(ty, ValueType::Int(_) | ValueType::UInt(_)))
    }

    /// Whether the design reads a session parameter or writes a trace: a trace's record is written by
    /// `rt::Carried`, and a session parameter's pin holds an `Option<f64>` the design reads as one, so
    /// its runtime is the files of `rt` and not `real.rs` alone.
    pub fn carried_sites(&self) -> bool {
        self.ir.bodies().any(|body| !body.session_parameters().is_empty() || !body.traces().is_empty())
    }

    /// Whether the design derives a param from another: its derive functions read pins through `rt::Carried`.
    pub fn derives(&self) -> bool {
        !piperine_plan::DerivePlan::of(&self.ir).bindings().is_empty()
    }

    /// Whether the design uses a number width (`f32`, `f16`, `f128`, an integer wider than 64 bits), whose
    /// generated code compiles the embedded numeric library (`rt/numbers/*.rs`) and `rt/widths.rs`: `f32` computes
    /// as Rust's own `f32`, but its pin bytes (`Carried`) and its `as` (`Convert`) live in that runtime.
    pub fn number_widths(&self) -> bool {
        crate::print::expression::widths::NumberTypes::used_by(&self.ir)
    }

    /// Whether the design holds a `table_model`, whose generated code builds an `rt::TableModel`.
    pub fn tables(&self) -> bool {
        self.ir.expressions().any(|e| matches!(e, Expr::Table { .. } | Expr::TableSlope { .. }))
    }

    /// The design as one Rust source text (`design.rs`); a form with no lowering is
    /// `Error::Unsupported` and gives no text. The same `Ir` always gives the same bytes.
    pub fn source(&self) -> Result<String, Error> {
        LibraryPrinter::design(&self.ir, &self.plans)
    }

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
        let numbers = self.number_widths();
        let runtime = Runtime::files_for(RuntimeForms { digital: numbers || self.digital() || self.loops() || self.integers() || self.carried_params() || self.carried_sites() || self.tables() || self.derives(), numbers });
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
