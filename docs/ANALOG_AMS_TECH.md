# Verilog-AMS Analog Subset — Especificação Técnica

> Referência: Accellera Verilog-AMS Language Reference Manual (LRM 2.4/2023).
> Este documento cobre exclusivamente o subconjunto analógico (Verilog-A) da linguagem.

---

## 1. Convenções Léxicas

### 1.1 Espaço em branco e comentários

```
// comentário de linha
/* comentário de bloco */
```

Whitespace (espaço, tab, newline) é ignorado exceto como separador de tokens.

### 1.2 Identificadores

```
identifier     ::= simple_identifier | escaped_identifier
simple_identifier ::= [a-zA-Z_] [a-zA-Z0-9_$]*
escaped_identifier ::= \ {non_whitespace}+ whitespace
```

Case-sensitive. Comprimento máximo definido pela implementação (mínimo 1024 caracteres).

### 1.3 Literais numéricos

```
integer_literal  ::= decimal_number | based_number
decimal_number   ::= [sign] unsigned_number
based_number     ::= [size] ' base_format unsigned_number
base_format      ::= [s] ( b | o | d | h )

real_literal     ::= fixed_point | scientific
fixed_point      ::= unsigned_number . unsigned_number
scientific       ::= real_literal ( e | E ) [sign] unsigned_number
```

Sufixos de escala (extensão Verilog-A):

| Sufixo | Fator | Sufixo | Fator |
|--------|-------|--------|-------|
| `T`    | 10¹²  | `m`    | 10⁻³  |
| `G`    | 10⁹   | `u`    | 10⁻⁶  |
| `M`    | 10⁶   | `n`    | 10⁻⁹  |
| `K`, `k` | 10³ | `p`    | 10⁻¹² |
|        |       | `f`    | 10⁻¹⁵ |
|        |       | `a`    | 10⁻¹⁸ |

### 1.4 Literais de string

```
string_literal ::= " { any_char_except_newline_or_dquote | escape_seq } "
escape_seq     ::= \n | \t | \\ | \" | \0 | \ddd (octal)
```

### 1.5 Palavras reservadas (subconjunto analógico)

```
above           abs            absdelay       ac_stim
analog          analysis       begin          branch
case            cross          ddt            ddx
default         disable        discipline     driver_update
else            end            endcase        enddiscipline
endfunction     endgenerate    endmodule      endnature
endparamset     endtask        event          exclude
final_step      flicker_noise  flow           for
from            function       generate       genvar
ground          idt            idt_nature      if
inf             initial_step   inout          input
integer         laplace_nd     laplace_np     laplace_zd
laplace_zp      last_crossing  limexp         localparam
macromodule     max            min            module
nature          net_resolution noise_table    noise_table_log
or              output         parameter      paramset
potential       real           slew           string
timer           transition     white_noise    wreal
zi_nd           zi_np          zi_zd          zi_zp
```

---

## 2. Diretivas de Preprocessador

### 2.1 Inclusão de arquivos

```verilog
`include "filename"
```

### 2.2 Macros de texto

```verilog
`define MACRO_NAME                      // flag
`define MACRO_NAME value                // object-like
`define MACRO_NAME(arg1, arg2) body     // function-like

`undef MACRO_NAME
```

Uso: `` `MACRO_NAME `` ou `` `MACRO_NAME(x, y) ``

### 2.3 Compilação condicional

```verilog
`ifdef MACRO_NAME
    // código se definido
`elsif OTHER_MACRO
    // alternativa
`else
    // fallback
`endif

`ifndef MACRO_NAME
    // código se NÃO definido
`endif
```

### 2.4 Outras diretivas

```verilog
`resetall                  // restaura todas as diretivas ao default
`default_discipline discipline_name
`timescale time_unit / time_precision
`default_transition time_expression
```

---

## 3. Constantes Predefinidas (`constants.vams`)

### 3.1 Constantes matemáticas

| Macro | Valor | Descrição |
|-------|-------|-----------|
| `` `M_E ``       | 2.7182818284590452354  | *e* (base do logaritmo natural) |
| `` `M_LOG2E ``   | 1.4426950408889634074  | log₂(*e*) |
| `` `M_LOG10E ``  | 0.43429448190325182765 | log₁₀(*e*) |
| `` `M_LN2 ``     | 0.69314718055994530942 | ln(2) |
| `` `M_LN10 ``    | 2.30258509299404568402 | ln(10) |
| `` `M_PI ``      | 3.14159265358979323846 | π |
| `` `M_TWO_PI ``  | 6.28318530717958647693 | 2π |
| `` `M_PI_2 ``    | 1.57079632679489661923 | π/2 |
| `` `M_PI_4 ``    | 0.78539816339744830962 | π/4 |
| `` `M_1_PI ``    | 0.31830988618379067154 | 1/π |
| `` `M_2_PI ``    | 0.63661977236758134308 | 2/π |
| `` `M_2_SQRTPI `` | 1.12837916709551257390 | 2/√π |
| `` `M_SQRT2 ``   | 1.41421356237309504880 | √2 |
| `` `M_SQRT1_2 `` | 0.70710678118654752440 | 1/√2 |

### 3.2 Constantes físicas

Cada constante existe em múltiplas versões: `_SPICE`, `_NIST2004`, `_NIST2010`.
A versão sem sufixo é um alias controlado por `ifdef`.

| Macro | Valor (NIST 2010) | Unidade | Descrição |
|-------|-------------------|---------|-----------|
| `` `P_Q ``  | 1.602176565e-19  | C       | Carga do elétron |
| `` `P_K ``  | 1.3806488e-23    | J/K     | Constante de Boltzmann |
| `` `P_H ``  | 6.62606957e-34   | J·s     | Constante de Planck |
| `` `P_C ``  | 2.99792458e8     | m/s     | Velocidade da luz no vácuo |
| `` `P_EPS0 `` | 8.854187817e-12 | F/m    | Permissividade do vácuo |
| `` `P_U0 ``  | 1.2566370614e-6  | H/m    | Permeabilidade do vácuo |
| `` `P_CELSIUS0 `` | 273.15      | K      | Zero Celsius em Kelvin |

