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
    fn check(
        &self,
        state: VariableState<Atomic>,
        _premises: &[Atomic],
        consequent: Option<&Atomic>,
    ) -> bool {
        let n = self.successors.len();

        let forced_edge = if let Some(consequent) = consequent {
            let mut found = None;

            for (index, successor) in self.successors.iter().enumerate() {
                for value in 1..=n as i32 {
                    if &successor.atomic_not_equal(value) == consequent {
                        found = Some((index, value));
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

        let mut graph = vec![Vec::new(); n];

        for from in 0..n {
            for value in 1..=n as i32 {
                let to = domain_value_to_index(value);

                if to >= n || from == to {
                    continue;
                }

                let edge_allowed = if let Some((forced_from, forced_value)) = forced_edge {
                    if from == forced_from {
                        value == forced_value
                    } else {
                        self.successors[from].induced_domain_contains(&state, value)
                    }
                } else {
                    self.successors[from].induced_domain_contains(&state, value)
                };

                if edge_allowed {
                    graph[from].push(to);
                    graph[to].push(from);
                }
            }
        }

        for neighbours in graph.iter_mut() {
            neighbours.sort_unstable();
            neighbours.dedup();
        }

        has_articulation_point_or_is_disconnected(&graph)
    }
}

fn has_articulation_point_or_is_disconnected(graph: &[Vec<usize>]) -> bool {
    let n = graph.len();

    if n <= 2 {
        return false;
    }

    let mut visited = vec![false; n];
    let mut discovery = vec![0usize; n];
    let mut low = vec![0usize; n];
    let mut parent = vec![None; n];
    let mut time = 0usize;

    fn dfs(
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

        let mut children = 0;

        for &v in &graph[u] {
            if !visited[v] {
                children += 1;
                parent[v] = Some(u);

                if dfs(v, graph, visited, discovery, low, parent, time) {
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

    if dfs(
        0,
        graph,
        &mut visited,
        &mut discovery,
        &mut low,
        &mut parent,
        &mut time,
    ) {
        return true;
    }

    visited.iter().any(|&seen| !seen)
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

        let consequent = neq("x1", 2);

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

        let consequent = neq("x1", 2);

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