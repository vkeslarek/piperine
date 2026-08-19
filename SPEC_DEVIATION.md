# SPEC_DEVIATION.md — Divergências entre o Piperine e o Verilog-AMS LRM 2.4

**Data da auditoria:** 2026-08-19
**Escopo:** capítulos 2–9 do LRM (`VAMS-LRM-2-4.md`, linhas 1214–16559) + spot-checks
seletivos dos anexos (Annex A gramática, Annex B keywords, Annex C subset analógico,
Annex F resolução de disciplines). O capítulo 1 (introdução) é não-normativo e não foi
auditado.

**Método:** auditoria read-only por capítulo, comparando cada seção normativa do LRM
contra o código (`crates/piperine-lang`, `piperine-codegen`, `piperine-solver`,
`piperine-api`), com evidência `arquivo:linha` para cada claim. Os 5 achados mais
críticos (bugs silenciosos) foram **re-verificados manualmente** pelo orquestrador; o
restante é evidência da auditoria read-only, não re-executada ponto a ponto — o status
de verificação de cada item está anotado.

**Legenda:**

| Símbolo | Significado |
|---|---|
| ❗ | Divergência semântica — o código faz algo DIFERENTE do que o LRM manda |
| ❌ | Construção ausente (não parseia, ou parseia e não baixa) |
| ⚠️ | Parcial — o núcleo funciona, partes faltam |
| ✅ | Conforme |
| ☠️ | **Drop silencioso** — viola o contrato fail-loud do CLAUDE.md (a classe mais grave) |

---

## 0. Sumário executivo

O Piperine cobre a **maior parte da gramática Annex A** (lexer completo com os 212
keywords da Table B.1, literais based/sized com x/z, expoentes + fatores de escala SI,
atributos, operadores com a tabela de precedência exata) e o **núcleo semântico
analógico** (contribuições, probes V/I, ddt/idt/transition/slew/laplace/zi, ruído,
eventos `cross`/`above`/`timer`/`initial_step`, resolução de disciplines Basic,
scheduler misto conservativo, homotopia, MNA).

Os problemas concentram-se em **cinco classes**:

1. **Drops silenciosos** (☠️, seção 1) — construções que parseiam e são descartadas
   sem erro. São violações diretas do contrato fail-loud e a prioridade máxima de
   correção. O mais grave é um **bug real** (§6.9.1: múltiplos blocos `analog` — só o
   último compila), da mesma família do bug multi-`always` corrigido em `f75beb4d`.
2. **Divergências semânticas** (❗, seção 2) — comportamento errado, não ausência:
   `=`/`<=` digitais idênticos, `log` errado, `limexp` sem limiting, alvo `potential()`
   virando fonte de fluxo.
3. **Construções ausentes** (❌, seção 3) — famílias gramaticais inteiras: switch
   branches, contribuições indiretas, referências hierárquicas, funções de usuário,
   `reg`/`wire`/`wreal`, driver-receiver segregation.
4. **Argumentos de operadores ignorados silenciosamente** (⚠️☠️, seção 4) — `idt`
   assert, `idtmod` offset, `absdelay` maxdelay, `zi` τ/t0, tolerâncias de eventos.
5. **Desvios registrados (disclosed)** (seção 5) — conformes por divulgação: PRNG
   não-1364, resolução Basic-only, scheduler Verilator-style.

E uma **sexta classe positiva**: as extensões Piperine além do LRM (seção 6).

---

## 1. Drops silenciosos — violações do contrato fail-loud ☠️

Regra violada (CLAUDE.md): *"When something cannot be faithfully lowered, fail loud —
never silently emit 0.0 or a no-op."* Cada item abaixo parseia (ou elabora) e é
descartado sem diagnóstico.

### 1.1 ☠️ §6.9.1 — Múltiplos blocos `analog`: só o ÚLTIMO compila **[RE-VERIFICADO — BUG REAL]**

- **LRM diz:** §6.9.1 "Concatenation of analog blocks" — múltiplos blocos `analog` de
  um módulo são concatenados como se fossem um só (mesmo espírito do `f75beb4d` para
  `always`).
- **Código faz:** em `lower_bodies`, o braço analógico faz
  `bodies[i].analog = Some(AnalogBody { … })` **incondicionalmente** por behavior —
  o segundo bloco sobrescreve o primeiro. O braço digital, em contraste, appenda
  (`body.stmts.extend(stmts)`).
- **Evidência:** `crates/piperine-codegen/src/resolve/pom/body.rs:436-445` (assign
  incondicional) vs `:415-433` (append digital). Provado empiricamente na auditoria:
  módulo com `I(a) <+ 1.0;` seguido de `I(a) <+ -V(a)/2k` resolve como se só o
  segundo existisse; a ordem invertida muda o resultado.
- **Impacto:** modelos reais com mais de um bloco `analog` simulam **faltando
  comportamento** — resultado numericamente errado, sem nenhum aviso. Pior classe
  possível: silencioso, dependente de ordem, e não detectável por teste de
  elaboração.
