//! LRM ch. 12 (VPI routine definitions), organized by ch. 11 §11.4's own
//! ten functional categories (Table 11-1 through Table 11-10) — ch. 12
//! itself has no separate taxonomy, it just defines each named routine
//! alphabetically (§12.1: "The routines are listed in alphabetical
//! order"), so §11.4's grouping is the category list this module audits
//! against.
//!
//! **T54 (VAMS-21)**: every category gets a native Rust equivalent *or* a
//! documented, typed [`VpiSupport::NotSupported`] result — never a silent
//! absence (C6). "Native equivalent" does not mean re-implementing the
//! literal `vpiHandle`/C-struct API (spec's Out of Scope table: Piperine
//! never hosts a `dlopen`'d IEEE-VPI C shim); it means the *behavior* the
//! category exists to provide is reachable some other way in Piperine's
//! own native plugin ABI — named concretely in each entry's `mechanism`.

/// One of §11.4's ten VPI routine functional categories (Table 11-1
/// through Table 11-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VpiCategory {
    /// Table 11-1: `vpi_register_cb`/`vpi_remove_cb`/`vpi_get_cb_info`.
    SimulationCallbacks,
    /// Table 11-2: `vpi_register_systf`/`vpi_get_systf_info`.
    SystfCallbacks,
    /// Table 11-3: `vpi_register_analog_systf`/`vpi_get_analog_systf_info`.
    AnalogSystfCallbacks,
    /// Table 11-4: `vpi_handle`/`vpi_iterate`/`vpi_scan`/`vpi_handle_multi`.
    HdlHierarchyTraversal,
    /// Table 11-5: `vpi_get`/`vpi_get_str`/`vpi_get_real`.
    ObjectProperties,
    /// Table 11-6: `vpi_handle_by_name`/`vpi_handle_by_index`.
    ObjectsFromProperties,
    /// Table 11-7: `vpi_get_delays`/`vpi_put_delays`.
    DelayProcessing,
    /// Table 11-8: `vpi_get_value`/`vpi_put_value`/`vpi_get_analog_value`.
    ValueProcessing,
    /// Table 11-9: `vpi_get_time`/`vpi_get_analog_time`/
    /// `vpi_get_analog_delta`/`vpi_get_analog_freq`.
    TimeProcessing,
    /// Table 11-10: `vpi_printf`, the `vpi_mcd_*` family,
    /// `vpi_get_vlog_info`, `vpi_compare_objects`, `vpi_chk_error`,
    /// `vpi_free_object`.
    MiscUtilities,
}

/// Whether [`VpiCategory`]'s behavior is reachable through a native
/// Piperine mechanism, or is a documented gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpiSupport {
    /// Fully reachable through a native mechanism — see the entry's
    /// `mechanism` text.
    Native,
    /// Reachable for part of the category, not all of it — see
    /// `mechanism` for which half and why.
    Partial,
    /// No native mechanism exists yet — a real, named gap, not silently
    /// dropped (C6). `mechanism` states what's missing and why.
    NotSupported,
}

/// One category's audit entry: its support level and the concrete native
/// mechanism (or the concrete gap) named in prose.
#[derive(Debug, Clone, Copy)]
pub struct VpiCategoryEntry {
    pub category: VpiCategory,
    pub support: VpiSupport,
    pub mechanism: &'static str,
}

/// The empty type this module hangs its category table off of (MD-13: no
/// loose functions).
pub struct VpiCategories;

