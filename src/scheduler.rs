use crate::frame::Frame;
use crate::system::System;
use std::any::TypeId;
use std::collections::{HashMap, HashSet, VecDeque};

/// A scheduler that manages system execution order based on dependencies.
/// Supports hierarchical system groups similar to Unity DOTS ECS.
pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
    wavefronts: Vec<Vec<usize>>,
}

impl Scheduler {
    /// Creates a new empty scheduler.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            wavefronts: Vec::new(),
        }
    }

    /// Adds a system to the scheduler.
    pub fn add_system<S: System>(&mut self, system: S) {
        self.systems.push(Box::new(system));
        self.wavefronts.clear();
    }

    /// Runs all systems using precomputed wavefronts. Call `build_wavefronts` after
    /// adding systems or changing dependencies before invoking `run`.
    pub fn run(&self, frame: &Frame) {
        for wave in &self.wavefronts {
            for &idx in wave {
                self.systems[idx].run(frame);
            }
        }
    }

    /// Builds wavefronts based on current systems and dependencies.
    pub fn build_wavefronts(&mut self) {
        self.wavefronts = self.compute_wavefronts();
    }

    pub fn wavefronts(&self) -> &[Vec<usize>] {
        &self.wavefronts
    }

    fn compute_wavefronts(&self) -> Vec<Vec<usize>> {
        let n = self.systems.len();
        let mut graph = DependencyGraph::new(n);

        let type_ids: Vec<TypeId> = self
            .systems
            .iter()
            .map(|s| std::any::Any::type_id(s.as_any()))
            .collect();

        let mut index_by_type: HashMap<TypeId, Vec<usize>> = HashMap::with_capacity(type_ids.len());
        for (idx, ty) in type_ids.iter().enumerate() {
            index_by_type.entry(*ty).or_default().push(idx);
        }

        // Precompute reads/writes per system for conflict checks
        let mut sys_reads: Vec<HashSet<TypeId>> = vec![HashSet::new(); n];
        let mut sys_writes: Vec<HashSet<TypeId>> = vec![HashSet::new(); n];
        for (i, system) in self.systems.iter().enumerate() {
            for &t in system.reads() {
                sys_reads[i].insert(t);
            }
            for &t in system.writes() {
                sys_writes[i].insert(t);
            }
        }

        // Helper to determine if two systems have a write-write or read-write conflict
        let has_conflict = |a: usize, b: usize| -> bool {
            // write-write
            if sys_writes[a].iter().any(|t| sys_writes[b].contains(t)) {
                return true;
            }
            // a writes, b reads
            if sys_writes[a].iter().any(|t| sys_reads[b].contains(t)) {
                return true;
            }
            // b writes, a reads
            if sys_writes[b].iter().any(|t| sys_reads[a].contains(t)) {
                return true;
            }
            false
        };

        // Add system-level before/after edges only when there is a conflict
        for (i, system) in self.systems.iter().enumerate() {
            for before_type in system.before() {
                if let Some(indices) = index_by_type.get(before_type) {
                    for &j in indices {
                        if i != j && has_conflict(i, j) {
                            graph.add_edge(i, j);
                        }
                    }
                }
            }
            for after_type in system.after() {
                if let Some(indices) = index_by_type.get(after_type) {
                    for &j in indices {
                        if i != j && has_conflict(j, i) {
                            graph.add_edge(j, i);
                        }
                    }
                }
            }
        }

        let mut reads_by_type: HashMap<TypeId, Vec<usize>> = HashMap::new();
        let mut writes_by_type: HashMap<TypeId, Vec<usize>> = HashMap::new();
        for (i, system) in self.systems.iter().enumerate() {
            for &t in system.reads() {
                reads_by_type.entry(t).or_default().push(i);
            }
            for &t in system.writes() {
                writes_by_type.entry(t).or_default().push(i);
            }
        }

        // Build group membership maps
        let mut systems_by_group: HashMap<TypeId, Vec<usize>> = HashMap::new();
        let mut groups_by_system: Vec<Vec<&dyn crate::system::SystemGroup>> = vec![Vec::new(); n];
        for (i, system) in self.systems.iter().enumerate() {
            if let Some(mut group) = system.parent() {
                loop {
                    let gid = group.as_any().type_id();
                    systems_by_group.entry(gid).or_default().push(i);
                    groups_by_system[i].push(group);
                    if let Some(parent) = group.parent() {
                        group = parent;
                    } else {
                        break;
                    }
                }
            }
        }
        // Enforce group Before/After constraints among systems
        let mut constrained_edges: HashSet<(usize, usize)> = HashSet::new();
        for (i, groups) in groups_by_system.iter().enumerate() {
            for group in groups {
                for before_type in group.before() {
                    if let Some(targets) = systems_by_group.get(before_type) {
                        for &j in targets {
                            if i != j && has_conflict(i, j) {
                                graph.add_edge(i, j);
                                constrained_edges.insert((i, j));
                            }
                        }
                    }
                }
                for after_type in group.after() {
                    if let Some(sources) = systems_by_group.get(after_type) {
                        for &j in sources {
                            if i != j && has_conflict(j, i) {
                                graph.add_edge(j, i);
                                constrained_edges.insert((j, i));
                            }
                        }
                    }
                }
            }
        }

        // Now add writer->reader edges, but do not contradict group constraints
        for (t, writers) in writes_by_type.iter() {
            if let Some(readers) = reads_by_type.get(t) {
                for &w in writers {
                    for &r in readers {
                        if w != r {
                            // If group constraints require r -> w, skip w -> r
                            if !constrained_edges.contains(&(r, w)) {
                                graph.add_edge(w, r);
                            }
                        }
                    }
                }
            }
            if writers.len() > 1 {
                for k in 0..writers.len() - 1 {
                    let a = writers[k];
                    let b = writers[k + 1];
                    if !constrained_edges.contains(&(b, a)) {
                        graph.add_edge(a, b);
                    }
                }
            }
        }

        graph.topological_levels()
    }

    /// Returns the number of systems in the scheduler.
    pub fn len(&self) -> usize {
        self.systems.len()
    }

    /// Returns true if the scheduler has no systems.
    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (idx, wf) in self.wavefronts.iter().enumerate() {
            let names: Vec<&'static str> = wf.iter().map(|&i| self.systems[i].name()).collect();
            writeln!(f, "wavefront {} names [ {} ]", idx + 1, names.join(", "))?;
        }
        Ok(())
    }
}

