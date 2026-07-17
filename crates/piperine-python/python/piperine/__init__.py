"""Piperine — Python bindings for analog/mixed-signal circuit simulation.

The typed public surface of the Piperine simulator (spec §10 — the uniform
host-neutral API). This pure-Python facade wraps the native ``_piperine``
extension (PyO3) so IDEs see full annotations + docstrings; runtime forwards
to the native engine with negligible cost.

Uniform shape (PY-15, binding): the call graph mirrors the bench layer — ::

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
are native Python ``for`` loops driving ``module.stage(label, param, value)``
(spec AC11/12).

Numpy arrays: ``Waveform.values`` / ``.axis`` are real ``np.ndarray``;
``ComplexWaveform.values`` is complex128 (spec AC7/8).
"""

from __future__ import annotations

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
    # live session (compile once, set, re-run)
    "LiveSession",
    # analyses
    "OpResult",
    "Trace",
    "Waveform",
    "ComplexWaveform",
    "AcTrace",
    "NoiseTrace",
    # config bundles (mirror headers/prelude.phdl)
    "Scale",
    "Solver",
    "OpConfig",
    "TranConfig",
    "AcConfig",
    "NoiseConfig",
]


# ── config bundles (mirror crates/piperine-lang/headers/prelude.phdl) ─────────


class Scale(Enum):
    """Frequency-sweep scale (prelude ``enum Scale``)."""

    Lin = "Lin"
    Dec = "Dec"
    Oct = "Oct"


@dataclass
class Solver:
    """Solver tolerance + iteration config (prelude ``bundle Solver``).

    Field defaults mirror ``headers/prelude.phdl`` exactly; the solver's own
    defaults (``Context::default``) are the source of truth on the Rust side.
    """

    temperature: float = 300.15
    reltol: float = 1e-3
    abstol: float = 1e-12
    gmin: float = 1e-12
    max_iter: int = 100


@dataclass
class OpConfig:
    """``$op`` config (prelude ``bundle OpConfig``)."""

    solver: Solver = field(default_factory=Solver)
    nodeset: dict[str, float] = field(default_factory=dict)


@dataclass
class TranConfig:
    """``$tran`` config (prelude ``bundle TranConfig``).

    ``step = 0.0`` selects the adaptive stepper (initial ``dt = stop/1000``).
    """

    stop: float
    step: float = 0.0
    start: float = 0.0
    ic: dict[str, float] = field(default_factory=dict)
    solver: Solver = field(default_factory=Solver)


@dataclass
class AcConfig:
    """``$ac`` config (prelude ``bundle AcConfig``).

    ``scale`` selects the sweep geometry: ``Dec``/``Oct`` → logarithmic,
    ``Lin`` → linear (matches the bench's ``logarithmic`` bool).
    """

    fstart: float
    fstop: float
    points: int = 100
    scale: Scale = Scale.Dec
    solver: Solver = field(default_factory=Solver)


@dataclass
class NoiseConfig:
    """``$noise`` config (prelude ``bundle NoiseConfig``)."""

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

Port = _piperine._Port
Net = _piperine._Net
Instance = _piperine._Instance
Param = _piperine._Param
Behavior = _piperine._Behavior
Selection = _piperine._Selection
Node = _piperine._Node

