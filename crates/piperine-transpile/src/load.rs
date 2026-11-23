//! A library in the process, checked (design "Loaded"; RS-04 AC 5, RS-05). This is the one file
//! of the feature with `unsafe` (RS-05 AC 4): opening a library, calling its one exported symbol
//! and reading what it returns. Each block says which promise of abi.md it leans on.
//!
//! Order of events in [`Loaded::open`], each step before the next:
//!
//! 1. the recorded hash is compared with the library's bytes: a library that fails is never
//!    opened, so none of its code runs;
//! 2. the library is opened and `piperine_abi_1` looked up (abi.md §2 entry 1: the symbol's
//!    suffix is the ABI major);
//! 3. `Library.size` and `Library.minor` are read, and nothing else is, until both are accepted
//!    (abi.md §12 "Versioning");
//! 4. the handle is leaked: abi.md §12 says a library and its prototypes live until the process
//!    ends, and the solver holds `&'static Module`s into it.
//!
//! The hash is checked on a file the cache keeps private (`0700`), so it cannot change between
//! the check and the open except by the user's own hand.

use std::path::{Path, PathBuf};

use piperine_abi::raw::{self, Library, List, Module};

use crate::print::derive::SYMBOL as DERIVE_SYMBOL;

use crate::cache::{Entry, Manifest};
use crate::error::Error;

/// The type of `piperine_abi_1` (abi.md §2 entry 1).
type Export = extern "C" fn() -> *const Library;

/// The type of `piperine_derive_2` (the transpiler's contract with its host, `print/derive.rs`):
/// the function `k` of the design's derive plan over the bytes of the parent's params, answering a 1
/// and the bytes of the child's param, or nothing.
type DeriveExport = for<'a> extern "C" fn(u32, List<'a, u8>) -> List<'static, u8>;

/// A loaded, checked library. Dropping a `Loaded` unloads nothing: its modules stay valid to the
/// end of the process.
#[derive(Debug)]
pub struct Loaded {
    file: PathBuf,
    library: &'static Library,
    /// `piperine_derive_2`, which a design that derives no param does not export.
    derive: Option<DeriveExport>,
}

impl Loaded {
    /// Loads the library of `entry` through `piperine_abi_1`. A library whose bytes do not hash
    /// to what the manifest records, that cannot be opened, that has no such symbol, whose
    /// `size` is smaller than this host reads or whose `minor` is newer than the host's, is an
    /// `Error::Load` naming the file; a library that fails its hash is not opened.
    ///
    /// Every successful open leaks one mapping (abi.md §12): one per distinct design.
    pub fn open(entry: &Entry) -> Result<Loaded, Error> {
        let file = entry.library();
        let fail = |cause: String| Error::Load { file: file.clone(), cause };

        let recorded = entry.manifest()?.library_sha256;
        let bytes = std::fs::read(&file).map_err(|e| fail(format!("cannot be read: {e}")))?;
        let actual = Manifest::hash(&bytes);
        if actual != recorded {
            return Err(fail(format!("its bytes changed: sha256 {actual}, recorded {recorded}")));
        }

        let (export, derive) = Loaded::export(&file).map_err(fail)?;
        let library = export();
        if library.is_null() || !library.is_aligned() {
            return Err(fail(format!("{} returned no library", raw::SYMBOL)));
        }
        // SAFETY: abi.md §2 entry 1 and §12: `piperine_abi_1` returns a pointer to a `Library`
        // that lives until the process ends. Only the first two fields are read, through the raw
        // pointer, as `Library`'s `#[repr(C)]` layout puts `size` and `minor` first (abi.md §3).
        let (size, minor) = unsafe { ((*library).size, (*library).minor) };
        if (size as usize) < size_of::<Library>() {
            return Err(fail(format!("Library.size {size} is smaller than the {} this host reads", size_of::<Library>())));
        }
        if minor > raw::MINOR {
            return Err(fail(format!("Library.minor {minor} is newer than the host's {}", raw::MINOR)));
        }
        // SAFETY: abi.md §12 "The library and its prototypes live until the process ends": the
        // handle was leaked by `export`, so the pointee is valid for `'static`; `size` was just
        // checked to cover every field of this host's `Library`.
        let library: &'static Library = unsafe { &*library };
        if !library.modules.is_readable() {
            return Err(fail("its module list is not readable by this host".to_string()));
        }
        Ok(Loaded { file, library, derive })
    }

    /// Opens `file`, finds `piperine_abi_1` and leaks the handle.
    fn export(file: &Path) -> Result<(Export, Option<DeriveExport>), String> {
        // SAFETY: opening a library runs its initializers; this is the contract of loading any
        // device library (abi.md §12: a library is code the user chose to build, and its hash was
        // just checked against the manifest the cache wrote when it built it).
        let handle = unsafe { libloading::Library::new(file) }.map_err(|e| format!("cannot be opened: {e}"))?;
        // SAFETY: abi.md §2 entry 1 gives `piperine_abi_1` the type `extern "C" fn() -> *const
        // Library`; a library whose symbol has another type breaks the ABI, which the version
        // check above and the cache key (the ABI sources are part of it) are there to prevent.
        let export = unsafe { handle.get::<Export>(raw::SYMBOL.as_bytes()) }
            .map(|symbol| *symbol)
            .map_err(|_| format!("has no symbol `{}`", raw::SYMBOL))?;
        // SAFETY: the export `piperine_derive_2` is printed by this transpiler with the type
        // `DeriveExport` (`print/derive.rs`), beside `piperine_abi_1` of abi.md §2 entry 1 and
        // under the same cache key, the hash of the source that prints both; a library with no
        // such symbol is a design that derives nothing.
        let derive = unsafe { handle.get::<DeriveExport>(DERIVE_SYMBOL.as_bytes()) }.ok().map(|symbol| *symbol);
        // abi.md §12: the library lives until the process ends; the function pointer and every
        // module read through it would dangle if the handle were closed.
        std::mem::forget(handle);
        Ok((export, derive))
    }

    /// The file this library was loaded from.
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Whether the library exports `piperine_derive_2`.
    pub fn derives(&self) -> bool {
        self.derive.is_some()
    }

    /// Derive function `k` over `params`, the pin bytes of the parent's params in declaration order;
    /// the pin bytes of the child's param, or `None` when the library exports no derive function or has
    /// none for this `k` and this width of params.
    pub fn derive(&self, k: u32, params: &[u8]) -> Option<Vec<u8>> {
        let answer = self.derive.map(|derive| derive(k, List::from_slice(params)))?;
        let bytes: Vec<u8> = (0..answer.len()).filter_map(|i| answer.get(i).copied()).collect();
        match bytes.split_first() {
            Some((1, value)) => Some(value.to_vec()),
            _ => None,
        }
    }

    /// The names of the library's modules, in order.
    pub fn names(&self) -> Vec<&'static str> {
        (0..self.library.modules.len()).filter_map(|i| self.library.modules.get(i)).map(|m| m.name().as_str()).collect()
    }

    /// The module called `name`. It lives until the process ends (abi.md §12), so it is
    /// `'static` whatever becomes of this `Loaded`.
    pub fn module(&self, name: &str) -> Option<&'static Module> {
        (0..self.library.modules.len())
            .filter_map(|i| self.library.modules.get(i))
            .find(|m| m.name().as_str() == name)
    }
}
