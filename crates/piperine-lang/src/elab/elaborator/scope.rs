//! What names the code being lowered: [`Scope`], one part per concern.

use crate::elab::analog::state::AnalogScope;
use crate::elab::digital::state::DigitalScope;
use crate::elab::value::state::ValueScope;
use crate::pom::FileId;

/// What names the code being lowered: the owner's params and locals (its nodes are those of its
/// frame, `Frame::nodes`), one part per concern.
pub(in crate::elab) struct Scope {
    /// The file, params, locals, frame and control flow of the code: what value lowering reads and writes.
    pub(in crate::elab) values: ValueScope,
    /// The branch and site context of the code: what analog lowering reads and writes.
    pub(in crate::elab) analog: AnalogScope,
    /// The carrier context of the code: what discrete lowering reads and writes.
    pub(in crate::elab) digital: DigitalScope,
}

impl Scope {
    /// Takes over what a standalone scope made from a copy of this one's (`standalone_scope`) added: the `ac_stim` sites
    /// and the statements that write them, the branches a flow read numbered, and the flow leaves. So a stimulus or a
    /// flow read inside a value a primitive evaluates on its own (a `noise` density, a `delay` input, a `ddt` state, a
    /// `ddx` value) is a site or a branch of the body. The `limit` sites it made stay its own: a `limit` there is read
    /// at a solution (`at_solution`) or differentiated through, never evaluated.
    pub(in crate::elab) fn adopt_standalone(&mut self, standalone: &mut Scope) {
        self.analog.stims = standalone.analog.stims.clone();
        self.analog.branches = standalone.analog.branches.clone();
        self.analog.flow_leaves.append(&mut standalone.analog.flow_leaves);
        self.analog.hoisted_steps.append(&mut standalone.analog.hoisted_steps);
    }

    /// A scope for code of the body outside its `analog` block and its functions (a drive, a
    /// reaction): the names of `scope` (params, frame), with no local of its own.
    pub(in crate::elab) fn sub_scope(scope: &Scope) -> Scope {
        let mut sub = Scope::new(scope.values.file);
        sub.values.params = scope.values.params.clone();
        sub.values.keys = scope.values.keys.clone();
        sub.values.frame = scope.values.frame;
        sub.values.unrolled = scope.values.unrolled.clone();
        sub.digital.ordinals = scope.digital.ordinals.clone();
        sub
    }

    /// The scope of code written in `file`: no param, no local, frame 0, outside every analog block, loop and function.
    pub(in crate::elab) fn new(file: FileId) -> Self {
        Scope { values: ValueScope::new(file), analog: AnalogScope::default(), digital: DigitalScope::default() }
    }
}
