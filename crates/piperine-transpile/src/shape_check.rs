//! A loaded prototype against the plan's shape, entry by entry (design "Tech Decisions": the
//! source describes itself, and a printing defect is an error at load, not a wrong matrix).
//! Everything a shape holds is compared: pins (with layouts and init bytes), terms, inputs,
//! watches, functions (cells and partials) and reactions, names and origins included.

use piperine_abi::raw::{Bytes, Layout, List, Shape};

/// Compares two shapes.
#[derive(Debug)]
pub struct ShapeCheck;

impl ShapeCheck {
    /// `None` when `loaded` equals `plan` entry by entry; otherwise the first entry that
    /// differs, with what each side holds.
    pub fn first_difference(plan: &Shape<'_>, loaded: &Shape<'_>) -> Option<String> {
        let (a, b) = (Self::entries(plan), Self::entries(loaded));
        for ((kind, expected), (_, found)) in a.iter().zip(&b) {
            if expected.len() != found.len() {
                return Some(format!("{kind} count: the plan has {}, the library {}", expected.len(), found.len()));
            }
            for (i, (want, got)) in expected.iter().zip(found).enumerate() {
                if want != got {
                    return Some(format!("{kind} {i}: the plan has {want}, the library {got}"));
                }
            }
        }
        None
    }

    /// Each list of the shape as one rendered line per entry.
    fn entries(shape: &Shape<'_>) -> Vec<(&'static str, Vec<String>)> {
        let each = |len: usize, line: &dyn Fn(usize) -> Option<String>| -> Vec<String> {
            (0..len).map(|i| line(i).unwrap_or_else(|| "<unreadable>".to_string())).collect()
        };
        vec![
            (
                "pin",
                each(shape.pins.len(), &|i| {
                    shape.pins.get(i).map(|p| {
                        format!(
                            "`{}` @ {} kind {} role {} global {} flags {} layout {} unit `{}` init {:?} check {} abstol {:?} count {}",
                            p.name.as_str(), p.origin.as_str(), p.kind, p.role, p.global, p.flags,
                            Self::layout(&p.layout), p.unit.as_str(), Self::bytes(&p.init), p.check, p.abstol, p.count
                        )
                    })
                }),
            ),
            (
                "term",
                each(shape.terms.len(), &|i| {
                    shape.terms.get(i).map(|t| {
                        format!(
                            "`{}` @ {} op {} plus {} minus {} abstol {} max {}",
                            t.name.as_str(), t.origin.as_str(), t.op, t.plus, t.minus, t.abstol, t.max
                        )
                    })
                }),
            ),
            (
                "input",
                each(shape.inputs.len(), &|i| {
                    shape.inputs.get(i).map(|x| format!("`{}` @ {} kind {}", x.name.as_str(), x.origin.as_str(), x.kind))
                }),
            ),
            (
                "watch",
                each(shape.watches.len(), &|i| {
                    shape.watches.get(i).map(|w| {
                        format!(
                            "`{}` @ {} kind {} layout {} event {} ttol {} etol {}",
                            w.name.as_str(), w.origin.as_str(), w.kind, Self::layout(&w.layout), w.event, w.ttol, w.etol
                        )
                    })
                }),
            ),
            (
                "function",
                each(shape.functions.len(), &|i| {
                    shape.functions.get(i).map(|f| {
                        format!(
                            "`{}` @ {} flags {} reads {:?} writes {:?} partials {:?} elements {:?}",
                            f.name.as_str(), f.origin.as_str(), f.flags,
                            Self::list(&f.reads), Self::list(&f.writes), Self::list(&f.partials), Self::list(&f.elements)
                        )
                    })
                }),
            ),
            (
                "reaction",
                each(shape.reactions.len(), &|i| {
                    shape.reactions.get(i).map(|r| {
                        format!(
                            "`{}` @ {} flags {} on {:?} reads {:?} writes {:?} elements {:?}",
                            r.name.as_str(), r.origin.as_str(), r.flags,
                            Self::list(&r.on), Self::list(&r.reads), Self::list(&r.writes), Self::list(&r.elements)
                        )
                    })
                }),
            ),
        ]
    }

    fn layout(layout: &Layout<'_>) -> String {
        let fields: Vec<String> = (0..layout.fields.len())
            .map(|i| match layout.fields.get(i) {
                Some(f) => format!(
                    "`{}` leaf {} at {} x{}",
                    f.name.as_str(), f.leaf, f.offset, f.count
                ),
                None => "<unreadable>".to_string(),
            })
            .collect();
        format!("{}/{} {fields:?}", layout.size, layout.align)
    }

    fn bytes(bytes: &Bytes<'_>) -> Vec<u8> {
        bytes.as_slice().to_vec()
    }

    fn list<T: std::fmt::Debug>(list: &List<'_, T>) -> Vec<String> {
        (0..list.len()).map(|i| list.get(i).map_or_else(|| "<unreadable>".to_string(), |x| format!("{x:?}"))).collect()
    }
}

#[cfg(test)]
mod tests {
    use piperine_abi::raw::{cell, part, role};
    use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf};

    use piperine_abi::raw::Cell;

    use super::ShapeCheck;

    /// A shape of one pin and its reads, as bytes.
    struct PinShape;

    impl PinShape {
        fn shape(pin: &str, read: u32) -> ShapeBuf {
            let mut b = ShapeBuf::new();
            b.pin(PinSpec::continuous(pin, role::PORT));
            b.function(FunctionSpec {
                name: "terms",
                origin: "t.msl:1:1",
                flags: 0,
                reads: vec![Cell { kind: cell::PIN, index: read, part: part::VALUE }],
                writes: vec![],
                partials: vec![],
                elements: vec![],
            });
            b
        }
    }

    #[test]
    fn equal_shapes_have_no_difference_and_a_changed_entry_is_named() {
        let (a, same, other_name, other_read) = (PinShape::shape("p", 0), PinShape::shape("p", 0), PinShape::shape("q", 0), PinShape::shape("p", 1));
        assert_eq!(ShapeCheck::first_difference(a.shape(), same.shape()), None);
        let name = ShapeCheck::first_difference(a.shape(), other_name.shape()).expect("a pin name differs");
        assert!(name.starts_with("pin 0:") && name.contains("`p`") && name.contains("`q`"), "{name}");
        let read = ShapeCheck::first_difference(a.shape(), other_read.shape()).expect("a read differs");
        assert!(read.starts_with("function 0:"), "{read}");
    }
}
