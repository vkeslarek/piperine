# dv-core Baseline (captured in T1, before any feature change)

Branch: `feature/device-verification`
Date: 2026-07-27
Command profile: every cargo invocation with `CARGO_PROFILE_DEV_DEBUG=0`.

## Workspace test counts (`cargo test --workspace`)

**Workspace total: 1204 passed, 0 failed, 4 ignored**

| Crate | Passed | Failed | Ignored |
|---|---|---|---|
| piperine (root) | 169 | 0 | 0 |
| piperine-lang | 355 | 0 | 0 |
| piperine-codegen | 152 | 0 | 0 |
| piperine-solver | 251 | 0 | 3 |
| piperine-api | 44 | 0 | 0 |
| piperine-python | 59 | 0 | 0 |
| piperine-plugin | 49 | 0 | 1 |
| piperine-plugin-macros | 7 | 0 | 0 |
| piperine-project | 26 | 0 | 0 |
| piperine-cli | 23 | 0 | 0 |
| piperine-lang-server | 69 | 0 | 0 |

(Sum of per-crate runs = 1204; matches the workspace run's total.)

## Clippy

`cargo clippy --workspace --all-targets` — exit 0, clean (zero warnings).

## Rustdoc warnings (`cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli`)

Exit 0, **0 warnings** workspace-wide, hence 0 for each of the four documenting
crates (piperine-lang, piperine-codegen, piperine-solver, piperine-api).
