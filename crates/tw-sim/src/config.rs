use tw_core::TracingMode;

/// Engine-wide configuration.
///
/// `Simulation::new()` uses [`SimulationConfig::default`]; use
/// [`Simulation::with_config`](crate::Simulation::with_config) (or
/// [`configure`](crate::Simulation::configure)) to change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SimulationConfig {
    /// How much causal information to record (see [`TracingMode`]).
    pub tracing_mode: TracingMode,
    /// Optional safety cap on the number of executed events. When reached, the
    /// event loop stops instead of running forever, and a warning is logged.
    pub max_events: Option<u64>,
    /// Seed reserved for future stochastic models. Currently unused.
    pub seed: u64,
}

impl SimulationConfig {
    pub fn with_tracing_mode(mut self, mode: TracingMode) -> Self {
        self.tracing_mode = mode;
        self
    }

    pub fn with_max_events(mut self, max_events: u64) -> Self {
        self.max_events = Some(max_events);
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}
