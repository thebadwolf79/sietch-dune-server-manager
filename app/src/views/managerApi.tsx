import { Map, PackagePlus, RadioTower, RefreshCw } from "lucide-react";
import { InfoRow } from "../components/primitives";
import type { AppConfig, ManagerApiInstallResult, ManagerApiStatus, TelemetryEnvelope } from "../types";

type ManagerApiPanelProps = {
  config: AppConfig;
  managerInstallNamespace: string;
  managerReadiness: string;
  managerTelemetryState: string;
  managerStatus: ManagerApiStatus | null;
  managerTelemetry: TelemetryEnvelope | null;
  managerInstall: ManagerApiInstallResult | null;
  managerError: string;
  busy: boolean;
  canInstallManagerApi: boolean;
  onInstall: () => void;
};

export function ManagerApiPanel({
  config,
  managerInstallNamespace,
  managerReadiness,
  managerTelemetryState,
  managerStatus,
  managerTelemetry,
  managerInstall,
  managerError,
  busy,
  canInstallManagerApi,
  onInstall
}: ManagerApiPanelProps) {
  return (
    <section className="panel">
      <div className="panel-title">
        <h2>Manager API</h2>
        <div className="button-row">
          <button onClick={onInstall} disabled={busy || !canInstallManagerApi}>
            <PackagePlus size={16} />
            Install Tool
          </button>
          <RadioTower size={19} />
        </div>
      </div>
      <section className="config-summary">
        <InfoRow label="URL" value={config.managerApiUrl || "Not configured"} />
        <InfoRow label="Install namespace" value={managerInstallNamespace || "Not configured"} />
        <InfoRow label="Binary" value={config.managerApiBinaryPath || "Not configured"} />
        <InfoRow label="API" value={managerReadiness} />
        <InfoRow label="Telemetry socket" value={managerTelemetryState} />
        <InfoRow label="Namespace" value={managerStatus?.namespace} />
        <InfoRow label="Director bridge" value={managerStatus?.directorConfigured ? "Configured" : "Unavailable"} />
        <InfoRow
          label="Telemetry"
          value={
            managerTelemetry?.payload
              ? `${managerTelemetry.payload.pods?.length ?? 0} pods, ${
                  managerTelemetry.payload.services?.length ?? 0
                } services`
              : "No events yet"
          }
        />
        <InfoRow
          label="Snapshot counts"
          value={
            managerStatus
              ? `${managerStatus.battlegroups} battlegroups, ${managerStatus.pods} pods, ${managerStatus.services} services`
              : "Unknown"
          }
        />
      </section>
      {managerInstall && (
        <p className="success-line">
          Installed {managerInstall.deployment} in {managerInstall.namespace}
        </p>
      )}
      {managerError && <p className="subtle-line">{managerError}</p>}
    </section>
  );
}

type ManagerToolsRequiredNoticeProps = {
  busy: boolean;
  canInstallManagerApi: boolean;
  onInstall: () => void;
};

export function ManagerToolsRequiredNotice({
  busy,
  canInstallManagerApi,
  onInstall
}: ManagerToolsRequiredNoticeProps) {
  return (
    <section className="tool-required panel">
      <div>
        <RadioTower size={24} />
        <h2>Manager tools must be installed</h2>
      </div>
      <p>
        BattleGroups, live config, pods, services, logs, and server actions are hidden until the Manager API is
        installed and reachable.
      </p>
      <button onClick={onInstall} disabled={busy || !canInstallManagerApi}>
        <PackagePlus size={16} />
        Install Tool
      </button>
    </section>
  );
}

type DirectorUnavailableNoticeProps = {
  busy: boolean;
  onRefresh: () => void;
};

export function DirectorUnavailableNotice({ busy, onRefresh }: DirectorUnavailableNoticeProps) {
  return (
    <section className="tool-required panel">
      <div>
        <Map size={24} />
        <h2>Director bridge is unavailable</h2>
      </div>
      <p>
        Native player telemetry, map runtime state, and the advanced Director console need the Manager API to detect
        and reach the internal Director service.
      </p>
      <button onClick={onRefresh} disabled={busy}>
        <RefreshCw size={16} />
        Refresh
      </button>
    </section>
  );
}
