# ngspice Control Commands Reference

Generated from ngspice source at `~/Git/ngspice/src/frontend/`. Excludes XSpice.

---

## Analysis Commands

### op — Operating Point

**Syntax:** `op`

Computes the DC operating point. No arguments. Sets up the linearized small-signal
model used by subsequent AC, noise, and TF analyses.

Results accessible as `v(node)`, `i(vsrc)`, `@device[param]`.

---

### dc — DC Sweep

**Syntax:** `dc <srcnam> <vstart> <vstop> <vincr> [<src2> <start2> <stop2> <incr2>]`

| Argument | Description |
|----------|-------------|
| srcnam | Voltage/current source name, resistor, or `temp` |
| vstart | Start value |
| vstop | Stop value |
| vincr | Step increment |
| src2 … | Optional second source for nested sweep |

`srcnam` may be any independent source name, a resistor value (sweeps resistance), or the
keyword `temp` (sweeps temperature in °C).

---

### ac — AC Analysis

**Syntax:** `ac <dec|oct|lin> <np> <fstart> <fstop>`

| Argument | Description |
|----------|-------------|
| dec | Logarithmic, points per decade |
| oct | Logarithmic, points per octave |
| lin | Linear spacing |
| np | Number of points (per decade/octave for log; total for lin) |
| fstart | Start frequency (Hz) |
| fstop | Stop frequency (Hz) |

Requires a prior OP to be valid (computed automatically if needed).

---

### tran — Transient Analysis

**Syntax:** `tran <tstep> <tstop> [<tstart> [<tmax>]] [uic]`

| Argument | Default | Description |
|----------|---------|-------------|
| tstep | — | Suggested output time step (s) |
| tstop | — | End time (s) |
| tstart | 0 | Start saving output at this time (s) |
| tmax | tstep | Maximum internal time step (s) |
| uic | — | Flag: use initial conditions (`.ic` values), skip OP |

---

### pss — Periodic Steady State

**Syntax:** `pss <fguess> <stabtime> <points> <harmonics> [uic] [sc_iter <n>] [steady_coeff <n>]`

| Argument | Description |
|----------|-------------|
| fguess | Guessed fundamental frequency (Hz) |
| stabtime | Stabilization time (s) before checking periodicity |
| points | Number of time points in one period |
| harmonics | Number of harmonics to consider from DC |
| uic | Use initial conditions |
| sc_iter | Max shooting-cycle iterations (default varies) |
| steady_coeff | Convergence coefficient |

---

### pz — Pole-Zero Analysis

**Syntax:** `pz <nodei> <nodeg> <nodej> <nodek> <vol|cur> <pol|zer|pz>`

| Argument | Description |
|----------|-------------|
| nodei, nodeg | Input port nodes (positive, negative) |
| nodej, nodek | Output port nodes (positive, negative) |
| vol | Transfer voltage ratio |
| cur | Transfer current ratio |
| pol | Find poles only |
| zer | Find zeros only |
| pz | Find both poles and zeros |

---

### noise — Noise Analysis

**Syntax:** `noise v(<output>[,<ref>]) <src> <dec|oct|lin> <np> <fstart> <fstop> [ptspersum]`

| Argument | Description |
|----------|-------------|
| v(output[,ref]) | Output node voltage expression |
| src | Input noise source (independent voltage or current source name) |
| dec/oct/lin | Frequency spacing |
| np | Points per interval |
| fstart | Start frequency (Hz) |
| fstop | Stop frequency (Hz) |
| ptspersum | Frequency points per noise summary line (default: all) |

Results: `inoise_spectrum`, `onoise_spectrum`, `inoise_total`, `onoise_total`.

---

### sens — Sensitivity Analysis

**Syntax (DC):** `sens <outvar>`

**Syntax (AC):** `sens <outvar> <dec|oct|lin> <np> <fstart> <fstop>`

| Argument | Description |
|----------|-------------|
| outvar | Output variable, e.g. `v(out)` or `v(out,ref)` or `i(vsrc)` |
| dec/oct/lin | Frequency spacing (for AC sens) |
| np | Number of frequency points |
| fstart/fstop | Frequency range (Hz) |

---

### disto — Distortion Analysis

**Syntax:** `disto <dec|oct|lin> <np> <fstart> <fstop> [<f2overf1>]`

| Argument | Default | Description |
|----------|---------|-------------|
| dec/oct/lin | — | Frequency spacing |
| np | — | Points per interval |
| fstart | — | Start frequency (Hz) |
| fstop | — | Stop frequency (Hz) |
| f2overf1 | 0.9 | Ratio of second frequency F2 to F1 |

