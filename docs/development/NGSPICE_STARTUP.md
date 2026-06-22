# ngspice Startup, CLI Options, and Compatibility Modes

Source: `src/main.c` (CLI parsing, compat mode), `src/misc/ivars.c` (env
vars), `src/include/ngspice/defines.h` (init file names).

---

## 1. Command-Line Options

```
ngspice [OPTIONS] [FILE ...]
```

| Short | Long               | Description                                                   |
|-------|--------------------|---------------------------------------------------------------|
| `-a`  | `--autorun`        | Run the loaded netlist immediately (no interactive prompt)    |
| `-b`  | `--batch`          | Batch mode: process FILE and exit (no interactive prompt)     |
| `-c FILE` | `--circuitfile=FILE` | Specify circuit file explicitly                       |
| `-i`  | `--interactive`    | Force interactive mode even when stdin is a file              |
| `-n`  | `--no-spiceinit`   | Skip loading `.spiceinit` / `spice.rc` user init files        |
| `-o FILE` | `--output=FILE`| Redirect output (stdout) to FILE                              |
| `-p`  | `--pipe`           | Pipe mode: read commands from stdin, write results to stdout  |
| `-q`  | `--completion`     | Enable readline command completion                            |
| `-r FILE` | `--rawfile=FILE` | Write simulation output to binary rawfile FILE             |
|       | `--soa-log=FILE`   | Write Safe Operating Area (SOA) warnings to FILE              |
| `-s`  | `--server`         | Server mode: read/write via file descriptors (IPC)            |
| `-t TERM` | `--term=TERM`  | Set terminal type for display purposes                        |
| `-h`  | `--help`           | Print help and exit                                           |
| `-v`  | `--version`        | Print version and exit                                        |

### Mode Selection Logic

| Condition                         | Effective mode      |
|-----------------------------------|---------------------|
| `-b` flag                         | Batch               |
| stdin is not a TTY, no `-i`       | Batch               |
| `-i` with piped stdin             | Interactive (forced)|
| TTY and no `-b`                   | Interactive         |
| `-s`                              | Server (overrides)  |
| `-p`                              | Pipe (overrides)    |

In batch mode (`-b`), ngspice processes all `.control`/`.endc` blocks and
exits. In interactive mode, a `ngspice X >` prompt is shown.

### Examples

```sh
# Batch run, save rawfile, redirect output
ngspice -b -r results.raw -o sim.log mycirc.spi

# Interactive with preloaded file
ngspice mycirc.spi

# Run and exit immediately (no interactive prompt)
ngspice -a mycirc.spi

# Pipe mode for scripted integration
echo "source mycirc.spi\nrun\nwrite out.raw\nquit" | ngspice -p
```

---

## 2. Initialization Files

ngspice loads initialization files in the following order at startup:

### System Init: `spinit`

Location: `$SPICE_LIB_DIR/scripts/spinit` (default: `/usr/local/share/ngspice/scripts/spinit`).

Contains global defaults: `set`, `alias`, `.options`, library paths. Loaded
before any user init or circuit file.

### User Init: `.spiceinit` / `spice.rc`

Loaded after `spinit` (unless `-n` is given). Search order:

1. `.spiceinit` in the **current directory**
2. `spice.rc` in the **current directory** (fallback)
3. `.spiceinit` in `$HOME` (if not found locally)
4. `spice.rc` in `$HOME` (fallback)

Defined by:
```c
#define INITSTR     ".spiceinit"
#define ALT_INITSTR "spice.rc"
```

The `-n` / `--no-spiceinit` flag skips steps 1–4.

### Typical `.spiceinit` Content

```spice
* Set compatibility mode
set ngbehavior = all

* Fixed RNG seed for reproducibility
set rndseed = 12345

* Default include path
set sourcepath = ( ~/spice/lib /usr/local/share/ngspice/models )

* Quiet banner
set noaskquit
```

---

## 3. Environment Variables

| Variable             | Overrides / Controls                                     | Default                              |
|----------------------|----------------------------------------------------------|--------------------------------------|
| `SPICE_LIB_DIR`      | Base library directory (spinit, scripts, help)           | Compile-time `$prefix/share/ngspice` |
| `SPICE_EXEC_DIR`     | Directory containing the `ngspice` binary                | Compile-time `$prefix/bin`           |
| `SPICE_SCRIPTS`      | Directory containing `spinit`                            | `$SPICE_LIB_DIR/scripts`             |
| `SPICE_PATH`         | Full path to ngspice binary (for `aspice` command)       | `$SPICE_EXEC_DIR/ngspice`            |
| `SPICE_NEWS`         | Path to news file shown at startup                       | `$SPICE_LIB_DIR/news`                |
| `SPICE_HELP_DIR`     | Help file directory                                      | `$SPICE_LIB_DIR/helpdir`             |
| `NGSPICE_INPUT_DIR`  | Extra input file search path (`.lib`, `.include`)        | None (Linux), `<bindir>/input` (Win) |
| `SPICE_NO_DATASEG_CHECK` | Disable memory limit check (set to any value)       | Not set                              |
| `SPICE_ASCIIRAWFILE` | 0 = binary rawfile, 1 = ASCII rawfile                    | 0 (binary)                           |
| `SPICE_HOST`         | Remote host for `aspice` command                         | None                                 |
| `SPICE_BUGADDR`      | Override bug-report email shown in help                  | Compile-time address                 |
| `SPICE_EDITOR`       | Editor for `edit` command                                | System default                       |
| `NGSPICE_MEAS_PRECISION` | Decimal digits in `.meas` output                   | 5                                    |

