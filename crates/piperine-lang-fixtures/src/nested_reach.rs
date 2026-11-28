//! The models of the nested-reach targets : a cell with every kind of item a parent
//! may read (a param, a `let` over the params, a node, a `let` over the solve, a carrier), and the wrappers that put it three and
//! four levels below the reader, by name, through a template and in generation.

/// The cell and its wrappers, every `pub let` of a wrapper one level deeper than the one before.
pub const MODELS: &str = "\
module Cell(a: Electrical) {
    pub k: f64 = 1.0;
    pub let twice = k * 2.0;
    pub signal sn: Electrical;
    pub let vs = v(sn);
    pub signal q: Bit<f64> = 2.5;
    hidden: f64 = 4.0;
}
analog for Cell { i(a, sn) +> v(a, sn) * k; i(sn, gnd) +> v(sn, gnd); }
module Level2(a: Electrical) { pub let c0 = Cell(a); }
module Level3(a: Electrical) { pub let mid = Level2(a); }
module Level4(a: Electrical) { pub let top = Level3(a); }
module Bank(a: Electrical) { cell: Cell; pub let c0 = cell(a); }
module Block(a: Electrical) { cell: Cell; pub let bank = Bank { cell: cell }(a); }
module Wing(a: Electrical) { cell: Cell; pub let block = Block { cell: cell }(a); }
module Row(a: Electrical) { cell: Cell; for j in 0..2 { pub let cells = cell(a); } }
module Grid(a: Electrical) { cell: Cell; pub let row = Row { cell: cell }(a); }
module Plane(a: Electrical) { cell: Cell; pub let grid = Grid { cell: cell }(a); }
module GBank<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let c0 = cell(a); }
module GBlock<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let bank = GBank::<_>(a) { cell: cell }; }
module GWing<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let block = GBlock::<_>(a) { cell: cell }; }
module GRow<C: Fn(Electrical)>(a: Electrical) { cell: C; for j in 0..2 { pub let cells = cell(a); } }
module GGrid<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let row = GRow::<_>(a) { cell: cell }; }
module GPlane<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let grid = GGrid::<_>(a) { cell: cell }; }
module Holder(a: Electrical) { make: Bank; pub let b = make(a); }
module Rows(a: Electrical) { for j in 0..2 { pub let cells = Cell(a) { k: 1.0 + (j as f64) }; } }
module Tower(a: Electrical) { pub let rows = Rows(a); }
module Tall(a: Electrical) { pub let tower = Tower(a); }
";

/// The `let`s a device model mixes a param and the solve in, written in place of the cell's `vs` (`MODELS.replace("pub let vs = v(sn);",
/// MIXED_LETS)`): `vs` again, a param `g` the placements by name leave at its default, and three `let`s over a param and a node.
pub const MIXED_LETS: &str = "pub let vs = v(sn); pub g: f64 = 0.5; pub let deep = k * vs; pub let gated = if k > 0.5 { vs } else { 0.0 }; pub let gi = g * v(sn);";

/// The placement of each wrapper at the top, with the path from it to the cell, and the number of levels below the reader the cell is.
pub const READS: [(&str, &str, &str); 12] = [
    ("let p = Level3(a);", "p.mid.c0", "3 by name"),
    ("let p = Level4(a);", "p.top.mid.c0", "4 by name"),
    ("let p = Block { cell: Cell { } }(a);", "p.bank.c0", "3 through templates"),
    ("let p = Holder { make: Bank { cell: Cell { } } }(a);", "p.b.c0", "3 through a template placed through a template"),
    ("let p = Wing { cell: Cell { } }(a);", "p.block.bank.c0", "4 through templates"),
    ("let p = Grid { cell: Cell { } }(a);", "p.row.cells[1]", "3 through a template in generation"),
    ("let p = Plane { cell: Cell { } }(a);", "p.grid.row.cells[1]", "4 through a template in generation"),
    ("let p = GBlock::<_>(a) { cell: Cell { } };", "p.bank.c0", "3 through generic templates"),
    ("let p = GWing::<_>(a) { cell: Cell { } };", "p.block.bank.c0", "4 through generic templates"),
    ("let p = GPlane::<_>(a) { cell: Cell { } };", "p.grid.row.cells[1]", "4 through generic templates in generation"),
    ("let p = Tower(a);", "p.rows.cells[1]", "3 by name in generation"),
    ("let p = Tall(a);", "p.tower.rows.cells[1]", "4 by name in generation"),
];
