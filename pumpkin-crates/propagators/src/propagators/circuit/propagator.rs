use fixedbitset::FixedBitSet;
use pumpkin_core::conjunction;
use pumpkin_core::declare_inference_label;
use pumpkin_core::predicate;
use pumpkin_core::predicates::PropositionalConjunction;
use pumpkin_core::proof::ConstraintTag;
use pumpkin_core::proof::InferenceCode;
use pumpkin_core::propagation::DomainEvents;
use pumpkin_core::propagation::Domains;
use pumpkin_core::propagation::LocalId;
use pumpkin_core::propagation::PropagationContext;
use pumpkin_core::propagation::Propagator;
use pumpkin_core::propagation::PropagatorConstructor;
use pumpkin_core::propagation::ReadDomains;
use pumpkin_core::state::Conflict;
use pumpkin_core::state::PropagationStatusCP;
use pumpkin_core::state::PropagatorConflict;
use pumpkin_core::variables::IntegerVariable;
use pumpkin_core::propagation::InferenceCheckers;
use pumpkin_core::create_statistics_struct;
use pumpkin_core::statistics::Statistic;

use crate::circuit::{CircuitChecker, CircuitArticulationChecker, CircuitStrongArticulationChecker};


// ADD STATISTIC
create_statistics_struct!(ArticulationStatistics {
    // Total number of articulation based conflict
    num_strong_articulation_points: u32,
    // Total number of articulation based edge pruning
    num_articulation_prunings: u32,
    // Total number of strong articulation basd conflicts
    num_strong_articulation_conflicts: u32,
});

// constructor for the propagator. ConstraintTag is for proof logging
#[derive(Debug, Clone)]
pub struct CircuitConstructor<Var> {
    pub successors: Box<[Var]>,
    pub constraint_tag: ConstraintTag,
}

// Propagator struct. Contains propagator info and inference code (latter for explanations)
#[derive(Debug, Clone)]
pub struct CircuitPropagator<Var> {
    pub successors: Box<[Var]>,
    prevent_inference_code: InferenceCode,
    articulation_inference_code: InferenceCode,
    strong_articulation_inference_code: InferenceCode,
    statistics: ArticulationStatistics,
}

// The whole propagator constructor itself
impl<Var> PropagatorConstructor 
    for CircuitConstructor<Var> 
where 
    // define the type of the variable
    Var : IntegerVariable + 'static, 
{
    type PropagatorImpl = CircuitPropagator<Var>; //associated type; specifies this constructor produces a CircuitConstructor when the solver instantiates it.

    // Creates and initializes the circuit propagator
    fn create(
        self,
        mut context: pumpkin_core::propagation::PropagatorConstructorContext,
    ) -> Self::PropagatorImpl {
        // registering for domain events; when should our propagator be enqueued. The flag DomainEvents: REMOVE determines that it should be queued on 'removal' of any value of the domain. 
        // LocalId is to internally indicate to what variable changes occur; unique for each var
        self.successors
            .iter()
            .enumerate()
            .for_each(|(index, successor)| {
                context.register(
                    successor.clone(),
                    DomainEvents::ANY_INT,
                    LocalId::from(index as u32),
                );
                context.register_backtrack(
                    successor.clone(),
                    DomainEvents::ANY_INT,
                    LocalId::from(index as u32),
                );
            });

        // create the actual propagator and generate new inference code
        CircuitPropagator {
            // set variables to base values
            successors: self.successors,
            prevent_inference_code: InferenceCode::new(self.constraint_tag, CircuitPrevent),
            articulation_inference_code: InferenceCode::new(self.constraint_tag, CircuitArticulation),
            strong_articulation_inference_code: InferenceCode::new(self.constraint_tag, CircuitStrongArticulation),
            statistics: ArticulationStatistics::default(),
        }
    }
    
    // Registers explanation checkers for all inference types produced by this propagator
    fn add_inference_checkers(&self, mut checkers: InferenceCheckers<'_>) {
        checkers.add_inference_checker(
            InferenceCode::new(self.constraint_tag, CircuitPrevent),
            Box::new(CircuitChecker {
                successors: self.successors.clone(),
            }),
        );

        checkers.add_inference_checker(
            InferenceCode::new(self.constraint_tag, CircuitArticulation),
            Box::new(CircuitArticulationChecker {
                successors: self.successors.clone(),
            }),
        );

        checkers.add_inference_checker(
            InferenceCode::new(self.constraint_tag, CircuitStrongArticulation),
            Box::new(CircuitStrongArticulationChecker {
                successors: self.successors.clone(),
            }),
        );

    }
}

declare_inference_label!(CircuitPrevent);
declare_inference_label!(CircuitArticulation);
declare_inference_label!(CircuitStrongArticulation);


