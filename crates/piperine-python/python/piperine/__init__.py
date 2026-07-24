"""Piperine — Python bindings for analog/mixed-signal circuit simulation.

The typed public surface of the Piperine simulator (spec §10 — the uniform
host-neutral API). This pure-Python facade wraps the native ``_piperine``
extension (PyO3) so IDEs see full annotations + docstrings; runtime forwards
to the native engine with negligible cost.

Uniform shape (PY-15, binding): the call graph mirrors the Rust host
session — ::

    import piperine
    design  = piperine.load("chip.phdl")        # -> Design
    module  = design.module("Amp")               # -> Module
    op      = module.op()                        # -> OpResult
    v_out   = op.v("out")                        # -> float
    trace   = module.tran(TranConfig(stop=1e-3, step=1e-6))  # -> Trace
    wave    = trace.v("out")                     # -> Waveform
    values  = wave.values                        # -> np.ndarray (real)
    axis    = wave.axis                          # -> np.ndarray (time)

Analyses are pure functions of (design + staged overrides + config); sweeps
are native Python ``for`` loops driving ``module.set(label, param, value)``
(spec AC11/12).

Numpy arrays: ``Waveform.values`` / ``.axis`` are real ``np.ndarray``;
``ComplexWaveform.values`` is complex128 (spec AC7/8).
"""

from __future__ import annotations

import dataclasses
import functools
import re
import typing
from dataclasses import dataclass, field
from enum import Enum

import _piperine

__all__ = [
    # load
    "load",
    # reflection
    "Design",
    "Module",
    "Port",
    "Net",
    "Instance",
    "Param",
    "Behavior",
    "Selection",
    "Node",
    # instance sub-views + solver statistics
    "InstanceView",
    "Terminal",
    "ModelDescriptor",
    "TerminalDescriptor",
    "ObservableDescriptor",
    "ParamDescriptor",
    "SolverStats",
    "LimitingReport",
    "NoiseContribution",
    # live session (compile once, set, re-run)
    "Session",
    "Sweep",
    "SweepPoint",
    "Grid",
    # analyses
    "OpResult",
    "Trace",
    "PssResult",
    "PssStats",
    "SensResult",
    "PoleZeroResult",
    "SpResult",
    "TfResult",
    "Waveform",
    "ComplexWaveform",
    "FourierComponent",
    "FourierResult",
    "AcTrace",
    "NoiseTrace",
    # config bundles (mirror headers/prelude.phdl)
    "Scale",
    "CrossDirection",
    "Direction",
    "Solver",
    "OpConfig",
    "TranConfig",
    "AcConfig",
    "NoiseConfig",
    # plotting (HOST-17, matplotlib-guarded)
    "plot",
    "bode",
    # SI unit helpers (HOST-21)
    "Hz",
    "ns",
    "mV",
    "C",
    # exception hierarchy (HOST-22)
    "SimulationError",
    "ElaborationError",
    "UnknownModule",
    "UnknownNet",
    "ConvergenceError",
]


# ── exception hierarchy (HOST-22) ──────────────────────────────────────────
#
# Every subclass also inherits the matching builtin exception type
# (`KeyError`/`ValueError`/`RuntimeError`) so existing `except KeyError`-style
# code (including LIVE-11's `Session.set` error-parity contract) keeps
# working completely unchanged — these are additive, more specific types
# layered over the same builtin taxonomy, not a replacement for it.


class SimulationError(Exception):
    """Base exception for every host-facing simulation failure (HOST-22).

    A raw native failure that doesn't fit a more specific subclass below is
    never silently swallowed — it propagates as its original builtin type
    (`KeyError`/`ValueError`/`RuntimeError`) unchanged; this hierarchy only
    adds sharper types for the failure modes it can positively identify.
    """


class ElaborationError(SimulationError, ValueError):
    """PHDL elaboration failed (parse / const-eval / instantiation) —
    raised by :func:`load` on a bad source file."""


class UnknownModule(SimulationError, ValueError):
    """A referenced module name does not exist in the design — raised by
    :meth:`Design.module` (a ``ValueError`` subclass: the native lookup
    already raises ``ValueError``, so this stays compatible with existing
    ``except ValueError`` code)."""


class UnknownNet(SimulationError, KeyError):
    """A referenced net/instance/param name is not addressable in the
    compiled circuit."""


class ConvergenceError(SimulationError, RuntimeError):
    """The Newton solver failed to converge.

    ``node``/``iteration``/``analysis`` are best-effort diagnostics: parsed
    from the underlying solver message when present, ``None`` otherwise —
    the solver's convergence-failure message does not always name the
    offending node.
    """

    def __init__(
        self,
        message: str,
        node: str | None = None,
        iteration: int | None = None,
        analysis: str | None = None,
    ) -> None:
        super().__init__(message)
        self.node = node
        self.iteration = iteration
        self.analysis = analysis


def _classify_analysis_error(exc: Exception, analysis: str) -> Exception | None:
    """Map a raw native analysis exception onto the [`SimulationError`]
    hierarchy (HOST-22) by message content, or return ``None`` to leave it
    unchanged (unmatched — passes through as its original builtin type).
    """
    if isinstance(exc, SimulationError):
        return None
    msg = str(exc)
    if "Failed to converge" in msg:
        match = re.search(r"after (\d+) iterations", msg)
        iteration = int(match.group(1)) if match else None
        return ConvergenceError(msg, iteration=iteration, analysis=analysis)
    if "is not addressable" in msg or "is not a solved analog net" in msg:
        return UnknownNet(msg)
    return None


def _wrap_analysis_errors(fn):
    """Decorator (HOST-22): call `fn`, reclassifying any raised exception
    through :func:`_classify_analysis_error`; an unmatched exception
    re-raises completely unchanged (same type, same traceback) — this is
    purely additive, never a behavior change for an already-tested raise
    site.
    """

    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        try:
            return fn(*args, **kwargs)
        except Exception as exc:
            mapped = _classify_analysis_error(exc, fn.__name__)
            if mapped is not None:
                raise mapped from exc
            raise

    return wrapper


