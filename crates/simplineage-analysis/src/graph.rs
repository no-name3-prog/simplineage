//! Directed lineage graph engine.
//!
//! Edge direction follows the metadata model: **upstream → downstream**
//! (`Dependency::from_id` → `Dependency::to_id`).

use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::Direction;
use petgraph::algo::{has_path_connecting, tarjan_scc};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{EdgeRef, IntoNodeReferences};
use serde::{Deserialize, Serialize};
use simplineage_core::model::Snapshot;
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
use simplineage_core::model::ids::ObjectId;

use crate::error::{GraphError, Result};

/// Metadata stored on each directed edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeData {
    /// Original dependency id when built from a [`Dependency`].
    pub dependency_id: Option<ObjectId>,
    /// Dependency classification.
    pub kind: DependencyKind,
    /// Relation vs column granularity.
    pub level: DependencyLevel,
}

impl Default for EdgeData {
    fn default() -> Self {
        Self {
            dependency_id: None,
            kind: DependencyKind::Inferred,
            level: DependencyLevel::Relation,
        }
    }
}

/// Options controlling traversal and path enumeration.
#[derive(Debug, Clone)]
pub struct TraversalOptions {
    /// Maximum hop depth (`None` = unlimited).
    pub max_depth: Option<usize>,
    /// Maximum number of paths to return for multi-path queries.
    pub max_paths: usize,
    /// Maximum path length (edges) for all-paths search.
    pub max_path_length: Option<usize>,
}

impl Default for TraversalOptions {
    fn default() -> Self {
        Self {
            max_depth: None,
            max_paths: 10_000,
            max_path_length: Some(64),
        }
    }
}

/// High-performance directed lineage graph.
///
/// Internally uses compact integer node indices with adjacency lists (via petgraph)
/// for O(1) neighbor access and cache-friendly BFS/DFS.
#[derive(Debug, Clone)]
pub struct LineageGraph {
    graph: DiGraph<ObjectId, EdgeData>,
    /// Map object id → node index.
    index: HashMap<ObjectId, NodeIndex>,
}

