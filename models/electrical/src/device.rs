use crate::node::NodeId;

/// Linear two-terminal resistor: V = I * R
#[derive(Debug, Clone, PartialEq)]
pub struct Resistor {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub resistance: f64,
}

impl Resistor {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, resistance: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            resistance,
        }
    }

    /// Conductance G = 1 / R
    #[inline]
    pub fn conductance(&self) -> f64 {
        1.0 / self.resistance
    }
}

/// Linear two-terminal capacitor: I = C * dV/dt
#[derive(Debug, Clone, PartialEq)]
pub struct Capacitor {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub capacitance: f64,
}

impl Capacitor {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, capacitance: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            capacitance,
        }
    }
}

/// Independent ideal voltage source: V_pos - V_neg = V
#[derive(Debug, Clone, PartialEq)]
pub struct VoltageSource {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub voltage: f64,
}

impl VoltageSource {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, voltage: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            voltage,
        }
    }
}

/// Independent ideal current source flowing from node_neg to node_pos
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentSource {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub current: f64,
}

impl CurrentSource {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, current: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            current,
        }
    }
}

/// Linearized companion model outputs for a nonlinear device at an operating point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionModel {
    /// Conductance between terminals (or primary dynamic conductance).
    pub g: f64,
    /// Transconductance for multi-terminal devices (e.g. gm for MOSFET).
    pub gm: f64,
    /// Equivalent history / linearization current entering positive or target terminal.
    pub i_eq: f64,
}

/// Semiconductor diode modeled using Shockley's equation with numerical limiting.
#[derive(Debug, Clone, PartialEq)]
pub struct Diode {
    pub name: String,
    pub node_pos: NodeId, // Anode
    pub node_neg: NodeId, // Cathode
    pub is_sat: f64,      // Saturation current (default ~1e-14 A)
    pub vt: f64,          // Thermal voltage (default ~0.02585 V at 300K)
    pub emission: f64,    // Emission coefficient (default 1.0)
}

impl Diode {
    pub fn new(name: impl Into<String>, anode: NodeId, cathode: NodeId) -> Self {
        Self {
            name: name.into(),
            node_pos: anode,
            node_neg: cathode,
            is_sat: 1e-14,
            vt: 0.02585,
            emission: 1.0,
        }
    }

    pub fn with_params(
        name: impl Into<String>,
        anode: NodeId,
        cathode: NodeId,
        is_sat: f64,
        vt: f64,
        emission: f64,
    ) -> Self {
        Self {
            name: name.into(),
            node_pos: anode,
            node_neg: cathode,
            is_sat,
            vt,
            emission,
        }
    }

    /// Evaluates diode current $I_D$ and dynamic conductance $g_d = dI_D/dV_D$
    /// with smooth linear extrapolation beyond $V_{crit}$ to prevent floating-point overflow.
    pub fn companion(&self, vd: f64) -> CompanionModel {
        let n_vt = self.emission * self.vt;
        let v_crit = (n_vt * (n_vt / (2.0f64.sqrt() * self.is_sat)).ln()).clamp(0.6, 0.85);
        let gmin = 1e-12;

        let (id, gd) = if vd <= v_crit {
            let exp_arg = (vd / n_vt).clamp(-50.0, 50.0);
            let exp_term = exp_arg.exp();
            let id = self.is_sat * (exp_term - 1.0);
            let gd = (self.is_sat / n_vt) * exp_term + gmin;
            (id, gd)
        } else {
            // Linear extrapolation beyond v_crit
            let exp_term = (v_crit / n_vt).exp();
            let id_crit = self.is_sat * (exp_term - 1.0);
            let gd_crit = (self.is_sat / n_vt) * exp_term + gmin;
            let id = id_crit + gd_crit * (vd - v_crit);
            (id, gd_crit)
        };

        // Linearized: I_D = gd * vd - i_eq  =>  i_eq = gd * vd - id
        let i_eq = gd * vd - id;
        CompanionModel {
            g: gd,
            gm: 0.0,
            i_eq,
        }
    }
}

/// Type of MOSFET transistor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MosfetType {
    Nmos,
    Pmos,
}

/// Level 1 MOSFET transistor (Shichman-Hodges model).
#[derive(Debug, Clone, PartialEq)]
pub struct Mosfet {
    pub name: String,
    pub mos_type: MosfetType,
    pub node_drain: NodeId,
    pub node_gate: NodeId,
    pub node_source: NodeId,
    pub vth: f64,    // Threshold voltage: >0 for NMOS, <0 for PMOS
    pub beta: f64,   // Transconductance parameter Kp * (W / L) in A/V^2
    pub lambda: f64, // Channel length modulation (1/V)
}