# ── config bundles (mirror crates/piperine-lang/headers/prelude.phdl) ─────────


class Scale(Enum):
    """Frequency-sweep scale (prelude ``enum Scale``)."""

    Lin = "Lin"
    Dec = "Dec"
    Oct = "Oct"


class CrossDirection(Enum):
    """Threshold-crossing search direction (HOST-23) for
    :meth:`Waveform.cross`/:meth:`ComplexWaveform.mag`'s crossing helpers:
    ``wf.cross(1.0, CrossDirection.Rising)``. Its ``.value`` is exactly the
    string the native `.cross()` still accepts, so either spelling works.
    """

    Rising = "Rising"
    Falling = "Falling"
    Either = "Either"


class Direction(Enum):
    """Port/terminal signal direction (HOST-23): ``Direction(port.
    direction)`` turns the native ``str`` reflection field
    (``"in"``/``"out"``/``"inout"``) into a real enum for symbolic
    comparison (``d is Direction.In`` instead of comparing strings).

    ``Port.direction``/``Terminal.direction`` themselves stay plain ``str``
    (unchanged, HOST-23 SPEC_DEVIATION — see the ``Direction`` note near
    the class registrations below) since they are `#[pyclass]` fields
    reflected straight off the POM/ABI with no Python-level wrapper to
    intercept; this enum is the ergonomic typed handle layered on top.
    """

    In = "in"
    Out = "out"
    Inout = "inout"


class _ConfigMixin:
    """Shared ``.with_(**overrides)`` immutable-copy helper (HOST-20) for the
    config-bundle dataclasses below: ``cfg.with_(reltol=1e-6)`` returns a new
    instance with the named fields replaced, leaving ``cfg`` untouched —
    ``dataclasses.replace`` under the hood. Every field is discoverable via
    ``inspect.signature(TranConfig)`` (a plain dataclass ``__init__``), so no
    hand-written stub is needed to keep the two in sync.
    """

    def with_(self, **overrides: typing.Any) -> typing.Self:
        """Return an immutable copy with the named fields replaced
        (``dataclasses.replace``); the original instance is untouched."""
        return dataclasses.replace(self, **overrides)


@dataclass
class Solver(_ConfigMixin):
    """Solver tolerance + iteration config (prelude ``bundle Solver``).

    Field defaults mirror ``headers/prelude.phdl`` exactly; the solver's own
    defaults (``Context::default``/``Policy::default``) are the source of
    truth on the Rust side. ``dc_damp_tolerance`` (DC damping/homotopy
    threshold) is the same knob the Rust ``SolverConfig`` carries — HOST-20
    canonicalizes the two hosts on one name (``Solver``) and one field set.
    """

    temperature: float = 300.15
    reltol: float = 1e-3
    abstol: float = 1e-12
    gmin: float = 1e-12
    max_iter: int = 100
    dc_damp_tolerance: float = 0.5


@dataclass
class OpConfig(_ConfigMixin):
    """DC operating-point config (prelude ``bundle OpConfig``)."""

    solver: Solver = field(default_factory=Solver)
    nodeset: dict[str, float] = field(default_factory=dict)


@dataclass
class TranConfig(_ConfigMixin):
    """Transient analysis config (prelude ``bundle TranConfig``).

    ``step = 0.0`` selects the adaptive stepper (initial ``dt = stop/1000``).
    ``record_device_state = True`` records per-step device runtime banks,
    unlocking ``Trace.i`` on state-reading devices (``delay``/``transition``/
    ``idt``); the default keeps that read a loud error.
    """

    stop: float
    step: float = 0.0
    start: float = 0.0
    ic: dict[str, float] = field(default_factory=dict)
    solver: Solver = field(default_factory=Solver)
    record_device_state: bool = False


@dataclass
class AcConfig(_ConfigMixin):
    """AC small-signal sweep config (prelude ``bundle AcConfig``).

    ``scale`` selects the sweep geometry: ``Dec``/``Oct`` → logarithmic,
    ``Lin`` → linear.
    """

    fstart: float
    fstop: float
    points: int = 100
    scale: Scale = Scale.Dec
    solver: Solver = field(default_factory=Solver)


@dataclass
class NoiseConfig(_ConfigMixin):
    """Output-referred noise analysis config (prelude ``bundle NoiseConfig``)."""

    out: str
    fstart: float
    fstop: float
    points: int = 100
    scale: Scale = Scale.Dec
    solver: Solver = field(default_factory=Solver)


# ── reflected POM children (typed aliases for autocomplete) ───────────────────
#
# The native _piperine extension returns these as #[pyclass] objects with the
# listed attributes; the facade re-exports them so the IDE offers .name /
# .direction / .ty / etc. on every reflected child. These are the runtime
# types — at runtime, ``module.ports()[0]`` IS a ``_piperine._Port``; the
# alias makes the type name match the public vocabulary.
#
# SPEC_DEVIATION (HOST-23): `.direction` on `Port`/`Terminal` stays a plain
# `str` (`"in"`/`"out"`/`"inout"`) rather than becoming the `Direction` enum
# directly — these are native `#[pyclass]` fields set from Rust, with no
# Python-level wrapper class here to intercept the getter (unlike
# `Waveform.cross`, which is a plain attribute-assignable method on an
# already-Python-visible class). Wrap with `Direction(port.direction)` for
# the typed enum. Reason: rewriting `_Port`/`_Terminal` as native-backed
# Python wrapper classes to intercept every reflection getter is a much
# larger, separable change than HOST-23's scope; flagged for the Verifier.

Port = _piperine._Port
Net = _piperine._Net
Instance = _piperine._Instance
Param = _piperine._Param
Behavior = _piperine._Behavior
Selection = _piperine._Selection
Node = _piperine._Node
# Sub-views and statistics reachable from result objects: an
# ``InstanceView`` (per-terminal ``.v/.i``) comes from
# ``result["instance.path"]`` (spec AC13); its terminals are ``Terminal``
# objects; ``.stats`` on any analysis result is a ``SolverStats``.
InstanceView = _piperine._InstanceView
Terminal = _piperine._Terminal
ModelDescriptor = _piperine._ModelDescriptor
TerminalDescriptor = _piperine._TerminalDescriptor
ObservableDescriptor = _piperine._ObservableDescriptor
ParamDescriptor = _piperine._ParamDescriptor
SolverStats = _piperine._SolverStats
LimitingReport = _piperine._LimitingReport
NoiseContribution = _piperine._NoiseContribution

