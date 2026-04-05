# Piperine — Gap Analysis ngspice vs piperine-api + Plano de Implementação

> NgSpice wrapper em Rust: circuitos como código, workers paralelos, API ergonômica.

## Visão Geral da Arquitetura

```text
                      piperine (bin/lib)
                           |
           +---------------+----------------+
           |               |                |
      piperine-api    piperine-ngspice   piperine-pool
      (circuit DSL,   (FFI bindings,     (worker pool,
       netlist gen,    safe wrapper)      job dispatch)
       units, etc.)
```

**Decisão chave**: Usar `libngspice.so` via FFI (não processo externo).
Cada worker é um **processo separado** (re-exec com `--worker`) porque ngspice
usa globals internos e não é thread-safe. Comunicação via stdin/stdout bincode.

---

## Estado Atual do piperine-api

- **24 devices**: R, C, L, K, D, Q, J, M, Z, V, I, E, G, F, H, B, S, W, T, O, U, P, Y, X
- **8+ model types**: resistor, capacitor, inductor, diode, bjt, mosfet, jfet, switch, mesfet, vdmos, ltra, urc, cpl, txl
- **7 analysis types**: Op, Dc, Ac, Tran, Noise, Tf, Sens
- **Waveforms**: Pulse, Sin, Exp, Pwl, Sffm, Am, TrNoise, TrRandom, External + AC
- **Measurement**: MAX, MIN, AVG, RMS, PP, INTEG, PARAM — via `Measurement` struct
- **Expr system**: completo — aritmética, comparação, lógica, ternário, funções built-in, hiperbólicas, funções de sinal, constantes físicas, table(), 40+ macros ergonômicos
- **Result types**: `TimeSeries<'_>`, `AcSpectrum<'_>` zero-copy
- **SubCircuit**: composição em nível Rust (não `.subckt` SPICE)
- **.PARAM**: `circuit.param()` + `subcircuit.param()`
- **.NODESET**: via `SolverOptions::nodeset()`
- **Node opaco**, units tipadas, `Dynamic<T>`, `SpiceComponent` separado de `Component`
- **Worker pool** com IPC bincode (piperine-ngspice + piperine-pool)

---

## 1. DEVICES FALTANTES

### 1.1 Prioridade Média (XSPICE)

| Device | Descrição | Notas |
|--------|-----------|-------|
| XSPICE instance | A device (code model instance) | Genérico — aceita qualquer code model |

O device `A` é um container genérico que instancia qualquer code model XSPICE.
Não precisamos tipar todos os 73 code models — basta um `XspiceInstance` genérico.

### 1.2 Prioridade Baixa (Verilog-A)

| Device | Símbolo | Descrição |
|--------|---------|-----------|
| OSDI device | N | Verilog-A compact model via OSDI interface |

---

## 2. DEPENDENT SOURCE MODES FALTANTES

### 2.1 E source (VCVS) — modos não-lineares

| Modo | Sintaxe | Prioridade |
|------|---------|------------|
| VOL | `E1 n+ n- VOL='expr'` | Alta |
| VALUE | `E1 n+ n- VALUE={expr}` | Alta |
| TABLE | `E1 n+ n- TABLE {expr}=(x1,y1)(x2,y2)...` | Média |
| LAPLACE | `E1 n+ n- LAPLACE {expr}={s_expr}` | Média |
| FREQ | `E1 n+ n- FREQ {expr}=(f1,mag1,phase1)...` | Baixa |
| Lógicas | AND/OR/NAND/NOR/XOR/NXOR modes | Baixa |
| POLY | `E1 n+ n- POLY(dim) ...` | Média |

### 2.2 G source (VCCS) — modos não-lineares

Mesmos modos que E: CUR, VALUE, TABLE, LAPLACE, FREQ, POLY

### 2.3 F/H sources — POLY

