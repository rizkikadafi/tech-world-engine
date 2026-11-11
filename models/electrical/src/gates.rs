//! Standard CMOS logic gates built from transistor physics models.
//!
//! Provides templates and instantiation helpers for:
//! - CMOS Inverter (NOT)
//! - CMOS NAND (2-input)
//! - CMOS NOR (2-input)
//! - CMOS XOR (2-input)
//! - CMOS AND (2-input, compound NAND + NOT)
//! - CMOS OR (2-input, compound NOR + NOT)

use serde::{Deserialize, Serialize};

use crate::circuit::Circuit;
use crate::error::CircuitError;
use crate::node::NodeId;
use crate::subcircuit::Subcircuit;

/// Physical and electrical configuration for CMOS logic gates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CmosGateConfig {
    /// Transistor threshold voltage |Vth| (V)
    pub vth: f64,
    /// NMOS transconductance parameter beta (A/V^2)
    pub beta_n: f64,
    /// PMOS transconductance parameter beta (A/V^2)
    pub beta_p: f64,
    /// Parasitic/load capacitance added to storage nodes in sequential logic (F).
    ///
    /// Combinational gates ignore this field; D latches and flip-flops use it
    /// to give their internal nodes a defined amount of charge storage.
    pub load_capacitance: f64,
}

impl Default for CmosGateConfig {
    fn default() -> Self {
        Self {
            vth: 0.8,
            beta_n: 1e-3,
            beta_p: 1e-3,
            load_capacitance: 1e-12,
        }
    }
}

impl CmosGateConfig {
    /// Returns a copy of this configuration with a different load capacitance.
    pub fn with_load_capacitance(mut self, load_capacitance: f64) -> Self {
        self.load_capacitance = load_capacitance;
        self
    }
}

/// Creates a CMOS Inverter subcircuit template.
///
/// Ports: `["IN", "OUT", "VDD", "GND"]`
pub fn cmos_inverter(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_inverter", ["IN", "OUT", "VDD", "GND"]);
    // PMOS pull-up: Source = VDD, Drain = OUT, Gate = IN
    sub.add_pmos("Mp", "OUT", "IN", "VDD", config.vth, config.beta_p);
    // NMOS pull-down: Drain = OUT, Source = GND, Gate = IN
    sub.add_nmos("Mn", "OUT", "IN", "GND", config.vth, config.beta_n);
    sub
}

/// Creates a 2-input CMOS NAND subcircuit template.
///
/// Ports: `["A", "B", "OUT", "VDD", "GND"]`
///
/// Topology:
/// - 2 PMOS in parallel between VDD and OUT
/// - 2 NMOS in series between OUT and GND (connected via internal node "n_mid")
pub fn cmos_nand(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_nand", ["A", "B", "OUT", "VDD", "GND"]);

    // Parallel PMOS pull-ups
    sub.add_pmos("Mp_a", "OUT", "A", "VDD", config.vth, config.beta_p);
    sub.add_pmos("Mp_b", "OUT", "B", "VDD", config.vth, config.beta_p);

    // Series NMOS pull-downs (sized with 2*beta_n to match drive strength)
    sub.add_nmos("Mn_a", "OUT", "A", "n_mid", config.vth, config.beta_n * 2.0);
    sub.add_nmos("Mn_b", "n_mid", "B", "GND", config.vth, config.beta_n * 2.0);

    sub
}

/// Creates a 2-input CMOS NOR subcircuit template.
///
/// Ports: `["A", "B", "OUT", "VDD", "GND"]`
///
/// Topology:
/// - 2 PMOS in series between VDD and OUT (connected via internal node "p_mid")
/// - 2 NMOS in parallel between OUT and GND
pub fn cmos_nor(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_nor", ["A", "B", "OUT", "VDD", "GND"]);

    // Series PMOS pull-ups (sized with 2*beta_p to match drive strength)
    sub.add_pmos("Mp_a", "p_mid", "A", "VDD", config.vth, config.beta_p * 2.0);
    sub.add_pmos("Mp_b", "OUT", "B", "p_mid", config.vth, config.beta_p * 2.0);

    // Parallel NMOS pull-downs
    sub.add_nmos("Mn_a", "OUT", "A", "GND", config.vth, config.beta_n);
    sub.add_nmos("Mn_b", "OUT", "B", "GND", config.vth, config.beta_n);

    sub
}