# Analysis-result types — no config-bundle translation needed, so they are
# plain re-exports of the native pyclasses. Their methods (.v/.i/.values/
# .axis/.mag/.phase/.db/.psd/.total) are the uniform-shape result readouts
# (PY-06–10 / spec AC4–10).
OpResult = _piperine._OpResult
Trace = _piperine._Trace
Waveform = _piperine._Waveform
ComplexWaveform = _piperine._ComplexWaveform
FourierComponent = _piperine._FourierComponent
FourierResult = _piperine._FourierResult
AcTrace = _piperine._AcTrace
NoiseTrace = _piperine._NoiseTrace


# ── Design + Module: config-bundle-aware wrappers ─────────────────────────────
#
# The native _Module.op/tran/ac/noise take positional args mirroring
# SimSession::run_*; the spec (AC6) calls for `module.tran(TranConfig(...))`.
# These thin wrappers accept a config-bundle dataclass, unpack it to the
# native positional signature, and forward. Reflection methods (ports/nets/
# instances/params/behaviors) delegate to the native; result objects come
# back unwrapped (they are the re-exported native types above).


class Design:
    """A loaded, elaborated POM design (spec AC1/2).

    Obtain one via :func:`load`. Reflect the top module (``design.top()``),
    look up a module by name (``design.module("Amp")``), enumerate modules
    (``design.modules()``), read constants (``design.const_("PI")``), or
    resolve a hierarchical selector path (``design.select("/r1/port::p")``).
    Read-only — the only mutation is :meth:`Module.set`.
    """

    def __init__(self, _native: _piperine._Design) -> None:
        self._native = _native

    def top(self) -> Module | None:
        """The elaborated top module, if one is set (spec AC2)."""
        m = self._native.top()
        return Module(m) if m is not None else None

    def module(self, name: str) -> Module:
        """Look up a module by name; raises :class:`UnknownModule` (a
        ``ValueError`` subclass, HOST-22) if absent."""
        try:
            return Module(self._native.module(name))
        except Exception as exc:
            raise UnknownModule(str(exc)) from exc

    def modules(self) -> list[Module]:
        """Every elaborated module."""
        return [Module(m) for m in self._native.modules()]

    def const_(self, name: str) -> typing.Any:
        """A global constant by name, or ``None`` if unknown."""
        return self._native.const_(name)

    def select(self, path: str) -> Selection:
        """Resolve a hierarchical selector path (Part IV selector).

        Path grammar: ``/``-separated steps, each ``name`` (default ``inst``
        axis) or ``axis::name`` (``net``/``port``/``param``/...). A leading
        ``/`` makes the path absolute (rooted at the inferred top module).
        Raises ``KeyError`` for zero matches, ``ValueError`` for a malformed
        path (fail loud).
        """
        return self._native.select(path)

    def compile(self, module: str | None = None) -> Session:
        """Compile a module **once** into a :class:`Session`.

        ``module = None`` compiles the design's top module (raises
        ``ValueError`` when no unambiguous top exists). The session holds the
        JIT-compiled circuit; ``set`` + re-run analyses never recompile.
        """
        if module is not None:
            return self.module(module).compile()
        top = self.top()
        if top is None:
            raise ValueError("design has no unambiguous top module; pass a module name")
        return top.compile()




class PssStats:
    """Shooting diagnostics for a PSS run.

    ``shoot_iterations`` — Newton iterations to the orbit;
    ``residual`` — final ``max|x(T) - x(0)|``;
    ``estimated_settle_time`` — how long a plain transient would need for
    its free response to decay below ``reltol`` (from the dominant
    monodromy eigenvalue), or ``None`` when no Jacobian was needed.
    """

    def __init__(self, shoot_iterations: int, residual: float, estimated_settle_time: float | None):
        self.shoot_iterations = shoot_iterations
        self.residual = residual
        self.estimated_settle_time = estimated_settle_time


class PssResult:
    """Periodic-steady-state result: one converged period + diagnostics.

    The uniform host shape (MD-22): ``.trace`` is a normal :class:`Trace`
    restricted to ``t in [tstab, tstab+period]``; ``.stats`` is
    :class:`PssStats`.
    """

    def __init__(self, trace: Trace, stats: PssStats):
        self.trace = trace
        self.stats = stats


class SensResult:
    """``.sens`` result: ``dV(output)/d(param)`` at the operating point.

    The uniform host shape (MD-22): a mapping keyed
    ``(output, "label.param")`` — identical to the Rust ``SensResult``.
    ``get(output, label, param)`` reads one entry; ``items()`` iterates.
    """

    def __init__(self, d: dict[tuple[str, str], float]):
        self._d = dict(d)

    def get(self, output: str, label: str, param: str) -> float | None:
        """The sensitivity of ``output`` w.r.t. ``label.param``, or None."""
        return self._d.get((output, f"{label}.{param}"))

    def items(self):
        """Iterate ``((output, "label.param"), value)`` pairs."""
        return self._d.items()


@dataclass
class PoleZeroResult:
    """``.pz`` result: poles and transmission zeros of the linearized
    input→output transfer function, in rad/s.

    The uniform host shape (MD-22): same field names as the Rust
    ``PoleZeroResult { poles, zeros }``. An empty ``zeros`` list is a
    legitimate answer (many networks have no finite transmission zero).
    """

    poles: list[complex]
    zeros: list[complex]


@dataclass
class SpResult:
    """``.sp`` result: the N-port scattering matrix over a frequency sweep.

    The uniform host shape (MD-22): same field names as the Rust
    ``SpResult { frequencies, s, z0, n_ports }``. ``s[k]`` is the
    ``n_ports x n_ports`` matrix at ``frequencies[k]``,
    ``s[k][i][j] == S_ij`` (port ``i`` response / port ``j`` excitation).
    """

    frequencies: list[float]
    s: list[list[list[complex]]]
    z0: list[float]
    n_ports: int


