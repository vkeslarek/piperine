# Implementing SPICE Components in piperine-ngspice

Step-by-step recipe for adding every SPICE device to
`crates/piperine-ngspice/src/hardware.rs` and registering it in `NgspicePlugin`.
No design decisions needed — fill in the SPICE line template for each device.

---

## Pattern

Every SPICE component needs three things:

### 1. A `HardwareDefinition` struct

```rust
#[derive(Debug)]
pub struct SpiceFoo;

impl HardwareDefinition for SpiceFoo {
    fn name(&self) -> &str { "foo" }  // ← must match the name in ngspice.ppr exactly
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    // Model-based devices only (d, npn, pnp, nmos, pmos, jfet_n, jfet_p,
    //   mesfet_n, mesfet_p, vdmos, vsw, isw, ltra, urc):
    fn spice_model_type(&self) -> Option<&'static str> { Some("TYPE") }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        // 1. Extract nets with require_net()
        // 2. Extract params with require_parameter() or optional pattern
        // 3. Return Box::new(SpiceFooInstance { ... })
    }
}
```

### 2. A `HardwareInstance` struct

```rust
#[derive(Debug)]
struct SpiceFooInstance { name: String, /* nets and params */ }

impl HardwareInstance for SpiceFooInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} ...", spice_name('X', &self.name), ...)]
    }
}
```

### 3. Register in `NgspicePlugin::register_hardware()`

```rust
registry.register(Box::new(SpiceFoo));
```

---

## Helpers in hardware.rs

```rust
// Required net — errors if port not connected.
fn require_net<'a>(connections: &'a ConnectionMap, port: &str, instance: &str)
    -> Result<&'a str, ElaborationError>

// Required f64 parameter — errors if missing or not a number.
fn require_parameter(parameters: &ParameterMap, name: &str, instance: &str)
    -> Result<f64, ElaborationError>

// Required string parameter — for model names, element references, etc.
fn require_string_parameter(parameters: &ParameterMap, name: &str, instance: &str)
    -> Result<String, ElaborationError>

// Optional real — safe fallback.
fn get_parameter_or(parameters: &ParameterMap, param: &str, default: f64) -> f64

// Optional string — safe fallback.
fn get_string_parameter_or(parameters: &ParameterMap, param: &str, default: &str) -> String

// SPICE element name prefix — avoids doubling: "V1" stays "V1", "myv" → "Vmyv".
fn spice_name(prefix: char, name: &str) -> String
```

---

## Device-by-device guide

### Inductor — `ind` ✅ DONE

Already implemented as `SpiceInductor`. Also implements `spice_instance_prefix() → Some('L')`
so that `parameter ref` resolution works correctly for `mutual`.

### Mutual Inductor — `mutual` ✅ DONE

Already implemented as `SpiceMutual`. Uses `parameter string inductor1, inductor2` — the user passes the SPICE element names directly as strings. The SPICE element name for an inductor includes the `L` prefix.

Usage in Piperine source:
```verilog
module transformer(inout p1, n1, p2, n2);
    ind #(.l(1e-6)) La(.p(p1), .n(n1));
    ind #(.l(2e-6)) Lb(.p(p2), .n(n2));
    // Pass SPICE element names (La, Lb) as strings:
    mutual #(.inductor1("La"), .inductor2("Lb"), .k(0.85)) K1();
endmodule
```

SPICE line: `K<name> La Lb [k]`

Note: `parameter ref` infrastructure exists in the parser and elaborator but `mutual` doesn't use it — it uses `parameter string` instead for simplicity.

---

### Pulse Voltage Source — `vpulse`

```rust
// SPICE: V{name} {p} {n} PULSE({v0} {v1} {td} {tr} {tf} {pw} {per})
// No model card.
// fn name() → "vpulse"

fn instantiate(...) {
    let p   = require_net(connections, "p", instance_name)?.to_string();
    let n   = require_net(connections, "n", instance_name)?.to_string();
    let v0  = require_parameter(parameters, "v0", instance_name)?;
    let v1  = require_parameter(parameters, "v1", instance_name)?;
    let td  = parameters.get("td").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let tr  = require_parameter(parameters, "tr", instance_name)?;
    let tf  = require_parameter(parameters, "tf", instance_name)?;
    let pw  = require_parameter(parameters, "pw", instance_name)?;
    let per = require_parameter(parameters, "per", instance_name)?;
    Ok(Box::new(SpiceVpulseInstance { name: instance_name.to_string(), p, n, v0, v1, td, tr, tf, pw, per }))
}

fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} PULSE({} {} {} {} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.v0, self.v1, self.td, self.tr, self.tf, self.pw, self.per)]
}
```

