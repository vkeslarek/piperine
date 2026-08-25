//! The lrm-conformance-closure conformance registry (MD-31 policy-invariant
//! gate, design.md's "Cross-cutting: `lrm_conformance_matrix.rs`"): makes
//! "N LRMC findings closed" mechanically checkable instead of a claim in a
//! doc. Same registry+exhaustiveness shape as
//! `crates/piperine-solver/tests/capabilities_contract.rs`/
//! `spec_failure_rules_guard.rs` (MD-31).
//!
//! **Two independent checks, both required:**
//! 1. This file's hardcoded [`REGISTRY`] — every LRMC-ID from
//!    `.specs/features/lrm-conformance-closure/spec.md`'s own Requirement
//!    Traceability table, each carrying its LRM clause, story, and a
//!    [`Status`] (`OutOfScope`, or `Closed` naming the real test
//!    that closes it — T67 flipped the last `Pending` entry, so the
//!    variant itself is gone: this registry stays a permanent lock, never
//!    reverts to tracking in-progress work) — is cross-checked against the
//!    *live* table text
//!    (`include_str!`, so a spec.md edit that adds/removes/renames an ID is
//!    caught, never silently drifting) for an exact ID-set match: no
//!    registry entry without a matching spec row, no spec row without a
//!    matching registry entry, no duplicate ID on either side.
//! 2. Every `Closed` entry's named test must actually exist — the file it
//!    names is read from disk and must contain a `#[test] fn <name>` (or
//!    `fn <name>` immediately preceded by `#[test]` on the line above) that
//!    is not `#[ignore]`d — a stale reference to a renamed/deleted test
//!    fails this gate, not just a doc claim.
//!
//! **Count note (disclosed, not silently reconciled):** spec.md's own
//! Coverage paragraph states "51 requirement IDs" / "51 mapped to tasks",
//! but its Requirement Traceability table, counted literally
//! (`grep -c '^| LRMC-'`), has 54 rows (52 tracked toward closure + 2
//! `Out of Scope`: LRMC-C10-15, LRMC-C10-17). This gate does not assert a
//! magic count — it asserts *set equality* against the table's actual
//! content, which is authoritative over the prose count per this task's own
//! instruction ("Source of the 51 IDs: spec.md's Requirement Traceability
//! table"). The discrepancy is spec.md's own prose vs. its own table, not
//! introduced by this gate.
//!
//! **Guard proven able to fail:** during T9's implementation, a registry
//! entry's `test_fn` was temporarily renamed to a nonexistent name and the
//! `every_closed_entry_names_a_real_test` check failed as expected, then
//! reverted — recorded in this task's commit body per the task's own
//! "Done when" requirement (not left as a comment here, since the mutation
//! itself is not part of the shipped gate).

use std::fs;
use std::path::PathBuf;

/// The status of one LRMC-ID's closure.
enum Status {
    /// Explicitly excluded from closure per spec.md's own Out of Scope
    /// table — never expected to gain a `Closed` status.
    OutOfScope,
    /// Closed by the named test — `file` is a path relative to the
    /// workspace root, `test_fn` is the `#[test] fn` name inside it.
    Closed { file: &'static str, test_fn: &'static str },
}

struct Entry {
    id: &'static str,
    story: &'static str,
    clause: &'static str,
    status: Status,
}

