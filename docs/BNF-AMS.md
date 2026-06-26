# Verilog-AMS BNF Coverage — Implementation Spec

**Standard:** Verilog-AMS 2.4.0, Annex A (Formal Syntax Definition)  
**Crate:** `crates/piperine-parser/`  
**Goal:** 100% BNF production coverage so any syntactically valid Verilog-AMS file parses without error.

---

## 1. Status Summary Table

| BNF Rule | Status | File:Line | Gap |
|---|---|---|---|
| `source_text` | ✅ | `grammar/mod.rs:17` | — |
| `module_declaration` | ⚠️ | `grammar/item.rs:67` | Missing `#(param_port_list)` before port list |
| `module_keyword` (module/macromodule/connectmodule) | ✅ | `grammar/item.rs:68` | — |
| `list_of_ports` — `(port, port)` bare names | ✅ | `grammar/item.rs:91` | — |
| `list_of_ports` — `.id([expr])` form | ❌ | `grammar/item.rs:99` | No `.ident(expr)` case |
| `port_reference` with optional range | ❌ | `grammar/item.rs:99` | Bare name ports accept no `[range]` |
| `list_of_port_declarations` — inline direction decls | ✅ | `grammar/item.rs:97` | — |
| `port_declaration` — all direction + type combos | ✅ | `grammar/item.rs:109` | — |
| `module_parameter_port_list` (`#(parameter...)`) | ❌ | `grammar/item.rs:77` | `module()` never checks `#` before ports |
| `paramset_declaration` | ✅ | `grammar/item.rs:136` | — |
| `nature_declaration` | ✅ | `grammar/item.rs:48` | — |
| `discipline_declaration` | ✅ | `grammar/item.rs:31` | — |
| `connectrules_declaration` | ⚠️ | `grammar/item.rs:173` | `connect_port_overrides` skipped; `parameter_value_assignment` missing |
| `connectrules` — connect_port_overrides | ❌ | `grammar/item.rs:185` | `ConnectPortOverrides` is empty struct; parser bumps to `;` |
| `config_declaration` | ⚠️ | `grammar/item.rs:203` | Stub — skips all content to `endconfig` |
| `udp_declaration` (`primitive`) | ❌ | `grammar/item.rs:8` | `item()` never dispatches on `primitive` keyword |
| `net_declaration` — net_type + discipline + range + names | ✅ | `grammar/item.rs:259` | — |
| `net_declaration` — `drive_strength` | ❌ | `grammar/item.rs:260` | Field always `None` |
| `net_declaration` — `charge_strength` | ❌ | `grammar/item.rs:260` | Field always `None` |
| `net_declaration` — `delay3` (`#delay`) | ❌ | `grammar/item.rs:260` | Field always `None` |
| `net_declaration` — `vectored`/`scalared` | ❌ | `grammar/item.rs:259` | Never parsed |
| `ground` net declaration | ✅ | `grammar/item.rs:286` | — |
| `reg_declaration` | ✅ | `grammar/item.rs:271` | via `var_decl` with `Type::Reg` |
| `integer/real/realtime/time/event` declarations | ✅ | `grammar/item.rs:271` | — |
| `genvar_declaration` | ✅ | `grammar/item.rs:272` | — |
| `branch_declaration` — named + port-branch | ✅ | `grammar/item.rs:599` | — |
| `analog_function_declaration` | ✅ | `grammar/item.rs:616` | — |
| `function_declaration` | ⚠️ | `grammar/item.rs:627` | `automatic` keyword not consumed |
| `task_declaration` | ❌ | — | Completely missing |
| `specparam_declaration` | ❌ | — | AST exists; no dispatch in `module_item()` |
| `specify_block` | ❌ | — | AST stub; no dispatch in `module_item()` |
| `aliasparam_declaration` | ✅ | `grammar/item.rs:577` | — |
| `parameter_override` (defparam) | ✅ | `grammar/item.rs:371` | — |
| `local_parameter_declaration` (localparam) | ✅ | `grammar/item.rs:307` | — |
| `module_instantiation` | ✅ | `grammar/item.rs:417` | — |
| `parameter_value_assignment` (`#(...)`) | ✅ | `grammar/item.rs:420` | — |
| `named_port_connection` (`.port(expr)`) | ✅ | `grammar/item.rs:451` | — |
| `wildcard port connection` (`.*`) | ✅ | `grammar/item.rs:447` | — |
| `gate_instantiation` | ❌ | — | AST exists; falls through to `module_instantiation()` |
| `continuous_assign` | ⚠️ | `grammar/item.rs:384` | `drive_strength` + `delay3` always `None` |
| `initial_construct` | ✅ | `grammar/item.rs:361` | — |
| `always_construct` | ✅ | `grammar/item.rs:366` | — |
| `analog_construct` | ✅ | `grammar/item.rs:616` | — |
| `generate_region` | ✅ | `grammar/item.rs:477` | — |
| `loop_generate_construct` | ✅ | `grammar/item.rs:502` | — |
| `if_generate_construct` | ✅ | `grammar/item.rs:522` | — |
| `case_generate_construct` | ✅ | `grammar/item.rs:532` | — |
| `analog_case_statement` — case/casex/casez | ✅ | `grammar/stmt.rs:169` | — |
| `case_statement` — case/casex/casez | ✅ | `grammar/stmt.rs:169` | — |
| `analog_conditional_statement` | ✅ | `grammar/stmt.rs:131` | — |
| `analog_loop_statement` — repeat/while/for | ✅ | `grammar/stmt.rs:74` | — |
| `forever_statement` | ✅ | `grammar/stmt.rs:83` | — |
| `contribution_statement` (`<+`) | ✅ | `grammar/stmt.rs:55` | — |
| `indirect_contribution_statement` | ✅ | `grammar/stmt.rs:42` | — |
| `analog_event_control_statement` (`@(cross/above/timer/...)`) | ✅ | `grammar/stmt.rs:198` | — |
| `blocking_assignment` | ✅ | `grammar/stmt.rs:40` | — |
| `nonblocking_assignment` (`<=`) | ✅ | `grammar/stmt.rs:49` | — |
| `procedural_continuous_assignments` | ✅ | `grammar/stmt.rs:248` | — |
| `seq_block` (begin/end) | ✅ | `grammar/stmt.rs:91` | — |
| `par_block` (fork/join) | ✅ | `grammar/stmt.rs:223` | — |
| `wait_statement` | ✅ | `grammar/stmt.rs:214` | — |
| `disable_statement` | ✅ | `grammar/stmt.rs:234` | — |
| `event_trigger` (`->`) | ✅ | `grammar/stmt.rs:241` | — |
| `system_task_enable` | ✅ | `grammar/expr.rs:225` | via SysCall expr |
| `task_enable` (user task call) | ⚠️ | `grammar/expr.rs:235` | Parsed as `ExprStmt`; no semantic distinction |
| Unary operators (all) | ✅ | `grammar/expr.rs:112` | Including reduction operators |
| Binary operators (all) | ✅ | `grammar/expr.rs:44` | Including `===`, `!==`, `<<<`, `>>>`, `**` |
| Bit-select `[e]` | ✅ | `grammar/expr.rs:131` | — |
| Part-select `[msb:lsb]` | ✅ | `grammar/expr.rs:136` | — |
| Part-select `[base+:width]` | ✅ | `grammar/expr.rs:139` | — |
| Part-select `[base-:width]` | ✅ | `grammar/expr.rs:142` | — |
| Concatenation `{...}` | ✅ | `grammar/expr.rs:179` | — |
| Multiple concatenation `{N{...}}` | ✅ | `grammar/expr.rs:182` | — |
| Ternary `?:` | ✅ | `grammar/expr.rs:15` | — |
| `mintypmax_expression` (`a:b:c`) | ✅ | `grammar/expr.rs:270` | — |
| Analog built-in functions (all) | ✅ | `grammar/expr.rs:235` | Generic `Call` |
| Analog filter functions (ddt/idt/transition/slew/...) | ✅ | `grammar/expr.rs:235` | Generic `Call` |
| Analog noise functions (white_noise/flicker_noise/...) | ✅ | `grammar/expr.rs:235` | Generic `Call` |
| `cross`, `above`, `timer`, `absdelta` | ✅ | `grammar/expr.rs:235` | Generic `Call` |
| `analysis(...)` | ✅ | `grammar/expr.rs:235` | Generic `Call` |
| `branch_probe_function_call` (`V(net)`, `I(branch)`) | ✅ | `grammar/expr.rs:235` | — |
| `port_probe_function_call` (`V(<port>)`) | ✅ | `grammar/expr.rs:218` | `Expr::PortFlow` |
| Number literals — decimal/binary/octal/hex | ✅ | `lexer.rs` | `SizedLit` token |
| Number literals — real + exponent | ✅ | `lexer.rs` | `Tok::Real` |
| Scale factors (T/G/M/K/k/m/u/n/p/f/a) | ✅ | `lexer.rs:283` | — |
| String literals | ✅ | `lexer.rs` | `Tok::Str` |
| `attribute_instance` (`(*attr=expr*)`) | ✅ | `grammar/mod.rs:128` | `AttrStart`/`AttrEnd` |
| Line/block comments | ✅ | `lexer.rs` | Discarded |
| `escaped_identifier` | ✅ | `lexer.rs` | `backslash_or_escaped_ident` |
| `system_function_identifier` (`$id`) | ✅ | `lexer.rs` | `Tok::SysCall` |
| Hierarchical identifier | ✅ | `grammar/mod.rs:142` | Dotted `Path` |
| Preprocessor directives (`` `include ``, `` `define ``, `` `ifdef ``) | ✅ | `model.rs` | `Tok::Tick` |

**Counts:** ~65 ✅ / ~8 ⚠️ / ~12 ❌

---

## 2. Implementation Guide

Each gap below has: current state, exact AST changes (with Rust code), exact parser changes (with code and line references), and a test snippet.

---

### P1 — Breaks Real AMS Files

---

### Gap 1: `task_declaration` (BNF A.2.7)

**BNF:**
```
task_declaration ::=
    task [ automatic ] task_identifier ;
    { task_item_declaration }
    statement_or_null
    endtask
  | task [ automatic ] task_identifier ( [ task_port_list ] ) ;
    { block_item_declaration }
    statement_or_null
    endtask

task_item_declaration ::=
    block_item_declaration
  | { attribute_instance } tf_input_declaration ;
  | { attribute_instance } tf_output_declaration ;
  | { attribute_instance } tf_inout_declaration ;

tf_input_declaration ::=
    input [ discipline_identifier ] [ reg ] [ signed ] [ range ] list_of_port_identifiers
  | input task_port_type list_of_port_identifiers

task_port_type ::= integer | real | realtime | time
```

**Current state:** No `task` keyword handler anywhere. `module_item()` in `grammar/item.rs:215` will fall through to `net_decl()` and produce a garbage `NetDecl` named `task`, then fail on the second token.

**AST changes** — add to `ast/item.rs`:

```rust
// Add to ModuleItem enum (ast/item.rs ~line 82):
TaskDecl(TaskDecl),

// New types to add at end of ast/item.rs:
#[derive(Debug, Clone)]
pub struct TaskDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub automatic: bool,
    pub name: Name,
    pub ports: Vec<TaskPort>,  // from parenthesized form; empty for old-style
    pub items: Vec<TaskItem>,
    pub body: Box<Stmt>,
}

