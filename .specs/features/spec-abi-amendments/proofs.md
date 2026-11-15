# spec-abi-amendments proofs

**HEAD**: 7464dd41
**Tree**: clean
**Shell**: /bin/sh
**Result**: 13 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run spec / test later_commit_is_not_a_time_point ... ok` | pass |
| C2 | `run spec / test time_point_runs_the_procedure ... ok` | pass |
| C3 | `run spec / test extern_c_interface_is_the_device_abi ... ok` | pass |
| C4 | `run spec / test structure_changing_set_is_refused ... ok` | pass |
| C5 | `run spec / test zero_bytes_are_the_unknown_value ... ok` | pass |
| C6 | `run spec / test continuation_variables_are_core_functions ... ok` | pass |
| C7 | `run spec / test records_travel_as_quiet_commits ... ok` | pass |
| C8 | `run spec / test jump_has_no_solve_of_its_own ... ok` | pass |
| C9 | `run spec / test reference_rows_describe_the_kept_jump ... ok` | pass |
| C10 | `run spec / test residual_test_uses_the_present_mapping ... ok` | pass |
| C11 | `run spec / test jump_solve_is_a_transient_step ... ok` | pass |
| C12 | `cargo test -p piperine-lang` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run spec` - exit 0

```
test e0162_is_defined_in_3_4 ... ok
test every_method_called_in_a_spec_fence_is_catalogued ... ok
test s3_states_the_intrinsic_contract ... ok
test time_point_runs_the_procedure ... ok
test tran_in_fences_is_the_module ... ok
test gnd_and_the_implicit_params_are_core_names ... ok
test zero_bytes_are_the_unknown_value ... ok
test every_error_code_is_defined_exactly_once ... ok
test s36_defines_the_package_codes ... ok

test result: ok. 79 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-spec-amend/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.77s
     Running tests/spec_text.rs (/var/tmp/piperine-target/debug/deps/spec_text-8e0716f5857daff9)
```

`cargo test -p piperine-lang` - exit 0

```
     Running tests/pom_body.rs (/var/tmp/piperine-target/debug/deps/pom_body-0eda4aa62bb7a864)
     Running tests/pom_classify.rs (/var/tmp/piperine-target/debug/deps/pom_classify-a28ce0a42d0b3e76)
     Running tests/pom_corpus.rs (/var/tmp/piperine-target/debug/deps/pom_corpus-2a1a1bbd546c609c)
     Running tests/pom_crates.rs (/var/tmp/piperine-target/debug/deps/pom_crates-a9f38328ce432af0)
     Running tests/pom_design.rs (/var/tmp/piperine-target/debug/deps/pom_design-6114fd27de7dd9d0)
     Running tests/pom_docs.rs (/var/tmp/piperine-target/debug/deps/pom_docs-031147b33902f8bc)
     Running tests/pom_reflect.rs (/var/tmp/piperine-target/debug/deps/pom_reflect-ef5de612485d375e)
     Running tests/pom_resolve.rs (/var/tmp/piperine-target/debug/deps/pom_resolve-f44ddda9709d3f18)
     Running tests/pom_tree.rs (/var/tmp/piperine-target/debug/deps/pom_tree-f9daa3229358c79f)
     Running tests/spec_text.rs (/var/tmp/piperine-target/debug/deps/spec_text-8e0716f5857daff9)
     Running tests/std_excerpts.rs (/var/tmp/piperine-target/debug/deps/std_excerpts-420843ec3999f39a)
     Running tests/std_galleries.rs (/var/tmp/piperine-target/debug/deps/std_galleries-f0c43a9b8d6704b6)
     Running tests/std_library.rs (/var/tmp/piperine-target/debug/deps/std_library-d659b89a2fdefd78)
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target/debug/deps/suite_hygiene-cfb978c38d167293)
   Doc-tests piperine_lang
```