impl Default for LineageGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl LineageGraph {
    /// Create an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            index: HashMap::new(),
        }
    }

    /// Build from a metadata [`Snapshot`] using its dependency edges.
    ///
    /// All object ids referenced by dependencies (and optionally isolated
    /// catalog objects) are included as nodes.
    #[must_use]
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        let mut g = Self::new();
        // Seed nodes from all catalog objects so isolated nodes appear.
        for obj in snapshot.object_index().keys() {
            g.ensure_node(obj);
        }
        for dep in &snapshot.dependencies {
            g.add_dependency(dep);
        }
        g
    }

    /// Build from an explicit dependency list.
    #[must_use]
    pub fn from_dependencies(deps: impl IntoIterator<Item = Dependency>) -> Self {
        let mut g = Self::new();
        for dep in deps {
            g.add_dependency(&dep);
        }
        g
    }

    /// Number of nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of directed edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Whether the graph contains `id`.
    #[must_use]
    pub fn contains(&self, id: &ObjectId) -> bool {
        self.index.contains_key(id)
    }

    /// All node ids in insertion order of discovery.
    #[must_use]
    pub fn nodes(&self) -> Vec<ObjectId> {
        self.graph
            .node_references()
            .map(|(_, id)| id.clone())
            .collect()
    }

    /// Ensure a node exists; return its index.
    pub fn ensure_node(&mut self, id: &ObjectId) -> NodeIndex {
        if let Some(&idx) = self.index.get(id) {
            return idx;
        }
        let idx = self.graph.add_node(id.clone());
        self.index.insert(id.clone(), idx);
        idx
    }

    /// Add a lineage edge (upstream → downstream).
    pub fn add_dependency(&mut self, dep: &Dependency) {
        let from = self.ensure_node(&dep.from_id);
        let to = self.ensure_node(&dep.to_id);
        self.graph.add_edge(
            from,
            to,
            EdgeData {
                dependency_id: Some(dep.id.clone()),
                kind: dep.kind.clone(),
                level: dep.level,
            },
        );
    }

    /// Add a simple edge between two ids (inferred relation-level).
    pub fn add_edge(&mut self, from: &ObjectId, to: &ObjectId) {
        let f = self.ensure_node(from);
        let t = self.ensure_node(to);
        self.graph.add_edge(f, t, EdgeData::default());
    }

    fn require(&self, id: &ObjectId) -> Result<NodeIndex> {
        self.index
            .get(id)
            .copied()
            .ok_or_else(|| GraphError::UnknownNode(id.to_string()))
    }

    fn id_of(&self, idx: NodeIndex) -> &ObjectId {
        &self.graph[idx]
    }

    // ── Traversal ───────────────────────────────────────────────────────

    /// All **upstream** ancestors of `id` (nodes that produce data consumed by `id`).
    ///
    /// Walks reverse edges with BFS. Order is breadth-first; `id` itself is excluded.
    pub fn upstream(&self, id: &ObjectId, opts: &TraversalOptions) -> Result<Vec<ObjectId>> {
        self.walk(id, Direction::Incoming, opts)
    }

    /// All **downstream** descendants of `id` (nodes that depend on `id`).
    ///
    /// Walks forward edges with BFS. Order is breadth-first; `id` itself is excluded.
    pub fn downstream(&self, id: &ObjectId, opts: &TraversalOptions) -> Result<Vec<ObjectId>> {
        self.walk(id, Direction::Outgoing, opts)
    }

    fn walk(
        &self,
        id: &ObjectId,
        direction: Direction,
        opts: &TraversalOptions,
    ) -> Result<Vec<ObjectId>> {
        let start = self.require(id)?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        // queue of (node, depth)
        let mut q = VecDeque::new();
        q.push_back((start, 0usize));
        seen.insert(start);

        while let Some((node, depth)) = q.pop_front() {
            if node != start {
                out.push(self.id_of(node).clone());
            }
            if let Some(max) = opts.max_depth {
                if depth >= max {
                    continue;
                }
            }
            for neigh in self.graph.neighbors_directed(node, direction) {
                if seen.insert(neigh) {
                    q.push_back((neigh, depth + 1));
                }
            }
        }
        Ok(out)
    }

    /// Shortest path (fewest hops) from `from` to `to` following downstream edges.
    ///
    /// Returns node ids including endpoints, or `None` if unreachable.
    pub fn shortest_path(&self, from: &ObjectId, to: &ObjectId) -> Result<Option<Vec<ObjectId>>> {
        let start = self.require(from)?;
        let goal = self.require(to)?;
        if start == goal {
            return Ok(Some(vec![from.clone()]));
        }

        let mut prev: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut q = VecDeque::new();
        let mut seen = HashSet::new();
        q.push_back(start);
        seen.insert(start);

        let mut found = false;
        'search: while let Some(node) = q.pop_front() {
            for edge in self.graph.edges_directed(node, Direction::Outgoing) {
                let next = edge.target();
                if seen.insert(next) {
                    prev.insert(next, node);
                    if next == goal {
                        found = true;
                        break 'search;
                    }
                    q.push_back(next);
                }
            }
        }

        if !found {
            // also report if petgraph agrees
            let _ = has_path_connecting(&self.graph, start, goal, None);
            return Ok(None);
        }

        let mut path = vec![goal];
        let mut cur = goal;
        while cur != start {
            cur = prev[&cur];
            path.push(cur);
        }
        path.reverse();
        Ok(Some(
            path.into_iter().map(|i| self.id_of(i).clone()).collect(),
        ))
    }

    /// Enumerate simple dependency paths from `from` to `to` (downstream direction).
    ///
    /// Paths are simple (no repeated nodes). Bounded by [`TraversalOptions`].
    pub fn all_paths(
        &self,
        from: &ObjectId,
        to: &ObjectId,
        opts: &TraversalOptions,
    ) -> Result<Vec<Vec<ObjectId>>> {
        let start = self.require(from)?;
        let goal = self.require(to)?;
        let mut paths = Vec::new();
        let mut current = vec![start];
        let mut on_path = HashSet::new();
        on_path.insert(start);

        self.dfs_paths(start, goal, &mut current, &mut on_path, &mut paths, opts);
        Ok(paths
            .into_iter()
            .map(|p| p.into_iter().map(|i| self.id_of(i).clone()).collect())
            .collect())
    }

    fn dfs_paths(
        &self,
        node: NodeIndex,
        goal: NodeIndex,
        current: &mut Vec<NodeIndex>,
        on_path: &mut HashSet<NodeIndex>,
        paths: &mut Vec<Vec<NodeIndex>>,
        opts: &TraversalOptions,
    ) {
        if paths.len() >= opts.max_paths {
            return;
        }
        if node == goal && current.len() > 1 {
            paths.push(current.clone());
            return;
        }
        if node == goal && current.len() == 1 {
            // start == goal: single-node path already handled by shortest_path;
            // for all_paths treat as one trivial path if requested.
            paths.push(current.clone());
            return;
        }
        if let Some(max_len) = opts.max_path_length {
            if current.len() > max_len {
                return;
            }
        }

        for next in self.graph.neighbors_directed(node, Direction::Outgoing) {
            if on_path.contains(&next) {
                continue; // avoid cycles
            }
            on_path.insert(next);
            current.push(next);
            self.dfs_paths(next, goal, current, on_path, paths, opts);
            current.pop();
            on_path.remove(&next);
            if paths.len() >= opts.max_paths {
                return;
            }
        }
    }

    // ── Cycles ──────────────────────────────────────────────────────────

    /// Detect directed cycles via Tarjan strongly connected components.
    ///
    /// Returns each cycle as a list of node ids (SCCs with size > 1, or
    /// self-loops as single-node cycles).
    #[must_use]
    pub fn detect_cycles(&self) -> Vec<Vec<ObjectId>> {
        let sccs = tarjan_scc(&self.graph);
        let mut cycles = Vec::new();
        for scc in sccs {
            if scc.len() > 1 {
                cycles.push(scc.into_iter().map(|i| self.id_of(i).clone()).collect());
            } else if let Some(&n) = scc.first() {
                // self-loop?
                if self.graph.find_edge(n, n).is_some() {
                    cycles.push(vec![self.id_of(n).clone()]);
                }
            }
        }
        cycles
    }

    /// Whether the graph contains any directed cycle.
    #[must_use]
    pub fn has_cycle(&self) -> bool {
        !self.detect_cycles().is_empty()
    }

    // ── Statistics ──────────────────────────────────────────────────────

    /// Compute summary statistics for the graph.
    #[must_use]
    pub fn statistics(&self) -> GraphStatistics {
        let n = self.node_count();
        let e = self.edge_count();
        let mut in_deg = Vec::with_capacity(n);
        let mut out_deg = Vec::with_capacity(n);
        let mut isolates = 0usize;
        let mut max_in = 0usize;
        let mut max_out = 0usize;

        for node in self.graph.node_indices() {
            let indeg = self
                .graph
                .neighbors_directed(node, Direction::Incoming)
                .count();
            let outdeg = self
                .graph
                .neighbors_directed(node, Direction::Outgoing)
                .count();
            in_deg.push(indeg);
            out_deg.push(outdeg);
            max_in = max_in.max(indeg);
            max_out = max_out.max(outdeg);
            if indeg == 0 && outdeg == 0 {
                isolates += 1;
            }
        }

        let avg_in = if n == 0 {
            0.0
        } else {
            in_deg.iter().sum::<usize>() as f64 / n as f64
        };
        let avg_out = if n == 0 {
            0.0
        } else {
            out_deg.iter().sum::<usize>() as f64 / n as f64
        };

        // Density for directed simple graph: e / (n*(n-1))
        let density = if n < 2 {
            0.0
        } else {
            e as f64 / (n as f64 * (n as f64 - 1.0))
        };

        let sccs = tarjan_scc(&self.graph);
        let cyclic_sccs = sccs.iter().filter(|s| s.len() > 1).count();
        // Weakly connected components: treat as undirected
        let wcc = weakly_connected_components(&self.graph);

        // Sources (in=0, out>0) and sinks (out=0, in>0)
        let mut sources = 0usize;
        let mut sinks = 0usize;
        for i in 0..n {
            if in_deg[i] == 0 && out_deg[i] > 0 {
                sources += 1;
            }
            if out_deg[i] == 0 && in_deg[i] > 0 {
                sinks += 1;
            }
        }

        GraphStatistics {
            node_count: n,
            edge_count: e,
            density,
            avg_in_degree: avg_in,
            avg_out_degree: avg_out,
            max_in_degree: max_in,
            max_out_degree: max_out,
            isolated_nodes: isolates,
            source_nodes: sources,
            sink_nodes: sinks,
            weakly_connected_components: wcc,
            strongly_connected_components: sccs.len(),
            cyclic_components: cyclic_sccs,
            has_cycle: cyclic_sccs > 0
                || self.graph.edge_indices().any(|e| {
                    let (a, b) = self.graph.edge_endpoints(e).unwrap();
                    a == b
                }),
        }
    }

    /// Edge data between two nodes if any edge exists.
    #[must_use]
    pub fn edges_between(&self, from: &ObjectId, to: &ObjectId) -> Vec<&EdgeData> {
        let (Some(&f), Some(&t)) = (self.index.get(from), self.index.get(to)) else {
            return Vec::new();
        };
        self.graph
            .edges_connecting(f, t)
            .map(|e| e.weight())
            .collect()
    }

    /// Out-neighbors (immediate downstream).
    pub fn out_neighbors(&self, id: &ObjectId) -> Result<Vec<ObjectId>> {
        let idx = self.require(id)?;
        Ok(self
            .graph
            .neighbors_directed(idx, Direction::Outgoing)
            .map(|n| self.id_of(n).clone())
            .collect())
    }

    /// In-neighbors (immediate upstream).
    pub fn in_neighbors(&self, id: &ObjectId) -> Result<Vec<ObjectId>> {
        let idx = self.require(id)?;
        Ok(self
            .graph
            .neighbors_directed(idx, Direction::Incoming)
            .map(|n| self.id_of(n).clone())
            .collect())
    }
}

