use crate::event::EventId;
use crate::time::SimulationTime;
use std::collections::HashMap;

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

    pub fn explain(&self, target: TraceId) -> Vec<&TraceNode> {
        let mut path = Vec::new();
        self.collect_causes(target, &mut path);
        path.reverse();
        path
    }

    fn collect_causes<'a>(&'a self, target: TraceId, path: &mut Vec<&'a TraceNode>) {
        if let Some(node) = self.nodes.get(&target) {
            path.push(node);
            for edge in &self.edges {
                if edge.effect == target {
                    self.collect_causes(edge.cause, path);
                }
            }
        }
    }
}
