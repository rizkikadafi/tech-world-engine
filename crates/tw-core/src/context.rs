use crate::event::EventId;
use crate::time::SimulationTime;
use crate::tracing::{CausalGraph, TraceId, TracingMode};

pub struct SimulationContext<'a> {
    pub time: SimulationTime,
    pub causal_graph: &'a mut CausalGraph,
    pub tracing_mode: TracingMode,
}

impl<'a> SimulationContext<'a> {
    pub fn new(
        time: SimulationTime,
        causal_graph: &'a mut CausalGraph,
        tracing_mode: TracingMode,
    ) -> Self {
        Self {
            time,
            causal_graph,
            tracing_mode,
        }
    }

    pub fn time(&self) -> SimulationTime {
        self.time
    }

    pub fn tracing_mode(&self) -> TracingMode {
        self.tracing_mode
    }

    pub fn record_trace(
        &mut self,
        event_id: EventId,
        description: impl Into<String>,
        cause: Option<TraceId>,
    ) -> TraceId {
        self.causal_graph
            .record_event(event_id, self.time, description, cause)
    }
}
