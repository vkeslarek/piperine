//! `Table`: the result of an analysis as named columns and rows of typed cells (CLI-05 AC 1, 2
//! and 7). A discrete net is decoded by its carrier type from the bytes the solver holds: the
//! value little-endian at its field's offset, its known byte beside it (`CarrierLayout`,
//! contract D3); a known byte of 0 is `Cell::Unknown`.

use piperine_abi::raw::leaf;
use piperine_ir::ValueType;
use piperine_plan::CarrierLayout;
use piperine_solver::prelude::{AcAnalysisResult, DcAnalysisResult, Net, PoleZeroResult, SensResult, TransferFunctionAnalysisResult, TransientAnalysisResult};

use crate::failure::Failure;
use crate::probe::Probe;
use crate::session::ParamPath;

/// One value of a table.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Real(f64),
    Bool(bool),
    Int(i128),
    /// A name, such as the signal column of an operating point.
    Text(String),
    /// A discrete value whose known byte is clear.
    Unknown,
}

/// Columns and rows; every row has one cell per column.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

impl Cell {
    /// The value of `bytes`, laid out as a carrier of `ty`. Bytes of another length than the
    /// layout's are a `Failure`, never a truncated value.
    pub fn decode(ty: &ValueType, bytes: &[u8]) -> Result<Cell, Failure> {
        let ty = match ty {
            ValueType::Flagged(inner) => inner,
            other => other,
        };
        let refused = |why: String| Failure::Backend(format!("cannot decode a value of type {ty}: {why}"));
        let layout = CarrierLayout::of(ty).ok_or_else(|| refused("no carrier holds it".into()))?;
        if bytes.len() != layout.size as usize {
            return Err(refused(format!("{} bytes where its layout holds {}", bytes.len(), layout.size)));
        }
        let field = layout.fields.get(0).ok_or_else(|| refused("its layout has no field".into()))?;
        if bytes[field.known as usize] == 0 {
            return Ok(Cell::Unknown);
        }
        let at = field.offset as usize;
        let le = |n: usize| {
            let mut word = [0u8; 8];
            word[..n].copy_from_slice(&bytes[at..at + n]);
            u64::from_le_bytes(word)
        };
        // a signed leaf of `n` bytes, sign-extended
        let signed = |n: usize| {
            let shift = 64 - 8 * n as u32;
            i128::from(((le(n) << shift) as i64) >> shift)
        };
        Ok(match field.leaf {
            leaf::BOOL => Cell::Bool(bytes[at] != 0),
            leaf::F64 => Cell::Real(f64::from_bits(le(8))),
            leaf::U8 => Cell::Int(i128::from(le(1))),
            leaf::U16 => Cell::Int(i128::from(le(2))),
            leaf::U32 => Cell::Int(i128::from(le(4))),
            leaf::U64 => Cell::Int(i128::from(le(8))),
            leaf::I8 => Cell::Int(signed(1)),
            leaf::I16 => Cell::Int(signed(2)),
            leaf::I32 => Cell::Int(signed(4)),
            leaf::I64 => Cell::Int(signed(8)),
            other => return Err(refused(format!("leaf {other} is no value a table shows"))),
        })
    }
}

