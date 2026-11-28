//! The C contract of abi.md §3: every struct `#[repr(C)]`
//! with its fields in the listed order, every enumeration a group of named
//! `u32` constants. Values start at 1 (flags at bit 0), so an all-zero entry
//! never carries a valid kind. Beside it, the one typed view of a cell's
//! bytes (`Pod`, with `from_bytes`, `from_bytes_mut` and `as_bytes`) the lenses use.
//!
//! One file per section of the contract: `views`, `calls`, `shape`, `constants`; every path is re-exported here.

mod calls;
mod constants;
mod shape;
mod views;

pub use calls::*;
pub use constants::*;
pub use shape::*;
pub use views::*;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    /// The comparison the layout tests share.
    struct LayoutChecks;

    impl LayoutChecks {
        /// One struct of the table: its measured size and alignment against the C
        /// layout, and each field's measured offset against the expected one.
        fn check(name: &str, size: (usize, usize), align: (usize, usize), fields: &[(&str, usize, usize)]) {
            assert_eq!(size.0, size.1, "size_of::<{name}>()");
            assert_eq!(align.0, align.1, "align_of::<{name}>()");
            for (field, got, want) in fields {
                assert_eq!(got, want, "offset of {name}.{field}");
            }
        }
    }
    
    #[test]
    fn struct_layouts_match_the_abi() {
        assert_eq!(size_of::<usize>(), 8, "the table is for a 64-bit target");
    
        LayoutChecks::check("Str", (size_of::<Str>(), 16), (align_of::<Str>(), 8), &[
            ("ptr", offset_of!(Str, ptr), 0),
            ("len", offset_of!(Str, len), 8),
        ]);
        LayoutChecks::check("Bytes", (size_of::<Bytes>(), 16), (align_of::<Bytes>(), 8), &[
            ("ptr", offset_of!(Bytes, ptr), 0),
            ("len", offset_of!(Bytes, len), 8),
        ]);
        LayoutChecks::check("List", (size_of::<List<Pin>>(), 24), (align_of::<List<Pin>>(), 8), &[
            ("ptr", offset_of!(List<Pin>, ptr), 0),
            ("len", offset_of!(List<Pin>, len), 8),
            ("stride", offset_of!(List<Pin>, stride), 16),
        ]);
        LayoutChecks::check("Library", (size_of::<Library>(), 64), (align_of::<Library>(), 8), &[
            ("size", offset_of!(Library, size), 0),
            ("minor", offset_of!(Library, minor), 4),
            ("name", offset_of!(Library, name), 8),
            ("build", offset_of!(Library, build), 24),
            ("modules", offset_of!(Library, modules), 40),
        ]);
        LayoutChecks::check("Module", (size_of::<Module>(), 64), (align_of::<Module>(), 8), &[
            ("name", offset_of!(Module, name), 0),
            ("prototype", offset_of!(Module, prototype), 16),
            ("data", offset_of!(Module, data), 24),
            ("create", offset_of!(Module, create), 32),
            ("destroy", offset_of!(Module, destroy), 40),
            ("eval", offset_of!(Module, eval), 48),
            ("react", offset_of!(Module, react), 56),
        ]);
        LayoutChecks::check("Value", (size_of::<Value>(), 24), (align_of::<Value>(), 8), &[
            ("pin", offset_of!(Value, pin), 0),
            ("bytes", offset_of!(Value, bytes), 8),
        ]);
        LayoutChecks::check("Created", (size_of::<Created>(), 24), (align_of::<Created>(), 8), &[
            ("status", offset_of!(Created, status), 0),
            ("instance", offset_of!(Created, instance), 8),
            ("shape", offset_of!(Created, shape), 16),
        ]);
        LayoutChecks::check("Instance", (size_of::<Instance>(), 0), (align_of::<Instance>(), 1), &[]);
        LayoutChecks::check("Note", (size_of::<Note>(), 256), (align_of::<Note>(), 4), &[
            ("code", offset_of!(Note, code), 0),
            ("len", offset_of!(Note, len), 4),
            ("text", offset_of!(Note, text), 8),
        ]);
        LayoutChecks::check("Shape", (size_of::<Shape>(), 152), (align_of::<Shape>(), 8), &[
            ("size", offset_of!(Shape, size), 0),
            ("pins", offset_of!(Shape, pins), 8),
            ("terms", offset_of!(Shape, terms), 32),
            ("inputs", offset_of!(Shape, inputs), 56),
            ("watches", offset_of!(Shape, watches), 80),
            ("functions", offset_of!(Shape, functions), 104),
            ("reactions", offset_of!(Shape, reactions), 128),
        ]);
        LayoutChecks::check("Pin", (size_of::<Pin>(), 144), (align_of::<Pin>(), 8), &[
            ("name", offset_of!(Pin, name), 0),
            ("origin", offset_of!(Pin, origin), 16),
            ("kind", offset_of!(Pin, kind), 32),
            ("role", offset_of!(Pin, role), 36),
            ("global", offset_of!(Pin, global), 40),
            ("flags", offset_of!(Pin, flags), 44),
            ("layout", offset_of!(Pin, layout), 48),
            ("unit", offset_of!(Pin, unit), 80),
            ("init", offset_of!(Pin, init), 96),
            ("check", offset_of!(Pin, check), 112),
            ("abstol", offset_of!(Pin, abstol), 120),
            ("count", offset_of!(Pin, count), 136),
        ]);
        LayoutChecks::check("Layout", (size_of::<Layout>(), 32), (align_of::<Layout>(), 8), &[
            ("size", offset_of!(Layout, size), 0),
            ("align", offset_of!(Layout, align), 4),
            ("fields", offset_of!(Layout, fields), 8),
        ]);
        LayoutChecks::check("Field", (size_of::<Field>(), 32), (align_of::<Field>(), 8), &[
            ("name", offset_of!(Field, name), 0),
            ("leaf", offset_of!(Field, leaf), 16),
            ("offset", offset_of!(Field, offset), 20),
            ("count", offset_of!(Field, count), 24),
        ]);
        LayoutChecks::check("Term", (size_of::<Term>(), 56), (align_of::<Term>(), 8), &[
            ("name", offset_of!(Term, name), 0),
            ("origin", offset_of!(Term, origin), 16),
            ("op", offset_of!(Term, op), 32),
            ("plus", offset_of!(Term, plus), 36),
            ("minus", offset_of!(Term, minus), 40),
            ("abstol", offset_of!(Term, abstol), 44),
            ("max", offset_of!(Term, max), 48),
        ]);
        LayoutChecks::check("Input", (size_of::<Input>(), 40), (align_of::<Input>(), 8), &[
            ("name", offset_of!(Input, name), 0),
            ("origin", offset_of!(Input, origin), 16),
            ("kind", offset_of!(Input, kind), 32),
        ]);
        LayoutChecks::check("Watch", (size_of::<Watch>(), 88), (align_of::<Watch>(), 8), &[
            ("name", offset_of!(Watch, name), 0),
            ("origin", offset_of!(Watch, origin), 16),
            ("kind", offset_of!(Watch, kind), 32),
            ("layout", offset_of!(Watch, layout), 40),
            ("event", offset_of!(Watch, event), 72),
            ("ttol", offset_of!(Watch, ttol), 76),
            ("etol", offset_of!(Watch, etol), 80),
        ]);
        LayoutChecks::check("Cell", (size_of::<Cell>(), 12), (align_of::<Cell>(), 4), &[
            ("kind", offset_of!(Cell, kind), 0),
            ("index", offset_of!(Cell, index), 4),
            ("part", offset_of!(Cell, part), 8),
        ]);
        LayoutChecks::check("Partial", (size_of::<Partial>(), 16), (align_of::<Partial>(), 4), &[
            ("write", offset_of!(Partial, write), 0),
            ("to", offset_of!(Partial, to), 4),
            ("read", offset_of!(Partial, read), 8),
            ("at", offset_of!(Partial, at), 12),
        ]);
        LayoutChecks::check("Function", (size_of::<Function>(), 136), (align_of::<Function>(), 8), &[
            ("name", offset_of!(Function, name), 0),
            ("origin", offset_of!(Function, origin), 16),
            ("flags", offset_of!(Function, flags), 32),
            ("reads", offset_of!(Function, reads), 40),
            ("writes", offset_of!(Function, writes), 64),
            ("partials", offset_of!(Function, partials), 88),
            ("elements", offset_of!(Function, elements), 112),
        ]);
        LayoutChecks::check("Reaction", (size_of::<Reaction>(), 136), (align_of::<Reaction>(), 8), &[
            ("name", offset_of!(Reaction, name), 0),
            ("origin", offset_of!(Reaction, origin), 16),
            ("flags", offset_of!(Reaction, flags), 32),
            ("on", offset_of!(Reaction, on), 40),
            ("reads", offset_of!(Reaction, reads), 64),
            ("writes", offset_of!(Reaction, writes), 88),
            ("elements", offset_of!(Reaction, elements), 112),
        ]);
        LayoutChecks::check("Statement", (size_of::<Statement>(), 24), (align_of::<Statement>(), 8), &[
            ("pin", offset_of!(Statement, pin), 0),
            ("offset", offset_of!(Statement, offset), 4),
            ("len", offset_of!(Statement, len), 8),
            ("flags", offset_of!(Statement, flags), 12),
            ("min_after", offset_of!(Statement, min_after), 16),
        ]);
        LayoutChecks::check("Eval", (size_of::<Eval>(), 48), (align_of::<Eval>(), 8), &[
            ("size", offset_of!(Eval, size), 0),
            ("input", offset_of!(Eval, input), 8),
            ("output", offset_of!(Eval, output), 16),
            ("seed", offset_of!(Eval, seed), 24),
            ("jac", offset_of!(Eval, jac), 32),
            ("note", offset_of!(Eval, note), 40),
        ]);
        LayoutChecks::check("React", (size_of::<React>(), 40), (align_of::<React>(), 8), &[
            ("size", offset_of!(React, size), 0),
            ("input", offset_of!(React, input), 8),
            ("host", offset_of!(React, host), 16),
            ("host_data", offset_of!(React, host_data), 24),
            ("note", offset_of!(React, note), 32),
        ]);
        LayoutChecks::check("Host", (size_of::<Host>(), 24), (align_of::<Host>(), 8), &[
            ("size", offset_of!(Host, size), 0),
            ("commit", offset_of!(Host, commit), 8),
            ("commit_element", offset_of!(Host, commit_element), 16),
        ]);
        LayoutChecks::check("ArrayView", (size_of::<ArrayView>(), 32), (align_of::<ArrayView>(), 8), &[
            ("context", offset_of!(ArrayView, context), 0),
            ("count", offset_of!(ArrayView, count), 8),
            ("size", offset_of!(ArrayView, size), 16),
            ("read", offset_of!(ArrayView, read), 24),
        ]);
    }
}