// Implementation of Propagator which has some basic functions (like defining the name) but also important functions propagate() and propagate_from_scratch()
impl<Var: IntegerVariable + 'static> Propagator for CircuitPropagator<Var> {
    fn name(&self) -> &str {
    "Circuit"
    }

    // Executes the full propagation routine
    fn propagate_from_scratch(&self, mut context: PropagationContext) -> PropagationStatusCP {
        self.remove_self_loops(&mut context)?;
        self.check(context.domains())?;
        self.prevent(&mut context)?;
        self.articulation_prune(&mut context)

        // Dummy variables
        let mut num_saps = 0;
        let mut num_prunings = 0;
        let mut num_conflicts = 0;
        self.propagate_strong_articulation_pruning(
            &mut context, 
            &mut num_saps, 
            &mut num_prunings,
            &mut num_conflicts,
        )
    }

    // Copy of propagate_from_scratch
    // With statistics
    fn propagate(&mut self, mut context: PropagationContext) -> PropagationStatusCP {
        self.remove_self_loops(&mut context)?;
        self.check(context.domains())?;
        self.prevent(&mut context)?;
        self.articulation_prune(&mut context)
        
        let mut num_saps = 0;
        let mut num_prunings = 0;
        let mut num_conflicts = 0;

        let result = self.propagate_strong_articulation_pruning(
            &mut context,
            &mut num_saps,
            &mut num_prunings,
            &mut num_conflicts,
        );

        self.statistics.num_strong_articulation_points += num_saps;
        self.statistics.num_articulation_prunings += num_prunings;
        self.statistics.num_strong_articulation_conflicts += num_conflicts;

        result

    }

    fn log_statistics(&self, statistic_logger: pumpkin_core::statistics::StatisticLogger) {
        self.statistics.log(statistic_logger);
    }
}

impl<Var: IntegerVariable + 'static> CircuitPropagator<Var> {
    // Removes all self-loop assignments
    // In a Hamiltonian circuit a node cannot point to itself, therefore
    // value i is removed from the domain of variable i
    fn remove_self_loops(&self, context: &mut PropagationContext) -> PropagationStatusCP {
        for (zero_indexed_node, domain_of_one_indexed_node) in self.successors.iter().enumerate() {
            context.post(
                predicate!(domain_of_one_indexed_node != index_to_domain_value(zero_indexed_node)),
                conjunction!(),
                &self.prevent_inference_code,
            )?;
        }
        Ok(())
    }
}

impl<Var: IntegerVariable + 'static> CircuitPropagator<Var> {
    // Prevents the formation of premature cycles
    // Searches for chains of fixed successor assignments and removes edges
    // that would close a cycle before all nodes have been visited
    fn prevent(&self, context: &mut PropagationContext) -> PropagationStatusCP {
        // collect all nodes that have an incoming enforced/fixed edge, these cannot be start of possible chains
        let mut has_incoming_edge = FixedBitSet::with_capacity(self.successors.len());
        // for every fixed edge we find, we follow it and add the resulting node to the list
        for successor in self.successors.iter() {
            if let Some(fixed_value) = context.fixed_value(successor) {
                has_incoming_edge.insert(domain_value_to_index(fixed_value));
            }
        }



        // For every node that has no fixed incoming edge, we try to create a chain.
        for unmarked in has_incoming_edge.zeroes() {
            // If the node has no fixed outgoing edge, we cannot create a chain and go to the next possible node to start a chain.
            let Some(fixed_value) = context.fixed_value(&self.successors[unmarked]) else {
                continue;
            };

            // If it does have an outgoing fixed edge, we can start creating a chain with our starting node.
            let mut chain = vec![unmarked];

            // Now we keep up extending our chain as long as we reach nodes that have a fixed outgoing edge.
            // We already know the upcoming node as we checked if the first node had a fixed outgoing edge;
            let mut next = domain_value_to_index(fixed_value);
            // Set to keep track which nodes have we already visited
            let mut seen = FixedBitSet::with_capacity(self.successors.len());
            seen.insert(unmarked);
            // And then we keep on looping until we end up in a node with no fixed outgoing edge.
            while let Some(fixed_value_next) = context.fixed_value(&self.successors[next]) {
                // If we arrive at a node that we have seen before we break the loop
                if seen.contains(next) {
                    break;
                }
                seen.insert(next);
                // We add the next value to the chain
                chain.push(next);
                // And continue to unfold the chain from there. As the domains themselves are 1-indexed, we need to transform them to 0-indexed for our own array.
                next = domain_value_to_index(fixed_value_next);
            }

            // We have found a chain. If the last node in the chain has a possible edge to the starting node, we prune that edge only if
            // the length of the chain is not n: if we have not visited all nodes yet we cannot return to the starting node already.
            if context.fixed_value(&self.successors[next]).is_none() 
                    && context.contains(&self.successors[next], index_to_domain_value(unmarked)) 
                    && chain.len() + 1 < self.successors.len() 
            {
                let reason = self.create_prevent_explanation(context.domains(), &chain);
                context.post(
                    predicate!(self.successors[next] != index_to_domain_value(unmarked)),
                    reason,
                    &self.prevent_inference_code,
                )?;
            }
        }

        Ok(())
    }


    // Creates an explanation for a prevent-subcycle pruning
    // The reason consists of all fixed assignments forming the detected chain
    fn create_prevent_explanation(
        &self,
        context: Domains,
        path: &[usize],
    ) -> PropositionalConjunction {
        path.iter()
            .map(|&index| {
                let var = &self.successors[index];

                predicate!(
                    var == context
                        .fixed_value(var)
                        .expect("Expected every variable in the chain to be assigned")
                )
            })
            .collect()
    }
}    

