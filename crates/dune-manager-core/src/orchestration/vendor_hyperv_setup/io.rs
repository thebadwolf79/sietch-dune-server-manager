//! Background readers, line buffering, redaction, and sink helpers for the vendor flow.

use std::{fs, io::Read, path::Path, sync::mpsc, thread};

use sha2::{Digest, Sha256};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{OperationSink, OrchestrationEvent, ProviderKind, StepAction, StepDomain},
};

#[derive(Debug, Clone, Copy)]
pub(super) enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub(super) struct StreamChunk {
    pub(super) kind: StreamKind,
    pub(super) text: String,
}

pub(super) fn spawn_reader<R>(mut reader: R, kind: StreamKind, tx: mpsc::Sender<StreamChunk>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    let text = String::from_utf8_lossy(&buffer[..size]).to_string();
                    if tx.send(StreamChunk { kind, text }).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

pub(super) fn emit_lines(
    sink: &mut impl OperationSink,
    scope: &'static str,
    pending: &mut String,
    chunk: &str,
) {
    pending.push_str(chunk);
    while let Some(index) = pending.find('\n') {
        let line = pending[..index].trim_end_matches('\r').trim().to_string();
        pending.replace_range(..=index, "");
        if !line.is_empty() {
            emit(sink, scope, redact_log_line(&line));
        }
    }
}

pub(super) fn flush_line(sink: &mut impl OperationSink, scope: &'static str, pending: &mut String) {
    let line = pending.trim();
    if !line.is_empty() {
        emit(sink, scope, redact_log_line(line));
    }
    pending.clear();
}

pub(super) fn emit(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: impl Into<String>,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.into(),
        domain: StepDomain::HyperV,
        action: StepAction::Configure,
        provider: ProviderKind::HyperV,
    });
}

pub(super) fn redact_log_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if [
        "token",
        "secret",
        "password",
        "apikey",
        "auth",
        "private key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        "[redacted sensitive vendor output]".to_string()
    } else {
        line.to_string()
    }
}

pub(super) fn sha256_file(path: &Path) -> CommandResult<String> {
    let bytes = fs::read(path)
        .map_err(|err| failure(format!("Failed to read {}: {err}", path.display())))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(super) fn last_non_empty_line(value: &str) -> Option<String> {
    value
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_vendor_output() {
        assert_eq!(
            redact_log_line("Self-Host Service Token: abc"),
            "[redacted sensitive vendor output]"
        );
        assert_eq!(redact_log_line("VM memory set"), "VM memory set");
    }
}