---

## 4. Natures e Disciplines

### 4.1 Nature

Uma nature define as propriedades de uma grandeza física contínua.

```verilog
nature nature_name ;
    units      = "unit_string" ;
    access     = access_function_name ;
    idt_nature = integral_nature_name ;   // opcional
    ddt_nature = derivative_nature_name ; // opcional
    abstol     = absolute_tolerance ;     // obrigatório
endnature
```

**Campos:**

| Campo | Tipo | Obrigatório | Descrição |
|-------|------|-------------|-----------|
| `units` | string | Sim | Unidade SI (e.g. `"V"`, `"A"`) |
| `access` | identifier | Sim | Nome da função de acesso (e.g. `V`, `I`) |
| `abstol` | real | Sim | Tolerância absoluta para convergência |
| `idt_nature` | identifier | Não | Nature resultante de integração temporal |
| `ddt_nature` | identifier | Não | Nature resultante de derivação temporal |
| `blowup` | real | Não | Limiar para detecção de divergência |

**Herança:** Uma nature pode herdar de outra:

```verilog
nature Voltage : Potential ;
    // herda tudo de Potential, pode sobrescrever
endnature
```

### 4.2 Discipline

Uma discipline combina natures para definir um domínio de sinal.

```verilog
discipline discipline_name ;
    potential nature_name ;     // natureza de potencial
    flow      nature_name ;     // natureza de fluxo
    domain    continuous ;      // continuous | discrete
enddiscipline
```

Para disciplines somente-potencial ou somente-fluxo, omite-se o campo correspondente.

### 4.3 Disciplines padrão (`disciplines.vams`)

```verilog
// --- Electrical ---
nature Voltage ;
    units    = "V" ;
    access   = V ;
    idt_nature = Flux ;
    abstol   = 1e-6 ;      // 1 µV
endnature

nature Current ;
    units    = "A" ;
    access   = I ;
    idt_nature = Charge ;
    abstol   = 1e-12 ;     // 1 pA
endnature

discipline electrical ;
    potential Voltage ;
    flow     Current ;
enddiscipline

// --- Thermal ---
nature Temperature ;
    units  = "K" ;
    access = Temp ;
    abstol = 1e-4 ;
endnature

nature Power ;
    units  = "W" ;
    access = Pwr ;
    abstol = 1e-9 ;
endnature

discipline thermal ;
    potential Temperature ;
    flow     Power ;
enddiscipline

// --- Kinematic / Mechanical (translational) ---
nature Position ;
    units      = "m" ;
    access     = Pos ;
    ddt_nature = Velocity ;
    abstol     = 1e-6 ;
endnature

nature Velocity ;
    units      = "m/s" ;
    access     = Vel ;
    ddt_nature = Acceleration ;
    idt_nature = Position ;
    abstol     = 1e-6 ;
endnature

nature Force ;
    units  = "N" ;
    access = F ;
    abstol = 1e-6 ;
endnature

discipline kinematic ;
    potential Position ;
    flow     Force ;
enddiscipline

// --- Rotational Mechanical ---
nature Angle ;
    units      = "rad" ;
    access     = Theta ;
    ddt_nature = Angular_Velocity ;
    abstol     = 1e-6 ;
endnature

nature Angular_Velocity ;
    units      = "rad/s" ;
    access     = Omega ;
    idt_nature = Angle ;
    abstol     = 1e-6 ;
endnature

nature Angular_Force ;
    units  = "N*m" ;
    access = Tau ;
    abstol = 1e-6 ;
endnature

discipline rotational ;
    potential Angle ;
    flow     Angular_Force ;
enddiscipline
```

**Ground node:** `ground` declara um nó de referência global (potencial = 0):

```verilog
ground gnd ;
```

---

## 5. Módulos

### 5.1 Declaração

```verilog
module module_name ( port_list ) ;
    // port declarations
    // parameter declarations
    // variable declarations
    // branch declarations
    // analog block(s)
endmodule
```

### 5.2 Portas

```verilog
// Estilo ANSI (inline)
module resistor ( inout electrical p, inout electrical n ) ;

// Estilo non-ANSI
module resistor ( p, n ) ;
    inout p, n ;
    electrical p, n ;
```

**Direções de porta:**

| Keyword | Descrição |
|---------|-----------|
| `inout`  | Bidirecional (potencial e fluxo) — padrão para analog |
| `input`  | Somente potencial (fluxo fixo em zero) |
| `output` | Somente fluxo (potencial não observável) |

### 5.3 Disciplina de porta

Portas podem ter disciplines explícitas ou inferidas:

```verilog
inout electrical p, n ;    // explícita
inout p, n ;                // inferida via `default_discipline ou contexto
```

---

## 6. Parâmetros

### 6.1 Parameter

```verilog
parameter type name = default_value [ range_spec ] ;
```

**Tipos válidos:**

| Tipo | Verilog-A keyword |
|------|-------------------|
| Real (ponto flutuante) | `real` |
| Inteiro | `integer` |
| String | `string` |

**Especificação de range:**

```verilog
parameter real R = 1.0 from (0:inf) ;       // exclusivo: 0 < R < ∞
parameter real R = 1.0 from [0:inf) ;       // inclusivo-exclusivo: 0 ≤ R < ∞
parameter real R = 1.0 from (-inf:inf) ;    // qualquer valor
parameter real R = 1.0 from [0:inf) exclude 0 ; // ≥ 0 mas ≠ 0
parameter integer type = 1 from [1:4] ;     // 1 ≤ type ≤ 4
parameter integer type = 1 from [1:4] exclude 3 ; // 1,2,4
```

**Delimitadores de range:**
- `[` / `]` — inclusivo
- `(` / `)` — exclusivo
- `inf` / `-inf` — infinito

**Múltiplos ranges:**

```verilog
parameter real x = 0.0 from [-1:1] from [10:20] ;  // -1≤x≤1 OR 10≤x≤20
```

### 6.2 Localparam

Parâmetro não exposto à instanciação:

```verilog
localparam real PI_SQ = `M_PI * `M_PI ;
```

### 6.3 Atributos de parâmetro

```verilog
(*desc="Ohmic resistance", units="Ohm", type="instance"*)
parameter real R = 0.0 from [0:inf) ;