#[derive(Debug, Clone)]
pub enum TaskItem {
    BlockItem(BlockItem),   // reg/integer/real/event decl or statement
    Port(TaskPort),         // input/output/inout declaration (old-style body)
}

#[derive(Debug, Clone)]
pub struct TaskPort {
    pub attrs: Vec<Attr>,
    pub dir: Direction,
    pub port_type: Option<Type>,   // integer|real|realtime|time
    pub discipline: Option<NameRef>,
    pub reg: bool,
    pub signed: bool,
    pub range: Option<BitRange>,
    pub names: Vec<Name>,
}
```

**Parser changes** — `grammar/item.rs`:

Step 1 — Add dispatch in `module_item()` after the `always` branch (around line 237):
```rust
// in module_item(), after:  if self.eat_kw("always") { return self.always_construct(attrs, start); }
if self.eat_kw("task") { return self.task_decl(attrs, start); }
```

Step 2 — Add `task_decl()` function to `impl Parser` in `grammar/item.rs`:
```rust
fn task_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    let automatic = self.eat_kw("automatic");
    let name = self.name()?;

    // Detect parenthesized port list: `task foo (input real a, ...);`
    let ports = if self.eat(&Tok::LParen) {
        let mut ports = Vec::new();
        while !self.at(&Tok::RParen) && !self.at_end() {
            let port_attrs = self.attrs()?;
            let dir = self.direction()?;
            ports.push(self.task_port_rest(port_attrs, dir)?);
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        ports
    } else {
        Vec::new()
    };

    self.expect(&Tok::Semi)?;

    // Body: old-style has declarations + final statement.
    // Parenthesized form has block_item_declarations + statement.
    let mut items = Vec::new();
    while !self.at_kw("endtask") && !self.at_end() {
        // Try to parse as task item (direction decl or block item).
        // A direction keyword starts a port decl; otherwise it's a block item.
        if self.at_dir() && ports.is_empty() {
            // Old-style port: `input real x;`
            let item_attrs = self.attrs()?;
            let dir = self.direction()?;
            let port = self.task_port_rest(item_attrs, dir)?;
            self.expect(&Tok::Semi)?;
            items.push(TaskItem::Port(port));
        } else if !self.at_stmt_kw() && (self.is_type_kw() || self.at_kw("parameter") || self.at_kw("localparam")) {
            let item_attrs = self.attrs()?;
            let item_start = self.span_start();
            if self.at_kw("parameter") || self.at_kw("localparam") {
                items.push(TaskItem::BlockItem(BlockItem::ParamDecl(self.param_decl(item_attrs, item_start)?)));
            } else {
                items.push(TaskItem::BlockItem(BlockItem::VarDecl(self.var_decl(item_attrs, item_start)?)));
            }
        } else {
            // Last item is the body statement.
            // Peek: is this the final statement before endtask?
            // We collect all statements as TaskItem::BlockItem(Stmt).
            let s = self.stmt()?;
            items.push(TaskItem::BlockItem(BlockItem::Stmt(s)));
        }
    }
    self.expect_kw("endtask")?;

    // Split items: last BlockItem::Stmt is the body; everything before is task items.
    // If no stmt items found, use Empty stmt as body.
    let body = {
        let last_stmt = items.iter().rposition(|i| matches!(i, TaskItem::BlockItem(BlockItem::Stmt(_))));
        if let Some(idx) = last_stmt {
            if let TaskItem::BlockItem(BlockItem::Stmt(s)) = items.remove(idx) {
                Box::new(s)
            } else { unreachable!() }
        } else {
            Box::new(Stmt::Empty(EmptyStmt { attrs: vec![] }))
        }
    };

    Ok(ModuleItem::TaskDecl(TaskDecl {
        span: Span { start, end: self.prev_end() },
        attrs, automatic, name, ports, items, body,
    }))
}

