# SPEC_DEVIATION.md — Divergências entre o Piperine e o Verilog-AMS LRM 2.4

**Data da auditoria:** 2026-08-24 (regeneração pós-onda-2, T70 de `lrm-conformance-wave2`)
**Escopo:** capítulos 2–9 do LRM (`VAMS-LRM-2-4.md`, linhas 1214–16559) + spot-checks
seletivos dos anexos (Annex A gramática, Annex B keywords, Annex C subset analógico,
Annex F resolução de disciplines). O capítulo 1 (introdução) é não-normativo e não foi
auditado.

**Método (regeneração):** este documento parte da auditoria original de 2026-08-19 (60
achados em §1–§5) e re-verifica CADA item individualmente contra HEAD desta branch
(`feature/device-verification`, após os 68 tasks da `lrm-conformance-wave2`, T1–T68).
Para cada item: (a) a evidência `arquivo:linha` original foi relida no arquivo atual
(linha atualizada quando o código se moveu); (b) quando o bug/gap não reproduz mais, o
commit que fechou foi localizado via `git log --oneline` (os ~68 commits mais recentes
da branch, um por task da onda 2) e citado por hash; (c) quando o item segue aberto, a
evidência foi re-lida e o id da cláusula do LRM confirmado explícito. Nenhum item foi
fechado por suposição — todo "fechado" abaixo cita um hash de commit real encontrado por
`git log`/`git show`, ou uma leitura de código atual que prova o bug ausente.

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

A onda 2 (`lrm-conformance-wave2`, T1–T68, spec em
`.specs/features/lrm-conformance-wave2/spec.md`) fechou **todos os 10 drops silenciosos
do §1 original**, **todas as 15 divergências semânticas do §2 original**, **todas as 14
famílias ausentes do §3 original** (uma com corte de escopo documentado), a maior parte
da tabela de argumentos ignorados do §4 original, e a maior parte da tabela de desvios
disclosed do §5 original — cada um com um task/commit dedicado, quase sempre 1:1 com o
achado original (T8 fechou 1.1, T9 fechou 1.2, ..., T67 fechou o desvio de switch-level
primitives).

O que sobrevive como aberto (seção "Ainda aberto" abaixo, ~10 itens) é quase todo um
**recorte de escopo documentado no próprio código** — não um bug esquecido: o argumento
`t0` de `zi_*` (§4.5.12) falha loud em vez de aplicar; o grau de `$discontinuity` é
aceito mas comprovadamente inerte para o integrador atual (pinado por teste, não
ignorado às cegas); a herança de `abstol` por natureza de fluxo (metade de §6.5.8) não
está implementada, só a de potencial; concatenação/replicação de string funciona apenas
quando toda a expressão é elaboration-constant; e um punhado de "declarado mas
elaboration-constant apenas" fica registrado como tal.

---

## 1. Fechados nesta onda (drops silenciosos, §1 original)

Todos os 10 itens ☠️ do §1 original (2026-08-19) foram corrigidos. Regra que valia
(CLAUDE.md): *"When something cannot be faithfully lowered, fail loud — never silently
emit 0.0 or a no-op."*