---

### tf — Transfer Function

**Syntax:** `tf <outvar> <inputsrc>`

| Argument | Description |
|----------|-------------|
| outvar | Output variable: `v(node)`, `v(node1,node2)`, or `i(vsrc)` |
| inputsrc | Input source name (voltage or current source) |

Computes DC transfer function, input resistance, and output resistance.

---

### run — Execute Analyses from Netlist

**Syntax:** `run [rawfile]`

Runs all analyses specified via dot-cards (`.op`, `.ac`, `.tran`, …) in the loaded netlist.
If `rawfile` is given, results are written to that file.

---

### resume — Resume Interrupted Simulation

**Syntax:** `resume`

Resumes a transient simulation that was interrupted by `Ctrl-C`.

---

### reset — Reset Circuit

**Syntax:** `reset`

Resets the circuit to its initial state; clears all simulation results.

---

---

## Breakpoints and Control Flow Debugging

### stop — Set Breakpoint

**Syntax:**
```
stop after <n>
stop when <node> <cond> <value>
stop when <node> <cond> <value> after <n>
```

| Form | Description |
|------|-------------|
| `after n` | Stop after n iterations |
| `when v(node) > value` | Stop when condition is met |
| conditions | `>`, `<`, `>=`, `<=`, `=`, `<>` |

Multiple conditions can be chained with AND logic by repeating `when` clauses.

---

### trace — Trace a Node

**Syntax:** `trace [node ...]`

Prints the value of each listed node at every time point during simulation.
With no arguments, traces all saved nodes.

---

### iplot — Interactive Plot

**Syntax:** `iplot [node ...]`

Plots listed nodes dynamically during simulation.

---

### step — Step Simulation

**Syntax:** `step [n]`

Steps the simulation forward by `n` iterations (default 1).

---

### status — Show Breakpoints

**Syntax:** `status`

Prints all active breakpoints and traces.

---

### delete — Delete Breakpoint

**Syntax:** `delete [n ...]`

Deletes breakpoints/traces by number (from `status`). With no argument, deletes all.

---

### where — Show Trouble Node

**Syntax:** `where`

Prints the node or element that was causing convergence problems.

---

---

## Data and Vector Commands

### let — Assign Vector

**Syntax:** `let <varname> = <expr>`  
**Syntax:** `let` (no args — same as `display`)

Assigns the result of an expression to a named vector in the current plot.
Vectors are shared between plots when referenced.

Examples:
```
let vdb = db(v(out))
let gain = v(out) / v(in)
let vmax = max(v(out))
```

---

### unlet — Delete Vector

**Syntax:** `unlet <varname> ...`

Removes named vectors from the current plot.

---

### print — Print Vector Values

**Syntax:** `print [col] [line] <expr> ...`

| Option | Description |
|--------|-------------|
| col | Print in columnar format (one point per line) |
| line | Print in row format (one vector per line) |
| expr | Any vector expression |

Examples: `print v(out)`, `print col v(in) v(out)`.

---

### display — List Vectors

**Syntax:** `display [vector ...]`

Lists all vectors in the current plot (with types and lengths). With arguments,
shows details for those specific vectors.

---

### plot — Plot Vectors

**Syntax:** `plot <expr> ... [vs <expr>] [xl <xlo> <xhi>] [yl <ylo> <yhi>] [title <str>] [xlabel <str>] [ylabel <str>]`

| Option | Description |
|--------|-------------|
| vs expr | Use this vector as the X axis |
| xl xlo xhi | X axis limits |
| yl ylo yhi | Y axis limits |
| title str | Plot title |
| xlabel/ylabel | Axis labels |

---

### asciiplot — ASCII Text Plot

**Syntax:** `asciiplot <expr> ... [vs <expr>] [xl <xlo> <xhi>] [yl <ylo> <yhi>]`

Same arguments as `plot` but renders to the terminal in ASCII art.

---

### gnuplot — Send to gnuplot

**Syntax:** `gnuplot <file> <expr> ... [vs <expr>] [options]`

Writes a gnuplot script and data to `file`; launches gnuplot if available.

---

### wrdata — Write Plain Data

**Syntax:** `wrdata <file> <expr> ...`

Writes column-separated data (scale + vectors) to a text file. Simpler than `write`.

---

### write — Write Raw File

**Syntax:** `write [file] [expr ...]`