impl Table {
    /// The operating point: columns `signal` and `value`, one row per probe in order. `net`
    /// gives the bytes of a discrete net by its path.
    pub fn operating_point(probes: &[Probe], result: &DcAnalysisResult, net: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Table, Failure> {
        let rows = probes
            .iter()
            .zip(Table::point(probes, result, net)?)
            .map(|(probe, value)| vec![Cell::Text(probe.label()), value])
            .collect();
        Ok(Table { columns: vec!["signal".into(), "value".into()], rows })
    }

    /// The value of every probe at one operating point, in order.
    pub fn point(probes: &[Probe], result: &DcAnalysisResult, net: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Vec<Cell>, Failure> {
        probes
            .iter()
            .map(|probe| {
                let value = match probe {
                    Probe::Node { id, .. } => result.get_node(id).map(Cell::Real),
                    Probe::Flow { branch, .. } => result.get_branch(branch.clone()).map(Cell::Real),
                    Probe::Net { path, ty } => net(path).map(|bytes| Cell::decode(ty, &bytes)).transpose()?,
                };
                value.ok_or_else(|| Failure::Backend(format!("the operating point holds no value for `{}`", probe.label())))
            })
            .collect()
    }

    /// The transfer function: columns `quantity` and `value`, the rows `gain`, `input_resistance`
    /// and `output_resistance` in that order (the solver's own field names).
    pub fn transfer(result: &TransferFunctionAnalysisResult) -> Table {
        let rows = [("gain", result.gain), ("input_resistance", result.input_resistance), ("output_resistance", result.output_resistance)]
            .into_iter()
            .map(|(quantity, value)| vec![Cell::Text(quantity.into()), Cell::Real(value)])
            .collect();
        Table { columns: vec!["quantity".into(), "value".into()], rows }
    }

    /// The poles and zeros: columns `kind`, `real` and `imag` (radians per second), one row per
    /// pole then one per zero, each in the solver's order.
    pub fn roots(result: &PoleZeroResult) -> Table {
        let kinds = [("pole", &result.poles), ("zero", &result.zeros)];
        let rows = kinds
            .into_iter()
            .flat_map(|(kind, roots)| roots.iter().map(move |root| vec![Cell::Text(kind.into()), Cell::Real(root.re), Cell::Real(root.im)]))
            .collect();
        Table { columns: vec!["kind".into(), "real".into(), "imag".into()], rows }
    }

    /// The sensitivities: the column `param` (the full path), then `d(<label>)` per probe; one row
    /// per `wrt` entry in order. `outputs[i]` is the net the solver differentiated for
    /// `probes[i]`, the key its result holds the derivative under. A derivative the result lacks is
    /// a `Failure`, never a zero.
    pub fn sensitivities(probes: &[Probe], outputs: &[Net], wrt: &[ParamPath], result: &SensResult) -> Result<Table, Failure> {
        let mut columns = vec!["param".to_string()];
        columns.extend(probes.iter().map(|probe| format!("d({})", probe.label())));
        let rows = wrt
            .iter()
            .map(|param| {
                let mut row = vec![Cell::Text(param.label())];
                for (probe, net) in probes.iter().zip(outputs) {
                    let derivative = result.get(net.label(), &param.instance, &param.param);
                    let missing = || Failure::Backend(format!("the sensitivity holds no derivative of `{}` by `{}`", probe.label(), param.label()));
                    row.push(Cell::Real(derivative.ok_or_else(missing)?));
                }
                Ok(row)
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        Ok(Table { columns, rows })
    }

    /// A DC sweep: the column `swept` (the param's full path), then one column per probe; one
    /// row per point, `points` giving the swept value and the probes' values at it.
    pub fn sweep(swept: &str, probes: &[Probe], points: Vec<(f64, Vec<Cell>)>) -> Table {
        let mut columns = vec![swept.to_string()];
        columns.extend(probes.iter().map(Probe::label));
        let rows = points
            .into_iter()
            .map(|(value, cells)| {
                let mut row = vec![Cell::Real(value)];
                row.extend(cells);
                row
            })
            .collect();
        Table { columns, rows }
    }

    /// An AC sweep: the column `freq`, then `mag(<path>)` and `phase(<path>)` per probe, the
    /// phase in degrees; one row per frequency. A discrete net has no phasor: `E0801`.
    pub fn frequency(probes: &[Probe], result: &AcAnalysisResult) -> Result<Table, Failure> {
        let mut columns = vec!["freq".to_string()];
        for probe in probes {
            if let Probe::Net { path, .. } = probe {
                let message = format!("`{path}` is a discrete net: an `ac` run prints nodes and flows, which have a phasor");
                return Err(Failure::Host { code: "E0801", message });
            }
            columns.push(format!("mag({})", probe.label()));
            columns.push(format!("phase({})", probe.label()));
        }
        let rows = result
            .iter()
            .map(|step| {
                let mut row = vec![Cell::Real(step.frequency)];
                for probe in probes {
                    let phasor = match probe {
                        Probe::Node { id, .. } => step.get_node(id),
                        Probe::Flow { branch, .. } => step.get_branch(branch.clone()),
                        Probe::Net { .. } => None,
                    };
                    let phasor = phasor.ok_or_else(|| Failure::Backend(format!("the step at {} Hz holds no value for `{}`", step.frequency, probe.label())))?;
                    row.push(Cell::Real(phasor.norm()));
                    row.push(Cell::Real(phasor.arg().to_degrees()));
                }
                Ok(row)
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        Ok(Table { columns, rows })
    }

    /// A transient: the column `time`, then one column per probe; one row per step the result
    /// keeps, in time order.
    pub fn transient(probes: &[Probe], result: &TransientAnalysisResult) -> Result<Table, Failure> {
        let mut columns = vec!["time".to_string()];
        columns.extend(probes.iter().map(Probe::label));
        let rows = result
            .iter()
            .map(|step| {
                let mut row = vec![Cell::Real(step.time())];
                for probe in probes {
                    let value = match probe {
                        Probe::Node { id, .. } => step.get_node(id).map(Cell::Real),
                        Probe::Flow { branch, .. } => step.get_branch(branch.clone()).map(Cell::Real),
                        Probe::Net { path, ty } => step.digital_named(path).map(|bytes| Cell::decode(ty, bytes)).transpose()?,
                    };
                    let missing = || Failure::Backend(format!("the step at {} holds no value for `{}`", step.time(), probe.label()));
                    row.push(value.ok_or_else(missing)?);
                }
                Ok(row)
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        Ok(Table { columns, rows })
    }

    /// The table on a print grid (CLI-11 AC 4 and 5): one row per multiple `k*step` of `step`
    /// not past `stop`, from the first multiple not before `from`. The first column is the time;
    /// a real column is interpolated linearly between the two accepted steps around the time (a
    /// time that is an accepted step gives that step's value), a `bool` or integer column holds
    /// the last step at or before it, and a value unknown at either step stays unknown. A
    /// multiple before the first step the table holds (a run that dropped its start, `from`) is
    /// not a row; one past its last step by more than rounding is a `Failure`.
    pub fn on_grid(&self, step: f64, from: f64, stop: f64) -> Result<Table, Failure> {
        let time = |row: &Vec<Cell>| match row.first() {
            Some(Cell::Real(t)) => Ok(*t),
            _ => Err(Failure::Backend("a table with no time in its first column has no print grid".into())),
        };
        let times = self.rows.iter().map(time).collect::<Result<Vec<f64>, Failure>>()?;
        let (Some(&first), Some(&last)) = (times.first(), times.last()) else { return Ok(self.clone()) };
        // the tolerance on k absorbs the rounding of `stop / step` and `from / step`
        let kmin = (from / step - 1e-9).ceil().max(0.0) as u64;
        let kmax = (stop / step + 1e-9).floor() as u64;
        let slack = 1e-9 * step;
        let mut rows = Vec::new();
        for k in kmin..=kmax {
            let at = k as f64 * step;
            if at < first {
                continue;
            }
            if at > last + slack {
                return Err(Failure::Backend(format!("the print grid reaches {at}, past the last step of the run at {last}")));
            }
            // the last step at or before `at`
            let lo = times.partition_point(|t| *t <= at).saturating_sub(1);
            let mut row = vec![Cell::Real(at)];
            for column in 1..self.columns.len() {
                let low = &self.rows[lo][column];
                let value = match (low, self.rows.get(lo + 1)) {
                    _ if times[lo] == at || at > last => low.clone(),
                    (Cell::Real(a), Some(next)) => match &next[column] {
                        Cell::Real(b) => Cell::Real(a + (b - a) * (at - times[lo]) / (times[lo + 1] - times[lo])),
                        _ => Cell::Unknown,
                    },
                    (other, _) => other.clone(),
                };
                row.push(value);
            }
            rows.push(row);
        }
        Ok(Table { columns: self.columns.clone(), rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex;
    use piperine_solver::prelude::{NodeIdentifier, TransferType};

    /// time, a real, a bool, an integer.
    fn run(steps: &[(f64, f64, bool, i128)]) -> Table {
        Table {
            columns: vec!["time".into(), "v".into(), "b".into(), "n".into()],
            rows: steps.iter().map(|(t, v, b, n)| vec![Cell::Real(*t), Cell::Real(*v), Cell::Bool(*b), Cell::Int(*n)]).collect(),
        }
    }

    fn times(table: &Table) -> Vec<f64> {
        table.rows.iter().map(|r| if let Cell::Real(t) = r[0] { t } else { panic!("a time") }).collect()
    }

    #[test]
    fn a_print_grid_has_one_row_per_multiple() {
        // CLI-11 AC 4: accepted steps at 0, 0.3, 0.7, 1, 2.5; step 1, stop 2.5: the multiples
        // 0, 1, 2 are not past 2.5 and 3 is
        let table = run(&[(0.0, 0.0, false, 0), (0.3, 3.0, false, 0), (0.7, 1.0, false, 0), (1.0, 9.0, false, 0), (2.5, 0.0, false, 0)]);
        let grid = table.on_grid(1.0, 0.0, 2.5).expect("grid");
        assert_eq!(times(&grid), [0.0, 1.0, 2.0]);
        assert_eq!(grid.columns, table.columns);
        // a stop that is a multiple keeps it: 0.5, 1.0 with stop 1.0 and step 0.5 is 3 rows
        let table = run(&[(0.0, 0.0, false, 0), (1.0, 1.0, false, 0)]);
        assert_eq!(times(&table.on_grid(0.5, 0.0, 1.0).expect("grid")), [0.0, 0.5, 1.0]);
        // CLI-11 AC 4, `from`: the rows start at the first multiple not before `from` (0.5:
        // multiples 1 and 2); the table starts at 0.6, as a run with `record_from` 0.5 would
        let table = run(&[(0.6, 6.0, false, 0), (1.5, 15.0, false, 0), (2.0, 20.0, false, 0)]);
        assert_eq!(times(&table.on_grid(1.0, 0.5, 2.0).expect("grid")), [1.0, 2.0]);
        // a multiple before the first kept step is no row: from 0.5 against a table from 1.2
        let table = run(&[(1.2, 1.0, false, 0), (2.0, 2.0, false, 0)]);
        assert_eq!(times(&table.on_grid(1.0, 0.5, 2.0).expect("grid")), [2.0]);
    }

    #[test]
    fn continuous_is_interpolated_and_discrete_is_held() {
        // CLI-11 AC 5: v is 1 at 0.7 and 9 at 1.0, so at 0.9 it is 1 + 8 * 0.2 / 0.3 = 6.333...;
        // at 1.0, an accepted step, it is that step's 9 exactly; b and n hold the last step at
        // or before the time: at 0.9 the values of 0.7, at 1.0 those of 1.0
        let table = run(&[(0.0, 0.0, false, 0), (0.7, 1.0, true, 7), (1.0, 9.0, false, 10), (2.0, 3.0, true, 20)]);
        let grid = table.on_grid(0.3, 0.0, 1.8).expect("grid");
        let at = |t: f64| grid.rows.iter().find(|r| r[0] == Cell::Real(t)).unwrap_or_else(|| panic!("no row at {t}: {grid:?}")).clone();
        // the multiples of 0.3 are k * 0.3 as the grid computes them
        let t09 = 3.0 * 0.3;
        let Cell::Real(v) = at(t09)[1] else { panic!("a real") };
        assert!((v - (1.0 + 8.0 * (t09 - 0.7) / 0.3)).abs() <= 1e-12, "{v}");
        assert_eq!(at(t09)[2], Cell::Bool(true));
        assert_eq!(at(t09)[3], Cell::Int(7));
        // the exact step: a grid of 1.0 over the same table
        let grid = table.on_grid(1.0, 0.0, 2.0).expect("grid");
        assert_eq!(grid.rows[1], vec![Cell::Real(1.0), Cell::Real(9.0), Cell::Bool(false), Cell::Int(10)]);
        assert_eq!(grid.rows[2], vec![Cell::Real(2.0), Cell::Real(3.0), Cell::Bool(true), Cell::Int(20)]);
        // an unknown stays unknown: at 0.5 between a real and an unknown, and a held unknown
        let table = Table {
            columns: vec!["time".into(), "v".into(), "b".into()],
            rows: vec![
                vec![Cell::Real(0.0), Cell::Real(1.0), Cell::Unknown],
                vec![Cell::Real(1.0), Cell::Unknown, Cell::Bool(true)],
            ],
        };
        let grid = table.on_grid(0.5, 0.0, 1.0).expect("grid");
        assert_eq!(grid.rows[1], vec![Cell::Real(0.5), Cell::Unknown, Cell::Unknown]);
        assert_eq!(grid.rows[2], vec![Cell::Real(1.0), Cell::Unknown, Cell::Bool(true)]);
    }

    #[test]
    fn a_grid_wider_than_the_run_is_the_row_at_zero() {
        // edge case: `print_step` greater than `stop` gives the single row at time 0
        let table = run(&[(0.0, 4.0, true, 1), (1.0, 8.0, false, 2), (2.5, 9.0, false, 3)]);
        let grid = table.on_grid(10.0, 0.0, 2.5).expect("grid");
        assert_eq!(grid.rows, [vec![Cell::Real(0.0), Cell::Real(4.0), Cell::Bool(true), Cell::Int(1)]]);
        // a table the run cut short of `stop` is a failure, never a row held past its end
        assert!(table.on_grid(1.0, 0.0, 3.5).is_err());
    }

    #[test]
    fn a_transfer_table_has_three_rows_in_order() {
        // HII-04 AC 1: columns quantity and value; rows gain, input then output resistance (the
        // solver's field names); a value is the solver's, to the bit
        let result = TransferFunctionAnalysisResult { gain: 0.75, input_resistance: 4000.0, output_resistance: 750.0, tf_type: TransferType::VoltageGain };
        let table = Table::transfer(&result);
        assert_eq!(table.columns, ["quantity", "value"]);
        assert_eq!(
            table.rows,
            [
                vec![Cell::Text("gain".into()), Cell::Real(0.75)],
                vec![Cell::Text("input_resistance".into()), Cell::Real(4000.0)],
                vec![Cell::Text("output_resistance".into()), Cell::Real(750.0)],
            ]
        );
    }

    #[test]
    fn roots_list_poles_then_zeros() {
        // HII-05 AC 1: kind, real, imag; every pole in the solver's order, then every zero
        let result = PoleZeroResult {
            poles: vec![Complex::new(-1.0e3, 2.0e3), Complex::new(-1.0e3, -2.0e3), Complex::new(-5.0, 0.0)],
            zeros: vec![Complex::new(7.0, 0.0)],
        };
        let table = Table::roots(&result);
        assert_eq!(table.columns, ["kind", "real", "imag"]);
        let row = |kind: &str, re: f64, im: f64| vec![Cell::Text(kind.into()), Cell::Real(re), Cell::Real(im)];
        assert_eq!(table.rows, [row("pole", -1.0e3, 2.0e3), row("pole", -1.0e3, -2.0e3), row("pole", -5.0, 0.0), row("zero", 7.0, 0.0)]);
        // no zeros is a legitimate answer: the poles alone
        let none = Table::roots(&PoleZeroResult { poles: vec![Complex::new(-1.0, 0.0)], zeros: Vec::new() });
        assert_eq!(none.rows, [row("pole", -1.0, 0.0)]);
    }

    #[test]
    fn a_sensitivity_table_has_a_row_per_wrt() {
        // HII-06 AC 1: `param`, then `d(<signal>)` per printed signal; one row per `wrt` entry in
        // the order given, each cell the derivative the result holds under that signal's net
        let probes = [
            Probe::Node { path: "Top.out".into(), id: NodeIdentifier::Anonymous(2) },
            Probe::Node { path: "Top.mid".into(), id: NodeIdentifier::Anonymous(3) },
        ];
        let outputs = [Net::node(1, "v(n2)"), Net::node(2, "v(n3)")];
        let wrt = [ParamPath { instance: "Top.r2".into(), param: "r".into() }, ParamPath { instance: "Top.r1".into(), param: "r".into() }];
        let mut d = std::collections::HashMap::new();
        for (net, param, value) in [("v(n2)", "Top.r2.r", 6.25e-4), ("v(n2)", "Top.r1.r", -1.875e-3), ("v(n3)", "Top.r2.r", 1.0), ("v(n3)", "Top.r1.r", 2.0)] {
            d.insert((net.to_string(), param.to_string()), value);
        }
        let table = Table::sensitivities(&probes, &outputs, &wrt, &SensResult { d }).expect("table");
        assert_eq!(table.columns, ["param", "d(Top.out)", "d(Top.mid)"]);
        assert_eq!(
            table.rows,
            [
                vec![Cell::Text("Top.r2.r".into()), Cell::Real(6.25e-4), Cell::Real(1.0)],
                vec![Cell::Text("Top.r1.r".into()), Cell::Real(-1.875e-3), Cell::Real(2.0)],
            ]
        );
        // a derivative the solver did not give is a failure, never a zero
        let empty = SensResult { d: std::collections::HashMap::new() };
        assert!(Table::sensitivities(&probes, &outputs, &wrt, &empty).is_err());
    }
}
