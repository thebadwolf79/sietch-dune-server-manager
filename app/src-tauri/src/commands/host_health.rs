//! Host Health & Hardening advisor.
//!
//! SSH-probes the (Alpine) VM for resource conditions an operator can't easily
//! see — no swap, high swappiness, low disk, recurring DB restarts / OOMKilled
//! pods — and turns them into severity-ranked findings with recommendations.
//! Safe, idempotent fixes (add a swapfile, set swappiness) can be applied with a
//! one-click `host_apply_fix` (the UI confirms first).
//!
//! Scope is host-OS hardening ONLY; it never modifies Funcom's game stack, so it
//! can't conflict with vendor updates ("wrap, don't replace"). The *check* is
//! fully unprivileged; only *apply* uses sudo.

use serde_json::Value;

use dune_manager_core::orchestration::RemoteCommandRunner;

use crate::commands::shared::{command_error_message, runner_for_remote_kind};
use crate::dto::{
    HealthFinding, HostApplyFixRequest, HostApplyFixResult, HostHealthReport, HostHealthRequest,
    HostMetrics,
};

/// Read-only probe (no sudo): everything here is world-readable on Alpine.
const PROBE_SCRIPT: &str = r#"export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH
echo "MEM_TOTAL_KB=$(awk '/^MemTotal:/{print $2}' /proc/meminfo)"
echo "MEM_AVAIL_KB=$(awk '/^MemAvailable:/{print $2}' /proc/meminfo)"
echo "SWAP_TOTAL_KB=$(awk '/^SwapTotal:/{print $2}' /proc/meminfo)"
echo "SWAP_FREE_KB=$(awk '/^SwapFree:/{print $2}' /proc/meminfo)"
echo "SWAPPINESS=$(cat /proc/sys/vm/swappiness 2>/dev/null)"
echo "DISK_ROOT_AVAIL_KB=$(df -k / | awk 'NR==2{print $4}')"
echo "DISK_ROOT_USE_PCT=$(df -k / | awk 'NR==2{gsub(/%/,"",$5); print $5}')"
echo "FSTAB_SWAP=$(grep -qi swap /etc/fstab 2>/dev/null && echo yes || echo no)"
"#;

// Thresholds (documented so they're easy to tune).
const SWAPPINESS_HIGH: i32 = 30;
const SWAPPINESS_TARGET: i64 = 10;
const DISK_LOW_GB: f64 = 5.0;
const DISK_USE_HIGH_PCT: i32 = 90;
const DB_RESTARTS_WARN: i32 = 5;
const SWAP_MIN_GB: i64 = 2;
const SWAP_MAX_GB: i64 = 16;

#[tauri::command]
pub async fn host_health_check(request: HostHealthRequest) -> Result<HostHealthReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;

        // Host-level probe (unprivileged).
        let probe = runner
            .run_script(PROBE_SCRIPT)
            .map_err(command_error_message)?;
        let mut metrics = parse_probe(&probe);

        // Cluster-level checks (best-effort, needs sudo + a namespace). A failure
        // here must not sink the host report, so we degrade gracefully.
        let mut cluster_checked = false;
        if let Some(ns) = request.namespace.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            match runner.run_json(
                &format!("sudo kubectl get pods -n {} -o json", shell_quote(ns)),
                "pod inventory",
            ) {
                Ok(json) => {
                    let (max_restarts, oom) = parse_pod_health(&json);
                    metrics.db_max_restarts = max_restarts;
                    metrics.oomkilled_pods = oom;
                    cluster_checked = true;
                }
                Err(_) => { /* leave cluster fields at defaults; report still useful */ }
            }
        }

        let findings = analyze(&metrics);
        let overall_severity = highest_severity(&findings).to_string();
        let summary = summarize(&findings);

        Ok(HostHealthReport {
            metrics,
            findings,
            overall_severity,
            summary,
            cluster_checked,
        })
    })
    .await
    .map_err(|err| format!("host_health_check worker failed: {err}"))?
}