| # | Achado original | LRM | Commit | O que mudou |
|---|---|---|---|---|
| 1.1 | Múltiplos blocos `analog`: só o último compila | §6.9.1 | `3091d291` (T8) | `resolve/pom/body.rs:693-706` — o braço analógico agora faz `match &mut bodies[i].analog { Some(body) => extend, None => insert }`, espelhando o braço digital; `StateId`s permanecem únicos por módulo (não por bloco), então nenhum offset é necessário. Verificado lendo o código atual. |
| 1.2 | `@(cross/above/timer)` em bloco digital vira combinacional | §7.3.5 | `2640496d` (T9) | `kernel/digital/compile.rs` roteia eventos analógicos por watch-term/`fired`; a forma `@(sig)`/`@*` nível continua combinacional (comportamento correto preservado). |
| 1.3 | `assign` contínuo (module-level) é no-op | §6.5.3/§7.3.6.4/§8.5.3.1/2 | `d99db804` (T10) | `elab/lower/vams.rs` baixa `ModuleItem::ContinuousAssign` para o caminho combinacional digital (alvo digital) ou o caminho de drive contínuo (alvo `wreal`); qualquer forma ainda não suportada falha loud citando §8.5.3.1. |
| 1.4 | `ground gnd;` parseia e é descartado | §3.6.4 | `8cd94b98` (T11) | `elab/lower/vams.rs:477-510` registra o nome declarado (`pending_grounds`/`ground_names`), com a lista hard-coded mantida como fallback de compatibilidade; `ground [3:0] bus;` e `ground discipline name;` agora parseiam e resolvem. |
| 1.5 | `final_step` é no-op; listas de análise descartadas | §5.10.2 | `79ea98a5` (T12) | `flatten/analog.rs:130` — `"final_step" => vec![EventSource::FinalStep { analyses: analysis_name_args(args)? }]`; o corpo dispara uma vez por análise, a lista de nomes filtra. |
| 1.6 | System-task desconhecida em corpo analógico cai em `_ => {}` | §9.2 | `6372d3f0` (T13) | `flatten/analog.rs:1080` — `other => return Err(CodegenError::unknown_task(other))`, mesmo helper compartilhado com o braço digital (`emit/stmt.rs`), citado no comentário como fix contra re-divergência. |
| 1.7 | `nodeset` (inicializador de rede) parseia e some | §3.6.3.2 | `274940cc` (T14) | `device/circuit.rs:42-45` (`CircuitCompiler::nodesets: HashMap<String, f64>`) e `elab/lower/vams.rs:1598-1620` levam o valor até `OpConfig` no host; um nodeset do host ainda tem precedência sobre o declarado (documentado e testado). |
| 1.8 | Atributos somem em params, vars, instâncias | §3.4.3/§3.2.1 | `0ebc723e` (T15) | `elab/lower/vams.rs` — instanciação (`:283`) e `Var` (`:315`) agora chamam `lower_generic_attrs` (o mesmo caminho schema-checado de ports/nets); os `attributes: Vec::new()` restantes no arquivo (`:268,342,467,502`) são sites internos legítimos (aliasparam sintético, var oculta de `event`, net implícita 1364, net sintética de `ground`) — não os sites que o achado original citava. |
| 1.9 | `@(expr)` nível não-call/ident nunca dispara | §5.10.3 | `1f458f83` (T16) | `flatten/analog.rs:126` — qualquer expressão que não seja `Call`/`Ident` vira `EventSource::Named { expr }`, amostrada e disparada por mudança. O único `Ok(vec![])` remanescente (`:102`) é `EventExpr::ImplicitAll` (`@*`), fora de escopo por design (precisa de coleta de sinais do corpo inteiro — feature distinta, documentada no comentário). |
| 1.10 | `genvar` fora de generate é skipado | §3.5 | `15943049` (T17) | `elab/lower/vams.rs:92-114` registra em `Module::genvars()` via prepass e chama `check_genvar_used_outside_generate`, que falha loud fora de contexto válido. |

---

## 2. Fechados nesta onda (divergências semânticas, §2 original)

Todos os 15 itens ❗ do §2 original foram corrigidos.

