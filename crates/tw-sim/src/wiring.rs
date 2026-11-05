//! Validation for component wiring.
//!
//! The engine routes [`EventPayload::PortSignal`](tw_core::EventPayload::PortSignal)
//! events by matching `Connection`s. A typo in a component id or port id would
//! otherwise make the signal disappear silently, so [`Simulation::validate_wiring`]
//! reports every problem at once before the simulation starts.
//!
//! [`Simulation::validate_wiring`]: crate::Simulation::validate_wiring

use std::collections::HashMap;
use std::fmt;

use tw_core::{Component, ComponentId, Connection, PortDirection, PortId};

/// A wiring problem found by [`Simulation::validate_wiring`](crate::Simulation::validate_wiring).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WiringError {
    /// The connection's source component is not registered.
    UnknownSourceComponent { connection: Connection },
    /// The connection's target component is not registered.
    UnknownTargetComponent { connection: Connection },
    /// The source component does not declare the connection's source port.
    UnknownSourcePort {
        component: ComponentId,
        port: PortId,
    },
    /// The target component does not declare the connection's target port.
    UnknownTargetPort {
        component: ComponentId,
        port: PortId,
    },
    /// The source port cannot emit signals (it is an input).
    SourceNotOutput {
        component: ComponentId,
        port: PortId,
        direction: PortDirection,
    },
    /// The target port cannot receive signals (it is an output).
    TargetNotInput {
        component: ComponentId,
        port: PortId,
        direction: PortDirection,
    },
    /// The exact same connection was registered more than once.
    DuplicateConnection { connection: Connection },
}

impl fmt::Display for WiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WiringError::UnknownSourceComponent { connection } => write!(
                f,
                "connection source component #{} is not registered",
                connection.source_component.0
            ),
            WiringError::UnknownTargetComponent { connection } => write!(
                f,
                "connection target component #{} is not registered",
                connection.target_component.0
            ),
            WiringError::UnknownSourcePort { component, port } => write!(
                f,
                "component #{} does not declare source port #{}",
                component.0, port.0
            ),
            WiringError::UnknownTargetPort { component, port } => write!(
                f,
                "component #{} does not declare target port #{}",
                component.0, port.0
            ),
            WiringError::SourceNotOutput {
                component,
                port,
                direction,
            } => write!(
                f,
                "source port #{} on component #{} is {direction:?}, not an output",
                port.0, component.0
            ),
            WiringError::TargetNotInput {
                component,
                port,
                direction,
            } => write!(
                f,
                "target port #{} on component #{} is {direction:?}, not an input",
                port.0, component.0
            ),
            WiringError::DuplicateConnection { connection } => write!(
                f,
                "duplicate connection #{}:{} -> #{}:{}",
                connection.source_component.0,
                connection.source_port.0,
                connection.target_component.0,
                connection.target_port.0
            ),
        }
    }
}

impl std::error::Error for WiringError {}

pub(crate) fn validate(
    components: &HashMap<ComponentId, Box<dyn Component>>,
    connections: &[Connection],
) -> Result<(), Vec<WiringError>> {
    let mut errors = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for connection in connections {
        let key = (
            connection.source_component,
            connection.source_port,
            connection.target_component,
            connection.target_port,
        );
        if !seen.insert(key) {
            errors.push(WiringError::DuplicateConnection {
                connection: connection.clone(),
            });
        }

        let source = match components.get(&connection.source_component) {
            Some(component) => Some(component),
            None => {
                errors.push(WiringError::UnknownSourceComponent {
                    connection: connection.clone(),
                });
                None
            }
        }
        .and_then(|component| {
            component
                .ports()
                .iter()
                .find(|port| port.id == connection.source_port)
                .map(|port| (component, port))
        });

        match source {
            Some((_, port)) => {
                if !matches!(
                    port.direction,
                    PortDirection::Output | PortDirection::Bidirectional
                ) {
                    errors.push(WiringError::SourceNotOutput {
                        component: connection.source_component,
                        port: connection.source_port,
                        direction: port.direction,
                    });
                }
            }
            None if components.contains_key(&connection.source_component) => {
                errors.push(WiringError::UnknownSourcePort {
                    component: connection.source_component,
                    port: connection.source_port,
                });
            }
            None => {}
        }

        let target = match components.get(&connection.target_component) {
            Some(component) => Some(component),
            None => {
                errors.push(WiringError::UnknownTargetComponent {
                    connection: connection.clone(),
                });
                None
            }
        }
        .and_then(|component| {
            component
                .ports()
                .iter()
                .find(|port| port.id == connection.target_port)
                .map(|port| (component, port))
        });

        match target {
            Some((_, port)) => {
                if !matches!(
                    port.direction,
                    PortDirection::Input | PortDirection::Bidirectional
                ) {
                    errors.push(WiringError::TargetNotInput {
                        component: connection.target_component,
                        port: connection.target_port,
                        direction: port.direction,
                    });
                }
            }
            None if components.contains_key(&connection.target_component) => {
                errors.push(WiringError::UnknownTargetPort {
                    component: connection.target_component,
                    port: connection.target_port,
                });
            }
            None => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