/// Creates a 2-input CMOS AND compound subcircuit (NAND + Inverter).
///
/// Ports: `["A", "B", "OUT", "VDD", "GND"]`
pub fn cmos_and(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_and", ["A", "B", "OUT", "VDD", "GND"]);

    // NAND stage: drives internal node "nand_out"
    sub.add_subcircuit(
        "NAND1",
        cmos_nand(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "nand_out"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Inverter stage: inverts "nand_out" to "OUT"
    sub.add_subcircuit(
        "INV1",
        cmos_inverter(config),
        &[
            ("IN", "nand_out"),
            ("OUT", "OUT"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    sub
}

/// Creates a 2-input CMOS OR compound subcircuit (NOR + Inverter).
///
/// Ports: `["A", "B", "OUT", "VDD", "GND"]`
pub fn cmos_or(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_or", ["A", "B", "OUT", "VDD", "GND"]);

    // NOR stage: drives internal node "nor_out"
    sub.add_subcircuit(
        "NOR1",
        cmos_nor(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "nor_out"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    // Inverter stage: inverts "nor_out" to "OUT"
    sub.add_subcircuit(
        "INV1",
        cmos_inverter(config),
        &[
            ("IN", "nor_out"),
            ("OUT", "OUT"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    sub
}

/// Creates a 2-input CMOS XOR subcircuit from four NAND gates.
///
/// Ports: `["A", "B", "OUT", "VDD", "GND"]`
///
/// Topology:
/// ```text
/// n1  = NAND(A, B)
/// n2  = NAND(A, n1)
/// n3  = NAND(B, n1)
/// OUT = NAND(n2, n3)
/// ```
pub fn cmos_xor(config: CmosGateConfig) -> Subcircuit {
    let mut sub = Subcircuit::new("cmos_xor", ["A", "B", "OUT", "VDD", "GND"]);

    sub.add_subcircuit(
        "NAND_AB",
        cmos_nand(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "n1"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    sub.add_subcircuit(
        "NAND_A",
        cmos_nand(config),
        &[
            ("A", "A"),
            ("B", "n1"),
            ("OUT", "n2"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    sub.add_subcircuit(
        "NAND_B",
        cmos_nand(config),
        &[
            ("A", "B"),
            ("B", "n1"),
            ("OUT", "n3"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    sub.add_subcircuit(
        "NAND_OUT",
        cmos_nand(config),
        &[
            ("A", "n2"),
            ("B", "n3"),
            ("OUT", "OUT"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    sub
}

impl Circuit {
    /// Adds a CMOS Inverter gate connected to the given nodes.
    pub fn add_cmos_inverter(
        &mut self,
        instance_name: &str,
        in_node: NodeId,
        out_node: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        let sub = cmos_inverter(CmosGateConfig::default());
        sub.instantiate(
            self,
            instance_name,
            &[
                ("IN", in_node),
                ("OUT", out_node),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a 2-input CMOS NAND gate connected to the given nodes.
    pub fn add_cmos_nand(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        out_node: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        let sub = cmos_nand(CmosGateConfig::default());
        sub.instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("OUT", out_node),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a 2-input CMOS NOR gate connected to the given nodes.
    pub fn add_cmos_nor(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        out_node: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        let sub = cmos_nor(CmosGateConfig::default());
        sub.instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("OUT", out_node),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a 2-input CMOS AND gate connected to the given nodes.
    pub fn add_cmos_and(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        out_node: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        let sub = cmos_and(CmosGateConfig::default());
        sub.instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("OUT", out_node),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a 2-input CMOS OR gate connected to the given nodes.
    pub fn add_cmos_or(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        out_node: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        let sub = cmos_or(CmosGateConfig::default());
        sub.instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("OUT", out_node),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }

    /// Adds a 2-input CMOS XOR gate connected to the given nodes.
    pub fn add_cmos_xor(
        &mut self,
        instance_name: &str,
        a: NodeId,
        b: NodeId,
        out_node: NodeId,
        vdd: NodeId,
        gnd: NodeId,
    ) -> Result<(), CircuitError> {
        let sub = cmos_xor(CmosGateConfig::default());
        sub.instantiate(
            self,
            instance_name,
            &[
                ("A", a),
                ("B", b),
                ("OUT", out_node),
                ("VDD", vdd),
                ("GND", gnd),
            ],
        )
    }
}