Writes simulation results in ngspice raw format (binary or ASCII depending on
`filetype` option) to `file`. If no file given, uses `rawfile` variable.

---

### wrs2p — Write S-Parameters

**Syntax:** `wrs2p <file>`

Writes current S-parameter data to a Touchstone `.s2p` file.

---

### load — Load Raw File

**Syntax:** `load [file ...]`

Loads simulation results from one or more raw files. Creates new plot(s).

---

### compose — Compose a Vector

**Syntax:** `compose <name> [parm=val ...]`  
**Syntax:** `compose <name> values <val> <val> ...`

| Parameter | Description |
|-----------|-------------|
| start=v | Start value |
| stop=v | End value |
| step=v | Step size |
| lin=n | n linearly spaced points |
| log=n | n logarithmically spaced points |
| dec=n | n points per decade (log) |
| center=v | Center of range |
| span=v | Size of range |
| gauss=n | n points from Gaussian distribution |
| mean=v | Gaussian mean |
| sd=v | Gaussian standard deviation |
| random=n | n random points |
| values v … | Explicit list of values |

---

### cross — Extract Crossing Index

**Syntax:** `cross <vecname> <number> [<vector> ...]`

Creates a scalar vector at the index where `vector` crosses zero for the `number`-th time.

---

### reshape — Reshape Vector Dimensions

**Syntax:** `reshape <vector> ... [<shape>]`

Changes the dimension sizes of multi-dimensional vectors without altering data.

---

### fft — Fast Fourier Transform

**Syntax:** `fft <vector> ...`

Computes FFT of each (real, time-domain) vector. Outputs a complex frequency-domain
vector in a new `fft1` plot. Time vector must be uniformly spaced.

Optional `set` variables affecting FFT:
- `specwindow` — window function: `none`, `hanning` (default), `cosine`, `flat_top`, `gaussian`, `triangle`, `bartlett`, `blackman`
- `specwindoworder` — order for Gaussian window (default 2)

---

### psd — Power Spectral Density

**Syntax:** `psd <vector> ...`

Like `fft` but outputs the one-sided power spectral density (V²/Hz).

---

### spec — Spectral Analysis

**Syntax:** `spec <fstart> <fstop> <fstep> <vector> ...`

Computes the spectrum using the given frequency grid, not FFT-based.

| Argument | Description |
|----------|-------------|
| fstart | Start frequency (Hz) |
| fstop | Stop frequency (Hz) |
| fstep | Frequency step (Hz) |

---

### fourier — Fourier Decomposition

**Syntax:** `fourier <fund_freq> <vector> ...`

Computes the Fourier coefficients (DC + first 9 harmonics) of a transient waveform
at the fundamental frequency `fund_freq`.

Output: prints harmonic table; writes to `fourier1` plot.

---

### linearize — Re-sample onto Uniform Grid

**Syntax:** `linearize [vector ...]`

Re-samples the current (transient) plot onto a uniform time grid using linear
interpolation. Required before FFT if the timestep was variable.

---

### meas — Measure Signal Properties

**Syntax:** `.meas[ure] <analysis> <name> <type> ...`  
Also available as interactive command: `meas <analysis> <name> <type> ...`

**Analysis types:** `tran`, `ac`, `dc`

**Measurement types:**

| Type | Syntax extension | Description |
|------|-----------------|-------------|
| `trig … targ …` | trig v(x) val=v rise=n  targ v(y) val=v rise=n | Delay between two events |
| `find <vec> at=<t>` | — | Value of vector at a time/frequency |
| `find <vec> when <vec2>=<val>` | | Value when another vector reaches a value |
| `when <vec>=<val>` | | Time when vector equals value |
| `avg <vec>` | [from=t1 to=t2] | Average over interval |
| `mean <vec>` | — | Alias for `avg` |
| `min <vec>` | — | Minimum value |
| `max <vec>` | — | Maximum value |
| `min_at <vec>` | — | Time at which minimum occurs |
| `max_at <vec>` | — | Time at which maximum occurs |
| `rms <vec>` | — | RMS value |
| `pp <vec>` | — | Peak-to-peak value |
| `integ <vec>` | — | Integral |
| `deriv <vec>` | — | Derivative |
| `param <expr>` | — | Arithmetic on previous meas results |

Common modifiers:
- `from=<t>`, `to=<t>` — restrict measurement window
- `td=<t>` — delay before measurement starts
- `rise=<n>`, `fall=<n>`, `cross=<n>` — Nth crossing event
- `val=<v>` — threshold value (default 0)

---

---