/// Summary statistics for a [`LineageGraph`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphStatistics {
    /// Node count.
    pub node_count: usize,
    /// Directed edge count.
    pub edge_count: usize,
    /// Directed density `e / (n*(n-1))`.
    pub density: f64,
    /// Mean in-degree.
    pub avg_in_degree: f64,
    /// Mean out-degree.
    pub avg_out_degree: f64,
    /// Maximum in-degree.
    pub max_in_degree: usize,
    /// Maximum out-degree.
    pub max_out_degree: usize,
    /// Nodes with degree 0.
    pub isolated_nodes: usize,
    /// Nodes with in-degree 0 and out-degree > 0.
    pub source_nodes: usize,
    /// Nodes with out-degree 0 and in-degree > 0.
    pub sink_nodes: usize,
    /// Weakly connected component count.
    pub weakly_connected_components: usize,
    /// Strongly connected component count.
    pub strongly_connected_components: usize,
    /// SCCs with more than one node (cyclic groups).
    pub cyclic_components: usize,
    /// Whether any directed cycle exists.
    pub has_cycle: bool,
}

fn weakly_connected_components(graph: &DiGraph<ObjectId, EdgeData>) -> usize {
    let n = graph.node_count();
    if n == 0 {
        return 0;
    }
    let mut seen = HashSet::new();
    let mut components = 0usize;
    for start in graph.node_indices() {
        if !seen.insert(start) {
            continue;
        }
        components += 1;
        let mut q = VecDeque::new();
        q.push_back(start);
        while let Some(node) = q.pop_front() {
            for neigh in graph
                .neighbors_directed(node, Direction::Outgoing)
                .chain(graph.neighbors_directed(node, Direction::Incoming))
            {
                if seen.insert(neigh) {
                    q.push_back(neigh);
                }
            }
        }
    }
    components
}

