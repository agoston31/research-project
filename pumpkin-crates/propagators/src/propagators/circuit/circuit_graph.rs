//! Shared directed-graph algorithms used by both `CircuitPropagator` and
//! `CircuitStrongArticulationChecker`.
//!
//! All functions operate on directed adjacency lists: `Vec<Vec<usize>>` where
//! index `i` holds the list of nodes that `i` has an outgoing edge to.

pub struct SccResult {
    // Maps each vertex to the id of its SCC (0-indexed in discovery order)
    pub vertex_to_scc: Vec<usize>,
    // Number of vertices in each SCC, indexed by SCC id
    pub scc_sizes: Vec<usize>,
    pub num_sccs: usize,
}

pub struct CondensationDag {
    pub outgoing: Vec<Vec<usize>>,
    pub incoming: Vec<Vec<usize>>,
}

impl CondensationDag {
    pub fn source_sccs(&self) -> Vec<usize> {
        self.incoming
            .iter()
            .enumerate()
            .filter_map(|(scc, inc)| inc.is_empty().then_some(scc))
            .collect()
    }

    pub fn sink_sccs(&self) -> Vec<usize> {
        self.outgoing
            .iter()
            .enumerate()
            .filter_map(|(scc, out)| out.is_empty().then_some(scc))
            .collect()
    }

    fn deduplicate_edges(&mut self) {
        for edges in self.outgoing.iter_mut().chain(self.incoming.iter_mut()) {
            edges.sort_unstable();
            edges.dedup();
        }
    }
}

// Returns the number of SCCs in `graph`, ignoring `removed_vertex` if given
pub fn count_sccs_without_vertex(graph: &[Vec<usize>], removed_vertex: Option<usize>) -> usize {
    tarjan_count(graph, removed_vertex)
}

// Computes the full SCC membership for every vertex, with `removed` absent
pub fn compute_sccs_without_vertex(graph: &[Vec<usize>], removed: usize) -> SccResult {
    tarjan_membership(graph, removed)
}

// Builds the condensation DAG of `graph` minus `removed_vertex`, using the
// precomputed SCC assignment in `scc_result`
pub fn build_condensation_dag(
    graph: &[Vec<usize>],
    removed_vertex: usize,
    scc_result: &SccResult,
) -> CondensationDag {
    let mut dag = CondensationDag {
        outgoing: vec![Vec::new(); scc_result.num_sccs],
        incoming: vec![Vec::new(); scc_result.num_sccs],
    };

    for from in 0..graph.len() {
        if from == removed_vertex {
            continue;
        }
        for &to in &graph[from] {
            if to == removed_vertex {
                continue;
            }
            let from_scc = scc_result.vertex_to_scc[from];
            let to_scc = scc_result.vertex_to_scc[to];
            if from_scc != to_scc {
                dag.outgoing[from_scc].push(to_scc);
                dag.incoming[to_scc].push(from_scc);
            }
        }
    }

    dag.deduplicate_edges();
    dag
}

// Returns the longest path weight through the DAG, where each SCC node is
// weighted by the number of original vertices it contains
pub fn longest_weighted_path_in_dag(dag: &CondensationDag, weights: &[usize]) -> usize {
    let order = topological_order(dag);
    let mut dp = weights.to_vec();

    for &u in &order {
        for &v in &dag.outgoing[u] {
            dp[v] = dp[v].max(dp[u] + weights[v]);
        }
    }

    dp.into_iter().max().unwrap_or(0)
}

// Returns all strong articulation points of `graph`: vertices whose removal
// increases the number of SCCs
pub fn find_strong_articulation_points(graph: &[Vec<usize>]) -> Vec<usize> {
    let n = graph.len();

    if n <= 2 {
        return Vec::new();
    }

    let baseline = tarjan_count(graph, None);

    (0..n)
        .filter(|&v| tarjan_count(graph, Some(v)) > baseline)
        .collect()
}

// Returns `true` if either the DAG has more than one source or sink, 
// or its longest weighted path covers fewer than `n - 1` vertices
pub fn dag_is_infeasible(graph: &[Vec<usize>], articulation: usize, n: usize) -> bool {
    let scc_result = tarjan_membership(graph, articulation);

    // If only one SCC remains the articulation didn't split anything
    if scc_result.num_sccs <= 1 {
        return false;
    }

    let dag = build_condensation_dag(graph, articulation, &scc_result);

    if dag.source_sccs().len() != 1 || dag.sink_sccs().len() != 1 {
        return true;
    }

    longest_weighted_path_in_dag(&dag, &scc_result.scc_sizes) < n - 1
}