## Device and Model Inspection

### show — Show Device Parameters

**Syntax:** `show [<device> ...] [: <param> ...]`

Prints current parameter values for listed devices. Without arguments, shows all
devices. Colon separates device list from parameter list.

Examples: `show r1`, `show m1 : vth0 tox`.

---

### showmod — Show Model Parameters

**Syntax:** `showmod [<model> ...] [: <param> ...]`

Like `show` but for model parameters.

---

### alter — Alter Device Parameters

**Syntax:**
```
alter <device> <param> = <value>
alter <device> = <value>
alter @<device>[<param>] = <value>
alter @<device>[<param>] = [ <v1> <v2> ... ]
```

Changes a device parameter in the loaded circuit without re-parsing.
The `@device[param]` form is preferred for unambiguous access.

For MOS devices, `alter m1 w = 2u` or `alter m1 l = 0.18u` are common.
For sources: `alter v1 = 1.5` (changes DC value).
For pulse sources: `alter @v1[pulse] = [ 0 5 10n 10n 10n 50n 100n ]`.

---

### altermod — Alter Model Parameters

**Syntax:** `altermod <model> [<type>] <param> = <value>`  
**Syntax:** `altermod [<instance> ...] : <param> = <value>`

Changes model parameters. The colon form finds the model shared by all listed
instances and updates it.

---

### devhelp — Device Help

**Syntax:** `devhelp [<device>]`

Prints a list of all device types, or parameters for a specific device type.

---

### inventory — Circuit Inventory

**Syntax:** `inventory`

Prints a count of each device type in the loaded circuit.

---

---

## Circuit Commands

### source — Load Netlist or Script

**Syntax:** `source <file>`

Reads and executes a SPICE netlist or a `.control` script file.

---

### listing — Print Netlist

**Syntax:** `listing [logical] [physical] [expand]`

| Option | Description |
|--------|-------------|
| logical | Print the logical (expanded) netlist |
| physical | Print the physical (as-read) netlist |
| expand | Expand subcircuits inline |

Without options, prints the current netlist.

---

### edit — Edit Netlist

**Syntax:** `edit [file]`

Opens the netlist (or `file`) in the system editor (`$editor`). After editing,
the new netlist is loaded automatically.

---

### setcirc — Select Active Circuit

**Syntax:** `setcirc [n]`

Lists all loaded circuits, or selects circuit number `n` as the active one.

---

### remcirc / removecirc — Remove Circuit

**Syntax:** `remcirc`

Removes the currently active circuit from memory.

---

### reset — Reset Circuit State

**Syntax:** `reset`

Clears all analysis data and resets the circuit to initial conditions.

---

### snsave / snload — Snapshot

**Syntax:** `snsave <file>`  
**Syntax:** `snload <file>`

Saves/loads a complete simulation snapshot (circuit + results) to/from `file`.

---

---

## Variable Commands

### set — Set Variable

**Syntax:** `set [<varname> [= <value>]] ...`

With no args: prints all set variables. With name only: sets a boolean flag.
With `= value`: sets a typed variable.

**Key simulation variables (also settable via `.options`):**

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `gmin` | real | 1e-12 | Minimum parallel conductance (S) |
| `reltol` | real | 1e-3 | Relative tolerance |
| `abstol` | real | 1e-12 | Absolute current tolerance (A) |
| `vntol` | real | 1e-6 | Voltage tolerance (V) |
| `trtol` | real | 7.0 | Transient truncation error factor |
| `chgtol` | real | 1e-14 | Charge tolerance (C) |
| `pivtol` | real | 1e-13 | Matrix pivot absolute tolerance |
| `pivrel` | real | 1e-3 | Matrix pivot relative tolerance |
| `itl1` | int | 100 | DC iteration limit |
| `itl2` | int | 50 | DC transfer curve iteration limit |
| `itl4` | int | 10 | Transient iteration limit per point |
| `method` | string | `trap` | Integration method: `trap` or `gear` |
| `maxord` | int | 2 | Maximum integration order (1–6) |
| `tnom` | real | 27.0 | Nominal temperature (°C) for model params |
| `temp` | real | 27.0 | Circuit operating temperature (°C) |
| `gshunt` | real | 0 | Shunt conductance from all nodes to ground |
| `bypass` | int | 0 | Allow element bypass for unchanged elements |
| `noopiter` | flag | off | Skip iterative OP; go directly to Gmin stepping |
| `keepopinfo` | flag | off | Record OP for each small-signal analysis |
| `defw` | real | 100µm | Default MOSFET width |
| `defl` | real | 100µm | Default MOSFET length |
| `defad` | real | 0 | Default MOSFET drain area |
| `defas` | real | 0 | Default MOSFET source area |
| `srcsteps` / `itl6` | int | — | Number of source-stepping steps |
| `gminsteps` | int | — | Number of Gmin-stepping steps |
| `minbreak` | real | — | Minimum time between breakpoints |
| `nodedamping` | flag | off | Limit node voltage change between iterations |
| `numdgt` | int | 6 | Digits in printed numbers |
| `nomod` | flag | off | Suppress model summary |
| `nopage` | flag | off | Suppress page breaks |
| `acct` | flag | off | Print simulation accounting/statistics |

