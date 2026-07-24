"""Type stub for the native `_piperine` PyO3 extension (T28 / HOST-26).

The compiled `_piperine*.so` carries no type information of its own — this
hand-written stub is what makes IDE autocomplete/type-checking work for the
native pyclasses the pure-Python `piperine/__init__.py` facade re-exports
by alias (e.g. ``Port = _piperine._Port``). Every class here backs a public
name in ``piperine.__all__``; see `__init__.py`'s own docstrings for the
higher-level narrative — this stub documents shape (methods, getters,
argument/return types), not usage recipes.
"""

from __future__ import annotations

from typing import Any, Iterator

# ── load ─────────────────────────────────────────────────────────────────

def load(path: str) -> _Design:
    """Load + elaborate a `.phdl`/`.ppr` file into a `_Design`."""

def load_str(src: str) -> _Design:
    """Elaborate PHDL/PPR source text directly into a `_Design` (no
    filesystem read)."""

# ── reflection: design + module + children ──────────────────────────────

class _Design:
    """A loaded, elaborated POM design."""

    def top(self) -> _Module | None: ...
    def module(self, name: str) -> _Module: ...
    def modules(self) -> list[_Module]: ...
    def const_(self, name: str) -> Any: ...
    def select(self, path: str) -> _Selection: ...

class _Module:
    """A reflected view of one POM module + the uniform analyses."""

    @property
    def name(self) -> str: ...
    def ports(self) -> list[_Port]: ...
    def nets(self) -> list[_Net]: ...
    def instances(self) -> list[_Instance]: ...
    def params(self) -> list[_Param]: ...
    def behaviors(self) -> list[_Behavior]: ...
    def op(self, nodeset: dict[str, float] | None = ..., solver: Any | None = ...) -> _OpResult: ...
    def sens(
        self,
        outputs: list[str],
        params: list[tuple[str, str]],
        dp_rel: float = ...,
        solver: Any | None = ...,
    ) -> dict[tuple[str, str], float]: ...
    def pss(
        self, period: float, tstab: float = ..., solver: Any | None = ...
    ) -> tuple[_Trace, int, float, float | None]: ...
    def pz(
        self,
        input_source: str,
        output: str,
        output_ref: str | None = ...,
        solver: Any | None = ...,
    ) -> tuple[list[complex], list[complex]]: ...
    def disto(
        self,
        f1: float,
        amplitude: float,
        output: str,
        f2: float | None = ...,
        output_ref: str | None = ...,
        solver: Any | None = ...,
    ) -> tuple[float | None, float | None, float | None, float | None]: ...
    def sp(
        self,
        fstart: float,
        fstop: float,
        points: int = ...,
        logarithmic: bool = ...,
        solver: Any | None = ...,
    ) -> tuple[list[float], list[list[list[complex]]], list[float], int]: ...
    def tran(
        self,
        stop: float,
        step: float | None = ...,
        start: float = ...,
        ic: dict[str, float] | None = ...,
        solver: Any | None = ...,
        record_device_state: bool = ...,
        probe: list[str] = ...,
    ) -> _Trace: ...
    def ac(
        self,
        fstart: float,
        fstop: float,
        points: int = ...,
        logarithmic: bool = ...,
        solver: Any | None = ...,
    ) -> _AcTrace: ...
    def noise(
        self,
        out: str,
        fstart: float,
        fstop: float,
        points: int = ...,
        reference: str = ...,
        logarithmic: bool = ...,
        solver: Any | None = ...,
    ) -> _NoiseTrace: ...
    def set(self, label: str, param: str, value: float) -> None: ...
    def compile(self) -> _Session: ...

class _Port:
    """A reflected module port."""

    @property
    def name(self) -> str: ...
    @property
    def direction(self) -> str: ...
    @property
    def ty(self) -> str: ...

class _Net:
    """A reflected `wire` declaration."""

    @property
    def name(self) -> str: ...
    @property
    def ty(self) -> str: ...

class _Instance:
    """A reflected submodule instance."""

    @property
    def name(self) -> str: ...
    @property
    def module(self) -> str: ...

class _Param:
    """A reflected module parameter (name/type/default — not the runtime
    `_ParamDescriptor` bounds/unit/scope catalog)."""

    @property
    def name(self) -> str: ...
    @property
    def ty(self) -> str: ...
    @property
    def default(self) -> Any: ...

class _Behavior:
    """A reflected `analog`/`digital` behavior block."""

    @property
    def name(self) -> str: ...
    @property
    def kind(self) -> str: ...

class _Selection:
    """The typed result of `_Design.select` — matched POM nodes."""

    def len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def nodes(self) -> list[_Node]: ...

class _Node:
    """One matched POM node from a selector resolution."""

    @property
    def kind(self) -> str: ...
    @property
    def name(self) -> str: ...

