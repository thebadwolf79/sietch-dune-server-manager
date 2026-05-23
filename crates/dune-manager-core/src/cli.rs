use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    database::{DuneDatabase, DuneDatabaseConfig, DEFAULT_DUNE_DATABASE_PORT},
    errors::failure,
    models::{CommandFailure, CommandResult},
    orchestration::{
        battlegroup_command_catalog, BattlegroupManagementOrchestrator, BattlegroupRef,
        BattlegroupUpdateOrchestrator, InstanceMap, MapInstanceOrchestrator, OpenSshRunner,
        OpenSshTarget, SetMapDisplayNameRequest, SetMapInstancesRequest,
        SshGuestBootstrapProvider, StructuredBattlegroupOps, StructuredKubectl, VecOperationSink,
    },
    security::redact_json,
};

/// Runs the CLI using process arguments and returns a process exit code.
///
/// Successful commands print pretty JSON to stdout. Failures print a redacted
/// JSON error envelope to stderr.
pub fn run_cli_from_env() -> i32 {
    match run_cli(std::env::args().skip(1).collect()) {
        Ok(mut value) => {
            redact_json(&mut value);
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
            );
            0
        }
        Err(err) => {
            let mut value = json!({
                "ok": false,
                "error": err.message,
                "stdout": err.stdout,
                "stderr": err.stderr,
                "code": err.code,
            });
            redact_json(&mut value);
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "{\"ok\":false}".to_string())
            );
            err.code.unwrap_or(1)
        }
    }
}

fn run_cli(args: Vec<String>) -> CommandResult<Value> {
    let args = CliArgs::new(args);
    if args.is_empty() || args.has_flag("--help") || args.has_flag("-h") {
        return Ok(json!({
            "ok": true,
            "usage": usage(),
        }));
    }

    let positional = args.positional_slice();
    match positional.as_slice() {
        ["flow", "battlegroup"] => to_json(battlegroup_command_catalog()),
        ["db", "ping"] => to_json(DuneDatabase::new(db_config(&args)?).health()?),
        ["db", "world-partitions"] => {
            let map = args.optional("--map");
            to_json(DuneDatabase::new(db_config(&args)?).world_partitions(map.as_deref())?)
        }
        ["ssh", "shell-spec"] => {
            let spec = ssh_runner(&args)?.interactive_shell_spec()?;
            to_json(spec)
        }
        ["bg", "list"] => to_json(bg_ops(&args)?.list()?),
        ["bg", "status"] => {
            let bg = battlegroup_ref(&args)?;
            to_json(bg_ops(&args)?.status(&bg)?)
        }
        ["bg", "patch-region"] => {
            let bg = battlegroup_ref(&args)?;
            let region = args.required("--region")?;
            bg_ops(&args)?.patch_region(&bg, &region)?;
            Ok(json!({ "ok": true }))
        }
        ["bg", "instances", "set"] => {
            let bg = battlegroup_ref(&args)?;
            let map = InstanceMap::parse(&args.required("--map")?)?;
            let count = usize::try_from(args.required_u64("--count")?)
                .map_err(|_| failure("--count is too large"))?;
            let mut request = SetMapInstancesRequest::new(bg, map, count);
            request.pvp_instance_count = args
                .optional_u64("--pvp-count")?
                .map(|value| {
                    usize::try_from(value).map_err(|_| failure("--pvp-count is too large"))
                })
                .transpose()?;
            let result =
                MapInstanceOrchestrator::new(ssh_runner(&args)?).set_instances(&request)?;
            let restart = if args.has_flag("--restart") {
                Some(bg_lifecycle(&args, "restart")?)
            } else {
                None
            };
            Ok(json!({
                "ok": true,
                "result": result,
                "restart": restart,
            }))
        }
        ["bg", "display-name", "set"] => {
            let bg = battlegroup_ref(&args)?;
            let map = InstanceMap::parse(&args.required("--map")?)?;
            let dimension = i64::try_from(args.required_u64("--dimension")?)
                .map_err(|_| failure("--dimension is too large"))?;
            let request =
                SetMapDisplayNameRequest::set(bg, map, dimension, args.required("--display-name")?);
            let result =
                MapInstanceOrchestrator::new(ssh_runner(&args)?).set_display_name(&request)?;
            let restart = if args.has_flag("--restart") {
                Some(bg_lifecycle(&args, "restart")?)
            } else {
                None
            };
            Ok(json!({
                "ok": true,
                "result": result,
                "restart": restart,
            }))
        }
        ["bg", "display-name", "clear"] => {
            let bg = battlegroup_ref(&args)?;
            let map = InstanceMap::parse(&args.required("--map")?)?;
            let dimension = i64::try_from(args.required_u64("--dimension")?)
                .map_err(|_| failure("--dimension is too large"))?;
            let request = SetMapDisplayNameRequest::clear(bg, map, dimension);
            let result =
                MapInstanceOrchestrator::new(ssh_runner(&args)?).set_display_name(&request)?;
            let restart = if args.has_flag("--restart") {
                Some(bg_lifecycle(&args, "restart")?)
            } else {
                None
            };
            Ok(json!({
                "ok": true,
                "result": result,
                "restart": restart,
            }))
        }
        ["bg", "pods"] => {
            let namespace = args.required("--namespace")?;
            to_json(bg_ops(&args)?.list_pods(&namespace)?)
        }
        ["bg", "pod-shell-spec"] => {
            let namespace = args.required("--namespace")?;
            let pod = args.required("--pod")?;
            to_json(bg_ops(&args)?.pod_shell_spec(&namespace, &pod)?)
        }
        ["bg", "export-logs"] => {
            let namespace = args.required("--namespace")?;
            to_json(bg_ops(&args)?.export_namespace_logs(&namespace)?)
        }
        ["bg", "export-operator-logs"] => to_json(bg_ops(&args)?.export_operator_logs()?),
        ["bg", "file-browser-url"] => {
            let vm_ip = args.required("--vm-ip")?;
            let kube = StructuredKubectl::new(ssh_runner(&args)?);
            to_json(BattlegroupManagementOrchestrator::new(kube).file_browser_url(&vm_ip)?)
        }
        ["bg", "director-url"] => {
            let bg = battlegroup_ref(&args)?;
            let vm_ip = args.required("--vm-ip")?;
            let kube = StructuredKubectl::new(ssh_runner(&args)?);
            to_json(BattlegroupManagementOrchestrator::new(kube).director_url(&bg, &vm_ip)?)
        }
        ["bg", "start"] => bg_lifecycle(&args, "start"),
        ["bg", "stop"] => bg_lifecycle(&args, "stop"),
        ["bg", "restart"] => bg_lifecycle(&args, "restart"),
        ["bg", "update"] => {
            let bg = battlegroup_ref(&args)?;
            let provider = SshGuestBootstrapProvider::new(ssh_runner(&args)?);
            let mut sink = VecOperationSink::default();
            BattlegroupUpdateOrchestrator::new(provider).update_from_steam(&bg, &mut sink)?;
            operation_ok(sink)
        }
        ["bg", "apply-downloaded-update"] => {
            let bg = battlegroup_ref(&args)?;
            let provider = SshGuestBootstrapProvider::new(ssh_runner(&args)?);
            let mut sink = VecOperationSink::default();
            BattlegroupUpdateOrchestrator::new(provider).update_from_downloads(&bg, &mut sink)?;
            operation_ok(sink)
        }
        other => Err(failure(format!(
            "Unknown command: {}",
            if other.is_empty() {
                "<none>".to_string()
            } else {
                other.join(" ")
            }
        ))),
    }
}

