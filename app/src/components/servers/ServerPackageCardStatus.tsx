import type { RemoteServerPackageStatus } from "../../types/server";
import Metric, { type MetricTone } from "../ui/Metric";

export type ServerPackageCardStatusProps = {
  guestPackage?: RemoteServerPackageStatus;
};

export default function ServerPackageCardStatus({ guestPackage }: ServerPackageCardStatusProps) {
  if (!guestPackage) return null;
  const downloaded = guestPackage.battlegroupVersion ?? "";
  const live = guestPackage.liveBattlegroupVersion ?? "";
  const liveTone: MetricTone = downloaded && live && downloaded !== live ? "warn" : "default";
  return (
    <div className="metric-grid">
      <Metric label="Installed build" value={guestPackage.installedBuildId ?? ""} />
      <Metric label="Downloaded" value={downloaded} />
      <Metric label="Running" value={live} tone={liveTone} />
      <Metric label="Operator" value={guestPackage.operatorVersion ?? ""} />
    </div>
  );
}