- **Correção sugerida:** acumular as contribuições/statements de cada bloco em uma
  lista e concatenar antes de fechar o `AnalogBody` (a mesma forma que o braço
  digital já usa). Teste de discriminação: módulo com dois blocos onde cada um
  contribui uma corrente; asserir que o OP reflete a soma. Sem o fix, o teste falha.
- **Status de verificação:** re-verificado manualmente pelo orquestrador (grep do
  assign incondicional + leitura do braço digital).

### 1.2 ☠️ §7.3.5 — `@(cross/above/timer)` em bloco digital: evento ignorado, corpo vira combinacional

- **LRM diz:** §7.3.5 "Detecting continuous events in a discrete context" — um bloco
  digital pode aguardar um evento analógico (`@(cross(V(x)-2.5, 1))`) e só então
  executar o corpo.
- **Código faz:** no compile digital, todo `EventControl` não-clocked tem o **evento
  descartado** e o corpo dobrado na lista combinacional — executa **todo passo**,
  não quando o evento dispara.
- **Evidência:** `crates/piperine-codegen/src/kernel/digital/compile.rs:126-137`
  (`Stmt::EventControl { body, .. } => comb_stmts.push(body.as_ref())`); watch terms
  só existem para blocos clocked (`:184`).
- **Impacto:** qualquer design que espere sincronizar a lógica digital com um
  cruzamento analógico roda a lógica continuamente — resultados divergentes,
  silenciosos.
- **Correção sugerida:** baixar o evento analógico para um watch term (mesmo
  mecanismo dos clocked) e rotear o corpo pelo caminho `fired`; ou, no mínimo,
  falhar loud com "evento contínuo em bloco digital ainda não suportado".
- **Status de verificação:** re-verificado manualmente pelo orquestrador.

### 1.3 ☠️ §6.5.3/§7.3.6.4/§8.5.3.1 — `assign` contínuo (module-level) é no-op silencioso

- **LRM diz:** §8.5.3.1/§8.5.3.2 — `assign net = expr;` no corpo do módulo é uma
  atribuição contínua; é também o único driver de um port `wreal` (§6.5.3) e de vars
  analógicas lidas digitalmente (§7.3.6.4).
- **Código faz:** `ModuleItem::ContinuousAssign(_)` está no braço no-op do elaborador.
- **Evidência:** `crates/piperine-lang/src/elab/lower/vams.rs:294` (no-op arm),
  zero consumidores em qualquer outro lugar.
- **Impacto:** `wreal out; assign out = V(in);` compila e **não drive nada**; o nó
  fica flutuante. Três seções do LRM mortas de uma vez.
- **Correção sugerida:** baixar para o corpo combinacional digital (mesma forma do
  `always @(*)`) quando digital, e para um contribution-style path quando analógico;
  enquanto não existir, **falhar loud**.
- **Status de verificação:** re-verificado manualmente pelo orquestrador.

### 1.4 ☠️ §3.6.4 — `ground gnd;` parseia e é descartado; ground é por lista mágica de nomes

- **LRM diz:** §3.6.4 "Ground declaration" — `ground gnd;` declara o nó de referência
  (inclusive com `[range]` e disciplina opcional).
- **Código faz:** a declaração é descartada no elaborador; o ground é reconhecido por
  uma lista hard-coded de nomes (`gnd`, `GND`, `vss`, `VSS`, `0`).
- **Evidência:** `vams.rs:288` (no-op) e `crates/piperine-lang/src/pom/net_type.rs:10-15`
  (`is_ground` por nomes).
- **Impacto:** `ground foo;` não faz nada (silencioso); `gnd` é ground **sem**
  declaração; `ground [range]`/`ground discipline` falham parse.
- **Correção sugerida:** registrar o nome declarado no POM e consultar `is_ground`
  contra o registro (com a lista atual como fallback de compatibilidade), ou falhar
  loud na declaração.
- **Status de verificação:** re-verificado manualmente pelo orquestrador.

### 1.5 ☠️ §5.10.2 — `final_step` é no-op silencioso; listas de análise descartadas

- **LRM diz:** §5.10.2 — `@(final_step)` executa o corpo ao fim de cada análise; as
  formas com lista (`initial_step("dc", "tran")`) restringem a quais análises o
  evento se aplica.
- **Código faz:** o corpo de `final_step` é descartado (apenas um diagnóstico interno
  é empurrado); o argumento de lista de análise de `initial_step("dc")` é parseado e
  ignorado.
- **Evidência:** `crates/piperine-codegen/src/flatten/analog.rs` ~`:1040` (arm
  FinalStep) e `:82-98` (listas descartadas).
- **Impacto:** lógica de final-de-análise (limpeza, relatórios) nunca roda.
- **Correção sugerida:** implementar o evento (dispara após o último accept) ou
  falhar loud; aceitar a lista e filtrar por análise.

### 1.6 ☠️ §9.2 — System-task calls desconhecidas em corpo analógico caem em `_ => {}`

- **LRM diz:** §9.2 — tasks desconhecidas não têm semântica; um simulador não deve
  inventá-las.