| Modo | Sintaxe | Prioridade |
|------|---------|------------|
| POLY(F) | `F1 n+ n- POLY(dim) Vsrc1 Vsrc2 ... coeffs` | Média |
| POLY(H) | `H1 n+ n- POLY(dim) Vsrc1 Vsrc2 ... coeffs` | Média |

### 2.4 Behavioral R/C/L

| Device | Sintaxe | Notas |
|--------|---------|-------|
| R behavioral | `R1 n+ n- R='expr'` | Resistência como expressão |
| C behavioral | `C1 n+ n- C='expr'` ou `Q='expr'` | Capacitância/carga como expressão |
| L behavioral | `L1 n+ n- L='expr'` ou `Flux='expr'` | Indutância/fluxo como expressão |

---

## 3. ANALYSES FALTANTES

### 3.1 Prioridade Alta

| Análise | Comando | Notas |
|---------|---------|-------|
| .PZ | `pz node1 node2 node3 node4 cur/vol pol/zer/pz` | Pole-Zero |
| .SP | `.SP dec/oct/lin np fstart fstop [donoise]` | S-Parameter (produz S/Y/Z) |
| .DC dual sweep | `.DC src1 s1 e1 i1 src2 s2 e2 i2` | Nested DC sweep |

### 3.2 Prioridade Média

| Análise | Comando | Notas |
|---------|---------|-------|
| .DISTO | `.DISTO dec/oct/lin np fstart fstop [f2overf1]` | Harmonic/intermod distortion |
| .PSS | `.PSS gfreq tstab oscnob pession ...` | Periodic Steady State (experimental) |
| .DC sweep em R/T | `.DC Rname start stop incr` / `.DC TEMP ...` | Sweep em resistor/temperatura |

### 3.3 Features faltantes em analyses existentes

| Feature | Análise | Notas |
|---------|---------|-------|
| pts_per_summary | .NOISE | Parâmetro opcional |
| filter strings | .SENS | Seleção de parâmetros |
| dual sweep | .DC | Segundo source aninhado |

---

## 4. NETLIST/CIRCUIT FEATURES FALTANTES

### 4.1 Prioridade Alta

| Feature | Comando | Notas |
|---------|---------|-------|
| .FUNC | `.func name(args) {expr}` | Funções definidas pelo usuário |
| .GLOBAL | `.global node1 node2 ...` | Nodes globais |
| .IC | `.ic V(node)=value` | Condições iniciais |
| .SAVE | `.save V(x) I(Vdd) all` | Seleção de outputs |
| .MEAS TRIG/TARG | `.meas tran delay TRIG...TARG...` | Medições de tempo e cruzamentos |
| .OPTIONS | Comprehensive | ~50 options (abstol, reltol, etc.) — SolverOptions incompleto |

### 4.2 Prioridade Média

| Feature | Comando | Notas |
|---------|---------|-------|
| .FOUR | `.four freq v(out)` | Fourier analysis |
| .CSPARAM | `.csparam name={expr}` | Constant SPICE params |
| .IF/.ELSE/.ENDIF | Condicional | Preprocessor condicional |
| .TEMP | `.temp t1 t2 t3` | Multi-temperature run |
| par('expr') | `par('v(out)*2')` | Expressões algébricas no output |
| Model binning | BSIM3/BSIM4 | Seleção automática de modelo por L/W |

### 4.3 Instance Parameters Faltantes

| Param | Devices | Notas |
|-------|---------|-------|
| m | Todos | Parallel multiplier |
| temp/dtemp | Todos com model | Temperatura da instância |
| off | D, Q, J, M | Inicialização off |
| ic | C, L, D, Q, J, M | Condições iniciais por instância |
| noisy | R | Controle de ruído por resistor |
| ac | V, I | Magnitude/fase AC pequeno sinal |
| scale | R | Fator de escala por instância |

### 4.4 Semiconductor Models Extras