| # | Achado original | LRM | Commit | O que mudou |
|---|---|---|---|---|
| 2.1 | `=`/`<=` digitais emitem código idêntico | §8.5.3.3/4 | `b6a7b284` (T23) | `emit/stmt.rs:66-79` separa os braços: `=` escreve na região Active; `<=` avalia contra os bancos vivos mas escreve na região Nba (`Region::Nba`/`RegionAction::Update`, `solver/digital/regions.rs`), commitada só ao fim do delta-cycle — `a<=b; b<=a;` agora troca independente da ordem. |
| 2.2 | `log` é alias de `ln` (devia ser log10) | §4.3 Tab. 4-14 | `bec48891` (T18) | `math.rs:81` — `let name = if name == "log" { "log10" } else { name };`; teste inline `log(100) == 2.0`. |
| 2.3 | `limexp` é `exp(min(x,80))` puro, sem limiting entre iterações | §4.5.13 | `4934f990` (T19) | `math.rs:57-62` documenta que `limexp` saiu da tabela stateless; dispatch dedicado em `kernel/analog/limexp.rs` com estado Newton-a-Newton (`yold`). |
| 2.4 | `potential(p) <+ …` classificado como fonte de FLUXO | §4.4/§5.5.1 | `c9925460` (T20) | `flatten/analog.rs:1405-1406` — `"V" \| "potential" => Some(NatureKind::Potential)`, `"I" \| "flow" => Some(NatureKind::Flow)`. |
| 2.5 | `===`/`!==` baixam para `==`/`!=` planos | §4.2.6 | `e68fc883` (T24) | `resolve/expr.rs` — `BinOp::CaseEq`/`BinOp::CaseNe` são variantes próprias (não mais aliasadas), com emissão 4-state dedicada. |
| 2.6 | `analysis()` casa com dc por default; `ac_stim` consome nome como magnitude; `noise_table` morto | §4.6 | `755401bb` (T21) | `emit/analog_expr.rs:451-489` — `analysis("bogus")` agora é erro loud citando os nomes válidos (`"dc"/"ac"/"tran"/"noise"/"static"/"ic"/"nodeset"`; `"ic"`/`"nodeset"` são aceitos mas nunca comparam verdadeiro — sentinela documentada, não um alias silencioso). `ac_stim`'s magnitude não é mais consumida pelo nome da análise (`resolve/pom/analog_ops.rs:473-482`). `noise_table`/`noise_table_log` deixaram de retornar `0.0` silencioso — ver nota em "Ainda aberto" abaixo (a função em si segue não implementada, mas agora falha loud). |
| 2.7 | Loops de runtime são erro duro; analog-`for` (§5.9.3) ausente | §5.9/§5.9.3 | `25a46f30` (T32) | `flatten/analog.rs:703-709`/`emit/stmt.rs:205-218` — `unroll_loop` desenrola `for`/`while`/`repeat` com limites elaboration-constant (com cap contra explosão de JIT); bound não-constante segue erro loud, agora citando §5.9.3. |
| 2.8 | `localparam` overridável via `defparam` | §3.4.5 | `9a20fb38` (T28) | `elab/lower/defparam.rs:143` — `if p.is_local { … }` agora falha loud com os dois spans. |
| 2.9 | Overloading de paramset: `HashMap`, segundo sobrescreve o primeiro | §6.4.2 | `603efddc` (T29) | `elab/lower/register.rs:44` — `self.syms.paramsets.entry(p.name.clone()).or_default().push(p.clone())`: vetor por nome, seleção por range implementada. |
| 2.10 | X/Z digital lido em contexto contínuo vira número silencioso (2.0/3.0) | §7.3.2 | `b39ed74f` (T25) | `kernel/digital/layout.rs:170-189` — leitura de um `Quad` X/Z no lado analógico é erro loud citando §7.3.2, nomeando a rede. |
| 2.11 | Compatibilidade de natures só por nome+domínio | §3.11.1 | `c1571508` (T30) | `elab/lower/disciplines.rs:330-369` — `disciplines_nature_compatible`/`natures_compatible` seguem a família de derivação, não só igualdade de nome. |
| 2.12 | Nome de connect-module auto-inserido não segue §7.8.5; direções ignoradas | §7.8.5 | `2ae8a791` (T31) | `elab/lower/connect.rs:97-101` implementa o esquema `SigName__ModuleName__BottomDiscipline`/`SigName__InstName__PortName`; seleção do connect-module (`:277-324`) agora considera direções (Tabela 7-2) e `connect_port_overrides`. |
| 2.13 | Sem quantização de tick digital | §8.4.3.3 | `4acade76` (T26) | `device/digital.rs:595-597` — `self.kernel.quantize_time(time)`, no-op sem `` `timescale ``, ativo quando declarada. |
| 2.14 | `$fatal` deferido ao `accept_timestep` | §9.7.3 | `aa62398f` (T27) | `kernel/analog/kernel.rs:836-846` — `$fatal` roteia para `fatal_flags`/`fatal_formats`, checado a cada iteração de Newton (`eval_fatal`), não mais só no accept; `$error`/`$warning`/`$info` continuam deferidos (regra correta, preservada). |
| 2.15 | `$simparam` chave desconhecida sem default retorna `0.0` | §9.15 | `3d4547f2` (T22) | `emit/analog_expr.rs:256-262` — erro loud nomeando a chave e o conjunto válido; a forma com default explícito continua funcionando. |

---

## 3. Fechados nesta onda (construções ausentes, §3 original)

13 dos 14 itens ❌ do §3 original foram implementados; 1 (3.10) foi implementado no caso
principal com um corte de escopo arquitetural documentado (ver "Ainda aberto").

| # | Achado original | LRM | Commit | O que mudou |
|---|---|---|---|---|
| 3.1 | Switch branches | §5.6.5 | `4c66a9a8` (T45) | `flatten/analog.rs:1194-1208` — contribuição de potencial condicional sobre um branch marcado `BranchKind::Switch` (detector do POM, `pom/branch.rs`, agora com call site) vira elemento de chave; headers congelados `diode.va:45`/`mos.va:30` compilam. |
| 3.2 | Contribuições indiretas (`V(out): V(in)==0;`) | §5.6.7 | `224537df` (T46) | `grammar/stmt.rs:420` — `Stmt::IndirectContribution`; resolvida como equação implícita via a capability `HAS_INTERNAL_UNKNOWNS`. |
| 3.3 | Referências hierárquicas / `$root` | §6.7/§6.2.1 | `253eb679` (T50) | `grammar/expr.rs:215,281` — `Expr::Hier { root, path }`; resolvido em codegen contra `Design::flat_modules` (`resolve/pom/body.rs`). |
| 3.4 | Overrides posicionais `#(2k)` | §6.3.2 | `98551b8b` (T49) | `elab/lower/vams.rs:825-876` — forma posicional (bind por ordem declarada), mistura posicional/nomeado é erro loud, excesso de argumentos é erro loud. |
| 3.5 | Funções de usuário (declara e morre na chamada) | §4.7 | `e6c905df` (T44) | `elab/lower/vams.rs:369-371` popula `functions`; `resolve/symbols.rs:411`/`resolve/pom/expr.rs:353-377` — `fn_by_name` agora resolve e `inline_user_fn` roda de fato. |
| 3.6 | `reg`/`wire`/`wreal`/`tri` não são tipos reais | §3.7/§7.3.1 | `2f6a372f` (T52) | `pom/net_type.rs:104-133` — `"reg"`/`"wreal"` parseiam como tipo de rede (`NetType::Digital1364`), não mais como nome de discipline; leitura em contexto contínuo segue Tabela 7-1. |
| 3.7 | Driver-receiver segregation ausente | §7.9/§9.22 | `d8cff14c` (T54) | `solver/digital/drivers.rs` — `DriverTable` populada pelo próprio loop de aplicação de eventos do scheduler; `$driver_count/state/strength` (`eval/tasks.rs:86-92,272-274`) leem dados reais; `tests/vams_driver_access_gap.rs` reescrito (não deletado) para a nova asserção. |
| 3.8 | `assign`/`force`/`release` procedurais ausentes | §8.5.3.1/2 | `c9b59bb6` (T53) | `grammar/stmt.rs:79-124` — `deassign`/`force`/`release` parseiam (`Stmt::ProceduralDeassign/Force/Release`). |
| 3.9 | File I/O só a nível de nome | §9.5 | `aba66239` (T55) | `eval/tasks.rs:280-301` — `$fopen/$fclose/$fdisplay/$sformat/$readmemh/...` roteados ao runtime real (`emit/runtime_io.rs`, `solver/core/output.rs`), analógico e digital. |
| 3.10 | `$node_alias` ausente | §9.20 | `718aac33` (T56) | Ver "Ainda aberto" — forma constante implementada; forma dinâmica segue documentadamente fora de escopo (MD-18). |
| 3.11 | `$simprobe` só a nível de nome | §9.16 | `635e4f9c` (T57) | `eval/tasks.rs:191` — implementado, proven equivalente ao probing do host. |
| 3.12 | Regiões 1b/3b da fila estratificada ausentes | §8.5.1 | `450e5da6` (T58) | `solver/digital/regions.rs:52-110` — `Region::D2a` (1b) e `Region::AnalogMacroProcess` (3b) adicionadas; ordenação das 4 regiões pré-existentes preservada. |
| 3.13 | Generate unrolla só instanciações | §6.6 | `16d4dd54` (T51) | `elab/lower/generate.rs` — nets, blocos analog/digital e vars agora desenrolam; `genblk<n>` implementado; `$param_given` funciona em condição de generate; `defparam` aplica após generate (§6.9.4). Um corte de escopo documentado sobrevive — ver "Ainda aberto". |
| 3.14 | Checagem de largura de ports; tolerância por nature | §6.5.7.1/§6.5.8 | `e0b3d637` (T48) | `elab/lower/`: checagem de largura de porta implementada. `solver/analyses/context.rs:49-63` — `Tolerances::node_vntol_overrides` carrega `abstol` de potencial por nó, testado tornando convergência observável mais apertada. Metade de fluxo — ver "Ainda aberto". |

