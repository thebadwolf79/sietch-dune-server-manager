use dune_manager_core::orchestration::{OperationSink, OrchestrationEvent};
use serde::Serialize;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationLogPayload {
    pub level: &'static str,
    pub scope: String,
    pub message: String,
}

pub struct TauriOperationSink {
    pub app: tauri::AppHandle,
}

impl TauriOperationSink {
    pub fn info(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "operation-log",
            OperationLogPayload {
                level: "info",
                scope: scope.into(),
                message: message.into(),
            },
        );
    }

    pub fn warn(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "operation-log",
            OperationLogPayload {
                level: "warn",
                scope: scope.into(),
                message: message.into(),
            },
        );
    }
}

impl OperationSink for TauriOperationSink {
    fn emit(&mut self, event: OrchestrationEvent) {
        self.info(event.step_id, event.message);
    }
}