Repeat same pattern for `ipulse` — replace `V` prefix with `I`, `v0`/`v1` with `i0`/`i1`.

### Sinusoidal Voltage Source — `vsin`

```rust
// SPICE: V{name} {p} {n} SIN({vo} {va} {freq} {td} {theta} {phi})
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} SIN({} {} {} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.vo, self.va, self.freq, self.td, self.theta, self.phi)]
}
```

Repeat for `isin` (prefix `I`, param names `io`/`ia` instead of `vo`/`va`).

### Exponential Voltage Source — `vexp`

```rust
// SPICE: V{name} {p} {n} EXP({v1} {v2} {td1} {tau1} {td2} {tau2})
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} EXP({} {} {} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.v1, self.v2, self.td1, self.tau1, self.td2, self.tau2)]
}
```

Repeat for `iexp` (prefix `I`, `i1`/`i2` instead of `v1`/`v2`).

### PWL Voltage Source — `vpwl`

```rust
// SPICE: V{name} {p} {n} PWL({points})
// points: parameter string, e.g. "0 0 1n 1 2n 0"
fn instantiate(...) {
    let p      = require_net(connections, "p", instance_name)?.to_string();
    let n      = require_net(connections, "n", instance_name)?.to_string();
    let points = require_string_parameter(parameters, "points", instance_name)?;
    Ok(Box::new(SpiceVpwlInstance { name: instance_name.to_string(), p, n, points }))
}
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} PWL({})", spice_name('V', &self.name), self.p, self.n, self.points)]
}
```

Repeat for `ipwl` (prefix `I`).

### SFFM Voltage Source — `vsffm`

```rust
// SPICE: V{name} {p} {n} SFFM({vo} {va} {fc} {mdi} {fs} {phasec} {phases})
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} SFFM({} {} {} {} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.vo, self.va, self.fc, self.mdi, self.fs, self.phasec, self.phases)]
}
```

### AM Voltage Source — `vam`

```rust
// SPICE: V{name} {p} {n} AM({sa} {fc} {fm} {td} {phases})
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} AM({} {} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.sa, self.fc, self.fm, self.td, self.phases)]
}
```

### TRNOISE Voltage Source — `vnoise`

```rust
// SPICE: V{name} {p} {n} TRNOISE({na} {nt} {nalpha} {namp})
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} TRNOISE({} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.na, self.nt, self.nalpha, self.namp)]
}
```

### TRRANDOM Voltage Source — `vrandom`

```rust
// SPICE: V{name} {p} {n} TRRANDOM({rtype} {ts} {td} {param1} {param2})
// rtype: i64 — use parameters.get("rtype").and_then(|v| v.as_f64()).unwrap_or(1.0) as i64
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} TRRANDOM({} {} {} {} {})",
        spice_name('V', &self.name),
        self.p, self.n,
        self.rtype, self.ts, self.td, self.param1, self.param2)]
}
```

### VCVS — `vcvs`

```rust
// SPICE: E{name} {p} {n} {cp} {cn} {gain}
// Ports: p, n, cp, cn
fn instantiate(...) {
    let p    = require_net(connections, "p", instance_name)?.to_string();
    let n    = require_net(connections, "n", instance_name)?.to_string();
    let cp   = require_net(connections, "cp", instance_name)?.to_string();
    let cn   = require_net(connections, "cn", instance_name)?.to_string();
    let gain = parameters.get("gain").and_then(|v| v.as_f64()).unwrap_or(1.0);
    ...
}
fn spice_lines(&self) -> Vec<String> {
    vec![format!("E{} {} {} {} {} {}", self.name, self.p, self.n, self.cp, self.cn, self.gain)]
}
```

### VCCS — `vccs`

```rust
// SPICE: G{name} {p} {n} {cp} {cn} {gm}
fn spice_lines(&self) -> Vec<String> {
    vec![format!("G{} {} {} {} {} {}", self.name, self.p, self.n, self.cp, self.cn, self.gm)]
}
```

### CCVS — `ccvs`

