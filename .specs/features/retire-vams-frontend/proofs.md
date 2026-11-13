# retire-vams-frontend proofs

**HEAD**: 66172a86
**Tree**: clean
**Shell**: /bin/sh
**Result**: 6 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `cd crates/piperine-lang && for p in src/ast src/grammar src/elab src/eval src/pom src/value src/lexer.rs src/p` | pass |
| C2 | `cd crates/piperine-lang/tests && test -z "$(ls *.rs / grep -v '^msl_')"` | pass |
| C3 | `cargo build -p piperine-lang --all-targets 2>&1 / (! grep -qE '^(warning/error)')` | pass |
| C4 | `out=$(cargo test -p piperine-lang --test msl_lexer --test msl_parser --test msl_errors --test msl_corpus --tes` | pass |
| C5 | `cd crates/piperine-lang && code=$(find src tests -name '*.rs' / xargs sed 's@//.*@@') && for d in $(sed -n '/^` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^warning')` | pass |

## Output

`cd crates/piperine-lang && for p in src/ast src/grammar src/elab src/eval src/pom src/value src/lexer.rs src/p` - exit 0

```
(no output)
```

`cd crates/piperine-lang/tests && test -z "$(ls *.rs / grep -v '^msl_')"` - exit 0

```
(no output)
```

`cargo build -p piperine-lang --all-targets 2>&1 / (! grep -qE '^(warning/error)')` - exit 0

```
(no output)
```

`out=$(cargo test -p piperine-lang --test msl_lexer --test msl_parser --test msl_errors --test msl_corpus --tes` - exit 0

```
(no output)
```

`cd crates/piperine-lang && code=$(find src tests -name '*.rs' / xargs sed 's@//.*@@') && for d in $(sed -n '/^` - exit 0

```
(no output)
```
