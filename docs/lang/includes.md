# Includes

Piperine supports file inclusion via the `` `include `` directive, similar to Verilog and SystemVerilog.

## Syntax

```verilog
`include "filename.ppr"
```

The file is included verbatim at the directive location, before parsing continues.

## Include paths

When calling `parse_with_includes()`, you supply a list of search directories. The parser looks for the file in each directory in order:

```rust
let dirs = vec![
    piperine_ngspice::ppr_dir(),         // bundled ngspice.ppr
    piperine_parser::bundled_header_dir(), // bundled stdlib headers
    PathBuf::from("./mylib"),             // project-local headers
];
let doc = parse_with_includes(src, &dirs)?;
```

## ngspice built-in library

The most common include is the bundled ngspice component declarations:

```verilog
`include "ngspice.ppr"
```

This makes all ngspice components available: `res`, `cap`, `ind`, `vsource`, `nmos`, `d`, etc.

The file lives at `crates/piperine-ngspice/ppr/ngspice.ppr`. The Rust function `piperine_ngspice::ppr_dir()` returns its parent directory so tools can add it to the include path.

## Include guards

`ngspice.ppr` uses include guards to prevent double-inclusion:

```verilog
`ifndef NGSPICE_PPR
`define NGSPICE_PPR

// ... declarations ...

`endif // NGSPICE_PPR
```

User files should do the same if they might be included multiple times:

```verilog
`ifndef MY_MODELS_PPR
`define MY_MODELS_PPR

// ... declarations ...

`endif
```

## Preprocessor directives

| Directive | Description |
|-----------|-------------|
| `` `include "file" `` | Include file |
| `` `ifndef MACRO `` | Conditional: if macro not defined |
| `` `define MACRO `` | Define a macro |
| `` `endif `` | End conditional |

Macro expansion (`` `MACRO `` substitution with values) is not currently supported — `define` is used only for guard macros.

## Relative vs absolute paths

Include paths are searched relative to the include search directories, not the including file's directory. Use explicit search directory configuration to find project-local files.