#[tauri::command]
pub async fn host_apply_fix(request: HostApplyFixRequest) -> Result<HostApplyFixResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;

        let script = match request.fix_id.as_str() {
            "add_swap" => {
                let gb = request.param.unwrap_or(8).clamp(SWAP_MIN_GB, SWAP_MAX_GB);
                add_swap_script(gb)
            }
            "set_swappiness" => {
                let value = request.param.unwrap_or(SWAPPINESS_TARGET).clamp(0, 100);
                set_swappiness_script(value)
            }
            other => return Err(format!("unknown fix id: {other}")),
        };

        // The whole script runs as root via `sudo sh -s` (same pattern the install
        // flow uses), so individual commands don't each need sudo.
        let out = runner
            .run_with_stdin("sudo sh -s", script.as_bytes())
            .map_err(command_error_message)?;

        Ok(HostApplyFixResult {
            ok: true,
            fix_id: request.fix_id,
            message: out.trim().to_string(),
        })
    })
    .await
    .map_err(|err| format!("host_apply_fix worker failed: {err}"))?
}

// --- pure helpers (unit-tested) --------------------------------------------

fn parse_probe(stdout: &str) -> HostMetrics {
    let mut mem_total_kb = 0u64;
    let mut mem_avail_kb = 0u64;
    let mut swap_total_kb = 0u64;
    let mut swap_free_kb = 0u64;
    let mut swappiness: Option<i32> = None;
    let mut disk_avail_kb = 0u64;
    let mut disk_use_pct: Option<i32> = None;
    let mut fstab_swap = false;

    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "MEM_TOTAL_KB" => mem_total_kb = value.parse().unwrap_or(0),
            "MEM_AVAIL_KB" => mem_avail_kb = value.parse().unwrap_or(0),
            "SWAP_TOTAL_KB" => swap_total_kb = value.parse().unwrap_or(0),
            "SWAP_FREE_KB" => swap_free_kb = value.parse().unwrap_or(0),
            "SWAPPINESS" => swappiness = value.parse().ok(),
            "DISK_ROOT_AVAIL_KB" => disk_avail_kb = value.parse().unwrap_or(0),
            "DISK_ROOT_USE_PCT" => disk_use_pct = value.parse().ok(),
            "FSTAB_SWAP" => fstab_swap = value.eq_ignore_ascii_case("yes"),
            _ => {}
        }
    }

    HostMetrics {
        mem_total_mb: mem_total_kb / 1024,
        mem_available_mb: mem_avail_kb / 1024,
        swap_total_mb: swap_total_kb / 1024,
        swap_used_mb: swap_total_kb.saturating_sub(swap_free_kb) / 1024,
        swappiness,
        disk_root_avail_gb: (disk_avail_kb as f64) / 1024.0 / 1024.0,
        disk_root_use_pct: disk_use_pct,
        fstab_swap,
        db_max_restarts: None,
        oomkilled_pods: Vec::new(),
    }
}

/// Parse `kubectl get pods -o json` into (max DB restartCount, OOMKilled pod names).
fn parse_pod_health(json: &Value) -> (Option<i32>, Vec<String>) {
    let mut max_db_restarts: Option<i32> = None;
    let mut oom: Vec<String> = Vec::new();
    let Some(items) = json.get("items").and_then(Value::as_array) else {
        return (None, oom);
    };
    for pod in items {
        let name = pod
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .unwrap_or("");
        let statuses = pod
            .pointer("/status/containerStatuses")
            .and_then(Value::as_array);
        let mut pod_restarts = 0i32;
        let mut pod_oom = false;
        if let Some(statuses) = statuses {
            for cs in statuses {
                pod_restarts += cs.get("restartCount").and_then(Value::as_i64).unwrap_or(0) as i32;
                for state_key in ["state", "lastState"] {
                    let reason = cs
                        .pointer(&format!("/{state_key}/terminated/reason"))
                        .and_then(Value::as_str);
                    if reason == Some("OOMKilled") {
                        pod_oom = true;
                    }
                }
            }
        }
        if pod_oom && !name.is_empty() {
            oom.push(name.to_string());
        }
        // DB statefulset pod carries "-db-dbdepl" in its name.
        if name.contains("-db-dbdepl") {
            max_db_restarts = Some(max_db_restarts.unwrap_or(0).max(pod_restarts));
        }
    }
    (max_db_restarts, oom)
}

/// Suggested swapfile size: ~25% of RAM, clamped to a sane range.
fn recommended_swap_gb(mem_total_mb: u64) -> i64 {
    let gb = (mem_total_mb / 1024) as i64;
    ((gb + 3) / 4).clamp(SWAP_MIN_GB, SWAP_MAX_GB)
}