# ── instance sub-views + introspection catalogs (HOST-07/09/12) ─────────

class _InstanceView:
    """A per-instance sub-view: terminal quantities + opvars + static
    introspection catalogs (model/terminals/observables/params)."""

    @property
    def label(self) -> str: ...
    def terminal_connections(self) -> list[_Terminal]: ...
    def v(self, port_a: str, port_b: str | None = ...) -> float | _Waveform: ...
    def i(self, port_a: str, port_b: str | None = ...) -> float | _Waveform: ...
    def __getitem__(self, port: str) -> float | _Waveform: ...
    def opvar(self, name: str) -> float: ...
    def opvars(self) -> list[tuple[str, float]]: ...
    @property
    def model(self) -> _ModelDescriptor: ...
    @property
    def terminals(self) -> list[_TerminalDescriptor]: ...
    def observables(self) -> list[_ObservableDescriptor]: ...
    def param(self, name: str) -> _ParamDescriptor: ...
    def params(self) -> list[_ParamDescriptor]: ...

class _Terminal:
    """One `_InstanceView.terminal_connections()` entry: port name + the
    top-level net it connects to."""

    @property
    def port(self) -> str: ...
    @property
    def net(self) -> str: ...

class _ModelDescriptor:
    """Model identity + version (HOST-09 / ABI-46)."""

    type_id: str
    version: str

class _TerminalDescriptor:
    """One terminal's static metadata (HOST-09 / ABI-27)."""

    name: str
    kind: str  # "external" | "internal" | "auxiliary"
    domain: str  # "analog" | "digital"
    direction: str  # "in" | "out" | "inout"

class _ObservableDescriptor:
    """One device-declared observable (HOST-09 / ABI-32)."""

    name: str
    kind: str  # "branch_current" | "charge" | "flux" | "state" | "var"
    cost: float

class _ParamDescriptor:
    """One parameter's metadata: bounds/unit/scope/invalidation
    (HOST-12)."""

    name: str
    bounds: tuple[float | None, float | None]
    unit: str | None
    scope: str
    invalidation: str

# ── analysis results (HOST-02/03/04/07/10/11/13) ─────────────────────────

class _LimitingReport:
    """One device's structured limiting diagnostic (HOST-10 / ABI-09)."""

    device: str
    net: str
    proposed: float
    limited_value: float
    limiter_name: str
    reason: str

class _NoiseContribution:
    """One noise source's contribution to the output noise (HOST-11)."""

    element: str
    source: str
    kind: str  # "thermal" | "shot" | "flicker" | "other"
    integrated_sq: float

class _SolverStats:
    """Per-analysis convergence + performance diagnostics."""

    newton_iterations: int
    converged: bool
    steps_accepted: int
    steps_rejected: int
    dt_min_floor_hits: int
    dt_min: float
    dt_max: float
    bypass_hits: int
    bypass_misses: int
    homotopy_strategy: str | None
    homotopy_levels: int
    assembly_time_ns: int
    solve_time_ns: int
    limiting: list[_LimitingReport]

class _OpResult:
    """The typed DC operating-point result (spec AC4/AC5)."""

    def v(self, a: str, b: str | None = ...) -> float: ...
    def i(self, a: str, b: str | None = ...) -> float: ...
    @property
    def stats(self) -> _SolverStats: ...
    def __getitem__(self, name: str) -> float | _InstanceView: ...

class _Trace:
    """The typed transient/DC-sweep result: `.v`/`.i` read a per-net
    `_Waveform` over the trace's axis."""

    def v(self, a: str, b: str | None = ...) -> _Waveform: ...
    def i(self, a: str, b: str | None = ...) -> _Waveform: ...
    def axis(self) -> _Waveform: ...
    @property
    def stats(self) -> _SolverStats: ...
    def __getitem__(self, name: str) -> _Waveform | _InstanceView: ...

class _Waveform:
    """A real-valued series of `(axis, value)` samples."""

    @property
    def values(self) -> Any: ...  # np.ndarray[float64]
    @property
    def axis(self) -> Any: ...  # np.ndarray[float64]
    def at(self, x: float) -> float: ...
    def rms(self) -> float: ...
    def mean(self) -> float: ...
    def min(self) -> float: ...
    def max(self) -> float: ...
    def peak_to_peak(self) -> float: ...
    def cross(self, level: float, dir: str = ...) -> float | None: ...
    def len(self) -> int: ...
    def __len__(self) -> int: ...
    def is_empty(self) -> bool: ...
    def fourier(self, f0: float, n_harmonics: int) -> _FourierResult: ...

class _FourierComponent:
    """One harmonic component of a `.fourier()` decomposition."""

    frequency: float
    magnitude: float
    phase: float
    norm_magnitude: float
    norm_phase: float

