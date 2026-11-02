//! Higher-level digital building blocks composed from CMOS gates.
//!
//! These are **not** behavioral models: every block is a `Subcircuit` made of
//! transistors, NAND/NOR gates, and capacitors. Truth tables and sequential
//! behavior therefore emerge from the circuit solver.
//!
//! - `cmos_half_adder`: XOR (sum) + AND (carry)
//! - `cmos_full_adder`: two half adders + OR
//! - `cmos_d_latch`: NAND-based gated D latch
//! - `cmos_dff`: master-slave positive-edge-triggered D flip-flop

use crate::circuit::Circuit;
use crate::error::CircuitError;
use crate::gates::{cmos_and, cmos_inverter, cmos_nand, cmos_or, cmos_xor, CmosGateConfig};
use crate::node::NodeId;
use crate::subcircuit::Subcircuit;

/// Creates a CMOS half adder.
///
/// Ports: `["A", "B", "SUM", "CARRY", "VDD", "GND"]`
pub fn cmos_half_adder(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_half_adder", ["A", "B", "SUM", "CARRY", "VDD", "GND"]);

    sub.add_subcircuit(
        "XOR1",
        cmos_xor(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "SUM"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    sub.add_subcircuit(
        "AND1",
        cmos_and(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "CARRY"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    sub
}

/// Creates a CMOS full adder from two half adders and an OR gate.
///
/// Ports: `["A", "B", "CIN", "SUM", "COUT", "VDD", "GND"]`
pub fn cmos_full_adder(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new(
        "cmos_full_adder",
        ["A", "B", "CIN", "SUM", "COUT", "VDD", "GND"],
    );

    // First half adder: A + B -> s1, c1
    sub.add_subcircuit(
        "HA1",
        cmos_half_adder(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("SUM", "s1"),
            ("CARRY", "c1"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Second half adder: s1 + CIN -> SUM, c2
    sub.add_subcircuit(
        "HA2",
        cmos_half_adder(config),
        &[
            ("A", "s1"),
            ("B", "CIN"),
            ("SUM", "SUM"),
            ("CARRY", "c2"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Carry out: c1 OR c2
    sub.add_subcircuit(
        "OR1",
        cmos_or(config),
        &[
            ("A", "c1"),
            ("B", "c2"),
            ("OUT", "COUT"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    sub
}

/// Creates a NAND-based gated D latch.
///
/// Ports: `["D", "EN", "Q", "Q_BAR", "VDD", "GND"]`
///
/// When `EN` is high the latch is transparent (`Q` follows `D`). When `EN`
/// is low the cross-coupled NAND pair holds the last value. Load capacitors
/// on `Q` and `Q_BAR` give the storage nodes defined charge.
pub fn cmos_d_latch(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_d_latch", ["D", "EN", "Q", "Q_BAR", "VDD", "GND"]);

    // D_BAR = NOT D
    sub.add_subcircuit(
        "INV_D",
        cmos_inverter(config),
        &[
            ("IN", "D"),
            ("OUT", "d_bar"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Active-low set/reset inputs to the cross-coupled NAND latch.
    sub.add_subcircuit(
        "NAND_S",
        cmos_nand(config),
        &[
            ("A", "D"),
            ("B", "EN"),
            ("OUT", "n_s"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    sub.add_subcircuit(
        "NAND_R",
        cmos_nand(config),
        &[
            ("A", "d_bar"),
            ("B", "EN"),
            ("OUT", "n_r"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Cross-coupled NAND storage pair.
    sub.add_subcircuit(
        "NAND_Q",
        cmos_nand(config),
        &[
            ("A", "n_s"),
            ("B", "Q_BAR"),
            ("OUT", "Q"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    sub.add_subcircuit(
        "NAND_QB",
        cmos_nand(config),
        &[
            ("A", "n_r"),
            ("B", "Q"),
            ("OUT", "Q_BAR"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Parasitic storage capacitance on the latch nodes.
    sub.add_capacitor("C_Q", "Q", "GND", config.load_capacitance);
    sub.add_capacitor("C_QB", "Q_BAR", "GND", config.load_capacitance);

    sub
}

/// Creates a master-slave positive-edge-triggered D flip-flop.
///
/// Ports: `["D", "CLK", "Q", "Q_BAR", "VDD", "GND"]`
///
/// The master latch is transparent while `CLK` is low; the slave latch is
/// transparent while `CLK` is high. On a rising clock edge the master holds
/// the value sampled just before the edge and the slave propagates it to `Q`.
pub fn cmos_dff(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_dff", ["D", "CLK", "Q", "Q_BAR", "VDD", "GND"]);

    // CLK_BAR = NOT CLK
    sub.add_subcircuit(
        "INV_CLK",
        cmos_inverter(config),
        &[
            ("IN", "CLK"),
            ("OUT", "clk_bar"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Master latch: transparent while CLK is low.
    sub.add_subcircuit(
        "MASTER",
        cmos_d_latch(config),
        &[
            ("D", "D"),
            ("EN", "clk_bar"),
            ("Q", "m_q"),
            ("Q_BAR", "m_q_bar"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Slave latch: transparent while CLK is high.
    sub.add_subcircuit(
        "SLAVE",
        cmos_d_latch(config),
        &[
            ("D", "m_q"),
            ("EN", "CLK"),
            ("Q", "Q"),
            ("Q_BAR", "Q_BAR"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    sub
}

impl Circuit {
    /// Adds a CMOS half adder connected to the given nodes.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cmos_half_adder(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        sum: NodeId,
        carry: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        cmos_half_adder(CmosGateConfig::default()).instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("SUM", sum),
                ("CARRY", carry),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a CMOS full adder connected to the given nodes.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cmos_full_adder(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        cin: NodeId,
        sum: NodeId,
        cout: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        cmos_full_adder(CmosGateConfig::default()).instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("CIN", cin),
                ("SUM", sum),
                ("COUT", cout),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a CMOS D latch connected to the given nodes.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cmos_d_latch(
        &mut self,
        instance_name: &str,
        d: NodeId,
        en: NodeId,
        q: NodeId,
        q_bar: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        cmos_d_latch(CmosGateConfig::default()).instantiate(
            self,
            instance_name,
            &[
                ("D", d),
                ("EN", en),
                ("Q", q),
                ("Q_BAR", q_bar),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a CMOS D flip-flop connected to the given nodes.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cmos_dff(
        &mut self,
        instance_name: &str,
        d: NodeId,
        clk: NodeId,
        q: NodeId,
        q_bar: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        cmos_dff(CmosGateConfig::default()).instantiate(
            self,
            instance_name,
            &[
                ("D", d),
                ("CLK", clk),
                ("Q", q),
                ("Q_BAR", q_bar),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }
}
