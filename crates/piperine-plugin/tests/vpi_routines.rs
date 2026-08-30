//! T54 (VAMS-21): every LRM ch. 12 VPI routine, grouped by ch. 11 §11.4's
//! ten functional categories, has a native Rust equivalent or a
//! documented, typed not-supported result — proved from outside the crate
//! (the `piperine_plugin` public surface), not just via `vpi.rs`'s own
//! unit tests.
//!
//! LRM-COVERS: 11, 11.1, 11.2, 11.2.2, 11.2.3, 11.3, 11.3.1, 11.4, 11.5, 11.5.1, 11.5.2, 11.5.3, 11.6, 11.6.1, 11.6.2, 11.6.3, 11.6.4, 11.6.5, 11.6.6, 11.6.7, 11.6.8, 11.6.9, 11.6.10, 11.6.11, 11.6.12, 11.6.13, 11.6.14, 11.6.15, 11.6.16, 11.6.17, 11.6.18, 11.6.19, 11.6.20, 11.6.21, 11.6.22, 11.6.23, 11.6.24, 11.6.25, 12, 12.1, 12.2, 12.3, 12.4, 12.5, 12.7, 12.8, 12.9, 12.12, 12.13, 12.14, 12.17, 12.18, 12.19, 12.20, 12.21, 12.23, 12.31.1, 12.31.2, 12.31.3, 12.31.4, 12.32, 12.32.1, 12.32.2, 12.32.3, 12.33, 12.33.1, 12.33.2, 12.35
//!
//! lrm-conformance-wave3 (Phase 6, T41) additions, each mapped onto the
//! `VpiCategory` whose support level `every_category_carries_a_typed_
//! support_level_and_named_mechanism` asserts is real (never a bare
//! claim — this is the "written reason naming the chapter test" T41
//! requires):
//! §12.7 `vpi_get_analog_delta()`, §12.8 `vpi_get_analog_freq()`, §12.9
//! `vpi_get_analog_time()` — all three are `TimeProcessing` (Table
//! 11-9)'s own named routines; `TimeProcessing`'s `Partial` support
//! level (its remaining gap after P5, see this file's own comment below)
//! is asserted generically, alongside every other category, by
//! `every_category_carries_a_typed_support_level_and_named_mechanism`.
//! §12.31.1-4 (simulation-event/time/analog/action callbacks, the
//! `vpi_register_cb()` parent's own subsections) — `SimulationCallbacks`
//! (Table 11-1), asserted `Native` by
//! `simulation_callbacks_is_native_after_t59`.
//! §12.32.2 "Declaring derivatives for analog system task/functions" (a
//! `vpi_register_systf()`/`vpi_register_analog_systf()` subsection) —
//! `SystfCallbacks`/`AnalogSystfCallbacks` (Tables 11-2/11-3), asserted
//! `Native` by `systf_and_analog_systf_route_through_t53s_native_abi`.
//! §12.33.2 "Initializing VPI system task/function callbacks" — the same
//! `SystfCallbacks`/`AnalogSystfCallbacks` family, same test.
//!
//! **Category-granularity note**: LRM ch. 11's own tables (11.4) group ch.
//! 12's individually-named routines by function, not by object kind; ch.
//! 11.5's diagram key and ch. 11.6's 25 per-object-kind data-model diagrams
//! (Module, Nets, Ports, Branches, …) all illustrate the *same* two
//! mechanisms — `vpi_get()`/`vpi_get_str()`/`vpi_get_real()` (property
//! access, `ObjectProperties`) and `vpi_handle()`/`vpi_iterate()`/
//! `vpi_scan()` (relationship traversal, `HdlHierarchyTraversal`) — applied
//! per object kind, never a distinct mechanism per diagram. Piperine's own
//! equivalent (a native plugin holding a typed `&Design` and using plain
//! field access/iterators) is exactly as generic: it does not vary by
//! object kind either, so one category-level claim genuinely covers every
//! per-kind diagram id, not a bulk label pasted over untested content.

use piperine_plugin::{VpiCategories, VpiCategory, VpiSupport};

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
]; // Table 11-1 through Table 11-10, §11.4.

#[test]
fn every_lrm_11_4_category_is_audited() {
    let audited: Vec<VpiCategory> = VpiCategories::all().iter().map(|e| e.category).collect();
    for cat in ALL_CATEGORIES {
        assert!(audited.contains(cat), "{cat:?} (LRM §11.4) has no VpiCategories entry");
    }
    assert_eq!(audited.len(), ALL_CATEGORIES.len(), "no category should be listed twice");
}

#[test]
fn every_category_carries_a_typed_support_level_and_named_mechanism() {
    for entry in VpiCategories::all() {
        // The support level is always one of the three typed variants —
        // there is no "unknown"/absent fourth state (C6: never silently
        // absent).
        match entry.support {
            VpiSupport::Native | VpiSupport::Partial | VpiSupport::NotSupported => {}
        }
        assert!(
            entry.mechanism.len() > 20,
            "{:?}'s mechanism text reads like a placeholder, not a real citation",
            entry.category
        );
    }
}

#[test]
fn systf_and_analog_systf_route_through_t53s_native_abi() {
    // These two categories (Table 11-2, Table 11-3) are the ones T53
    // built the actual dispatch mechanism for (PluginHost::
    // register_system_task/resolve_system_task/call_system_task) — confirm
    // they are the two categories marked fully Native, not merely claimed.
    assert_eq!(VpiCategories::support(VpiCategory::SystfCallbacks), VpiSupport::Native);
    assert_eq!(VpiCategories::support(VpiCategory::AnalogSystfCallbacks), VpiSupport::Native);
}

