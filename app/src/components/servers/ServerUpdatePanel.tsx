import { Box, Flex, Text } from "@radix-ui/themes";

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
    <Flex direction="column" gap="4" mt="3">
      <div>
        <Text size="2" weight="bold" className="font-display" mb="2" style={{ display: "block", color: "var(--color-text-primary)" }}>
          Package versions
        </Text>
        {status?.package ? (
          <ServerPackageCardStatus guestPackage={status.package} />
        ) : (
          <Box
            className="bracket chamfer"
            p="4"
            style={{
              background: "var(--color-bg-panel)",
              border: "1px solid var(--color-border-hair)",
              borderRadius: "var(--radius-3)",
            }}
          >
            <Text size="2" style={{ color: "var(--color-text-muted)" }}>
              No package information yet. Refresh the server to fetch versions.
            </Text>
          </Box>
        )}
      </div>

      <div>
        <Text size="2" weight="bold" className="font-display" mb="2" style={{ display: "block", color: "var(--color-text-primary)" }}>
          Apply update
        </Text>
        <Box
          className="bracket chamfer"
          p="4"
          style={{
            background: "var(--color-bg-panel)",
            border: "1px solid var(--color-border-hair)",
            borderRadius: "var(--radius-3)",
          }}
        >
          {updateAvailable ? (
            <Flex direction="column" gap="3">
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
                  className="chamfer-sm"
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
        </Box>
      </div>
    </Flex>
  );
}