- **Código faz:** no walk analógico, qualquer task call não reconhecida cai no braço
  `_ => {}` — silenciosa. O caminho digital é loud-unsupported.
- **Evidência:** `flatten/analog.rs:686` (arm `_ => {}`); contraste:
  `emit/stmt.rs:151` (digital loud).
- **Impacto:** um typo em `$display`-like ou uma task não suportada some sem aviso.
- **Correção sugerida:** erro `unsupported` nomeando a task (mesma mensagem do
  digital).
- **Status de verificação:** auditoria (evidência file:line), não re-executado.

### 1.7 ☠️ §3.6.3.2 — `nodeset` (`electrical a = 5.0;`) parseia e some

- **LRM diz:** §3.6.3.2 — o inicializador de declaração de rede é um nodeset (chute
  inicial do Newton).
- **Código faz:** o campo `nodeset` existe na AST e na gramática, mas **nenhum
  consumidor** existe no pipeline.
- **Evidência:** `grammar/item.rs:393-409` / `ast/item.rs:399` (parse) — grep: zero
  leituras fora do frontend.
- **Impacto:** o chute inicial é ignorado — convergência pode degradar (ou falhar)
  em circuitos bistáveis.
- **Correção sugerida:** ligar ao `OpConfig.nodeset` do host (mecanismo já existe em
  `analyses/dc.rs:303`) ou falhar loud até ligar.

### 1.8 ☠️ §3.4.3/§3.2.1 — Atributos parseiam e somem em params, vars e instâncias

- **LRM diz:** §3.4.3 — `(* units = "V" *, desc = … *) parameter …` carrega
  unidade/descrição; §3.2.1 — metadata em declarações de variável de saída.
- **Código faz:** ports/nets/módulos carregam atributos; params, vars e instâncias
  constroem `attributes: Vec::new()`.
- **Evidência:** `vams.rs:177` (param), `:1355` (var), `:662`/`:787` (instância).
- **Impacto:** metadata de modelo (`units` etc.) não chega à reflexão/host; LSP e
  introspecção veem atributo vazio.
- **Correção sugerida:** usar o mesmo caminho de `lower_port_attrs`/`lower_net_attrs`
  (schema-checado, loud) para esses sites.

### 1.9 ☠️ §5.10.3 — `@(expr)` level-events que não são call/ident nunca disparam

- **LRM diz:** §5.10.3 — `@(V(a) > 2.5)` (monitored event por expressão) dispara
  quando a expressão muda.
- **Código faz:** o tradutor de event-expr retorna `Ok(vec![])` para qualquer
  expressão que não seja um call conhecido ou identificador — o evento vira vazio,
  o corpo nunca roda.
- **Evidência:** `flatten/analog.rs:97-98`.
- **Impacto:** gatilhos por expressão em corpo analógico são silenciosamente mortos.
- **Correção sugerida:** suportar o nível por comparação do valor amostrado ou
  falhar loud.

### 1.10 ☠️ §3.5 — `genvar` declarado fora de generate é skipado

- **LRM diz:** §3.5 — `genvar i;` declara índice de generate.
- **Código faz:** a declaração parseia e o elaborador a descarta; genvars só existem
  de fato dentro do `UnrollGenerate`.
- **Evidência:** `vams.rs:289`.
- **Impacto:** menor (o uso legítimo é dentro de generate), mas silencioso.
- **Correção sugerida:** registrar no POM (`Module::genvars()` já existe) ou falhar
  loud fora de contexto válido.

---

## 2. Divergências semânticas ❗ (comportamento errado, não ausência)

### 2.1 ❗ §8.5.3.3/§8.5.3.4 — `=` e `<=` digitais emitem código idêntico **[RE-VERIFICADO]**

- **LRM diz:** blocking (`=`) atualiza imediatamente no Active region; non-blocking
  (`<=`) agenda para o NBA region — a base de toda semântica de registrador Verilog.
- **Código faz:** os dois braços do emit são unificados
  (`S::Assign {..} | S::NonBlockingAssign {..} =>` mesma ação).
- **Evidência:** `crates/piperine-codegen/src/emit/stmt.rs:66-68`.
- **O que salva:** blocos clocked usam o two-phase seq/comb com leitura pré-edge
  (`vars_int_old`), que acidentalmente entrega a semântica certa para o idiom
  `always @(posedge clk) q <= q + 1`. Para lógica combinacional com `<=` e para
  misturas `=`/`<=` no mesmo bloco, a semântica é errada.
- **Impacto:** a divergência de escalonamento digital mais consequente do projeto.
- **Correção sugerida:** separar os braços — `<=` escreve num banco de pending
  aplicado no fim do delta-cycle (NBA region já existe em `regions.rs`); `=` escreve
  direto. Teste de discriminação: bloco com dois `<=` interdependentes (swap).
- **Status de verificação:** re-verificado manualmente pelo orquestrador.

### 2.2 ❗ §4.3 (Table 4-14) — `log` é alias de `ln`, mas o LRM define `log` decimal