impl Mosfet {
    pub fn new_nmos(
        name: impl Into<String>,
        drain: NodeId,
        gate: NodeId,
        source: NodeId,
        vth: f64,
        beta: f64,
    ) -> Self {
        Self {
            name: name.into(),
            mos_type: MosfetType::Nmos,
            node_drain: drain,
            node_gate: gate,
            node_source: source,
            vth: vth.abs(), // positive for NMOS
            beta,
            lambda: 0.02,
        }
    }

    pub fn new_pmos(
        name: impl Into<String>,
        drain: NodeId,
        gate: NodeId,
        source: NodeId,
        vth: f64,
        beta: f64,
    ) -> Self {
        Self {
            name: name.into(),
            mos_type: MosfetType::Pmos,
            node_drain: drain,
            node_gate: gate,
            node_source: source,
            vth: -vth.abs(), // negative for PMOS
            beta,
            lambda: 0.02,
        }
    }

    pub fn with_lambda(mut self, lambda: f64) -> Self {
        self.lambda = lambda;
        self
    }

    /// Evaluates drain current $I_D$, transconductance $g_m = \partial I_D / \partial V_{GS}$,
    /// and output conductance $g_{ds} = \partial I_D / \partial V_{DS}$.
    pub fn companion(&self, vg: f64, vd: f64, vs: f64) -> CompanionModel {
        let gmin = 1e-12;
        match self.mos_type {
            MosfetType::Nmos => {
                let vgs = vg - vs;
                let vds = (vd - vs).max(0.0); // positive Vds convention
                let vth = self.vth;

                if vgs <= vth {
                    // Cutoff
                    CompanionModel {
                        g: gmin,
                        gm: 0.0,
                        i_eq: gmin * vds,
                    }
                } else {
                    let vov = vgs - vth;
                    let (ids, gm, gds) = if vds < vov {
                        // Linear / Triode
                        let ids = self.beta * (vov * vds - 0.5 * vds * vds) * (1.0 + self.lambda * vds);
                        let gm = self.beta * vds * (1.0 + self.lambda * vds);
                        let gds = self.beta * (vov - vds) * (1.0 + self.lambda * vds)
                            + self.lambda * self.beta * (vov * vds - 0.5 * vds * vds)
                            + gmin;
                        (ids, gm, gds)
                    } else {
                        // Saturation
                        let ids = 0.5 * self.beta * vov * vov * (1.0 + self.lambda * vds);
                        let gm = self.beta * vov * (1.0 + self.lambda * vds);
                        let gds = 0.5 * self.lambda * self.beta * vov * vov + gmin;
                        (ids, gm, gds)
                    };

                    let i_eq = gm * vgs + gds * vds - ids;
                    CompanionModel { g: gds, gm, i_eq }
                }
            }
            MosfetType::Pmos => {
                let vsg = vs - vg;
                let vsd = (vs - vd).max(0.0); // positive Vsd convention
                let abs_vth = self.vth.abs();

                if vsg <= abs_vth {
                    // Cutoff
                    CompanionModel {
                        g: gmin,
                        gm: 0.0,
                        i_eq: gmin * vsd,
                    }
                } else {
                    let vov = vsg - abs_vth;
                    let (isd, gm, gds) = if vsd < vov {
                        // Linear / Triode
                        let isd = self.beta * (vov * vsd - 0.5 * vsd * vsd) * (1.0 + self.lambda * vsd);
                        let gm = self.beta * vsd * (1.0 + self.lambda * vsd);
                        let gds = self.beta * (vov - vsd) * (1.0 + self.lambda * vsd)
                            + self.lambda * self.beta * (vov * vsd - 0.5 * vsd * vsd)
                            + gmin;
                        (isd, gm, gds)
                    } else {
                        // Saturation
                        let isd = 0.5 * self.beta * vov * vov * (1.0 + self.lambda * vsd);
                        let gm = self.beta * vov * (1.0 + self.lambda * vsd);
                        let gds = 0.5 * self.lambda * self.beta * vov * vov + gmin;
                        (isd, gm, gds)
                    };

                    let i_eq = gm * vsg + gds * vsd - isd;
                    CompanionModel { g: gds, gm, i_eq }
                }
            }
        }
    }
}

