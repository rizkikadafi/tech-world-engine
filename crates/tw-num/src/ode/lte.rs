//! Local Truncation Error (LTE) estimation and adaptive timestep control.
//!
//! Modeled after `CKTtrunc` and `CKTterr` in `ngspice-47/src/spicelib/analysis/ckttrunc.c`.

/// Configuration for adaptive timestep control based on LTE.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepControlConfig {
    /// Initial time step.
    pub dt_init: f64,
    /// Maximum allowed time step.
    pub dt_max: f64,
    /// Minimum allowed time step before giving up (`delmin`).
    pub dt_min: f64,
    /// Truncation error tolerance multiplier (matches ngspice `trtol = 7.0`).
    pub trtol: f64,
    /// Relative tolerance for LTE.
    pub reltol: f64,
    /// Absolute tolerance for LTE.
    pub abstol: f64,
    /// Maximum factor by which dt can grow in a single successful step (default: 2.0).
    pub max_growth_factor: f64,
    /// Maximum factor by which dt can shrink when LTE fails (default: 0.1).
    pub min_shrink_factor: f64,
}

impl Default for StepControlConfig {
    fn default() -> Self {
        Self {
            dt_init: 1e-9,
            dt_max: 1e-6,
            dt_min: 1e-15,
            trtol: 7.0,
            reltol: 1e-3,
            abstol: 1e-6,
            max_growth_factor: 2.0,
            min_shrink_factor: 0.1,
        }
    }
}

/// Result of evaluating LTE on a candidate step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepDecision {
    /// Step is accepted with recommended next dt.
    Accept { next_dt: f64 },
    /// Step is rejected due to excessive LTE; should retry with smaller dt.
    Reject { retry_dt: f64 },
}

/// Evaluates Backward Euler LTE given state history:
/// - `current`: accepted state at $t_n$
/// - `candidate`: newly converged state at $t_{n+1} = t_n + dt$
/// - `prev`: state at $t_{n-1}$ (if available)
/// - `dt`: current time step $t_{n+1} - t_n$
/// - `dt_old`: previous time step $t_n - t_{n-1}$ (if available)
pub fn evaluate_step_lte(
    candidate: &[f64],
    current: &[f64],
    prev: Option<&[f64]>,
    dt: f64,
    dt_old: Option<f64>,
    config: &StepControlConfig,
) -> StepDecision {
    if candidate.len() != current.len() {
        return StepDecision::Reject {
            retry_dt: (dt * 0.5).max(config.dt_min),
        };
    }

    // On the very first step after initial conditions, second derivatives are not yet
    // defined (matches ngspice dctran.c: "no check on first time point").
    let (prev_vals, h_prev) = match (prev, dt_old) {
        (Some(p), Some(h)) if h > 0.0 && p.len() == current.len() => (p, h),
        _ => return StepDecision::Accept { next_dt: dt },
    };

    let mut min_recommended_dt = dt * config.max_growth_factor;
    let mut step_acceptable = true;

    for i in 0..candidate.len() {
        let x_cand = candidate[i];
        let x_curr = current[i];

        // First difference: dx/dt at current step
        let diff_curr = (x_cand - x_curr) / dt;
        let diff_prev = (x_curr - prev_vals[i]) / h_prev;

        // Second derivative estimation for Backward Euler:
        // LTE = 0.5 * dt^2 * |d^2 x / dt^2|
        let lte = 0.5 * dt * (diff_curr - diff_prev).abs();

        let tol = config.reltol * x_curr.abs().max(x_cand.abs()) + config.abstol;
        let allowed_error = config.trtol * tol;

        if lte > allowed_error && lte > 1e-18 {
            step_acceptable = false;
            // Proposed dt from LTE: dt * sqrt(allowed_error / lte)
            let factor = (allowed_error / lte)
                .sqrt()
                .clamp(config.min_shrink_factor, 0.9);
            let target_dt = dt * factor;
            if target_dt < min_recommended_dt {
                min_recommended_dt = target_dt;
            }
        } else if lte > 1e-18 {
            let factor = (allowed_error / lte)
                .sqrt()
                .clamp(0.5, config.max_growth_factor);
            let target_dt = dt * factor;
            if target_dt < min_recommended_dt {
                min_recommended_dt = target_dt;
            }
        }
    }

    let next_dt = min_recommended_dt.clamp(config.dt_min, config.dt_max);

    if step_acceptable {
        StepDecision::Accept { next_dt }
    } else {
        StepDecision::Reject {
            retry_dt: next_dt.min(dt * 0.5).max(config.dt_min),
        }
    }
}
