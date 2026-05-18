import { Box, Flex, Heading, Text, Badge, Button, Grid } from "@radix-ui/themes";
import {
  type DuneVmCandidate,
  type LocalHyperVRuntime,
  type ServerPackageStatus,
  type ServerTunnelStatus,
  type ServerTunnelStartRequest,
  type RemoteServerComponent,
  type RemoteServerPackageStatus
} from "../../types";
import {
  localServerKey,
  primaryLocalServerIp,
  isBattlegroupStarted,
  isDirectorReadyPhase
} from "../../utils/storage";
import { formatGiB, formatDuration } from "../../utils/helpers";
import { Metric } from "../Common";
import { BusySpinner, ServerTunnelControls } from "./TunnelControls";
import { ComponentHealthList } from "./ComponentHealth";

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
