import { Flex, Text } from "@radix-ui/themes";

import type { RemoteServerRecord, RemoteServerStatus } from "../../types/server";
import { hasBattlegroupUpdateAvailable } from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import ServerPackageCardStatus from "./ServerPackageCardStatus";

export type ServerUpdatePanelProps = {
  server: RemoteServerRecord;
  status?: RemoteServerStatus;
  busyLabel?: string;
  onUpdateBattlegroup: () => void;
};

/**
 * Per-server Update sub-tab: package versions strip + Update Server action
 * (only enabled when the wrapper has staged a newer version than what is
 * currently running in Kubernetes).
 */
export default function ServerUpdatePanel({
  server: _server,
  status,
  busyLabel,
  onUpdateBattlegroup,
}: ServerUpdatePanelProps) {
  const updateAvailable = hasBattlegroupUpdateAvailable(status?.package);
  const busy = !!busyLabel;
  return (
    <Flex direction="column" gap="4">
      <div>
        <div className="section-title">Package versions</div>
        {status?.package ? (
          <ServerPackageCardStatus guestPackage={status.package} />
        ) : (
          <Text size="2" style={{ color: "var(--color-text-muted)" }}>
            No package information yet. Refresh the server to fetch versions.
          </Text>
        )}
      </div>

      <div>
        <div className="section-title">Apply update</div>
        {updateAvailable ? (
          <Flex direction="column" gap="2">
            <Text size="2" style={{ color: "var(--color-text-secondary)" }}>
              A newer battlegroup version is downloaded on the host. Apply it to roll the
              running images.
            </Text>
            <div>
              <ActionButton
                onClick={onUpdateBattlegroup}
                busy={busy}
                disabled={busy || !status}
                tone="accent"
                pendingLabel="Updating"
                title="Run vendor `battlegroup update` (steamcmd + operators + maps + images)"
              >
                Update Server
              </ActionButton>
            </div>
          </Flex>
        ) : (
          <Text size="2" style={{ color: "var(--color-text-muted)" }}>
            The downloaded battlegroup version matches what is currently running. No
            update pending.
          </Text>
        )}
      </div>
    </Flex>
  );
}