fn bg_lifecycle(args: &CliArgs, action: &str) -> CommandResult<Value> {
    let bg = battlegroup_ref(args)?;
    let timeout = args.optional_u64("--director-timeout")?.unwrap_or(60);
    let kube = StructuredKubectl::new(ssh_runner(args)?);
    let orchestrator = BattlegroupManagementOrchestrator::new(kube);
    let mut sink = VecOperationSink::default();
    let director_port = match action {
        "start" => orchestrator.start_and_wait_director(&bg, timeout, &mut sink)?,
        "stop" => {
            orchestrator.stop(&bg, &mut sink)?;
            None
        }
        "restart" => orchestrator.restart_and_wait_director(&bg, timeout, &mut sink)?,
        _ => unreachable!("validated by caller"),
    };
    Ok(json!({
        "ok": true,
        "directorNodePort": director_port,
        "events": sink.events,
    }))
}

fn bg_ops(args: &CliArgs) -> CommandResult<StructuredBattlegroupOps<OpenSshRunner>> {
    Ok(StructuredBattlegroupOps::new(ssh_runner(args)?))
}

fn optional_port(args: &CliArgs, name: &str) -> CommandResult<Option<u16>> {
    args.optional_u64(name)?
        .map(|value| {
            u16::try_from(value).map_err(|_| failure(format!("{name} must fit in a TCP port")))
        })
        .transpose()
}

fn db_config(args: &CliArgs) -> CommandResult<DuneDatabaseConfig> {
    let host = args.required("--db-host")?;
    let port = optional_port(args, "--db-port")?.unwrap_or(DEFAULT_DUNE_DATABASE_PORT);
    let database = args
        .optional("--db-name")
        .unwrap_or_else(|| "dune".to_string());
    let user = args
        .optional("--db-user")
        .unwrap_or_else(|| "dune".to_string());
    let password = db_password(args)?.unwrap_or_else(|| "dune".to_string());
    Ok(DuneDatabaseConfig {
        host,
        port,
        database,
        user,
        password,
    })
}

fn db_password(args: &CliArgs) -> CommandResult<Option<String>> {
    if let Some(value) = args.optional("--db-password") {
        return Ok(Some(value));
    }
    if let Some(path) = args.optional("--db-password-file") {
        let text = std::fs::read_to_string(&path).map_err(|err| {
            failure(format!(
                "Failed to read database password file {path}: {err}"
            ))
        })?;
        let password = text.trim_end_matches(['\r', '\n']).to_string();
        if password.is_empty() {
            return Err(failure("Database password file is empty"));
        }
        return Ok(Some(password));
    }
    if let Some(name) = args.optional("--db-password-env") {
        let password = std::env::var(&name)
            .map_err(|_| failure(format!("Environment variable {name} is not set")))?;
        if password.is_empty() {
            return Err(failure(format!("Environment variable {name} is empty")));
        }
        return Ok(Some(password));
    }
    Ok(None)
}

