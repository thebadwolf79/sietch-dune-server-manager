import { Card, Flex, Box, Heading, Text, Button, Badge, Grid, Link, TextArea } from "@radix-ui/themes";
import {
  type DuneVmCandidate,
  type RemoteServerRecord,
  type RemoteServerStatus,
  type RemoteServerComponent,
  type LocalHyperVRuntime,
  type ProxmoxVmStatus,
  type ServerTunnelStatus,
  type ServerTunnelStartRequest,
  type TunnelService,
  type ServerPackageStatus,
  type RemoteServerPackageStatus
} from "../types";
import {
  localServerKey,
  remoteServerDefaultUser,
  remoteServerKindLabel,
  serverTunnelKey,
  componentLogStateKey,
  isCriticalRestartComponent,
  primaryLocalServerIp,
  copyTextToClipboard,
  isBattlegroupStarted,
  isDirectorReadyPhase
} from "../utils/storage";
import { formatGiB, formatDuration } from "../utils/helpers";
import { Metric, InfoRow } from "./Common";

function BusySpinner() {
  return <Box className="inline-spinner" aria-hidden style={{ display: "inline-block", width: "12px", height: "12px", border: "2px solid rgba(255,255,255,0.2)", borderTopColor: "var(--bronze-9)", borderRadius: "50%", animation: "spin 0.8s linear infinite" }} />;
}

export function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <Box className="empty-state" p="5" style={{ textAlign: "center", border: "1px dashed rgba(255,255,255,0.1)", borderRadius: "8px", background: "rgba(0,0,0,0.1)" }}>
      <Heading size="3" mb="1">{title}</Heading>
      <Text as="p" size="2" color="gray">
        {body}
      </Text>
    </Box>
  );
}

export function ServerPackageCardStatus({
  guestPackage,
  packageStatus,
}: {
  guestPackage?: RemoteServerPackageStatus;
  packageStatus: ServerPackageStatus | null;
}) {
  if (!guestPackage && !packageStatus) return null;
  const installed = guestPackage?.installedBuildId || null;
  const latest = packageStatus?.latestBuildId || packageStatus?.installedBuildId || null;
  const downloadedImage = guestPackage?.battlegroupVersion || null;
  const liveImage = guestPackage?.liveBattlegroupVersion || null;
  const updateRequired = Boolean(installed && latest && installed !== latest);
  const tone = !installed ? "amber" : updateRequired ? "amber" : "green";
  const label = !installed ? "Build unknown" : updateRequired ? "Update required" : "Current";
  return (
    <Flex align="center" gap="2" mt="3" wrap="wrap">
      <Metric label="Server Package" value={installed || "unknown"} />
      <Badge color={tone} variant="surface">
        {label}
      </Badge>
      {latest ? (
        <Text size="1" color="gray" className="mono">
          latest {latest}
        </Text>
      ) : null}
      {downloadedImage ? (
        <Text size="1" color="gray" className="mono">
          images {downloadedImage}
        </Text>
      ) : null}
      {liveImage && liveImage !== downloadedImage ? (
        <Text size="1" color="gray" className="mono">
          live {liveImage}
        </Text>
      ) : null}
    </Flex>
  );
}