/// Convenience: build graph and run upstream impact (all ancestors).
pub fn upstream_impact(snapshot: &Snapshot, node: &ObjectId) -> Result<Vec<ObjectId>> {
    let g = LineageGraph::from_snapshot(snapshot);
    g.upstream(node, &TraversalOptions::default())
}

/// Convenience: build graph and run downstream impact (all descendants).
pub fn downstream_impact(snapshot: &Snapshot, node: &ObjectId) -> Result<Vec<ObjectId>> {
    let g = LineageGraph::from_snapshot(snapshot);
    g.downstream(node, &TraversalOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};

    fn oid(s: &str) -> ObjectId {
        ObjectId::from_trusted(s)
    }

    fn dep(from: &str, to: &str) -> Dependency {
        Dependency {
            id: oid(&format!("d:{from}->{to}")),
            from_id: oid(from),
            to_id: oid(to),
            kind: DependencyKind::Manual,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        }
    }

    /// A → B → C, A → C, D isolated edge E→F with cycle F→E
    fn sample() -> LineageGraph {
        LineageGraph::from_dependencies([
            dep("A", "B"),
            dep("B", "C"),
            dep("A", "C"),
            dep("E", "F"),
            dep("F", "E"),
        ])
    }

    #[test]
    fn upstream_downstream() {
        let g = sample();
        let up = g.upstream(&oid("C"), &TraversalOptions::default()).unwrap();
        assert!(up.contains(&oid("A")));
        assert!(up.contains(&oid("B")));
        let down = g
            .downstream(&oid("A"), &TraversalOptions::default())
            .unwrap();
        assert!(down.contains(&oid("B")));
        assert!(down.contains(&oid("C")));
    }

    #[test]
    fn shortest_path_and_all_paths() {
        let g = sample();
        let sp = g.shortest_path(&oid("A"), &oid("C")).unwrap().unwrap();
        assert_eq!(sp, vec![oid("A"), oid("C")]); // direct edge is shorter than A-B-C

        let paths = g
            .all_paths(&oid("A"), &oid("C"), &TraversalOptions::default())
            .unwrap();
        assert!(paths.len() >= 2);
        assert!(paths.iter().any(|p| p.len() == 2));
        assert!(paths.iter().any(|p| p.len() == 3));
    }

    #[test]
    fn cycle_detection() {
        let g = sample();
        assert!(g.has_cycle());
        let cycles = g.detect_cycles();
        assert!(!cycles.is_empty());
        let flat: HashSet<_> = cycles.into_iter().flatten().collect();
        assert!(flat.contains(&oid("E")));
        assert!(flat.contains(&oid("F")));
    }

    #[test]
    fn statistics_smoke() {
        let g = sample();
        let s = g.statistics();
        assert_eq!(s.node_count, 5); // A B C E F
        assert_eq!(s.edge_count, 5);
        assert!(s.has_cycle);
        assert!(s.density > 0.0);
        assert!(s.weakly_connected_components >= 2);
    }

    #[test]
    fn max_depth_limits_downstream() {
        let g = sample();
        let opts = TraversalOptions {
            max_depth: Some(1),
            ..Default::default()
        };
        let down = g.downstream(&oid("A"), &opts).unwrap();
        assert!(down.contains(&oid("B")));
        assert!(down.contains(&oid("C"))); // C is also direct child
        // B's child not reached only via B if max_depth is from start...
        // depth 1 means only immediate neighbors
        assert!(!down.iter().any(|x| x.as_str() == "missing"));
        let only_b_path = LineageGraph::from_dependencies([dep("A", "B"), dep("B", "C")]);
        let d = only_b_path.downstream(&oid("A"), &opts).unwrap();
        assert_eq!(d, vec![oid("B")]);
    }

    #[test]
    fn unknown_node_errors() {
        let g = sample();
        assert!(g.upstream(&oid("Z"), &TraversalOptions::default()).is_err());
    }
}