# Analysis-result types — no config-bundle translation needed, so they are
# plain re-exports of the native pyclasses. Their methods (.v/.i/.values/
# .axis/.mag/.phase/.db/.psd/.total) are the uniform-shape result readouts
# (PY-06–10 / spec AC4–10).
OpResult = _piperine._OpResult
Trace = _piperine._Trace
Waveform = _piperine._Waveform
ComplexWaveform = _piperine._ComplexWaveform
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
    Read-only — the only mutation is :meth:`Module.stage`.
    """

    def __init__(self, _native: _piperine._Design) -> None:
        self._native = _native

    def top(self) -> Module | None:
        """The elaborated top module, if one is set (spec AC2)."""
        m = self._native.top()
        return Module(m) if m is not None else None

    def module(self, name: str) -> Module:
        """Look up a module by name; raises ``ValueError`` if absent."""
        return Module(self._native.module(name))

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

    def compile(self, module: str | None = None) -> LiveSession:
        """Compile a module **once** into a :class:`LiveSession`.

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

    def op(self, config: OpConfig | None = None) -> OpResult:
        """Run a DC operating-point analysis (spec AC3).

        ``config.nodeset`` seeds the Newton initial guess; ``config.solver``
        carries the tolerances + ``max_iter`` (prelude ``bundle Solver``).
        """
        if config is None:
            return self._native.op()
        nodeset = config.nodeset if config.nodeset else None
        return self._native.op(nodeset, config.solver)

    def tran(self, config: TranConfig) -> Trace:
        """Run a transient analysis (spec AC6).

        ``config.step = 0.0`` (the prelude default) selects the adaptive
        stepper; a positive ``step`` seeds the initial ``dt``. ``config.ic``
        presets node voltages (spec §5.1 ``TranConfig.ic``); ``config.solver``
        carries the tolerances + ``max_iter``.
        """
        step = config.step if config.step != 0.0 else None
        ic = config.ic if config.ic else None
        return self._native.tran(config.stop, step, config.start, ic, config.solver)

    def ac(self, config: AcConfig) -> AcTrace:
        """Run an AC small-signal sweep (spec AC8).

        ``config.scale`` maps to logarithmic (``Dec``/``Oct``) or linear
        (``Lin``); ``config.solver`` carries the tolerances.
        """
        logarithmic = config.scale in (Scale.Dec, Scale.Oct)
        return self._native.ac(
            config.fstart, config.fstop, config.points, logarithmic, config.solver
        )

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

    def stage(self, label: str, param: str, value: float) -> None:
        """Stage a parameter override (spec AC11/12).

        The next analysis on this module uses ``value`` for the instance
        ``label``'s ``param``. Staging is pure — the held ``Design`` is not
        mutated; overrides replay onto each analysis's fork. Sweeps are
        native Python ``for`` loops.
        """
        self._native.stage(label, param, value)

    def compile(self) -> LiveSession:
        """Compile this module **once** into a :class:`LiveSession`.

        Currently staged overrides are baked into the compilation; the
        parent :class:`Design` stays untouched.
        """
        return LiveSession(self._native.compile())


class LiveSession:
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

    def __init__(self, _native: _piperine._LiveSession) -> None:
        self._native = _native

    @property
    def rebuilds(self) -> int:
        """How many automatic structural rebuilds this session performed
        (``0`` until a structural set lands)."""
        return self._native.rebuilds

    def set(self, label: str, param: str, value: float) -> None:
        """Write a parameter on the compiled circuit, effective from the
        next analysis run.

        Raises ``KeyError`` for an unknown instance label or parameter
        (the message lists the element's parameters), ``ValueError`` for a
        value outside the parameter's declared bounds — no partial apply.
        """
        self._native.set(label, param, value)

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

    def op(self, config: OpConfig | None = None) -> OpResult:
        """Run a DC operating point on the held circuit (spec AC3 shape)."""
        if config is None:
            return self._native.op()
        nodeset = config.nodeset if config.nodeset else None
        return self._native.op(nodeset, config.solver)

    def tran(self, config: TranConfig) -> Trace:
        """Run a transient on the held circuit (spec AC6 shape), honoring
        any pending :meth:`schedule_set` entries."""
        step = config.step if config.step != 0.0 else None
        ic = config.ic if config.ic else None
        return self._native.tran(config.stop, step, config.start, ic, config.solver)

    def ac(self, config: AcConfig) -> AcTrace:
        """Run an AC small-signal sweep on the held circuit (spec AC8
        shape)."""
        logarithmic = config.scale in (Scale.Dec, Scale.Oct)
        return self._native.ac(
            config.fstart, config.fstop, config.points, logarithmic, config.solver
        )

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


# ── load ──────────────────────────────────────────────────────────────────────


def load(path: str) -> Design:
    """Load + elaborate a ``.phdl``/``.ppr`` file into a :class:`Design`
    (spec AC1).

    Raises ``ValueError`` (with the diagnostic) on a parse/elaboration
    failure or an unreadable file — never a silent success.
    """
    return Design(_piperine.load(path))