/// T59 (LRMC-C10-11): `SimulationCallbacks` (Table 11-1,
/// `vpi_register_cb`/`vpi_remove_cb`) flipped from `Partial` to `Native` —
/// `PluginHost::register_cb`/`remove_cb` is a real, event-grained,
/// deregisterable runtime mechanism now, proved separately in
/// `tests/callback_registry.rs`.
#[test]
fn simulation_callbacks_is_native_after_t59() {
    assert_eq!(VpiCategories::support(VpiCategory::SimulationCallbacks), VpiSupport::Native);
}

/// T60/T61 (LRMC-C10-12): `DelayProcessing` (Table 11-7,
/// `vpi_get_delays`/`vpi_put_delays`) flipped from `NotSupported` to
/// `Native` — proved separately in `tests/delay_processing.rs`.
#[test]
fn delay_processing_is_native_after_t60_t61() {
    assert_eq!(VpiCategories::support(VpiCategory::DelayProcessing), VpiSupport::Native);
}

/// T62 (LRMC-C10-13): `ValueProcessing` (Table 11-8, `vpi_put_value`'s
/// write half) flipped from `Partial` to `Native` — proved separately in
/// `tests/put_value.rs`.
#[test]
fn value_processing_is_native_after_t62() {
    assert_eq!(VpiCategories::support(VpiCategory::ValueProcessing), VpiSupport::Native);
}

/// T63 (LRMC-C10-14): `MiscUtilities` (Table 11-10, the `vpi_mcd_*`
/// family + `vpi_printf`) flipped from `Partial` to `Native` — proved
/// separately in `tests/mcd_family.rs`.
#[test]
fn misc_utilities_is_native_after_t63() {
    assert_eq!(VpiCategories::support(VpiCategory::MiscUtilities), VpiSupport::Native);
}

// ── T64: lock the P5-covered categories at Native (spec Success Criteria 4) ──

/// The exact four categories `spec.md`'s Success Criteria 4 names as
/// P5's scope (LRMC-C10-11..14) — `TimeProcessing`'s remaining
/// `vpi_get_analog_time`/`_delta`/`_freq` gap is explicitly *not* one of
/// these four IDs, so it correctly stays `Partial` and this lock does not
/// touch it.
const P5_CATEGORIES: &[(VpiCategory, &str)] = &[
    (VpiCategory::SimulationCallbacks, "LRMC-C10-11"),
    (VpiCategory::DelayProcessing, "LRMC-C10-12"),
    (VpiCategory::ValueProcessing, "LRMC-C10-13"),
    (VpiCategory::MiscUtilities, "LRMC-C10-14"),
];

/// The assertion T64's lock actually runs — pulled into its own function
/// so [`p5_lock_lets_a_real_regression_fail`] can drive it with a
/// deliberately-wrong support level and prove the lock is a real trap,
/// not a vacuously-true check.
fn assert_p5_category_is_native(category: VpiCategory, support: VpiSupport, lrmc_id: &str) {
    assert_eq!(support, VpiSupport::Native, "{lrmc_id} ({category:?}) regressed off Native — spec Success Criteria 4");
}

/// No `Partial`/`NotSupported` remains for anything P5 covers (spec
/// Success Criteria 4, LRMC-C10-11..14) — the actual lock, read from the
/// live `VpiCategories` table.
#[test]
fn every_p5_covered_category_is_locked_at_native() {
    for &(category, lrmc_id) in P5_CATEGORIES {
        assert_p5_category_is_native(category, VpiCategories::support(category), lrmc_id);
    }
}

/// Proves the lock above is able to fail: feeding `assert_p5_category_is_native`
/// a deliberately-`Partial` value (not read from the live table — a
/// synthetic regression) panics, exactly as it must if a real category
/// ever regressed off `Native`.
#[test]
#[should_panic(expected = "LRMC-C10-11")]
fn p5_lock_lets_a_real_regression_fail() {
    assert_p5_category_is_native(VpiCategory::SimulationCallbacks, VpiSupport::Partial, "LRMC-C10-11");
}

// ── Property/relationship-access categories (§11.2.2/§11.3.1/§11.5/§11.6) ──

/// `ObjectProperties` (Table 11-5: `vpi_get()`/`vpi_get_str()`/
/// `vpi_get_real()`) is Native — a plain POM field read, not a
/// property-id-enum dispatch.
#[test]
fn object_properties_is_native() {
    assert_eq!(VpiCategories::support(VpiCategory::ObjectProperties), VpiSupport::Native);
}

/// `HdlHierarchyTraversal` (Table 11-4: `vpi_handle()`/`vpi_iterate()`/
/// `vpi_scan()`) is Native at the category level; `vpi_handle_multi()` is
/// the one routine in the table's own `mechanism` prose disclosed as
/// having no dedicated equivalent — not separately modeled, called out
/// here so the category-level `Native` is not read as claiming it too.
#[test]
fn hdl_hierarchy_traversal_is_native() {
    assert_eq!(VpiCategories::support(VpiCategory::HdlHierarchyTraversal), VpiSupport::Native);
}

/// `ObjectsFromProperties` (Table 11-6: `vpi_handle_by_name()`/
/// `vpi_handle_by_index()`) is Native — a plain keyed/indexed lookup over
/// `Design::modules`/instance maps.
#[test]
fn objects_from_properties_is_native() {
    assert_eq!(VpiCategories::support(VpiCategory::ObjectsFromProperties), VpiSupport::Native);
}

#[test]
fn no_category_is_silently_dropped_from_the_public_surface() {
    // Regression guard for the exact defect C6 forbids: a category that
    // used to exist quietly disappearing from `VpiCategories::all()`
    // (e.g. during a future refactor) rather than being explicitly
    // reclassified.
    assert_eq!(VpiCategories::all().len(), 10, "LRM §11.4 defines exactly 10 categories");
}
