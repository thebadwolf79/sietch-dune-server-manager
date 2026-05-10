import { HardDrive, Play, Square, Wifi } from "lucide-react";
import { InfoRow, StatusInfoRow } from "../components/primitives";
import type { GuestConnection, HostStatus, VmStatus } from "../types";
import { formatBytes, vmHealthLabel } from "../utils";

type VmRequiredNoticeProps = {
  vm: VmStatus;
  busy: boolean;
  canControlVm: boolean;
  vmIsRunning: boolean;
  vmIsChanging: boolean;
  vmIsStarting: boolean;
  onStart: () => void;
};

export function VmRequiredNotice({
  vm,
  busy,
  canControlVm,
  vmIsRunning,
  vmIsChanging,
  vmIsStarting,
  onStart
}: VmRequiredNoticeProps) {
  return (
    <section className="tool-required panel">
      <div>
        <HardDrive size={24} />
        <h2>{vmIsChanging ? `VM is ${vm.state}` : "VM must be running"}</h2>
      </div>
      <p>
        {vmIsStarting
          ? "Hyper-V is starting the VM. Guest SSH, Manager API, BattleGroups, Director telemetry, and logs will load once the VM reports Running and has an IP address."
          : "Guest SSH, Manager API, BattleGroups, Director telemetry, and logs are skipped until Hyper-V reports the VM state as Running and an IP address is available."}
      </p>
      <button onClick={onStart} disabled={busy || !canControlVm || vmIsRunning || vmIsChanging}>
        <Play size={16} />
        Start VM
      </button>
    </section>
  );
}

type HostVmPanelsProps = {
  host: HostStatus | null;
  vm: VmStatus | null;
  guest: GuestConnection | null;
  busy: boolean;
  canControlVm: boolean;
  vmIsRunning: boolean;
  vmIsChanging: boolean;
  startVmDisabledReason: string;
  stopVmDisabledReason: string;
  onStart: () => void;
  onStop: () => void;
};

export function HostVmPanels({
  host,
  vm,
  guest,
  busy,
  canControlVm,
  vmIsRunning,
  vmIsChanging,
  startVmDisabledReason,
  stopVmDisabledReason,
  onStart,
  onStop
}: HostVmPanelsProps) {
  return (
    <section className="grid two">
      <article className="panel">
        <div className="panel-title">
          <h2>Host & VM</h2>
          <div className="button-row">
            <button
              onClick={onStart}
              disabled={busy || !canControlVm || vmIsRunning || vmIsChanging}
              title={startVmDisabledReason}
            >
              <Play size={16} />
              Start VM
            </button>
            <button
              onClick={onStop}
              disabled={busy || !canControlVm || !vmIsRunning || vmIsChanging}
              title={stopVmDisabledReason}
            >
              <Square size={16} />
              Stop VM
            </button>
          </div>
        </div>
        <InfoRow label="Hyper-V" value={host?.hypervAvailable ? "Available" : "Unavailable"} />
        <InfoRow label="vmms service" value={host?.vmmsStatus} />
        <StatusInfoRow label="VM state" value={vm?.state} />
        <InfoRow label="Hyper-V health" value={vmHealthLabel(vm?.state, vm?.status)} />
        <InfoRow label="Memory" value={vm ? formatBytes(vm.memoryAssignedBytes) : null} />
        <InfoRow label="Uptime" value={vm?.uptime} />
        <InfoRow label="VM path" value={vm?.path} />
      </article>

      <article className="panel">
        <div className="panel-title">
          <h2>Guest Connection</h2>
          <Wifi size={19} />
        </div>
        <InfoRow label="IP" value={guest?.ip ?? vm?.ipAddresses?.[0]} />
        <InfoRow label="SSH user" value={guest?.sshUser} />
        <InfoRow label="Hostname" value={guest?.hostname} />
        <InfoRow label="Kernel" value={guest?.kernel} />
        <InfoRow label="Passwordless sudo" value={guest?.sudo ? "Ready" : "Unavailable"} />
        <InfoRow label="kubectl" value={guest?.kubectl ? "Ready" : "Unavailable"} />
      </article>
    </section>
  );
}
