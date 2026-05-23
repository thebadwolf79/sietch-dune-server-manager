//! Runs the unmodified vendor Hyper-V setup script through stdio.

use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::OperationSink,
    shell::suppress_console_window,
};

use super::io::{
    emit, emit_lines, flush_line, last_non_empty_line, sha256_file, spawn_reader, StreamChunk,
    StreamKind,
};
use super::models::{VendorHyperVSetupRequest, VendorHyperVSetupResult, VendorPromptAnswer};
use super::prompt_driver::VendorPromptDriver;
use super::scripts::{vendor_powershell_wrapper, write_wrapper_script};

/// Runs the unmodified vendor Hyper-V setup script through stdio.
pub struct VendorHyperVSetupRunner {
    package_dir: PathBuf,
}

impl VendorHyperVSetupRunner {
    /// Creates a runner for a detected server package directory.
    pub fn new(package_dir: impl Into<PathBuf>) -> Self {
        Self {
            package_dir: package_dir.into(),
        }
    }

    /// Executes the vendor setup script with an in-process `Read-Host` answer shim.
    pub fn run(
        &self,
        request: &VendorHyperVSetupRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<VendorHyperVSetupResult> {
        let script_dir = self.script_dir()?;
        let script_path = script_dir.join("initial-setup.ps1");
        let script_sha256 = sha256_file(&script_path)?;
        emit(
            sink,
            "vendor.hyperv.script",
            format!(
                "Running vendor Hyper-V setup script {} (sha256 {}).",
                script_path.display(),
                script_sha256
            ),
        );

        let wrapper_path = write_wrapper_script(&vendor_powershell_wrapper(request, &script_dir))?;
        let mut command = Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &wrapper_path.to_string_lossy(),
            ])
            .current_dir(&script_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        suppress_console_window(&mut command);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = fs::remove_file(&wrapper_path);
                return Err(failure(format!(
                    "Failed to start vendor Hyper-V setup script: {err}"
                )));
            }
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| failure("Failed to open vendor setup stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| failure("Failed to open vendor setup stderr"))?;
        let (tx, rx) = mpsc::channel();
        spawn_reader(stdout, StreamKind::Stdout, tx.clone());
        spawn_reader(stderr, StreamKind::Stderr, tx);

        let mut stdout_text = String::new();
        let mut stderr_text = String::new();
        let mut stdout_line = String::new();
        let mut stderr_line = String::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(150)) {
                Ok(StreamChunk { kind, text }) => match kind {
                    StreamKind::Stdout => {
                        stdout_text.push_str(&text);
                        emit_lines(sink, "vendor.stdout", &mut stdout_line, &text);
                    }
                    StreamKind::Stderr => {
                        stderr_text.push_str(&text);
                        emit_lines(sink, "vendor.stderr", &mut stderr_line, &text);
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|err| failure(format!("Failed to poll vendor setup: {err}")))?
                    {
                        flush_line(sink, "vendor.stdout", &mut stdout_line);
                        flush_line(sink, "vendor.stderr", &mut stderr_line);
                        let _ = fs::remove_file(&wrapper_path);
                        if status.success() {
                            return Ok(VendorHyperVSetupResult {
                                script_path,
                                script_sha256,
                            });
                        }
                        return Err(failure(format!(
                            "Vendor Hyper-V setup script exited with {status}. Last stderr: {}",
                            last_non_empty_line(&stderr_text).unwrap_or_else(|| "none".to_string())
                        )));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = child.wait().map_err(|err| {
                        failure(format!("Failed to wait for vendor setup: {err}"))
                    })?;
                    flush_line(sink, "vendor.stdout", &mut stdout_line);
                    flush_line(sink, "vendor.stderr", &mut stderr_line);
                    let _ = fs::remove_file(&wrapper_path);
                    if status.success() {
                        return Ok(VendorHyperVSetupResult {
                            script_path,
                            script_sha256,
                        });
                    }
                    return Err(failure(format!(
                        "Vendor Hyper-V setup script exited with {status}. Last stderr: {}",
                        last_non_empty_line(&stderr_text).unwrap_or_else(|| "none".to_string())
                    )));
                }
            }
        }
    }

    /// Exercises prompt matching against a transcript without starting PowerShell.
    pub fn dry_run_answers(
        &self,
        request: &VendorHyperVSetupRequest,
        transcript: &str,
    ) -> Vec<VendorPromptAnswer> {
        let mut driver = VendorPromptDriver::new(request.clone());
        driver.observe(transcript, "")
    }

    fn script_dir(&self) -> CommandResult<PathBuf> {
        let candidates = [
            self.package_dir.join("battlegroup-management"),
            self.package_dir.join("internal-scripts"),
        ];
        candidates
            .into_iter()
            .find(|path| path.join("initial-setup.ps1").is_file())
            .ok_or_else(|| {
                failure(format!(
                    "Could not find vendor initial-setup.ps1 under {}",
                    self.package_dir.display()
                ))
            })
    }
}

