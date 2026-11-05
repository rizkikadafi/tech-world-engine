use thiserror::Error;
use tw_core::ComponentId;
use tw_sim::wiring::WiringError;

#[derive(Debug, Error)]
pub enum SceneError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unknown component kind '{kind}' for component #{component_id:?}")]
    UnknownComponentKind {
        kind: String,
        component_id: ComponentId,
    },

    #[error("failed to build component #{component_id:?} (kind '{kind}'): {reason}")]
    ComponentFactoryFailed {
        kind: String,
        component_id: ComponentId,
        reason: String,
    },

    #[error("component #{id:?} not found in simulation")]
    ComponentNotFound { id: ComponentId },

    #[error("port '{port}' not found on component #{component:?}")]
    PortNotFound {
        component: ComponentId,
        port: String,
    },

    #[error("wiring validation failed: {0:?}")]
    WiringFailed(Vec<WiringError>),

    #[error("invalid stimulus: {reason}")]
    InvalidStimulus { reason: String },
}