---

## 4. Fechados nesta onda (argumentos de operador antes ignorados, §4 original)

| Operador/construct | Argumento | Commit | Estado atual |
|---|---|---|---|
| `ddt(expr, abstol\|nature)` | abstol/nature | `b7c1b1f7` (T33) | Carregado (`resolve/pom/analog_ops.rs:84-90`), observável no timestep/LTE. |
| `idt(expr, ic, assert)` | `assert` | `e40424b2` (T34) | Carregado (`:96-100`); reset ao `ic` enquanto assertado. |
| `idtmod(expr, ic, modulus, offset)` | `offset` | `b5e26b39` (T35) | Carregado (`:118-124`). |
| `absdelay(expr, td, maxdelay)` | `maxdelay` | `c8c99a56` (T36) | Clamp `td>maxdelay ? maxdelay : td` (`:165-183`) — implementa o texto literal do LRM ("maxdelay será usado como substituto"), não um erro de build-time. |
| `slew(expr, max_rise, max_fall)` | default de `max_fall` | `a4e7810c` (T37) | Corrigido: 1-argumento agora usa `-max_rise` no lugar de `0.0`. |
| `laplace_*(expr, ε, ε)` | abstol | `7422e34e` (T39) | Carregado (`:382-390`). |
| `zi_*(expr, T, τ, t0)` | `τ` | `1f826883` (T40) | `τ` carregado (`:440-463`). `t0` — ver "Ainda aberto". |
| `cross/above/timer` | tolerâncias + `enable` | `4ec7d9dc` (T41) | `time_tol`/`expr_tol`/`enable` carregados (`flatten/analog.rs:136-158`, `kernel/analog/kernel.rs:116-135`). |
| `transition(…, time_tol)` | `time_tol` | `923565ea` (T38) | Carregado no schedule de breakpoints (`kernel/analog/kernel.rs:82-84`). |
| `initial_step`/`final_step("dc","tran")` | lista de análises | `79ea98a5` (T12) | Filtra por análise (`flatten/analog.rs:129-130`). |
| `analysis()` sem `"ic"`/`"static"`/`"nodeset"` | variantes | `755401bb` (T21) | Aceitas (ver §2.6 acima); `"ic"`/`"nodeset"` nunca comparam verdadeiro (sentinela documentada). |
| Atributos em statements | parse error | `b4f46aa7` (T43) | `(* full_case *) case … endcase` e as demais formas de §2.9 parseiam e chegam ao POM do statement. |
| `$discontinuity(degree)` | grau | `0bb15225` (T42) | Ver "Ainda aberto" — provadamente inerte para o integrador atual, agora pinado por teste em vez de só um comentário. |

