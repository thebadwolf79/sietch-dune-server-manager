//! Sync driver around the vendor `/home/dune/.dune/bin/battlegroup` wrapper.

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{BattlegroupRef, BattlegroupState, RemoteCommandRunner},
};

use super::status_parser::parse_wrapper_status;

/// Lifecycle and status operations exposed by a vendor wrapper driver.
///
/// Implemented by [`VendorBattlegroupWrapper`] and by test mocks. Production
/// orchestrators use this trait so they remain decoupled from the SSH
/// transport.
pub trait BattlegroupWrapperOps {
    /// Reads current battlegroup state via the wrapper's `status` action.
    fn status(&self, battlegroup: &BattlegroupRef) -> CommandResult<BattlegroupState>;
    /// Starts the battlegroup via the wrapper's `start` action.
    fn start(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome>;
    /// Stops the battlegroup via the wrapper's `stop` action.
    fn stop(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome>;
    /// Restarts the battlegroup via the wrapper's `restart` action.
    fn restart(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome>;
    /// Runs the wrapper's `update` action (steamcmd + operators + maps + images).
    fn update(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome>;
}

/// Path to the vendor wrapper script on the guest.
pub const VENDOR_WRAPPER_PATH: &str = "/home/dune/.dune/bin/battlegroup";

/// One of the vendor wrapper actions this driver supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperAction {
    /// `battlegroup status` — read current battlegroup state.
    Status,
    /// `battlegroup start` — clears `spec.stop`.
    Start,
    /// `battlegroup stop` — sets `spec.stop=true`.
    Stop,
    /// `battlegroup restart` — stop, sleep 5, start.
    Restart,
    /// `battlegroup update` — steamcmd, operator update, map update, image patch.
    Update,
}

impl WrapperAction {
    /// Returns the subcommand string passed to the wrapper.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Update => "update",
        }
    }
}

/// Captured output of a wrapper invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperOutcome {
    /// Action that was executed.
    pub action: WrapperAction,
    /// Combined stdout captured from the wrapper.
    pub stdout: String,
}

/// Driver that shells out to the vendor battlegroup wrapper.
#[derive(Debug, Clone)]
pub struct VendorBattlegroupWrapper<R> {
    runner: R,
}

impl<R> VendorBattlegroupWrapper<R>
where
    R: RemoteCommandRunner,
{
    /// Creates a wrapper driver around a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Borrows the underlying runner.
    pub fn runner(&self) -> &R {
        &self.runner
    }

    /// Invokes the vendor wrapper for the given action against a specific
    /// battlegroup. Returns captured stdout on success.
    pub fn invoke(
        &self,
        battlegroup: &BattlegroupRef,
        action: WrapperAction,
    ) -> CommandResult<WrapperOutcome> {
        battlegroup.validate()?;
        let script = build_wrapper_script(&battlegroup.namespace, action);
        let stdout = self.runner.run_script(&script)?;
        Ok(WrapperOutcome { action, stdout })
    }
}

impl<R> BattlegroupWrapperOps for VendorBattlegroupWrapper<R>
where
    R: RemoteCommandRunner,
{
    fn status(&self, battlegroup: &BattlegroupRef) -> CommandResult<BattlegroupState> {
        let outcome = self.invoke(battlegroup, WrapperAction::Status)?;
        parse_wrapper_status(&outcome.stdout).ok_or_else(|| {
            failure(format!(
                "Could not parse vendor battlegroup status output:\n{}",
                outcome.stdout
            ))
        })
    }

    fn start(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        self.invoke(battlegroup, WrapperAction::Start)
    }

    fn stop(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        self.invoke(battlegroup, WrapperAction::Stop)
    }

    fn restart(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        self.invoke(battlegroup, WrapperAction::Restart)
    }

    fn update(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        self.invoke(battlegroup, WrapperAction::Update)
    }
}

/// Builds the POSIX-sh snippet that drives the vendor wrapper for a known
/// namespace. The wrapper's own `select_battlegroup` enumerates
/// `funcom-seabass-*` namespaces and prompts if there is more than one, so
/// the snippet finds the target namespace's 1-based index against the same
/// listing and pipes it on stdin. When only one namespace exists, the
/// wrapper auto-selects and the piped index is harmless.
///
/// A trailing `N` line is piped after the index so any `Retry? [Y/N]`
/// follow-up prompt is answered with "no" instead of blocking.
///
/// The launcher is intentionally POSIX-sh (no bash arrays / process
/// substitution) because [`crate::orchestration::RemoteCommandRunner::run_script`]
/// pipes scripts to `sh -s`, which is `dash` on Ubuntu.
fn build_wrapper_script(target_ns: &str, action: WrapperAction) -> String {
    let ns_literal = sh_single_quoted(target_ns);
    let action_literal = sh_single_quoted(action.as_str());
    format!(
        r#"set -eu
TARGET_NS={ns_literal}
ACTION={action_literal}
WRAPPER={wrapper_literal}
if [ ! -x "$WRAPPER" ]; then
  echo "Vendor wrapper not found at $WRAPPER" >&2
  exit 1
fi
idx=$(sudo kubectl get ns --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null \
  | grep '^funcom-seabass-' \
  | awk -v target="$TARGET_NS" 'BEGIN{{ found=0 }} {{ i++; if ($1==target) {{ print i; found=1; exit }} }} END{{ if (!found) exit 1 }}')
if [ -z "$idx" ]; then
  echo "Battlegroup namespace $TARGET_NS not found in funcom-seabass-* listing" >&2
  exit 1
fi
printf '%s\nN\n' "$idx" | "$WRAPPER" "$ACTION"
"#,
        ns_literal = ns_literal,
        action_literal = action_literal,
        wrapper_literal = sh_single_quoted(VENDOR_WRAPPER_PATH),
    )
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_actions_have_subcommand_names() {
        assert_eq!(WrapperAction::Status.as_str(), "status");
        assert_eq!(WrapperAction::Start.as_str(), "start");
        assert_eq!(WrapperAction::Stop.as_str(), "stop");
        assert_eq!(WrapperAction::Restart.as_str(), "restart");
        assert_eq!(WrapperAction::Update.as_str(), "update");
    }

    #[test]
    fn build_wrapper_script_quotes_namespace_and_action() {
        let script = build_wrapper_script("funcom-seabass-it's", WrapperAction::Status);
        assert!(script.contains("'funcom-seabass-it'\"'\"'s'"));
        assert!(script.contains("'status'"));
        assert!(script.contains("/home/dune/.dune/bin/battlegroup"));
        assert!(script.contains("printf '%s\\nN\\n'"));
    }

    #[test]
    fn build_wrapper_script_is_posix_sh_safe() {
        let script = build_wrapper_script("funcom-seabass-foo", WrapperAction::Status);
        // Bash arrays and process substitution must not appear; the script is
        // sent to `sh -s`, which is dash on Ubuntu.
        assert!(!script.contains("namespaces=()"));
        assert!(!script.contains("namespaces+=("));
        assert!(!script.contains("${!namespaces"));
        assert!(!script.contains("<("));
    }
}