/// The full registry — one entry per LRMC-ID in spec.md's Requirement
/// Traceability table, in the table's own order.
const REGISTRY: &[Entry] = &[
    Entry { id: "LRMC-C1-01", story: "P1", clause: "§2.6.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_ch1_lexical_form.rs", test_fn: "illegal_digit_for_declared_base_is_a_typed_lex_error" } },
    Entry { id: "LRMC-C1-02", story: "P1", clause: "§2.6.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_four_state_literals.rs", test_fn: "sixteen_bit_hex_x_evaluates_to_a_four_state_logic_value_not_zero" } },
    Entry { id: "LRMC-C1-03", story: "P1", clause: "§2.6.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_ch1_lexical_form.rs", test_fn: "whitespace_after_base_letter_is_accepted" } },
    Entry { id: "LRMC-C1-04", story: "P1", clause: "§2.6.1/§2.6.2", status: Status::Closed { file: "crates/piperine-lang/tests/vams_ch1_lexical_form.rs", test_fn: "underscore_immediately_after_decimal_point_is_rejected" } },
    Entry { id: "LRMC-C1-05", story: "P1", clause: "§2.9/§2.9.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_ch1_lexical_form.rs", test_fn: "attribute_instance_after_binary_operator_parses_from_lrm_example_6" } },
    Entry { id: "LRMC-C1-06", story: "P1", clause: "§3.4.2/3.4.4/3.4.6/3.4.8/4.2.14", status: Status::Closed { file: "crates/piperine-lang/tests/vams_assignment_pattern_elab.rs", test_fn: "param_array_assignment_pattern_elaborates_from_lrm_3_4_4" } },
    Entry { id: "LRMC-C1-07", story: "P1", clause: "§3.4.7", status: Status::Closed { file: "crates/piperine-lang/tests/vams_param_ranges.rs", test_fn: "aliasparam_over_an_integer_parameter_carries_integer_type" } },
    Entry { id: "LRMC-C1-08", story: "P1", clause: "§4.4/§3.6.1.2", status: Status::Closed { file: "crates/piperine-lang/tests/expressions.rs", test_fn: "a_custom_discipline_access_name_parses_as_a_plain_call" } },
    Entry { id: "LRMC-C1-09", story: "P1", clause: "§4.4/§5.4.2/§5.4.3", status: Status::Closed { file: "crates/piperine-lang/tests/vams_lower_analog.rs", test_fn: "custom_discipline_accessor_resolves_potential_via_named_branch" } },
    Entry { id: "LRMC-C1-10", story: "P1", clause: "§4.4/§5.4.3", status: Status::Closed { file: "crates/piperine-lang/tests/vams_lower_analog.rs", test_fn: "port_access_parses_and_elaborates_from_lrm_5_4_3_diode_example" } },
    Entry { id: "LRMC-C1-11", story: "P1", clause: "§4.2.1.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_real_to_int_rounding.rs", test_fn: "positive_fractional_bound_rounds_up_35_7_to_36" } },
    Entry { id: "LRMC-C1-12", story: "P1", clause: "§4.5.11.1-4", status: Status::Closed { file: "crates/piperine-codegen/tests/vams_laplace_registry.rs", test_fn: "fabricated_laplace_pm_fails_loud" } },
    Entry { id: "LRMC-C5-01", story: "P2", clause: "§5.6", status: Status::Closed { file: "crates/piperine-codegen/tests/vams_laplace_registry.rs", test_fn: "laplace_nd_resolves" } },
    Entry { id: "LRMC-C5-02", story: "P2", clause: "§5.10.3.1", status: Status::Closed { file: "tests/vams_cross_direction_e2e.rs", test_fn: "rising_direction_fires_on_a_rising_crossing" } },
    Entry { id: "LRMC-C5-03", story: "P2", clause: "§5.10.3.4", status: Status::Closed { file: "tests/vams_absdelta_e2e.rs", test_fn: "absdelta_event_re_evaluates_at_the_lrm_specified_points" } },
    Entry { id: "LRMC-C5-04", story: "P2", clause: "§5.10.4", status: Status::Closed { file: "tests/vams_named_events_e2e.rs", test_fn: "digital_trigger_wakes_an_analog_named_event_sensitivity_in_order" } },
    Entry { id: "LRMC-C5-05", story: "P2", clause: "§5.10.5", status: Status::Closed { file: "tests/vams_digital_events_in_analog_e2e.rs", test_fn: "analog_block_sensitive_to_a_digital_posedge_elaborates_and_simulates" } },
    Entry { id: "LRMC-C5-06", story: "P2", clause: "§6.3.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_defparam.rs", test_fn: "defparam_changes_the_named_instance_effective_parameter_value" } },
    Entry { id: "LRMC-C5-07", story: "P2", clause: "§6.4", status: Status::Closed { file: "crates/piperine-lang/tests/vams_paramset.rs", test_fn: "paramset_instance_resolves_to_the_associated_module_with_bound_params" } },
    Entry { id: "LRMC-C5-08", story: "P2", clause: "§6.2.2", status: Status::Closed { file: "crates/piperine-lang/tests/vams_array_of_instances.rs", test_fn: "array_of_instances_parses_and_elaborates_to_the_specified_range" } },
    Entry {
        id: "LRMC-C7-01",
        story: "P3",
        clause: "§7.4.4.1/§7.4.4.3/§7.5-7.8",
        status: Status::Closed {
            file: "tests/vams_connect_modules_e2e.rs",
            test_fn: "auto_inserted_connect_module_bridges_a_genuine_discipline_crossing_end_to_end",
        },
    },
    Entry {
        id: "LRMC-C7-02",
        story: "P3",
        clause: "§7.7.1",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/items_remaining.rs",
            test_fn: "connect_insertion_full_form_with_mode_params_and_overrides_parses",
        },
    },
    Entry {
        id: "LRMC-C7-03",
        story: "P3",
        clause: "§7.7.3/§7.7.4",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/vams_connect_modules.rs",
            test_fn: "split_segments_one_connect_module_instance_per_port",
        },
    },
    Entry {
        id: "LRMC-C7-04",
        story: "P3",
        clause: "§7.4.4.3",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/vams_discipline_resolution.rs",
            test_fn: "resolveto_forces_the_named_discipline_instead_of_failing_loud",
        },
    },
    Entry {
        id: "LRMC-C7-05",
        story: "P3",
        clause: "§7.6/§9.22-9.23",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/vams_system_tasks.rs",
            test_fn: "driver_access_calls_parse_and_elaborate_inside_a_connectmodule_body",
        },
    },
    Entry {
        id: "LRMC-C7-06",
        story: "P3",
        clause: "§7.3.6",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/vams_cross_domain_concurrency.rs",
            test_fn: "analog_event_sampled_variable_drives_a_digital_event_control",
        },
    },
    Entry {
        id: "LRMC-C7-07",
        story: "P3",
        clause: "§9.2/Table 9-1",
        status: Status::Closed {
            file: "tests/vams_session_output.rs",
            test_fn: "session_builder_output_captures_display_through_a_real_session",
        },
    },
    Entry {
        id: "LRMC-C7-08",
        story: "P3",
        clause: "§9.2/§9.4.2/§9.4.4",
        status: Status::Closed {
            file: "crates/piperine-lang/src/eval/tasks.rs",
            test_fn: "display_format_b_and_h_render_unknown_and_highz_bits",
        },
    },
    Entry {
        id: "LRMC-C7-09",
        story: "P3",
        clause: "§9.2-9.23",
        status: Status::Closed {
            file: "crates/piperine-lang/src/eval/rng.rs",
            test_fn: "random_is_deterministic_given_the_same_seed",
        },
    },
    Entry {
        id: "LRMC-C7-10",
        story: "P3",
        clause: "§9.2",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/system_task_migration_completeness.rs",
            test_fn: "undeclared_diagnostic_name_still_fails_loud_at_compile_time",
        },
    },
    Entry {
        id: "LRMC-C7-11",
        story: "P3",
        clause: "§9.17.1",
        status: Status::Closed {
            file: "crates/piperine-codegen/tests/vams_discontinuity.rs",
            test_fn: "discontinuity_prevents_step_control_mis_stepping_across_the_jump",
        },
    },
    Entry {
        id: "LRMC-C7-12",
        story: "P3",
        clause: "§8.3",
        status: Status::Closed {
            file: "crates/piperine-solver/src/digital/events.rs",
            test_fn: "stratified_regions_resolve_the_canonical_active_nba_race_through_evaluate_until_stable",
        },
    },
    Entry {
        id: "LRMC-C7-13",
        story: "P3",
        clause: "§8.5.3.5",
        status: Status::Closed {
            file: "tests/vams_gates_and_udps_e2e.rs",
            test_fn: "gate_primitives_simulate_end_to_end_and_produce_lrm_truth_table_values",
        },
    },
    Entry {
        id: "LRMC-C7-14",
        story: "P3",
        clause: "§8.4.6",
        status: Status::Closed {
            file: "crates/piperine-solver/src/digital/interface.rs",
            test_fn: "a2d_crossing_interpolates_within_the_published_error_bound",
        },
    },
    Entry { id: "LRMC-C10-01", story: "P4", clause: "§10.2", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "default_discipline_sets_the_named_discipline" } },
    Entry { id: "LRMC-C10-02", story: "P4", clause: "§10.3", status: Status::Closed { file: "crates/piperine-codegen/tests/default_transition_semantics.rs", test_fn: "transition_without_explicit_times_uses_the_directive_value" } },
    Entry { id: "LRMC-C10-03", story: "P4", clause: "§10.5", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "vams_enable_ifdef_takes_the_ams_branch" } },
    Entry { id: "LRMC-C10-04", story: "P4", clause: "§10.6", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "section_10_6_own_example_parses_and_records_the_version" } },
    Entry { id: "LRMC-C10-05", story: "P4", clause: "ch.10 table (celldefine/endcelldefine)", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "celldefine_endcelldefine_records_the_enclosed_span" } },
    Entry { id: "LRMC-C10-06", story: "P4", clause: "ch.10 table (line)", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "line_directive_remaps_reported_line_numbers" } },
    Entry { id: "LRMC-C10-07", story: "P4", clause: "ch.10 table (pragma)", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "all_1364_inherited_directives_no_longer_raise_undefined_macro" } },
    Entry { id: "LRMC-C10-08", story: "P4", clause: "ch.10 table (default_nettype)", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "default_nettype_none_makes_an_undeclared_net_an_error" } },
    Entry { id: "LRMC-C10-09", story: "P4", clause: "ch.10 table (unconnected_drive)", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "unconnected_drive_sets_pull0_or_pull1" } },
    Entry { id: "LRMC-C10-10", story: "P4", clause: "ch.10 table (timescale/resetall)", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "timescale_1ns_1ps_records_the_expected_exponents" } },
    Entry { id: "LRMC-C10-11", story: "P4→P5", clause: "§11.2.1/Table 11-1", status: Status::Closed { file: "crates/piperine-plugin/tests/callback_registry.rs", test_fn: "register_cb_fires_at_exactly_the_requested_event" } },
    Entry { id: "LRMC-C10-12", story: "P4→P5", clause: "§11.4 Table 11-7", status: Status::Closed { file: "crates/piperine-plugin/tests/delay_processing.rs", test_fn: "put_delays_changes_observed_event_timing_in_a_live_run" } },
    Entry { id: "LRMC-C10-13", story: "P4→P5", clause: "§11.4 Table 11-8", status: Status::Closed { file: "crates/piperine-plugin/tests/put_value.rs", test_fn: "put_value_changes_the_subsequent_solve_numerically" } },
    Entry { id: "LRMC-C10-14", story: "P4→P5", clause: "§11.4 Table 11-10", status: Status::Closed { file: "crates/piperine-plugin/tests/mcd_family.rs", test_fn: "mcd_printf_writes_to_every_selected_channel" } },
    Entry { id: "LRMC-C10-15", story: "Out of Scope", clause: "Annex C.4", status: Status::OutOfScope },
    Entry { id: "LRMC-C10-16", story: "P4", clause: "Annex D.3", status: Status::Closed { file: "crates/piperine-lang/tests/vams_headers.rs", test_fn: "driver_access_vams_bit_values_match_the_lrm_exactly" } },
    Entry { id: "LRMC-C10-17", story: "Out of Scope", clause: "Annex E.2.1/E.3.2", status: Status::OutOfScope },
    Entry { id: "LRMC-C10-18", story: "P4", clause: "Annex F §7.4.4.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_annex_f_domain.rs", test_fn: "continuous_wins_over_discrete_matching_figure_7_3" } },
    Entry { id: "LRMC-C10-19", story: "P4", clause: "Annex F §7.4.4.1 step 4.b", status: Status::Closed { file: "crates/piperine-lang/tests/vams_annex_f_domain.rs", test_fn: "unresolved_net_falls_back_to_default_discipline_directive" } },
    Entry {
        id: "LRMC-C10-20",
        story: "P4",
        clause: "Annex F §7.4.1",
        status: Status::Closed {
            file: "crates/piperine-lang/tests/vams_discipline_resolution.rs",
            test_fn: "narrowest_resolveto_rule_wins_over_a_broader_one",
        },
    },
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const SPEC: &str = include_str!("../.specs/features/lrm-conformance-closure/spec.md");