---

## 5. Fechados nesta onda (desvios disclosed, §5 original)

| Desvio original | Commit | Estado atual |
|---|---|---|
| PRNG não-1364 | `015826d1` (T59) | `eval/rng.rs` reimplementado linha a linha a partir do listing de IEEE 1364-2005 §17.9.3; comentário de disclosure removido. |
| Resolução só Basic | `9ae20ce0` (T60) | Detail (§7.4.4.2) implementada ao lado de Basic (`elab/lower/disciplines.rs:18-30`); comentário "fora de escopo" removido. |
| Scheduler Verilator-style (potencial divergência de ordering) | `76831967` | Teste diferencial (`RegionQueue` vs scheduler de produção) prova ordenação observável idêntica; qualquer diferença intencional agora é pinada por teste, não só um comentário. |
| `paramset name (module);` com parênteses (forma não-LRM) | `e1aaf412` (T61) | Forma LRM sem parênteses (`paramset name module;`) agora também parseia; a forma com parênteses permanece como extensão sancionada (§6, não mais um desvio). |
| `$strobe`/`$monitor` chamam imediatamente | `6032c197` (T62) | Deferem à região Monitor (`emit/stmt.rs:135-176`); `$display` continua imediato (Active), correto. |
| A2D bridge sem consumidor | `288bac77` (T63) | `A2dComparator` é o consumidor real (`solver/digital/interface.rs:108-109`, `piperine_codegen::device::A2dComparator`). |
| `$driver_*` stubs loud | `d8cff14c` (T54) | Ver §3.7 acima — `DriverTable` populada de verdade. |
| Switch-level primitives fora de escopo | `4a132380` (T67) | `tran/rtran/tranif0/1/rtranif0/1` implementados (`kernel/digital/gates.rs`); comentário "fora de escopo" removido. |
| `$mfactor` lê param por instância apenas | `7a1403a6` (T64) | Escala fluxo de branch automaticamente e propaga hierarquicamente (produto `$mfactor_specified * $mfactor_hier`, `device/analog/instance.rs:278-300`). |
| `$xposition/$yposition/$angle/$hflip/$vflip` hard-coded | `e8047b11` (T65) | Lidos de dados de posicionamento por instância reais (via o caminho de atributos de instância do T15), compostos hierarquicamente. |
| Concatenação/replicação de strings rejeitada loud | `3444e540` (T66) | Caso elaboration-constant (todo operando string-literal) implementado — ver "Ainda aberto" para o caso dinâmico, que segue rejeitado. |