@dataclass
class DistoResult:
    """``.disto`` result: small-signal Volterra distortion ratios.

    The uniform host shape (MD-22): same field names as the Rust
    ``DistoResult { hd2, hd3, im2, im3 }``. Single-tone runs report
    ``hd2``/``hd3`` (``im2``/``im3`` are ``None``); two-tone runs report
    ``im2`` (at ``F1+F2``) and ``im3`` (at ``2·F1−F2``).
    """

    hd2: float | None
    hd3: float | None
    im2: float | None
    im3: float | None


@dataclass
class TfResult:
    """``.tf`` result (HOST-03): DC small-signal transfer characteristics
    from unit excitations on the system linearized at the operating point.

    The uniform host shape (MD-22): same field names as the Rust
    ``TfResult { gain, z_in, z_out }``. Binds the existing solver ``.tf``
    driver — no new solver math; voltage-source input only (documented
    limit, not a gap).
    """

    gain: float
    z_in: float
    z_out: float


class Module:
    """A reflected view of one POM module (spec AC14) + the four analyses.

    Reflection (``ports``/``nets``/``instances``/``params``/``behaviors``)
    is read-only. The four analyses (``op``/``tran``/``ac``/``noise``) build
    a fresh session per call over a forked design with staged overrides
    replayed (spec §9 isolation). Staging is pure — the parent ``Design`` is
    never mutated (spec AC11).
    """

    def __init__(self, _native: _piperine._Module) -> None:
        self._native = _native

    @property
    def name(self) -> str:
        """The module's declared name."""
        return self._native.name

    def ports(self) -> list[Port]:
        """The module's ports (name, direction, discipline type)."""
        return list(self._native.ports())

    def nets(self) -> list[Net]:
        """The module's ``wire`` declarations (name, discipline type)."""
        return list(self._native.nets())

    def instances(self) -> list[Instance]:
        """The module's submodule instances (label, module name)."""
        return list(self._native.instances())

    def params(self) -> list[Param]:
        """The module's params (name, type, default value)."""
        return list(self._native.params())

    def behaviors(self) -> list[Behavior]:
        """The module's ``analog``/``digital`` behavior blocks."""
        return list(self._native.behaviors())

    # ── analyses (spec AC3/6/8/9) ──────────────────────────────────────────

    @_wrap_analysis_errors
    def op(self, config: OpConfig | None = None) -> OpResult:
        """Run a DC operating-point analysis (spec AC3).

        ``config.nodeset`` seeds the Newton initial guess; ``config.solver``
        carries the tolerances + ``max_iter`` (prelude ``bundle Solver``).
        """
        if config is None:
            return self._native.op()
        nodeset = config.nodeset if config.nodeset else None
        return self._native.op(nodeset, config.solver)

    @_wrap_analysis_errors
    def sens(
        self,
        outputs: list[str],
        params: list[tuple[str, str]],
        dp_rel: float = 1.0e-6,
        solver: Solver | None = None,
    ) -> SensResult:
        """Run a DC sensitivity analysis (``.sens``).

        ``dV(output)/d(param)`` at the operating point for each
        ``(label, param)`` pair, by central finite difference over the
        compile-once restamp path. Unknown nets/elements/params and
        rebuild-class parameters fail loud.
        """
        return SensResult(self._native.sens(outputs, params, dp_rel, solver))

    @_wrap_analysis_errors
    def pss(
        self,
        period: float,
        tstab: float = 0.0,
        solver: Solver | None = None,
    ) -> PssResult:
        """Run a periodic-steady-state analysis (single shooting).

        One converged period ``t in [tstab, tstab+period]`` as a
        :class:`Trace` plus :class:`PssStats` (iterations, residual, and the
        estimated natural settling time). The drive period is user-supplied;
        non-periodic circuits and digital ``k*T`` dividers fail loud.
        """
        trace, iters, residual, settle = self._native.pss(period, tstab, solver)
        return PssResult(trace, PssStats(iters, residual, settle))

    @_wrap_analysis_errors
    def pz(
        self,
        input_source: str,
        output: str,
        output_ref: str | None = None,
        solver: Solver | None = None,
    ) -> PoleZeroResult:
        """Run a pole-zero analysis (``.pz``).

        Poles (and transmission zeros) of the linearized input→output
        transfer function at the DC operating point. ``input_source`` is the
        driving voltage source's instance label; ``output`` is the measured
        net name, optionally differential against ``output_ref``. A circuit
        with no reactive elements (no finite poles) fails loud, as does a
        device whose AC stamp is not affine in ``jω``.
        """
        poles, zeros = self._native.pz(input_source, output, output_ref, solver)
        return PoleZeroResult(poles=list(poles), zeros=list(zeros))

    @_wrap_analysis_errors
    def sp(
        self,
        fstart: float,
        fstop: float,
        points: int = 100,
        logarithmic: bool = True,
        solver: Solver | None = None,
    ) -> SpResult:
        """Run an N-port S-parameter analysis (``.sp``).

        The scattering matrix over a frequency sweep for every node
        carrying an ``@rfport(num, z0)`` attribute in this module. A module
        with no declared ports, a non-positive ``z0``, colliding ports
        (same ``num`` or the same node), or a port on an unaddressable node
        fails loud.
        """
        frequencies, s, z0, n_ports = self._native.sp(fstart, fstop, points, logarithmic, solver)
        return SpResult(frequencies=list(frequencies), s=s, z0=list(z0), n_ports=n_ports)

    @_wrap_analysis_errors
    def disto(
        self,
        f1: float,
        amplitude: float,
        output: str,
        f2: float | None = None,
        output_ref: str | None = None,
        solver: Solver | None = None,
    ) -> DistoResult:
        """Run a distortion analysis (``.disto``).

        Small-signal Volterra distortion at the DC operating point.
        Single-tone (``f2 = None``) reports ``hd2``/``hd3``; two-tone
        reports ``im2`` (at ``F1+F2``) and ``im3`` (at ``2·F1−F2``) with
        equal-amplitude tones. ``amplitude`` scales every AC stimulus
        magnitude in the circuit. Non-positive ``f1``/``amplitude``,
        ``f2 == f1``, an unaddressable output, no first-order response, or
        a current-controlled nonlinearity fails loud.
        """
        hd2, hd3, im2, im3 = self._native.disto(f1, amplitude, output, f2, output_ref, solver)
        return DistoResult(hd2=hd2, hd3=hd3, im2=im2, im3=im3)

    @_wrap_analysis_errors
    def tran(self, config: TranConfig) -> Trace:
        """Run a transient analysis (spec AC6).

        ``config.step = 0.0`` (the prelude default) selects the adaptive
        stepper; a positive ``step`` seeds the initial ``dt``. ``config.ic``
        presets node voltages; ``config.solver`` carries the tolerances +
        ``max_iter``.
        """
        step = config.step if config.step != 0.0 else None
        ic = config.ic if config.ic else None
        return self._native.tran(
            config.stop, step, config.start, ic, config.solver, config.record_device_state
        )

    @_wrap_analysis_errors
    def ac(self, config: AcConfig) -> AcTrace:
        """Run an AC small-signal sweep (spec AC8).

        ``config.scale`` maps to logarithmic (``Dec``/``Oct``) or linear
        (``Lin``); ``config.solver`` carries the tolerances.
        """
        logarithmic = config.scale in (Scale.Dec, Scale.Oct)
        return self._native.ac(
            config.fstart, config.fstop, config.points, logarithmic, config.solver
        )

    @_wrap_analysis_errors
    def noise(self, config: NoiseConfig) -> NoiseTrace:
        """Run an output-referred noise analysis (spec AC9)."""
        logarithmic = config.scale in (Scale.Dec, Scale.Oct)
        return self._native.noise(
            config.out,
            config.fstart,
            config.fstop,
            config.points,
            "gnd",
            logarithmic,
            config.solver,
        )

    # ── staging (spec AC11/12) ─────────────────────────────────────────────

    @_wrap_analysis_errors
    def set(self, label: str, param: str, value: float) -> None:
        """Set a parameter override for the next analysis (spec AC11/12).

        The next analysis on this module uses ``value`` for the instance
        ``label``'s ``param``. Setting is pure — the held ``Design`` is not
        mutated; overrides replay onto each analysis's fork. Sweeps are
        native Python ``for`` loops. Same verb as :meth:`Session.set`:
        both mean "subsequent analyses see the new value".
        """
        self._native.set(label, param, value)

    def compile(self) -> Session:
        """Compile this module **once** into a :class:`Session`.

        Currently staged overrides are baked into the compilation; the
        parent :class:`Design` stays untouched.
        """
        return Session(self._native.compile())