/// Every `LRMC-*` id appearing as the first column of a Requirement
/// Traceability table row in spec.md, parsed directly from the live file.
fn spec_table_ids() -> Vec<String> {
    SPEC.lines()
        .filter(|l| l.trim_start().starts_with("| LRMC-"))
        .map(|l| {
            let id = l.trim_start().trim_start_matches('|').split('|').next().unwrap().trim();
            id.to_string()
        })
        .collect()
}

#[test]
fn registry_has_no_duplicate_ids() {
    let mut seen = std::collections::HashSet::new();
    for e in REGISTRY {
        assert!(seen.insert(e.id), "duplicate LRMC-ID in REGISTRY: {}", e.id);
    }
}

#[test]
fn registry_exactly_matches_spec_table_id_set() {
    let spec_ids = spec_table_ids();
    assert!(spec_ids.len() >= 50, "parsed only {} spec.md table rows — the table's shape changed and this guard went blind", spec_ids.len());

    let mut spec_seen = std::collections::HashSet::new();
    for id in &spec_ids {
        assert!(spec_seen.insert(id.clone()), "spec.md's own table has a duplicate row: {id}");
    }

    let registry_ids: std::collections::HashSet<&str> = REGISTRY.iter().map(|e| e.id).collect();

    let missing_from_registry: Vec<&String> =
        spec_ids.iter().filter(|id| !registry_ids.contains(id.as_str())).collect();
    assert!(
        missing_from_registry.is_empty(),
        "spec.md table rows with no REGISTRY entry: {missing_from_registry:?}"
    );

    let orphaned_in_registry: Vec<&str> =
        registry_ids.iter().filter(|id| !spec_seen.contains(&id.to_string())).copied().collect();
    assert!(
        orphaned_in_registry.is_empty(),
        "REGISTRY entries with no matching spec.md table row: {orphaned_in_registry:?}"
    );
}