---

## 6. Ainda aberto (recortes de escopo documentados — não bugs esquecidos)

Cada linha abaixo tem uma justificativa explícita no código (comentário citando a razão)
e não é um drop silencioso: cada caso ainda ausente falha loud ou está formalmente fora
do escopo do task que fechou o item pai.

| Desvio | LRM | Evidência (HEAD) |
|---|---|---|
| `zi_*`'s `t0` (start-time do primeiro transition) não é aplicado — falha loud citando a razão arquitetural (não há estado sample-and-hold para deslocar) | §4.5.12 | `crates/piperine-codegen/src/resolve/pom/analog_ops.rs:406-434` (`ZTransform::err_t0`) |
| `$discontinuity(degree)` é aceito e **provadamente inerte** para o integrador TR-BDF2 atual — pinado por teste dedicado, não um comentário solto | §9.17.1 | `crates/piperine-codegen/tests/discontinuity_degree_inert.rs` |
| `noise_table`/`noise_table_log` (§4.6.4.3/4) não estão implementados — deixaram de retornar `0.0` silencioso e agora falham loud nomeando a chamada | §4.6.4.3, §4.6.4.4 | `crates/piperine-codegen/tests/analysis_dependent_functions.rs:174-210` (`noise_table_call_fails_loud_not_silently_zero`) |
| `analysis("ic")`/`analysis("nodeset")` são aceitos sintaticamente mas nunca comparam verdadeiro (sentinela `NEVER`) — o solver não rastreia essas duas sub-fases hoje | §4.6 Tab. 4-17 | `crates/piperine-codegen/src/emit/analog_expr.rs:463-479` |
| `$analog_node_alias`/`$analog_port_alias` com argumento **não-constante** (dinâmico, por sweep-point) segue fora de escopo — conflita com MD-18 (compile-once); a forma **constante** foi implementada (T56) | §9.20 | `crates/piperine-codegen/src/emit/analog_expr.rs:331-345` |
| `ddt`/`idt`/`idtmod`'s argumento `abstol`\|`nature` — `ddt` e `idtmod` (offset) já carregam; o `abstol`\|`nature` de cauda de `idt`/`idtmod` (o 4º/5º argumento, distinto do `abstol` de 2 args de `ddt` já implementado) segue fora de escopo, documentado como "tracked separately" | §4.5.4/§4.5.5 | `crates/piperine-codegen/src/resolve/pom/analog_ops.rs:97-99,116-118` |
| `parameter`/`localparam` declarados dentro de um bloco `generate` não são desenrolados — nets/behaviors/vars/instâncias já são (T51); um parâmetro generate-local cai em `_ => {}` | §6.6 | `crates/piperine-lang/src/elab/lower/generate.rs:19-25` |
| Herança de `abstol` por **natureza de fluxo** (a metade de §6.5.8 sobre flow, distinta da de potencial já implementada em T48) não está coberta | §6.5.8 | `crates/piperine-solver/src/analyses/context.rs:58-63` |
| Concatenação/replicação de string funciona apenas quando toda a expressão é elaboration-constant (o caso do `$display`/format-string); uma expressão de string dinâmica em concat/replication segue rejeitada loud (mesmo caminho que já rejeitava vetores numéricos) | §3.3/§4.2.13 | `crates/piperine-codegen/src/emit/digital_expr.rs:82-84` |