impl<Var: IntegerVariable + 'static> CircuitPropagator<Var> {
    // Detects already formed invalid cycles
    // Follows all fixed successor assignments and reports a conflict whenever a cycle
    // is encountered that is not a Hamiltonian cycle containing all nodes exactly once
    fn check(&self, context: Domains) -> PropagationStatusCP {
        let n = self.successors.len();

        for start in 0..n {
            let mut visited = FixedBitSet::with_capacity(n);
            let mut cycle_path = Vec::new();

            let mut current = start;

            loop {
                // get the domain of the current node
                let domain = &self.successors[current];

                // check if the node already has an enforced edge
                let Some(fixed_value) = context.fixed_value(domain) else {
                    // if no edge is enforced, we stop following the cycle
                    break;
                };

                // if we already visited this node before
                if visited.contains(current) {
                    // check if we visited all nodes in this iteration and whether we would go to the starting node,
                    // creating a Hamiltonian cycle
                    if visited.count_ones(..) == n &&  current == start {
                        return Ok(());
                    }
                    
                    // Otherwise, we raise a conflict
                    return Err(Conflict::Propagator(PropagatorConflict {
                        conjunction: self.create_check_explanation(context, &cycle_path),
                        inference_code: self.prevent_inference_code.clone(),
                    }));
                }

                visited.insert(current);
                cycle_path.push(current);

                let next_index = domain_value_to_index(fixed_value);
                if next_index >= n {
                    break;
                    // should not happen; nodes should not be able to refer outside of range
                }
                current = next_index;
            }

        }
        Ok(())
    }

    // Creates a conflict explanation for a detected subcycle
    // The explanation contains all fixed assignments that participate in the offending cycle
    fn create_check_explanation(
        &self,
        context: Domains,
        cycle: &[usize],
    ) -> PropositionalConjunction {
        cycle
            .iter()
            .map(|&index| {
                let var = &self.successors[index];

                predicate!(
                    var == context
                        .fixed_value(var)
                        .expect("Found a subcycle")
                )
            })
            .collect()
    }
}

