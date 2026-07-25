"""Testbench for the `@pip` decorator surface (plugin-interface v2,
PLG-06/10/11): declaration and registration are one decorator. Run by
`piperine test` (or the scripted_plugin Rust test via the embedded host).
"""
import piperine as pip


@pip.script("lint")
def lint(args, ctx):
    return 0


@pip.hook.after_elaborate
def check(ctx):
    pass


@pip.device("Glue")
class Glue:
    pass


# Declaration lands in the per-load registration table the host reads back.
table = pip._take_registry()
assert table["scripts"] == ["lint"], table
assert table["hooks"] == ["after_elaborate"], table
assert table["devices"] == ["Glue"], table

# The hook catalog is frozen at five — any other phase name raises.
try:
    pip.hook.not_a_phase
    raise AssertionError("unknown hook phase accepted")
except AttributeError:
    pass

assert pip.HOOK_PHASES == (
    "after_parse",
    "after_elaborate",
    "transform_design",
    "before_lower",
    "after_solve",
), pip.HOOK_PHASES


# Re-declaring an occupied script name is a declaration-time conflict.
@pip.script("dup")
def dup_a(args, ctx):
    return 0


try:

    @pip.script("dup")
    def dup_b(args, ctx):
        return 0

    raise AssertionError("duplicate script name accepted")
except ValueError:
    pass

print("lint_tb OK")
