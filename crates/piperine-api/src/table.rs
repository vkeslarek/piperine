//! `Table`: the result of an analysis as named columns and rows of typed cells (CLI-05 AC 1, 2
//! and 7). A discrete net is decoded by its carrier type from the bytes the solver holds: the
//! value little-endian at its field's offset, its known byte beside it (`CarrierLayout`,
//! contract D3); a known byte of 0 is `Cell::Unknown`.

use piperine_abi::raw::leaf;
use piperine_ir::ValueType;
use piperine_plan::CarrierLayout;
use piperine_solver::prelude::{DcAnalysisResult, TransientAnalysisResult};

use crate::failure::Failure;
use crate::probe::Probe;

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
}
