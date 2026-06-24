# Emulator Roadmap — OSDI → Verilog-AMS → Workbench

Plano de evolução do solver próprio até workbench AMS completo.
Cada fase é autossuficiente — entrega valor sozinha e habilita a próxima.

---

## Fase 1 — OSDI host

**Goal:** solver carrega e usa qualquer `.osdi` compilado pelo OpenVAF.

### O que é necessário

```
dlopen(.osdi)
  → ler OSDI_NUM_DESCRIPTORS + OSDI_DESCRIPTORS
  → para cada OsdiDescriptor:
      alocar model_data   (descriptor.model_size bytes)
      alocar inst_data    (descriptor.instance_size bytes)
      montar node_mapping (descriptor.node_mapping_offset dentro de inst_data)
      montar jacobian_ptrs (descriptor.jacobian_ptr_resist_offset dentro de inst_data)
```

No loop Newton-Raphson:

```
setup_model(handle, model, sim_params)
setup_instance(handle, inst, model, temperature, num_terminals, sim_params)

loop:
  eval(handle, inst, model, sim_info)        // calcula tudo internamente
  load_residual_resist(inst, model, rhs)     // stamp no vetor F
  load_jacobian_resist(inst, model)          // stamp na matriz J (via ponteiros)
  solve J·Δx = -F
  atualiza tensões
  verifica convergência
```

Para transiente (adiciona reativo):

```
eval com flags CALC_REACT_RESIDUAL | CALC_REACT_JACOBIAN
load_residual_react(inst, model, rhs)
load_jacobian_tran(inst, model, alpha)   // alpha = 1/dt (BE) ou 2/dt (trap)
load_spice_rhs_tran(inst, model, rhs, prev_solve, alpha)
```

### Quirks do OSDI que quebram quem não lê o header

- **Node collapsing** — `descriptor.collapsible` lista pares de nós que podem ser curto-circuitados (nó interno com resistência zero). Verificar `collapsed_offset` em inst_data para saber quais foram colapsados.
- **Jacobian pointer setup** — antes do primeiro eval, preencher os ponteiros em `jacobian_ptr_resist_offset` apontando para as entradas corretas da matriz esparsa. O OSDI stampa diretamente nesses ponteiros — não passa índice, passa ponteiro.
- **State variables** — `descriptor.num_states` alocados em `state_idx_off`. Conter estado entre passos de tempo (idt, filtros, etc.).
- **`bound_step`** — `descriptor.bound_step_offset` em inst_data. Depois do eval, ler esse valor; se > 0, o device pediu que o próximo Δt seja ≤ esse valor.
- **`EVAL_RET_FLAG_LIM`** — se eval retornar esse flag, limiters foram ativados; re-solve com `load_limit_rhs_*`.
- **`given_flag_model` / `given_flag_instance`** — para saber quais parâmetros foram explicitamente setados (vs default).

### Entregável

Solver faz DC + transiente com qualquer device compilado pelo OpenVAF.
Roda os integration tests do OpenVAF-Reloaded (DIODE, BSIM4, HICUML2, etc.).

---

## Fase 2 — Verilog-A

**Goal:** `.va` files são compilados automaticamente e registrados como devices.

### O que é necessário

1. **Compilação** — chamar `openvaf::compile()` com o arquivo `.va`.
   Já existe em `piperine-openvaf`. Retorna `.osdi`.

2. **Extração de metadata** — do `OsdiDescriptor` lido ao carregar o `.osdi`:
   - `descriptor.name` → nome do módulo VA
   - `descriptor.nodes[0..num_terminals]` → ports (terminais externos)
   - `descriptor.nodes[num_terminals..num_nodes]` → nós internos
   - `descriptor.param_opvar` → lista de parâmetros com tipos, defaults, flags

3. **Registro no solver** — o descriptor vira um `DeviceType` no registry do solver.
   Instanciação cria `(model_data, inst_data)` + conecta nós à netlist.

4. **Netlist integration** — `.ppr` já resolve módulos VA via `OsdiHardwareDefinition`.
   Só conectar o registry do solver novo em vez do ngspice.

### Entregável

`NgspiceSession.from_file("lpf/lpf.ppr")` usa o solver próprio em vez do ngspice.
Mesmo resultado, zero IPC overhead, solver completamente controlável.