/// A dependency graph for systems.
struct DependencyGraph {
    n: usize,
    adjacency: Vec<Vec<usize>>,
    in_degree: Vec<usize>,
}

impl DependencyGraph {
    fn new(n: usize) -> Self {
        Self {
            n,
            adjacency: vec![Vec::new(); n],
            in_degree: vec![0; n],
        }
    }

    fn add_edge(&mut self, from: usize, to: usize) {
        if from < self.n && to < self.n && !self.adjacency[from].contains(&to) {
            self.adjacency[from].push(to);
            self.in_degree[to] += 1;
        }
    }

    // Linear topological sort is intentionally removed from public use; levels are used instead.

    fn topological_levels(self) -> Vec<Vec<usize>> {
        let mut levels: Vec<Vec<usize>> = Vec::new();
        let mut in_degree = self.in_degree;
        let mut queue: VecDeque<usize> = VecDeque::new();
        for (i, deg) in in_degree.iter().enumerate() {
            if *deg == 0 {
                queue.push_back(i);
            }
        }
        let mut visited = 0usize;
        while visited < self.n {
            if queue.is_empty() {
                let mut rem: Vec<usize> = Vec::new();
                for (i, deg) in in_degree.iter().enumerate() {
                    if *deg > 0 {
                        rem.push(i);
                    }
                }
                if !rem.is_empty() {
                    levels.push(rem);
                }
                break;
            }
            let round = queue.len();
            let mut level: Vec<usize> = Vec::with_capacity(round);
            for _ in 0..round {
                if let Some(node) = queue.pop_front() {
                    level.push(node);
                    visited += 1;
                    for &adj in &self.adjacency[node] {
                        if in_degree[adj] > 0 {
                            in_degree[adj] -= 1;
                            if in_degree[adj] == 0 {
                                queue.push_back(adj);
                            }
                        }
                    }
                }
            }
            if !level.is_empty() {
                levels.push(level);
            }
        }
        levels
    }
}
