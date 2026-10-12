use crate::event::EventId;
use crate::time::SimulationTime;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TracingMode {
    /// No causal nodes are recorded. Observation/event log still works.
    /// Use this for large simulations where memory must not grow with events.
    Off,
    /// Records one causal node per executed event (black-box level).
    #[default]
    Abstract,
    /// Records additional nodes for composite internal routing (deep refinement).
    Hierarchical,
}

impl TracingMode {
    /// Whether any causal information should be recorded.
    pub fn is_enabled(self) -> bool {
        !matches!(self, TracingMode::Off)
    }

    /// Whether composite-internal routing nodes should be recorded.
    pub fn is_hierarchical(self) -> bool {
        matches!(self, TracingMode::Hierarchical)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraceId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
    pub id: TraceId,
    pub event_id: EventId,
    pub time: SimulationTime,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalEdge {
    pub cause: TraceId,
    pub effect: TraceId,
    pub relation: String,
}

#[derive(Debug, Clone, Default)]
pub struct CausalGraph {
    nodes: HashMap<TraceId, TraceNode>,
    edges: Vec<CausalEdge>,
    next_trace_id: u64,
}

impl CausalGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: TraceNode) {
        self.nodes.insert(node.id, node);
    }

    pub fn add_edge(&mut self, edge: CausalEdge) {
        self.edges.push(edge);
    }

    pub fn record_event(
        &mut self,
        event_id: EventId,
        time: SimulationTime,
        description: impl Into<String>,
        cause: Option<TraceId>,
    ) -> TraceId {
        self.next_trace_id += 1;
        let trace_id = TraceId(self.next_trace_id);
        let node = TraceNode {
            id: trace_id,
            event_id,
            time,
            description: description.into(),
        };
        self.add_node(node);

        if let Some(cause_id) = cause {
            self.add_edge(CausalEdge {
                cause: cause_id,
                effect: trace_id,
                relation: "caused".to_string(),
            });
        }

        trace_id
    }

    pub fn nodes(&self) -> &HashMap<TraceId, TraceNode> {
        &self.nodes
    }

    pub fn edges(&self) -> &[CausalEdge] {
        &self.edges
    }

    /// Return the unique causal ancestors of `target` (including `target`)
    /// in causal order (causes before effects).
    ///
    /// Each [`TraceNode`] appears exactly once. Nodes are ordered by trace id,
    /// which is creation order and therefore always topological: a node's cause
    /// is always recorded before the node itself.
    pub fn explain(&self, target: TraceId) -> Vec<&TraceNode> {
        let mut included: HashSet<TraceId> = HashSet::new();
        let mut stack = vec![target];

        while let Some(id) = stack.pop() {
            if !self.nodes.contains_key(&id) || !included.insert(id) {
                continue;
            }
            for edge in &self.edges {
                if edge.effect == id {
                    stack.push(edge.cause);
                }
            }
        }

        let mut nodes: Vec<&TraceNode> = included
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .collect();
        nodes.sort_by_key(|node| node.id.0);
        nodes
    }
}