#[test]
fn out_of_scope_entries_match_spec_out_of_scope_findings() {
    // LRMC-C10-15/17 are the only two findings spec.md's own Out of Scope
    // framing excludes from closure (spec.md's Coverage note names them by
    // id). Any other entry marked OutOfScope, or either of these two NOT
    // marked OutOfScope, is a registry/spec drift.
    for e in REGISTRY {
        let should_be_out_of_scope = e.id == "LRMC-C10-15" || e.id == "LRMC-C10-17";
        let is_out_of_scope = matches!(e.status, Status::OutOfScope);
        assert_eq!(
            is_out_of_scope, should_be_out_of_scope,
            "{}: OutOfScope status mismatch (is={is_out_of_scope}, should_be={should_be_out_of_scope})",
            e.id
        );
    }
}

/// Every `Closed` entry's named test must actually exist — a stale
/// reference to a renamed/deleted test fails this gate, not just a doc
/// claim (T9's own "Done when": "test fails if an ID is missing,
/// duplicated, or names a non-existent test").
#[test]
fn every_closed_entry_names_a_real_test() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for e in REGISTRY {
        let Status::Closed { file, test_fn } = &e.status else { continue };
        let path = root.join(file);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{}: file `{file}` does not exist", e.id));
            continue;
        };
        if !test_function_exists(&contents, test_fn) {
            failures.push(format!("{}: no `#[test] fn {test_fn}` found in `{file}`", e.id));
        }
    }
    assert!(failures.is_empty(), "closing-test references that don't resolve:\n  {}", failures.join("\n  "));
}

/// True if `contents` has a `#[test]`-attributed `fn <name>` — the
/// attribute may be on the line immediately above the `fn`, or (less
/// commonly in this codebase) share a line via `#[test] fn`. Does not
/// match an `#[ignore]`d test — an ignored test is not a real closing
/// proof (suite_hygiene's own rule against switched-off coverage applies
/// here too).
fn test_function_exists(contents: &str, name: &str) -> bool {
    let needle_owned = format!("fn {name}(");
    let needle = needle_owned.as_str();
    let lines: Vec<&str> = contents.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains(needle) {
            continue;
        }
        // Same-line `#[test] fn name(...)`.
        if line.contains("#[test]") {
            return true;
        }
        // Look back over attribute lines immediately above for `#[test]`,
        // stopping at the first non-attribute line; reject if `#[ignore]`
        // appears in that same attribute block.
        let mut saw_test = false;
        let mut saw_ignore = false;
        let mut j = i;
        while j > 0 {
            j -= 1;
            let prev = lines[j].trim();
            if prev.starts_with("#[test]") {
                saw_test = true;
            } else if prev.starts_with("#[ignore") {
                saw_ignore = true;
            } else if prev.starts_with('#') || prev.starts_with("///") || prev.is_empty() {
                // keep scanning past other attributes/doc comments/blank lines
            } else {
                break;
            }
        }
        if saw_test && !saw_ignore {
            return true;
        }
    }
    false
}

#[test]
fn test_function_exists_helper_distinguishes_real_ignored_and_absent() {
    let src = "#[test]\nfn real_one() {}\n\n#[test]\n#[ignore]\nfn ignored_one() {}\n\nfn not_a_test() {}\n";
    assert!(test_function_exists(src, "real_one"));
    assert!(!test_function_exists(src, "ignored_one"), "an #[ignore]d test must not count as a real closing proof");
    assert!(!test_function_exists(src, "not_a_test"), "a fn with no #[test] attribute must not count");
    assert!(!test_function_exists(src, "totally_absent"));
}

#[test]
fn every_id_has_a_non_empty_clause_and_story() {
    for e in REGISTRY {
        assert!(!e.clause.is_empty(), "{}: empty LRM clause", e.id);
        assert!(!e.story.is_empty(), "{}: empty story", e.id);
    }
}