| Feature | Device | Notas |
|---------|--------|-------|
| Cap semiconductor | C | Geométrico: CJ, CJSW, DEFW, DEFL, NARROW, SHORT, DI, THICK |
| Inductor model | L | Geométrico: IND, CSECT, DIA, LENGTH, NT, MU |
| SOA checks | Q, M, D | Safe Operating Area warnings |
| RF Port | V | portnum, z0 para .SP |

---

## 5. XSPICE CODE MODELS (Ch 8)

O XSPICE fornece 73 code models prontos. **Estratégia recomendada:**
Não tipar todos — criar `XspiceInstance` genérico + helpers para os mais usados.

### 5.1 Catálogo Completo

**Analog (34 models):**
gain, summer, multiplier, divider, limiter, controlled_limiter, pwl_controlled,
pwl_time_controlled, filesource, multi_input_pwl, aswitch, alt_aswitch, zener,
current_limiter, hysteresis, differentiator, integrator, s_xfer, pwl_xfer,
slew_rate, inductive_coupling, magnetic_core, sine_osc, triangle_osc,
square_osc, controlled_oneshot, cap_meter, ind_meter, memristor,
table2d, table3d, simple_diode, analog_delay, potentiometer

**Digital (25 models):**
d_buffer, d_inverter, d_and, d_nand, d_or, d_nor, d_xor, d_xnor,
d_tristate, d_pullup, d_pulldown, d_dff, d_jkff, d_tff, d_srff,
d_dlatch, d_srlatch, d_state, d_fdiv, d_ram, d_source, d_lut,
d_lut_g, d_process, d_cosim

**Hybrid (9 models):**
dac_bridge, adc_bridge, bidi_bridge, controlled_osc, d_to_real,
oneshot_z, real_gain, real_to_v, pwm_osc

**Transmission Line (5 models):**
ltline, lcouple, microstrip, coupled_microstrip, microstrip_open_end

### 5.2 Implementação Recomendada

- `XspiceInstance` genérico com params como `HashMap<String, XspiceParam>`
- Helpers tipados para os mais usados: `dac_bridge`, `adc_bridge`, `d_source`, `filesource`
- Node types XSPICE: digital (12 values), real, int — modelar como `XspiceNodeType` enum

---

## 6. SHARED LIBRARY API (piperine-ngspice)

### 6.1 Já wrappados

- ngSpice_Init (7 callbacks), ngSpice_Init_Sync
- ngSpice_Command, ngSpice_Circ, ngGet_Vec_Info

### 6.2 Faltantes

| Função | Prioridade | Notas |
|--------|-----------|-------|
| ngSpice_Init_Evt | Média | Callbacks XSPICE events |
| ngSpice_Raw_Evt | Baixa | Raw XSPICE event data |
| ngSpice_SetBkpt | Baixa | Breakpoints na simulação |
| ngSpice_LockRealloc / UnlockRealloc | Baixa | Thread safety |
| ngSpice_AllEvtNodes | Média | Lista todos os event nodes |
| ngGet_Evt_NodeInfo | Média | Info de um event node |
| GetSyncData callback | Média | Sincronização de time-step |

---

## 7. MONTE CARLO / ESTATÍSTICA (Ch 18)

Não é um comando .MC — usa funções estatísticas + loops de controle:

| Feature | Implementação | Prioridade |
|---------|--------------|------------|
| gauss(nom, rvar, sigma) | Função em .param | Alta |
| agauss(nom, avar, sigma) | Função em .param | Alta |
| unif(nom, rvar) | Função em .param | Alta |
| aunif(nom, avar) | Função em .param | Alta |
| mc_runs + altermod/alter | Control script loop | Média |
| Statistical functions | rnd, sgauss, sunif, poisson, exponential | Baixa |

**Estratégia:** Não precisa de API especial — suporte a .param + .func + control commands
é suficiente. Pode ter um helper `MonteCarlo::run(circuit, analysis, n_runs, params)`.