**Key interactive variables:**

| Variable | Type | Description |
|----------|------|-------------|
| `curplot` | string | Name of current active plot |
| `plots` | list | List of all plot names |
| `rawfile` | string | Default output rawfile name |
| `filetype` | string | `ascii` or `binary` raw file format |
| `history` | int | Number of history entries to keep |
| `prompt` | string | Command prompt string |
| `width` | int | Output line width |
| `height` | int | Output page height |
| `sourcepath` | list | Search path for `source` command |
| `editor` | string | Editor for `edit` command |
| `appendwrite` | flag | Append to rawfile instead of overwrite |
| `noaskquit` | flag | Exit without prompting |
| `nomoremode` | flag | Disable paged output |
| `noglob` | flag | Disable filename globbing |
| `nosort` | flag | Disable sorting of vector display |
| `units` | string | `degrees` or `radians` for phase |
| `specwindow` | string | FFT window: `none`, `hanning`, `cosine`, `flat_top`, `gaussian`, `triangle`, `bartlett`, `blackman` |
| `specwindoworder` | int | Gaussian window order (default 2) |
| `fourgridsize` | int | Grid size for fourier analysis |
| `dpolydegree` | int | Polynomial interpolation degree |

---

### unset — Unset Variable

**Syntax:** `unset <varname> ...`

Removes (undefines) named variables.

---

### option — Set Simulation Options

**Syntax:** `option [<name>=<value> ...]`  
Alias: `options`

Like `set` but specifically for simulation option variables; also prints a
summary of all current option values when called with no arguments.

---

---

## Plot Management

### setplot — Select Active Plot

**Syntax:** `setplot [<plotname>]`

Lists all available plots, or sets the named plot as current.
Special names: `new` (create a new empty plot).

---

### destroy — Delete Plot

**Syntax:** `destroy [<plotname> ...]`

Removes plots from memory. Without arguments, removes current plot.

---

### setscale — Set Plot Scale

**Syntax:** `setscale <vector>`

Sets the named vector as the scale (X-axis) for the current plot.

---

### diff — Diff Two Plots

**Syntax:** `diff <plot1> <plot2> [<vec> ...]`

Compares vectors in two plots and reports differences. Useful for regression.

---

### transpose — Transpose Multi-Dimensional Vector

**Syntax:** `transpose <varname> ...`

Performs matrix transposition on multi-dimensional vectors.

---

---

## Script Flow Control

These keywords are only valid inside `.control ... .endc` blocks or sourced scripts.

### if / else / end

```
if <condition>
  ...
else
  ...
end
```

Conditions use standard comparison operators: `=`, `<>`, `>`, `<`, `>=`, `<=`.

---

### while / end

```
while <condition>
  ...
end
```

Executes the block while condition is TRUE.

---

### dowhile / end

```
dowhile <condition>
  ...
end
```

Executes block at least once, then repeats while condition is TRUE.

---

### repeat / end

```
repeat [<n>]
  ...
end
```

Repeats block `n` times (or forever if `n` omitted).

---

### foreach / end

```
foreach <varname> <val1> <val2> ...
  ...
end
```

Iterates over listed values, setting `$varname` each time.

---

### break

Exits the innermost `while`, `dowhile`, `repeat`, or `foreach` loop.

---

### continue

Skips to the next iteration of the innermost loop.

---

### label / goto

```
label <word>
goto <word>
```

Defines a jump target; `goto` transfers control there. Useful for retry loops.

---

---

## Alias and Definition Commands

### define — Define Function

**Syntax:** `define <funcname>(<args>) <expr>`  
**Syntax:** `define` (no args — lists all defined functions)

Defines a user function.

Example: `define db(x) 20*log10(abs(x))`

---

### undefine — Remove Function

**Syntax:** `undefine <funcname> ...`