---

## 4. Compatibility Modes (`ngbehavior`)

Set via `.spiceinit` or `.control`:

```spice
set ngbehavior = all     ; default
set ngbehavior = hs      ; hspice-like
set ngbehavior = ps      ; pspice-like
set ngbehavior = spice3  ; strict spice3 (minimal preprocessing)
```

| Mode     | Enum value           | Description                                              |
|----------|----------------------|----------------------------------------------------------|
| `all`    | `COMPATMODE_ALL`     | **Default.** All preprocessing enabled (E/G/L/R/C rewrites, B-source compat, `temper`) |
| `hs`     | `COMPATMODE_HS`      | HSpice compatibility: `$` parameter syntax, `/` allowed in model names, extra `.param` functions |
| `ps`     | `COMPATMODE_PS`      | PSpice compatibility: `{expr}` for `.param`, VCVS keyword on E-source |
| `spice3` | `COMPATMODE_SPICE3`  | Strict Spice3: minimal preprocessing, no E/G/B rewrites  |
| `native` | `COMPATMODE_NATIVE`  | Same as `all` (code alias, rarely used directly)         |

### What Each Mode Affects

| Feature                               | `all` | `hs` | `ps` | `spice3` |
|---------------------------------------|:-----:|:----:|:----:|:--------:|
| E/G `VALUE=` → B-source rewrite       | ✓     | ✓    | ✓    | ✗        |
| E `TABLE` / G `TABLE` rewrite         | ✓     | ✓    | ✓    | ✗        |
| B-source `temper` keyword             | ✓     | ✓    | ✓    | ✗        |
| HSpice `$` in `.param` expressions    | ✗     | ✓    | ✗    | ✗        |
| PSpice `VCVS` keyword on E-source     | ✓     | ✗    | ✓    | ✗        |
| `.param` inside `.subckt` (local)     | ✓     | ✓    | ✓    | ✗        |

Most users should leave `ngbehavior = all` (the default).

---

## 5. Important Startup `.options`

Set in netlist or `.spiceinit` as `.options OPTION=VALUE`.

| Option         | Default | Description                                              |
|----------------|---------|----------------------------------------------------------|
| `TNOM`         | 27      | Nominal temperature (°C) for model parameter extraction  |
| `TEMP`         | 27      | Circuit operating temperature (°C)                       |
| `GMIN`         | 1e-12   | Minimum conductance stamped on all nodes                 |
| `ABSTOL`       | 1e-12   | Absolute current convergence tolerance (A)               |
| `VNTOL`        | 1e-6    | Absolute voltage convergence tolerance (V)               |
| `RELTOL`       | 0.001   | Relative convergence tolerance                           |
| `ITL1`         | 100     | DC iteration limit                                       |
| `ITL2`         | 50      | DC transfer curve iteration limit                        |
| `ITL4`         | 10      | Transient time-step iteration limit                      |
| `MAXORD`       | 2       | Maximum integration order (1 or 2)                       |
| `METHOD`       | TRAP    | Integration method: TRAP (trapezoidal) or GEAR          |
| `NOOPINFO`     | off     | Suppress operating-point output                          |
| `SAVECURRENTS` | off     | Auto-save all terminal currents                          |
| `NGDEBUG`      | off     | Enable debug output (deck dump to `debug-out*.txt`)      |

---

## 6. Batch Output Without `.control`

In batch mode (`-b`), ngspice processes the netlist cards then evaluates
`.print`, `.plot`, `.four`, and `.probe` cards in the netlist before exiting.

```spice
.tran 1n 100n
.print tran v(out) i(V1)
.end
```

Combined with `-r`:

```sh
ngspice -b -r results.raw mycirc.spi
```

The `.raw` file is in binary format by default. Set `SPICE_ASCIIRAWFILE=1`
or `.options rawfmt=ascii` for human-readable output.

---

## 7. Shared Library Mode (libngspice)

When built as `libngspice.so` / `libngspice.dll`, the startup is driven by
the embedding application rather than `main.c`. The library reads `.spiceinit`
from the process working directory by default. The same `rndseed`, `ngbehavior`,
and environment variables apply.

Init files are loaded via `sharedspice.c:ngSpice_Init()` → same sequence as
standalone: system `spinit` first, then `.spiceinit`/`spice.rc`.
