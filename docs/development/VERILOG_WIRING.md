# Verilog / Verilog-AMS / SystemVerilog — Structural + Behavioral Mixing

Como as três linguagens modelam hierarquia, wiring e o que acontece quando
você mistura instâncias estruturais com blocos comportamentais no mesmo módulo.

---

## O modelo fundamental

Em Verilog (e derivados), **um módulo é simultaneamente**:

1. Um **namespace** de nets e instâncias (estrutural)
2. Um **processo concorrente** — behavioral blocks que correm em paralelo com as instâncias

Não há separação forçada. Um módulo pode ter qualquer combinação:

```
module foo;
    // estrutural: instâncias de outros módulos
    // comportamental: initial, always, analog
    // híbrido: assign, analog <+ que lê de net de instância filha
endmodule
```

---

## 1. Verilog — mistura estrutural + behavioral

### O caso básico: testbench

```verilog
module tb;
    reg  clk = 0;
    wire data_out;

    // ── estrutural ──────────────────────────────────────────────────────────
    dut DUT (.clk(clk), .out(data_out));

    // ── comportamental — corre em PARALELO com DUT ──────────────────────────
    always #5 clk = ~clk;           // gerador de clock: toggle a cada 5 ns

    initial begin
        @(posedge clk);             // suspende até próxima borda de subida
        @(posedge clk);
        $display("data=%b", data_out);  // lê net estrutural diretamente
        $finish;
    end
endmodule
```

`initial` e `always` podem **ler qualquer net** do módulo — incluindo nets que
pertencem às instâncias filhas. Eles rodam como processos concorrentes ao lado
das instâncias.

### O que `@(...)` faz dentro de `initial`

`@(event)` é um **wait point**: o processo suspende aqui e retoma quando o
evento dispara.

```verilog
initial begin
    // espera eventos em nets estruturais:
    @(posedge clk)              // subida de clock
    @(negedge clk)              // descida de clock
    @(clk)                      // qualquer borda
    @(posedge clk or negedge rst)   // qualquer dos dois
    @(data_out)                 // qualquer mudança de data_out
end
```

### Módulo que é DUT e monitor ao mesmo tempo

```verilog
module amp_with_monitor(in, out, vdd);
    input  in;
    output out;
    inout  vdd;
    wire   mid;

    // ── estrutural ──────────────────────────────────────────────────────────
    nmos Mn (.d(out), .g(in),  .s(gnd), .b(gnd));
    pmos Mp (.d(out), .g(in),  .s(vdd), .b(vdd));

    // ── behavioral no mesmo módulo — monitora a própria rede interna ────────
    always @(out) begin
        if (out === 1'bx)
            $warning("output metastable at t=%t", $time);
    end
end
```

O `always @(out)` monitora a net `out` do próprio módulo — que também é
a net de saída das instâncias filhas `Mn`/`Mp`. **Mesmo scope, acesso direto.**

---

## 2. Verilog-AMS — o que muda quando nets são `electrical`

### Módulo estrutural com `analog begin`

```verilog
module lpf_monitored(in, out);
    inout in, out;
    electrical in, out, mid;

    // ── estrutural ──────────────────────────────────────────────────────────
    res #(.r(1e3)) R1 (.p(in),  .n(mid));
    cap #(.c(1e-9)) C1 (.p(mid), .n(gnd));
    res #(.r(1e3)) R2 (.p(mid), .n(out));
    cap #(.c(1e-9)) C2 (.p(out), .n(gnd));

    // ── analog block no mesmo módulo — acessa nets das instâncias filhas ────
    analog begin
        // V(mid) é a net entre R1 e C1 — declarada neste módulo,
        // conectada às instâncias filhas. Acesso direto.
        if (V(mid) > 5.0)
            $warning("mid node clipping");
    end
endmodule
```

O `analog begin` do pai pode **ler** qualquer net do scope. Pode também
**contribuir** para nets que não são forçadas pelas instâncias filhas:

```verilog
analog begin
    // adiciona condutor de fuga entre mid e gnd
    // sem modificar nenhuma instância filha
    I(mid, gnd) <+ V(mid) * 1e-9;   // 1 GΩ leakage
end
```

### `always @(cross(...))` — evento analógico em módulo estrutural

```verilog
module lpf_soa(in, out);
    inout in, out;
    electrical in, out, mid;

    res #(.r(1e3)) R1 (.p(in),  .n(mid));
    cap #(.c(1e-9)) C1 (.p(mid), .n(gnd));

    // monitora net 'out' com evento analógico
    analog begin
        @(cross(V(out) - 3.3, +1))      // V(out) passa de 3.3V subindo
            $warning("output exceeded 3.3V rail");

        @(initial_step)
            $display("simulation started");
    end
endmodule
```

`cross(expr, dir)` dispara quando `expr` cruza zero. Aqui `V(out) - 3.3`
monitora a net `out` das instâncias filhas. O evento **força um ponto de tempo
extra** no solver de transiente no cruzamento.

### `above(...)` no módulo pai

