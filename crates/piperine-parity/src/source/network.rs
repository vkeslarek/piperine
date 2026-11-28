//! The oracle of the rigs that join nodes by unit branches: Kirchhoff's law solved by Rust.

/// A network of nodes joined by branches of 1 S.
pub struct UnitBranchNetwork;

impl UnitBranchNetwork {
    /// The voltage of each of `count` nodes joined by 1 S branches (`None` is ground) and fed `amps` at node `fed`: Kirchhoff's current
    /// law `G v = i` over the branches, solved by Gaussian elimination.
    pub fn voltages(count: usize, branches: &[(usize, Option<usize>)], fed: usize, amps: f64) -> Vec<f64> {
        let mut g = vec![vec![0.0; count + 1]; count];
        for (from, to) in branches {
            g[*from][*from] += 1.0;
            if let Some(to) = to {
                g[*to][*to] += 1.0;
                g[*from][*to] -= 1.0;
                g[*to][*from] -= 1.0;
            }
        }
        g[fed][count] = amps;
        for pivot in 0..count {
            for row in pivot + 1..count {
                let factor = g[row][pivot] / g[pivot][pivot];
                for column in pivot..=count {
                    g[row][column] -= factor * g[pivot][column];
                }
            }
        }
        let mut v = vec![0.0; count];
        for row in (0..count).rev() {
            let known: f64 = (row + 1..count).map(|column| g[row][column] * v[column]).sum();
            v[row] = (g[row][count] - known) / g[row][row];
        }
        v
    }
}
