# Generic Module Monomorphization

## Generic modules in PHDL

PHDL supports generic module parameters via the syntax:

```phdl
mod Foo[N: Natural] {
    // N is a compile-time constant parameter
}
```

where `N` is a `Natural` const parameter.  A generic module is a template — it
cannot be instantiated directly; it must first be *monomorphized* by binding
all const parameters to concrete values.

## Mangling scheme

Monomorphized module names encode their const arguments with a double-underscore
separator:

| Generic module     | Instantiation     | Monomorphized name  |
| ------------------ | ----------------- | ------------------- |
| `Foo[N]`           | `Foo[8]`          | `Foo__8`            |
| `Bar[M][N]`        | `Bar[2][3]`       | `Bar__2_3`          |

Underscores separate multiple const arguments.

## On-demand monomorphization

Monomorphization is **not** pre-computed during elaboration startup.  It is
triggered on demand when `lower_mod_stmt()` encounters an instance with const
arguments.

## `Elaborator::monomorphize()`

```rust
fn monomorphize(
    &mut self,
    module_name: &str,
    const_args: &[ConstArg],
) -> Result<String, String>
```

Algorithm:

1. **Compute the monomorphized name.**  Concatenate `module_name`, `__`, and
   each const argument separated by `_`.
2. **Check `mono_cache`.**  If a module with this name has already been
   monomorphized, return the name immediately (no re-elaboration).
3. **Look up the generic module declaration** from `module_decls`.
4. **Validate** that the number of const parameters in the declaration matches
   the number of provided arguments.
5. **Create a `ConstEnv`** binding each const parameter to its concrete value.
6. **Clone the module declaration** and rename it to the monomorphized name.
7. **Insert a placeholder** `Module` into `mono_cache` **before** elaboration
   to break recursion.  This allows self-referential instantiations (a
   monomorphized instance of a generic module may instantiate itself with the
   same or different const args).
8. **Call `elab_mod_inner()`** with:
   - the monomorphized declaration (cloned and renamed)
   - the const env (bound to concrete values)
   - an empty `type_subst`
9. **Replace the placeholder** in `mono_cache` with the fully elaborated
   module.
10. **Return** the monomorphized name.

## How instances trigger monomorphization

In `lower_mod_stmt()`, when an instance statement is processed:

1. Const arguments are evaluated as `Natural` values via
   `env.eval_nat(arg)`.
2. The monomorphized name is computed from the base module name and the
   evaluated const args.
3. If const args are non-empty, `monomorphize()` is called to ensure the
   concrete module exists.
4. The instance’s `module` field is set to the monomorphized name.

This ensures that every instance in the elaborated design references a
concrete (non-generic) module.

## The `elaborate()` driver method

The elaboration driver proceeds in two phases:

1. **Non-generic modules** — all modules with **both** `const_params` and
   `type_params` empty are elaborated first via `elab_mod_inner()`.
2. **Generic modules** — monomorphized modules are built on demand during
   `elab_mod_inner()` / `lower_mod_stmt()` as instances are encountered.

After all top-level elaboration is finished, `mono_cache` entries are merged
into the program’s module map:

```rust
for (name, elab_mod) in self.mono_cache {
    prog.modules_map_mut()
        .entry(name)
        .or_insert(elab_mod);
}
```

The `or_insert` semantics ensure that already-elaborated top-level modules
(non-generic) are not overwritten by cache entries.

## Post-monomorphization guarantees

- **`is_generic()` on `Module` always returns `false`** after
  monomorphization.  Once a module is monomorphized, all its const and type
  parameters are resolved; it behaves identically to a hand-written concrete
  module.
- Every module in the final `Design` is concrete.

## `mono_cache`

The `mono_cache` lives on the `Elaborator` struct:

```rust
HashMap<String, Module>
```

It maps mangled name → elaborated `Module`.  Placeholder insertion (step 7
above) ensures that recursive instantiations terminate — when a module
monomorphized as `Foo__8` encounters another instance of `Foo[8]`, the
already-inserted placeholder short-circuits the lookup.
