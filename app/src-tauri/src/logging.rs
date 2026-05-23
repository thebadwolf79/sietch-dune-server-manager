use std::sync::Arc;

use dune_manager_core::orchestration::{OperationSink, OrchestrationEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::log_file::LogFile;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationLogPayload {
    pub level: &'static str,
    pub scope: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
}

pub struct TauriOperationSink {
    pub app: AppHandle,
    pub server_id: Option<String>,
}

impl TauriOperationSink {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            server_id: None,
        }
    }

    pub fn for_server(app: AppHandle, server_id: impl Into<String>) -> Self {
        Self {
            app,
            server_id: Some(server_id.into()),
        }
    }

    pub fn info(&self, scope: impl Into<String>, message: impl Into<String>) {
        self.emit_level("info", scope, message);
    }

    pub fn warn(&self, scope: impl Into<String>, message: impl Into<String>) {
        self.emit_level("warn", scope, message);
    }

    fn emit_level(
        &self,
        level: &'static str,
        scope: impl Into<String>,
        message: impl Into<String>,
    ) {
        let scope_text = scope.into();
        let message_text = message.into();
        let payload = OperationLogPayload {
            level,
            scope: scope_text.clone(),
            message: message_text.clone(),
            server_id: self.server_id.clone(),
        };
        let _ = self.app.emit("operation-log", &payload);
        if let Some(log_file) = self.app.try_state::<Arc<LogFile>>() {
            let _ = log_file.append(level, &scope_text, &message_text);
        }
    }
}

impl OperationSink for TauriOperationSink {
    fn emit(&mut self, event: OrchestrationEvent) {
        self.info(event.step_id, event.message);
    }
}