fn battlegroup_ref(args: &CliArgs) -> CommandResult<BattlegroupRef> {
    Ok(BattlegroupRef {
        namespace: args.required("--namespace")?,
        name: args.required("--name")?,
    })
}

fn ssh_runner(args: &CliArgs) -> CommandResult<OpenSshRunner> {
    ssh_runner_with_default_user(args, "dune")
}

fn ssh_runner_with_default_user(
    args: &CliArgs,
    default_user: &str,
) -> CommandResult<OpenSshRunner> {
    Ok(OpenSshRunner::new(OpenSshTarget::new(
        args.required("--ssh")?,
        args.required("--key")?,
        args.optional("--user")
            .unwrap_or_else(|| default_user.to_string()),
        args.required("--host")?,
    )))
}

fn operation_ok(sink: VecOperationSink) -> CommandResult<Value> {
    Ok(json!({
        "ok": true,
        "events": sink.events,
    }))
}

fn to_json(value: impl Serialize) -> CommandResult<Value> {
    serde_json::to_value(value).map_err(|err| failure(format!("Failed to serialize output: {err}")))
}

#[derive(Debug, Clone)]
struct CliArgs {
    args: Vec<String>,
}

impl CliArgs {
    fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    fn positional_slice(&self) -> Vec<&str> {
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

    fn has_flag(&self, name: &str) -> bool {
        self.args.iter().any(|arg| arg == name)
    }

    fn required(&self, name: &str) -> CommandResult<String> {
        self.optional(name)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| failure(format!("Missing required argument {name}")))
    }

    fn optional(&self, name: &str) -> Option<String> {
        self.args
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    }

    fn optional_u64(&self, name: &str) -> CommandResult<Option<u64>> {
        self.optional(name)
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| failure(format!("{name} must be an unsigned integer")))
            })
            .transpose()
    }

    fn required_u64(&self, name: &str) -> CommandResult<u64> {
        self.optional_u64(name)?
            .ok_or_else(|| failure(format!("Missing required argument {name}")))
    }
}

fn usage() -> Vec<&'static str> {
    vec![
        "dune-manager-cli flow battlegroup",
        "dune-manager-cli db ping --db-host IP [--db-port 15432] [--db-name dune] [--db-user dune] [--db-password PASSWORD | --db-password-file PATH | --db-password-env NAME]",
        "dune-manager-cli db world-partitions --db-host IP [--map MAP] [--db-port 15432] [--db-name dune] [--db-user dune] [--db-password PASSWORD | --db-password-file PATH | --db-password-env NAME]",
        "dune-manager-cli ssh shell-spec --ssh PATH --key PATH --host IP [--user dune]",
        "dune-manager-cli bg list --ssh PATH --key PATH --host IP [--user dune]",
        "dune-manager-cli bg status --ssh PATH --key PATH --host IP --namespace NS --name BG [--user dune]",
        "dune-manager-cli bg start|stop|restart --ssh PATH --key PATH --host IP --namespace NS --name BG [--director-timeout 60]",
        "dune-manager-cli bg patch-region --ssh PATH --key PATH --host IP --namespace NS --name BG --region Europe",
        "dune-manager-cli bg instances set --ssh PATH --key PATH --host IP --namespace NS --name BG --map survival-1|deep-desert --count N [--pvp-count N] [--restart]",
        "dune-manager-cli bg display-name set --ssh PATH --key PATH --host IP --namespace NS --name BG --map survival-1|deep-desert --dimension N --display-name NAME [--restart]",
        "dune-manager-cli bg display-name clear --ssh PATH --key PATH --host IP --namespace NS --name BG --map survival-1|deep-desert --dimension N [--restart]",
        "dune-manager-cli bg pods --ssh PATH --key PATH --host IP --namespace NS",
        "dune-manager-cli bg pod-shell-spec --ssh PATH --key PATH --host IP --namespace NS --pod POD",
        "dune-manager-cli bg export-logs --ssh PATH --key PATH --host IP --namespace NS",
        "dune-manager-cli bg export-operator-logs --ssh PATH --key PATH --host IP",
        "dune-manager-cli bg update --ssh PATH --key PATH --host IP --namespace NS --name BG",
        "dune-manager-cli bg file-browser-url --ssh PATH --key PATH --host IP --vm-ip IP",
        "dune-manager-cli bg director-url --ssh PATH --key PATH --host IP --namespace NS --name BG --vm-ip IP",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_usage_when_no_args_are_supplied() {
        let value = run_cli(vec![]).unwrap();
        assert_eq!(value["ok"], true);
        assert!(value["usage"].as_array().unwrap().len() > 5);
    }

    #[test]
    fn missing_required_arg_fails_cleanly() {
        let err = run_cli(vec!["bg".into(), "status".into()]).unwrap_err();
        assert!(err.message.contains("--namespace"));
    }
}