fn analyze(m: &HostMetrics) -> Vec<HealthFinding> {
    let mut out = Vec::new();
    let oom_pressure = !m.oomkilled_pods.is_empty()
        || m.db_max_restarts.map(|r| r >= DB_RESTARTS_WARN).unwrap_or(false);

    // 1. Swap.
    if m.swap_total_mb == 0 {
        let gb = recommended_swap_gb(m.mem_total_mb);
        out.push(HealthFinding {
            id: "swap_absent".to_string(),
            severity: if oom_pressure { "critical" } else { "warning" }.to_string(),
            title: "No swap configured".to_string(),
            detail: format!(
                "The VM has {} MB RAM and zero swap. With no swap, a memory spike (backups or world load) trips the kernel OOM-killer, which can drop Postgres and cascade a battlegroup restart.",
                m.mem_total_mb
            ),
            recommendation: format!(
                "Add a {gb} GB swapfile so spikes spill to disk instead of OOM-killing the database."
            ),
            fix_id: Some("add_swap".to_string()),
            fix_label: Some(format!("Add {gb} GB swap")),
            fix_param: Some(gb),
        });
    } else {
        // 2. Swappiness (only meaningful once swap exists).
        if let Some(sw) = m.swappiness {
            if sw > SWAPPINESS_HIGH {
                out.push(HealthFinding {
                    id: "swappiness_high".to_string(),
                    severity: "info".to_string(),
                    title: format!("Swappiness is {sw}"),
                    detail: "A high swappiness makes the kernel swap hot pages early, which can hurt game/DB latency on this box.".to_string(),
                    recommendation: format!(
                        "Lower vm.swappiness to {SWAPPINESS_TARGET} — keep hot pages in RAM but retain swap as a safety net under real pressure."
                    ),
                    fix_id: Some("set_swappiness".to_string()),
                    fix_label: Some(format!("Set swappiness to {SWAPPINESS_TARGET}")),
                    fix_param: Some(SWAPPINESS_TARGET),
                });
            }
        }
        if !m.fstab_swap {
            out.push(HealthFinding {
                id: "swap_not_persisted".to_string(),
                severity: "warning".to_string(),
                title: "Swap is active but not persisted".to_string(),
                detail: "Swap is on now but there's no /etc/fstab entry, so it won't survive a reboot.".to_string(),
                recommendation: "Re-run the swap fix to persist it (fstab + boot service).".to_string(),
                fix_id: Some("add_swap".to_string()),
                fix_label: Some("Persist swap".to_string()),
                fix_param: Some((m.swap_total_mb / 1024).max(SWAP_MIN_GB as u64) as i64),
            });
        }
    }

    // 3. Disk.
    let disk_low = m.disk_root_avail_gb > 0.0 && m.disk_root_avail_gb < DISK_LOW_GB;
    let disk_full = m.disk_root_use_pct.map(|p| p >= DISK_USE_HIGH_PCT).unwrap_or(false);
    if disk_low || disk_full {
        out.push(HealthFinding {
            id: "disk_low".to_string(),
            severity: "warning".to_string(),
            title: "Low free disk on /".to_string(),
            detail: format!(
                "Root filesystem has {:.1} GB free{}. Backups/dumps and a swapfile need headroom.",
                m.disk_root_avail_gb,
                m.disk_root_use_pct.map(|p| format!(" ({p}% used)")).unwrap_or_default()
            ),
            recommendation: "Prune old DB dumps / free space before enabling more swap or backups.".to_string(),
            fix_id: None,
            fix_label: None,
            fix_param: None,
        });
    }

    // 4. Cluster signals (only when checked).
    if let Some(restarts) = m.db_max_restarts {
        if restarts >= DB_RESTARTS_WARN {
            out.push(HealthFinding {
                id: "db_restarts_high".to_string(),
                severity: "warning".to_string(),
                title: format!("Database has restarted {restarts} times"),
                detail: "Repeated DB restarts usually mean memory pressure or instability on the node.".to_string(),
                recommendation: "Ensure swap is configured (above) and watch for OOMKilled pods; investigate if it keeps climbing.".to_string(),
                fix_id: None,
                fix_label: None,
                fix_param: None,
            });
        }
    }
    if !m.oomkilled_pods.is_empty() {
        out.push(HealthFinding {
            id: "pods_oomkilled".to_string(),
            severity: "warning".to_string(),
            title: format!("{} pod(s) were OOMKilled", m.oomkilled_pods.len()),
            detail: format!("OOMKilled: {}. The node ran out of memory and the kernel killed these.", m.oomkilled_pods.join(", ")),
            recommendation: "Add swap (above) so spikes don't OOM-kill pods; consider memory limits if it recurs.".to_string(),
            fix_id: None,
            fix_label: None,
            fix_param: None,
        });
    }

    if out.is_empty() {
        out.push(HealthFinding {
            id: "ok".to_string(),
            severity: "ok".to_string(),
            title: "No hardening issues detected".to_string(),
            detail: "Swap, swappiness, disk, and pod health all look healthy.".to_string(),
            recommendation: String::new(),
            fix_id: None,
            fix_label: None,
            fix_param: None,
        });
    }
    out
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "warning" => 2,
        "info" => 1,
        _ => 0,
    }
}