export function serverPackageUpdateRequired(
  guestPackage: RemoteServerPackageStatus | undefined,
  packageStatus: ServerPackageStatus | null,
): boolean {
  const installed = guestPackage?.installedBuildId?.trim();
  const latest = (packageStatus?.latestBuildId || packageStatus?.installedBuildId || "").trim();
  return Boolean(installed && latest && installed !== latest);
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

export function ComponentHealthGroup({
  title,
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: {
  title: string;
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog?: (component: RemoteServerComponent) => void;
  onRestart?: (component: RemoteServerComponent) => void;
}) {
  if (components.length === 0) return null;
  return (
    <details className="component-group" style={{ width: "100%" }}>
      <summary className="component-group-summary" style={{ listStyle: "none", cursor: "pointer", padding: "6px 0" }}>
        <Flex align="center" justify="between" gap="2">
          <Text size="1" weight="medium" color="gray" className="component-group-title">
            {title}
          </Text>
          <Badge color="gray" variant="soft">
            {components.length}
          </Badge>
        </Flex>
      </summary>
      <Flex direction="column" gap="2" mt="2">
        {components.map((component) => {
          const logKey = componentLogStateKey(serverKey, component);
          const logText = logs[logKey];
          const busy = !!logBusy[logKey];
          const restarting = !!restartBusy[logKey];
          return (
            <details key={`${component.logKey}-${component.name}`} className="component-row" style={{ listStyle: "none", border: "1px solid rgba(255,255,255,0.05)", borderRadius: "4px", overflow: "hidden", marginBottom: "4px" }}>
              <summary className="component-summary" style={{ listStyle: "none", cursor: "pointer", padding: "8px", background: "rgba(255,255,255,0.02)" }}>
                <Flex align="center" justify="between" gap="3" width="100%">
                  <Box minWidth="0">
                    <Flex align="center" gap="2" wrap="wrap">
                      <Text size="2" weight="medium">
                        {component.name}
                      </Text>
                      <Badge color={component.tone} variant="soft">
                        {component.state}
                      </Badge>
                    </Flex>
                    <Text as="div" size="2" color="gray" className="component-summary-text">
                      {component.summary}
                    </Text>
                  </Box>
                  <Flex gap="2" style={{ flexShrink: 0 }}>
                    <Button
                      type="button"
                      size="1"
                      variant="surface"
                      disabled={busy || restarting}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const row = event.currentTarget.closest("details");
                        if (row) row.open = true;
                        onRefreshLog?.(component);
                      }}
                    >
                      {busy ? "Loading logs" : logText ? "Refresh logs" : "View logs"}
                    </Button>
                    <Button
                      type="button"
                      size="1"
                      color={isCriticalRestartComponent(component) ? "amber" : "bronze"}
                      variant="soft"
                      disabled={busy || restarting}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const row = event.currentTarget.closest("details");
                        if (row) row.open = true;
                        onRestart?.(component);
                      }}
                    >
                      {restarting ? "Restarting" : "Restart"}
                    </Button>
                  </Flex>
                </Flex>
              </summary>
              <Box className="component-body" p="3" style={{ background: "rgba(0,0,0,0.15)", borderTop: "1px solid rgba(255,255,255,0.05)" }}>
                {component.details.length > 0 ? (
                  <ul className="component-details" style={{ margin: 0, paddingLeft: "16px", color: "var(--gray-9)", fontSize: "13px" }}>
                    {component.details.map((detail) => (
                      <li key={detail}>{detail}</li>
                    ))}
                  </ul>
                ) : (
                  <Text as="div" size="1" color="gray">
                    No additional details reported.
                  </Text>
                )}
                {logText ? (
                  <>
                    <Flex justify="end" mt="2">
                      <Button
                        type="button"
                        size="1"
                        variant="soft"
                        onClick={() => void copyTextToClipboard(logText)}
                      >
                        Copy logs
                      </Button>
                    </Flex>
                    <Box className="component-log" mt="2" p="2" style={{ background: "rgba(0,0,0,0.3)", borderRadius: "4px", maxHeight: "150px", overflowY: "auto" }}>
                      {logText.split(/\r?\n/).map((line, index) => (
                        <Text as="div" size="1" className="mono" key={`${component.logKey}-${index}`} style={{ whiteSpace: "pre-wrap" }}>
                          {line || "\u00a0"}
                        </Text>
                      ))}
                    </Box>
                  </>
                ) : null}
              </Box>
            </details>
          );
        })}
      </Flex>
    </details>
  );
}