class Session:
    """A compiled circuit held live across analyses (compile once, set,
    re-run — the optimization-loop primitive).

    Obtain one via :meth:`Design.compile` / :meth:`Module.compile`.
    Elaboration + JIT happen exactly once; :meth:`set` writes parameters
    directly on the compiled circuit through the solver's restamp path (no
    re-elaboration, no re-JIT), and the analyses re-run on the same
    compiled circuit. Addressing is the PHDL scheme: flat instance labels,
    bundle fields flattened to ``{param}_{field}`` (e.g. ``model_is``).

    Result objects are identical to :class:`Module`'s analyses (same
    types, same readouts).
    """

    def __init__(self, _native: _piperine._Session) -> None:
        self._native = _native

    @property
    def rebuilds(self) -> int:
        """How many automatic structural rebuilds this session performed
        (``0`` until a structural set lands)."""
        return self._native.rebuilds

    @_wrap_analysis_errors
    def set(self, label: str, param: str, value: float) -> None:
        """Write a parameter on the compiled circuit, effective from the
        next analysis run.

        Raises ``KeyError`` for an unknown instance label or parameter
        (the message lists the element's parameters), ``ValueError`` for a
        value outside the parameter's declared bounds — no partial apply.
        """
        self._native.set(label, param, value)

    @_wrap_analysis_errors
    def schedule_set(self, t: float, label: str, param: str, value: float) -> None:
        """Schedule ``set`` at simulation time ``t`` for the next
        :meth:`tran` run.

        The integrator lands exactly on ``t`` (forced breakpoint) and the
        write applies there; several sets on the same parameter apply in
        scheduling order (last write wins). Unknown names fail loud when
        the set lands, same as :meth:`set`.
        """
        self._native.schedule_set(t, label, param, value)

    # ── analyses on the held circuit (same shapes as Module's) ─────────────

    @_wrap_analysis_errors
    def op(self, config: OpConfig | None = None) -> OpResult:
        """Run a DC operating point on the held circuit (spec AC3 shape)."""
        if config is None:
            return self._native.op()
        nodeset = config.nodeset if config.nodeset else None
        return self._native.op(nodeset, config.solver)

    @_wrap_analysis_errors
    def tran(self, config: TranConfig) -> Trace:
        """Run a transient on the held circuit (spec AC6 shape), honoring
        any pending :meth:`schedule_set` entries."""
        step = config.step if config.step != 0.0 else None
        ic = config.ic if config.ic else None
        return self._native.tran(
            config.stop, step, config.start, ic, config.solver, config.record_device_state
        )

    @_wrap_analysis_errors
    def ac(self, config: AcConfig) -> AcTrace:
        """Run an AC small-signal sweep on the held circuit (spec AC8
        shape)."""
        logarithmic = config.scale in (Scale.Dec, Scale.Oct)
        return self._native.ac(
            config.fstart, config.fstop, config.points, logarithmic, config.solver
        )

    @_wrap_analysis_errors
    def noise(self, config: NoiseConfig) -> NoiseTrace:
        """Run an output-referred noise analysis on the held circuit (spec
        AC9 shape)."""
        logarithmic = config.scale in (Scale.Dec, Scale.Oct)
        return self._native.noise(
            config.out,
            config.fstart,
            config.fstop,
            config.points,
            "gnd",
            logarithmic,
            config.solver,
        )

    @_wrap_analysis_errors
    def sens(
        self,
        outputs: list[str],
        params: list[tuple[str, str]],
        dp_rel: float = 1.0e-6,
        solver: Solver | None = None,
    ) -> SensResult:
        """Run a DC sensitivity analysis (``.sens``) on the held circuit
        (HOST-02), same shape as :meth:`Module.sens`."""
        return SensResult(self._native.sens(outputs, params, dp_rel, solver))

    @_wrap_analysis_errors
    def pss(
        self,
        period: float,
        tstab: float = 0.0,
        solver: Solver | None = None,
    ) -> PssResult:
        """Run a periodic-steady-state analysis on the held circuit
        (HOST-02), same shape as :meth:`Module.pss`."""
        trace, iters, residual, settle = self._native.pss(period, tstab, solver)
        return PssResult(trace, PssStats(iters, residual, settle))

    @_wrap_analysis_errors
    def pz(
        self,
        input_source: str,
        output: str,
        output_ref: str | None = None,
        solver: Solver | None = None,
    ) -> PoleZeroResult:
        """Run a pole-zero analysis (``.pz``) on the held circuit
        (HOST-02), same shape as :meth:`Module.pz`."""
        poles, zeros = self._native.pz(input_source, output, output_ref, solver)
        return PoleZeroResult(poles=list(poles), zeros=list(zeros))

    @_wrap_analysis_errors
    def disto(
        self,
        f1: float,
        amplitude: float,
        output: str,
        f2: float | None = None,
        output_ref: str | None = None,
        solver: Solver | None = None,
    ) -> DistoResult:
        """Run a distortion analysis (``.disto``) on the held circuit
        (HOST-02), same shape as :meth:`Module.disto`."""
        hd2, hd3, im2, im3 = self._native.disto(f1, amplitude, output, f2, output_ref, solver)
        return DistoResult(hd2=hd2, hd3=hd3, im2=im2, im3=im3)

    @_wrap_analysis_errors
    def sp(
        self,
        fstart: float,
        fstop: float,
        points: int = 100,
        logarithmic: bool = True,
        solver: Solver | None = None,
    ) -> SpResult:
        """Run an N-port S-parameter analysis (``.sp``) on the held circuit
        (HOST-02), same shape as :meth:`Module.sp`."""
        frequencies, s, z0, n_ports = self._native.sp(fstart, fstop, points, logarithmic, solver)
        return SpResult(frequencies=list(frequencies), s=s, z0=list(z0), n_ports=n_ports)

    @_wrap_analysis_errors
    def tf(
        self,
        output: str,
        input_source: str,
        output_ref: str | None = None,
        solver: Solver | None = None,
    ) -> TfResult:
        """Run a transfer-function analysis (``.tf``, HOST-03) on the held
        circuit: DC small-signal gain, input resistance, and output
        resistance from unit excitations on the system linearized at the
        operating point. Binds the existing solver ``.tf`` driver — no new
        solver math (voltage-source input only, MD-14).
        """
        native = self._native.tf(output, input_source, output_ref, solver)
        return TfResult(gain=native.gain, z_in=native.z_in, z_out=native.z_out)

    @_wrap_analysis_errors
    def dc(
        self,
        label: str,
        param: str,
        values: list[float],
        nodeset: dict[str, float] | None = None,
        solver: Solver | None = None,
    ) -> Trace:
        """Run a compile-once DC sweep (``.dc``, HOST-05): restamp
        ``label``'s ``param`` for each of ``values`` on the one compilation
        (MD-18), returning a swept :class:`Trace` over the axis — read the
        same way as :meth:`tran`/:meth:`pss` (``.v``/``.i``/``.axis``).

        ``nodeset`` seeds the Newton initial guess at every swept point
        (same knob :meth:`op`/:meth:`tran` accept — HOST-20 nodeset parity
        with the Rust ``Session::dc``).
        """
        return self._native.dc(label, param, list(values), nodeset or None, solver)

    def sweep(self, label: str, param: str, values: list[float]) -> "Sweep":
        """A fluent single-knob sweep over ``label.param`` (HOST-18):
        ``for point in session.sweep("r1", "r", [1e3, 2e3, 3e3]): point.op()``.

        Each ``point`` is a :class:`SweepPoint` — a ``Session`` view at that
        knob value (every :class:`Session` method is available directly on
        it via attribute delegation). Reuses :meth:`set`'s compile-once
        restamp (MD-18); a structural knob auto-rebuilds and counts it in
        :attr:`rebuilds` (LIVE-14), same as a bare :meth:`set` — the sweep
        adds no separate rebuild path. Each iteration builds a fresh native
        ``_Session.sweep`` iterator (so a :class:`Sweep` can be iterated
        more than once); this wrapper turns each native ``(value, index)``
        step into a :class:`SweepPoint` view of *this* ``Session``.
        """
        return Sweep(self, label, param, list(values))

    def sweep_grid(self, axes: dict[str, list[float]]) -> "Grid":
        """A named multi-axis sweep grid (HOST-19):
        ``session.sweep_grid({"r1.r": [1e3, 2e3], "c1.c": [1e-9, 2e-9]})``
        iterates every combination in row-major (outer-axis-first) order;
        :meth:`Grid.map` collects results into an axis-shaped
        ``numpy.ndarray``.

        Each key is a ``"label.param"`` path (the same addressing
        :meth:`sweep`/:meth:`set`/``probe=`` use) — not a bare kwarg name —
        since PHDL parameters are addressed by flat instance label, and a
        dotted path is not a valid Python identifier.
        """
        parsed: list[tuple[str, str, list[float]]] = []
        for path, values in axes.items():
            label, param = path.split(".", 1)
            parsed.append((label, param, list(values)))
        return Grid(self, parsed)


