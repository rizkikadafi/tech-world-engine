//! SPICE-standard multi-layer convergence criteria.
//!
//! Modeled directly after `NIconvTest` in `ngspice-47/src/maths/ni/niconv.c`.
//!
//! Convergence requires that for each variable:
//! - For voltage variables: `|new - old| <= reltol * max(|old|, |new|) + vntol`
//! - For current variables: `|new - old| <= reltol * max(|old|, |new|) + abstol`

/// Configuration tolerances for SPICE convergence testing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvergenceConfig {
    /// Relative tolerance for all variables (default: 1e-3).
    pub reltol: f64,
    /// Absolute voltage tolerance (default: 1e-6 V).
    pub vntol: f64,
    /// Absolute current tolerance (default: 1e-12 A).
    pub abstol: f64,
    /// Maximum allowed Newton-Raphson iterations before reporting failure (default: 100).
    pub max_iter: usize,
}

impl Default for ConvergenceConfig {
    fn default() -> Self {
        Self {
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            max_iter: 100,
        }
    }
}

/// Variable type for convergence checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableType {
    Voltage,
    Current,
}

/// Evaluates convergence across old and new iteration states.
///
/// Returns `true` if all variables satisfy their tolerance thresholds.
pub fn check_convergence(
    old: &[f64],
    new: &[f64],
    var_types: &[VariableType],
    config: &ConvergenceConfig,
) -> bool {
    if old.len() != new.len() || old.len() != var_types.len() {
        return false;
    }

    for i in 0..new.len() {
        let v_old = old[i];
        let v_new = new[i];
        if v_new.is_nan() || v_old.is_nan() {
            return false;
        }

        let diff = (v_new - v_old).abs();
        let max_mag = v_old.abs().max(v_new.abs());

        let tol = match var_types[i] {
            VariableType::Voltage => config.reltol * max_mag + config.vntol,
            VariableType::Current => config.reltol * max_mag + config.abstol,
        };

        if diff > tol {
            return false;
        }
    }

    true
}
