import { Box, Flex, Text, Button, Link } from "@radix-ui/themes";
import {
  type ServerTunnelStatus,
  type ServerTunnelStartRequest,
  type TunnelService
} from "../../types";
import { serverTunnelKey } from "../../utils/storage";

export function BusySpinner() {
  return (
    <Box
      className="inline-spinner"
      aria-hidden
      style={{
        display: "inline-block",
        width: "12px",
        height: "12px",
        border: "2px solid rgba(255,255,255,0.2)",
        borderTopColor: "var(--bronze-9)",
        borderRadius: "50%",
        animation: "spin 0.8s linear infinite"
      }}
    />
  );
}

export function ServerTunnelControls({
  serverKey,
  namespace,
  host,
  serverKind,
  vmName,
  user,
  keyPath,
  canStartDirectorTunnel,
  canStartFileBrowserTunnel,
  canStartDatabaseTunnel,
  canStartPgHeroTunnel,
  tunnels,
  tunnelBusy,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
}: {
  serverKey: string;
  namespace: string;
  host: string;
  serverKind: "hyperv" | "ubuntu" | "alpine";
  vmName?: string;
  user?: string;
  keyPath?: string;
  canStartDirectorTunnel: boolean;
  canStartFileBrowserTunnel: boolean;
  canStartDatabaseTunnel: boolean;
  canStartPgHeroTunnel: boolean;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onStartTunnel?: (request: ServerTunnelStartRequest) => void;
  onStopTunnel?: (tunnelId: string) => void;
  onOpenTunnel?: (tunnel: ServerTunnelStatus) => void;
}) {
  const services: Array<{ service: TunnelService; label: string }> = [
    { service: "director", label: "Director UI" },
    { service: "fileBrowser", label: "File Browser" },
    { service: "database", label: "Postgres" },
    { service: "pgHero", label: "PgHero" },
  ];
  return (
    <Box className="tunnel-controls" mt="3">
      <Flex direction="column" gap="2">
        {services.map(({ service, label }) => {
          const tunnelId = serverTunnelKey(serverKey, service);
          const active = tunnels[tunnelId];
          const busy = !!tunnelBusy[tunnelId];
          const serviceAvailable =
            service === "director"
              ? canStartDirectorTunnel
              : service === "pgHero"
                ? canStartPgHeroTunnel
              : service === "database"
                ? canStartDatabaseTunnel
                : canStartFileBrowserTunnel;
          const openLabel = service === "database" ? "Copy URI" : `Open ${label}`;
          const disabled =
            busy || !onStopTunnel || (!active && (!serviceAvailable || !host.trim() || !namespace.trim() || !onStartTunnel));
          return (
            <Flex key={service} align="center" justify="between" gap="3" wrap="wrap" className="tunnel-row">
              <Flex direction="column" gap="1" minWidth="0">
                <Text size="2" weight="medium">
                  {label}
                </Text>
                <Text size="1" color="gray">
                  {active
                    ? `Forwarding remote port ${active.remotePort} to local port ${active.localPort}`
                    : !serviceAvailable
                      ? service === "director"
                        ? "Requires started BattleGroup and healthy Director"
                        : "Requires started BattleGroup"
                      : !host.trim() || !namespace.trim()
                        ? "Requires detected server namespace and IP"
                        : "Tunnel stopped"}
                </Text>
              </Flex>
              <Flex align="center" gap="2" wrap="wrap" justify="end">
                {active ? (
                  <Button
                    type="button"
                    size="1"
                    variant="surface"
                    onClick={() => onOpenTunnel?.(active)}
                  >
                    {openLabel}
                  </Button>
                ) : null}
                <Button
                  type="button"
                  size="1"
                  variant={active ? "soft" : "surface"}
                  color={active ? "red" : undefined}
                  disabled={disabled}
                  onClick={() => {
                    if (active) {
                      onStopTunnel?.(tunnelId);
                      return;
                    }
                    onStartTunnel?.({
                      tunnelId,
                      serverKind,
                      service,
                      host,
                      user,
                      keyPath,
                      vmName,
                      namespace,
                    });
                  }}
                >
                  {busy ? (
                    <Flex align="center" gap="1">
                      <BusySpinner /> Working
                    </Flex>
                  ) : active ? (
                    `Stop Tunnel`
                  ) : (
                    `Start Tunnel`
                  )}
                </Button>
                {active ? (
                  <Link
                    size="1"
                    href="#"
                    className="mono tunnel-url"
                    onClick={(event) => {
                      event.preventDefault();
                      onOpenTunnel?.(active);
                    }}
                  >
                    {active.url}
                  </Link>
                ) : null}
              </Flex>
            </Flex>
          );
        })}
      </Flex>
    </Box>
  );
}