// ---------------------------------------------------------------------------
// Wave 2 (`lrm-conformance-wave2`, T71) — a second, independent registry.
//
// This section is append-only relative to everything above it: the wave-1
// `REGISTRY`, `SPEC`, `Entry`/`Status`, `spec_table_ids()`,
// `test_function_exists()` and all five wave-1 tests are untouched. Wave-2
// tracks its own `LRMW2-NN` ids from
// `.specs/features/lrm-conformance-wave2/spec.md`'s own Requirement
// Traceability table ("User Stories & Requirements" section) via a
// parallel `WAVE2_REGISTRY`/`WAVE2_SPEC`/`wave2_spec_table_ids()` — the same
// two-independent-checks shape as wave-1, kept as a second instance rather
// than generalizing the mechanism (a generalization here would touch the
// wave-1 code above, which this task must not do).
//
// spec.md's Out of Scope table (checked at authoring time) excludes LRM
// *sections* from `lrm_section_coverage.rs`'s coverage gate (feeding
// LRMW2-65), not any `LRMW2-NN` requirement id itself — no wave-2 row is
// framed as excluded from closure the way wave-1's LRMC-C10-15/17 were, so
// there is no `wave2_out_of_scope_...` test mirroring wave-1's fourth test;
// all 66 rows are `Status::Closed`.
//
// Two ids have no real closing test and are deliberately left `Pending`
// rather than fabricated:
//   - LRMW2-01 ("capture a pre-feature baseline") names
//     `.specs/features/lrm-conformance-wave2/baseline.md`, a measurement
//     document with no assertions of its own (T1's own "Tests: none
//     (measurement task)").
//   - LRMW2-66 ("SPEC_DEVIATION.md regenerated against HEAD") names a doc
//     file, also with no assertions of its own (T70's own "Tests: none
//     (doc)").
// `Status::Pending` exists in this registry (unlike wave-1's, where the
// variant was removed once the last pending entry closed) purely to carry
// these two disclosed gaps without inventing a test; every `Pending` entry
// is asserted to be one of exactly these two ids by
// `wave2_pending_entries_are_only_the_disclosed_doc_ids`.