class SweepPoint:
    """A :class:`Session` view at one sweep coordinate (HOST-18/19).

    Every :class:`Session` method/property (``op``/``tran``/``ac``/…) is
    reachable directly via attribute delegation to the underlying session —
    ``point.op()`` runs on the session restamped (or rebuilt) to this
    point's value. ``.value``/``.index`` name the sweep coordinate: a
    single ``float``/``int`` for a :class:`Sweep`, a ``tuple`` (one entry
    per axis) for a :class:`Grid`.
    """

    def __init__(self, session: Session, value, index) -> None:
        self._session = session
        self.value = value
        self.index = index

    def __getattr__(self, name: str):
        return getattr(self._session, name)


class Sweep:
    """A fluent single-knob sweep (HOST-18, :meth:`Session.sweep`):
    iterating yields one :class:`SweepPoint` per value, in order, restamped
    (or rebuilt, for a structural knob) onto the session's one compilation.

    Each ``for point in sweep`` builds a fresh native ``_Session.sweep``
    iterator (which does the actual restamping) — a :class:`Sweep` is a
    reusable recipe, not a single-use iterator, so it can be iterated more
    than once.
    """

    def __init__(self, session: Session, label: str, param: str, values: list[float]) -> None:
        self._session = session
        self._label = label
        self._param = param
        self._values = values

    def __len__(self) -> int:
        return len(self._values)

    def __iter__(self):
        native_sweep = self._session._native.sweep(self._label, self._param, self._values)
        for value, index in native_sweep:
            yield SweepPoint(self._session, value, index)