fn task_port_rest(&mut self, attrs: Vec<Attr>, dir: Direction) -> PResult<TaskPort> {
    // Optional: `task_port_type` (integer|real|realtime|time) OR `reg`
    let mut port_type = None;
    let mut reg = false;
    if self.at_any_kw(&["integer", "real", "realtime", "time"]) {
        port_type = Some(self.type_()?);
    } else {
        reg = self.eat_kw("reg");
    }
    let discipline = self.opt_discipline();
    let signed = self.opt_signed();
    let range = self.parse_range()?;
    let names = self.name_list()?;
    Ok(TaskPort { attrs, dir, port_type, discipline, reg, signed, range, names })
}
```

Step 3 — Add `task` to `at_stmt_kw()` in `grammar/stmt.rs:124` so block items don't confuse `task` with a variable type:
```rust
// in at_stmt_kw():
self.at_any_kw(&[
    "begin", "if", "while", "for", "case", "casex", "casez",
    "repeat", "forever", "wait", "fork", "disable",
    "assign", "force", "deassign", "release",
    "task",  // ADD THIS
]) || self.at(&Tok::Arrow)
```

**Test snippet** (currently fails to parse):
```verilog
module tb;
  task automatic drive_bus;
    input real voltage;
    input integer cycles;
    integer i;
    begin
      for (i = 0; i < cycles; i = i + 1)
        $display("t=%g v=%g", $abstime, voltage);
    end
  endtask
endmodule
```

---

### Gap 2: `module_parameter_port_list` — `#(parameter...)` on module header (BNF A.1.3)

**BNF:**
```
module_declaration ::=
    { attribute_instance } module_keyword module_identifier
    [ module_parameter_port_list ]
    list_of_ports ;
    { module_item }
    endmodule

module_parameter_port_list ::=
    # ( parameter_declaration { , parameter_declaration } )
```

**Current state:** `module()` in `grammar/item.rs:77`:
```rust
let ports = if self.at(&Tok::LParen) {
    Some(self.module_ports()?)
} else {
    None
};
```
It never checks for `#`. So `module Foo #(parameter real R=1k) (a,b);` fails because after parsing `Foo`, the parser sees `#` and doesn't know what to do.

**AST changes** — modify `ModuleDecl` in `ast/item.rs`:
```rust
// Replace the existing struct (currently at line 46-53):
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub kind: ModuleKind,
    pub name: Name,
    pub param_ports: Vec<ParamDecl>,  // ADD: from #(parameter ...) header
    pub ports: Option<Vec<ModulePort>>,
    pub items: Vec<ModuleItem>,
}
```

**Parser changes** — `grammar/item.rs`, modify `module()` starting at line 76:

```rust
fn module(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleDecl> {
    let kind = if self.eat_kw("macromodule") {
        ModuleKind::Macromodule
    } else if self.eat_kw("connectmodule") {
        ModuleKind::Connectmodule
    } else {
        self.expect_kw("module")?;
        ModuleKind::Module
    };
    let name = self.name()?;

    // NEW: optional #(parameter_declaration {, parameter_declaration})
    let mut param_ports = Vec::new();
    if self.eat(&Tok::Hash) {
        self.expect(&Tok::LParen)?;
        while !self.at(&Tok::RParen) && !self.at_end() {
            let pp_start = self.span_start();
            let pp_attrs = self.attrs()?;
            // Each item is a parameter or localparam declaration (without trailing ;)
            // The param_decl() function consumes the trailing `;`, so we use a
            // variant that doesn't expect `;`:
            param_ports.push(self.module_param_port(pp_attrs, pp_start)?);
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
    }

    let ports = if self.at(&Tok::LParen) {
        Some(self.module_ports()?)
    } else {
        None
    };
    self.expect(&Tok::Semi)?;
    let mut items = Vec::new();
    while !self.at_kw("endmodule") && !self.at_kw("endconnectmodule") && !self.at_end() {
        items.push(self.module_item()?);
    }
    self.expect_kw("endmodule")?;
    Ok(ModuleDecl { attrs, kind, name, param_ports, ports, items,
                    span: Span { start, end: self.prev_end() } })
}

/// Parse one parameter declaration in a `#(...)` module header.
/// Like `param_decl()` but does NOT consume a trailing `;`.
fn module_param_port(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ParamDecl> {
    let kind = if self.eat_kw("localparam") { ParamKind::LocalParam }
               else { self.expect_kw("parameter")?; ParamKind::Parameter };
    let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
    let name = self.name()?;
    self.skip_range();
    self.expect(&Tok::Assign)?;
    let default = self.expr()?;
    let mut constraints = Vec::new();
    while self.at_kw("from") || self.at_kw("exclude") {
        constraints.push(self.param_constraint()?);
    }
    Ok(ParamDecl {
        attrs, kind, ty,
        params: vec![Param { name, default, constraints }],
        span: Span { start, end: self.prev_end() },
    })
}
```

**Note:** Anywhere `ModuleDecl` is constructed or pattern-matched downstream, add `param_ports: vec![]` to the struct literal.

**Test snippet:**
```verilog
module lpf #(parameter real R = 1k, parameter real C = 1n) (in, out);
  input in;
  output out;
  electrical in, out;
  analog V(out) <+ V(in) / (1.0 + $realtime * R * C);
endmodule
```

---

### Gap 3: `specparam_declaration` dispatch (BNF A.2.1.1)

**BNF:**
```
specparam_declaration ::= specparam [ range ] list_of_specparam_assignments ;
list_of_specparam_assignments ::= specparam_assignment { , specparam_assignment }
specparam_assignment ::= specparam_identifier = constant_mintypmax_expression
```

**Current state:** `SpecparamDecl` AST type (at `ast/item.rs:479`) and `ModuleItem::Specparam` (at `ast/item.rs:80`) both exist. `module_item()` in `grammar/item.rs:215` never dispatches on `specparam` — the keyword falls through to `net_decl()` which tries to parse it as a discipline name, creating garbage.

**AST changes:** None — `SpecparamDecl` and `ModuleItem::Specparam` already exist.

**Parser changes** — `grammar/item.rs`, add to `module_item()` after the `aliasparam` branch (around line 233):

```rust
// in module_item(), after: if self.at_kw("aliasparam") { return self.alias_param(attrs, start); }
if self.eat_kw("specparam") { return self.specparam_decl(attrs, start); }
```

Add the `specparam_decl()` function:
```rust
fn specparam_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    let range = self.parse_range()?;
    let mut assignments = Vec::new();
    loop {
        let name = self.name()?;
        self.expect(&Tok::Assign)?;
        let expr = self.expr()?;
        // consume optional `:typ:max` mintypmax suffix
        let expr = self.opt_mintypmax(expr)?;
        assignments.push((name, expr));
        if !self.eat(&Tok::Comma) { break; }
    }
    self.expect(&Tok::Semi)?;
    Ok(ModuleItem::Specparam(SpecparamDecl {
        span: Span { start, end: self.prev_end() },
        attrs, range, assignments,
    }))
}
```

**Test snippet:**
```verilog
module delay_line (in, out);
  input in; output out;
  specparam tpd = 2.5e-9 : 3e-9 : 4e-9;
  specparam trise = 0.5e-9, tfall = 0.7e-9;
  specify
    (in => out) = tpd;
  endspecify
endmodule
```

---

### Gap 4: `specify_block` dispatch (BNF A.7.1)

**BNF:**
```
specify_block ::= specify { specify_item } endspecify
specify_item ::=
    specparam_declaration
  | pulsestyle_declaration
  | showcancelled_declaration
  | path_declaration
  | system_timing_check