- **LRM diz:** Table 4-14 — `log(x)` = logaritmo **decimal** (equivalente a
  `$log10`); `ln(x)` = natural.
- **Código faz:** `"log"` mapeia para a mesma função de `ln`.
- **Evidência:** `crates/piperine-lang/src/math.rs:77`.
- **Impacto:** qualquer modelo usando `log()` recebe o valor errado por um fator
  1/ln(10) ≈ 0.434 — silencioso e numericamente errado.
- **Correção sugerida:** trocar o alias para log10; manter `ln`; teste com valor
  conhecido (`log(100) == 2`).

### 2.3 ❗ §4.5.13 — `limexp` é `min(80).exp()` puro, sem o limiting de saída

- **LRM diz:** §4.5.13 — "limited exponential": além de limitar o argumento, a saída
  é **limitada iteração-a-iteração** (o crescimento entre iterações Newton é
  controlado — é o que faz `limexp` convergir onde `exp` não converge).
- **Código faz:** `limexp(x) = exp(min(x, 80))` — um clamp simples.
- **Evidência:** `math.rs:43-44,62`.
- **Impacto:** perda da propriedade de convergência que motiva a função; modelos que
  dependem dela podem divergir.
- **Correção sugerida:** implementar o limiting com estado entre iterações (o slot
  de estado já existe no catálogo de intrínsecos) ou falhar loud até ter o
  comportamento real.

### 2.4 ❗ §4.4/§5.6.1 — Alvo `potential(p) <+ …` é classificado como fonte de FLUXO

- **LRM diz:** §5.5.1 — `potential`/`flow` são os access functions genéricos; um
  contribution com alvo `potential(p)` é uma fonte de potencial.
- **Código faz:** o flatten mapeia só o literal `"V"` para `NatureKind::Potential`;
  qualquer outro nome — inclusive `"potential"` — cai em `Flow`.
- **Evidência:** `flatten/analog.rs:884-891`.
- **Impacto:** `potential(p) <+ 5.0;` cria uma fonte de corrente fantasma — o
  circuito resolve errado, silenciosamente.
- **Correção sugerida:** mapear `"V"`/`"potential"` → Potential, `"I"`/`"flow"` →
  Flow; teste de discriminação.

### 2.5 ❗ §4.2.6 — `===`/`!==` baixam para igualdade simples

- **LRM diz:** §4.2.6 — case equality trata X/Z como valores (identidade 4-state),
  distinto de `==`/`!=` lógicos.
- **Código faz:** os dois operadores são baixados como `==`/`!=` planos.
- **Evidência:** `crates/piperine-codegen/src/resolve/expr.rs:63-64`.
- **Impacto:** comparações com X/Z dão resultados errados em lógica digital.
- **Correção sugerida:** emitir comparação bit-a-bit dos pares (value, mask) 4-state.

### 2.6 ❗ §4.6 — Funções dependentes de análise quebradas

- **`analysis("bogus")` casa com DC:** nomes desconhecidos mapeiam para kind 0
  (`= dc`), então a condição é verdadeira em DC — o LRM manda erro ou false.
  Evidência: `emit/analog_expr.rs:249-262`.
- **`ac_stim` consome o `analysis_name` como magnitude:** o argumento string de
  análise é mal-interpretado no lowering. Evidência: `resolve/pom/analog_ops.rs:362-369`.
- **`noise_table`/`noise_table_log` ausentes:** keywords existem, o lowerer é
  `real(0.0)` morto. Evidência: `analog_ops.rs:371-376`.
- **`analysis()` sem `"ic"`/`"static"`/`"nodeset"`:** só um nome simples é aceito.
- **Impacto:** condicionais de análise (o idiom de modelos de ruído/AC) comportam-se
  errado.

### 2.7 ❗ §5.9 — Loops de runtime são erro duro em corpos analógico E digital; analog-`for` ausente

- **LRM diz:** §5.9 — `for`/`while`/`repeat` são statements legais; §5.9.3 define o
  analog-`for` (forma com genvar usada para gerar contribuições).
- **Código faz:** `CodegenError` ("runtime loop/delay-control statement") no analog;
  o digital igual; a forma §5.9.3 não existe.
- **Evidência:** `flatten/analog.rs:730-733`, `emit/stmt.rs:153-155`.
- **Impacto:** o LRM usa `for` em analog para somas e modelos paramétricos — fontes
  reais de modelos falham em compilar (loud, ao menos).
- **Correção sugerida:** unroll de `for` com limites de elaboration-constant (a
  forma §5.9.3) como passo de lowering; `while`/`repeat` com limite const igual.

### 2.8 ❗ §3.4.5 — `localparam` é overridável via `defparam`

- **LRM diz:** §3.4.5 — localparam não pode ser sobrescrito (nem por defparam).
- **Código faz:** `Param.is_local` é gravado mas o `ApplyDefparams` nunca o consulta.
- **Evidência:** `elab/lower/defparam.rs` (nenhuma referência a `is_local`).
- **Impacto:** violação de encapsulamento silenciosa.
- **Correção sugerida:** erro loud no defparam apontando para localparam.