---

## 7. Extensões Piperine além do LRM (superset sancionado) — spot-check

Seção informativa (não é uma divergência), re-verificada rapidamente — sem mudanças
relevantes desde 2026-08-19, exceto que `paramset name (module);` (parênteses) migrou de
"desvio disclosed" (§5 antigo) para aqui, já que a forma LRM sem parênteses agora também
é aceita (T61) e a forma com parênteses é puramente uma extensão adicional, não mais o
único caminho.

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
| Nets sentinel `$unconnected` | `crates/piperine-lang/src/pom/net_type.rs:38-50` — verificado, ainda presente |
| `paramset name (module);` com parênteses — agora puramente extensão adicional (a forma LRM sem parênteses também parseia desde T61) | `crates/piperine-lang/src/grammar/item.rs:1505-1516` |
| Inicializadores de variável (`integer st = 0;`) | na verdade **LRM-conformes** (A.2.2.1) — verificado: `crates/piperine-lang/tests/annex_a_grammar.rs:81` referencia A.2.2.1 diretamente |

---

## 8. Plano de ação ranqueado (pós onda-2)

Todos os itens de alta severidade (§1/§2/§3 originais) estão fechados. O que resta é
pequeno e de baixo risco:

1. **`noise_table`/`noise_table_log` (§4.6.4.3/4)** — a única lacuna funcional real
   restante (não apenas um recorte de escopo de argumento): a função em si não existe,
   apenas falha loud em vez de `0.0`. Próximo candidato a implementação completa.
2. **`zi_*`'s `t0`** e **`idt`/`idtmod`'s `abstol`\|`nature` de cauda** — argumentos de
   operador ainda não aplicados, ambos já falham loud (não silenciosos), baixa
   prioridade.
3. **`$analog_node_alias` dinâmico** — bloqueado por decisão arquitetural (MD-18), não
   revisitar sem reabrir essa decisão.
4. **`$discontinuity(degree)`** — inerte por design do integrador atual; só relevante se
   o integrador mudar.
5. **Parâmetros dentro de `generate`** e **herança de `abstol` de fluxo** — lacunas
   pequenas, de baixo impacto de modelagem, cada uma já documentada no código-fonte.
6. **Concatenação/replicação de string dinâmica** — bloqueada pela ausência de um tipo
   string em runtime na ABI da JIT digital; exigiria uma mudança de ABI maior que o
   escopo desta onda.

---

## 9. Status de verificação

- **Todo item de §1–§5 do documento original (2026-08-19)** foi re-lido individualmente
  contra HEAD desta branch neste ciclo (T70): 10 itens de §1, 15 de §2, 14 de §3, 13
  linhas de tabela de §4, 11 linhas de tabela de §5 — 63 claims no total.
- **51 fecharam** (commit citado, código lido para confirmar a ausência do bug/gap) — 10
  do §1, 15 do §2, 14 do §3 (13 completos + 1 fechado com recorte, 3.10), 12 do §4 (uma
  linha, atributos em statements, incluída), 11 do §5 (uma migrada para extensão
  sancionada em vez de permanecer disclosed).
- **9 seguem abertos**, cada um re-verificado com evidência atual `arquivo:linha` e a
  cláusula do LRM explícita (seção 6 acima) — todos são recortes de escopo documentados
  no próprio código-fonte, nenhum é um drop silencioso.
- **Nenhum item foi fechado por suposição** — cada "fechado" cita um hash de commit
  real localizado via `git log --oneline`/`git show`, ou evidência de leitura de código
  direta quando a busca por commit não isolou um hash único (nenhum caso deste
  documento precisou desse fallback — todos os 51 fechamentos têm hash).
- **Não auditado nesta regeneração:** capítulo 1 (introdutório, não-normativo), anexos em
  profundidade (só spot-checks de A/B/C/F, herdados da auditoria original), e o
  comportamento do host (`piperine-api`) além do que os capítulos 6–9 tocam — mesmo
  escopo do documento original.

*Este documento substitui a versão de 2026-08-19. Regenerado após a conclusão da onda 2
de correções de conformidade (`lrm-conformance-wave2`, T1–T68); a próxima regeneração
deve seguir a mesma disciplina — cada item fechado precisa de um commit citável, cada
item aberto precisa de evidência `arquivo:linha` relida no momento da regeneração.*
