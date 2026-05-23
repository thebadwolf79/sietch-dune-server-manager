import { Grid } from "@radix-ui/themes";

import type { RemoteServerPackageStatus } from "../../types/server";
import Metric from "../ui/Metric";

export type ServerPackageCardStatusProps = {
  guestPackage?: RemoteServerPackageStatus;
};

export default function ServerPackageCardStatus({ guestPackage }: ServerPackageCardStatusProps) {
  if (!guestPackage) return null;
  return (
    <Grid columns="4" gap="3" mt="3">
      <Metric label="Installed Build" value={guestPackage.installedBuildId || "unknown"} />
      <Metric label="BattleGroup Version" value={guestPackage.battlegroupVersion || "unknown"} />
      <Metric label="Live Version" value={guestPackage.liveBattlegroupVersion || "unknown"} />
      <Metric label="Operator" value={guestPackage.operatorVersion || "unknown"} />
    </Grid>
  );
}