(*desc="Model name", type="model"*)
parameter string model_name = "default" ;
```

**Atributos padrão:**

| Atributo | Tipo | Descrição |
|----------|------|-----------|
| `desc` | string | Descrição textual |
| `units` | string | Unidade física |
| `type` | string | `"model"` ou `"instance"` |

### 6.4 Aliasparam

Cria um alias para outro parâmetro:

```verilog
aliasparam resistance = R ;
```

### 6.5 Paramset

Conjunto nomeado de valores de parâmetros para um módulo:

```verilog
paramset paramset_name module_name ;
    .param1(value1),
    .param2(value2) ;
    // pode conter parameter declarations locais
    parameter real scale = 1.0 ;
    .param3(value3 * scale) ;
endparamset
```

---

## 7. Tipos de Dados e Variáveis

### 7.1 Tipos básicos

```verilog
real      x ;           // ponto flutuante (64 bits IEEE 754)
real      x = 0.0 ;     // com inicialização
integer   n ;           // inteiro com sinal (32 bits mínimo)
integer   n = 0 ;
string    s ;           // cadeia de caracteres
string    s = "hello" ;
```

### 7.2 Arrays

```verilog
real    v[0:9] ;        // array de 10 reais (índices 0..9)
integer flags[1:8] ;    // array de 8 inteiros (índices 1..8)
```

### 7.3 Genvar

Variável de iteração para `analog for` — avaliada em elaboração, não em runtime:

```verilog
genvar i ;
```

### 7.4 Variáveis versus contribuições

| Declaração | Natureza | Uso |
|------------|----------|-----|
| `real x ;` + `x = expr ;` | Variável procedural | Armazena valor intermediário |
| `I(a,b) <+ expr ;` | Contribuição | Injeta corrente/tensão no circuito |

Variáveis **NÃO** retêm valor entre passos de tempo a menos que sejam explicitamente mantidas. Contribuições são acumulativas dentro do mesmo bloco analog.

---

## 8. Branches

### 8.1 Declaração de branch

```verilog
branch (node_a, node_b) branch_name ;
branch (node_a)          port_branch ;  // branch de porta (outro terminal é terra)
```

### 8.2 Branch implícito

Quando se usa `V(a, b)` ou `I(a, b)` sem declarar branch, um branch anônimo é criado implicitamente.

### 8.3 Relação Kirchhoff

Em um branch `(a, b)`:
- **Potencial:** `V(br) = V(a) - V(b)` (convenção: a é referência positiva)
- **Fluxo:** `I(br)` flui de `a` para `b` (convenção de receptor passivo)
- **Conservação (KCL):** A soma de todas as correntes entrando em um nó é zero
- **Continuidade (KVL):** A soma das tensões ao longo de um laço é zero

---

## 9. Bloco `analog`

### 9.1 Estrutura

```verilog
module mydevice(p, n) ;
    inout electrical p, n ;
    parameter real R = 1k ;

    analog begin
        // statements
    end
endmodule
```

O bloco `analog` é avaliado a cada passo de tempo pelo simulador. Ele descreve as equações constitutivas do dispositivo.

### 9.2 `analog initial`

Executado **uma única vez** antes do início da análise:

```verilog
analog initial begin
    // inicialização de variáveis
    x = 0.0 ;
end
```

---

## 10. Operador de Contribuição

### 10.1 Sintaxe

```verilog
access_function( branch_or_nodes ) <+ expression ;
```

### 10.2 Contribuição a potencial (tensão)

```verilog
V(p, n) <+ vdc ;                        // fonte de tensão ideal: V(p,n) = vdc
V(br)   <+ R * I(br) ;                  // resistor: V = R·I
```

### 10.3 Contribuição a fluxo (corrente)

```verilog
I(p, n) <+ V(p, n) / R ;               // resistor: I = V/R
I(p, n) <+ C * ddt(V(p, n)) ;          // capacitor: I = C·dV/dt
```

### 10.4 Acumulação

Múltiplas contribuições ao mesmo branch são **somadas**:

```verilog
I(p, n) <+ V(p, n) / R ;    // corrente resistiva
I(p, n) <+ C * ddt(V(p, n)) ; // corrente capacitiva
// Resultado: I = V/R + C·dV/dt
```

### 10.5 Contribuição indireta

Resolve para um valor de branch que satisfaz uma equação:

```verilog
// "Encontrar V(out) tal que V(in) - Vref == 0"
V(out) : V(in) - Vref == 0 ;

