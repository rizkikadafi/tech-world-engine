//! Edge conversion between Godot's floating-point frame delta and the
//! engine's deterministic integer ticks.
//!
//! The engine itself never sees `f64` seconds (INVARIAN-4). This accumulator is
//! the only place where wall-clock time enters the simulation.

/// Converts a stream of frame deltas (seconds) into whole simulation ticks.
#[derive(Debug, Clone)]
pub struct TickClock {
    ticks_per_second: f64,
    accumulator: f64,
}

impl TickClock {
    pub fn new(ticks_per_second: u64) -> Self {
        Self {
            ticks_per_second: ticks_per_second.max(1) as f64,
            accumulator: 0.0,
        }
    }

    pub fn ticks_per_second(&self) -> u64 {
        self.ticks_per_second as u64
    }

    pub fn set_ticks_per_second(&mut self, ticks_per_second: u64) {
        self.ticks_per_second = ticks_per_second.max(1) as f64;
        self.accumulator = 0.0;
    }

    pub fn reset(&mut self) {
        self.accumulator = 0.0;
    }

    /// Feed one frame delta and return how many whole ticks are due.
    ///
    /// The fractional remainder stays in the accumulator, so ticks are not lost
    /// when a frame spans less than one tick.
    pub fn ticks_for(&mut self, delta_seconds: f64) -> u64 {
        if !delta_seconds.is_finite() || delta_seconds <= 0.0 {
            return 0;
        }

        self.accumulator += delta_seconds * self.ticks_per_second;
        let whole = self.accumulator.floor();
        self.accumulator -= whole;

        if whole <= 0.0 {
            0
        } else {
            // Float-to-int casts saturate in Rust, so this is safe even for
            // pathological deltas (e.g. after a long debugger pause).
            whole as u64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_fractional_ticks() {
        let mut clock = TickClock::new(1000);
        assert_eq!(clock.ticks_for(0.0005), 0);
        assert_eq!(clock.ticks_for(0.0006), 1);
        assert_eq!(clock.ticks_for(0.002), 2);
    }

    #[test]
    fn ignores_invalid_deltas() {
        let mut clock = TickClock::new(1000);
        assert_eq!(clock.ticks_for(0.0), 0);
        assert_eq!(clock.ticks_for(-1.0), 0);
        assert_eq!(clock.ticks_for(f64::NAN), 0);
    }

    #[test]
    fn zero_tps_is_clamped() {
        let mut clock = TickClock::new(0);
        assert_eq!(clock.ticks_per_second(), 1);
        assert_eq!(clock.ticks_for(1.0), 1);
    }
}