impl<Var: IntegerVariable + 'static> CircuitPropagator<Var> {
    // Builds two support graphs based on the domains
    // An undirected graph
    // And a directed one
    fn build_support_graph(&self, context: &Domains) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let n = self.successors.len();
        let mut undirected_graph = vec![Vec::new(); n];
        let mut directed_graph = vec![Vec::new(); n];

        for from in 0..n {
            for value in 1..=n as i32 {
                if !context.contains(&self.successors[from], value) {
                    continue;
                }
                let to = domain_value_to_index(value);
                if to >= n || from == to {
                    continue;
                }
                directed_graph[from].push(to);
            }
        }

        // Build undirected graph only from directed edges, no reverse duplication
        for from in 0..n {
            for &to in &directed_graph[from] {
                undirected_graph[from].push(to);
                // Only add reverse if the reverse directed edge does NOT already exist
                // to avoid double-adding when both a->b and b->a are in directed graph
                if !directed_graph[to].contains(&from) {
                    undirected_graph[to].push(from);
                }
            }
        }

        // dedup both
        for neighbours in undirected_graph.iter_mut() {
            neighbours.sort_unstable();
            neighbours.dedup();
        }
        for neighbours in directed_graph.iter_mut() {
            neighbours.sort_unstable();
            neighbours.dedup();
        }

        (undirected_graph, directed_graph)
    }

    // Checks if an undirected graph is disconnected
    fn is_disconnected(graph: &[Vec<usize>]) -> bool {
        let n = graph.len();

        if n <= 1 {
            return false;
        }

        let mut visited = vec![false; n];
        let mut stack = vec![0];

        visited[0] = true;

        while let Some(u) = stack.pop() {
            for &v in &graph[u] {
                if !visited[v] {
                    visited[v] = true;
                    stack.push(v);
                }
            }
        }

        visited.iter().any(|&seen| !seen)
    }

    // Checks whether the graph contains an articulation point
    fn has_articulation_point(graph: &[Vec<usize>]) -> bool {

        let n = graph.len();

        if n <= 2 {
            return false;
        }

        for removed in 0..n {
            // find a start node that isn't the removed one
            let Some(start) = (0..n).find(|&i| i != removed) else {
                continue;
            };

            let mut visited = vec![false; n];
            visited[removed] = true; // treat removed node as already visited

            let mut stack = vec![start];

            while let Some(u) = stack.pop() {
                if visited[u] {
                    continue;
                }
                visited[u] = true;
                for &v in &graph[u] {
                    if !visited[v] {
                        stack.push(v);
                    }
                }
            }

            // if any node is still unvisited, removing `removed` disconnected the graph
            if visited.iter().any(|&seen| !seen) {
                return true;
            }
        }

        false
    }

    // Constructs a graph with a candidate edge removed.
    fn build_reduced_graph(
        undirected_graph: &[Vec<usize>],
        directed_graph: &[Vec<usize>],
        from: usize,
        to: usize,
    ) -> Vec<Vec<usize>> {
        let reverse_also_exists = directed_graph[to].contains(&from);

        let remove_undirected_edge = !reverse_also_exists;

        undirected_graph
            .iter()
            .enumerate()
            .map(|(u, neighbours)| {
                neighbours
                    .iter()
                    .copied()
                    .filter(|&v| {
                        if !remove_undirected_edge {
                            return true;
                        }

                        !((u == from && v == to) || (u == to && v == from))
                    })
                    .collect()
            })
            .collect()
    }

    // Creates explanation for why edge (from -> to) must be forced:
    // all edges that are already removed from the domain are part of the reason
    fn create_force_edge_explanation(
        &self,
        context: Domains,
        from: usize,
        to: usize,
    ) -> PropositionalConjunction {
        let n = self.successors.len();
        let forced_value = index_to_domain_value(to);
        let mut predicates = Vec::new();

        for u in 0..n {
            for value in 1..=n as i32 {
                let v = domain_value_to_index(value);
                if u == from && value == forced_value {
                    continue;
                }
                if v >= n || u == v {
                    continue;
                }
                if !context.contains(&self.successors[u], value) {
                    predicates.push(predicate!(self.successors[u] != value));
                }
            }
        }

        predicates.into_iter().collect()
    }

    // Creates a conflict explanation for articulation-based failure
    // The explanation contains all currently removed edges
    fn create_full_articulation_conflict_explanation(
        &self,
        context: &Domains,
    ) -> PropositionalConjunction {
        let n = self.successors.len();
        let mut predicates = Vec::new();

        for from in 0..n {
            for value in 1..=n as i32 {
                let to = domain_value_to_index(value);
                if to >= n || from == to {
                    continue;
                }
                if !context.contains(&self.successors[from], value) {
                    predicates.push(predicate!(self.successors[from] != value));
                }
            }
        }

        predicates.into_iter().collect()
    }

    // Performs articulation-based propagation
    // First detects global failure if the support graph is disconnected or contains an articulation point
    // Then tests individual edges and forces those whose removal would introduce either property
    fn articulation_prune(&self, context: &mut PropagationContext) -> PropagationStatusCP {
        let (undirected_graph, directed_graph) = self.build_support_graph(&context.domains());

        if Self::is_disconnected(&undirected_graph) || Self::has_articulation_point(&undirected_graph) {
            let reason = self.create_full_articulation_conflict_explanation(&context.domains());
            return Err(Conflict::Propagator(PropagatorConflict {
                conjunction: reason,
                inference_code: self.articulation_inference_code.clone(),
            }));
        }

        let n = self.successors.len();

        for from in 0..n {
            if context.is_fixed(&self.successors[from]) {
                continue;
            }

            // If only one possible value exists, it will be caught by other propagation
            if directed_graph[from].len() <= 1 {
                continue;  // skip — not an articulation inference
            }

            for &to in &directed_graph[from] {
                let value = index_to_domain_value(to);
                let reduced = Self::build_reduced_graph(&undirected_graph, &directed_graph, from, to);

                if Self::is_disconnected(&reduced) || Self::has_articulation_point(&reduced) {
                    let reason = self.create_force_edge_explanation(context.domains(), from, to);
                    context.post(
                        predicate!(self.successors[from] == value),
                        reason,
                        &self.articulation_inference_code,
                    )?;
                }
            }
        }

        Ok(())
    }

}

#[derive(Debug, Clone)]
struct SccResult {
    vertex_to_scc: Vec<usize>,
    scc_sizes: Vec<usize>,
    num_sccs: usize,
}

#[derive(Debug, Clone)]
struct CondensationDag {
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
}