fn highest_severity(findings: &[HealthFinding]) -> &'static str {
    let max = findings.iter().map(|f| severity_rank(&f.severity)).max().unwrap_or(0);
    match max {
        3 => "critical",
        2 => "warning",
        1 => "info",
        _ => "ok",
    }
}

fn summarize(findings: &[HealthFinding]) -> String {
    let actionable = findings.iter().filter(|f| f.severity != "ok").count();
    if actionable == 0 {
        "No hardening issues detected.".to_string()
    } else {
        let fixable = findings.iter().filter(|f| f.fix_id.is_some()).count();
        format!(
            "{actionable} issue(s) found{}.",
            if fixable > 0 { format!(", {fixable} with one-click fixes") } else { String::new() }
        )
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// Idempotent swapfile creation/persistence. Hardened per the QC + Stress review:
/// single-flight lock, disk-headroom pre-check (no partial file on a full disk),
/// safe recreate on size mismatch, and never `rm` a swapfile that's still active.
fn add_swap_script(gb: i64) -> String {
    const TMPL: &str = r#"set -eu
export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH
SIZE_GB=__GB__
WANT_BYTES=$((SIZE_GB*1024*1024*1024))
# Single-flight: atomic mkdir lock so two clicks can't race.
if ! mkdir /tmp/dune-swap.lock 2>/dev/null; then echo "another swap operation is in progress" >&2; exit 1; fi
trap 'rmdir /tmp/dune-swap.lock 2>/dev/null || true' EXIT
CUR_BYTES=0
[ -f /swapfile ] && CUR_BYTES=$(stat -c %s /swapfile 2>/dev/null || echo 0)
if grep -q '^/swapfile' /proc/swaps 2>/dev/null && [ "$CUR_BYTES" = "$WANT_BYTES" ]; then
  echo "swap already active at requested size; nothing to create"
else
  # Disk headroom (requested size + 10%) BEFORE writing anything.
  FREE_KB=$(df -k / | awk 'NR==2{print $4}')
  NEED_KB=$((SIZE_GB*1024*1024)); NEED_KB=$((NEED_KB + NEED_KB/10))
  if [ "${FREE_KB:-0}" -lt "$NEED_KB" ]; then echo "not enough free disk on /: need ~${NEED_KB}KB, have ${FREE_KB:-0}KB" >&2; exit 1; fi
  # Replace any existing (wrong-size / inactive) swapfile, but never rm an active one.
  swapoff /swapfile 2>/dev/null || true
  if grep -q '^/swapfile' /proc/swaps 2>/dev/null; then echo "swapfile is still active (swapoff failed) — free memory and retry" >&2; exit 1; fi
  rm -f /swapfile
  if ! fallocate -l ${SIZE_GB}G /swapfile 2>/dev/null; then rm -f /swapfile; dd if=/dev/zero of=/swapfile bs=1M count=$((SIZE_GB*1024)) status=none; fi
  chmod 600 /swapfile
  mkswap /swapfile >/dev/null
  # Some kernels reject a fallocate'd (holey) swapfile; rebuild with dd if so.
  if ! swapon /swapfile 2>/tmp/dune_swaperr; then rm -f /swapfile; dd if=/dev/zero of=/swapfile bs=1M count=$((SIZE_GB*1024)) status=none; chmod 600 /swapfile; mkswap /swapfile >/dev/null; swapon /swapfile; fi
fi
# Persist (only reached once swap is active).
grep -q '^/swapfile' /etc/fstab || printf '%s\n' '/swapfile none swap sw 0 0' >> /etc/fstab
rc-update add swap boot >/dev/null 2>&1 || true
echo 'swap active:'; grep /swapfile /proc/swaps || true
free -m | grep -i swap
"#;
    TMPL.replace("__GB__", &gb.to_string())
}

/// Set + persist vm.swappiness.
fn set_swappiness_script(value: i64) -> String {
    const TMPL: &str = r#"set -eu
export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH
mkdir -p /etc/sysctl.d
printf '%s\n' '# Dune server-manager hardening' 'vm.swappiness=__VAL__' > /etc/sysctl.d/99-dune-swap.conf
( sysctl -w vm.swappiness=__VAL__ >/dev/null 2>&1 ) || ( echo __VAL__ > /proc/sys/vm/swappiness )
rc-update add sysctl boot >/dev/null 2>&1 || true
echo "swappiness now: $(cat /proc/sys/vm/swappiness)"
"#;
    TMPL.replace("__VAL__", &value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(probe: &str) -> HostMetrics {
        parse_probe(probe)
    }

    #[test]
    fn parses_probe_lines() {
        let m = metrics(
            "MEM_TOTAL_KB=30813696\nMEM_AVAIL_KB=14237696\nSWAP_TOTAL_KB=0\nSWAP_FREE_KB=0\nSWAPPINESS=60\nDISK_ROOT_AVAIL_KB=68419584\nDISK_ROOT_USE_PCT=31\nFSTAB_SWAP=no\n",
        );
        assert_eq!(m.mem_total_mb, 30091);
        assert_eq!(m.swap_total_mb, 0);
        assert_eq!(m.swappiness, Some(60));
        assert!(!m.fstab_swap);
        assert!((m.disk_root_avail_gb - 65.25).abs() < 0.2);
        assert_eq!(m.disk_root_use_pct, Some(31));
    }

    #[test]
    fn recommends_swap_when_absent() {
        let m = metrics("MEM_TOTAL_KB=30813696\nSWAP_TOTAL_KB=0\nSWAP_FREE_KB=0\nSWAPPINESS=60\nDISK_ROOT_AVAIL_KB=68419584\nDISK_ROOT_USE_PCT=31\nFSTAB_SWAP=no\n");
        let f = analyze(&m);
        let swap = f.iter().find(|x| x.id == "swap_absent").expect("swap finding");
        assert_eq!(swap.severity, "warning");
        assert_eq!(swap.fix_id.as_deref(), Some("add_swap"));
        assert_eq!(swap.fix_param, Some(8)); // ~25% of 29 GB, clamped
    }

    #[test]
    fn swap_absent_is_critical_under_oom_pressure() {
        let mut m = metrics("MEM_TOTAL_KB=30813696\nSWAP_TOTAL_KB=0\nSWAP_FREE_KB=0\n");
        m.oomkilled_pods = vec!["dump-pod".to_string()];
        let f = analyze(&m);
        let swap = f.iter().find(|x| x.id == "swap_absent").unwrap();
        assert_eq!(swap.severity, "critical");
        assert_eq!(highest_severity(&f), "critical");
    }

    #[test]
    fn flags_high_swappiness_only_with_swap() {
        let m = metrics("MEM_TOTAL_KB=8388608\nSWAP_TOTAL_KB=4194304\nSWAP_FREE_KB=4194304\nSWAPPINESS=60\nFSTAB_SWAP=yes\n");
        let f = analyze(&m);
        assert!(f.iter().any(|x| x.id == "swappiness_high"));
        assert!(f.iter().all(|x| x.id != "swap_absent"));
    }

    #[test]
    fn clean_host_reports_ok() {
        let m = metrics("MEM_TOTAL_KB=8388608\nSWAP_TOTAL_KB=4194304\nSWAP_FREE_KB=4194304\nSWAPPINESS=10\nDISK_ROOT_AVAIL_KB=68419584\nDISK_ROOT_USE_PCT=31\nFSTAB_SWAP=yes\n");
        let f = analyze(&m);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "ok");
        assert_eq!(highest_severity(&f), "ok");
    }

    #[test]
    fn parses_pod_health_oom_and_restarts() {
        let json: Value = serde_json::from_str(r#"{
            "items": [
              {"metadata":{"name":"x-db-dbdepl-sts-0"},
               "status":{"containerStatuses":[{"restartCount":8,"lastState":{"terminated":{"reason":"Error"}}}]}},
              {"metadata":{"name":"x-dump-pod"},
               "status":{"containerStatuses":[{"restartCount":0,"lastState":{"terminated":{"reason":"OOMKilled"}}}]}}
            ]}"#).unwrap();
        let (restarts, oom) = parse_pod_health(&json);
        assert_eq!(restarts, Some(8));
        assert_eq!(oom, vec!["x-dump-pod".to_string()]);
    }
}
