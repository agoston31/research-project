use fixedbitset::FixedBitSet;
use pumpkin_checking::AtomicConstraint;
use pumpkin_checking::CheckerVariable;
use pumpkin_checking::InferenceChecker;
use pumpkin_checking::VariableState;

#[derive(Debug, Clone)]
pub struct CircuitChecker<Var> {
    pub successors: Box<[Var]>,
}

#[derive(Debug, Clone)]
pub struct CircuitArticulationChecker<Var> {
    pub successors: Box<[Var]>,
}

impl<Var, Atomic> InferenceChecker<Atomic> for CircuitChecker<Var>
where
    Var: CheckerVariable<Atomic>,
    Atomic: AtomicConstraint,
{
    // Validates a subcycle conflict explanation
    // The checker follows fixed successor assignments. If these assignments form a cycle
    // that does not include all nodes, the explanation is accepted as a valid conflict
    fn check(
        &self,
        state: VariableState<Atomic>,
        _premises: &[Atomic],
        _consequent: Option<&Atomic>,
    ) -> bool {
        // Try all the successors as possible starting points
        for successor in self.successors.iter() {
            // Skip if successor is not yet fixed
            let Some(next_node) = successor.induced_fixed_value(&state) else {
                continue;
            };

            // circuit is 1-indexed
            let mut next_idx = usize::try_from(next_node).unwrap() - 1;

            let mut visited = FixedBitSet::with_capacity(self.successors.len());

            loop {
                if visited.contains(next_idx) {
                    if visited.count_ones(..) < self.successors.len() {
                        return true; // proper subcycle = conflict
                    } else {
                        return false; // full Hamiltonian cycle = no conflict
                    }
                }

                visited.insert(next_idx);

                // Move on to the next node if there is one
                let Some(next_node) = self.successors[next_idx].induced_fixed_value(&state) else {
                    break;
                };

                next_idx = usize::try_from(next_node).unwrap() - 1;
            }
        }

        false
    }
}

impl<Var, Atomic> InferenceChecker<Atomic> for CircuitArticulationChecker<Var>
where
    Var: CheckerVariable<Atomic> + 'static,
    Atomic: AtomicConstraint + PartialEq,
{   
    // Validates an articulation-based conflict or pruning explanation.
    // The premises describe removed edges. If a consequent is present, the
    // corresponding forced edge is also temporarily removed. The explanation is valid 
    // if the resulting support graph is disconnected or contains an articulation point
    fn check(
        &self,
        _state: VariableState<Atomic>,
        premises: &[Atomic],
        consequent: Option<&Atomic>,
    ) -> bool {
        let n = self.successors.len();

        let forced_edge = if let Some(consequent) = consequent {
            let mut found = None;

            for (from, successor) in self.successors.iter().enumerate() {
                for value in 1..=n as i32 {
                    if &successor.atomic_equal(value) == consequent {
                        let to = domain_value_to_index(value);
                        found = Some((from, to));
                        break;
                    }
                }

                if found.is_some() {
                    break;
                }
            }

            found
        } else {
            None
        };

        let mut directed_allowed = vec![vec![false; n]; n];

        for from in 0..n {
            for value in 1..=n as i32 {
                let to = domain_value_to_index(value);

                if to >= n || from == to {
                    continue;
                }

                let removed_by_premises = premises.iter().any(|premise| {
                    premise == &self.successors[from].atomic_not_equal(value)
                });

                directed_allowed[from][to] = !removed_by_premises;
            }
        }

        if let Some((from, to)) = forced_edge {
            directed_allowed[from][to] = false;
        }

        let mut graph = vec![Vec::new(); n];

        for u in 0..n {
            for v in (u + 1)..n {
                if directed_allowed[u][v] || directed_allowed[v][u] {
                    graph[u].push(v);
                    graph[v].push(u);
                }
            }
        }

        is_disconnected(&graph) || has_articulation_point(&graph)
    }
}

// Returns whether the undirected support graph is disconnected
fn is_disconnected(graph: &[Vec<usize>]) -> bool {
    let n = graph.len();

    if n <= 1 {
        return false;
    }

    let mut visited = vec![false; n];
    dfs_mark(0, graph, &mut visited);

    visited.iter().any(|&seen| !seen)
}