// Opamp ideal: V(out) tal que V(inp) - V(inn) == 0
V(out) : V(inp, inn) == 0 ;
```

Sintaxe:

```
target_access : condition_expression == 0 ;
```

---

## 11. Funções de Acesso

### 11.1 Acesso a potencial

```verilog
V(a)        // potencial do nó a (referência ao ground)
V(a, b)     // diferença de potencial: V(a) - V(b)
V(br)       // potencial do branch br
```

### 11.2 Acesso a fluxo

```verilog
I(a)        // corrente entrando na porta a
I(a, b)     // corrente pelo branch (a, b): de a para b
I(br)       // corrente pelo branch br
```

### 11.3 Acesso a fluxo de porta

```verilog
I(<a>)      // corrente contribuída à porta a pelo módulo corrente
```

A sintaxe `<>` acessa apenas a contribuição do módulo, excluindo hierarquias superiores.

### 11.4 Acesso genérico

O nome da função de acesso é determinado pela nature da discipline:

```verilog
// Para discipline thermal: access Temp (potential), Pwr (flow)
Temp(node_a)
Pwr(node_a, node_b)
```

---

## 12. Funções Matemáticas

### 12.1 Funções aritméticas

| Função | Assinatura | Descrição |
|--------|------------|-----------|
| `abs(x)` | `real → real` | Valor absoluto |
| `min(x, y)` | `real × real → real` | Mínimo |
| `max(x, y)` | `real × real → real` | Máximo |
| `sqrt(x)` | `real → real` | Raiz quadrada (x ≥ 0) |
| `pow(x, y)` | `real × real → real` | x^y |
| `exp(x)` | `real → real` | e^x |
| `ln(x)` | `real → real` | Logaritmo natural (x > 0) |
| `log(x)` | `real → real` | Logaritmo base 10 (x > 0) |
| `ceil(x)` | `real → real` | Arredondamento para cima |
| `floor(x)` | `real → real` | Arredondamento para baixo |
| `hypot(x, y)` | `real × real → real` | √(x² + y²) |

### 12.2 Funções trigonométricas

| Função | Domínio | Contradomínio |
|--------|---------|---------------|
| `sin(x)` | ℝ | [-1, 1] |
| `cos(x)` | ℝ | [-1, 1] |
| `tan(x)` | ℝ \ {π/2 + nπ} | ℝ |
| `asin(x)` | [-1, 1] | [-π/2, π/2] |
| `acos(x)` | [-1, 1] | [0, π] |
| `atan(x)` | ℝ | (-π/2, π/2) |
| `atan2(y, x)` | ℝ × ℝ | (-π, π] |

### 12.3 Funções hiperbólicas

| Função | Domínio |
|--------|---------|
| `sinh(x)` | ℝ |
| `cosh(x)` | ℝ |
| `tanh(x)` | ℝ |
| `asinh(x)` | ℝ |
| `acosh(x)` | [1, ∞) |
| `atanh(x)` | (-1, 1) |

---

## 13. Operadores Analógicos (Filtros)

Operadores analógicos são **stateful** — mantêm estado interno entre avaliações.

> **Restrição de contexto:** Operadores analógicos NÃO podem aparecer dentro de:
> - `if`/`case` com condição que varia durante análise (pode variar entre passos de tempo)
> - `for`/`while`/`repeat` loops (exceto `analog for` com `genvar`)
>
> Razão: O simulador precisa criar instâncias fixas de cada operador durante elaboração.

### 13.1 Derivada temporal — `ddt`

```verilog
ddt(expr)
ddt(expr, abstol)
ddt(expr, nature)
```

Calcula dx/dt. O simulador usa o método de integração numérica (Backward Euler, Trapezoidal, etc.) para computar.

**Uso típico (capacitor):**

```verilog
I(p, n) <+ C * ddt(V(p, n)) ;
```

### 13.2 Integral temporal — `idt`

```verilog
idt(expr)
idt(expr, ic)
idt(expr, ic, assert)
idt(expr, ic, assert, abstol)
```

| Parâmetro | Tipo | Descrição |
|-----------|------|-----------|
| `expr` | real | Expressão a integrar |
| `ic` | real | Condição inicial (valor em t=0) |
| `assert` | integer | Se ≠ 0, força IC (reset do integrador) |
| `abstol` | real | Tolerância absoluta |

**Uso típico (indutor):**

```verilog
V(p, n) <+ L * ddt(I(p, n)) ;
// Equivalente: I(p,n) <+ idt(V(p,n)/L, I0) ;
```

### 13.3 Derivada parcial — `ddx`

```verilog
ddx(expr, unknown)
```

Calcula ∂expr/∂unknown onde `unknown` é um potencial ou fluxo de nó/branch. Usado para computar Jacobianos explícitos.

```verilog
real gm ;
gm = ddx(I(d, s), V(g, s)) ;   // transcondutância
```

### 13.4 Atraso absoluto — `absdelay`

```verilog
absdelay(expr, delay)
absdelay(expr, delay, max_delay)
```

| Parâmetro | Tipo | Descrição |
|-----------|------|-----------|
| `expr` | real | Sinal a atrasar |
| `delay` | real | Atraso em segundos (≥ 0) |
| `max_delay` | real | Atraso máximo (para alocação de buffer) |

```verilog
V(out) <+ absdelay(V(in), td) ;  // linha de transmissão ideal
```

### 13.5 Transição — `transition`

```verilog
transition(expr)
transition(expr, td)
transition(expr, td, rise_time)
transition(expr, td, rise_time, fall_time)
transition(expr, td, rise_time, fall_time, time_tol)
```

Suaviza transições discretas com rampas lineares. Gera breakpoints no simulador.

| Parâmetro | Tipo | Default | Descrição |
|-----------|------|---------|-----------|
| `expr` | real | — | Expressão piecewise-constant |
| `td` | real | 0 | Atraso |
| `rise_time` | real | — | Tempo de subida |
| `fall_time` | real | rise_time | Tempo de descida |
| `time_tol` | real | mínimo do simulador | Tolerância temporal |

### 13.6 Slew — `slew`

```verilog
slew(expr)
slew(expr, max_pos_slew_rate)
slew(expr, max_pos_slew_rate, max_neg_slew_rate)
```

Limita a taxa de variação (dV/dt) do sinal.

| Parâmetro | Tipo | Default | Descrição |
|-----------|------|---------|-----------|
| `expr` | real | — | Sinal de entrada |
| `max_pos_slew_rate` | real | ∞ | Taxa máxima positiva (V/s) |
| `max_neg_slew_rate` | real | -max_pos | Taxa máxima negativa (V/s) |

### 13.7 Exponencial limitada — `limexp`

```verilog
limexp(expr)
```

Equivalente a `exp(expr)` mas com limiting interno para auxiliar convergência do Newton-Raphson. O simulador limita internamente o argumento entre iterações.

**Uso:** Obrigatório para diodos e outros dispositivos com exponenciais que divergem:

```verilog
I(p, n) <+ Is * (limexp(V(p,n) / Vt) - 1.0) ;
```

### 13.8 Último cruzamento — `last_crossing`

```verilog
last_crossing(expr)
last_crossing(expr, direction)
```

Retorna o tempo do último cruzamento por zero da expressão.

| Parâmetro | Valor | Descrição |
|-----------|-------|-----------|
| `direction` | +1 | Cruzamento ascendente |
| `direction` | -1 | Cruzamento descendente |
| `direction` | 0 (default) | Qualquer direção |

### 13.9 Filtros Laplace (domínio s)

Funções de transferência no domínio de Laplace, avaliadas em transient.

#### `laplace_zp` — zeros e polos

```verilog
laplace_zp(expr, ζ, ρ)
```

- `ζ` = array de zeros (pares: {real, imag})
- `ρ` = array de polos (pares: {real, imag})

H(s) = k · ∏(s - zᵢ) / ∏(s - pᵢ)

#### `laplace_zd` — zeros, coeficientes do denominador

```verilog
laplace_zd(expr, ζ, d)
```

- `ζ` = array de zeros
- `d` = coeficientes do polinômio denominador [d₀, d₁, d₂, ...] → d₀ + d₁s + d₂s² + ...

#### `laplace_np` — coeficientes do numerador, polos

```verilog
laplace_np(expr, n, ρ)
```

- `n` = coeficientes do polinômio numerador
- `ρ` = array de polos

#### `laplace_nd` — coeficientes do numerador, coeficientes do denominador

```verilog
laplace_nd(expr, n, d)
```

- `n` = coeficientes do numerador [n₀, n₁, ...]
- `d` = coeficientes do denominador [d₀, d₁, ...]

H(s) = (n₀ + n₁s + n₂s² + ...) / (d₀ + d₁s + d₂s² + ...)

### 13.10 Filtros Z (domínio z)

Funções de transferência no domínio Z para sistemas amostrados.

#### `zi_zp` — zeros e polos

```verilog
zi_zp(expr, ζ, ρ, T)
```

- `T` = período de amostragem

#### `zi_zd` — zeros, coeficientes do denominador

```verilog
zi_zd(expr, ζ, d, T)
```

#### `zi_np` — coeficientes do numerador, polos

```verilog
zi_np(expr, n, ρ, T)
```

#### `zi_nd` — coeficientes do numerador, coeficientes do denominador

```verilog
zi_nd(expr, n, d, T)
```

H(z) = (n₀ + n₁z⁻¹ + n₂z⁻² + ...) / (d₀ + d₁z⁻¹ + d₂z⁻² + ...)

### 13.11 Estímulo AC — `ac_stim`

```verilog
ac_stim()
ac_stim(analysis_name)
ac_stim(analysis_name, mag)
ac_stim(analysis_name, mag, phase)
```

Retorna o estímulo de pequeno sinal para análise AC. Em DC/transient retorna 0.

```verilog
V(p, n) <+ vdc + ac_stim("ac", 1.0, 0.0) ;
```

---

## 14. Funções de Ruído

Funções de ruído contribuem espectro de potência durante análise de ruído (small-signal). Em DC e transient, retornam 0.

### 14.1 Ruído branco — `white_noise`

```verilog
white_noise(power)
white_noise(power, name)
```

PSD constante em toda faixa de frequência.

| Parâmetro | Tipo | Descrição |
|-----------|------|-----------|
| `power` | real | Densidade espectral de potência (A²/Hz ou V²/Hz) |
| `name` | string | Identificador para o relatório de ruído |

```verilog
// Ruído térmico de resistor (Johnson-Nyquist)
I(p, n) <+ white_noise(4.0 * `P_K * $temperature / R, "thermal") ;
```

### 14.2 Ruído flicker (1/f) — `flicker_noise`

```verilog
flicker_noise(power, exponent)
flicker_noise(power, exponent, name)
```

PSD proporcional a 1/f^exponent.

| Parâmetro | Tipo | Descrição |
|-----------|------|-----------|
| `power` | real | PSD a 1 Hz |
| `exponent` | real | Expoente (tipicamente 1.0) |
| `name` | string | Identificador |

```verilog
// Ruído 1/f de MOSFET
I(d, s) <+ flicker_noise(Kf * Id / (Cox * L * L), Af, "flicker") ;
```

### 14.3 Tabela de ruído — `noise_table`

```verilog
noise_table(vector)
noise_table(vector, name)
```

Interpolação linear em tabela de {frequência, PSD}:

```verilog
noise_table({1.0, 1e-20, 100.0, 1e-22, 1e6, 1e-24}, "measured")
```

O `vector` contém pares (f₁, psd₁, f₂, psd₂, ...).

### 14.4 Tabela de ruído log — `noise_table_log`

```verilog
noise_table_log(vector)
noise_table_log(vector, name)
```

Como `noise_table` mas com interpolação em escala log-log.

---

## 15. Eventos Analógicos

### 15.1 Detecção de cruzamento — `cross`

```verilog
@(cross(expr))
@(cross(expr, direction))
@(cross(expr, direction, time_tol))
@(cross(expr, direction, time_tol, expr_tol))
```

| Parâmetro | Valor | Descrição |
|-----------|-------|-----------|
| `direction` | +1 | Cruzamento ascendente (de negativo para positivo) |
| `direction` | -1 | Cruzamento descendente |
| `direction` | 0 (default) | Qualquer direção |
| `time_tol` | real | Tolerância temporal para precisão do cruzamento |
| `expr_tol` | real | Tolerância na expressão |

```verilog
@(cross(V(clk) - vth, +1))
    count = count + 1 ;
```

O simulador ajusta o timestep para localizar o cruzamento com a tolerância especificada.

### 15.2 Timer — `timer`

```verilog
@(timer(start_time))
@(timer(start_time, period))
@(timer(start_time, period, time_tol))
```

| Parâmetro | Tipo | Descrição |
|-----------|------|-----------|
| `start_time` | real | Tempo absoluto do primeiro disparo |
| `period` | real | Período de repetição (0 = disparo único) |
| `time_tol` | real | Tolerância temporal |

```verilog
@(timer(0, T_clk))
    phase = !phase ;
```

### 15.3 Detecção de limiar — `above`

```verilog
@(above(expr))
@(above(expr, time_tol))
@(above(expr, time_tol, expr_tol))
```

Gera evento quando `expr` cruza zero de baixo para cima (rising edge de `expr > 0`). Retorna 1 se `expr > 0`, senão 0.

```verilog
@(above(V(in) - Vth))
    $display("Threshold exceeded at %g", $abstime) ;
```

### 15.4 Eventos globais de análise

```verilog
@(initial_step)     // primeiro passo de qualquer análise
@(initial_step("ac"))   // primeiro passo da análise AC
@(initial_step("tran")) // primeiro passo da análise transient

@(final_step)       // último passo de qualquer análise
@(final_step("tran"))
```

Nomes de análise válidos:
- `"ac"` — análise AC (small-signal)
- `"dc"` — análise DC (operating point)
- `"tran"` — análise transient
- `"noise"` — análise de ruído
- `"static"` — alias para `"dc"`
- `"ic"` — initial condition

---

## 16. System Tasks e System Functions

### 16.1 Ambiente de simulação

| Função | Retorno | Descrição |
|--------|---------|-----------|
| `$temperature` | real | Temperatura de simulação em Kelvin |
| `$vt` | real | Tensão térmica: kT/q (≈ 25.86 mV a 300 K) |
| `$vt(T)` | real | Tensão térmica na temperatura T |
| `$abstime` | real | Tempo absoluto de simulação (segundos) |
| `$realtime` | real | Tempo de simulação em unidades de `timescale` |

### 16.2 Controle de simulação

| Task/Function | Descrição |
|---------------|-----------|
| `$bound_step(max_dt)` | Sugere timestep máximo ao simulador |
| `$discontinuity(order)` | Sinaliza descontinuidade de ordem n |
| `$limit(access, limiter, ...)` | Aplica limiting a uma variável de branch |
| `$finish` | Termina a simulação |
| `$finish(n)` | Termina com nível de diagnóstico n |
| `$stop` | Pausa a simulação (modo interativo) |

#### `$discontinuity`

```verilog
$discontinuity(0) ;   // valor descontínuo (salto)
$discontinuity(1) ;   // primeira derivada descontínua (kink)
$discontinuity(2) ;   // segunda derivada descontínua
$discontinuity(-1) ;  // não remove breakpoint anterior
```

#### `$limit`

```verilog
$limit(V(p,n), "pnjlim", Vt, Vcrit) ;   // limiting de junção PN
$limit(V(g,s), "fetlim", Vto) ;          // limiting de FET
```

Limiters padrão:

| Limiter | Parâmetros | Uso |
|---------|------------|-----|
| `"pnjlim"` | Vt, Vcrit | Junção PN (diodo, BJT B-E, B-C) |
| `"fetlim"` | Vto | Gate-source FET |

### 16.3 Consulta de análise

```verilog
analysis("dc")      // retorna 1 se análise DC ativa, senão 0
analysis("ac")
analysis("tran")
analysis("noise")
analysis("ic")
analysis("static")
```

### 16.4 Consulta de parâmetros

```verilog
$param_given(param_name)     // 1 se parâmetro foi explicitamente fornecido, 0 se default
$port_connected(port_name)   // 1 se porta conectada na instanciação
```

### 16.5 Parâmetros do simulador

```verilog
$simparam("gmin")             // retorna gmin do simulador
$simparam("gmin", 1e-12)      // com valor default se não encontrado
$simparam("tnom")
$simparam("imax")
$simparam("imelt")
$simparam("sourceScaleFactor")
$simparam("minr")
$simparam("scale")
$simparam("shrink")
```

### 16.6 Saída e diagnóstico

```verilog
$display("format_string", arg1, arg2, ...) ;
$strobe("format_string", arg1, arg2, ...) ;   // print ao final do timestep
$write("format_string", arg1, ...) ;           // sem newline
$monitor("format_string", arg1, ...) ;         // a cada mudança

$debug("format_string", arg1, ...) ;           // output de debug (pode ser ignorado)

$warning("format_string", arg1, ...) ;         // warning não-fatal
$error("format_string", arg1, ...) ;           // erro não-fatal
$fatal(finish_num, "format_string", arg1, ...) ; // erro fatal — termina simulação
```

**Format specifiers:**

| Spec | Tipo |
|------|------|
| `%d`, `%0d` | Inteiro decimal |
| `%b` | Binário |
| `%o` | Octal |
| `%h`, `%x` | Hexadecimal |
| `%e` | Real notação científica |
| `%f` | Real ponto fixo |
| `%g` | Real (automático) |
| `%r` | Real (raw/full precision) |
| `%s` | String |
| `%m` | Nome hierárquico do módulo |
| `%l` | Nome da biblioteca |
| `%%` | Literal % |

### 16.7 Funções de arquivo

```verilog
integer fd ;
fd = $fopen("filename", "mode") ;   // "w", "r", "a"
$fclose(fd) ;
$fwrite(fd, "format", ...) ;
$fdisplay(fd, "format", ...) ;
$fstrobe(fd, "format", ...) ;
$fgets(str, fd) ;
$fscanf(fd, "format", var1, ...) ;
$feof(fd) ;
```

### 16.8 Funções de conversão

```verilog
$clog2(n)          // ⌈log₂(n)⌉
$rtoi(real_val)    // real → integer (trunca)
$itor(int_val)     // integer → real
$realtobits(r)     // real → bit pattern (64 bits)
$bitstoreal(b)     // bit pattern → real
```

### 16.9 Random

```verilog
$random               // integer aleatório
$random(seed)          // com semente
$rdist_uniform(seed, start, end)
$rdist_normal(seed, mean, std_dev)
$rdist_exponential(seed, mean)
$rdist_poisson(seed, mean)
$rdist_chi_square(seed, dof)
$rdist_t(seed, dof)
$rdist_erlang(seed, k, mean)
```

---

## 17. Controle de Fluxo

### 17.1 Condicional — `if`/`else`

```verilog
if (condition)
    statement ;
else if (condition)
    statement ;
else
    statement ;
```

### 17.2 Case

```verilog
case (expression)
    value1 : statement ;
    value2, value3 : statement ;
    default : statement ;
endcase
```

### 17.3 Loops

```verilog
// Repeat
repeat (count)
    statement ;

// While
while (condition)
    statement ;

// For (procedural)
for (init ; condition ; update)
    statement ;
```

> Operadores analógicos (ddt, idt, etc.) **NÃO** podem ser usados dentro de loops procedurais (`for`, `while`, `repeat`) nem dentro de `if`/`case` com condições que variam em runtime.

### 17.4 Analog `for` com `genvar`

Exceção à regra acima — `genvar` é expandido em elaboração:

```verilog
genvar i ;
analog begin
    for (i = 0 ; i < N ; i = i + 1) begin
        I(pin[i], gnd) <+ V(pin[i], gnd) / R[i] ;
    end
end
```

O `analog for` com `genvar` cria N instâncias estáticas de cada operador — é safe para `ddt`/`idt`.

---

## 18. Generate

### 18.1 Generate block

```verilog
generate
    genvar i ;
    for (i = 0 ; i < 4 ; i = i + 1) begin : stage
        // instanciações ou declarações replicadas
    end
endgenerate
```

### 18.2 Conditional generate

```verilog
generate
    if (PARAM > 0) begin : pos_block
        // ...
    end else begin : neg_block
        // ...
    end
endgenerate
```

---

## 19. Funções e Tasks Definidas pelo Usuário

### 19.1 Analog function

```verilog
analog function real func_name ;
    input arg1, arg2 ;
    real arg1, arg2 ;
    begin
        func_name = arg1 + arg2 ;  // valor de retorno = nome da função
    end
endfunction
```

**Restrições:**
- Não pode conter operadores analógicos (ddt, idt, etc.)
- Não pode acessar sinais do circuito (V, I)
- Não pode chamar system tasks ($display, etc.)
- Apenas funções matemáticas puras

**Tipos de retorno:** `real` ou `integer`.

### 19.2 Analog task (extensão — limitado)

Algumas implementações suportam:

```verilog
task task_name ;
    // ...
endtask
```

---

## 20. Operadores da Linguagem

### 20.1 Operadores aritméticos

| Operador | Descrição |
|----------|-----------|
| `+` | Adição (unário: identidade) |
| `-` | Subtração (unário: negação) |
| `*` | Multiplicação |
| `/` | Divisão |
| `%` | Módulo (inteiros apenas) |
| `**` | Exponenciação |

### 20.2 Operadores relacionais

| Operador | Descrição |
|----------|-----------|
| `==` | Igualdade |
| `!=` | Desigualdade |
| `<`  | Menor que |
| `<=` | Menor ou igual |
| `>`  | Maior que |
| `>=` | Maior ou igual |

### 20.3 Operadores lógicos

| Operador | Descrição |
|----------|-----------|
| `&&` | AND lógico |
| `\|\|` | OR lógico |
| `!` | NOT lógico |

### 20.4 Operadores bit-a-bit (inteiros)

| Operador | Descrição |
|----------|-----------|
| `&` | AND |
| `\|` | OR |
| `^` | XOR |
| `~` | NOT |
| `~^`, `^~` | XNOR |

### 20.5 Operadores de shift (inteiros)

| Operador | Descrição |
|----------|-----------|
| `<<` | Shift left |
| `>>` | Shift right |
| `<<<` | Shift left aritmético |
| `>>>` | Shift right aritmético |

### 20.6 Operador condicional (ternário)

```verilog
result = condition ? value_if_true : value_if_false ;
```

### 20.7 Concatenação

```verilog
{a, b, c}        // concatenação de bits
{N{a}}           // replicação
```

### 20.8 Precedência (maior para menor)

1. `()` — agrupamento
2. `!`, `~`, `+`, `-` (unários)
3. `**`
4. `*`, `/`, `%`
5. `+`, `-`
6. `<<`, `>>`, `<<<`, `>>>`
7. `<`, `<=`, `>`, `>=`
8. `==`, `!=`
9. `&`
10. `^`, `~^`
11. `|`
12. `&&`
13. `||`
14. `?:` (ternário)

---

## 21. Instanciação de Módulos

```verilog
module_name #( .param1(value1), .param2(value2) )
    instance_name ( .port1(net1), .port2(net2) ) ;

// Conexão posicional
module_name instance_name ( net1, net2, net3 ) ;

// Array de instâncias
module_name instance_name [0:N-1] ( ... ) ;
```

---

## 22. Hierarquia de Nós e Escopo

### 22.1 Referência hierárquica

```verilog
instance_name.signal_name
top.sub1.sub2.node
```

### 22.2 Escopo de variáveis

- **Parâmetros:** Escopo do módulo, visíveis externamente.
- **Variáveis (`real`, `integer`):** Escopo do módulo ou bloco nomeado.
- **Natures/Disciplines:** Escopo global.
- **Genvar:** Escopo do generate block.

---

## 23. Conectividade Mixed-Signal (AMS)

> Esta seção é específica de Verilog-AMS (não Verilog-A puro).

### 23.1 Connect modules

```verilog
connectmodule a2d (input electrical ain, output logic dout) ;
    parameter real vth = 1.5 ;
    assign dout = V(ain) > vth ? 1'b1 : 1'b0 ;
endmodule
```

### 23.2 Connect rules

```verilog
connectrules rules_name ;
    connect a2d_auto input electrical, output logic ;
    connect d2a_auto input logic, output electrical ;
endconnectrules
```

### 23.3 Discipline resolution

Quando sinais de diferentes disciplines se encontram, o simulador insere automaticamente connect modules conforme as connect rules ativas.

### 23.4 `wreal`

Wire resolvido em tempo real — um sinal contínuo que usa resolução de driver:

```verilog
wreal sig ;
```

---

## 24. Semântica de Avaliação

### 24.1 Modelo de execução do bloco `analog`

1. O simulador inicia com condições iniciais (DC operating point ou IC).
2. A cada passo de tempo, o bloco `analog` é avaliado.
3. Contribuições são acumuladas; equações constitutivas são formadas.
4. O simulador resolve o sistema não-linear (Newton-Raphson).
5. Se convergiu, avança para o próximo passo; senão, re-avalia.

### 24.2 Contribuições são aditivas

```verilog
I(p, n) <+ G * V(p, n) ;      // contribuição 1
I(p, n) <+ C * ddt(V(p, n)) ; // contribuição 2
// Equação final: I = G·V + C·dV/dt
```

### 24.3 Branches não contribuídos

Se nenhuma contribuição é feita a um branch declarado, a equação padrão é:
- Para **potencial:** V(br) = 0 (curto-circuito)
- Para **fluxo:** I(br) = 0 (circuito aberto)

### 24.4 Restrições do operador `<+`

- O lado esquerdo deve ser um **access function** aplicado a um branch ou par de nós.
- Não se pode contribuir simultaneamente potencial e fluxo ao mesmo branch.
- Contribuições diretas e indiretas ao mesmo branch são proibidas.

---

## 25. Atributos e Anotações

### 25.1 Syntax

```verilog
(* attribute_name = value *)
(* attr1 = "string", attr2 = 42 *)
```

### 25.2 Atributos padrão para parâmetros

| Atributo | Tipo | Descrição |
|----------|------|-----------|
| `desc` | string | Descrição legível |
| `units` | string | Unidade física |
| `type` | string | `"model"` ou `"instance"` |
| `info` | string | Informação adicional |

### 25.3 Atributos de módulo

```verilog
(* ams_language = "Verilog-AMS" *)
(* model_type = "resistor" *)
module resistor ( ... ) ;
```

---

## 26. Apêndice A — Gramática Formal (BNF simplificado)

```bnf
source_text
    ::= { description }

description
    ::= module_declaration
    |   nature_declaration
    |   discipline_declaration
    |   paramset_declaration

module_declaration
    ::= { attribute_instance } 'module' module_identifier
        [ '(' list_of_ports ')' ] ';'
        { module_item }
        'endmodule'

module_item
    ::= port_declaration
    |   parameter_declaration
    |   variable_declaration
    |   branch_declaration
    |   analog_block
    |   analog_function_declaration
    |   module_instantiation
    |   generate_region

analog_block
    ::= 'analog' statement

analog_initial_block
    ::= 'analog' 'initial' statement

statement
    ::= 'begin' [ ':' block_identifier ] { statement } 'end'
    |   contribution_statement
    |   indirect_contribution_statement
    |   procedural_assignment
    |   conditional_statement
    |   case_statement
    |   loop_statement
    |   event_controlled_statement
    |   system_task_call
    |   ';'

contribution_statement
    ::= branch_access '<+' expression ';'

indirect_contribution_statement
    ::= branch_access ':' expression '==' expression ';'

branch_access
    ::= access_identifier '(' node_or_branch { ',' node_or_branch } ')'

event_controlled_statement
    ::= '@(' event_expression ')' statement

event_expression
    ::= 'initial_step' [ '(' string { ',' string } ')' ]
    |   'final_step'   [ '(' string { ',' string } ')' ]
    |   'cross' '(' expression { ',' expression } ')'
    |   'timer' '(' expression { ',' expression } ')'
    |   'above' '(' expression { ',' expression } ')'

parameter_declaration
    ::= 'parameter' type_identifier parameter_identifier '=' expression
        [ range_constraint ] ';'

range_constraint
    ::= 'from' range_expression { 'exclude' exclude_expression }
    |   'exclude' exclude_expression

range_expression
    ::= open_or_close expression ':' expression close_or_open

nature_declaration
    ::= 'nature' nature_identifier [ ':' parent_nature ] ';'
        { nature_attribute }
        'endnature'

discipline_declaration
    ::= 'discipline' discipline_identifier ';'
        [ 'potential' nature_identifier ';' ]
        [ 'flow' nature_identifier ';' ]
        [ 'domain' domain_type ';' ]
        'enddiscipline'
```

---

## 27. Apêndice B — Tabela de Referência Rápida

### Operadores analógicos

| Operador | Domínio | Stateful | Restrições |
|----------|---------|----------|------------|
| `ddt(x)` | Tempo | Sim | Não em if/case variável |
| `idt(x,ic,a,t)` | Tempo | Sim | Não em if/case variável |
| `ddx(x,u)` | — | Não | Nenhuma |
| `absdelay(x,d,m)` | Tempo | Sim | Não em if/case variável |
| `transition(x,...)` | Tempo | Sim | Não em if/case variável |
| `slew(x,...)` | Tempo | Sim | Não em if/case variável |
| `limexp(x)` | — | Sim | Não em if/case variável |
| `last_crossing(x,d)` | Tempo | Sim | Não em if/case variável |
| `laplace_*(x,...)` | s | Sim | Não em if/case variável |
| `zi_*(x,...,T)` | z | Sim | Não em if/case variável |
| `ac_stim(...)` | AC | Não | — |

### System functions (retorno)

| Função | Tipo Retorno | Análise |
|--------|-------------|---------|
| `$temperature` | real (K) | Todas |
| `$vt` / `$vt(T)` | real (V) | Todas |
| `$abstime` | real (s) | Todas |
| `analysis("name")` | integer (0/1) | Todas |
| `$param_given(p)` | integer (0/1) | Elaboração |
| `$port_connected(p)` | integer (0/1) | Elaboração |
| `$simparam("name")` | real | Todas |
| `$simparam("name",d)` | real | Todas |

### Noise functions

| Função | PSD | Dependência |
|--------|-----|-------------|
| `white_noise(p)` | p | Constante |
| `flicker_noise(p,e)` | p/f^e | 1/f^e |
| `noise_table(v)` | PWL | Tabelada |
| `noise_table_log(v)` | PWL log | Tabelada (log-log) |
