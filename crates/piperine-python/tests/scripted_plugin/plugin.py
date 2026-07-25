"""The scripted-plugin fixture (plugin-interface v2, PLG-06/10/11): every
contribution is a decorator — one CLI script, all five frozen lifecycle
hooks, and a device-binding marker — never an imperative register call.
Each hook writes a marker file (through the capability-gated ctx) naming
its phase and proving its payload arrived; the Rust test fires the phases
and reads the markers back.
"""
import piperine as pip


@pip.script("lint")
def lint(args, ctx):
    """Record the dispatched args and exit 0 (the dispatch proof)."""
    ctx.fs_write("lint_args.txt", " ".join(args))
    return 0


@pip.script("design_probe")
def design_probe(args, ctx):
    """Touch `ctx.design()` from a script — must raise (fail loud): the
    design is only available in the elaboration hooks."""
    ctx.design()
    return 0


@pip.hook.after_parse
def after_parse(ctx, source):
    ctx.fs_write("phase_after_parse.txt", "source:%d" % len(source))


@pip.hook.after_elaborate
def after_elaborate(ctx):
    ctx.fs_write("phase_after_elaborate.txt", "design:%s" % (ctx.design() is not None))


@pip.hook.transform_design
def transform_design(ctx, staging):
    ctx.fs_write("phase_transform_design.txt", "staging:%s" % (staging.design() is not None))


@pip.hook.before_lower
def before_lower(ctx):
    ctx.fs_write("phase_before_lower.txt", "design:%s" % (ctx.design() is not None))


@pip.hook.after_solve
def after_solve(ctx, result):
    ctx.fs_write("phase_after_solve.txt", "analysis:%s" % result.analysis)


@pip.device("Glue")
class Glue:
    """The `@device` binding marker for a Python-glue plugin."""