```

**Current state:** `SpecifyBlock` (at `ast/item.rs:473`) is an empty struct. `ModuleItem::Specify` (at `ast/item.rs:79`) exists. `module_item()` never dispatches on `specify`.

**Design decision:** The specify block content (timing checks, path delays) is very complex. A pragmatic first implementation skips the body content and just parses enough to not error on valid files. A full implementation would add path_declaration, system_timing_check, etc. to the AST.

**AST changes** — minimal first pass, extend `SpecifyBlock` in `ast/item.rs`:
```rust
// Replace empty struct:
#[derive(Debug, Clone)]
pub struct SpecifyBlock {
    pub span: Span,
    // Full AST for specify items is future work.
    // For now, store raw item count so the block is recognized but not analyzed.
    pub item_count: usize,
}
```

**Parser changes** — `grammar/item.rs`, add to `module_item()`:

```rust
// in module_item(), after: if self.eat_kw("assign") { return self.continuous_assign(attrs, start); }
if self.eat_kw("specify") { return self.specify_block(attrs, start); }
```

Add `specify_block()`:
```rust
fn specify_block(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    // Skip contents: track paren depth to correctly skip nested path
    // expressions like `(posedge clk => (q:data))`.
    let mut item_count = 0;
    let mut depth = 0usize;
    while !self.at_end() {
        if self.at_kw("endspecify") && depth == 0 { break; }
        match self.peek() {
            Some(Tok::LParen) => { depth += 1; self.bump(); }
            Some(Tok::RParen) => { if depth > 0 { depth -= 1; } self.bump(); }
            Some(Tok::Semi) => { item_count += 1; self.bump(); }
            _ => { self.bump(); }
        }
    }
    self.expect_kw("endspecify")?;
    Ok(ModuleItem::Specify(SpecifyBlock { span: Span { start, end: self.prev_end() }, item_count }))
}
```

**Note:** For a full implementation, `SpecifyBlock` should gain `items: Vec<SpecifyItem>` and a proper parser. The skip-based approach suffices to parse files that contain specify blocks without crashing.

**Test snippet:**
```verilog
module dff (q, d, clk);
  output q; input d, clk;
  specify
    specparam tpd_clk_q = 3e-9;
    (posedge clk => (q : d)) = tpd_clk_q;
    $setup(d, posedge clk, 1e-9);
  endspecify
endmodule
```

---

### Gap 5: `gate_instantiation` routing (BNF A.3)

**BNF:**
```
gate_instantiation ::=
    cmos_switchtype [delay3] cmos_switch_instance { , cmos_switch_instance } ;
  | enable_gatetype [drive_strength] [delay3] enable_gate_instance ... ;
  | n_input_gatetype [drive_strength] [delay2] n_input_gate_instance ... ;
  | n_output_gatetype [drive_strength] [delay2] n_output_gate_instance ... ;
  | pulldown [pulldown_strength] pull_gate_instance ... ;
  | pullup [pullup_strength] pull_gate_instance ... ;
  | ... (see BNF for all forms)