```rust
// SPICE: H{name} {p} {n} {vsrc} {transres}
// vsrc is a parameter string — name of the sense voltage source instance.
fn instantiate(...) {
    let p        = require_net(connections, "p", instance_name)?.to_string();
    let n        = require_net(connections, "n", instance_name)?.to_string();
    let vsrc     = require_string_parameter(parameters, "vsrc", instance_name)?;
    let transres = parameters.get("transres").and_then(|v| v.as_f64()).unwrap_or(1.0);
    ...
}
fn spice_lines(&self) -> Vec<String> {
    vec![format!("H{} {} {} {} {}", self.name, self.p, self.n, self.vsrc, self.transres)]
}
```

### CCCS — `cccs`

```rust
// SPICE: F{name} {p} {n} {vsrc} {gain}
fn spice_lines(&self) -> Vec<String> {
    vec![format!("F{} {} {} {} {}", self.name, self.p, self.n, self.vsrc, self.gain)]
}
```

### Voltage-Controlled Switch — `vsw`

```rust
// SPICE: S{name} {p} {n} {cp} {cn} {model}
// spice_model_type: Some("SW")
// Ports: p, n, cp, cn
// model comes from paramset
fn spice_model_type(&self) -> Option<&'static str> { Some("SW") }
fn spice_lines(&self) -> Vec<String> {
    vec![format!("S{} {} {} {} {} {}",
        self.name, self.p, self.n, self.cp, self.cn, self.model)]
}
```

### Current-Controlled Switch — `isw`

```rust
// SPICE: W{name} {p} {n} {vsrc} {model}
// spice_model_type: Some("CSW")
fn spice_model_type(&self) -> Option<&'static str> { Some("CSW") }
fn spice_lines(&self) -> Vec<String> {
    vec![format!("W{} {} {} {} {}", self.name, self.p, self.n, self.vsrc, self.model)]
}
```

---

## Semiconductor Devices (model-based)

All use `paramset` for the model card. The `model` parameter (string) is
injected into `parameters` by the elaborator before calling `instantiate`.

### Diode — `d`

```rust
// SPICE: D{name} {a} {c} {model} [AREA={area}]
// spice_model_type: Some("D")
// Ports: a, c
fn spice_model_type(&self) -> Option<&'static str> { Some("D") }
fn instantiate(...) {
    let a     = require_net(connections, "a", instance_name)?.to_string();
    let c     = require_net(connections, "c", instance_name)?.to_string();
    let model = require_string_parameter(parameters, "model", instance_name)?;
    let area  = parameters.get("area").and_then(|v| v.as_f64()).unwrap_or(1.0);
    ...
}
fn spice_lines(&self) -> Vec<String> {
    let mut line = format!("D{} {} {} {}", self.name, self.a, self.c, self.model);
    if (self.area - 1.0).abs() > 1e-15 { line.push_str(&format!(" AREA={}", self.area)); }
    vec![line]
}
```

Example paramset:
```
`include "ngspice.ppr"

paramset d1n4148 d;
    .model = "d1n4148";
    .is    = 2.52e-9;
    .n     = 1.752;
    .bv    = 75;