Removes a user-defined function.

---

### alias — Define Command Alias

**Syntax:** `alias <word> <command>`  
**Syntax:** `alias` (no args — lists all aliases)

---

### unalias — Remove Alias

**Syntax:** `unalias <word> ...`

---

### deftype — Define Vector Type

**Syntax:** `deftype <spec> <name> <pat> ...`

Redefines the type name associated with a vector type code. `spec` is `v` (vector)
or `p` (plot).

---

### settype — Change Vector Type

**Syntax:** `settype <type> <vec> ...`

Changes the type annotation of a vector (e.g., from `voltage` to `current`).

---

---

## Shell and Utility Commands

### echo — Print Text

**Syntax:** `echo [<text> ...]`

Prints arguments to output. Supports `$varname` expansion.

---

### shell — Run Shell Command

**Syntax:** `shell [<command>]`

Runs a shell command. Without argument, opens an interactive shell.

---

### cd — Change Directory

**Syntax:** `cd [<dir>]`

Changes the current working directory.

---

### history — Show Command History

**Syntax:** `history [-r] [n]`

Prints last `n` commands (default: all). `-r` reverses order.

---

### shift — Shift Argument List

**Syntax:** `shift [<varname>] [<n>]`

Shifts a list variable left by `n` positions (default 1). Used in scripts.

---

### strcmp — String Compare

**Syntax:** `strcmp <resultvar> <s1> <s2>`

Sets `$resultvar` to the C `strcmp` result of `s1` vs `s2`.

---

### version — Print Version

**Syntax:** `version [-s]`

Prints ngspice version string.

---

### rusage — Resource Usage

**Syntax:** `rusage [<resource> ...]`

Prints resource usage statistics (CPU time, memory, iteration counts).

Resources: `time`, `space`, `totiter`, `traniter`, `tranpoints`, `accept`,
`rejected`, `equations`, `loadtime`, `factortime`, `solvetime`.

---

### rehash — Rebuild Command Hash

**Syntax:** `rehash`

Rebuilds the command completion hash table (used after adding commands).

---

### help / newhelp / tutorial

**Syntax:** `help [<topic>]`

Prints help text. `newhelp` and `tutorial` use the HTML-based help system.

---

### quit / exit

**Syntax:** `quit [<code>]`

Exits ngspice. Optional exit code returned to shell.

---

---

## Save and Probe

### save — Save Specific Vectors

**Syntax:** `save [all | <node> | @<device>[<param>] ...]`

Selects which vectors to save during simulation (reduces memory use).
Without arguments when called after `stop`, saves current state.

- `save all` — save everything (default)
- `save v(out) i(v1)` — save only these
- `save @m1[id]` — save device operating point current

---

### probe — Alias for save

**Syntax:** `probe <nodes ...>`

Equivalent to `save` in ngspice; marks nodes for output.

---

---

## Dot-Card Analysis Control (Netlist Syntax)

These are netlist statements (not interactive commands) but control simulation behavior.

| Dot-card | Syntax | Description |
|----------|--------|-------------|
| `.op` | `.op` | Operating point |
| `.dc` | `.dc <src> <start> <stop> <step>` | DC sweep |
| `.ac` | `.ac <dec/oct/lin> <np> <fstart> <fstop>` | AC analysis |
| `.tran` | `.tran <tstep> <tstop> [tstart [tmax]] [uic]` | Transient |
| `.pz` | `.pz <ni> <ng> <nj> <nk> vol/cur pol/zer/pz` | Pole-zero |
| `.noise` | `.noise V(out) srcname <pts>` | Noise |
| `.sens` | `.sens <outvar> [ac ...]` | Sensitivity |
| `.disto` | `.disto <pts> <f1> <f2> [f2ovf1]` | Distortion |
| `.tf` | `.tf <outvar> <inputsrc>` | Transfer function |
| `.options` | `.options [key=val ...]` | Simulation options |
| `.ic` | `.ic V(node)=val ...` | Initial conditions |
| `.nodeset` | `.nodeset V(node)=val ...` | Initial node guesses |
| `.meas` | `.meas <an> <name> <type> ...` | Measurement |
| `.save` | `.save <nodes ...>` | Select saved vectors |
| `.probe` | `.probe <nodes ...>` | Alias for `.save` |
| `.include` | `.include <file>` | Include another file |
| `.lib` | `.lib [file] <section>` | Load library section |
| `.param` | `.param <name>=<val>` | Circuit parameter |
| `.subckt` | `.subckt <name> <ports>` | Subcircuit definition |
| `.ends` | `.ends [name]` | End subcircuit |
| `.model` | `.model <name> <type> [params]` | Model definition |
| `.global` | `.global <node> ...` | Declare global nodes |
| `.temp` | `.temp <T1> [T2 ...]` | Temperature sweep |
| `.control` | `.control` | Begin control script block |
| `.endc` | `.endc` | End control script block |
| `.end` | `.end` | End of netlist |