// Counts SCCs using Tarjan's algorithm
fn tarjan_count(graph: &[Vec<usize>], removed: Option<usize>) -> usize {
    let n = graph.len();
    let mut index_counter = 0usize;
    let mut tarjan_stack = Vec::new();
    let mut on_stack = vec![false; n];
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut lowlink = vec![0usize; n];
    let mut scc_count = 0usize;

    for root in 0..n {
        if Some(root) == removed || indices[root].is_some() {
            continue;
        }

        // work_stack entries: (vertex, index into its neighbour list)
        let mut work_stack: Vec<(usize, usize)> = Vec::new();

        indices[root] = Some(index_counter);
        lowlink[root] = index_counter;
        index_counter += 1;
        tarjan_stack.push(root);
        on_stack[root] = true;
        work_stack.push((root, 0));

        'outer: while let Some((u, ni)) = work_stack.last_mut() {
            let u = *u;
            while *ni < graph[u].len() {
                let v = graph[u][*ni];
                *ni += 1;

                if Some(v) == removed {
                    continue;
                }

                if indices[v].is_none() {
                    // Tree edge: initialise and recurse
                    indices[v] = Some(index_counter);
                    lowlink[v] = index_counter;
                    index_counter += 1;
                    tarjan_stack.push(v);
                    on_stack[v] = true;
                    work_stack.push((v, 0));
                    continue 'outer;
                } else if on_stack[v] {
                    lowlink[u] = lowlink[u].min(indices[v].unwrap());
                }
            }

            // All neighbours of u processed
            work_stack.pop();

            // Propagate lowlink to parent
            if let Some(&(parent, _)) = work_stack.last() {
                lowlink[parent] = lowlink[parent].min(lowlink[u]);
            }

            // SCC root: pop everything down to u from the Tarjan stack
            if lowlink[u] == indices[u].unwrap() {
                loop {
                    let w = tarjan_stack.pop().unwrap();
                    on_stack[w] = false;
                    if w == u {
                        break;
                    }
                }
                scc_count += 1;
            }
        }
    }

    scc_count
}

// Similar to `tarjan_count` but also records which SCC each vertex belongs to and
// the size of each SCC
fn tarjan_membership(graph: &[Vec<usize>], removed: usize) -> SccResult {
    let n = graph.len();
    let mut index_counter = 0usize;
    let mut tarjan_stack: Vec<usize> = Vec::new();
    let mut on_stack = vec![false; n];
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut lowlink = vec![0usize; n];
    let mut vertex_to_scc = vec![usize::MAX; n];
    let mut scc_sizes: Vec<usize> = Vec::new();

    for root in 0..n {
        if root == removed || indices[root].is_some() {
            continue;
        }

        let mut work_stack: Vec<(usize, usize)> = Vec::new();

        indices[root] = Some(index_counter);
        lowlink[root] = index_counter;
        index_counter += 1;
        tarjan_stack.push(root);
        on_stack[root] = true;
        work_stack.push((root, 0));

        'outer: while let Some((u, ni)) = work_stack.last_mut() {
            let u = *u;
            while *ni < graph[u].len() {
                let v = graph[u][*ni];
                *ni += 1;

                if v == removed {
                    continue;
                }

                if indices[v].is_none() {
                    indices[v] = Some(index_counter);
                    lowlink[v] = index_counter;
                    index_counter += 1;
                    tarjan_stack.push(v);
                    on_stack[v] = true;
                    work_stack.push((v, 0));
                    continue 'outer;
                } else if on_stack[v] {
                    lowlink[u] = lowlink[u].min(indices[v].unwrap());
                }
            }

            work_stack.pop();

            if let Some(&(parent, _)) = work_stack.last() {
                lowlink[parent] = lowlink[parent].min(lowlink[u]);
            }

            if lowlink[u] == indices[u].unwrap() {
                let scc_id = scc_sizes.len();
                let mut size = 0usize;

                loop {
                    let w = tarjan_stack.pop().unwrap();
                    on_stack[w] = false;
                    vertex_to_scc[w] = scc_id;
                    size += 1;
                    if w == u {
                        break;
                    }
                }

                scc_sizes.push(size);
            }
        }
    }

    SccResult {
        num_sccs: scc_sizes.len(),
        vertex_to_scc,
        scc_sizes,
    }
}

// Topological sort
fn topological_order(dag: &CondensationDag) -> Vec<usize> {
    let n = dag.outgoing.len();
    let mut indegree = vec![0usize; n];

    for u in 0..n {
        for &v in &dag.outgoing[u] {
            indegree[v] += 1;
        }
    }

    let mut queue: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, &d)| (d == 0).then_some(i))
        .collect();

    let mut order = Vec::with_capacity(n);

    while let Some(u) = queue.pop() {
        order.push(u);
        for &v in &dag.outgoing[u] {
            indegree[v] -= 1;
            if indegree[v] == 0 {
                queue.push(v);
            }
        }
    }

    order
}