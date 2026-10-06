#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimulationTime {
    ticks: u64,
}

impl SimulationTime {
    pub const ZERO: Self = Self { ticks: 0 };

    pub fn new(ticks: u64) -> Self {
        Self { ticks }
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn advance(&mut self, amount: u64) {
        self.ticks += amount;
    }
}