---

## Fase 3 — Event System

**Goal:** `@(cross(...))`, `@(above(...))`, `@(timer(...))` com breakpoints reais no solver.

Essa fase é o que separa um "SPICE clone" de um "AMS solver".

### Modelo de eventos analógicos

Eventos analógicos não são callbacks assíncronos — são **condições que o solver
tem que honrar com precisão**. O solver precisa:

1. **Detectar** que uma condição mudou de sinal entre t_{n-1} e t_n
2. **Rejeitar** o passo aceito
3. **Bisectar** o intervalo [t_{n-1}, t_n] até encontrar t_cross com tolerância
4. **Aceitar** t_cross como ponto de tempo
5. **Disparar** a ação associada ao evento
6. **Continuar** a partir de t_cross

### Implementação

```
estrutura Breakpoint:
  kind: Cross { expr_id, direction } | Above { expr_id } | Timer { next, period }
  tolerance: (time_tol, expr_tol)
  action: fn(&mut SimState)

loop de transiente:
  propor t_{n+1} = t_n + dt_adaptivo
  resolver para t_{n+1}
  
  para cada breakpoint ativo:
    avaliar expr em t_n e t_{n+1}
    se mudou de sinal (Cross/Above):
      dt = bisect(t_n, t_{n+1}, expr, tol)
      aceitar t_n + dt como ponto de cruzamento
      disparar action
      re-arm Cross (Above não re-arma)
  
  para Timer:
    se t_{n+1} >= next_fire:
      forçar t_{n+1} = next_fire
      disparar action
      next_fire += period
```

### Como os devices OSDI alimentam o event system

O `eval` de um device calcula expressões internas (tensões de branch, correntes).
Para avaliar `cross(V(out) - Vth)`, o solver precisa expor `V(out)` como expressão
avaliável. Isso vem naturalmente — `V(out)` é simplesmente a diferença de tensão
entre dois nós do vetor de solução.

Para expressões mais complexas (`I(branch)`, probes internos ao device), o device
precisa expor via `access(inst, model, id, ACCESS_FLAG_READ)` com o id do opvar.

### `initial_step` / `final_step`

Mais simples — callbacks em `on_analysis_start` / `on_analysis_end`.
Já funcionam via OSDI (os devices podem ter esse código em `eval` com flag `ANALYSIS_IC`).

### Entregável

`@(cross(V(out) - 1.65, +1))` em módulos VA estruturais dispara com precisão.
Timing de cruzamento correto dentro de tolerância.
`@(timer(0, 1e-9))` força timesteps periódicos.

---

## Fase 4 — Verilog-AMS

**Goal:** nets tipadas, connect modules automáticos, módulos digitais com Verilator.

Essa fase é a mais complexa. Pode ser atacada em subfases.

### 4A — Disciplinas e nets tipadas

```verilog
electrical vin, vout;    // domínio elétrico
thermal    dT;           // domínio térmico
wire       clk;          // digital
```

Cada net no solver tem uma disciplina. A disciplina diz:
- Qual solver cuida dela (analógico, digital, térmico)
- Qual é a unidade de potential e flow
- Qual é o `abstol` para convergência

Implementação: adicionar `discipline: Option<DisciplineId>` nos nós da netlist.
Solver analógico só cuida de nós com `domain = continuous`.

### 4B — Connect modules automáticos

Quando `electrical` conecta a `wire` (digital), o elaborador insere um connect module:

```
Detecção no elaborador:
  net N conecta porta electrical (A) e porta wire (B)
  → inserir instância de connect_module entre A e B
  → connect_module vem das connectrules registradas

Connect module elétrico→digital:
  analog begin
    @(cross(V(a) - Vth, +1))  d = 1;
    @(cross(V(a) - Vth, -1))  d = 0;
  end

Connect module digital→elétrico:
  analog V(a) <+ transition(d ? Vhi : Vlo, 0, tr);
```

Inserção automática = zero boilerplate no `.ppr`. Designer conecta diretamente.

### 4C — Co-simulação com digital (Verilator)

O digital scheduler roda ao lado do solver analógico:

