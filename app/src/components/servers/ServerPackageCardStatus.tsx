import type { RemoteServerPackageStatus } from "../../types/server";
import MetricTile, { type MetricTone } from "../ui/MetricTile";

export type ServerPackageCardStatusProps = {
  guestPackage?: RemoteServerPackageStatus;
};

export default function ServerPackageCardStatus({ guestPackage }: ServerPackageCardStatusProps) {
  if (!guestPackage) return null;
  const downloaded = guestPackage.battlegroupVersion ?? "";
  const live = guestPackage.liveBattlegroupVersion ?? "";
  const liveTone: MetricTone = downloaded && live && downloaded !== live ? "warning" : "muted";
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
        gap: "12px",
        marginTop: "8px",
      }}
    >
      <MetricTile label="Installed build" value={guestPackage.installedBuildId ?? "—"} tone="muted" />
      <MetricTile label="Downloaded" value={downloaded || "—"} tone="muted" />
      <MetricTile label="Running" value={live || "—"} tone={liveTone} />
      <MetricTile label="Operator" value={guestPackage.operatorVersion ?? "—"} tone="muted" />
    </div>
  );
}