class _FourierResult:
    """A `.fourier()` result: fundamental frequency, harmonics, THD."""

    fundamental: float
    harmonics: list[_FourierComponent]
    thd: float

class _ComplexWaveform:
    """A complex-valued series of `(frequency, value)` samples (AC)."""

    @property
    def values(self) -> Any: ...  # np.ndarray[complex128]
    @property
    def axis(self) -> Any: ...  # np.ndarray[float64]
    @property
    def mag(self) -> _Waveform: ...
    @property
    def phase(self) -> _Waveform: ...
    @property
    def db(self) -> _Waveform: ...
    def at(self, x: float) -> complex: ...
    def len(self) -> int: ...
    def __len__(self) -> int: ...
    def is_empty(self) -> bool: ...

class _AcTrace:
    """The typed AC small-signal sweep result."""

    def v(self, a: str, b: str | None = ...) -> _ComplexWaveform: ...
    def axis(self) -> _Waveform: ...

class _NoiseTrace:
    """The typed output-referred noise result."""

    def psd(self) -> _Waveform: ...
    def total(self) -> float: ...
    def by_source(self) -> dict[str, _Waveform]: ...
    def contributions(self) -> list[_NoiseContribution]: ...

class _TfResult:
    """The `.tf` result: DC small-signal gain + input/output resistance."""

    gain: float
    z_in: float
    z_out: float

# ── live session (compile once, set, re-run — HOST-01/02/18/19) ─────────

class _Session:
    """A compiled circuit held live across analyses."""

    @property
    def rebuilds(self) -> int: ...
    def set(self, label: str, param: str, value: float) -> None: ...
    def op(self, nodeset: dict[str, float] | None = ..., solver: Any | None = ...) -> _OpResult: ...
    def schedule_set(self, t: float, label: str, param: str, value: float) -> None: ...
    def tran(
        self,
        stop: float,
        step: float | None = ...,
        start: float = ...,
        ic: dict[str, float] | None = ...,
        solver: Any | None = ...,
        record_device_state: bool = ...,
        probe: list[str] = ...,
    ) -> _Trace: ...
    def ac(
        self,
        fstart: float,
        fstop: float,
        points: int = ...,
        logarithmic: bool = ...,
        solver: Any | None = ...,
    ) -> _AcTrace: ...
    def noise(
        self,
        out: str,
        fstart: float,
        fstop: float,
        points: int = ...,
        reference: str = ...,
        logarithmic: bool = ...,
        solver: Any | None = ...,
    ) -> _NoiseTrace: ...
    def sens(
        self,
        outputs: list[str],
        params: list[tuple[str, str]],
        dp_rel: float = ...,
        solver: Any | None = ...,
    ) -> dict[tuple[str, str], float]: ...
    def pss(
        self, period: float, tstab: float = ..., solver: Any | None = ...
    ) -> tuple[_Trace, int, float, float | None]: ...
    def pz(
        self,
        input_source: str,
        output: str,
        output_ref: str | None = ...,
        solver: Any | None = ...,
    ) -> tuple[list[complex], list[complex]]: ...
    def disto(
        self,
        f1: float,
        amplitude: float,
        output: str,
        f2: float | None = ...,
        output_ref: str | None = ...,
        solver: Any | None = ...,
    ) -> tuple[float | None, float | None, float | None, float | None]: ...
    def sp(
        self,
        fstart: float,
        fstop: float,
        points: int = ...,
        logarithmic: bool = ...,
        solver: Any | None = ...,
    ) -> tuple[list[float], list[list[list[complex]]], list[float], int]: ...
    def tf(
        self,
        output: str,
        input_source: str,
        output_ref: str | None = ...,
        solver: Any | None = ...,
    ) -> _TfResult: ...
    def dc(
        self,
        label: str,
        param: str,
        values: list[float],
        nodeset: dict[str, float] | None = ...,
        solver: Any | None = ...,
    ) -> _Trace: ...
    def sweep(self, label: str, param: str, values: list[float]) -> _Sweep: ...
    def sweep_grid(self, axes: list[tuple[str, str, list[float]]]) -> _Grid: ...

class _Sweep:
    """The native single-knob sweep iterator behind `Session.sweep`."""

    def __len__(self) -> int: ...
    def __iter__(self) -> Iterator[tuple[float, int]]: ...
    def __next__(self) -> tuple[float, int]: ...

class _Grid:
    """The native multi-axis sweep grid iterator behind
    `Session.sweep_grid`."""

    def __len__(self) -> int: ...
    def __iter__(self) -> Iterator[tuple[list[float], list[int]]]: ...
    def __next__(self) -> tuple[list[float], list[int]]: ...