---

## Predefined Script Variables

These variables are pre-defined at startup in `.control` and interactive
interpreter contexts (source: `src/frontend/cpitf.c`).

### Physical Constants and Math

| Variable  | Value                  | Description                           |
|-----------|------------------------|---------------------------------------|
| `pi`      | 3.14159265358979…      | π                                     |
| `e`       | 2.71828182844590…      | Euler's number                        |
| `c`       | 2.997925e8             | Speed of light (m/s)                  |
| `i`       | 0+1j                   | Imaginary unit (complex)              |
| `kelvin`  | -273.15                | 0 K in °C (use to convert: `T+kelvin` gives T in K) |
| `echarge` | 1.60219e-19            | Elementary charge (C)                 |
| `boltz`   | 1.38062e-23            | Boltzmann constant (J/K)              |
| `planck`  | 6.62620e-34            | Planck constant (J·s)                 |
| `yes`     | 1                      | Boolean true alias                    |
| `TRUE`    | 1                      | Boolean true alias                    |
| `no`      | 0                      | Boolean false alias                   |
| `FALSE`   | 0                      | Boolean false alias                   |

### Plot State Variables

| Variable        | Type   | Description                                              |
|-----------------|--------|----------------------------------------------------------|
| `curplot`       | string | Name of the currently active plot (e.g., `tran1`)       |
| `curplotname`   | string | Human-readable name of the current plot                  |
| `curplottitle`  | string | Title string of the current plot                         |
| `curplotdate`   | string | Timestamp when current plot was created                  |
| `plots`         | list   | Names of all plots in the current session                |

### Pre-defined Functions (auto-defined with `define`)

| Function    | Expansion                         | Description                     |
|-------------|-----------------------------------|---------------------------------|
| `max(x,y)`  | `(x gt y)*x + (x le y)*y`        | Maximum of two vectors          |
| `min(x,y)`  | `(x lt y)*x + (x ge y)*y`        | Minimum of two vectors          |
| `vdb(x)`    | `db(v(x))`                        | Node voltage in dB              |
| `vdb(x,y)`  | `db(v(x) - v(y))`                 | Differential voltage in dB      |
| `vm(x)`     | `mag(v(x))`                       | Magnitude of node voltage       |
| `vm(x,y)`   | `mag(v(x) - v(y))`                | Magnitude of differential       |
| `vp(x)`     | `ph(v(x))`                        | Phase of node voltage (degrees) |
| `vr(x)`     | `re(v(x))`                        | Real part of node voltage       |
| `vi(x)`     | `im(v(x))`                        | Imaginary part of node voltage  |
| `vg(x)`     | `group_delay(v(x))`               | Group delay at node             |
| `gd(x)`     | `group_delay(v(x))`               | Alias for group delay           |

---

## Background Simulation (Shared Library / libngspice Only)

When using ngspice as `libngspice.so`, these commands run the simulation in a
background thread so the embedding application stays responsive. They are
not available in standalone interactive mode.

| Command      | Description                                                     |
|--------------|-----------------------------------------------------------------|
| `bg_run`     | Start background simulation (equivalent to `run` but async)    |
| `bg_halt`    | Pause a running background simulation                           |
| `bg_resume`  | Resume a halted background simulation                           |
| `bg_pstop`   | Stop background simulation at next pause point                  |

The embedding application polls via the `ngSpice_Circ()` / `ngSpice_Command()` API
and uses callbacks (`SendChar`, `SendStat`, `ControlledExit`, `SendData`,
`SendInitData`, `BGThreadRunning`) to receive output.

---

## Event-Driven (Digital / XSpice) Commands

These commands display or export event-driven (digital) simulation data
from XSpice event-driven nodes. Excluded from piperine scope but listed for
completeness.

| Command         | Description                                              |
|-----------------|----------------------------------------------------------|
| `eprint`        | Print event-driven (digital) node values                 |
| `edisplay`      | Display all event-driven node names                      |
| `eprvcd`        | Export event-driven data as VCD (Value Change Dump)      |
| `esave`         | Select event-driven nodes to save                        |