// Marks all vertices reachable from the given start vertex using DFS
fn dfs_mark(u: usize, graph: &[Vec<usize>], visited: &mut [bool]) {
    visited[u] = true;

    for &v in &graph[u] {
        if !visited[v] {
            dfs_mark(v, graph, visited);
        }
    }
}

// Returns whether the undirected support graph contains an articulation point
fn has_articulation_point(graph: &[Vec<usize>]) -> bool {
    let n = graph.len();

    if n <= 2 {
        return false;
    }

    let mut visited = vec![false; n];
    let mut discovery = vec![0usize; n];
    let mut low = vec![0usize; n];
    let mut parent = vec![None; n];
    let mut time = 0usize;

    for u in 0..n {
        if !visited[u]
            && articulation_dfs(
                u,
                graph,
                &mut visited,
                &mut discovery,
                &mut low,
                &mut parent,
                &mut time,
            )
        {
            return true;
        }
    }

    false
}

// Performs Tarjan-style DFS for articulation point detection
fn articulation_dfs(
    u: usize,
    graph: &[Vec<usize>],
    visited: &mut [bool],
    discovery: &mut [usize],
    low: &mut [usize],
    parent: &mut [Option<usize>],
    time: &mut usize,
) -> bool {
    visited[u] = true;
    *time += 1;
    discovery[u] = *time;
    low[u] = *time;

    let mut children = 0usize;

    for &v in &graph[u] {
        if !visited[v] {
            children += 1;
            parent[v] = Some(u);

            if articulation_dfs(v, graph, visited, discovery, low, parent, time) {
                return true;
            }

            low[u] = low[u].min(low[v]);

            if parent[u].is_none() && children > 1 {
                return true;
            }

            if parent[u].is_some() && low[v] >= discovery[u] {
                return true;
            }
        } else if parent[u] != Some(v) {
            low[u] = low[u].min(discovery[v]);
        }
    }

    false
}

const VALUE_OFFSET: usize = 1;

#[inline]
fn domain_value_to_index(domain_value: i32) -> usize {
    domain_value as usize - VALUE_OFFSET
}

// Tests

#[cfg(test)]
mod tests {
    use pumpkin_checking::TestAtomic;
    use pumpkin_checking::VariableState;

    use super::*;

    fn eq(name: &'static str, value: i32) -> TestAtomic {
        TestAtomic {
            name,
            comparison: pumpkin_checking::Comparison::Equal,
            value,
        }
    }

    #[test]
    fn conflict_simple_subcycle() {
        // 1 -> 2 -> 1, node 3 outside
        let premises = [eq("x1", 2), eq("x2", 1)];

        let state = VariableState::prepare_for_conflict_check(premises, None)
            .expect("no conflicting atomics");

        let checker = CircuitChecker {
            successors: vec!["x1", "x2", "x3"].into(),
        };

        assert!(checker.check(state, &premises, None));
    }

    #[test]
    fn no_conflict_full_hamiltonian_cycle() {
        // 1 -> 2 -> 3 -> 1
        let premises = [eq("x1", 2), eq("x2", 3), eq("x3", 1)];

        let state = VariableState::prepare_for_conflict_check(premises, None)
            .expect("no conflicting atomics");

        let checker = CircuitChecker {
            successors: vec!["x1", "x2", "x3"].into(),
        };

        assert!(!checker.check(state, &premises, None));
    }

    #[test]
    fn no_conflict_incomplete_path() {
        // 1 -> 2 -> 3, but x3 is not fixed
        let premises = [eq("x1", 2), eq("x2", 3)];

        let state = VariableState::prepare_for_conflict_check(premises, None)
            .expect("no conflicting atomics");

        let checker = CircuitChecker {
            successors: vec!["x1", "x2", "x3"].into(),
        };

        assert!(!checker.check(state, &premises, None));
    }

