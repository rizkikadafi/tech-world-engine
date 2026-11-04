//! Mapping between physical time (seconds) and discrete simulation ticks.
//!
//! `tw-core` deliberately models time as opaque `u64` ticks. Domain models that
//! measure physical quantities (propagation delay, rise time, ...) need an
//! explicit conversion, which is what [`TimeBase`] provides.

use crate::error::CircuitError;

/// Physical duration of one discrete simulation tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeBase {
    /// Seconds represented by a single simulation tick.
    pub seconds_per_tick: f64,
}

impl TimeBase {
    /// One nanosecond per tick, a convenient default for CMOS digital logic.
    pub const NANOSECOND: Self = Self {
        seconds_per_tick: 1e-9,
    };

    /// Creates a time base from a duration in seconds.
    pub fn new(seconds_per_tick: f64) -> Self {
        Self { seconds_per_tick }
    }

    /// Rejects non-finite or non-positive time bases.
    pub fn validate(&self) -> Result<(), CircuitError> {
        if !self.seconds_per_tick.is_finite() || self.seconds_per_tick <= 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "TimeBase seconds_per_tick must be finite and positive, got {}",
                self.seconds_per_tick
            )));
        }
        Ok(())
    }

    /// Converts a physical duration to whole ticks, rounding up.
    ///
    /// A non-positive or non-finite duration maps to one tick: a derived
    /// component must never produce a zero-delay (same-tick) transition.
    pub fn delay_ticks(&self, seconds: f64) -> u64 {
        if !seconds.is_finite() || seconds <= 0.0 {
            return 1;
        }
        let ticks = (seconds / self.seconds_per_tick).ceil();
        if ticks < 1.0 {
            1
        } else {
            ticks as u64
        }
    }

    /// Converts whole ticks to a physical duration in seconds.
    pub fn ticks_to_seconds(&self, ticks: u64) -> f64 {
        ticks as f64 * self.seconds_per_tick
    }
}

impl Default for TimeBase {
    fn default() -> Self {
        Self::NANOSECOND
    }
}
