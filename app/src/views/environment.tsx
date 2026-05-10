import { RefreshCw } from "lucide-react";
import { InfoRow } from "../components/primitives";
import type { AppConfig, VmStatus } from "../types";

type EnvironmentPanelProps = {
  config: AppConfig;
  vm: VmStatus | null;
  managerInstallNamespace: string;
  configSaved: boolean;
  busy: boolean;
  onDetect: () => void;
};

export function EnvironmentPanel({
  config,
  vm,
  managerInstallNamespace,
  configSaved,
  busy,
  onDetect
}: EnvironmentPanelProps) {
  return (
    <section className="settings-band">
      <div className="panel-title">
        <h2>Detected Environment</h2>
        <button onClick={onDetect} disabled={busy}>
          <RefreshCw size={16} />
          Detect
        </button>
      </div>
      <div className="detected-grid">
        <InfoRow label="Server install path" value={config.installPath || "Not found"} />
        <InfoRow label="VM name" value={config.vmName || "Not found"} />
        <InfoRow label="VM IP" value={config.vmIp || vm?.ipAddresses?.[0] || "Not found"} />
        <InfoRow label="SSH user" value={config.sshUser || "Not found"} />
        <InfoRow label="SSH path" value={config.sshPath || "Not found"} />
        <InfoRow label="Manager API URL" value={config.managerApiUrl || "Not installed"} />
        <InfoRow label="Manager namespace" value={managerInstallNamespace || "Not detected"} />
        <InfoRow label="Manager binary" value={config.managerApiBinaryPath || "Not found"} />
        <InfoRow label="Director internal URL" value={config.managerApiDirectorUrl || "Not detected"} />
        <InfoRow label="Manager token" value={config.managerApiToken ? "Stored" : "Will be generated on install"} />
      </div>
      {configSaved && <p className="success-line">Saved to app config.json</p>}
    </section>
  );
}