/// The status of one LRMW2-ID's closure — mirrors wave-1's [`Status`]
/// (same two variant shapes) plus `Pending` for the two ids with no
/// assertable closing test (see the section note above).
enum Wave2Status {
    /// Closed by the named test — `file` is a path relative to the
    /// workspace root; `test_fn` is the `#[test] fn` name inside it, or
    /// (for LRMW2-51/55 which close via an inline `#[cfg(test)]` module in
    /// a `src/` file rather than an integration test target) an in-crate
    /// unit test found the same way.
    Closed { file: &'static str, test_fn: &'static str },
    /// No real test exists to name — a disclosed gap, not a fabricated
    /// closing proof. See the section note above for why LRMW2-01/66 are
    /// the only two.
    Pending { reason: &'static str },
}

struct Wave2Entry {
    id: &'static str,
    story: &'static str,
    clause: &'static str,
    status: Wave2Status,
}

/// One entry per `LRMW2-NN` id in wave-2 spec.md's Requirement Traceability
/// table, in the table's own order. Each `Closed` entry's test was verified
/// to exist on disk (not guessed) by reading the task in
/// `.specs/features/lrm-conformance-wave2/tasks.md` that carries the
/// matching `**Requirement**: LRMW2-NN` line, then confirming the named
/// `#[test] fn` is real at HEAD.
const WAVE2_REGISTRY: &[Wave2Entry] = &[
    Wave2Entry { id: "LRMW2-01", story: "P0", clause: "—", status: Wave2Status::Pending { reason: "baseline.md is a measurement document (T1: \"Tests: none (measurement task)\") with no assertion to name" } },
    Wave2Entry { id: "LRMW2-02", story: "P0", clause: "whole LRM", status: Wave2Status::Closed { file: "tests/lrm_section_coverage.rs", test_fn: "covered_and_excluded_partition_the_lrm" } },
    Wave2Entry { id: "LRMW2-03", story: "P0", clause: "—", status: Wave2Status::Closed { file: "tests/lrm_conformance_matrix.rs", test_fn: "wave2_registry_exactly_matches_spec_table_id_set" } },
    Wave2Entry { id: "LRMW2-04", story: "P1", clause: "§6.9.1", status: Wave2Status::Closed { file: "tests/vams_analog_block_concat_e2e.rs", test_fn: "two_ddt_blocks_keep_independent_integrator_state" } },
    Wave2Entry { id: "LRMW2-05", story: "P1", clause: "§7.3.5", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/digital_analog_event_sensitivity.rs", test_fn: "crossing_sensitive_digital_block_fires_once_per_crossing_not_every_step" } },
    Wave2Entry { id: "LRMW2-06", story: "P1", clause: "§6.5.3, §7.3.6.4, §8.5.3.1", status: Wave2Status::Closed { file: "tests/vams_continuous_assign_e2e.rs", test_fn: "wreal_target_is_driven_by_assign_end_to_end" } },
    Wave2Entry { id: "LRMW2-07", story: "P1", clause: "§3.6.4", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_ground.rs", test_fn: "bare_ground_of_an_already_declared_net_registers_it" } },
    Wave2Entry { id: "LRMW2-08", story: "P1", clause: "§5.10.2", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/final_step.rs", test_fn: "final_step_fires_exactly_once_per_transient_analysis" } },
    Wave2Entry { id: "LRMW2-09", story: "P1", clause: "§9.2", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/unknown_system_task.rs", test_fn: "unknown_task_in_analog_body_errors_naming_task_and_clause" } },
    Wave2Entry { id: "LRMW2-10", story: "P1", clause: "§3.6.3.2", status: Wave2Status::Closed { file: "tests/net_declaration_nodeset.rs", test_fn: "declared_nodeset_positive_selects_the_positive_branch" } },
    Wave2Entry { id: "LRMW2-11", story: "P1", clause: "§3.4.3, §3.2.1", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_param_var_instance_attrs.rs", test_fn: "parameter_attribute_survives_into_the_pom" } },
    Wave2Entry { id: "LRMW2-12", story: "P1", clause: "§5.10.3", status: Wave2Status::Closed { file: "tests/vams_level_event_expr_e2e.rs", test_fn: "level_event_on_a_comparison_expression_fires_on_change" } },
    Wave2Entry { id: "LRMW2-13", story: "P1", clause: "§3.5", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_genvar_outside_generate.rs", test_fn: "declared_genvar_is_visible_in_the_pom" } },
    Wave2Entry { id: "LRMW2-14", story: "P2", clause: "§8.5.3.3/4", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/digital_jit.rs", test_fn: "nonblocking_swap_in_a_clocked_block_exchanges_values" } },
    Wave2Entry { id: "LRMW2-15", story: "P2", clause: "Table 4-14", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/log_base10.rs", test_fn: "log_call_produces_base_10_residual" } },
    Wave2Entry { id: "LRMW2-16", story: "P2", clause: "§4.5.13", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/limexp_convergence.rs", test_fn: "limexp_keeps_undamped_newton_finite_where_plain_exp_overflows" } },
    Wave2Entry { id: "LRMW2-17", story: "P2", clause: "§4.4, §5.5.1", status: Wave2Status::Closed { file: "tests/generic_access_functions.rs", test_fn: "potential_access_function_produces_a_voltage_source" } },
    Wave2Entry { id: "LRMW2-18", story: "P2", clause: "§4.2.6", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/case_equality.rs", test_fn: "case_equality_treats_x_as_an_ordinary_comparable_bit" } },
    Wave2Entry { id: "LRMW2-19", story: "P2", clause: "§4.6", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/analysis_dependent_functions.rs", test_fn: "analysis_unknown_name_fails_loud_by_typed_kind" } },
    Wave2Entry { id: "LRMW2-20", story: "P2", clause: "§5.9, §5.9.3", status: Wave2Status::Closed { file: "tests/analog_for_loop_unroll.rs", test_fn: "for_loop_sum_reaches_the_dc_operating_point" } },
    Wave2Entry { id: "LRMW2-21", story: "P2", clause: "§3.4.5", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_defparam.rs", test_fn: "defparam_onto_a_localparam_fails_with_typed_error_kind" } },
    Wave2Entry { id: "LRMW2-22", story: "P2", clause: "§6.4.2", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_paramset_overloading.rs", test_fn: "disjoint_range_selects_the_short_channel_paramset" } },
    Wave2Entry { id: "LRMW2-23", story: "P2", clause: "§7.3.2", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/digital_x_in_continuous_context.rs", test_fn: "x_valued_reg_fails_loud_naming_the_net_and_the_clause" } },
    Wave2Entry { id: "LRMW2-24", story: "P2", clause: "§3.11.1", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_nature_compatibility.rs", test_fn: "discipline_derived_from_voltage_connects_to_electrical" } },
    Wave2Entry { id: "LRMW2-25", story: "P2", clause: "§7.8.5", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_connect_naming_direction.rs", test_fn: "merged_instance_name_follows_the_lrm_scheme" } },
    Wave2Entry { id: "LRMW2-26", story: "P2", clause: "§8.4.3.3", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/digital_timescale_quantization.rs", test_fn: "quantize_time_rounds_to_the_declared_precision_grid" } },
    Wave2Entry { id: "LRMW2-27", story: "P2", clause: "§9.7.3", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/fatal_immediate.rs", test_fn: "fatal_is_routed_out_of_the_deferred_diagnostics_batch" } },
    Wave2Entry { id: "LRMW2-28", story: "P2", clause: "§9.15", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/simparam_unknown_key.rs", test_fn: "unknown_key_without_default_fails_loud_by_typed_kind" } },
    Wave2Entry { id: "LRMW2-29", story: "P3", clause: "§4.5.5", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/idt_assert_arg.rs", test_fn: "idt_with_an_explicit_assert_carries_it" } },
    Wave2Entry { id: "LRMW2-30", story: "P3", clause: "§4.5.6", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/idtmod_offset.rs", test_fn: "two_different_offset_params_const_eval_to_two_different_values" } },
    Wave2Entry { id: "LRMW2-31", story: "P3", clause: "§4.5.4", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/absdelay_maxdelay_arg.rs", test_fn: "absdelay_without_maxdelay_carries_the_bare_td" } },
    Wave2Entry { id: "LRMW2-32", story: "P3", clause: "§4.5.16", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/zi_tau_arg.rs", test_fn: "zi_without_tau_has_none" } },
    Wave2Entry { id: "LRMW2-33", story: "P3", clause: "§5.10.3", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/cross_tol_enable_arg.rs", test_fn: "cross_without_tolerances_or_enable_carries_none_and_the_default_enable" } },
    Wave2Entry { id: "LRMW2-34", story: "P3", clause: "§4.5.12", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/slew_fall_default.rs", test_fn: "slew_with_one_rate_defaults_fall_to_rise_not_zero" } },
    Wave2Entry { id: "LRMW2-35", story: "P3", clause: "§4.5.15", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/laplace_abstol_arg.rs", test_fn: "laplace_without_an_abstol_argument_has_none" } },
    Wave2Entry { id: "LRMW2-36", story: "P3", clause: "§4.5.11", status: Wave2Status::Closed { file: "tests/transition_time_tol.rs", test_fn: "a_tighter_time_tol_forces_more_accepted_steps_through_the_ramp" } },
    Wave2Entry { id: "LRMW2-37", story: "P3", clause: "§4.5.3", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/ddt_abstol_arg.rs", test_fn: "ddt_without_a_second_argument_has_no_abstol" } },
    Wave2Entry { id: "LRMW2-38", story: "P3", clause: "§9.17", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/discontinuity_degree_inert.rs", test_fn: "discontinuity_degree_does_not_change_the_step_pattern" } },
    Wave2Entry { id: "LRMW2-39", story: "P3", clause: "§2.9", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_ch1_lexical_form.rs", test_fn: "attribute_instance_before_case_parses_from_lrm_example_1" } },
    Wave2Entry { id: "LRMW2-40", story: "P4", clause: "§5.6.5", status: Wave2Status::Closed { file: "tests/switch_branch_relay_e2e.rs", test_fn: "relay_toggles_between_open_and_shorted_state_across_a_transient" } },
    Wave2Entry { id: "LRMW2-41", story: "P4", clause: "§5.6.7", status: Wave2Status::Closed { file: "tests/indirect_contribution_opamp_e2e.rs", test_fn: "opamp_follower_output_tracks_the_input" } },
    Wave2Entry { id: "LRMW2-42", story: "P4", clause: "§6.7, §6.2.1", status: Wave2Status::Closed { file: "tests/hierarchical_ref_e2e.rs", test_fn: "both_forms_read_the_nested_net_at_the_operating_point" } },
    Wave2Entry { id: "LRMW2-43", story: "P4", clause: "§6.3.2", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_positional_param_overrides.rs", test_fn: "positional_overrides_bind_in_declaration_order" } },
    Wave2Entry { id: "LRMW2-44", story: "P4", clause: "§4.7", status: Wave2Status::Closed { file: "tests/analog_function_e2e.rs", test_fn: "analog_function_return_drives_a_contribution" } },
    Wave2Entry { id: "LRMW2-45", story: "P4", clause: "§3.7, §7.3.1", status: Wave2Status::Closed { file: "tests/onebit_dac_e2e.rs", test_fn: "discrete_reg_read_zero_drives_the_analog_output_to_zero" } },
    Wave2Entry { id: "LRMW2-46", story: "P4", clause: "§7.9, §9.22", status: Wave2Status::Closed { file: "crates/piperine-solver/tests/digital_driver_segregation.rs", test_fn: "multiply_driven_net_resolves_per_table_7_3_and_driver_table_is_live" } },
    Wave2Entry { id: "LRMW2-47", story: "P4", clause: "§8.5.3.1/2", status: Wave2Status::Closed { file: "tests/procedural_continuous_assign_e2e.rs", test_fn: "before_force_the_normal_driver_sets_the_value" } },
    Wave2Entry { id: "LRMW2-48", story: "P4", clause: "§9.5", status: Wave2Status::Closed { file: "crates/piperine-codegen/src/emit/runtime_io.rs", test_fn: "a_model_writes_a_file_across_a_transient_and_the_test_reads_it_back" } },
    Wave2Entry { id: "LRMW2-49", story: "P4", clause: "§9.20", status: Wave2Status::Closed { file: "tests/node_alias_e2e.rs", test_fn: "aliased_node_tracks_the_target_nets_driven_value" } },
    Wave2Entry { id: "LRMW2-50", story: "P4", clause: "§9.16", status: Wave2Status::Closed { file: "tests/simprobe_matches_host_probe.rs", test_fn: "simprobe_agrees_with_the_host_probe_api_on_a_live_circuit" } },
    Wave2Entry { id: "LRMW2-51", story: "P4", clause: "§8.5.1", status: Wave2Status::Closed { file: "crates/piperine-solver/src/digital/regions.rs", test_fn: "d2a_and_analog_macro_process_resolve_in_the_lrm_stated_order" } },
    Wave2Entry { id: "LRMW2-52", story: "P4", clause: "§6.6, §6.9.4", status: Wave2Status::Closed { file: "tests/generate_items_e2e.rs", test_fn: "generated_net_instances_and_behavior_combine_in_a_real_dc_solve" } },
    Wave2Entry { id: "LRMW2-53", story: "P4", clause: "§6.5.7.1, §6.5.8", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/port_size_matching.rs", test_fn: "scalar_port_connected_to_a_vector_net_fails_loud_citing_6_5_7_1" } },
    Wave2Entry { id: "LRMW2-54", story: "P4", clause: "Table 4-14/4-15", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/math_intrinsics_lrm.rs", test_fn: "ceil_matches_table_4_14" } },
    Wave2Entry { id: "LRMW2-55", story: "P5", clause: "§9.13 / 1364 §17.9.3", status: Wave2Status::Closed { file: "crates/piperine-lang/src/eval/rng.rs", test_fn: "random_matches_the_ieee_1364_2005_17_9_3_reference_stream_for_a_fixed_seed" } },
    Wave2Entry { id: "LRMW2-56", story: "P5", clause: "§7.4.4.2", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_discipline_resolution.rs", test_fn: "detail_mode_ignores_resolveto_and_fails_loud_on_the_same_discrete_pair" } },
    Wave2Entry { id: "LRMW2-57", story: "P5", clause: "§8.5.1", status: Wave2Status::Closed { file: "crates/piperine-solver/tests/scheduler_region_queue_parity.rs", test_fn: "single_driver_per_net_matches_the_region_queue_oracle_across_every_region" } },
    Wave2Entry { id: "LRMW2-58", story: "P5", clause: "§6.4", status: Wave2Status::Closed { file: "crates/piperine-lang/tests/vams_paramset.rs", test_fn: "both_paramset_forms_parse_to_the_same_pom" } },
    Wave2Entry { id: "LRMW2-59", story: "P5", clause: "§8.5.1, §9.4", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/vams_digital_monitor_defer.rs", test_fn: "strobe_lands_after_display_from_the_same_evaluation_despite_source_order" } },
    Wave2Entry { id: "LRMW2-60", story: "P5", clause: "§7.3.3", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/a2d_comparator_interpolation.rs", test_fn: "crossing_schedules_at_the_interpolated_instant_not_the_timepoint" } },
    Wave2Entry { id: "LRMW2-61", story: "P5", clause: "§9.18", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/vams_mfactor_scaling.rs", test_fn: "mfactor_scales_branch_flow_without_the_model_reading_it" } },
    Wave2Entry { id: "LRMW2-62", story: "P5", clause: "§9.19", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/vams_placement.rs", test_fn: "declared_placement_reaches_the_models_reads" } },
    Wave2Entry { id: "LRMW2-63", story: "P5", clause: "§3.3, §4.2", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/vams_string_concat.rs", test_fn: "concatenation_joins_string_literals" } },
    Wave2Entry { id: "LRMW2-64", story: "P5", clause: "§8.3", status: Wave2Status::Closed { file: "crates/piperine-codegen/tests/vams_switch_primitives.rs", test_fn: "tranif1_conducts_when_enabled" } },
    Wave2Entry { id: "LRMW2-65", story: "P6", clause: "whole LRM", status: Wave2Status::Closed { file: "tests/lrm_section_coverage.rs", test_fn: "covered_and_excluded_partition_the_lrm" } },
    Wave2Entry { id: "LRMW2-66", story: "P6", clause: "—", status: Wave2Status::Pending { reason: "SPEC_DEVIATION.md is a regenerated doc (T70: \"Tests: none (doc)\") with no assertion to name" } },
];

const WAVE2_SPEC: &str = include_str!("../.specs/features/lrm-conformance-wave2/spec.md");

/// Every `LRMW2-*` id appearing as the first column of a Requirement
/// Traceability table row in wave-2 spec.md, parsed directly from the live
/// file — the same shape as wave-1's `spec_table_ids()`, adapted for the
/// `LRMW2-` prefix.
fn wave2_spec_table_ids() -> Vec<String> {
    WAVE2_SPEC
        .lines()
        .filter(|l| l.trim_start().starts_with("| LRMW2-"))
        .map(|l| {
            let id = l.trim_start().trim_start_matches('|').split('|').next().unwrap().trim();
            id.to_string()
        })
        .collect()
}

#[test]
fn wave2_registry_has_no_duplicate_ids() {
    let mut seen = std::collections::HashSet::new();
    for e in WAVE2_REGISTRY {
        assert!(seen.insert(e.id), "duplicate LRMW2-ID in WAVE2_REGISTRY: {}", e.id);
    }
}

#[test]
fn wave2_registry_exactly_matches_spec_table_id_set() {
    let spec_ids = wave2_spec_table_ids();
    assert_eq!(spec_ids.len(), 66, "expected 66 LRMW2-* rows in spec.md's table, parsed {}", spec_ids.len());

    let mut spec_seen = std::collections::HashSet::new();
    for id in &spec_ids {
        assert!(spec_seen.insert(id.clone()), "wave-2 spec.md's own table has a duplicate row: {id}");
    }

    let registry_ids: std::collections::HashSet<&str> = WAVE2_REGISTRY.iter().map(|e| e.id).collect();

    let missing_from_registry: Vec<&String> =
        spec_ids.iter().filter(|id| !registry_ids.contains(id.as_str())).collect();
    assert!(
        missing_from_registry.is_empty(),
        "wave-2 spec.md table rows with no WAVE2_REGISTRY entry: {missing_from_registry:?}"
    );

    let orphaned_in_registry: Vec<&str> =
        registry_ids.iter().filter(|id| !spec_seen.contains(&id.to_string())).copied().collect();
    assert!(
        orphaned_in_registry.is_empty(),
        "WAVE2_REGISTRY entries with no matching wave-2 spec.md table row: {orphaned_in_registry:?}"
    );
}

/// Every `Wave2Status::Pending` entry must be one of the two ids disclosed
/// in the section note above (LRMW2-01/66) — any other id landing here
/// would silently swap a fabricatable closing proof for a shrug, which is
/// exactly what this task's constraints forbid.
#[test]
fn wave2_pending_entries_are_only_the_disclosed_doc_ids() {
    for e in WAVE2_REGISTRY {
        let should_be_pending = e.id == "LRMW2-01" || e.id == "LRMW2-66";
        let is_pending = matches!(e.status, Wave2Status::Pending { .. });
        assert_eq!(
            is_pending, should_be_pending,
            "{}: Pending status mismatch (is={is_pending}, should_be={should_be_pending})",
            e.id
        );
        if let Wave2Status::Pending { reason } = &e.status {
            assert!(!reason.is_empty(), "{}: Pending entry with no disclosed reason", e.id);
        }
    }
}

/// Mirrors wave-1's `every_closed_entry_names_a_real_test` (reusing the
/// exact same `test_function_exists` helper, since this is the same file)
/// for `WAVE2_REGISTRY`'s `Closed` entries.
#[test]
fn every_wave2_closed_entry_names_a_real_test() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for e in WAVE2_REGISTRY {
        let Wave2Status::Closed { file, test_fn } = &e.status else { continue };
        let path = root.join(file);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{}: file `{file}` does not exist", e.id));
            continue;
        };
        if !test_function_exists(&contents, test_fn) {
            failures.push(format!("{}: no `#[test] fn {test_fn}` found in `{file}`", e.id));
        }
    }
    assert!(failures.is_empty(), "closing-test references that don't resolve:\n  {}", failures.join("\n  "));
}

#[test]
fn every_wave2_id_has_a_non_empty_clause_and_story() {
    for e in WAVE2_REGISTRY {
        assert!(!e.clause.is_empty(), "{}: empty LRM clause", e.id);
        assert!(!e.story.is_empty(), "{}: empty story", e.id);
    }
}
