# Piperine CLI & Decentralized Dependency Manager

Este documento serve como a especificação detalhada final da CLI (Command Line Interface) do Piperine. O sistema não é apenas uma interface em torno do compilador, mas sim um empacotador de infraestrutura completa, possuindo controle de manifesto (`Piperine.toml`) e um poderoso **sistema nativo de resolução de dependências git**.

## 1. O Manifesto (`Piperine.toml`)

O `Piperine.toml` é a raiz do seu projeto. A CLI mapeia as instruções escritas nesse arquivo para determinar como e de onde buscar códigos-fonte de terceiros para o seu hardware ou biblioteca de simulação, além de definir meta-dados gerais.

### Estrutura Geral
```toml
[project]
name = "meu_projeto_spice"
version = "0.1.0"
authors = ["Autor <autor@email.com>"]
edition = "2024"

[dependencies]
# Dependência remota em um branch específico de release
spice = { git = "https://github.com/vkeslarek/piperine-spice.git", version = "0.1.0" }

# Dependência remota buscando a branch `develop` explicitamente
std_analog = { git = "https://github.com/piperine/std_analog.git", branch = "develop" }

# Dependência remota congelada (pinned) num commit sha específico
dsp = { git = "https://gitlab.com/corp/dsp.git", rev = "a1b2c3d4" }

# Dependência remota apontando para a latest tag referenciada pelo origin
utils = { git = "https://github.com/piperine/utils.git" } 

# Dependência apontando para um caminho no sistema local
local_lib = { path = "../local_lib" }
```

### O Hash de Versionamento (`Piperine.lock`)
Sempre que uma instrução for resolvida pela primeira vez (ou se sofrer modificação), a CLI iterará sobre o repositório git clonado e extrairá o **HEAD Hash** gerando/atualizando o arquivo `Piperine.lock`. Ele garante reprodutibilidade integral. Nenhuma atualização ocorrerá independentemente do que aconteça no servidor remoto a menos que explicitamente solicitado ou caso não exista lock.

---

## 2. A Árvore de Comandos de Dependência

A CLI oferece três comandos literais desenhados para não te forçar a editar o `Piperine.toml` diretamente e para realizar a checagem cruzada da existência desses pacotes antes de registrá-los em código:

### 2.1 `piperine add <nome> [opções]`
Adiciona com segurança bibliotecas e pacotes ao seu projeto de forma segura:
* Se você adicionar um pacote, a CLI tentará **baixá-lo, clonar e fazer o checkout da tag informada imediatamente**.
* Se a URL estiver incorreta, ou o branch ou commit não existirem, a CLI usa uma política de "Fail-Fast": **Ela não salva o nome do pacote no seu arquivo `Piperine.toml`** preservando-o intacto.

**Argumentos suportados (mutuamente exclusivos em versão):**
- `--git <url>`: O endereço do repositório
- `--version <x.y.z>`: O sistema autocompletará referenciando a tag local remota formatada do release: `release/vx.y.z`
- `--branch <name>`: Aponta diretamente para uma branch
- `--rev <hash>`: Faz checkout de um Hash
- `--path <diretorio>`: Liga uma lib a um sistema de diretórios local

### 2.2 `piperine remove <nome>`
A rotina inteligente de deleção:
1. Remove a linha de dependência do seu manifesto TOML.
2. Reconstrói a árvore de dependências virtuais do restante das bibliotecas remanescentes.
3. Se verificar que **nenhuma outra sub-dependência utiliza este repositório internamente**, ela deleta o diretório do cache em `target/deps/<nome>`, limpando a sua máquina de lixos.

### 2.3 `piperine tree`
Visualiza a topologia de todas as bibliotecas clonadas. Essencial quando você está lidando com pacotes AMS enormes. Ele reflete de onde as instâncias dos códigos fontes do compiler estão sendo lidas.
```text
$ piperine tree
meu_projeto v0.1.0
├── spice (/home/user/meu_projeto/target/deps/spice)
├── dsp (/home/user/meu_projeto/target/deps/dsp)
```

---

## 3. Comandos de Pipeline & Integração

### 3.1 Resolvendo Transições
Sempre que você chamar **`piperine build`**, **`run`**, **`test`** ou **`check`**, o compilador entra em uma rotina estrita de validação de pacotes antes mesmo de processar o primeiro AST:

1. A chamada ao `Resolver::new` da crate `piperine-project` é invocada.
2. É feita a leitura do `Piperine.toml` e `Piperine.lock`.
3. Todos os pacotes são postos num hash de validação.
4. **Resolução de Conflito Estrita**: Se um pacote externo A puxar `dsp v0.1.0` e um pacote externo B puxar `dsp v0.2.0`, diferente de outros package managers, o Piperine aplica *HARD FAILURE*, jogando um erro não-recuperável. A equipe deve resolver manualmente os paths e as calls para alinhar versões, não há tolerância para versionamentos duplos (uma vez que os simuladores lidam com hardware unificado).
5. O `build_source_map` recebe a árvore de bibliotecas completa, iterando e fazendo os injects como `SourceMap::add_namespace`.

Isso permite que um desenvolvedor consiga abrir um arquivo Phdl do projeto dele e simplesmente declarar:
```phdl
use spice::sources::vsrc;

mod meu_top {
    // ...
}
```
A CLI instruirá o compilador a buscar o pacote diretamente no `target/deps/spice/src/sources.phdl`.

### 3.2 O Scaffolding (`piperine new <nome>`)
Configura a base de um novo repositório isolado, criando um diretório e inserindo nativamente um `Piperine.toml` básico juntamente à hierarquia de diretórios `src/main.phdl`.

### 3.3 A Standard Library Fallback Interna
Se a execução for feita num repositório em que a CLI ainda roda sob source (ex: executando o CLI localmente de dentro do diretório do clone do Piperine na máquina do desenvolvedor de ferramentas), ela injeta também dinamicamente `piperine::*` rastreando a pasta superior `crates/piperine-lang/headers/` para viabilizar simulações de desenvolvimento, demonstrando sua flexibilidade como toolchain.
