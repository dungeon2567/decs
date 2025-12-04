use crate::frame::Frame;
use crate::system::System;
use std::any::TypeId;
use std::collections::{HashMap, VecDeque};

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

        for (i, system) in self.systems.iter().enumerate() {
            for before_type in system.before() {
                if let Some(indices) = index_by_type.get(before_type) {
                    for &j in indices {
                        if i != j {
                            graph.add_edge(i, j);
                        }
                    }
                }
            }
            for after_type in system.after() {
                if let Some(indices) = index_by_type.get(after_type) {
                    for &j in indices {
                        if i != j {
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

        for (t, writers) in writes_by_type.iter() {
            if let Some(readers) = reads_by_type.get(t) {
                for &w in writers {
                    for &r in readers {
                        if w != r {
                            graph.add_edge(w, r);
                        }
                    }
                }
            }
            if writers.len() > 1 {
                for k in 0..writers.len() - 1 {
                    graph.add_edge(writers[k], writers[k + 1]);
                }
            }
        }

        for i in 0..n {
            if let Some(mut group) = self.systems[i].parent() {
                loop {
                    for before_type in group.before() {
                        if let Some(bs) = index_by_type.get(before_type) {
                            for &k in bs {
                                if i != k {
                                    graph.add_edge(i, k);
                                }
                            }
                        }
                    }
                    for after_type in group.after() {
                        if let Some(as_) = index_by_type.get(after_type) {
                            for &k in as_ {
                                if i != k {
                                    graph.add_edge(k, i);
                                }
                            }
                        }
                    }
                    if let Some(parent) = group.parent() {
                        group = parent;
                    } else {
                        break;
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

    fn topological_levels(&mut self) -> Vec<Vec<usize>> {
        let mut levels: Vec<Vec<usize>> = Vec::new();
        let mut in_degree = self.in_degree.clone();
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