n_input_gatetype ::= and | nand | or | nor | xor | xnor
n_output_gatetype ::= buf | not
enable_gatetype ::= bufif0 | bufif1 | notif0 | notif1
mos_switchtype ::= nmos | pmos | rnmos | rpmos
cmos_switchtype ::= cmos | rcmos
pass_switchtype ::= tran | rtran
pass_en_switchtype ::= tranif0 | tranif1 | rtranif1 | rtranif0
```

**Current state:** `GateInstantiation` AST (at `ast/item.rs:487`) and `ModuleItem::GateInstantiation` (at `ast/item.rs:81`) exist. `module_item()` at line 246 never checks for gate keywords before calling `is_module_instantiation()`. Gate names like `and`, `nand`, `or`, `buf`, `not`, `nmos`, `pmos`, `tran`, `cmos` are `Tok::Ident` and match the `is_module_instantiation()` heuristic (since the pattern `Ident Ident (` matches), so they parse as module instantiations with wrong structure.

**AST changes:** `GateInstance` and `GateInstantiation` already exist. No changes needed.

**Parser changes** — `grammar/item.rs`:

Step 1 — Add a constant for all gate type keywords:
```rust
// Add near top of grammar/item.rs, after the NET_TYPES and DIRECTIONS constants:
const GATE_TYPES: &[&str] = &[
    // n-input gates
    "and", "nand", "or", "nor", "xor", "xnor",
    // n-output gates
    "buf", "not",
    // enable gates
    "bufif0", "bufif1", "notif0", "notif1",
    // MOS switches
    "nmos", "pmos", "rnmos", "rpmos",
    // CMOS switches
    "cmos", "rcmos",
    // pass switches
    "tran", "rtran",
    // pass-enable switches
    "tranif0", "tranif1", "rtranif0", "rtranif1",
    // pull gates
    "pullup", "pulldown",
];
```

Step 2 — Add a helper to `impl Parser` in `grammar/mod.rs`:
```rust
fn at_gate_type(&self) -> bool {
    matches!(self.peek(), Some(Tok::Ident(s)) if GATE_TYPES.contains(&s.as_str()))
}
```

Step 3 — Add dispatch in `module_item()` BEFORE the `is_module_instantiation()` check (around line 246):
```rust
// ADD BEFORE: if self.is_module_instantiation() {
if self.at_gate_type() {
    return self.gate_instantiation(attrs, start);
}
```

Step 4 — Add `gate_instantiation()` parser:
```rust
fn gate_instantiation(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    let gate_type = self.name()?;  // consumes the gate keyword (it's an Ident)

    // Optional drive_strength: `(strength0, strength1)` — heuristic: if `(` follows
    // and next is a strength keyword, skip it.
    // For now: skip drive_strength (not needed to parse structurally).
    // TODO: parse DriveStrength here when fields are populated.

    // Optional delay2/delay3: `#delay` or `#(...)` 
    // For now: skip delay.
    // TODO: parse Delay here when fields are populated.
    if self.eat(&Tok::Hash) {
        if self.eat(&Tok::LParen) {
            let mut depth = 1;
            while depth > 0 && !self.at_end() {
                match self.peek() {
                    Some(Tok::LParen) => { depth += 1; self.bump(); }
                    Some(Tok::RParen) => { depth -= 1; self.bump(); }
                    _ => { self.bump(); }
                }
            }
        } else {
            // #scalar_delay
            self.expr()?;
        }
    }

    let mut instances = Vec::new();
    loop {
        // Optional instance name
        let name = if matches!(self.peek(), Some(Tok::Ident(_)))
            && !matches!(self.peek_at(1), Some(Tok::LParen))
        {
            // Has a name: `g1 (...)` or `g1 [range] (...)`
            let n = self.name()?;
            let r = self.parse_range()?;
            Some((n, r))
        } else {
            None
        };
        self.expect(&Tok::LParen)?;
        let mut terminals = Vec::new();
        while !self.at(&Tok::RParen) && !self.at_end() {
            if self.at(&Tok::Comma) {
                terminals.push(None);  // empty terminal (positional gap)
            } else {
                terminals.push(Some(self.expr()?));
            }
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        instances.push(GateInstance { name, terminals });
        if !self.eat(&Tok::Comma) { break; }
    }
    self.expect(&Tok::Semi)?;
    Ok(ModuleItem::GateInstantiation(GateInstantiation {
        span: Span { start, end: self.prev_end() },
        attrs, gate_type, instances,
    }))
}
```

**Note:** `nmos` and `pmos` are also valid device names in AMS models (MOSFET ports). The gate dispatch must happen BEFORE `is_module_instantiation()` so that primitive uses are correctly classified. If a user has a paramset named `nmos`, it will shadow the primitive — this is correct LRM behavior.

**Test snippet:**
```verilog
module logic_gates (y1, y2, a, b, c);
  output y1, y2; input a, b, c;
  and  g_and(y1, a, b);
  nand #(1e-9, 2e-9) g_nand(y2, a, b, c);
  not  (w, a);
  buf  bufA (w2, a);
endmodule
```

---

### P2 — Missing Features

---

### Gap 6: `module_parameter_port_list` + `list_of_ports` `.id(expr)` form (BNF A.1.3)

**BNF:**
```
list_of_ports ::= ( port { , port } )
port ::=
    [ port_expression ]
  | . port_identifier ( [ port_expression ] )

port_expression ::=
    port_reference
  | { port_reference { , port_reference } }

port_reference ::= port_identifier [ [ constant_range_expression ] ]
```

**Current state:** `module_ports()` in `grammar/item.rs:91-106`. When `self.at_dir()` is false and the current token is not an inline direction, it calls `self.name()` and pushes `ModulePort::Name`. It does NOT handle:
- `.port_id([port_expr])` — named external connection style
- `port_reference` with a range like `bus[3:0]`

**AST changes** — modify `ModulePort` in `ast/item.rs`:
```rust
// Replace existing enum:
#[derive(Debug, Clone)]
pub enum ModulePort {
    PortDecl(PortDecl),
    Name(Name),
    NamedExternal { port: Name, expr: Option<PortExpr> },  // ADD: .id([expr])
}

// New type:
#[derive(Debug, Clone)]
pub enum PortExpr {
    Ref { name: Name, range: Option<BitRange> },
    Concat(Vec<(Name, Option<BitRange>)>),
}
```

**Parser changes** — `grammar/item.rs`, modify `module_ports()`:
```rust
fn module_ports(&mut self) -> PResult<Vec<ModulePort>> {
    self.expect(&Tok::LParen)?;
    let mut ports = Vec::new();
    while !self.at(&Tok::RParen) {
        let start = self.span_start();
        let attrs = self.attrs()?;
        if self.at_dir() {
            ports.push(ModulePort::PortDecl(self.port_decl(attrs, start)?));
        } else if self.eat(&Tok::Dot) {
            // .port_id([port_expression])
            let port = self.name()?;
            self.expect(&Tok::LParen)?;
            let expr = if self.at(&Tok::RParen) { None } else { Some(self.port_expr()?) };
            self.expect(&Tok::RParen)?;
            ports.push(ModulePort::NamedExternal { port, expr });
        } else if self.at(&Tok::Comma) || self.at(&Tok::RParen) {
            // Empty port (blank entry)
            ports.push(ModulePort::Name(Name(String::new())));
        } else {
            // port_reference or port_expression
            ports.push(ModulePort::Name(self.name()?));
            // Ignore optional range on bare port names for now — they're informational
            self.skip_range();
        }
        if !self.eat(&Tok::Comma) { break; }
    }
    self.expect(&Tok::RParen)?;
    Ok(ports)
}

fn port_expr(&mut self) -> PResult<PortExpr> {
    if self.eat(&Tok::LBrace) {
        // Concatenation: { port_ref, port_ref, ... }
        let mut refs = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at_end() {
            let name = self.name()?;
            let range = self.parse_range()?;
            refs.push((name, range));
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RBrace)?;
        Ok(PortExpr::Concat(refs))
    } else {
        let name = self.name()?;
        let range = self.parse_range()?;
        Ok(PortExpr::Ref { name, range })
    }
}
```

**Test snippet:**
```verilog
// Named external port style:
module wrapper (.clk(sys_clk), .data(bus_data[7:0]));
endmodule

// Port reference with range:
module bus_mod (out_bus, in_a, in_b);
  output [7:0] out_bus;
  input in_a, in_b;
endmodule
```

---

### Gap 7: `function_declaration` — `automatic` keyword (BNF A.2.6)

**BNF:**
```
function_declaration ::=
    function [ automatic ] [ function_range_or_type ] function_identifier ;
    ...
```

**Current state:** `function()` in `grammar/item.rs:627`:
```rust
fn function(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    self.expect_kw("function")?;
    let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
    let name = self.name()?;
    ...
```
`automatic` is not consumed. If a file has `function automatic real f;`, the parser tries to parse `automatic` as the type (via `is_type_kw()` Ident-Ident heuristic), finds no `=` and fails.

**AST changes** — add field to `Function` in `ast/item.rs`:
```rust
#[derive(Debug, Clone)]
pub struct Function {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub automatic: bool,   // ADD
    pub is_analog: bool,   // ADD: was `analog function` vs plain `function`
    pub ty: Option<Type>,
    pub name: Name,
    pub items: Vec<FunctionItem>,
}
```

**Note:** The existing code sets `is_analog` implicitly by being called from the `analog` branch; track it explicitly so downstream consumers can distinguish.

**Parser changes** — `grammar/item.rs`, modify `function()` at line 627:
```rust
fn function(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    let is_analog = self.eat_kw("analog");  // consume 'analog' if present (called from analog())
    self.expect_kw("function")?;
    let automatic = self.eat_kw("automatic");  // ADD
    let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
    let name = self.name()?;
    // ... rest unchanged
    Ok(ModuleItem::Function(Function {
        attrs, automatic, is_analog, ty, name, items,
        span: Span { start, end: self.prev_end() },
    }))
}
```

Also adjust `analog()` at line 616 to not double-consume `analog`:
```rust
fn analog(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    self.expect_kw("analog")?;
    if self.at_kw("function") {
        // function() will no longer consume 'analog' since we just did
        return self.function_no_analog_prefix(attrs, start);
    }
    // ... rest unchanged
}
```

Or simpler: keep the existing call path, just add `automatic` consumption inside `function()` after `expect_kw("function")`.

**Minimal fix** (3-line change to `function()` in `grammar/item.rs:628`):
```rust
fn function(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    self.expect_kw("function")?;
    let automatic = self.eat_kw("automatic");  // ADD THIS LINE
    let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
    // ... rest of function unchanged, just pass automatic to struct ...
```

**Test snippet:**
```verilog
module math;
  function automatic real clamp;
    input real val, lo, hi;
    clamp = (val < lo) ? lo : (val > hi ? hi : val);
  endfunction
endmodule
```

---

### Gap 8: `net_declaration` — `drive_strength`, `charge_strength`, `delay3` (BNF A.2.1.3)

**BNF:**
```
net_declaration ::=
    net_type [ discipline_identifier ] [ drive_strength ] [ signed ]
    [ delay3 ] list_of_net_identifiers ;
  | trireg [ charge_strength ] [ signed ] [ delay3 ] list_of_net_identifiers ;
  | ...

drive_strength ::=
    ( strength0 , strength1 ) | ( strength1 , strength0 )
  | ( strength0 , highz1 ) | ( strength1 , highz0 )
  | ( highz0 , strength1 ) | ( highz1 , strength0 )

strength0 ::= supply0 | strong0 | pull0 | weak0
strength1 ::= supply1 | strong1 | pull1 | weak1

charge_strength ::= ( small ) | ( medium ) | ( large )

delay3 ::=
    # delay_value
  | # ( mintypmax_expression [ , mintypmax_expression [ , mintypmax_expression ] ] )
```

**Current state:** `net_decl()` in `grammar/item.rs:259`:
```rust
fn net_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    let net_type = self.opt_net_type();
    let discipline = self.opt_discipline();
    let range = self.parse_range()?;
    let names = self.declarator_list()?;
    self.expect(&Tok::Semi)?;
    Ok(ModuleItem::NetDecl(NetDecl {
        attrs, net_type, drive_strength: None, charge_strength: None, delay: None,
        discipline, range, names,
        span: Span { start, end: self.prev_end() },
    }))
}
```
The `DriveStrength`, `ChargeStrength`, `Delay` AST types exist (in `ast/item.rs:287-310`). Parser never calls them.

**Parser changes** — `grammar/item.rs`, add helper functions and update `net_decl()`:

```rust
fn net_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    let net_type = self.opt_net_type();
    // Optional drive_strength or charge_strength (only if LParen follows)
    let (drive_strength, charge_strength) = self.opt_drive_or_charge_strength(net_type.as_ref())?;
    // Optional vectored/scalared (consume and discard — no AST field)
    self.eat_kw("vectored");
    self.eat_kw("scalared");
    // Optional signed
    self.opt_signed();
    let discipline = self.opt_discipline();
    let range = self.parse_range()?;
    // Optional delay3
    let delay = self.opt_delay()?;
    let names = self.declarator_list()?;
    self.expect(&Tok::Semi)?;
    Ok(ModuleItem::NetDecl(NetDecl {
        attrs, net_type, drive_strength, charge_strength, delay,
        discipline, range, names,
        span: Span { start, end: self.prev_end() },
    }))
}

/// Parse `(strength0, strength1)` or `(small)` etc. if present.
/// `net_type` is needed to disambiguate: `trireg` allows charge_strength, others allow drive_strength.
fn opt_drive_or_charge_strength(
    &mut self,
    net_type: Option<&NetType>,
) -> PResult<(Option<DriveStrength>, Option<ChargeStrength>)> {
    // Only parse if `(` is next AND the token after `(` looks like a strength keyword.
    if !self.at(&Tok::LParen) { return Ok((None, None)); }
    // Peek inside: is next token a strength keyword?
    let is_strength = matches!(self.peek_at(1), Some(Tok::Ident(s)) if 
        matches!(s.as_str(), "supply0"|"strong0"|"pull0"|"weak0"|
                              "supply1"|"strong1"|"pull1"|"weak1"|
                              "highz0"|"highz1"|"small"|"medium"|"large")
    );
    if !is_strength { return Ok((None, None)); }

    self.expect(&Tok::LParen)?;
    let s0_str = self.ident()?;

    // charge_strength: (small), (medium), (large)
    if matches!(s0_str.as_str(), "small"|"medium"|"large") {
        self.expect(&Tok::RParen)?;
        let cs = match s0_str.as_str() {
            "small" => ChargeStrength::Small,
            "medium" => ChargeStrength::Medium,
            _ => ChargeStrength::Large,
        };
        return Ok((None, Some(cs)));
    }

    // drive_strength: (s0, s1)
    self.expect(&Tok::Comma)?;
    let s1_str = self.ident()?;
    self.expect(&Tok::RParen)?;
    let strength0 = parse_strength(&s0_str)?;
    let strength1 = parse_strength(&s1_str)?;
    Ok((Some(DriveStrength { strength0, strength1 }), None))
}

/// Parse optional `#delay` or `#(expr)` or `#(e,e,e)`.
fn opt_delay(&mut self) -> PResult<Option<Delay>> {
    if !self.eat(&Tok::Hash) { return Ok(None); }
    if self.eat(&Tok::LParen) {
        let e1 = self.expr()?;
        let e1 = self.opt_mintypmax(e1)?;
        if self.eat(&Tok::Comma) {
            let e2 = self.expr()?;
            let e2 = self.opt_mintypmax(e2)?;
            if self.eat(&Tok::Comma) {
                let e3 = self.expr()?;
                let e3 = self.opt_mintypmax(e3)?;
                self.expect(&Tok::RParen)?;
                Ok(Some(Delay::Paren3(e1, e2, e3)))
            } else {
                self.expect(&Tok::RParen)?;
                Ok(Some(Delay::Paren2(e1, e2)))
            }
        } else {
            self.expect(&Tok::RParen)?;
            Ok(Some(Delay::Paren1(e1)))
        }
    } else {
        Ok(Some(Delay::Single(self.expr()?)))
    }
}

fn parse_strength(s: &str) -> PResult<Strength> {
    match s {
        "supply0" => Ok(Strength::Supply0), "strong0" => Ok(Strength::Strong0),
        "pull0"   => Ok(Strength::Pull0),   "weak0"   => Ok(Strength::Weak0),
        "supply1" => Ok(Strength::Supply1), "strong1" => Ok(Strength::Strong1),
        "pull1"   => Ok(Strength::Pull1),   "weak1"   => Ok(Strength::Weak1),
        "highz0"  => Ok(Strength::Highz0),  "highz1"  => Ok(Strength::Highz1),
        other => Err(format!("unknown strength: {other}")),
    }
}
```

Also apply the same `opt_delay()` logic to `continuous_assign()` in `grammar/item.rs:384`.

**Test snippet:**
```verilog
module driver;
  wire (strong0, weak1) #(1e-9, 2e-9) data;
  trireg (small) #5 cap_node;
  assign #(1.5e-9) out = a & b;
endmodule
```

---

### Gap 9: `connectrules` — `connect_port_overrides` + `parameter_value_assignment` (BNF A.1.8)

**BNF:**
```
connect_insertion ::=
    connect connectmodule_identifier [ connect_mode ]
    [ parameter_value_assignment ] [ connect_port_overrides ] ;

connect_port_overrides ::=
    discipline_identifier , discipline_identifier
  | input discipline_identifier , output discipline_identifier
  | output discipline_identifier , input discipline_identifier
  | inout discipline_identifier , inout discipline_identifier
```

**Current state:** `connectrules_decl()` at `grammar/item.rs:180`:
```rust
// After consuming module + optional mode:
while !self.eat(&Tok::Semi) && !self.at_end() { self.bump(); }
```
It silently skips everything to `;`.

**AST changes** — replace empty `ConnectPortOverrides` in `ast/item.rs`:
```rust
#[derive(Debug, Clone)]
pub struct ConnectPortOverrides {
    pub input_disc: Option<Name>,   // discipline on the input side
    pub output_disc: Option<Name>,  // discipline on the output side
}
```

**Parser changes** — `grammar/item.rs`, replace the skip-to-semi in the `Insertion` branch:
```rust
// REPLACE: while !self.eat(&Tok::Semi) && !self.at_end() { self.bump(); }
// WITH:
let params = if self.at(&Tok::Hash) {
    // parameter_value_assignment #(...)
    self.eat(&Tok::Hash);
    self.expect(&Tok::LParen)?;
    let mut ps = Vec::new();
    while !self.at(&Tok::RParen) && !self.at_end() {
        if self.eat(&Tok::Dot) {
            let name = self.name()?;
            self.expect(&Tok::LParen)?;
            let expr = self.expr()?;
            self.expect(&Tok::RParen)?;
            ps.push(ParamAssignment::Named { param: name, expr });
        } else {
            ps.push(ParamAssignment::Ordered(self.expr()?));
        }
        if !self.eat(&Tok::Comma) { break; }
    }
    self.expect(&Tok::RParen)?;
    ps
} else {
    Vec::new()
};

let port_overrides = if !self.at(&Tok::Semi) {
    // connect_port_overrides: [input/output/inout] discipline, [input/output/inout] discipline
    let (first_dir, first_disc) = if self.at_dir() {
        (Some(self.direction()?), self.name()?)
    } else {
        (None, self.name()?)
    };
    self.expect(&Tok::Comma)?;
    let (_second_dir, second_disc) = if self.at_dir() {
        (Some(self.direction()?), self.name()?)
    } else {
        (None, self.name()?)
    };
    // Determine input/output discipline from direction
    let (input_disc, output_disc) = match first_dir {
        Some(Direction::Output) => (Some(second_disc), Some(first_disc)),
        _ => (Some(first_disc), Some(second_disc)),
    };
    Some(ConnectPortOverrides { input_disc, output_disc })
} else {
    None
};

self.expect(&Tok::Semi)?;
items.push(ConnectrulesItem::Insertion { module, mode, params, port_overrides });
```

**Test snippet:**
```verilog
connectrules cmos_to_ams;
  connect cmos_buf merged #(.vdd(1.8)) electrical, logic;
  connect electrical resolveto exclude;
endconnectrules
```

---

### Gap 10: `udp_declaration` — `primitive` keyword (BNF A.5)

**BNF:**
```
udp_declaration ::=
    { attribute_instance } primitive udp_identifier ( udp_port_list ) ;
    udp_port_declaration { udp_port_declaration }
    udp_body
    endprimitive

udp_body ::= combinational_body | sequential_body
combinational_body ::= table combinational_entry { combinational_entry } endtable
combinational_entry ::= level_input_list : output_symbol ;
sequential_body ::= [ udp_initial_statement ] table sequential_entry ... endtable
```

**Current state:** `Item::Primitive(PrimitiveDecl)` exists at `ast/mod.rs:34`. `PrimitiveDecl` is an empty stub. `item()` at `grammar/item.rs:8` never dispatches on `primitive`.

**AST changes** — extend `PrimitiveDecl` in `ast/item.rs`:
```rust
// Replace empty struct:
#[derive(Debug, Clone)]
pub struct PrimitiveDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub ports: Vec<Name>,
    pub port_decls: Vec<PortDecl>,
    pub body: UdpBody,
}

#[derive(Debug, Clone)]
pub enum UdpBody {
    Combinational(Vec<UdpEntry>),
    Sequential { initial: Option<(Name, String)>, entries: Vec<UdpEntry> },
}

#[derive(Debug, Clone)]
pub struct UdpEntry {
    pub inputs: Vec<String>,  // level/edge symbols as strings
    pub current_state: Option<String>,  // None for combinational
    pub next_state: String,
}
```

**Parser changes** — `grammar/item.rs`:

Step 1 — Add dispatch in `item()` at line 23 (before the final `Err`):
```rust
} else if self.eat_kw("primitive") {
    Ok(Item::Primitive(self.primitive_decl(attrs, start)?))
} else {
    Err(...)
}
```

Step 2 — Add `primitive_decl()`:
```rust
fn primitive_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<PrimitiveDecl> {
    let name = self.name()?;
    self.expect(&Tok::LParen)?;
    let mut ports = Vec::new();
    while !self.at(&Tok::RParen) && !self.at_end() {
        ports.push(self.name()?);
        if !self.eat(&Tok::Comma) { break; }
    }
    self.expect(&Tok::RParen)?;
    self.expect(&Tok::Semi)?;

    // Port declarations
    let mut port_decls = Vec::new();
    while self.at_dir() {
        let pd_start = self.span_start();
        let pd_attrs = self.attrs()?;
        let pd = self.port_decl(pd_attrs, pd_start)?;
        self.expect(&Tok::Semi)?;
        port_decls.push(pd);
    }

    // UDP body: optional `initial output_port = init_val ;` then `table ... endtable`
    let mut initial_stmt = None;
    if self.eat_kw("initial") {
        let out_name = self.name()?;
        self.expect(&Tok::Assign)?;
        // init_val: 1'b0 | 1'b1 | 1'bx | ... | 0 | 1
        let init_val = self.ident()?;  // crude: accept any ident/number token as string
        self.expect(&Tok::Semi)?;
        initial_stmt = Some((out_name, init_val));
    }

    self.expect_kw("table")?;
    let mut entries = Vec::new();
    while !self.at_kw("endtable") && !self.at_end() {
        entries.push(self.udp_entry(initial_stmt.is_some())?);
    }
    self.expect_kw("endtable")?;

    let body = if initial_stmt.is_some() {
        UdpBody::Sequential { initial: initial_stmt, entries }
    } else {
        UdpBody::Combinational(entries)
    };

    self.expect_kw("endprimitive")?;
    Ok(PrimitiveDecl {
        span: Span { start, end: self.prev_end() },
        attrs, name, ports, port_decls, body,
    })
}

