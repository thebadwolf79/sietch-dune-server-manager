import { useCallback, useEffect, useState } from "react";
import { Flex } from "@radix-ui/themes";

import { vmGetState, vmHostReadiness, vmStart, vmStop } from "../../services/tauri";
import {
  battlegroupActionsEnabled,
  canManageVm,
  canStartVm,
  systemStateLabel,
  type SystemState,
} from "../../types/vm";
import ActionButton from "../ui/ActionButton";
import Metric from "../ui/Metric";

// Funcom's self-hosted VM is created with this fixed name by initial-setup.
const DEFAULT_VM_NAME = "dune-awakening";

/**
 * Host-only Hyper-V VM power controls (issue #28).
 *
 * Renders nothing unless this machine can manage the VM (i.e. the manager is
 * running on the Hyper-V host) — on a remote/connect-only machine the probe
 * reports not-capable and the section stays hidden. When capable, it shows the
 * VM state and Start/Stop actions, gated on the same `SystemState` vocabulary the
 * Rust backend uses.
 */
export default function VmPowerControls({ vmName }: { vmName?: string }) {
  // Fall back to Funcom's default whether the name is absent, empty, or blank —
  // the parent may pass `worldUniqueName || undefined`, but guard the empty
  // string case too so a misconfigured record can't target a "" VM.
  const resolvedVmName = vmName?.trim() || DEFAULT_VM_NAME;
  const [capable, setCapable] = useState<boolean | null>(null); // null = still probing
  const [state, setState] = useState<SystemState | null>(null);
  const [busy, setBusy] = useState<null | "start" | "stop">(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setState(await vmGetState(resolvedVmName));
    } catch (err) {
      setError(String(err));
    }
  }, [resolvedVmName]);

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const readiness = await vmHostReadiness();
        if (!active) return;
        const ok = canManageVm(readiness);
        setCapable(ok);
        if (ok) await refresh();
      } catch {
        // Probe failed (remote machine / no Hyper-V) -> connect-only mode.
        if (active) setCapable(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [refresh]);

  // Still probing, connect-only, or no state yet: render nothing.
  if (capable !== true || !state) return null;

  const onStart = async () => {
    setBusy("start");
    setError(null);
    try {
      setState(await vmStart(resolvedVmName));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  };

  const onStop = async () => {
    setBusy("stop");
    setError(null);
    try {
      setState(await vmStop(resolvedVmName));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Flex direction="column" gap="3">
      <div className="metric-grid">
        <Metric label="Virtual Machine" value={resolvedVmName} />
        <Metric label="VM State" value={systemStateLabel(state)} />
      </div>
      {error ? <div className="server-error">{error}</div> : null}
      <div className="action-row">
        <ActionButton
          onClick={onStart}
          busy={busy === "start"}
          disabled={!!busy || !canStartVm(state)}
          tone="accent"
          pendingLabel="Starting VM"
          title="Power on the Hyper-V VM"
        >
          Start VM
        </ActionButton>
        <ActionButton
          onClick={onStop}
          busy={busy === "stop"}
          disabled={!!busy || !battlegroupActionsEnabled(state)}
          tone="danger"
          pendingLabel="Stopping VM"
          title="Turn off the Hyper-V VM"
        >
          Stop VM
        </ActionButton>
      </div>
    </Flex>
  );
}