```verilog
analog begin
    @(above(V(mid) - 5.0))
        $display("mid exceeded 5V at t=%g", $abstime);
end
```

`above` dispara **uma vez** quando `V(mid) > 5.0` torna-se verdadeiro.
Não re-dispara até que a condição vá a falso e volte a verdadeiro.

### Contribuição e leitura na mesma net estrutural

```verilog
module ota_with_cmfb(inp, inm, out, vcm);
    inout inp, inm, out, vcm;
    electrical inp, inm, out, vcm, cmfb_ctrl;

    // ── instâncias estruturais do core OTA ──────────────────────────────────
    diff_pair DP (.inp(inp), .inm(inm), .out(out));
    tail_curr TC (.tail(tail_node), .bias(ibias));

    // ── CMFB — lê V(out) das instâncias e contribui em cmfb_ctrl ──────────
    analog begin
        // lê saída do core (net estrutural)
        real vcm_error;
        vcm_error = (V(out) + V(inp)) / 2.0 - V(vcm);

        // contribui num net separado baseado na leitura
        I(cmfb_ctrl, gnd) <+ Gcmfb * vcm_error;
    end
endmodule
```

O `analog begin` do módulo pai atua como um **bloco de feedback** — lê nets
das instâncias filhas e injeta corrente em outro net do mesmo módulo.

---

## 3. Verilog-AMS — `connectrules` e discipline mismatch

Quando uma net `electrical` conecta a uma porta `logic`, o elaborador insere
automaticamente um módulo de conversão:

```verilog
connectrules my_ams_rules;
    connect electrical, logic   with e2d_comparator;
    connect logic, electrical   with d2a_driver;
endconnectrules

module e2d_comparator(a, d);
    input  a;   electrical a;
    output d;   logic d;
    parameter real vth = 1.65, rise = 1e-9, fall = 1e-9;

    analog begin
        @(cross(V(a) - vth, +1))
            d = 1;       // dispara evento digital a partir de evento analógico
        @(cross(V(a) - vth, -1))
            d = 0;
    end
endmodule

module d2a_driver(d, a);
    input  d;   logic d;
    output a;   electrical a;
    parameter real vhi = 3.3, vlo = 0.0, tr = 1e-9;

    analog begin
        V(a) <+ transition(d ? vhi : vlo, 0, tr);
    end
endmodule
```

Esses conectores são inseridos **automaticamente** pelo elaborador quando
detecta disciplinas incompatíveis na mesma net. O designer não precisa
instanciá-los manualmente.

---

## 4. SystemVerilog — o que adiciona sobre Verilog

### 4.1 `interface` — bundle nomeado de nets + portas

```systemverilog
interface apb_if(input logic clk);
    logic        psel, penable, pwrite;
    logic [31:0] paddr, pwdata, prdata;
    logic        pready;

    // modports: visão de cada participante
    modport master(output psel, penable, pwrite, paddr, pwdata,
                   input  prdata, pready);
    modport slave (input  psel, penable, pwrite, paddr, pwdata,
                   output prdata, pready);
endinterface

module apb_master(apb_if.master bus, input logic clk);
    always_ff @(posedge clk) begin
        bus.psel <= 1;
        // ...
    end
endmodule

module apb_slave(apb_if.slave bus);
    always_ff @(posedge bus.clk) begin
        if (bus.psel && bus.penable)
            bus.prdata <= mem[bus.paddr];
    end
endmodule

module top;
    logic clk = 0;
    always #5 clk = ~clk;

    apb_if bus(.clk(clk));          // instância de interface

    apb_master M (.bus(bus.master), .clk(clk));
    apb_slave  S (.bus(bus.slave));
endmodule
```

Interface = net bundle com **papéis** (modports). Cada instância filha vê
só as direções que lhe cabem. Simplifica wiring de protocolos.

### 4.2 `always_ff`, `always_comb`, `always_latch`

```systemverilog
always_ff @(posedge clk or posedge rst) begin
    if (rst) q <= 0;
    else     q <= d;
end

always_comb begin
    y = a & b;    // combinacional — sem lista de sensitividade manual
end
```

Versões tipadas de `always` — a ferramenta verifica se o comportamento
corresponde ao tipo declarado.

### 4.3 Clocking block — abstração de ciclo

```systemverilog
clocking cb @(posedge clk);
    input  #2 data_out;    // sample 2ns antes da borda
    output #1 data_in;     // drive 1ns depois da borda
endclocking

// testbench usa ciclos, não tempo absoluto
initial begin
    ##1;                     // espera 1 ciclo
    cb.data_in <= 1;
    ##1;
    assert(cb.data_out == 1);
end
```

### 4.4 `program` block — escopo de testbench

```systemverilog
program automatic tb;
    initial begin
        // código de testbench — sem risco de corrida com o design
        // 'program' blocks rodam na região 'Reactive' (depois de 'Active')
    end
endprogram
```

Evita race conditions entre estímulo e amostragem — o `program` roda
numa fase de scheduling posterior à do design.