export function ComponentHealthList({
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: {
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog?: (component: RemoteServerComponent) => void;
  onRestart?: (component: RemoteServerComponent) => void;
}) {
  if (components.length === 0) return null;
  const systems = components.filter((component) => component.category !== "map");
  const maps = components.filter((component) => component.category === "map");
  return (
    <Box className="component-health" mt="3">
      <Flex direction="column" gap="3">
        <ComponentHealthGroup
          title="Systems"
          serverKey={serverKey}
          components={systems}
          logs={logs}
          logBusy={logBusy}
          restartBusy={restartBusy}
          onRefreshLog={onRefreshLog}
          onRestart={onRestart}
        />
        <ComponentHealthGroup
          title="Maps"
          serverKey={serverKey}
          components={maps}
          logs={logs}
          logBusy={logBusy}
          restartBusy={restartBusy}
          onRefreshLog={onRefreshLog}
          onRestart={onRestart}
        />
      </Flex>
    </Box>
  );
}

export function ServerCard({
  candidate,
  compact = false,
  runtime,
  runtimeError,
  packageStatus,
  componentLogs,
  componentLogBusy,
  componentRestartBusy,
  busyLabel,
  tunnels,
  tunnelBusy,
  onRemove,
  onRefresh,
  onStart,
  onStop,
  onStartBattlegroup,
  onStopBattlegroup,
  onUpdateBattlegroup,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshComponentLog,
  onRestartComponent,
}: {
  candidate: DuneVmCandidate;
  compact?: boolean;
  runtime?: LocalHyperVRuntime;
  runtimeError?: string;
  packageStatus: ServerPackageStatus | null;
  componentLogs: Record<string, string>;
  componentLogBusy: Record<string, boolean>;
  componentRestartBusy: Record<string, boolean>;
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onRemove?: () => void;
  onRefresh?: () => void;
  onStart?: () => void;
  onStop?: () => void;
  onStartBattlegroup?: () => void;
  onStopBattlegroup?: () => void;
  onUpdateBattlegroup?: () => void;
  onStartTunnel?: (request: ServerTunnelStartRequest) => void;
  onStopTunnel?: (tunnelId: string) => void;
  onOpenTunnel?: (tunnel: ServerTunnelStatus) => void;
  onRefreshComponentLog?: (component: RemoteServerComponent) => void;
  onRestartComponent?: (component: RemoteServerComponent) => void;
}) {
  const vm = candidate.vm;
  const primaryIp = vm.ipv4Addresses[0] ?? "No IPv4 reported";
  const diskLabel = vm.diskSizeBytes > 0 ? `${formatGiB(vm.diskSizeBytes)} disk` : "Disk size unknown";
  const usedDiskLabel = vm.diskFileSizeBytes > 0 ? `${formatGiB(vm.diskFileSizeBytes)} used` : "usage unknown";
  const busy = !!busyLabel;
  const canStart = vm.state === "off" || vm.state === "saved" || vm.state === "paused";
  const canStop = vm.state === "running" || vm.state === "starting" || vm.state === "paused";
  const battlegroup = runtime?.status?.battlegroup;
  const guestPackage = runtime?.status?.package;
  const serverUpdateRequired = serverPackageUpdateRequired(guestPackage, packageStatus);
  const battlegroupStarted = battlegroup ? isBattlegroupStarted(battlegroup) : false;
  const battlegroupStartRequested = battlegroup ? !battlegroup.stop : false;
  const battlegroupStopped = battlegroup ? battlegroup.stop : false;
  const runtimeComponents = Array.isArray(runtime?.components) ? runtime.components : [];
  const serverKey = localServerKey(candidate);
  const statusBadgeColor = runtimeError
    ? "red"
    : battlegroup
      ? battlegroupStarted
        ? "green"
        : battlegroupStartRequested
          ? "amber"
          : battlegroupStopped
            ? "gray"
            : "green"
      : vm.state === "running"
        ? "green"
        : vm.state === "off"
          ? "gray"
          : "amber";
  const statusBadgeLabel = runtimeError
    ? "Check failed"
    : battlegroup
      ? battlegroupStarted
        ? "Started"
        : battlegroupStartRequested
          ? "Starting"
          : "Stopped"
      : vm.state;

  return (
    <Box className="server-card">
      <Flex align="start" justify="between" gap="3">
        <Box>
          <Flex align="center" gap="2">
            <Heading size={compact ? "3" : "4"}>{vm.name}</Heading>
            <Badge color="bronze" variant="soft">
              Hyper-V
            </Badge>
            <Badge color={candidate.confidence === "high" ? "green" : candidate.confidence === "medium" ? "amber" : "gray"} variant="soft">
              {candidate.confidence}
            </Badge>
          </Flex>
          <Text as="div" size="2" color="gray">
            {primaryIp} · {runtime?.battlegroupName || "setup pending"}
          </Text>
        </Box>
        <Flex align="center" gap="2">
          <Button
            type="button"
            size="1"
            variant="surface"
            disabled={busy}
            onClick={(event) => {
              event.stopPropagation();
              onRefresh?.();
            }}
          >
            Refresh
          </Button>
          <Badge color={statusBadgeColor} variant="surface">
            {busy ? (
              <Flex align="center" gap="1">
                <BusySpinner /> {busyLabel}
              </Flex>
            ) : (
              statusBadgeLabel
            )}
          </Badge>
          <Button
            type="button"
            size="1"
            color="red"
            variant="soft"
            disabled={busy}
            onClick={(event) => {
              event.stopPropagation();
              onRemove?.();
            }}
          >
            Forget
          </Button>
        </Flex>
      </Flex>

      <Grid columns={compact ? "2" : "5"} gap="3" mt="3">
        <Metric label="Namespace" value={runtime?.namespace || "pending"} />
        <Metric label="BattleGroup" value={runtime?.battlegroupName || "pending"} />
        <Metric label="Type" value="Local Hyper-V VM" />
        <Metric label="Guest IP" value={primaryIp} />
        <Metric label="VM State" value={vm.rawState || vm.state} />
      </Grid>
      <Grid columns={compact ? "2" : "5"} gap="3" mt="3">
        <Metric label="Memory" value={formatGiB(vm.memoryAssignedBytes)} />
        <Metric label="CPU" value={vm.processorCount ? `${vm.processorCount} cores` : "unknown"} />
        <Metric label="Disk" value={`${diskLabel}; ${usedDiskLabel}`} />
        <Metric label="Switch" value={vm.switchNames.join(", ") || "none"} />
        <Metric label="Uptime" value={formatDuration(vm.uptimeSeconds)} />
      </Grid>
      <ServerPackageCardStatus guestPackage={guestPackage} packageStatus={packageStatus} />
      {runtimeError ? (
        <Box className="server-error" mt="3">
          <Text size="2">{runtimeError}</Text>
        </Box>
      ) : null}
      {packageStatus?.updateAvailable ? (
        <Box className="setup-guide" mt="3">
          <Text size="2">
            Server package build {packageStatus.latestBuildId || "latest"} is available. Stop the BattleGroup fully before updating this VM.
          </Text>
        </Box>
      ) : null}
      {runtime && battlegroup ? (
        <Box className="server-state" mt="3">
          <Grid columns="2" gap="3">
            <Metric
              label="BattleGroup State"
              value={`${battlegroup.phase || "unknown"}; stop=${battlegroup.stop ? "true" : "false"}`}
            />
            <Metric label="Director" value={battlegroup.directorPhase || "unknown"} />
            <Metric label="Server Group" value={battlegroup.serverGroupPhase || "unknown"} />
          </Grid>
          <ComponentHealthList
            serverKey={serverKey}
            components={runtimeComponents}
            logs={componentLogs}
            logBusy={componentLogBusy}
            restartBusy={componentRestartBusy}
            onRefreshLog={onRefreshComponentLog}
            onRestart={onRestartComponent}
          />
        </Box>
      ) : null}
      <ServerTunnelControls
        serverKey={serverKey}
        namespace={runtime?.namespace ?? ""}
        host={primaryLocalServerIp(candidate)}
        serverKind="hyperv"
        vmName={vm.name}
        canStartDirectorTunnel={!!battlegroup && !battlegroup.stop && isDirectorReadyPhase(battlegroup.directorPhase)}
        canStartFileBrowserTunnel={!!battlegroup && !battlegroup.stop}
        canStartDatabaseTunnel={!!battlegroup && !battlegroup.stop}
        canStartPgHeroTunnel={!!battlegroup && !battlegroup.stop}
        tunnels={tunnels}
        tunnelBusy={tunnelBusy}
        onStartTunnel={onStartTunnel}
        onStopTunnel={onStopTunnel}
        onOpenTunnel={onOpenTunnel}
      />
      <Flex align="center" justify="between" gap="2" mt="3" wrap="wrap">
        <Flex gap="2" wrap="wrap">
          <Button
            size="1"
            variant="surface"
            disabled={busy || !runtime || !battlegroupStopped}
            onClick={onStartBattlegroup}
          >
            Start BattleGroup
          </Button>
          <Button
            size="1"
            variant="surface"
            disabled={busy || !runtime || !battlegroupStartRequested}
            onClick={onStopBattlegroup}
          >
            Stop BattleGroup
          </Button>
          {serverUpdateRequired ? (
            <Button
              size="2"
              color="amber"
              variant="solid"
              disabled={busy || !runtime || !packageStatus?.complete}
              onClick={onUpdateBattlegroup}
            >
              Update Server
            </Button>
          ) : null}
          <Button size="1" variant="surface" disabled={busy || !canStart} onClick={onStart}>
            Start VM
          </Button>
          <Button size="1" variant="surface" disabled={busy || !canStop} onClick={onStop}>
            Stop VM
          </Button>
        </Flex>
        {busyLabel ? (
          <Text size="1" color="gray" className="mono">
            {busyLabel}
          </Text>
        ) : null}
      </Flex>
    </Box>
  );
}

export function RemoteServerCard({
  server,
  compact = false,
  onRemove,
  status,
  proxmoxVmStatus,
  components,
  packageStatus,
  componentLogs,
  componentLogBusy,
  componentRestartBusy,
  statusError,
  busyLabel,
  tunnels,
  tunnelBusy,
  onRefresh,
  onStartBattlegroup,
  onStopBattlegroup,
  onUpdateBattlegroup,
  onRefreshProxmoxVm,
  onStartProxmoxVm,
  onStopProxmoxVm,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshComponentLog,
  onRestartComponent,
}: {
  server: RemoteServerRecord;
  compact?: boolean;
  onRemove?: () => void;
  status?: RemoteServerStatus;
  proxmoxVmStatus?: ProxmoxVmStatus;
  components: RemoteServerComponent[];
  packageStatus: ServerPackageStatus | null;
  componentLogs: Record<string, string>;
  componentLogBusy: Record<string, boolean>;
  componentRestartBusy: Record<string, boolean>;
  statusError?: string;
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onRefresh?: () => void;
  onStartBattlegroup?: () => void;
  onStopBattlegroup?: () => void;
  onUpdateBattlegroup?: () => void;
  onRefreshProxmoxVm?: () => void;
  onStartProxmoxVm?: () => void;
  onStopProxmoxVm?: () => void;
  onStartTunnel?: (request: ServerTunnelStartRequest) => void;
  onStopTunnel?: (tunnelId: string) => void;
  onOpenTunnel?: (tunnel: ServerTunnelStatus) => void;
  onRefreshComponentLog?: (component: RemoteServerComponent) => void;
  onRestartComponent?: (component: RemoteServerComponent) => void;
}) {
  const liveStatus = statusError ? undefined : status;
  const guestPackage = liveStatus?.package;
  const serverUpdateRequired = serverPackageUpdateRequired(guestPackage, packageStatus);
  const liveComponents = liveStatus ? components : [];
  const battlegroupStarted = liveStatus ? isBattlegroupStarted(liveStatus.battlegroup) : false;
  const battlegroupStartRequested = liveStatus ? !liveStatus.battlegroup.stop : false;
  const battlegroupStopped = liveStatus ? liveStatus.battlegroup.stop : false;
  const busy = !!busyLabel;
  return (
    <Box className="server-card">
      <Flex align="start" justify="between" gap="3">
        <Box>
          <Flex align="center" gap="2">
            <Heading size={compact ? "3" : "4"}>{server.name}</Heading>
            <Badge color="bronze" variant="soft">
              {remoteServerKindLabel(server.type)}
            </Badge>
          </Flex>
          <Text as="div" size="2" color="gray">
            {server.host} · {server.battlegroupName || "setup pending"}
          </Text>
        </Box>
        <Flex align="center" gap="2">
          <Button
            type="button"
            size="1"
            variant="surface"
            disabled={busy}
            onClick={(event) => {
              event.stopPropagation();
              onRefresh?.();
            }}
          >
            Refresh
          </Button>
          <Badge
            color={
              statusError
                ? "red"
                : battlegroupStarted
                  ? "green"
                  : battlegroupStartRequested
                    ? "amber"
                  : battlegroupStopped
                    ? "gray"
                    : server.phase === "Setup running"
                      ? "amber"
                      : "green"
            }
            variant="surface"
          >
            {statusError
              ? "Check failed"
              : busyLabel
                ? "Retrieving"
                : liveStatus
                ? battlegroupStarted
                  ? "Started"
                  : battlegroupStartRequested
                    ? "Starting"
                    : "Stopped"
                : server.phase}
          </Badge>
          {onRemove ? (
            <Button
              type="button"
              size="1"
              color="red"
              variant="soft"
              onClick={(event) => {
                event.stopPropagation();
                onRemove();
              }}
            >
              Forget
            </Button>
          ) : null}
        </Flex>
      </Flex>

      <Grid columns={compact ? "2" : "5"} gap="3" mt="3">
        <Metric label="Namespace" value={server.namespace || "pending"} />
        <Metric label="BattleGroup" value={server.battlegroupName || "pending"} />
        <Metric label="Type" value={server.type === "alpine" ? "Alpine VM over SSH" : "Ubuntu over SSH"} />
        <Metric label="Created" value={new Date(server.createdAt).toLocaleString()} />
        {server.provisioner ? <Metric label="Proxmox VM" value={`${server.provisioner.node}/${server.provisioner.vmid}`} /> : null}
      </Grid>
      {server.provisioner ? (
        <Flex align="center" gap="2" mt="3" wrap="wrap">
          <Badge color={proxmoxVmStatus?.status === "running" ? "green" : "gray"} variant="surface">
            VM {proxmoxVmStatus?.status || "unknown"}
          </Badge>
          <Button size="1" variant="surface" disabled={busy} onClick={onRefreshProxmoxVm}>
            VM Status
          </Button>
          <Button size="1" variant="surface" disabled={busy || proxmoxVmStatus?.status === "running"} onClick={onStartProxmoxVm}>
            Start VM
          </Button>
          <Button size="1" variant="surface" disabled={busy || proxmoxVmStatus?.status === "stopped"} onClick={onStopProxmoxVm}>
            Stop VM
          </Button>
        </Flex>
      ) : null}
      <ServerPackageCardStatus guestPackage={guestPackage} packageStatus={packageStatus} />
      {busyLabel ? (
        <Flex align="center" gap="2" mt="3">
          <BusySpinner />
          <Text size="2" color="gray">
            {busyLabel}
          </Text>
        </Flex>
      ) : null}
      {statusError ? (
        <Box className="server-error" mt="3">
          <Text size="2">{statusError}</Text>
        </Box>
      ) : null}
      {packageStatus?.updateAvailable ? (
        <Box className="setup-guide" mt="3">
          <Text size="2">
            Server package build {packageStatus.latestBuildId || "latest"} is available. Stop the BattleGroup fully before updating this server.
          </Text>
        </Box>
      ) : null}
      <Box className="server-state" mt="3">
        <Grid columns="2" gap="3">
          <Metric
            label="BattleGroup State"
            value={
              liveStatus
                ? `${liveStatus.battlegroup.phase || "unknown"}; stop=${liveStatus.battlegroup.stop ? "true" : "false"}`
                : statusError || "Checking"
            }
          />
          <Metric
            label="Director"
            value={liveStatus ? liveStatus.battlegroup.directorPhase || "unknown" : statusError || "Checking"}
          />
          <Metric
            label="Server Group"
            value={liveStatus ? liveStatus.battlegroup.serverGroupPhase || "unknown" : statusError || "Checking"}
          />
        </Grid>
        <ComponentHealthList
          serverKey={server.id}
          components={liveComponents}
          logs={componentLogs}
          logBusy={componentLogBusy}
          restartBusy={componentRestartBusy}
          onRefreshLog={onRefreshComponentLog}
          onRestart={onRestartComponent}
        />
        <ServerTunnelControls
          serverKey={server.id}
          namespace={server.namespace}
          host={server.host}
          serverKind={server.type}
          user={server.user || remoteServerDefaultUser(server.type)}
          keyPath={server.type === "ubuntu" ? server.keyPath : undefined}
          canStartDirectorTunnel={!!liveStatus && !liveStatus.battlegroup.stop && isDirectorReadyPhase(liveStatus.battlegroup.directorPhase)}
          canStartFileBrowserTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          canStartDatabaseTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          canStartPgHeroTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          tunnels={tunnels}
          tunnelBusy={tunnelBusy}
          onStartTunnel={onStartTunnel}
          onStopTunnel={onStopTunnel}
          onOpenTunnel={onOpenTunnel}
        />
        <Flex align="center" justify="between" gap="2" mt="3" wrap="wrap">
          <Flex gap="2" wrap="wrap">
            <Button size="1" variant="surface" disabled={busy} onClick={onRefresh}>
              Refresh
            </Button>
            <Button
              size="1"
              variant="surface"
              disabled={busy || !liveStatus || !battlegroupStopped}
              onClick={onStartBattlegroup}
            >
              Start BattleGroup
            </Button>
            <Button
              size="1"
              variant="surface"
              disabled={busy || !liveStatus || !battlegroupStartRequested}
              onClick={onStopBattlegroup}
            >
              Stop BattleGroup
            </Button>
            {serverUpdateRequired ? (
              <Button
                size="2"
                color="amber"
                variant="solid"
                disabled={busy || !liveStatus}
                onClick={onUpdateBattlegroup}
              >
                Update Server
              </Button>
            ) : null}
          </Flex>
          {busyLabel ? (
            <Text size="1" color="gray" className="mono">
              {busyLabel}
            </Text>
          ) : null}
        </Flex>
      </Box>
    </Box>
  );
}

export function ServersPage({
  duneVms,
  remoteServers,
  remoteStatuses,
  remoteComponents,
  localRuntimes,
  localRuntimeErrors,
  remoteComponentLogs,
  remoteComponentLogBusy,
  remoteComponentRestartBusy,
  remoteStatusErrors,
  remoteBusy,
  serverPackageStatus,
  proxmoxVmStatuses,
  tunnels,
  tunnelBusy,
  onAddLocalServer,
  onAddRemoteServer,
  onRemoveLocalServer,
  onRefreshLocalServer,
  onStartLocalServer,
  onStopLocalServer,
  onRemoveRemoteServer,
  onRefreshRemoteStatus,
  onStartRemoteBattlegroup,
  onStopRemoteBattlegroup,
  onUpdateRemoteBattlegroup,
  onRefreshProxmoxVm,
  onStartProxmoxVm,
  onStopProxmoxVm,
  onStartLocalBattlegroup,
  onStopLocalBattlegroup,
  onUpdateLocalBattlegroup,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshRemoteComponentLog,
  onRestartRemoteComponent,
  onRefreshLocalComponentLog,
  onRestartLocalComponent,
}: {
  duneVms: DuneVmCandidate[];
  remoteServers: RemoteServerRecord[];
  remoteStatuses: Record<string, RemoteServerStatus>;
  remoteComponents: Record<string, RemoteServerComponent[]>;
  localRuntimes: Record<string, LocalHyperVRuntime>;
  localRuntimeErrors: Record<string, string>;
  remoteComponentLogs: Record<string, string>;
  remoteComponentLogBusy: Record<string, boolean>;
  remoteComponentRestartBusy: Record<string, boolean>;
  remoteStatusErrors: Record<string, string>;
  remoteBusy: Record<string, string>;
  serverPackageStatus: ServerPackageStatus | null;
  proxmoxVmStatuses: Record<string, ProxmoxVmStatus>;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onAddLocalServer: () => void;
  onAddRemoteServer: () => void;
  onRemoveLocalServer: (server: DuneVmCandidate) => void;
  onRefreshLocalServer: (server: DuneVmCandidate) => void;
  onStartLocalServer: (server: DuneVmCandidate) => void;
  onStopLocalServer: (server: DuneVmCandidate) => void;
  onRemoveRemoteServer: (server: RemoteServerRecord) => void;
  onRefreshRemoteStatus: (server: RemoteServerRecord) => void;
  onStartRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onStopRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onUpdateRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onRefreshProxmoxVm: (server: RemoteServerRecord) => void;
  onStartProxmoxVm: (server: RemoteServerRecord) => void;
  onStopProxmoxVm: (server: RemoteServerRecord) => void;
  onStartLocalBattlegroup: (server: DuneVmCandidate) => void;
  onStopLocalBattlegroup: (server: DuneVmCandidate) => void;
  onUpdateLocalBattlegroup: (server: DuneVmCandidate) => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshRemoteComponentLog: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
  onRestartRemoteComponent: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
  onRefreshLocalComponentLog: (server: DuneVmCandidate, component: RemoteServerComponent) => void;
  onRestartLocalComponent: (server: DuneVmCandidate, component: RemoteServerComponent) => void;
}) {
  return (
    <Card size="3" variant="surface" className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="5">Servers</Heading>
            <Text as="p" size="2" color="gray" mb="0">
              Setup and basic management run through the desktop app and CLI tooling.
            </Text>
          </Box>
          <Flex gap="2" wrap="wrap" justify="end">
            <Button type="button" variant="surface" onClick={onAddLocalServer}>
              Add local Hyper-V server
            </Button>
            <Button type="button" variant="surface" onClick={onAddRemoteServer}>
              Add remote server
            </Button>
          </Flex>
        </Flex>
        <Box className="setup-scroll" style={{ flexGrow: 1, overflowY: "auto" }}>
          <Flex direction="column" gap="3">
            {duneVms.length + remoteServers.length > 0 ? (
              <>
                {duneVms.map((candidate) => (
                  <ServerCard
                    key={candidate.vm.name}
                    candidate={candidate}
                    compact
                    runtime={localRuntimes[localServerKey(candidate)]}
                    runtimeError={localRuntimeErrors[localServerKey(candidate)]}
                    packageStatus={serverPackageStatus}
                    componentLogs={remoteComponentLogs}
                    componentLogBusy={remoteComponentLogBusy}
                    componentRestartBusy={remoteComponentRestartBusy}
                    busyLabel={remoteBusy[localServerKey(candidate)]}
                    tunnels={tunnels}
                    tunnelBusy={tunnelBusy}
                    onRemove={() => onRemoveLocalServer(candidate)}
                    onRefresh={() => onRefreshLocalServer(candidate)}
                    onStart={() => onStartLocalServer(candidate)}
                    onStop={() => onStopLocalServer(candidate)}
                    onStartBattlegroup={() => onStartLocalBattlegroup(candidate)}
                    onStopBattlegroup={() => onStopLocalBattlegroup(candidate)}
                    onUpdateBattlegroup={() => onUpdateLocalBattlegroup(candidate)}
                    onStartTunnel={onStartTunnel}
                    onStopTunnel={onStopTunnel}
                    onOpenTunnel={onOpenTunnel}
                    onRefreshComponentLog={(component) => onRefreshLocalComponentLog(candidate, component)}
                    onRestartComponent={(component) => onRestartLocalComponent(candidate, component)}
                  />
                ))}
                {remoteServers.map((server) => (
                  <RemoteServerCard
                    key={server.id}
                    server={server}
                    compact
                    status={remoteStatuses[server.id]}
                    proxmoxVmStatus={proxmoxVmStatuses[server.id]}
                    components={remoteComponents[server.id] ?? []}
                    componentLogs={remoteComponentLogs}
                    componentLogBusy={remoteComponentLogBusy}
                    componentRestartBusy={remoteComponentRestartBusy}
                    statusError={remoteStatusErrors[server.id]}
                    packageStatus={serverPackageStatus}
                    busyLabel={remoteBusy[server.id]}
                    tunnels={tunnels}
                    tunnelBusy={tunnelBusy}
                    onRemove={() => onRemoveRemoteServer(server)}
                    onRefresh={() => onRefreshRemoteStatus(server)}
                    onStartBattlegroup={() => onStartRemoteBattlegroup(server)}
                    onStopBattlegroup={() => onStopRemoteBattlegroup(server)}
                    onUpdateBattlegroup={() => onUpdateRemoteBattlegroup(server)}
                    onRefreshProxmoxVm={() => onRefreshProxmoxVm(server)}
                    onStartProxmoxVm={() => onStartProxmoxVm(server)}
                    onStopProxmoxVm={() => onStopProxmoxVm(server)}
                    onStartTunnel={onStartTunnel}
                    onStopTunnel={onStopTunnel}
                    onOpenTunnel={onOpenTunnel}
                    onRefreshComponentLog={(component) => onRefreshRemoteComponentLog(server, component)}
                    onRestartComponent={(component) => onRestartRemoteComponent(server, component)}
                  />
                ))}
              </>
            ) : (
              <EmptyState
                title="No Dune servers detected"
                body="Create a new server or add a remote server profile."
              />
            )}
          </Flex>
        </Box>
      </Flex>
    </Card>
  );
}
