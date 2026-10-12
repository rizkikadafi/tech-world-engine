use crate::component::{Component, ComponentId, Port, PortBinding, PortDirection};
use crate::connection::Connection;
use crate::context::SimulationContext;
use crate::event::{Event, EventPayload};
use crate::state::State;
use crate::tracing::TracingMode;
use std::collections::{HashMap, HashSet};

/// A wiring problem found while validating a [`CompositeComponent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeError {
    /// A binding refers to an external port the composite does not declare.
    UnknownExternalPort { external_port: crate::component::PortId },
    /// A binding or internal connection names a subcomponent that does not exist.
    UnknownSubcomponent { component: ComponentId },
    /// A binding or internal connection names a port the subcomponent does not declare.
    UnknownSubcomponentPort {
        component: ComponentId,
        port: crate::component::PortId,
    },
    /// The external and internal ports carry signals in incompatible directions.
    DirectionMismatch {
        external_port: crate::component::PortId,
        internal_component: ComponentId,
        internal_port: crate::component::PortId,
    },
    /// The exact same binding was declared more than once.
    DuplicateBinding {
        external_port: crate::component::PortId,
        internal_component: ComponentId,
        internal_port: crate::component::PortId,
    },
    /// An internal connection references an unknown subcomponent or port.
    UnknownConnectionEndpoint {
        component: ComponentId,
        port: crate::component::PortId,
    },
}

/// A component built from other components.
///
/// **Hierarchy lives inside the composite.** The engine only sees top-level
/// components: it dispatches events to the composite, which routes them to
/// subcomponents over [`Connection`]s and `PortBinding`s, then resolves the
/// subcomponents' outputs back out. Subcomponents are never registered with the
/// engine directly.
#[derive(Debug)]
pub struct CompositeComponent {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    subcomponents: HashMap<ComponentId, Box<dyn Component>>,
    internal_connections: Vec<Connection>,
    port_bindings: Vec<PortBinding>,
}

impl CompositeComponent {
    pub fn new<S: Into<String>>(id: ComponentId, name: S) -> Self {
        Self {
            id,
            name: name.into(),
            ports: Vec::new(),
            state: State::new(),
            subcomponents: HashMap::new(),
            internal_connections: Vec::new(),
            port_bindings: Vec::new(),
        }
    }

    pub fn add_port(&mut self, port: Port) {
        self.ports.push(port);
    }

    pub fn add_subcomponent(&mut self, component: Box<dyn Component>) {
        self.subcomponents.insert(component.id(), component);
    }

    pub fn add_internal_connection(&mut self, connection: Connection) {
        self.internal_connections.push(connection);
    }

    pub fn bind_port(&mut self, binding: PortBinding) {
        self.port_bindings.push(binding);
    }

    pub fn subcomponents(&self) -> &HashMap<ComponentId, Box<dyn Component>> {
        &self.subcomponents
    }

    pub fn internal_connections(&self) -> &[Connection] {
        &self.internal_connections
    }

    pub fn port_bindings(&self) -> &[PortBinding] {
        &self.port_bindings
    }