### 2.9 ❗ §6.4.2 — Overloading de paramset ausente (overwrite silencioso)

- **LRM diz:** §6.4.2 — vários paramsets com o mesmo nome selecionam por range de
  parâmetro.
- **Código faz:** `HashMap` por nome — o segundo sobrescreve o primeiro.
- **Evidência:** `elab/lower/register.rs:40`.
- **Correção sugerida:** vetor por nome + seleção por ranges; enquanto não, falhar
  loud em duplicata.

### 2.10 ❗ §7.3.2 — X/Z de rede digital lidos no contexto contínuo viram números silenciosos

- **LRM diz:** §7.3.2 — acessar X/Z de uma rede discreta em contexto contínuo é
  **erro** (ou definido pelo simulador de forma explícita, nunca numérica cega).
- **Código faz:** quad→f64 é numérico: X→2.0, Z→3.0, sem erro. Literais based em
  expressão analógica, em contraste, falham loud.
- **Evidência:** `device/digital.rs:17-45`.
- **Correção sugerida:** erro loud na conversão de X/Z (ou constante nomeada, nunca
  o valor bruto).

### 2.11 ❗ §3.11.1 — Compatibilidade por família de natures ausente

- **LRM diz:** §3.11.1 — natures compatíveis por herança e compatibilidade de
  unidades (Voltage e uma derivada highvoltage são compatíveis).
- **Código faz:** compara só nome + domínio (+ `resolveto`); famílias derivadas são
  rejeitadas.
- **Evidência:** `elab/lower/disciplines.rs:205-245`.
- **Impacto:** disciplinas derivadas (o idiom de domínios de tensão) não conectam.

### 2.12 ❗ §7.8.5 — Nome de instância auto-inserida não segue o esquema do LRM; direções ignoradas

- **LRM diz:** §7.8.5 — `SigName__ModuleName__BottomDiscipline` /
  `SigName__InstName__PortName`.
- **Código faz:** `__connect_{inst}_{net}`; a seleção do connect module considera só
  o par de disciplines, ignorando as direções dos ports (Table 7-2) e
  `connect_port_overrides`.
- **Evidência:** `elab/lower/connect.rs:88,178-223`; `ast/item.rs:704-713` (parsed,
  nunca lido).
- **Impacto:** nomes divergentes de outras ferramentas; seleção errada de bridge em
  topologias com direções mistas.

### 2.13 ❗ §8.4.3.3 — Sem quantização de tick digital

- **LRM diz:** §8.4.3.3 — o tempo digital é quantizado em múltiplos do tick (a
  precisão da timescale), e os eventos A2D são arredondados ao grid.
- **Código faz:** tempo digital é `f64` contínuo; o rounding ao grid de precisão
  (agora disponível no POM via `Module::timescale`) não é aplicado.
- **Evidência:** `digital/events.rs:49-60`.
- **Impacto:** tempos de evento com erro de arredondamento contra simuladores que
  quantizam; a semântica de timescale fica pela metade.

### 2.14 ❗ §9.7.3 — `$fatal` é deferido ao accept (LRM: dispara sem checar rejeição)

- **LRM diz:** §9.7.3 — "`$fatal` terminates the simulation without checking whether
  the iteration would be rejected."
- **Código faz:** os diagnostics — inclusive o fatal — só emitem no
  `accept_timestep`.
- **Evidência:** `kernel/analog/kernel.rs:744-759`.
- **Impacto:** um fatal numa iteração rejeitada é silenciosamente engolido.
- **Correção sugerida:** rota separada para fatal (emissão imediata) vs. não-fatais
  (deferidos — a regra de rejeição está certa para `$error/$warning/$info`).

### 2.15 ❗ §9.15 — `$simparam` com chave desconhecida sem default retorna 0.0 silencioso

- **LRM diz:** §9.15 — chave inválida sem default é erro.
- **Código faz:** `0.0`.
- **Evidência:** `emit/analog_expr.rs:223`.
- **Correção sugerida:** erro loud nomeando a chave e as válidas.

---

## 3. Construções ausentes ❌ (famílias inteiras)

### 3.1 ❌ §5.6.5 — Switch branches

- **LRM diz:** §5.6.5 — um contribution de potencial sob `if` num branch que também
  tem contribuição de fluxo modela um switch (relay idiom).
- **Código faz:** `CodegenError::unsupported("conditional potential contribution")`.
- **Evidência:** `flatten/analog.rs:772-774`; os headers congelados documentam
  (`diode.va:45`, `mos.va:30`). O detector de switch existe no POM mas não tem call
  sites (`pom/branch.rs:176-178`).
- **Impacto:** modelos de relé/chave — o idiom mais idiomático do Verilog-A para
  switches — não compilam.
- **Correção sugerida:** a rota de switch: contribuição potencial condicional vira
  elemento tipo chave (o mecanismo de branches já distingue os tipos).

### 3.2 ❌ §5.6.7 — Contribuições indiretas