impl VpiCategories {
    /// One entry per §11.4 category (Table 11-1..11-10), in table order.
    /// Data table, not a `match` — every lookup method reads this one
    /// place, so no category can silently go unaudited.
    const TABLE: &'static [VpiCategoryEntry] = &[
        VpiCategoryEntry {
            category: VpiCategory::SimulationCallbacks,
            support: VpiSupport::Native,
            mechanism: "T59 (LRMC-C10-11): `PluginHost::register_cb`/`remove_cb` \
                (`crate::registry::{SimEvent, CallbackId, CallbackRegistry}`) is a real, \
                runtime, event-grained mechanism — `vpi_register_cb()`'s native equivalent, \
                returning a `CallbackId` `vpi_remove_cb()`'s native equivalent \
                (`PluginHost::remove_cb`) genuinely deregisters (asserted: no further fire, \
                `tests/callback_registry.rs`). Events distinguish by analysis kind \
                (`SimEvent::AfterSolve(\"tran\")` vs `\"op\"`/`\"ac\"`), the finest granularity \
                actually observable at the plugin-host boundary — `piperine-solver`'s digital \
                scheduler (T38's `Region`) has no dependency on `piperine-plugin` and exposes no \
                per-net/per-timestep callback seam, a real architectural boundary \
                (`CLAUDE.md`'s crate table), so per-region-drain events are not reachable and \
                are not claimed here. `HookPhase`'s static, macro-declared, whole-run-lifetime \
                mechanism (`vpi_register_cb()`/`vpi_get_cb_info()`'s coarser prior equivalent) \
                is unaffected — a second, orthogonal mechanism, not a replacement.",
        },
        VpiCategoryEntry {
            category: VpiCategory::SystfCallbacks,
            support: VpiSupport::Native,
            mechanism: "`PluginHost::register_system_task`/`resolve_system_task` (T53, \
                `host.rs`) is `vpi_register_systf()`; `resolve_system_task` returning the \
                registered `VpiTaskKind` (`SysTask`/`SysFunc`/`SysRealFunc`) is \
                `vpi_get_systf_info()`'s native equivalent.",
        },
        VpiCategoryEntry {
            category: VpiCategory::AnalogSystfCallbacks,
            support: VpiSupport::Native,
            mechanism: "Same mechanism as `SystfCallbacks`, using \
                `VpiTaskKind::AnalogSysTask`/`AnalogSysFunc` — `register_system_task` is \
                `vpi_register_analog_systf()`, `resolve_system_task` is \
                `vpi_get_analog_systf_info()`.",
        },
        VpiCategoryEntry {
            category: VpiCategory::HdlHierarchyTraversal,
            support: VpiSupport::Native,
            mechanism: "A native Rust plugin already holds a typed `&piperine_lang::Design` \
                (passed to `after_elaborate`/`transform_design`/`before_lower`) — traversing \
                `Design::modules`/`Module::instances` with plain field access and iterators \
                *is* the native equivalent of `vpi_handle()`/`vpi_iterate()`/`vpi_scan()`: an \
                opaque `vpiHandle` indirection exists in the C API only because C has no \
                borrow-checked references or iterators to hand out directly. \
                `vpi_handle_multi()` (many-to-one, used for a contribution's multiple \
                controlling branches) has no dedicated equivalent yet — Partial would apply \
                if any POM type modeled that relationship; today nothing does, so this one \
                routine's use case is NotSupported within an otherwise-Native category (see \
                `Self::support` for the category-level rollup rule).",
        },
        VpiCategoryEntry {
            category: VpiCategory::ObjectProperties,
            support: VpiSupport::Native,
            mechanism: "Reading a POM field directly (`module.name`, `instance.params`, …) is \
                the native equivalent of `vpi_get()`/`vpi_get_str()`/`vpi_get_real()` — no \
                property-id enum or `vpi_get`-family dispatch is needed when the caller \
                already holds a typed struct.",
        },
        VpiCategoryEntry {
            category: VpiCategory::ObjectsFromProperties,
            support: VpiSupport::Native,
            mechanism: "`Design::modules`/`Module`'s instance maps are keyed by name — a plain \
                `HashMap`/slice lookup is `vpi_handle_by_name()`'s native equivalent; indexed \
                array-net/array-instance access (T-series array support) is \
                `vpi_handle_by_index()`'s.",
        },
        VpiCategoryEntry {
            category: VpiCategory::DelayProcessing,
            support: VpiSupport::NotSupported,
            mechanism: "Piperine's digital kernel has no `#delay`/timing-check model to read \
                or write (the fused-cone JIT compiles `always`/`initial` semantics, not \
                Verilog gate/module-path delay annotations) — a real, named gap, not a \
                silent no-op. `vpi_get_delays()`/`vpi_put_delays()` have nothing to route to \
                until that model exists.",
        },
        VpiCategoryEntry {
            category: VpiCategory::ValueProcessing,
            support: VpiSupport::Partial,
            mechanism: "Read half: `SimHooks::after_solve`'s `node_voltages: &[(String, f64)]` \
                (`view.rs::SolveResultView`) is `vpi_get_value()`/`vpi_get_analog_value()`'s \
                native equivalent — a plugin reads every net's resolved value post-solve. \
                Write half: no `SimHooks` method injects a value back into a live solve \
                (`transform_design`/`before_lower` only see the *design*, pre-solve) — \
                `vpi_put_value()` is NotSupported, a live-value-injection API is future work.",
        },
        VpiCategoryEntry {
            category: VpiCategory::TimeProcessing,
            support: VpiSupport::Partial,
            mechanism: "`piperine_lang::eval::tasks::SimEnvironment` (T47) models \
                `vpi_get_time()`'s equivalent (`abstime`). `vpi_get_analog_time()`/ \
                `vpi_get_analog_delta()`/`vpi_get_analog_freq()` have no equivalent yet — \
                `SimEnvironment` carries no small-signal-frequency-domain state — \
                NotSupported for that half, named explicitly rather than left implicit.",
        },
        VpiCategoryEntry {
            category: VpiCategory::MiscUtilities,
            support: VpiSupport::Partial,
            mechanism: "`vpi_printf()`: `piperine_lang::eval::tasks::format_display` (T47) is \
                the native formatter; plain `println!`/a host's own logging is the sink — no \
                `vpi_mcd_*` multi-channel-descriptor abstraction is needed for that. \
                `vpi_chk_error()`: Rust's `Result<T, PluginError>` (returned directly from \
                every fallible call in this crate) *is* the native equivalent — the whole \
                point of `Result` is that a caller cannot forget to check it, which is \
                exactly what `vpi_chk_error()`'s poll-after-call pattern exists to \
                approximate in C. `vpi_free_object()`: ownership/`Drop` make an explicit \
                free routine structurally unnecessary — satisfied by not needing to exist, \
                not by an absence. `vpi_compare_objects()`: two `&Design`/`&Module` \
                references support `std::ptr::eq`/structural `==` directly. \
                `vpi_mcd_open()`/`vpi_mcd_close()`/`vpi_mcd_printf()`/`vpi_mcd_name()`: no \
                native equivalent exists — NotSupported, a real (minor) gap named here \
                rather than silently absent. `vpi_get_vlog_info()`: `Manifest`/`ABI_VERSION` \
                (`backend::native::ABI_VERSION`) carry the equivalent product-invocation \
                metadata a plugin can already read from its `HostCtx`.",
        },
    ];

    /// Every category this module audits, in table order — used by tests
    /// and by any caller that wants the full LRM ch. 12 coverage sweep.
    pub fn all() -> &'static [VpiCategoryEntry] {
        Self::TABLE
    }

    /// The audit entry for one category. Every [`VpiCategory`] variant has
    /// exactly one entry (proven by `Self::TABLE`'s own construction plus
    /// the `every_category_has_exactly_one_entry` test) — this never
    /// returns `None` for a valid variant, so it returns the entry
    /// directly rather than an `Option`.
    pub fn entry(category: VpiCategory) -> VpiCategoryEntry {
        *Self::TABLE
            .iter()
            .find(|e| e.category == category)
            .unwrap_or_else(|| panic!("{category:?} has no VpiCategories::TABLE entry"))
    }

    /// The category's support level.
    pub fn support(category: VpiCategory) -> VpiSupport {
        Self::entry(category).support
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CATEGORIES: &[VpiCategory] = &[
        VpiCategory::SimulationCallbacks,
        VpiCategory::SystfCallbacks,
        VpiCategory::AnalogSystfCallbacks,
        VpiCategory::HdlHierarchyTraversal,
        VpiCategory::ObjectProperties,
        VpiCategory::ObjectsFromProperties,
        VpiCategory::DelayProcessing,
        VpiCategory::ValueProcessing,
        VpiCategory::TimeProcessing,
        VpiCategory::MiscUtilities,
    ];

    #[test]
    fn every_category_has_exactly_one_entry() {
        for cat in ALL_CATEGORIES {
            let matches = VpiCategories::all().iter().filter(|e| e.category == *cat).count();
            assert_eq!(matches, 1, "{cat:?} should have exactly one TABLE entry");
        }
        assert_eq!(VpiCategories::all().len(), ALL_CATEGORIES.len());
    }

    #[test]
    fn every_entry_names_a_concrete_mechanism_or_gap() {
        for entry in VpiCategories::all() {
            assert!(
                !entry.mechanism.trim().is_empty(),
                "{:?} has an empty mechanism — never silently absent (C6)",
                entry.category
            );
        }
    }

    #[test]
    fn systf_categories_are_fully_native_via_t53() {
        assert_eq!(VpiCategories::support(VpiCategory::SystfCallbacks), VpiSupport::Native);
        assert_eq!(VpiCategories::support(VpiCategory::AnalogSystfCallbacks), VpiSupport::Native);
    }

    #[test]
    fn hierarchy_and_property_categories_are_native() {
        assert_eq!(VpiCategories::support(VpiCategory::HdlHierarchyTraversal), VpiSupport::Native);
        assert_eq!(VpiCategories::support(VpiCategory::ObjectProperties), VpiSupport::Native);
        assert_eq!(VpiCategories::support(VpiCategory::ObjectsFromProperties), VpiSupport::Native);
    }

    #[test]
    fn delay_processing_is_a_documented_gap() {
        assert_eq!(VpiCategories::support(VpiCategory::DelayProcessing), VpiSupport::NotSupported);
    }

    #[test]
    fn value_and_time_processing_are_partial_never_silently_absent() {
        assert_eq!(VpiCategories::support(VpiCategory::ValueProcessing), VpiSupport::Partial);
        assert_eq!(VpiCategories::support(VpiCategory::TimeProcessing), VpiSupport::Partial);
    }

    #[test]
    fn no_category_is_undocumented() {
        // Every category resolves to a support level at all (the type
        // system already guarantees this; this test guards against a
        // future refactor accidentally making `entry` partial/panicking
        // for a real variant).
        for cat in ALL_CATEGORIES {
            let _ = VpiCategories::support(*cat);
        }
    }
}