class Grid:
    """A named multi-axis sweep grid (HOST-19, :meth:`Session.sweep_grid`):
    iterating yields one :class:`SweepPoint` per row-major combination;
    :meth:`map` collects results into an axis-shaped ``numpy.ndarray``.

    Each ``for point in grid`` builds a fresh native ``_Session.sweep_grid``
    iterator (which does the actual restamping) — a :class:`Grid` is a
    reusable recipe, not a single-use iterator (:meth:`map` iterates it
    internally, but a `Grid` can still be iterated again afterward).
    """

    def __init__(self, session: Session, axes: list[tuple[str, str, list[float]]]) -> None:
        self._session = session
        self._axes = axes

    @property
    def shape(self) -> tuple[int, ...]:
        """The grid's shape — one length per axis, outer axis first."""
        return tuple(len(values) for _, _, values in self._axes)

    def __len__(self) -> int:
        n = 1
        for size in self.shape:
            n *= size
        return n

    def __iter__(self):
        native_grid = self._session._native.sweep_grid(self._axes)
        for coord, index in native_grid:
            yield SweepPoint(self._session, tuple(coord), tuple(index))

    def map(self, fn) -> "np.ndarray":  # noqa: F821
        """Apply ``fn(point)`` at every grid combination (row-major) and
        return the results as a ``numpy.ndarray`` shaped like :attr:`shape`
        (HOST-19) — ``result[i, j, ...] == fn(point at axis values
        (axes[0][i], axes[1][j], ...))``.
        """
        import numpy as np

        values = [fn(point) for point in self]
        return np.array(values).reshape(self.shape)


# ── load ──────────────────────────────────────────────────────────────────────


def load(path: str) -> Design:
    """Load + elaborate a ``.phdl``/``.ppr`` file into a :class:`Design`
    (spec AC1).

    Raises :class:`ElaborationError` (a ``ValueError`` subclass, HOST-22)
    with the diagnostic on a parse/elaboration failure or an unreadable
    file — never a silent success.
    """
    try:
        return Design(_piperine.load(path))
    except Exception as exc:
        raise ElaborationError(str(exc)) from exc


# ── SI unit helpers (HOST-21) ────────────────────────────────────────────────
#
# Mirror the Rust `Freq`/`Time` newtypes' string parsing (`units.rs`): a
# string value takes an optional SI prefix (f/p/n/u(µ)/m/k/M/G/T) and an
# optional trailing unit-name suffix; a non-string value is taken as already
# being in the function's base unit — SI prefixes never apply to a raw
# `float`/`int` (a bare number is not re-parsed as a string).

_SI_PREFIXES = {
    "f": 1e-15,
    "p": 1e-12,
    "n": 1e-9,
    "u": 1e-6,
    "µ": 1e-6,
    "m": 1e-3,
    "k": 1e3,
    "M": 1e6,
    "G": 1e9,
    "T": 1e12,
}


def _parse_si(value: str, unit_suffix: str) -> float:
    """Parse `value` as `<number><optional SI prefix>`, with an optional
    trailing `unit_suffix` stripped first if present (e.g. `_parse_si
    ("10MHz", "Hz")` and `_parse_si("10M", "Hz")` both yield `1e7`). Raises
    ``ValueError`` on anything else (fail loud, no silent default)."""
    trimmed = value.strip()
    body = trimmed[: -len(unit_suffix)] if trimmed.endswith(unit_suffix) else trimmed
    if not body:
        raise ValueError(f"`{trimmed}` has no numeric part")
    mult = 1.0
    if body[-1] in _SI_PREFIXES:
        mult = _SI_PREFIXES[body[-1]]
        body = body[:-1]
    try:
        return float(body) * mult
    except ValueError as e:
        raise ValueError(
            f"cannot parse `{trimmed}` as a number (expected `<number>` optionally followed by "
            f"an SI prefix (k/M/G/m/u/n/p/f) and/or the `{unit_suffix}` suffix)"
        ) from e


def Hz(value: float | str) -> float:
    """A frequency in Hz (HOST-21): ``Hz(1e6) == 1e6``,
    ``Hz("10M") == 1e7``, ``Hz("10MHz") == 1e7``. SI prefixes only apply
    when ``value`` is a ``str`` — a raw ``float``/``int`` is returned as-is,
    never re-parsed as a string.
    """
    if isinstance(value, str):
        return _parse_si(value, "Hz")
    return float(value)


def ns(value: float | str) -> float:
    """A duration in nanoseconds, converted to seconds (HOST-21):
    ``ns(10) == 10e-9``, ``ns("10n") == 10e-9``, ``ns("10ns") == 10e-9``.
    """
    if isinstance(value, str):
        return _parse_si(value, "s")
    return float(value) * 1e-9