- **LRM diz:** §5.6.7 — `V(out): V(in) == 0.0;` (forma com condição).
- **Código faz:** a gramática de statement não tem o `:`; parse error.
- **Evidência:** `grammar/stmt.rs:311-342` (só `=`/`<=`/`<+`).

### 3.3 ❌ §6.7/§6.2.1 — Referências hierárquicas e `$root`

- **LRM diz:** §6.7 — nomes hierárquicos (`top.net`, `$root.top.net`) em expressões;
  §6.2.1 define `$root`.
- **Código faz:** nomes pontuados parseiam **apenas** como alvos de `defparam`;
  `$root` não é lexado em lugar nenhum. Acesso hierárquico em corpo analógico existe
  só para 1 nível (`instance.port` via o mapa de ports).
- **Evidência:** `grammar/item.rs:724-731`; `resolve/pom/body.rs:282-300`.
- **Impacto:** cross-module references (OOMR) — um recurso comum em bancadas de
  teste Verilog-AMS — são parse errors.

### 3.4 ❌ §6.3.2 — Overrides posicionais `#(2k)`

- **LRM diz:** §6.3.2 — `res #(2k) r1(...);` por ordem.
- **Código faz:** só nomeado; posicional falha "`#(...)` parameter overrides must be
  named".
- **Evidência:** `vams.rs:637-641`.

### 3.5 ❌ §4.7 — Funções de usuário (declara parses, chamada morre)

- **LRM diz:** §4.7 — `analog function`, definição, argumentos, retorno, chamada.
- **Código faz:** `ModuleItem::Function` é **dropado** na elaboração; `fn_by_name`
  nunca é populado; a chamada morre loud no emit ("should be inlined").
- **Evidência:** `vams.rs:292`, `resolve/pom/expr.rs:262-283`, `emit/analog_expr.rs:70-72`.
- **Impacto:** a pior combinação: declara sem erro, falha no fim do pipeline.
- **Correção sugerida:** no mínimo falhar loud na DECLARAÇÃO enquanto o inlining não
  existir; idealmente implementar o inlining (o caminho já está esboçado).

### 3.6 ❌ §3.7/§7.3.1 — `reg`/`wire`/`wreal`/`tri` não são tipos reais

- **LRM diz:** §3.7 — famílias de tipos de rede do 1364 (`wire`, `tri`, `wreal`…);
  §7.3.1 Table 7-1 — leitura de `reg`/wire em contexto contínuo.
- **Código faz:** a gramática de net-decl só parseia `ident declarator`; `wreal in;`
  vira uma declaração de net com **nome de discipline** `"wreal"` — falha depois com
  erro de resolução enganoso. `reg` em corpo digital falha IR validation.
- **Evidência:** `grammar/item.rs:393-412`; probes t4/t19/t23 da auditoria.
- **Impacto:** modelagem digital idiomática (registradores como vars `reg`,
  `wire` estrutural) impossível; erros confusos.
