//! The levelized order of the discrete functions (abi.md §9 D1): every
//! function after the writers of what it reads, a strongly connected group of
//! functions that passes through a `RELAX` net as one relaxation step, any
//! other cycle refused.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::net::{NetId, NetSpec};
use crate::plan::FnNode;
use crate::error::DigitalError;

/// One step of the order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A function with no cycle through it: index into `Dag::function`.
    Run(usize),
    /// A cycle through `RELAX` nets: its functions in (instance, function)
    /// order and the `RELAX` nets its edges pass through.
    Relax { fns: Vec<usize>, nets: Vec<NetId> },
}

#[derive(Clone, Debug)]
pub struct Dag {
    functions: Vec<FnNode>,
    order: Vec<Step>,
    readers: Vec<Vec<usize>>,
}

impl Dag {
    /// Orders `functions`; the result does not depend on the order they are given in.
    pub fn build(functions: &[FnNode], nets: &[NetSpec]) -> Result<Dag, DigitalError> {
        // a stable numbering: (instance, function), ties by input position
        let mut functions: Vec<FnNode> = functions.to_vec();
        functions.sort_by_key(|f| (f.instance, f.k));

        let mut readers: Vec<Vec<usize>> = vec![Vec::new(); nets.len()];
        for (i, f) in functions.iter().enumerate() {
            for r in &f.reads {
                readers[r.0 as usize].push(i);
            }
        }
        // an edge goes from a function to every function that reads a part of a net it writes: elements of one array
        // net that one function writes and another reads are an edge only when they share an element
        let edges: Vec<Vec<usize>> = functions.iter().map(|f| Self::successors(&functions, &readers, f, |_| true)).collect();

        // abi.md §9 D1: every directed cycle passes through a RELAX net. Take the edges through
        // RELAX nets away; what remains must have no cycle.
        let strict: Vec<Vec<usize>> = functions.iter().map(|f| Self::successors(&functions, &readers, f, |net| !nets[net.0 as usize].relax)).collect();
        for members in components(&strict) {
            if members.len() > 1 || strict[members[0]].contains(&members[0]) {
                let what = members.iter().map(|m| functions[*m].name.clone()).collect();
                return Err(DigitalError::Bind { rule: "cycle", what });
            }
        }

        let comps = components(&edges);
        let mut comp_of = vec![0usize; functions.len()];
        for (c, members) in comps.iter().enumerate() {
            for m in members {
                comp_of[*m] = c;
            }
        }

        // a group of several functions, or one that feeds itself, is a cycle
        let mut steps: Vec<Option<Step>> = Vec::with_capacity(comps.len());
        for members in &comps {
            let cyclic = members.len() > 1 || edges[members[0]].contains(&members[0]);
            if !cyclic {
                steps.push(Some(Step::Run(members[0])));
                continue;
            }
            let mut relaxed: Vec<NetId> = Vec::new();
            let mut internal = false;
            for m in members {
                for w in &functions[*m].writes {
                    if readers[w.0 as usize].iter().any(|r| members.contains(r)) {
                        internal = true;
                        if nets[w.0 as usize].relax && !relaxed.contains(w) {
                            relaxed.push(*w);
                        }
                    }
                }
            }
            debug_assert!(internal);
            if relaxed.is_empty() {
                let what = members.iter().map(|m| functions[*m].name.clone()).collect();
                return Err(DigitalError::Bind { rule: "cycle", what });
            }
            relaxed.sort();
            steps.push(Some(Step::Relax { fns: members.clone(), nets: relaxed }));
        }

        // topological order of the groups, ties broken by the smallest member
        let mut indegree = vec![0usize; comps.len()];
        let mut succ: Vec<Vec<usize>> = vec![Vec::new(); comps.len()];
        for (f, to) in edges.iter().enumerate() {
            for t in to {
                let (a, b) = (comp_of[f], comp_of[*t]);
                if a != b && !succ[a].contains(&b) {
                    succ[a].push(b);
                    indegree[b] += 1;
                }
            }
        }
        let mut ready: BinaryHeap<Reverse<(usize, usize)>> =
            (0..comps.len()).filter(|c| indegree[*c] == 0).map(|c| Reverse((comps[c][0], c))).collect();
        let mut order = Vec::with_capacity(comps.len());
        while let Some(Reverse((_, c))) = ready.pop() {
            order.push(steps[c].take().expect("each group is emitted once"));
            for s in &succ[c] {
                indegree[*s] -= 1;
                if indegree[*s] == 0 {
                    ready.push(Reverse((comps[*s][0], *s)));
                }
            }
        }
        Ok(Dag { functions, order, readers })
    }

    /// The functions that read a part of a net `f` writes, through the nets `through` admits, in increasing order.
    fn successors(functions: &[FnNode], readers: &[Vec<usize>], f: &FnNode, through: impl Fn(NetId) -> bool) -> Vec<usize> {
        let mut to: Vec<usize> = f
            .write_parts
            .iter()
            .filter(|written| through(written.net))
            .flat_map(|written| readers[written.net.0 as usize].iter().copied().filter(move |r| functions[*r].read_parts.iter().any(|read| read.overlaps(written))))
            .collect();
        to.sort_unstable();
        to.dedup();
        to
    }

    /// The steps in the order they run.
    pub fn order(&self) -> &[Step] {
        &self.order
    }

    /// The number of functions.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// The function behind an index of `Step`.
    pub fn function(&self, i: usize) -> &FnNode {
        &self.functions[i]
    }

    /// The functions that read `net`, in order.
    pub fn readers(&self, net: NetId) -> &[usize] {
        self.readers.get(net.0 as usize).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Tarjan's strongly connected components, iterative. Each component lists its
/// members in increasing order.
fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = edges.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut comps: Vec<Vec<usize>> = Vec::new();
    let mut next = 0usize;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        // (node, position of the next edge to follow)
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&(v, e)) = call.last() {
            if let Some(&w) = edges[v].get(e) {
                call.last_mut().expect("non-empty").1 += 1;
                if index[w] == usize::MAX {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                call.pop();
                if let Some(&(parent, _)) = call.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut members = Vec::new();
                    loop {
                        let w = stack.pop().expect("v is on the stack");
                        on_stack[w] = false;
                        members.push(w);
                        if w == v {
                            break;
                        }
                    }
                    members.sort_unstable();
                    comps.push(members);
                }
            }
        }
    }
    comps
}
