use std::collections::HashMap;
use crate::digital::DigitalNet;
use crate::core::element::Element;

// ---------------------------------------------------------------------------
// DigitalTopology — DAG order + back edges for a fixed set of devices
// ---------------------------------------------------------------------------

pub struct DigitalTopology {
    /// Element indices (into the original `digital_runtimes` vec) in topological order.
    pub topo_order: Vec<usize>,
    /// Back edges as (src_topo_pos, dst_topo_pos) where src > dst.
    /// When the device at src_topo_pos changes its outputs, restart from dst_topo_pos.
    pub back_edges: Vec<(usize, usize)>,
}

impl DigitalTopology {
    pub fn build(devices: &[Box<dyn Element>]) -> Self {
        let n = devices.len();
        if n == 0 {
            return Self { topo_order: Vec::new(), back_edges: Vec::new() };
        }

        // net → element index that produces it. A pure-analog element drives no
        // nets (its `boundary()` is empty), so it never appears here.
        let mut output_to_dev: HashMap<DigitalNet, usize> = HashMap::new();
        for (i, dev) in devices.iter().enumerate() {
            for &net in dev.boundary().outputs {
                output_to_dev.insert(net, i);
            }
        }

        // adj[i] = elements that consume at least one of element i's outputs
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
        for (j, dev) in devices.iter().enumerate() {
            for &net in dev.boundary().inputs {
                if let Some(&i) = output_to_dev.get(&net)
                    && i != j && !adj[i].contains(&j) {
                        adj[i].push(j);
                    }
            }
        }

        // Iterative DFS topo sort with back-edge detection.
        // color: 0=unvisited, 1=on-stack, 2=done
        let mut color = vec![0u8; n];
        let mut topo_rev: Vec<usize> = Vec::with_capacity(n);
        let mut raw_back: Vec<(usize, usize)> = Vec::new(); // (src_dev, dst_dev)

        for start in 0..n {
            if color[start] != 0 { continue; }
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            color[start] = 1;
            while let Some((v, ai)) = stack.last_mut() {
                let v = *v;
                if *ai < adj[v].len() {
                    let u = adj[v][*ai];
                    *ai += 1;
                    match color[u] {
                        0 => { color[u] = 1; stack.push((u, 0)); }
                        1 => { raw_back.push((v, u)); } // back edge
                        _ => {}
                    }
                } else {
                    color[v] = 2;
                    topo_rev.push(v);
                    stack.pop();
                }
            }
        }

        let topo_order: Vec<usize> = topo_rev.into_iter().rev().collect();

        let mut dev_to_pos = vec![0usize; n];
        for (pos, &dev) in topo_order.iter().enumerate() {
            dev_to_pos[dev] = pos;
        }

        let back_edges = raw_back.iter()
            .map(|&(src, dst)| (dev_to_pos[src], dev_to_pos[dst]))
            .collect();

        Self { topo_order, back_edges }
    }
}