---

## 8. PLANO DE IMPLEMENTAÇÃO — FASES

### Fase 1 — Dependent Source Modes

**Arquivos:** `devices/vcvs.rs`, `devices/vccs.rs`, `devices/cccs.rs`, `devices/ccvs.rs`

1. Adicionar modos VALUE/TABLE/LAPLACE/FREQ ao E/G como enum `NonLinearMode`
2. Adicionar POLY aos E/G/F/H
3. Behavioral R/C/L — adicionar `expression` field aos devices existentes
4. Testes: cada modo gera string SPICE correta

### Fase 2 — Analyses Faltantes

**Arquivos:** `analysis.rs`

1. PoleZeroAnalysis (6 formas: cur/vol × pol/zer/pz)
2. SParamAnalysis (como AC + donoise)
3. DistortionAnalysis (harmonic/intermod)
4. DC dual sweep (adicionar segundo source ao DcAnalysis)
5. PSS (experimental — baixa prioridade)
6. .FOUR como post-processing

### Fase 3 — Netlist Features

**Arquivos:** `circuit.rs`, `spice.rs`, novo `param.rs`

1. .FUNC — `circuit.func()` com lista de argumentos e Expr
2. .GLOBAL — `circuit.global_node()`
3. .IC — `circuit.initial_condition()`
4. .SAVE selective — `analysis.save()` seleção de vetores de saída
5. .MEAS TRIG/TARG — `Measurement::trig(...).targ(...)` builder
6. .OPTIONS comprehensive — expandir `SolverOptions` com ~50 options
7. .IF/.ELSE conditional — via `circuit.conditional()`

### Fase 4 — Instance Parameters

**Arquivos:** Todos `devices/*.rs`

1. Adicionar `m: Option<f64>` (parallel multiplier) a todos os devices
2. Adicionar `temp/dtemp: Option<f64>` aos devices com model
3. Adicionar `off: bool`, `ic: Option<Vec<f64>>` onde aplicável
4. Adicionar `noisy: Option<bool>` ao Resistor
5. Atualizar `SpiceComponent::into_spice()` para emitir esses params

### Fase 5 — XSPICE Support

**Arquivos:** Novo `devices/xspice.rs`, atualizar piperine-ngspice

1. `XspiceInstance` genérico com params não-tipados
2. `XspiceNodeType` enum (analog, digital, real, int)
3. Helpers tipados para dac_bridge, adc_bridge, d_source
4. ngSpice_Init_Evt, ngSpice_AllEvtNodes no piperine-ngspice
5. Automatic bridging support

### Fase 6 — Monte Carlo & Statistical

**Arquivos:** Novo `monte_carlo.rs`, atualizar `circuit.rs`

1. Funções estatísticas: gauss, agauss, unif, aunif como Expr helpers
2. `MonteCarlo` helper que gera loop de control commands
3. .MEAS parser para resultados
4. .FOUR post-processing

---

## 9. FEATURES DELIBERADAMENTE EXCLUÍDAS

| Feature | Razão |
|---------|-------|
| Optimization (Ch 19) | External-only (scripts/ASCO) — fora do escopo |
| Interactive interpreter (Ch 13) | UI concern — não é API |
| Verilog-A / OSDI (Ch 9) | Muito nichado — escape hatch via raw() |
| d_cosim / d_process | Co-simulation — muito complexo, pouco uso |
| Model binning BSIM | Geralmente vem de .lib externo |
| SOA warnings | Passthrough do log do ngspice |

---

## 10. VERIFICAÇÃO

1. Cada fase termina com `cargo check -p piperine-api`
2. Testes unitários: serialização SPICE de cada novo feature
3. Testes E2E: executar simulações reais via worker pool
4. Teste de regressão: circuitos existentes continuam funcionando
5. Exemplo completo: CMOS inverter com TRAN + PULSE + .MEAS
