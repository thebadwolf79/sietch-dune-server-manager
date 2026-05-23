//! CLI argument parser, usage text, and error JSON conversion.

use serde_json::{json, Value};

use crate::{
    errors::failure,
    models::{CommandFailure, CommandResult},
};

#[derive(Debug, Clone)]
pub(super) struct CliArgs {
    args: Vec<String>,
}

impl CliArgs {
    pub(super) fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    pub(super) fn positional_slice(&self) -> Vec<&str> {
        let mut values = Vec::new();
        let mut index = 0;
        while index < self.args.len() {
            let arg = &self.args[index];
            if arg.starts_with("--") {
                if self
                    .args
                    .get(index + 1)
                    .is_some_and(|next| !next.starts_with("--"))
                {
                    index += 2;
                } else {
                    index += 1;
                }
            } else {
                values.push(arg.as_str());
                index += 1;
            }
        }
        values
    }

    pub(super) fn has_flag(&self, name: &str) -> bool {
        self.args.iter().any(|arg| arg == name)
    }

    pub(super) fn required(&self, name: &str) -> CommandResult<String> {
        self.optional(name)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| failure(format!("Missing required argument {name}")))
    }

    pub(super) fn optional(&self, name: &str) -> Option<String> {
        self.args
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    }

    pub(super) fn optional_u64(&self, name: &str) -> CommandResult<Option<u64>> {
        self.optional(name)
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| failure(format!("{name} must be an unsigned integer")))
            })
            .transpose()
    }

    pub(super) fn required_u64(&self, name: &str) -> CommandResult<u64> {
        self.optional_u64(name)?
            .ok_or_else(|| failure(format!("Missing required argument {name}")))
    }
}

pub(super) fn usage() -> Vec<&'static str> {
    vec![
        "dune-manager-cli flow battlegroup",
        "dune-manager-cli db ping --db-host IP [--db-port 15432] [--db-name dune] [--db-user dune] [--db-password PASSWORD | --db-password-file PATH | --db-password-env NAME]",
        "dune-manager-cli db world-partitions --db-host IP [--map MAP] [--db-port 15432] [--db-name dune] [--db-user dune] [--db-password PASSWORD | --db-password-file PATH | --db-password-env NAME]",
        "dune-manager-cli bg list --key PATH --host IP [--port 22] [--user dune]",
        "dune-manager-cli bg status --key PATH --host IP --namespace NS --name BG [--user dune]",
        "dune-manager-cli bg start|stop|restart --key PATH --host IP --namespace NS --name BG [--director-timeout 60]",
        "dune-manager-cli bg patch-region --key PATH --host IP --namespace NS --name BG --region Europe",
        "dune-manager-cli bg instances set --key PATH --host IP --namespace NS --name BG --map survival-1|deep-desert --count N [--pvp-count N] [--restart]",
        "dune-manager-cli bg display-name set --key PATH --host IP --namespace NS --name BG --map survival-1|deep-desert --dimension N --display-name NAME [--restart]",
        "dune-manager-cli bg display-name clear --key PATH --host IP --namespace NS --name BG --map survival-1|deep-desert --dimension N [--restart]",
        "dune-manager-cli bg pods --key PATH --host IP --namespace NS",
        "dune-manager-cli bg pod-shell-spec --key PATH --host IP --namespace NS --pod POD",
        "dune-manager-cli bg export-logs --key PATH --host IP --namespace NS",
        "dune-manager-cli bg export-operator-logs --key PATH --host IP",
        "dune-manager-cli bg update --key PATH --host IP --namespace NS --name BG",
        "dune-manager-cli bg file-browser-url --key PATH --host IP --vm-ip IP",
        "dune-manager-cli bg director-url --key PATH --host IP --namespace NS --name BG --vm-ip IP",
    ]
}

impl From<CommandFailure> for Value {
    fn from(value: CommandFailure) -> Self {
        json!({
            "ok": false,
            "error": value.message,
            "stdout": value.stdout,
            "stderr": value.stderr,
            "code": value.code,
        })
    }
}