fn udp_entry(&mut self, sequential: bool) -> PResult<UdpEntry> {
    // Table entries are sequences of 0/1/x/X/?/b/B/r/R/f/F/p/P/n/N/* separated by spaces.
    // We collect tokens until `;`, parsing them as strings.
    // Format: inputs : [current_state :] next_state ;
    let mut tokens = Vec::new();
    while !self.at(&Tok::Semi) && !self.at_end() {
        // Each symbol is an Int token or an Ident token (for x,X,?,b,B,r,R,f,F,*)
        match self.peek() {
            Some(Tok::Ident(s)) => { tokens.push(s.clone()); self.bump(); }
            Some(Tok::Int(s)) => { tokens.push(s.clone()); self.bump(); }
            Some(Tok::Star) => { tokens.push("*".to_string()); self.bump(); }
            Some(Tok::Colon) => { tokens.push(":".to_string()); self.bump(); }
            Some(Tok::Minus) => { tokens.push("-".to_string()); self.bump(); }
            _ => { self.bump(); }  // skip unexpected
        }
    }
    self.expect(&Tok::Semi)?;
    // Split by ":"
    let colon_positions: Vec<usize> = tokens.iter().enumerate()
        .filter(|(_, t)| t.as_str() == ":")
        .map(|(i, _)| i)
        .collect();
    if sequential && colon_positions.len() >= 2 {
        let c1 = colon_positions[0];
        let c2 = colon_positions[1];
        let inputs = tokens[..c1].to_vec();
        let current_state = Some(tokens[c1+1..c2].join(""));
        let next_state = tokens[c2+1..].join("");
        Ok(UdpEntry { inputs, current_state, next_state })
    } else if let Some(&c) = colon_positions.last() {
        let inputs = tokens[..c].to_vec();
        let next_state = tokens[c+1..].join("");
        Ok(UdpEntry { inputs, current_state: None, next_state })
    } else {
        Err("malformed UDP table entry".to_string())
    }
}
```

**Test snippet:**
```verilog
primitive mux21 (out, sel, a, b);
  output out; input sel, a, b;
  table
    0  0  ? : 0;
    0  1  ? : 1;
    1  ?  0 : 0;
    1  ?  1 : 1;
  endtable