---

## Additional Vector Operations

### cutout — Extract Sub-Range of Vector

**Syntax:** `cutout <vec> [<start> [<end>]]`

Extracts a contiguous sub-range from a vector by index or time value.
Useful for analyzing a specific time window.

### destroy — Delete Plot(s)

**Syntax:** `destroy [<plotname> ...]`

Without arguments, destroys the current plot. With plot names, destroys
those specific plots. Frees memory.

```spice
destroy tran1 tran2 tran3
```

Use after Monte Carlo runs to reclaim memory.

### diff — Compare Two Plots

**Syntax:** `diff <plot1> <plot2> [<tol>]`

Compares all vectors in `plot1` against `plot2`. Reports vectors that
differ by more than `tol` (default from `set diff_abstol`, `diff_reltol`,
`diff_vntol`).

### transpose — Swap Dimensions

**Syntax:** `transpose <vecname>`

Transposes a multi-dimensional vector (e.g., swaps rows and columns).
Used with 2D AC sweep results.

---

## File I/O Commands

### write — Write Raw File

**Syntax:** `write [<filename>] [<vec1> <vec2> ...]`

Writes selected (or all) vectors to a binary ngspice raw file.
Default filename is `ngspice.raw` or the value of `set rawfile`.

```spice
write results.raw v(out) v(in)
```

Format is binary by default. Set `set filetype = ascii` for text.

### wrdata — Write Plain ASCII Data

**Syntax:** `wrdata [<filename>] <vec1> [<vec2> ...]`

Writes two-column plain text: scale vector (e.g., time) and data vector.
No header. Easy to import into external tools.

```spice
wrdata output.txt v(out)
```

### wrs2p — Write S-Parameters (Touchstone)

**Syntax:** `wrs2p <filename>`

Writes the current S-parameter data in Touchstone `.s2p` format.
Requires a prior S-parameter simulation (2-port via `sp` analysis or
port element setup).

### wrnodev — Write Vector Excluding Devices

**Syntax:** `wrnodev [<filename>]`

Like `write`, but excludes device branch currents. Useful for writing
compact voltage-only raw files.

### alterparam — Alter Parameter Value

**Syntax:** `alterparam <param> = <value>`

Changes a `.param` value after circuit load. Unlike `alter` (which targets
device instance parameters), `alterparam` updates circuit-level parameters.

```spice
.control
  set vdd = 1.8
  alterparam vdd = {$vdd}
  run
.endc
```

---

## Important `set` Variables Reference

Key variables that affect simulation behavior when set in `.control` or
`.spiceinit`:

| Variable          | Type    | Description                                              |
|-------------------|---------|----------------------------------------------------------|
| `ngbehavior`      | string  | Compatibility mode: `all`, `hs`, `ps`, `spice3`         |
| `rndseed`         | integer | RNG seed for Monte Carlo (see NGSPICE_STATISTICAL.md)   |
| `rawfile`         | string  | Default raw file path for `write`                        |
| `filetype`        | string  | `binary` or `ascii` for raw file format                  |
| `rawfileprec`     | integer | Decimal digits in ASCII raw file (default 15)            |
| `numdgt`          | integer | Display digits in `print` output                         |
| `sourcepath`      | list    | Search paths for `.include` / `.lib` files               |
| `noaskquit`       | bool    | Suppress quit confirmation prompt                        |
| `nomod`           | bool    | Suppress model parameter listing                         |
| `nomoremode`      | bool    | Suppress `--more--` paging in long output                |
| `appendwrite`     | bool    | Append to rawfile instead of overwriting                 |
| `nosubckt`        | bool    | Disable subcircuit expansion (debug)                     |
| `notrnoise`       | bool    | Disable transient noise sources                          |
| `diff_abstol`     | real    | Absolute tolerance for `diff` command (default 1e-12)    |
| `diff_reltol`     | real    | Relative tolerance for `diff` command (default 1e-3)     |
| `diff_vntol`      | real    | Voltage tolerance for `diff` command (default 1e-6)      |
| `width`           | integer | Output line width for `asciiplot` and print (default 80) |
| `height`          | integer | Plot height in lines for `asciiplot`                     |
| `polydegree`      | integer | Polynomial degree for `plot` curve fitting               |
| `polysteps`       | integer | Number of steps for polynomial plots                     |
| `history`         | integer | Number of commands kept in history                       |
| `prompt`          | string  | Interactive prompt string                                |
| `unixcom`         | bool    | Allow shell commands without `!` prefix                  |