    /// Validate the composite's wiring.
    ///
    /// Checks that every binding and internal connection refers to declared
    /// external ports, existing subcomponents, and ports those subcomponents
    /// actually expose; that bound ports carry compatible signal directions; and
    /// that no binding is declared twice. Returns every problem found so callers
    /// can report all wiring errors at once.
    pub fn validate(&self) -> Result<(), Vec<CompositeError>> {
        let mut errors = Vec::new();

        let external_direction = |port: crate::component::PortId| {
            self.ports
                .iter()
                .find(|p| p.id == port)
                .map(|p| p.direction)
        };

        let subcomponent_port = |component: ComponentId, port: crate::component::PortId| {
            self.subcomponents
                .get(&component)
                .and_then(|sub| sub.ports().iter().find(|p| p.id == port))
                .map(|p| p.direction)
        };

        let mut seen_bindings = HashSet::new();
        for binding in &self.port_bindings {
            let key = (
                binding.external_port,
                binding.internal_component,
                binding.internal_port,
            );
            if !seen_bindings.insert(key) {
                errors.push(CompositeError::DuplicateBinding {
                    external_port: binding.external_port,
                    internal_component: binding.internal_component,
                    internal_port: binding.internal_port,
                });
            }

            let Some(external_direction) = external_direction(binding.external_port) else {
                errors.push(CompositeError::UnknownExternalPort {
                    external_port: binding.external_port,
                });
                continue;
            };

            if !self.subcomponents.contains_key(&binding.internal_component) {
                errors.push(CompositeError::UnknownSubcomponent {
                    component: binding.internal_component,
                });
                continue;
            }

            let Some(internal_direction) =
                subcomponent_port(binding.internal_component, binding.internal_port)
            else {
                errors.push(CompositeError::UnknownSubcomponentPort {
                    component: binding.internal_component,
                    port: binding.internal_port,
                });
                continue;
            };

            let compatible = match external_direction {
                PortDirection::Bidirectional => true,
                PortDirection::Input => matches!(
                    internal_direction,
                    PortDirection::Input | PortDirection::Bidirectional
                ),
                PortDirection::Output => matches!(
                    internal_direction,
                    PortDirection::Output | PortDirection::Bidirectional
                ),
            };
            if !compatible {
                errors.push(CompositeError::DirectionMismatch {
                    external_port: binding.external_port,
                    internal_component: binding.internal_component,
                    internal_port: binding.internal_port,
                });
            }
        }

        for connection in &self.internal_connections {
            if !self.subcomponents.contains_key(&connection.source_component)
                || self
                    .subcomponent_port_exists(connection.source_component, connection.source_port)
                    .is_none()
            {
                errors.push(CompositeError::UnknownConnectionEndpoint {
                    component: connection.source_component,
                    port: connection.source_port,
                });
            }
            if !self.subcomponents.contains_key(&connection.target_component)
                || self
                    .subcomponent_port_exists(connection.target_component, connection.target_port)
                    .is_none()
            {
                errors.push(CompositeError::UnknownConnectionEndpoint {
                    component: connection.target_component,
                    port: connection.target_port,
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn subcomponent_port_exists(
        &self,
        component: ComponentId,
        port: crate::component::PortId,
    ) -> Option<()> {
        self.subcomponents
            .get(&component)
            .and_then(|sub| sub.ports().iter().find(|p| p.id == port))
            .map(|_| ())
    }

    /// Resolve events emitted by a subcomponent.
    ///
    /// 1. If the subcomponent output port is bound to an **external** output port
    ///    → emit an external PortSignal (source = composite id, target = external consumer).
    /// 2. If the subcomponent output port has an **internal connection** to another subcomponent
    ///    → dispatch the event to that subcomponent and recurse.
    fn resolve_sub_outputs(
        &mut self,
        sub_events: Vec<Event>,
        ctx: &mut SimulationContext,
    ) -> Vec<Event> {
        let mut external_outputs = Vec::new();

        for sub_event in sub_events {
            if let EventPayload::PortSignal { port_id, ref value } = sub_event.payload {
                let sub_exec_trace_id = if ctx.tracing_mode() == TracingMode::Hierarchical {
                    Some(ctx.record_trace(
                        sub_event.id,
                        format!(
                            "Subcomponent #{} emitted Output(Port #{}) value='{}'",
                            sub_event.source.0, port_id.0, value
                        ),
                        sub_event.cause,
                    ))
                } else {
                    sub_event.cause
                };

                // --- 1. Check external output port bindings ---
                let mut ext_ports = Vec::new();
                for binding in &self.port_bindings {
                    if binding.internal_component == sub_event.source
                        && binding.internal_port == port_id
                    {
                        ext_ports.push(binding.external_port);
                    }
                }
                for ext_port in ext_ports {
                    let ext_cause = if ctx.tracing_mode() == TracingMode::Hierarchical {
                        Some(ctx.record_trace(
                            sub_event.id,
                            format!(
                                "Composite '{}' (#{}) bound Subcomponent #{} Output(Port #{}) to External Port #{}",
                                self.name, self.id.0, sub_event.source.0, port_id.0, ext_port.0
                            ),
                            sub_exec_trace_id,
                        ))
                    } else {
                        sub_exec_trace_id
                    };

                    let mut ext_event = Event::new(
                        sub_event.id,
                        sub_event.time,
                        self.id,
                        self.id,
                        EventPayload::PortSignal {
                            port_id: ext_port,
                            value: value.clone(),
                        },
                    );
                    ext_event.cause = ext_cause;
                    external_outputs.push(ext_event);
                }

                // --- 2. Forward through internal connections ---
                let mut internal_targets = Vec::new();
                for conn in &self.internal_connections {
                    if conn.source_component == sub_event.source && conn.source_port == port_id {
                        internal_targets.push((conn.target_component, conn.target_port));
                    }
                }

                for (target_id, target_port) in internal_targets {
                    let fwd_cause = if ctx.tracing_mode() == TracingMode::Hierarchical {
                        Some(ctx.record_trace(
                            sub_event.id,
                            format!(
                                "Composite '{}' (#{}) routed Internal Connection from Subcomponent #{} to Subcomponent #{} Port #{}",
                                self.name, self.id.0, sub_event.source.0, target_id.0, target_port.0
                            ),
                            sub_exec_trace_id,
                        ))
                    } else {
                        sub_exec_trace_id
                    };

                    if let Some(target_sub) = self.subcomponents.get_mut(&target_id) {
                        let mut fwd_event = Event::new(
                            sub_event.id,
                            sub_event.time,
                            sub_event.source,
                            target_id,
                            EventPayload::PortSignal {
                                port_id: target_port,
                                value: value.clone(),
                            },
                        );
                        fwd_event.cause = fwd_cause;
                        let mut further = target_sub.handle_event(&fwd_event, ctx);
                        for out in &mut further {
                            if out.cause.is_none() {
                                out.cause = fwd_cause;
                            }
                        }
                        external_outputs.extend(self.resolve_sub_outputs(further, ctx));
                    }
                }
            }
        }

        external_outputs
    }
}

impl Component for CompositeComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut SimulationContext) -> Vec<Event> {
        let mut output_events = Vec::new();

        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            let mut bindings_to_trigger = Vec::new();
            for binding in &self.port_bindings {
                if binding.external_port == port_id {
                    bindings_to_trigger.push((binding.internal_component, binding.internal_port));
                }
            }

            for (internal_comp_id, internal_port_id) in bindings_to_trigger {
                let route_cause = if ctx.tracing_mode() == TracingMode::Hierarchical {
                    Some(ctx.record_trace(
                        event.id,
                        format!(
                            "Composite '{}' (#{}) routed External Input Port #{} to Subcomponent #{} Port #{}",
                            self.name, self.id.0, port_id.0, internal_comp_id.0, internal_port_id.0
                        ),
                        event.cause,
                    ))
                } else {
                    event.cause
                };

                if let Some(subcomponent) = self.subcomponents.get_mut(&internal_comp_id) {
                    let mut sub_event = Event::new(
                        event.id,
                        event.time,
                        event.source,
                        internal_comp_id,
                        EventPayload::PortSignal {
                            port_id: internal_port_id,
                            value: value.clone(),
                        },
                    );
                    sub_event.cause = route_cause;
                    let mut sub_outputs = subcomponent.handle_event(&sub_event, ctx);
                    for out in &mut sub_outputs {
                        if out.cause.is_none() {
                            out.cause = route_cause;
                        }
                    }
                    output_events.extend(self.resolve_sub_outputs(sub_outputs, ctx));
                }
            }
        } else {
            let mut all_sub_outputs = Vec::new();
            for subcomponent in self.subcomponents.values_mut() {
                let mut sub_outputs = subcomponent.handle_event(event, ctx);
                for out in &mut sub_outputs {
                    if out.cause.is_none() {
                        out.cause = event.cause;
                    }
                }
                all_sub_outputs.extend(sub_outputs);
            }
            output_events.extend(self.resolve_sub_outputs(all_sub_outputs, ctx));
        }

        output_events
    }

    fn state(&self) -> &State {
        &self.state
    }
}
