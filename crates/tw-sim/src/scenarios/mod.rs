//! Reusable, side-effect free simulation scenarios.
//!
//! Scenarios build a [`Simulation`](crate::Simulation) (or engine) and expose
//! query helpers. They intentionally perform **no printing** so that the CLI,
//! integration tests, and examples can all share the exact same wiring.
//!
//! Keeping the wiring here means we do not duplicate simulation setup across
//! tests and demos, and the CLI never contains simulation logic itself.

pub mod counter;
pub mod digital;
pub mod resource_contention;