endprimitive
```

---

### Gap 11: `config_declaration` — real content (BNF A.1.5)

**BNF:**
```
config_declaration ::=
    config config_identifier ;
    design_statement
    { config_rule_statement }
    endconfig

design_statement ::= design { [ library_identifier . ] cell_identifier } ;

config_rule_statement ::=
    default_clause liblist_clause ;
  | inst_clause liblist_clause ;
  | inst_clause use_clause ;
  | cell_clause liblist_clause ;
  | cell_clause use_clause ;
```

**Current state:** `config_decl()` at `grammar/item.rs:203` skips all content:
```rust
fn config_decl(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ConfigDecl> {
    self.expect_kw("config")?;
    while !self.at_kw("endconfig") && !self.at_end() { self.bump(); }
    self.expect_kw("endconfig")?;
    Ok(ConfigDecl { span: Span { start, end: self.prev_end() } })
}
```

**AST changes** — replace `ConfigDecl` in `ast/item.rs`:
```rust
#[derive(Debug, Clone)]
pub struct ConfigDecl {
    pub span: Span,
    pub name: Name,
    pub design: Vec<ConfigCellRef>,
    pub rules: Vec<ConfigRule>,
}

#[derive(Debug, Clone)]
pub struct ConfigCellRef {
    pub library: Option<Name>,
    pub cell: Name,
}

#[derive(Debug, Clone)]
pub enum ConfigRule {
    Default(LiblistOrUse),
    Inst { path: Vec<Name>, clause: LiblistOrUse },
    Cell { cell_ref: ConfigCellRef, clause: LiblistOrUse },
}

#[derive(Debug, Clone)]
pub enum LiblistOrUse {
    Liblist(Vec<Name>),
    Use { cell_ref: ConfigCellRef, config: bool },
}
```

**Parser changes** — replace `config_decl()`:
```rust
fn config_decl(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ConfigDecl> {
    self.expect_kw("config")?;
    let name = self.name()?;
    self.expect(&Tok::Semi)?;

    // design_statement
    self.expect_kw("design")?;
    let mut design = Vec::new();
    while !self.at(&Tok::Semi) && !self.at_end() {
        design.push(self.config_cell_ref()?);
    }
    self.expect(&Tok::Semi)?;

    let mut rules = Vec::new();
    while !self.at_kw("endconfig") && !self.at_end() {
        rules.push(self.config_rule()?);
    }
    self.expect_kw("endconfig")?;
    Ok(ConfigDecl { span: Span { start, end: self.prev_end() }, name, design, rules })
}

fn config_cell_ref(&mut self) -> PResult<ConfigCellRef> {
    let first = self.name()?;
    if self.eat(&Tok::Dot) {
        let cell = self.name()?;
        Ok(ConfigCellRef { library: Some(first), cell })
    } else {
        Ok(ConfigCellRef { library: None, cell: first })
    }
}

fn config_rule(&mut self) -> PResult<ConfigRule> {
    if self.eat_kw("default") {
        let clause = self.liblist_or_use()?;
        self.expect(&Tok::Semi)?;
        Ok(ConfigRule::Default(clause))
    } else if self.eat_kw("instance") {
        let mut path = vec![self.name()?];
        while self.eat(&Tok::Dot) { path.push(self.name()?); }
        let clause = self.liblist_or_use()?;
        self.expect(&Tok::Semi)?;
        Ok(ConfigRule::Inst { path, clause })
    } else {
        self.expect_kw("cell")?;
        let cell_ref = self.config_cell_ref()?;
        let clause = self.liblist_or_use()?;
        self.expect(&Tok::Semi)?;
        Ok(ConfigRule::Cell { cell_ref, clause })
    }
}

fn liblist_or_use(&mut self) -> PResult<LiblistOrUse> {
    if self.eat_kw("liblist") {
        let mut libs = Vec::new();
        while matches!(self.peek(), Some(Tok::Ident(_))) 
            && !self.at_any_kw(&["default","instance","cell","endconfig"]) 
        {
            libs.push(self.name()?);
        }
        Ok(LiblistOrUse::Liblist(libs))
    } else {
        self.expect_kw("use")?;
        let cell_ref = self.config_cell_ref()?;
        let config = self.eat(&Tok::Colon) && self.eat_kw("config");
        Ok(LiblistOrUse::Use { cell_ref, config })
    }
}
```

**Test snippet:**
```verilog
config top_cfg;
  design work.top;
  default liblist work std;
  instance top.u1 use fast_lib.nand2;
  cell work.dff liblist fast_lib;
endconfig
```

---

## 3. Priority Ordering

### P1 — Breaks Valid AMS Files (implement first)

| # | Gap | Why P1 |
|---|---|---|
| 1 | `task_declaration` | Any testbench or behavioral model with `task ... endtask` fails |
| 2 | `module_parameter_port_list` | `module Foo #(parameter real R=1k)` is standard parameterized AMS — common in all cell libraries |
| 3 | `specparam_declaration` dispatch | `specparam tpd = 3ns` fails; appears in SV timing models |
| 4 | `specify_block` dispatch | `specify ... endspecify` in timing models causes parse failure |
| 5 | `gate_instantiation` routing | `and g1(y,a,b)` parses as module instantiation; downstream netlisting produces wrong topology |

### P2 — Missing Standard Features

| # | Gap | Why P2 |
|---|---|---|
| 6 | `function automatic` | `automatic` in function signatures is standard SV; common in verification |
| 7 | `drive_strength` / `charge_strength` / `delay3` | SPICE-targeted netlists use drive strengths; delays appear in gate-level models |
| 8 | `udp_declaration` | Primitive user-defined logic tables; less common in AMS but part of the standard |
| 9 | `connectrules connect_port_overrides` | Needed for discipline bridging rules |
| 10 | `config_declaration` content | Config blocks needed for library resolution |

### P3 — Edge Cases / Completeness

| # | Gap | Why P3 |
|---|---|---|
| 11 | `list_of_ports` `.id(expr)` form | Named external port style; uncommon in AMS but part of LRM |
| 12 | `port_reference` with range | `port bus[3:0]` in old-style port list |
| 13 | `continuous_assign` drive_strength + delay | Most AMS assign doesn't use these |
| 14 | `task_enable` semantic distinction | Currently works as `ExprStmt`; semantic check only |

---

## 4. Quick Reference: Parser API Patterns

Use these patterns exactly when implementing new rules:

### Keyword dispatch (item-level)
```rust
// In module_item() or item():
if self.eat_kw("keyword") { return self.my_handler(attrs, start); }
// OR for non-consuming dispatch:
if self.at_kw("keyword") { return self.my_handler(attrs, start); }
```

### Consuming a keyword
```rust
self.eat_kw("kw")      // returns bool, does NOT error if absent
self.expect_kw("kw")?  // errors if absent
```

### Token inspection
```rust
self.peek()            // Option<&Tok> at current position
self.peek_at(n)        // Option<&Tok> at position+n
self.at(&Tok::Semi)    // bool: current token == Semi
self.at_kw("kw")       // bool: current token == Ident("kw")
self.at_any_kw(&[...]) // bool: current token in keyword list
self.at_dir()          // bool: current token is input/output/inout/terminal
```

### Consuming tokens
```rust
self.bump()            // consume and return current token
self.eat(&Tok::Comma)  // consume if match, return bool
self.expect(&Tok::Semi)?  // consume or error
```

### Lookahead helpers
```rust
self.is_type_kw()       // true if current looks like a type keyword (including Ident Ident heuristic)
self.at_primitive_type_kw()  // only real primitive keywords (no heuristic)
self.at_stmt_kw()       // true if current starts a statement
self.assign_before_semi()    // scan ahead: is there = before ;?
self.idx_after_range(pos)    // skip balanced [...] from pos, return new pos
```

### Parsing common constructs
```rust
let name = self.name()?              // consume Ident, return Name
let path = self.path()?             // consume dotted path
let expr = self.expr()?             // full Pratt expression
let range = self.parse_range()?    // Option<BitRange>: consumes [msb:lsb] if present
let attrs = self.attrs()?          // consume (* ... *) attr instances
self.skip_range()                  // consume [range] and discard
self.opt_signed()                  // consume `signed` if present, return bool
self.opt_net_type()                // consume net type keyword if present
self.opt_discipline()              // consume discipline (Ident before another Ident)
```

### Building spans
```rust
let start = self.span_start();   // capture before parsing
// ... parse ...
Span { start, end: self.prev_end() }  // build span after parsing
```

### Typical handler skeleton
```rust
fn my_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    self.expect_kw("mykeyword")?;
    let name = self.name()?;
    // ... parse children ...
    self.expect(&Tok::Semi)?;
    Ok(ModuleItem::MyDecl(MyDecl {
        span: Span { start, end: self.prev_end() },
        attrs, name, /* ... */
    }))
}
```

---

## 5. Test Snippets Summary

One file that tests all P1 gaps together:

```verilog
// Verilog-AMS file that exercises every P1 gap.
// This file must parse without error after all P1 fixes are applied.

// Gap 2: module with parameter port list
module amp #(parameter real gain = 10.0, parameter real bw = 1e6) (in, out);
  input electrical in;
  output electrical out;
  // Gap 3: specparam
  specparam tpd = 2.5e-9 : 3e-9 : 4e-9;
  // Gap 4: specify block
  specify
    specparam trise = 1e-9;
    (in => out) = tpd;
  endspecify
  // Gap 5: gate instantiation
  wire w;
  and  g1(w, in, in);
  not  inv1(out, w);
  // Gap 1: task
  task automatic bias_check;
    input real v;
    if (v < 0.0)
      $warning("negative bias %g", v);
  endtask
  analog V(out) <+ gain * V(in);
endmodule

// Gap 10: UDP (P2, but shown here)
primitive inv_udp (out, in);
  output out; input in;
  table
    0 : 1;
    1 : 0;
    x : x;
  endtable
endprimitive
```
