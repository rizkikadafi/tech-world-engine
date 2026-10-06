use crate::simulation::Simulation;
use tw_core::SimulationTime;

pub struct SimulationEngine {
    simulation: Simulation,
}

impl SimulationEngine {
    pub fn new(simulation: Simulation) -> Self {
        Self { simulation }
    }

    pub fn simulation(&self) -> &Simulation {
        &self.simulation
    }

    pub fn simulation_mut(&mut self) -> &mut Simulation {
        &mut self.simulation
    }

    pub fn run_ticks(&mut self, ticks: u64) {
        let target = SimulationTime::new(self.simulation.current_time().ticks() + ticks);
        self.simulation.run_until(target);
    }
}