impl<Var: IntegerVariable + 'static> CircuitPropagator<Var> {
    fn propagate_strong_articulation_pruning(
        &self,
        context: &mut PropagationContext,
        num_saps: &mut u32,
        num_prunings: &mut u32,
        num_conflicts: &mut u32,
    ) -> PropagationStatusCP {
        let n = self.successors.len();

        // Snapshot: build graph and capture all removed edges before any posting
        let (directed_graph, removed_edges) = {
            let snapshot = context.domains();
            let (_, directed_graph) = self.build_support_graph(&snapshot);
            
            // Collect all removed edges as predicates right now
            let removed_edges = self.create_full_articulation_conflict_explanation(&snapshot);
            (directed_graph, removed_edges)
        };
        // snapshot borrow is dropped here; context is free again

        let articulation_points = Self::find_strong_articulation_points(&directed_graph);
        *num_saps += articulation_points.len() as u32;

        for articulation in articulation_points {
            let scc_result = Self::compute_sccs_without_vertex(&directed_graph, articulation);

            if scc_result.num_sccs <= 1 {
                continue;
            }

            let dag = Self::build_condensation_dag(&directed_graph, articulation, &scc_result);
            let source_sccs = dag.source_sccs();
            let sink_sccs = dag.sink_sccs();

            if source_sccs.len() != 1 || sink_sccs.len() != 1 {
                *num_conflicts += 1;
                return Err(Conflict::Propagator(PropagatorConflict {
                    conjunction: removed_edges,
                    inference_code: self.strong_articulation_inference_code.clone(),
                }));
            }

            let source_scc = source_sccs[0];
            let sink_scc = sink_sccs[0];

            let longest_path_weight =
                Self::longest_weighted_path_in_dag(&dag, &scc_result.scc_sizes);

            if longest_path_weight < n - 1 {
                *num_conflicts += 1;
                return Err(Conflict::Propagator(PropagatorConflict {
                    conjunction: removed_edges,
                    inference_code: self.strong_articulation_inference_code.clone(),
                }));
            }

            // Prune articulation -> non-source
            for value in 1..=n as i32 {
                let to = domain_value_to_index(value);
                if to >= n || to == articulation {
                    continue;
                }
                // Check against directed_graph (snapshot), not live domains
                if !directed_graph[articulation].contains(&to) {
                    continue;
                }
                if scc_result.vertex_to_scc[to] != source_scc {
                    *num_prunings += 1;
                    context.post(
                        predicate!(self.successors[articulation] != value),
                        removed_edges.clone(),
                        &self.strong_articulation_inference_code,
                    )?;
                }
            }

            // Prune non-sink -> articulation
            let articulation_value = index_to_domain_value(articulation);
            for from in 0..n {
                if from == articulation {
                    continue;
                }
                // Check against directed_graph (snapshot), not live domains
                if !directed_graph[from].contains(&articulation) {
                    continue;
                }
                if scc_result.vertex_to_scc[from] != sink_scc {
                    *num_prunings += 1;
                    context.post(
                        predicate!(self.successors[from] != articulation_value),
                        removed_edges.clone(),
                        &self.strong_articulation_inference_code,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn build_condensation_dag(
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

    fn longest_weighted_path_in_dag(
        dag: &CondensationDag,
        weights: &[usize],
    ) -> usize {
        let order = Self::topological_order(dag);
        let mut dp = weights.to_vec();

        for &u in &order {
            for &v in &dag.outgoing[u] {
                dp[v] = dp[v].max(dp[u] + weights[v]);
            }
        }

        dp.into_iter().max().unwrap_or(0)
    }

    fn topological_order(dag: &CondensationDag) -> Vec<usize> {
        let n = dag.outgoing.len();
        let mut indegree = vec![0; n];

        for u in 0..n {
            for &v in &dag.outgoing[u] {
                indegree[v] += 1;
            }
        }

        let mut stack: Vec<usize> = indegree
            .iter()
            .enumerate()
            .filter_map(|(i, &deg)| (deg == 0).then_some(i))
            .collect();

        let mut order = Vec::new();

        while let Some(u) = stack.pop() {
            order.push(u);

            for &v in &dag.outgoing[u] {
                indegree[v] -= 1;
                if indegree[v] == 0 {
                    stack.push(v);
                }
            }
        }

        order
    }

    fn find_strong_articulation_points(graph: &[Vec<usize>]) -> Vec<usize> {
        let n = graph.len();

        if n <= 2 {
            return Vec::new();
        }

        let original_scc_count = Self::count_sccs_without_vertex(graph, None);

        let mut strong_articulation_points = Vec::new();

        for removed_vertex in 0..n {
            let scc_count_after_removal =
                Self::count_sccs_without_vertex(graph, Some(removed_vertex));

            if scc_count_after_removal > original_scc_count {
                strong_articulation_points.push(removed_vertex);
            }
        }

        strong_articulation_points
    }

    fn count_sccs_without_vertex(
        graph: &[Vec<usize>],
        removed_vertex: Option<usize>,
    ) -> usize {
        let n = graph.len();

        let mut index = 0usize;
        let mut stack = Vec::new();
        let mut on_stack = vec![false; n];
        let mut indices = vec![None; n];
        let mut lowlink = vec![0usize; n];
        let mut scc_count = 0usize;

        fn strong_connect(
            vertex: usize,
            graph: &[Vec<usize>],
            removed_vertex: Option<usize>,
            index: &mut usize,
            stack: &mut Vec<usize>,
            on_stack: &mut [bool],
            indices: &mut [Option<usize>],
            lowlink: &mut [usize],
            scc_count: &mut usize,
        ) {
            indices[vertex] = Some(*index);
            lowlink[vertex] = *index;
            *index += 1;

            stack.push(vertex);
            on_stack[vertex] = true;

            for &successor in &graph[vertex] {
                if Some(successor) == removed_vertex {
                    continue;
                }

                if indices[successor].is_none() {
                    strong_connect(
                        successor,
                        graph,
                        removed_vertex,
                        index,
                        stack,
                        on_stack,
                        indices,
                        lowlink,
                        scc_count,
                    );

                    lowlink[vertex] = lowlink[vertex].min(lowlink[successor]);
                } else if on_stack[successor] {
                    lowlink[vertex] = lowlink[vertex].min(indices[successor].unwrap());
                }
            }

            if lowlink[vertex] == indices[vertex].unwrap() {
                loop {
                    let w = stack.pop().unwrap();
                    on_stack[w] = false;

                    if w == vertex {
                        break;
                    }
                }

                *scc_count += 1;
            }
        }

        for vertex in 0..n {
            if Some(vertex) == removed_vertex {
                continue;
            }

            if indices[vertex].is_none() {
                strong_connect(
                    vertex,
                    graph,
                    removed_vertex,
                    &mut index,
                    &mut stack,
                    &mut on_stack,
                    &mut indices,
                    &mut lowlink,
                    &mut scc_count,
                );
            }
        }

        scc_count
    }

    fn compute_sccs_without_vertex(graph: &[Vec<usize>], removed: usize) -> SccResult {
        let n = graph.len();

        let mut index = 0usize;
        let mut stack = Vec::new();
        let mut on_stack = vec![false; n];
        let mut indices = vec![None; n];
        let mut lowlink = vec![0usize; n];

        let mut vertex_to_scc = vec![usize::MAX; n];
        let mut scc_sizes = Vec::new();

        fn strong_connect(
            vertex: usize,
            graph: &[Vec<usize>],
            removed: usize,
            index: &mut usize,
            stack: &mut Vec<usize>,
            on_stack: &mut [bool],
            indices: &mut [Option<usize>],
            lowlink: &mut [usize],
            vertex_to_scc: &mut [usize],
            scc_sizes: &mut Vec<usize>,
        ) {
            indices[vertex] = Some(*index);
            lowlink[vertex] = *index;
            *index += 1;

            stack.push(vertex);
            on_stack[vertex] = true;

            for &successor in &graph[vertex] {
                if successor == removed {
                    continue;
                }

                if indices[successor].is_none() {
                    strong_connect(
                        successor,
                        graph,
                        removed,
                        index,
                        stack,
                        on_stack,
                        indices,
                        lowlink,
                        vertex_to_scc,
                        scc_sizes,
                    );

                    lowlink[vertex] = lowlink[vertex].min(lowlink[successor]);
                } else if on_stack[successor] {
                    lowlink[vertex] = lowlink[vertex].min(indices[successor].unwrap());
                }
            }

            if lowlink[vertex] == indices[vertex].unwrap() {
                let scc_id = scc_sizes.len();
                let mut size = 0usize;

                loop {
                    let w = stack.pop().unwrap();
                    on_stack[w] = false;
                    vertex_to_scc[w] = scc_id;
                    size += 1;

                    if w == vertex {
                        break;
                    }
                }

                scc_sizes.push(size);
            }
        }

        for vertex in 0..n {
            if vertex == removed {
                continue;
            }

            if indices[vertex].is_none() {
                strong_connect(
                    vertex,
                    graph,
                    removed,
                    &mut index,
                    &mut stack,
                    &mut on_stack,
                    &mut indices,
                    &mut lowlink,
                    &mut vertex_to_scc,
                    &mut scc_sizes,
                );
            }
        }

        SccResult {
            num_sccs: scc_sizes.len(),
            vertex_to_scc,
            scc_sizes,
        }
    }

    fn create_edge_pruning_explanation(
        &self,
        context: &Domains,
    ) -> PropositionalConjunction {
        self.create_full_articulation_conflict_explanation(context)
    }

    fn create_dag_conflict_explanation(
        &self,
        context: &Domains,
    ) -> PropositionalConjunction {
        self.create_full_articulation_conflict_explanation(context)
    }
}

impl CondensationDag {
    fn source_sccs(&self) -> Vec<usize> {
        self.incoming
            .iter()
            .enumerate()
            .filter_map(|(scc, incoming)| incoming.is_empty().then_some(scc))
            .collect()
    }

    fn sink_sccs(&self) -> Vec<usize> {
        self.outgoing
            .iter()
            .enumerate()
            .filter_map(|(scc, outgoing)| outgoing.is_empty().then_some(scc))
            .collect()
    }

    fn deduplicate_edges(&mut self) {
        for edges in self.outgoing.iter_mut() {
            edges.sort_unstable();
            edges.dedup();
        }

        for edges in self.incoming.iter_mut() {
            edges.sort_unstable();
            edges.dedup();
        }
    }
}


const VALUE_OFFSET: usize = 1;

#[inline]
fn domain_value_to_index(domain_value: i32) -> usize {
    domain_value as usize - VALUE_OFFSET
}

#[inline]
fn index_to_domain_value(index: usize) -> i32 {
    index as i32 + VALUE_OFFSET as i32
}

#[cfg(test)]
mod tests { 
    use pumpkin_core::{propagation::ReadDomains, state::State};

    use crate::circuit::CircuitConstructor;

    //VALID FULL HAMILTONIAN PATH (NO CONFLICT)
    #[test]
    fn circuit_hamiltonian_path_conflict_detection() {
        let mut state = State::default();

        let x = state.new_interval_variable(2, 2, None);
        let y = state.new_interval_variable(3, 3, None);
        let z = state.new_interval_variable(1, 1, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x, y, z].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_ok(),
            "If there is a cycle concerning all variables, then no conflict should be reported"
        )
    }
    // SIMPLE SUBCYCLE (SHOULD CONFLICT)
    #[test]
    fn circuit_conflict_detection_simple() {
        let mut state = State::default();

        let x = state.new_interval_variable(2, 2, None);
        let y = state.new_interval_variable(1, 1, None);
        let z = state.new_interval_variable(1, 3, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x, y, z].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_err(),
            "If there is a cycle concerning all variables, then no conflict should be reported"
        )
    }

    // SELF LOOP REMOVAL 
    #[test]
    fn circuit_removes_self_loops() {
        let mut state = State::default();
        let x = state.new_interval_variable(1, 3, None);
        let y = state.new_interval_variable(1, 3, None);
        let z = state.new_interval_variable(1, 3, None);

        let constraint_tag = state.new_constraint_tag();
        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x, y, z].into(),
            constraint_tag,
        });

        let _ = state.propagate_to_fixed_point();
        assert!(
            !state.get_domains().contains(&x, 1), 
            "Self-loop x=1 mst be removed"
        );
    }

    //SELF LOOP FIXED (CONFLICT)
    #[test]
    fn circuit_self_loop_fixed() {
        let mut state = State::default();
        let x = state.new_interval_variable(1, 1, None);
        let y = state.new_interval_variable(1, 3, None);
        let z = state.new_interval_variable(1, 3, None);

        let constraint_tag = state.new_constraint_tag();
        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x, y, z].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();
        assert!(result.is_err(), "Forced self-loop = conflict");
        
    }

    //Prevent should not prune closing hamilton cycle edge
    #[test]
    fn circuit_prevent_not_prune_closing_edge() {
        let mut state = State::default();
        let x = state.new_interval_variable(2, 2, None); 
        let y = state.new_interval_variable(3, 3, None); 
        let z = state.new_interval_variable(1, 3, None); 

        let constraint_tag = state.new_constraint_tag();
        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x, y, z].into(),
            constraint_tag,
        });

        let _ = state.propagate_to_fixed_point();

        assert!(
        state.get_domains().contains(&z, 1),
        "Closing edge z-x completes a full Hamiltonian cycle and must NOT be pruned"
    );
    }

    //Edge case: single variable must conflict
    #[test]
    fn circuit_single_variable_conflict() {
        let mut state = State::default();

        let x = state.new_interval_variable(1, 1, None);

       let constraint_tag = state.new_constraint_tag();
        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();
        assert!(result.is_err(), "Single node with self-loop must conflict");
    }

    //test two variables okay
    #[test]
    fn circuit_two_variable_cycle_ok() {
        let mut state = State::default();

        let x = state.new_interval_variable(2, 2, None);
        let y = state.new_interval_variable(1, 1, None);

        let constraint_tag = state.new_constraint_tag();
        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x, y].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();
        assert!(result.is_ok(), "2-cycle is a valid Hamiltonian cycle");
    }

    // MULTIPLE DISCONNECTED COMPONENTS (SHOULD CONFLICT)
    #[test]
    fn circuit_articulation_multiple_components_conflict() {
        let mut state = State::default();

        // Component 1: nodes 1, 2, 3
        let x1 = state.new_interval_variable(1, 3, None);
        let x2 = state.new_interval_variable(1, 3, None);
        let x3 = state.new_interval_variable(1, 3, None);

        // Component 2: nodes 4, 5, 6
        let x4 = state.new_interval_variable(4, 6, None);
        let x5 = state.new_interval_variable(4, 6, None);
        let x6 = state.new_interval_variable(4, 6, None);

        // Component 3: nodes 7, 8, 9
        let x7 = state.new_interval_variable(7, 9, None);
        let x8 = state.new_interval_variable(7, 9, None);
        let x9 = state.new_interval_variable(7, 9, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x1, x2, x3, x4, x5, x6, x7, x8, x9].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_err(),
            "A possible graph with multiple disconnected components cannot contain a Hamiltonian circuit"
        );
    }

    //test subcycle length n 
    #[test]
    fn circuit_doesnt_end_at_start() {
        let mut state = State::default();

        let a = state.new_interval_variable(2, 2, None);
        let b = state.new_interval_variable(3, 3, None);
        let x = state.new_interval_variable(4, 4, None);
        let y = state.new_interval_variable(2, 2, None);

        let constraint_tag = state.new_constraint_tag();
        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![a, b, x, y].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();
        assert!(result.is_err(), "cycle doesnt end at start");
    }

    // ARTICULATION POINT POSSIBLE GRAPH (SHOULD CONFLICT)
    #[test]
    fn circuit_articulation_point_conflict() {
        let mut state = State::default();

        let x1 = state.new_interval_variable(2, 2, None);
        let x2 = state.new_interval_variable(1, 4, None);
        let x3 = state.new_interval_variable(2, 2, None);
        let x4 = state.new_interval_variable(2, 2, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x1, x2, x3, x4].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_err(),
            "A possible graph with an articulation point cannot contain a Hamiltonian circuit"
        );
    }

    // ARTICULATION SHOULD NOT REJECT A SIMPLE 4-CYCLE POSSIBLE GRAPH
    #[test]
    fn circuit_articulation_no_conflict_on_cycle_graph() {
        let mut state = State::default();

        let x1 = state.new_interval_variable(2, 4, None);
        let x2 = state.new_interval_variable(1, 3, None);
        let x3 = state.new_interval_variable(2, 4, None);
        let x4 = state.new_interval_variable(1, 3, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x1, x2, x3, x4].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_ok(),
            "A biconnected possible graph should not be rejected"
        );
    }

    // PATH-SHAPED POSSIBLE GRAPH HAS ARTICULATION POINTS
    #[test]
    fn circuit_articulation_path_graph_conflict() {
        let mut state = State::default();

        let x1 = state.new_interval_variable(2, 2, None);
        let x2 = state.new_interval_variable(1, 3, None);
        let x3 = state.new_interval_variable(2, 4, None);
        let x4 = state.new_interval_variable(3, 3, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x1, x2, x3, x4].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_err(),
            "A path-shaped possible graph has articulation points"
        );
    }

    // PREVENT SHOULD NOT LOOP ON LOLLIPOP STRUCTURE
    #[test]
    fn circuit_lollipop_fixed_structure_conflict() {
        let mut state = State::default();

        let x1 = state.new_interval_variable(2, 2, None);
        let x2 = state.new_interval_variable(3, 3, None);
        let x3 = state.new_interval_variable(4, 4, None);
        let x4 = state.new_interval_variable(2, 2, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x1, x2, x3, x4].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_err(),
            "A fixed lollipop structure is not a valid Hamiltonian circuit"
        );
    }

    // ARTICULATION PRUNING SHOULD REMOVE EDGE 1 -> 4
    #[test]
    fn circuit_articulation_prunes_edge_that_creates_path_graph() {
        let mut state = State::default();

        let x1 = state.new_interval_variable(4, 5, None);
        let x2 = state.new_interval_variable(1, 1, None);
        let x3 = state.new_interval_variable(2, 2, None);
        let x4 = state.new_interval_variable(3, 3, None);
        let x5 = state.new_interval_variable(4, 4, None);

        let constraint_tag = state.new_constraint_tag();

        let _ = state.add_propagator(CircuitConstructor {
            successors: vec![x1, x2, x3, x4, x5].into(),
            constraint_tag,
        });

        let result = state.propagate_to_fixed_point();

        assert!(
            result.is_ok(),
            "The instance should remain satisfiable via 1 -> 5 -> 4 -> 3 -> 2 -> 1"
        );

        assert!(
            !state.get_domains().contains(&x1, 4),
            "Edge 1 -> 4 should be pruned because forcing it creates a path graph"
        );

        assert!(
            state.get_domains().contains(&x1, 5),
            "Edge 1 -> 5 should remain because it completes the Hamiltonian cycle"
        );
    }
}