endparamset
```

### NPN BJT — `npn` / PNP — `pnp`

```rust
// SPICE: Q{name} {c} {b} {e} {model} [AREA={area}]
// spice_model_type: Some("NPN") for npn, Some("PNP") for pnp
// Ports: c, b, e
fn spice_lines(&self) -> Vec<String> {
    let mut line = format!("Q{} {} {} {} {}", self.name, self.c, self.b, self.e, self.model);
    if (self.area - 1.0).abs() > 1e-15 { line.push_str(&format!(" AREA={}", self.area)); }
    vec![line]
}
```

Two separate structs `SpiceNpn` and `SpicePnp`, identical except:
- `fn name() → "npn"` or `"pnp"`
- `fn spice_model_type() → Some("NPN")` or `Some("PNP")`

### NPN/PNP with substrate — `npn4` / `pnp4`

Same as above but adds `sub` port:
```rust
// SPICE: Q{name} {c} {b} {e} {sub} {model}
// Ports: c, b, e, sub
fn spice_lines(&self) -> Vec<String> {
    vec![format!("Q{} {} {} {} {} {}", self.name, self.c, self.b, self.e, self.sub, self.model)]
}
```

### N-channel MOSFET — `nmos` / P-channel — `pmos`

```rust
// SPICE: M{name} {d} {g} {s} {b} {model} W={w} L={l} [NRD={nrd} NRS={nrs}]
// spice_model_type: Some("NMOS") or Some("PMOS")
// Ports: d, g, s, b
// Params: model, w (required), l (required), nrd, nrs, temp
fn spice_lines(&self) -> Vec<String> {
    let mut line = format!("M{} {} {} {} {} {} W={} L={}",
        self.name, self.d, self.g, self.s, self.b, self.model, self.w, self.l);
    if self.nrd != 0.0 { line.push_str(&format!(" NRD={}", self.nrd)); }
    if self.nrs != 0.0 { line.push_str(&format!(" NRS={}", self.nrs)); }
    vec![line]
}
```

Two structs: `SpiceNmos` (`"nmos"`, `"NMOS"`) and `SpicePmos` (`"pmos"`, `"PMOS"`).

### N-type JFET — `jfet_n` / P-type — `jfet_p`

```rust
// SPICE: J{name} {d} {g} {s} {model} [AREA={area}]
// spice_model_type: Some("NJF") or Some("PJF")
// Ports: d, g, s
fn spice_lines(&self) -> Vec<String> {
    let mut line = format!("J{} {} {} {} {}", self.name, self.d, self.g, self.s, self.model);
    if (self.area - 1.0).abs() > 1e-15 { line.push_str(&format!(" AREA={}", self.area)); }
    vec![line]
}
```

### N-type MESFET — `mesfet_n` / P-type — `mesfet_p`

```rust
// SPICE: Z{name} {d} {g} {s} {model} [AREA={area}]
// spice_model_type: Some("NMF") or Some("PMF")
// Ports: d, g, s
fn spice_lines(&self) -> Vec<String> {
    let mut line = format!("Z{} {} {} {} {}", self.name, self.d, self.g, self.s, self.model);
    if (self.area - 1.0).abs() > 1e-15 { line.push_str(&format!(" AREA={}", self.area)); }
    vec![line]
}
```

### Power MOSFET — `vdmos`

```rust
// SPICE: M{name} {d} {g} {s} {model} W={w} L={l}
// spice_model_type: Some("VDMOS")
// Ports: d, g, s   (3-terminal, no body/bulk)
fn spice_model_type(&self) -> Option<&'static str> { Some("VDMOS") }
fn spice_lines(&self) -> Vec<String> {
    vec![format!("M{} {} {} {} {} W={} L={}",
        self.name, self.d, self.g, self.s, self.model, self.w, self.l)]
}
```

---

## Transmission Lines

### Ideal T-Line — `tline`

```rust
// SPICE: T{name} {ap} {an} {bp} {bn} Z0={z0} TD={td}
// No model card.
// Ports: ap, an, bp, bn
// Params: z0 (default 50.0), td (default 1e-9)
fn spice_lines(&self) -> Vec<String> {
    vec![format!("{} {} {} {} {} Z0={} TD={}",
        spice_name('T', &self.name),
        self.ap, self.an, self.bp, self.bn, self.z0, self.td)]
}
```

### Lossy T-Line — `ltra`

```rust
// SPICE: O{name} {ap} {an} {bp} {bn} {model}
// spice_model_type: Some("LTRA")
// Ports: ap, an, bp, bn
fn spice_model_type(&self) -> Option<&'static str> { Some("LTRA") }
fn spice_lines(&self) -> Vec<String> {
    vec![format!("O{} {} {} {} {} {}",
        self.name, self.ap, self.an, self.bp, self.bn, self.model)]
}
```

### URC — `urc`

```rust
// SPICE: U{name} {a} {b} {ref_} {model} L={length}
// spice_model_type: Some("URC")
// Ports: a, b, ref_
fn spice_model_type(&self) -> Option<&'static str> { Some("URC") }
fn spice_lines(&self) -> Vec<String> {
    vec![format!("U{} {} {} {} {} L={}",
        self.name, self.a, self.b, self.ref_, self.model, self.length)]
}
```

---

## RF Port — `port`

```rust
// SPICE: P{name} {p} {n} port={num} z0={z0}
// Ports: p, n
// Params: num (integer, default 1), z0 (default 50.0)
fn spice_lines(&self) -> Vec<String> {
    vec![format!("P{} {} {} port={} z0={}",
        self.name, self.p, self.n, self.num, self.z0)]
}
```

For `num`: `parameters.get("num").and_then(|v| v.as_f64()).unwrap_or(1.0) as i64`.

---

## Subcircuit Passthrough — `subckt`

```rust
// Emits: X{name} {ports} {subckt_name} [{params}]
// No port resolution — `ports` is a pre-formatted string from the user.
fn instantiate(...) {
    let subckt_name = require_string_parameter(parameters, "subckt_name", instance_name)?;
    let ports       = require_string_parameter(parameters, "ports", instance_name)?;
    let param_str   = get_string_parameter_or(parameters, "params", "");
    Ok(Box::new(SpiceSubcktInstance { name: instance_name.to_string(), subckt_name, ports, param_str }))
}
fn spice_lines(&self) -> Vec<String> {
    if self.param_str.is_empty() {
        vec![format!("X{} {} {}", self.name, self.ports, self.subckt_name)]
    } else {
        vec![format!("X{} {} {} {}", self.name, self.ports, self.subckt_name, self.param_str)]
    }
}
```

---

## Registration in `NgspicePlugin`

Add to `register_hardware()` in `crates/piperine-ngspice/src/lib.rs`:

```rust
fn register_hardware(&self, registry: &mut HardwareRegistry) {
    use hardware::*;
    // Already done:
    registry.register(Box::new(SpiceResistor));
    registry.register(Box::new(SpiceCapacitor));
    registry.register(Box::new(SpiceInductor));
    registry.register(Box::new(SpiceMutual::new()));
    registry.register(Box::new(SpiceVoltageSource));
    registry.register(Box::new(SpiceCurrentSource));
    registry.register(Box::new(SpiceBSourceV::new()));
    registry.register(Box::new(SpiceBSourceI::new()));

    // Add these:
    registry.register(Box::new(SpiceVpulse));
    registry.register(Box::new(SpiceIpulse));
    registry.register(Box::new(SpiceVsin));
    registry.register(Box::new(SpiceIsin));
    registry.register(Box::new(SpiceVexp));
    registry.register(Box::new(SpiceIexp));
    registry.register(Box::new(SpiceVpwl));
    registry.register(Box::new(SpiceIpwl));
    registry.register(Box::new(SpiceVsffm));
    registry.register(Box::new(SpiceVam));
    registry.register(Box::new(SpiceVnoise));
    registry.register(Box::new(SpiceVrandom));
    registry.register(Box::new(SpiceVcvs));
    registry.register(Box::new(SpiceVccs));
    registry.register(Box::new(SpiceCcvs));
    registry.register(Box::new(SpiceCccs));
    registry.register(Box::new(SpiceVsw));
    registry.register(Box::new(SpiceIsw));
    registry.register(Box::new(SpiceDiode));
    registry.register(Box::new(SpiceNpn));
    registry.register(Box::new(SpicePnp));
    registry.register(Box::new(SpiceNpn4));
    registry.register(Box::new(SpicePnp4));
    registry.register(Box::new(SpiceNmos));
    registry.register(Box::new(SpicePmos));
    registry.register(Box::new(SpiceJfetN));
    registry.register(Box::new(SpiceJfetP));
    registry.register(Box::new(SpiceMesfetN));
    registry.register(Box::new(SpiceMesfetP));
    registry.register(Box::new(SpiceVdmos));
    registry.register(Box::new(SpiceTline));
    registry.register(Box::new(SpiceLtra));
    registry.register(Box::new(SpiceUrc));
    registry.register(Box::new(SpicePort));
    registry.register(Box::new(SpiceSubckt));
}
```

---

## Key rules

1. **`fn name()` must exactly match the name in `ngspice.ppr`** — e.g. `"d"`, `"nmos"`.
2. **Parameter names must match the ppr declaration** — e.g. `"model"`, not `"mname"`.
3. **`spice_model_type()` only on model-based devices** — d, npn, pnp, nmos, pmos, jfet_n/p, mesfet_n/p, vdmos, vsw (SW), isw (CSW), ltra (LTRA), urc (URC).
4. **Model-based devices get `model` from `paramset`** — use `require_string(parameters, "model", ...)` and the elaborator injects it.
5. **Emit optional params only when non-default** — e.g. `AREA` only when ≠ 1.0.
6. **No prefix doubling** — always use `spice_name('R', &self.name)` not `format!("R{}", self.name)`.
7. **Ground net is `"0"`** — `require_net` + `resolve_net` already convert `gnd` → `"0"`.
8. **CPL is complex** — skip; it's a multi-port element with matrix params.

---

## Status checklist

| `ngspice.ppr` module  | Rust struct      | SPICE prefix | spice_model_type | Status |
|-----------------------|-----------------|--------------|------------------|--------|
| `res`                 | `SpiceResistor` | `R`          | None             | ✅ done |
| `cap`                 | `SpiceCapacitor`| `C`          | None             | ✅ done |
| `ind`                 | `SpiceInductor` | `L`          | None             | ✅ done |
| `mutual`              | `SpiceMutual`   | `K`          | None             | ✅ done |
| `vsource`             | `SpiceVoltageSource`| `V`      | None             | ✅ done |
| `isource`             | `SpiceCurrentSource`| `I`      | None             | ✅ done |
| `bsource_v`           | `SpiceBSourceV` | `B`          | None             | ✅ done |
| `bsource_i`           | `SpiceBSourceI` | `B`          | None             | ✅ done |
| `vpulse`              | `SpiceVpulse`   | `V`          | None             | ✅ done |
| `ipulse`              | `SpiceIpulse`   | `I`          | None             | ✅ done |
| `vsin`                | `SpiceVsin`     | `V`          | None             | ✅ done |
| `isin`                | `SpiceIsin`     | `I`          | None             | ✅ done |
| `vexp`                | `SpiceVexp`     | `V`          | None             | ✅ done |
| `iexp`                | `SpiceIexp`     | `I`          | None             | ✅ done |
| `vpwl`                | `SpiceVpwl`     | `V`          | None             | ✅ done |
| `ipwl`                | `SpiceIpwl`     | `I`          | None             | ✅ done |
| `vsffm`               | `SpiceVsffm`    | `V`          | None             | ✅ done |
| `vam`                 | `SpiceVam`      | `V`          | None             | ✅ done |
| `vnoise`              | `SpiceVnoise`   | `V`          | None             | ✅ done |
| `vrandom`             | `SpiceVrandom`  | `V`          | None             | ✅ done |
| `vcvs`                | `SpiceVcvs`     | `E`          | None             | ✅ done |
| `vccs`                | `SpiceVccs`     | `G`          | None             | ✅ done |
| `ccvs`                | `SpiceCcvs`     | `H`          | None             | ✅ done |
| `cccs`                | `SpiceCccs`     | `F`          | None             | ✅ done |
| `vsw`                 | `SpiceVsw`      | `S`          | `"SW"`           | ✅ done |
| `isw`                 | `SpiceIsw`      | `W`          | `"CSW"`          | ✅ done |
| `d`                   | `SpiceDiode`    | `D`          | `"D"`            | ✅ done |
| `npn`                 | `SpiceNpn`      | `Q`          | `"NPN"`          | ✅ done |
| `pnp`                 | `SpicePnp`      | `Q`          | `"PNP"`          | ✅ done |
| `npn4`                | `SpiceNpn4`     | `Q`          | `"NPN"`          | ✅ done |
| `pnp4`                | `SpicePnp4`     | `Q`          | `"PNP"`          | ✅ done |
| `nmos`                | `SpiceNmos`     | `M`          | `"NMOS"`         | ✅ done |
| `pmos`                | `SpicePmos`     | `M`          | `"PMOS"`         | ✅ done |
| `jfet_n`              | `SpiceJfetN`    | `J`          | `"NJF"`          | ✅ done |
| `jfet_p`              | `SpiceJfetP`    | `J`          | `"PJF"`          | ✅ done |
| `mesfet_n`            | `SpiceMesfetN`  | `Z`          | `"NMF"`          | ✅ done |
| `mesfet_p`            | `SpiceMesfetP`  | `Z`          | `"PMF"`          | ✅ done |
| `vdmos`               | `SpiceVdmos`    | `M`          | `"VDMOS"`        | ✅ done |
| `tline`               | `SpiceTline`    | `T`          | None             | ✅ done |
| `ltra`                | `SpiceLtra`     | `O`          | `"LTRA"`         | ✅ done |
| `urc`                 | `SpiceUrc`      | `U`          | `"URC"`          | ✅ done |
| `port`                | `SpicePort`     | `P`          | None             | ✅ done |
| `subckt`              | `SpiceSubckt`   | `X`          | None             | ✅ done |
| `isffm`               | `SpiceIsffm`    | `I`          | None             | ✅ done |
| `iam`                 | `SpiceIam`      | `I`          | None             | ✅ done |
| `inoise`              | `SpiceInoise`   | `I`          | None             | ✅ done |
| `irandom`             | `SpiceIrandom`  | `I`          | None             | ✅ done |
| `cpl`                 | `SpiceCpl`      | `P`          | None             | ✅ done |
| `txl`                 | `SpiceTxl`      | `Y`          | None             | ✅ done |