def mV(value: float | str) -> float:
    """A voltage in millivolts, converted to volts (HOST-21):
    ``mV(300) == 0.3``, ``mV("300m") == 0.3``.
    """
    if isinstance(value, str):
        return _parse_si(value, "V")
    return float(value) * 1e-3


def C(value: float) -> float:
    """A temperature in degrees Celsius, converted to Kelvin (HOST-21) — the
    unit ``Solver.temperature`` expects: ``C(27) == 300.15``.
    """
    return float(value) + 273.15


# ── plotting (HOST-17, matplotlib-guarded) ─────────────────────────────────
#
# matplotlib is never a hard dependency (spec Out-of-Scope / AC4): every
# entry point below imports it lazily and raises a clear ``ImportError``
# when it's absent, rather than failing at `import piperine` time or
# silently no-op'ing. Figures are returned, not shown — a library call
# forcing a blocking ``plt.show()`` would hang a headless/test process; the
# caller decides whether/how to display or save the figure (``fig.show()``,
# ``fig.savefig(...)``, or nothing at all in a notebook, which renders the
# returned `Figure` inline).


def _require_matplotlib():
    try:
        import matplotlib.pyplot as plt
    except ImportError as e:
        raise ImportError(
            "piperine.plot/bode/Waveform.plot requires matplotlib — "
            "install it with `pip install matplotlib`"
        ) from e
    return plt


def plot(waveform: Waveform | dict[str, Waveform], **kwargs) -> "matplotlib.figure.Figure":  # noqa: F821
    """Plot one real :class:`Waveform`, or several keyed by label
    (``{"vout": wf1, "vin": wf2}``), on one axis (HOST-17). ``xlabel``/
    ``ylabel``/``title`` kwargs label the axes; unrecognized kwargs are
    ignored. Requires matplotlib — raises ``ImportError`` with an install
    hint when it's not installed (no hard dependency, no silent no-op).
    """
    plt = _require_matplotlib()
    fig, ax = plt.subplots()
    if isinstance(waveform, dict):
        for label, wf in waveform.items():
            ax.plot(wf.axis, wf.values, label=label)
        ax.legend()
    else:
        ax.plot(waveform.axis, waveform.values)
    ax.set_xlabel(kwargs.get("xlabel", "axis"))
    ax.set_ylabel(kwargs.get("ylabel", "value"))
    if "title" in kwargs:
        ax.set_title(kwargs["title"])
    ax.grid(True)
    return fig


def bode(cw: ComplexWaveform, **kwargs) -> "matplotlib.figure.Figure":  # noqa: F821
    """Bode plot (magnitude in dB + phase in degrees, log-frequency x-axis)
    of a :class:`ComplexWaveform` (HOST-17). ``title`` kwarg labels the
    figure. Requires matplotlib — raises ``ImportError`` with an install
    hint when it's not installed.
    """
    import numpy as np

    plt = _require_matplotlib()
    fig, (ax_mag, ax_phase) = plt.subplots(2, 1, sharex=True)
    freq = cw.axis
    ax_mag.semilogx(freq, cw.db.values)
    ax_mag.set_ylabel("Magnitude (dB)")
    ax_mag.grid(True, which="both")
    ax_phase.semilogx(freq, np.degrees(cw.phase.values))
    ax_phase.set_ylabel("Phase (deg)")
    ax_phase.set_xlabel("Frequency (Hz)")
    ax_phase.grid(True, which="both")
    if "title" in kwargs:
        fig.suptitle(kwargs["title"])
    return fig


def _waveform_plot_method(self, **kwargs):
    """``wf.plot()`` (HOST-17) — same as :func:`plot` bound onto
    :class:`Waveform`."""
    return plot(self, **kwargs)


def _complex_waveform_plot_method(self, **kwargs):
    """``cw.plot()`` (HOST-17) — a `ComplexWaveform`'s natural render is a
    Bode plot; same as :func:`bode` bound onto :class:`ComplexWaveform`."""
    return bode(self, **kwargs)


# Save the native `.cross()` (still string-keyed) before overwriting it
# below (HOST-23) — the same class-attribute-assignment technique `.plot()`
# uses, so `Waveform.cross` accepts the typed `CrossDirection` enum while
# still accepting a bare string for backward compatibility with any
# existing caller.
#
# `Waveform` is the native `_piperine._Waveform` *class object itself*
# (shared/cached across every embedded-interpreter re-materialization of
# this facade module — `piperine run`/`run_script` re-executes this file's
# top level on every call in the same process, e.g. once per example in
# `run_examples.rs`). Capturing `Waveform.cross` unconditionally on every
# re-execution would, on the *second* execution, capture the *already-
# wrapped* `_waveform_cross_enum` from the first — infinite recursion the
# first time a wrapped `.cross()` calls what it thinks is "the native
# method". The `hasattr` guard makes the capture idempotent: only the
# first execution in a process captures the true native method.
if not hasattr(Waveform, "_host_cross_native"):
    Waveform._host_cross_native = Waveform.cross


def _waveform_cross_enum(self, level: float, dir: CrossDirection | str = CrossDirection.Either) -> float | None:
    """``wf.cross(level, dir)`` (HOST-23): `dir` accepts a
    :class:`CrossDirection` enum member (or the legacy string spelling) —
    the first axis value where the waveform crosses `level` in that
    direction, or ``None``.
    """
    value = dir.value if isinstance(dir, CrossDirection) else dir
    return Waveform._host_cross_native(self, level, value)


# Bind `.plot()`/`.cross()` onto the native pyclasses themselves (not just
# the facade aliases above) — the native `_piperine._Waveform`/
# `_ComplexWaveform` types PyO3 generates are ordinary heap types, so a
# plain class-attribute assignment works exactly like it would on a
# pure-Python class; this is the only way to add `.plot()` without
# threading a matplotlib dependency into the Rust `piperine-python` crate
# itself (kept out per the spec's "matplotlib as a hard dependency"
# Out-of-Scope entry), and the same technique layers the typed
# `CrossDirection` enum over `.cross()` without a native signature change.
Waveform.cross = _waveform_cross_enum
Waveform.plot = _waveform_plot_method
ComplexWaveform.plot = _complex_waveform_plot_method