    #[test]
    fn conflict_fixed_self_loop() {
        // 1 -> 1, with more than one node
        let premises = [eq("x1", 1)];

        let state = VariableState::prepare_for_conflict_check(premises, None)
            .expect("no conflicting atomics");

        let checker = CircuitChecker {
            successors: vec!["x1", "x2", "x3"].into(),
        };

        assert!(checker.check(state, &premises, None));
    }

    #[test]
    fn no_conflict_two_variable_cycle() {
        // 1 -> 2 -> 1
        let premises = [eq("x1", 2), eq("x2", 1)];

        let state = VariableState::prepare_for_conflict_check(premises, None)
            .expect("no conflicting atomics");

        let checker = CircuitChecker {
            successors: vec!["x1", "x2"].into(),
        };

        assert!(!checker.check(state, &premises, None));
    }

    #[test]
    fn conflict_three_node_subcycle_with_four_nodes() {
        // 1 -> 2 -> 3 -> 1, node 4 outside
        let premises = [
            eq("x1", 2),
            eq("x2", 3),
            eq("x3", 1),
        ];

        let state = VariableState::prepare_for_conflict_check(premises, None)
            .expect("no conflicting atomics");

        let checker = CircuitChecker {
            successors: vec!["x1", "x2", "x3", "x4"].into(),
        };

        assert!(checker.check(state, &premises, None));
    }

    fn neq(name: &'static str, value: i32) -> TestAtomic {
        TestAtomic {
            name,
            comparison: pumpkin_checking::Comparison::NotEqual,
            value,
        }
    }

    #[test]
    fn articulation_checker_detects_disconnected_graph() {
        let premises = [
            // Separate {1,2} from {3,4}
            neq("x1", 3),
            neq("x1", 4),
            neq("x2", 3),
            neq("x2", 4),
            neq("x3", 1),
            neq("x3", 2),
            neq("x4", 1),
            neq("x4", 2),
        ];

        let state = VariableState::prepare_for_conflict_check(premises.clone(), None)
            .expect("no conflicting atomics");

        let checker = CircuitArticulationChecker {
            successors: vec!["x1", "x2", "x3", "x4"].into(),
        };

        assert!(checker.check(state, &premises, None));
    }

    #[test]
    fn articulation_checker_rejects_connected_non_articulation_conflict() {
        let premises = vec![];

        let state = VariableState::prepare_for_conflict_check(premises.clone(), None)
            .expect("premises should be consistent");

        let checker = CircuitArticulationChecker {
            successors: vec!["x1", "x2", "x3", "x4"].into(),
        };

        assert!(!checker.check(state, &premises, None));
    }

    #[test]
    fn articulation_checker_accepts_pruning_explanation() {
        let premises = vec![
            neq("x1", 1), neq("x1", 3), neq("x1", 4),

            neq("x2", 2), neq("x2", 3), neq("x2", 4), neq("x2", 5),

            neq("x3", 1), neq("x3", 3), neq("x3", 4), neq("x3", 5),

            neq("x4", 1), neq("x4", 2), neq("x4", 4), neq("x4", 5),

            neq("x5", 1), neq("x5", 2), neq("x5", 3), neq("x5", 5),
        ];

        let consequent = eq("x1", 2);

        let state = VariableState::prepare_for_conflict_check(
            premises.clone(),
            Some(consequent.clone()),
        )
        .expect("premises and consequent should be consistent");

        let checker = CircuitArticulationChecker {
            successors: vec!["x1", "x2", "x3", "x4", "x5"].into(),
        };

        assert!(checker.check(state, &premises, Some(&consequent)));
    }

    #[test]
    fn articulation_checker_rejects_bad_pruning_explanation() {
        let premises = vec![
            neq("x1", 1),
            neq("x2", 2),
            neq("x3", 3),
            neq("x4", 4),
        ];

        let consequent = eq("x1", 2);

        let state = VariableState::prepare_for_conflict_check(
            premises.clone(),
            Some(consequent.clone()),
        )
        .expect("premises and consequent should be consistent");

        let checker = CircuitArticulationChecker {
            successors: vec!["x1", "x2", "x3", "x4"].into(),
        };

        assert!(!checker.check(state, &premises, Some(&consequent)));
    }
}