```
loop de co-simulação:
  
  t_analog_next = solver.propose_step()
  t_digital_next = digital_scheduler.next_event()
  t_sync = min(t_analog_next, t_digital_next)
  
  solver.advance_to(t_sync)          // resolve analógico até t_sync
  digital_scheduler.advance_to(t_sync) // avança eventos digitais
  
  // troca de valores na fronteira:
  para cada connect_module e→d:
    digital_scheduler.set_input(net, solver.voltage(net))
  para cada connect_module d→e:
    solver.set_source(net, digital_scheduler.output(net))
  
  // verificar breakpoints cross/above nos connect modules
  // re-sincronizar se necessário
```

Opções de engine digital:
- **Verilator** (SystemVerilog, compilado, rápido) — via shared memory / IPC
- **Icarus Verilog** (Verilog, interpretado, flexível) — via VPI
- **Motor próprio simples** (apenas `wire`/`reg` + `always @(posedge/negedge)`) — para casos básicos

### Entregável

Módulo `.ppr` com instâncias VA + instâncias digitais Verilog.
Connect modules inseridos automaticamente.
Co-simulação transiente com sincronização de eventos.

---

## Fase 5 — Workbench

**Goal:** API Python (e Rust) rica, paralela, introspectável. Substitui a camada ngspice.

### O que muda em relação ao atual

Atual: Python → IPC → piperine-worker → libngspice  
Novo:  Python → PyO3 → solver próprio (in-process, zero IPC)

Ganhos:
- **Acesso direto** a tensões, correntes, estados internos — sem `GetVec` por IPC
- **Callbacks Python durante a simulação** — não só depois
- **Breakpoints em Python** — `sess.on_cross(expr, callback)` registra função Python
- **Paralelo real** — múltiplas sessões = múltiplas instâncias do solver, sem worker processes

### API target

```python
import piperine as ppr

sess = ppr.Session.from_file("amp/amp.ppr", module="amp_tb")

# análises básicas (igual ao hoje)
op   = sess.op()
tran = sess.tran("1n", "10u")
ac   = sess.ac("dec", 50, 1e2, 1e8)

# introspection durante simulação
@sess.on_cross("V(out) - 1.65", rising=True)
def log_crossing(t, state):
    print(f"crossing at t={t:.3e}, V(out)={state.voltage('out'):.4f}")

# acesso a estados internos de devices VA
tran = sess.tran("1n", "10u")
gm_trace = tran.opvar("M1", "gm")   # lê opvar do MOSFET via OSDI access()

# parallel MC sem overhead de IPC
sessions = [sess.fork() for _ in range(30)]  # clona estado do solver
futures  = [s.tran_async("1n", "10u") for s in sessions]
results  = ppr.join_all(futures)
```

### `sess.fork()` — clone de solver

Diferente de criar sessão do zero (re-elabora, re-seta parâmetros):
`fork()` clona o estado interno do solver — vetor de solução, estados de devices,
configuração do netlist. Cópia barata, ideal para MC onde só os parâmetros variam.

### Entregável

API Python completa. Callbacks durante simulação. Fork barato para MC paralelo.
Sem ngspice como dependência runtime (só como fallback opcional).

---

## Sequência de dependências

```
Fase 1: OSDI host
    ↓
Fase 2: Verilog-A (OpenVAF → .osdi → solver)
    ↓
Fase 3: Event System (cross/above/timer com bisection)
    ↓
Fase 4A: Nets tipadas + disciplinas
    ↓
Fase 4B: Connect modules automáticos
    ↓
Fase 4C: Co-simulação digital
    ↓
Fase 5: Workbench Python
```

Fases 3 e 4A podem correr em paralelo depois da Fase 2.
Fase 5 pode começar parcialmente após Fase 2 (API sem eventos ou co-sim).

---

## O que NÃO está neste roadmap

- **Harmonic Balance** — OSDI 0.4 tem `load_jacobian_with_offset_*` e `write_jacobian_array_*` para HB, mas é análise separada. Fase posterior.
- **Noise analysis** — `load_noise` já está no OSDI. Adicionável na Fase 2 como análise extra.
- **Sensitivity / Pole-Zero** — requer solver especializado. Fase posterior.
- **Layout parasíticos** — extração RC externa, ingestão via netlist. Não muda o solver.