### 4.5 `package` — namespace compartilhado

```systemverilog
package lpf_pkg;
    parameter real R_NOM = 1e3;
    parameter real C_NOM = 100e-9;
    typedef real real_arr_t[0:3];

    function automatic real fc(real r, c);
        return 1.0 / (2.0 * 3.14159 * r * c);
    endfunction
endpackage

module top;
    import lpf_pkg::*;
    res #(.r(R_NOM)) R1(...);
endmodule
```

### 4.6 Hierarquia de referência — `$root` e acesso hierárquico

```systemverilog
// de qualquer lugar:
$root.tb.DUT.mid_node    // referência hierárquica absoluta

// dentro de um módulo:
tb.DUT.mid_node          // relativa à raiz de simulação
```

Permite que um bloco behavioral num módulo leia nets de qualquer ponto da hierarquia.

---

## 5. O que acontece com `initial begin @(analog_event)` em módulo estrutural

Essa é a intersecção mais interessante — Verilog-AMS suporta eventos analógicos
dentro de `initial`/`always` em módulos que também têm instâncias:

```verilog
module system_monitor;
    electrical vdd, out;

    // ── estrutural ──────────────────────────────────────────────────────────
    power_rail    PWR (.vout(vdd));
    amplifier_va  AMP (.in(inp), .out(out), .vdd(vdd));

    // ── behavioral analógico no módulo pai ──────────────────────────────────
    real peak_v;
    integer violations;

    analog begin
        @(initial_step)
            violations = 0;

        @(cross(V(vdd) - 3.0, -1))    // monitora net de instância filha
            violations = violations + 1;

        @(above(V(out) - 3.6))        // out da instância AMP
            $error("output exceeded abs max: V(out)=%g", V(out));

        @(final_step)
            $display("total VDD droop violations: %d", violations);
    end
endmodule
```

**O que o simulator faz:**

1. Elabora o módulo — conecta `vdd` e `out` às instâncias
2. O `analog begin` do módulo pai passa a ser **parte do mesmo sistema de equações**
   que os `analog begin` das instâncias filhas
3. `cross(V(vdd) - 3.0)` força o solver a refinar o passo de tempo quando
   a condição se aproxima de zero — **o solver de transiente sabe sobre esse evento**
4. Quando o cruzamento acontece, o processo analógico acorda e executa a ação

Não é polling. Não é amostrado. É evento analógico real — o solver de transiente
insere pontos de breakpoint automaticamente.

---

## 6. Resumo do modelo de scheduling

```
Tempo t_n:
  ┌─────────────────────────────────────────────────────┐
  │  Analog solver (NR iterations):                     │
  │    - avalia todos analog blocks (instâncias + pai)  │
  │    - resolve KCL/KVL com todas contribuições        │
  │    - verifica condições de cross/above              │
  │    - se cruzamento: reduz passo, re-resolve         │
  └───────────────────────┬─────────────────────────────┘
                          │ convergiu
  ┌───────────────────────▼─────────────────────────────┐
  │  Event scheduler (digital side, se houver):         │
  │    Active region:   assign, always_comb             │
  │    NBA region:      always_ff (non-blocking <=)     │
  │    Observed region: clocking block input sampling   │
  │    Reactive region: program blocks                  │
  │    Postponed region: $strobe, $monitor              │
  └─────────────────────────────────────────────────────┘
                          │
  ┌───────────────────────▼─────────────────────────────┐
  │  Avança para t_{n+1} (controlado pelo solver)       │
  └─────────────────────────────────────────────────────┘
```

`initial begin @(posedge clk)` — processo digital, entra na fila da `Active region`.  
`analog begin @(cross(...))` — processo analógico, controlado pelo NR solver.  
Os dois podem coexistir no mesmo módulo — schedulers diferentes, mesmo namespace de nets.

---

## 7. O que isso implica para Piperine

| Capacidade | Como Verilog faz | Piperine hoje | Gap |
|-----------|-----------------|---------------|-----|
| Wiring estrutural | módulo com instâncias | ✅ `.ppr` | — |
| Behavioral no módulo pai | `initial`/`always` ao lado das instâncias | `.py` externo | arquitetura diferente |
| Leitura de nets de instâncias filhas em behavioral | acesso direto por scope | `sess.get_v(...)` via IPC | overhead de IPC |
| Eventos analógicos no módulo pai | `@(cross(...))` em módulo estrutural | `always @(step)` + SOA (compilado) | cross/above/timer não têm solver-level breakpoints |
| Monitor hierárquico de nets internas | `V(pai.filho.net)` | não exposto via Python | precisa de `sess.get_v` por nome |
| `interface` / `modport` | ✅ SV | ❌ | wiring de protocolos mais complexo |
| `generate for` | ✅ | ❌ | N instâncias repetidas |
| `package` | ✅ SV | ❌ | constantes compartilhadas entre arquivos |
| Scheduling de regiões | Active/NBA/Observed/Reactive | not applicable (Python é o testbench) | — |
