# Verilog-AMS Built-in Tasks and Functions

**Merged into `docs/GAPS.md` and `docs/BUILTINS.md` (2026-07-01).** This
file was a wishlist/design doc predating most of the actual
implementation work. Its content is now split:

- **What each builtin *is* and how it's used** (arguments, semantics,
  per-function reference) — see `docs/BUILTINS.md`.
- **What's still not implemented or not runtime-consumed** (e.g.
  `$limit`, `$param_given`, `$bound_step` having no solver-side effect,
  `cross`/`above`/`timer` not firing, `$finish`/`$fatal` having no
  runtime effect) — see `docs/GAPS.md`.

The Wave A–E priority buckets in the original are superseded by
`docs/GAPS.md`'s severity ratings on the same items.
