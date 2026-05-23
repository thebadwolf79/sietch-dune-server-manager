use serde::Serialize;

use crate::orchestration::{ProviderKind, StepAction, StepDomain};

/// Structured event emitted while an orchestration flow is running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationEvent {
    /// Stable step identifier.
    pub step_id: &'static str,
    /// User-facing message for the step.
    pub message: String,
    /// Operational domain the step belongs to.
    pub domain: StepDomain,
    /// Kind of action being performed.
    pub action: StepAction,
    /// Provider boundary responsible for the step.
    pub provider: ProviderKind,
}

/// Receives orchestration progress events.
pub trait OperationSink {
    /// Emits a single orchestration event.
    fn emit(&mut self, event: OrchestrationEvent);
}

/// Operation sink that stores all events in memory.
#[derive(Default)]
pub struct VecOperationSink {
    /// Events emitted so far.
    pub events: Vec<OrchestrationEvent>,
}

impl OperationSink for VecOperationSink {
    fn emit(&mut self, event: OrchestrationEvent) {
        self.events.push(event);
    }
}

pub(crate) fn emit_hyperv_event(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: impl Into<String>,
    domain: StepDomain,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.into(),
        domain,
        action,
        provider: ProviderKind::HyperV,
    });
}