- **Correção sugerida:** pelo menos falhar loud no parse ("tipo de rede X não
  suportado — use discipline") em vez do erro enganoso de discipline.

### 3.7 ❌ §7.9/§7.8.4 — Driver-receiver segregation

- **LRM diz:** §7.9 — separação driver/receptor na fronteira A2D/D2A; §9.22 —
  `$driver_*` leem essa estrutura.
- **Código faz:** ausente; `$driver_*` são stubs loud (disclosed); a `DriverTable`
  existe mas nunca é populada.
- **Evidência:** `solver/src/digital/drivers.rs` (tabela viva, zero escritores),
  `tests/vams_driver_access_gap.rs`.
- **Impacto:** bridges mistos sofisticados (split com direção) não modeláveis.

### 3.8 ❌ §8.5.3.1/§8.5.3.2 — `assign`/`force`/`release` contínuos e procedurais

- **LRM diz:** §8.5.3.1/§8.5.3.2 — continuous assign e procedural continuous assign.
- **Código faz:** sem gramática (só o for-loop `assign` interno).
- **Evidência:** `grammar/stmt.rs:178`.

### 3.9 ❌ §9.5 — File I/O (name-level only)

- **LRM diz:** §9.5 — `$fopen/$fclose/$fdisplay/...` família completa.
- **Código faz:** nomes no fail-loud table (`eval/tasks.rs:263-277`) + `parse_scanf`
  puro; nada wired a runtime. Analógico: **silencioso** (`flatten/analog.rs:686`);
  digital: loud-unsupported. Ausentes por completo: `$sformat/$swrite/$readmemb/
  $readmemh/$fgets/$ungetc/$fread`.
- **Correção sugerida:** no mínimo o analógico deve ser loud (ver 1.6).

### 3.10 ❌ §9.20 — `$node_alias`

- **LRM diz:** §9.20 — alias de nó por instância.
- **Código faz:** ausente, com razão arquitetural registrada (re-topologia por ponto
  conflita com MD-18 compile-once).
- **Evidência:** `analog_expr.rs:304-318`.
- **Nota:** este é o caso "ausente com razão documentada" — manter como está até o
  MD-18 ser revisitado.

### 3.11 ❌ §9.16 — `$simprobe`

- **LRM diz:** §9.16 — probe dinâmico de simulação.
- **Código faz:** name-level only.

### 3.12 ❌ §8.5.1 — Regiões 1b e 3b da fila estratificada

- **LRM diz:** §8.5.1 — sete regiões; as ausentes são 1b (eventos D2A explícitos) e
  3b (macro-process analógico).
- **Código faz:** 4 de 7 (`regions.rs:62-69`); A2D bridge sem consumidor
  (`interface.rs:108-119`); FIFO intra-região documentado no lugar do
  não-determinismo permitido.
- **Impacto:** a sincronização A2D de granularidade fina não existe — o modelo
  conservativo cobre a maioria dos casos, mas eventos analógicos em meio a
  delta-cycles digitais não são representáveis.

### 3.13 ❌ §6.6 — Generate unrolla SÓ instanciações

- **LRM diz:** §6.6 — loop/if/case generate podem conter qualquer item de módulo
  (nets, blocos analog, vars).
- **Código faz:** só instanciações; o resto é `_ => {}` silencioso
  (`generate.rs:111`); `$param_given` não funciona em condição de generate (probe
  t27); nomes `genblk<n>` ausentes (labels `l1__{i}`); defparams rodam ANTES do
  generate (§6.9.4 — ordem divergente).
- **Impacto:** o exemplo `rcline`/`adc` do próprio LRM não compila fielmente.

### 3.14 ❌ §6.5.7.1/§6.5.8 — Checagem de largura de ports e tolerâncias por nature

- **LRM diz:** §6.5.7.1 (size matching), §6.5.8 (abstol mínima herdada entre natures
  conectadas).
- **Código faz:** só a regra de largura de array-de-instâncias; tolerância é uma
  global única (`solver/analyses/context.rs:28-47`), `Nature::abstol` nunca sai do
  POM.

---

## 4. Argumentos de operadores silenciosamente ignorados ⚠️☠️

Todos parseiam, mas o lowering descarta o argumento sem aviso — violação do
fail-loud. A correção mínima de cada um é **falhar loud** até implementar a
semântica.

| Operador/construct | Argumento ignorado | Evidência |
|---|---|---|
| `idt(expr, ic, assert)` | `assert` | `resolve/pom/analog_ops.rs:83-92` |
| `idtmod(expr, ic, modulus, offset)` | `offset` (e abstol) | `analog_ops.rs:94-107` |
| `absdelay(expr, td, maxdelay)` | `maxdelay` | `analog_ops.rs:139-148` |
| `zi_nd/np/zd/zp(expr, T, τ, t0)` | `τ` e `t0` | `analog_ops.rs:330-360` |
| `cross(expr, dir, time_tol, expr_tol)` / `above` / `timer` | tolerâncias e `enable` | `flatten/analog.rs:103-123` |
| `slew(expr, max_rise, max_fall)` | fall default `0.0` (LRM: `-rise`) | `analog_ops.rs:173-183` |
| `laplace_*(expr, ε, ε)` | ε/abstol | `analog_ops.rs:400-410` |
| `transition(…, time_tol)` | `time_tol` "intentionally not carried" | `kernel/analog/kernel.rs:82-84` |
| `ddt(expr, abstol)` / natureza | abstol/natureza | `analog_ops.rs:73-81` |
| `initial_step("dc", "tran")` | lista de análises | `flatten/analog.rs:82-98` |
| `$discontinuity(degree)` | grau aceito e ignorado (documentado) | `flatten/analog.rs:652-670` |
| `analysis()` sem `"ic"`/`"static"`/`"nodeset"` | variantes | `analog_expr.rs:249-262` |
| Atributos em statements (`(* full_case *) case …`) | parse error (§2.9 Ex.1) | `cursor.rs:136-150` |

---

## 5. Desvios DISCLOSED (conformes por registro — atenção, não bugs)

Estes estão documentados no código com razão explícita. São pontos de atenção para
quem esperar comportamento idêntico a outros simuladores, mas não violam o contrato
do projeto.

| Desvio | Onde está documentado |
|---|---|
| **PRNG `$random`/`$rdist_*` não é o C do IEEE 1364-2005 §17.9.3** — fluxo de bits não bate com simuladores vendor; contrato comportamental (determinismo por seed, formas das distribuições) cumprido | `crates/piperine-lang/src/eval/rng.rs:5-36` |
| **Resolução de disciplines: só Basic (§7.4.4.1)** — Detail explicitamente fora de escopo | `elab/lower/disciplines.rs:18-28` |
| **Scheduler digital Verilator-style (DAG two-phase), não o while-loop literal de 1364** | `solver/src/digital/scheduler.rs:1-12` |
| **`paramset name (module);` com parênteses** — divergência de FORMA do LRM (`paramset name module;`) | `tests/vams_paramset.rs:6-18` |
| **`$strobe`/`$monitor` digitais não deferem para a Monitor region** (chamam imediatamente) | `emit/stmt.rs:113-124` |
| **A2D bridge sem consumidor** (mecanismo de interpolação existe, nenhum elemento o usa) | `digital/interface.rs:108-119` |
| **`$driver_*` stubs loud** — DriverTable existe, nunca populada | `tests/vams_driver_access_gap.rs` |
| **`$node_alias` ausente** — re-topologia por ponto conflita com MD-18 | `analog_expr.rs:304-318` |
| **Switch-level primitives (tran/rtran/tranif)** fora de escopo | `kernel/digital/gates.rs:15-23` |
| **`$mfactor` lê param por instância apenas** — sem scaling automático de fluxo de ramo nem propagação hierárquica | `device/analog/instance.rs:245-264` |
| **`$xposition/$yposition/$angle/$hflip/$vflip`** hard-coded (0.0/1), sem combinação hierárquica | `eval/tasks.rs:756-770` |
| **Concatenação/replicação de strings não suportada** (rejeitada loud) | `digital_expr.rs:82` |

---

## 6. Extensões Piperine além do LRM (superset sancionado)

O Piperine é declaradamente um superset (decisão do usuário). Extensões ativas:

| Extensão | Regime |
|---|---|
| `constraint begin … end` com `require`/`var`/`target`/`cover`/`cover cross` e scoping `@ dc`/`@ tran`/`@ ac` (+ janelas `after`/`dur`) | MD-49 |
| `const <name> = <expr>;` file-scope | MD-50 |
| Cláusula `tol gauss/uniform(...) [global]` em parâmetros | catálogo RV |
| Schema de atributo `(* rfport, num, z0 *)` (portas S-parameter `.sp`) | core-owned |
| `///` doc comments anexados a declarações | §1.1 do Part I |
| Seed UIC `@(initial_step) V(a,b) <+ value;` (SPICE `.ic`) | extensão |
| Escape `\%` em strings | lexer |
| `'{` como lexema distinto (assignment patterns) | lexer |
| Operadores `<<<`/`>>>` | lexer |
| Nets sentinel `$unconnected` | `pom/net_type.rs:42-62` |
| Inicializadores de variável (`integer st = 0;`) | na verdade **LRM-conformes** (A.2.2.1) — o handoff anterior estava errado |

---

## 7. Plano de ação ranqueado

1. **Bug §6.9.1 (blocos analog sobrepostos)** — bug silencioso real; conserto
   mecânico (append), teste de discriminação. *Prioridade máxima.*
2. **Drops silenciosos restantes (1.2–1.10)** — todos violam o fail-loud; a maioria
   é "ficar loud" barato.
3. **`=` vs `<=` digital (§8.5.3.3/8.5.3.4)** — semântica blocking/non-blocking real;
   o coração do escalonamento digital.
4. **`assign` contínuo (1.3)** — destrava wreal, `assign` digital e §7.3.6.4 juntos.
5. **Divergências numéricas pequenas e perigosas:** `log` decimal (2.2), `limexp`
   (2.3), alvo `potential()` (2.4) — meia dúzia de linhas cada, resultados errados
   hoje.
6. **Famílias gramaticais ausentes de maior valor de modelagem:** switch branches
   (3.1), loops analógicos + analog-`for` (2.7), contribuições indiretas (3.2).
7. **Funções de usuário (3.5)** — no mínimo loud na declaração já; inlining como
   follow-up.
8. **`reg`/`wire`/`wreal` como tipos reais (3.6)** — remove erros enganosos e
   habilita §7.3.1 completo.
9. **Argumentos ignorados (seção 4)** — loud até implementar.
10. **Math gaps** (`ceil`, `atan2`, `hypot`, `asinh`, `acosh`, `atanh`) — linhas no
    catálogo de intrínsecos.
11. **`$fatal` imediato (2.14)** e **`$simparam` erro em chave desconhecida (2.15)**
    — duas linhas de fail-loud.
12. **Referências hierárquicas/`$root` (3.3), overrides posicionais (3.4),
    overloading de paramset (2.9), localparam-defparam (2.8)** — superfície de
    elaboração estrutural.

---

## 8. Status de verificação

- **Re-verificados manualmente pelo orquestrador** (5): 1.1 (blocos analog), 1.2
  (eventos digitais), 1.3 (assign contínuo), 1.4 (ground), 2.1 (`=`/`<=`).
- **Evidência da auditoria read-only** (demais itens): cada claim carrega
  `arquivo:linha` da auditoria por capítulo; não foram re-executados ponto a ponto.
  Itens marcados "unverified" na auditoria (ex.: §7.3.3 interpolação por tick,
  §7.2.2 atribuição em dois contextos) foram omitidos deste documento ou listados
  como não-verificados.
- **Não auditado:** capítulo 1 (introdutório), anexos em profundidade (só
  spot-checks de A/B/C/F), e o comportamento do host (`piperine-api`) além do que
  os capítulos 6–9 tocam.

*Este documento substitui qualquer SPEC_DEVIATIONS.md anterior. Regenerado após a
conclusão de cada onda de correções de conformidade.*
