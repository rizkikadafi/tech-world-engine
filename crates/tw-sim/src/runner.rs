use crate::engine::SimulationEngine;

pub struct SimulationRunner;

impl SimulationRunner {
    pub fn run(engine: &mut SimulationEngine, duration_ticks: u64) {
        engine.run_ticks(duration_ticks);
    }
}
