import {
  Component,
  type ErrorInfo,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import {
  AlertDialog,
  Badge,
  Box,
  Button,
  Card,
  Dialog,
  Flex,
  Grid,
  Heading,
  Link,
  Select,
  TabNav,
  Text,
  TextArea,
  TextField,
  Theme,
} from "@radix-ui/themes";
import { ChevronDownIcon, ChevronUpIcon, CubeIcon } from "@radix-ui/react-icons";

const pages = [{ id: "servers", label: "Servers" }] as const;
type PageId = (typeof pages)[number]["id"];

type RemoteServerKind = "ubuntu";
type LogLevel = "debug" | "info" | "warn" | "error";
type LogLevelFilter = LogLevel;
type UpdateStatus = "idle" | "checking" | "available" | "current" | "installing" | "relaunching" | "failed";
type DetectionState = "idle" | "detecting" | "ready" | "failed";
type TunnelService = "director" | "fileBrowser" | "database" | "pgHero";
type BadgeTone = "green" | "amber" | "red" | "gray" | "bronze";

const startupUpdateChecksEnabled = import.meta.env.VITE_ENABLE_STARTUP_UPDATE_CHECK === "true";
const remoteServersStorageKey = "dune-manager.remote-servers";
const maxStoredLogRows = 2500;
const maxRenderedLogRows = 1200;

type LogRow = {
  id: number;
  timestamp: string;
  level: LogLevel;
  scope: string;
  message: string;
};

type OperationLogPayload = {
  level: LogLevel;
  scope: string;
  message: string;
};

type RemoteServerRecord = {
  type: RemoteServerKind;
  id: string;
  name: string;
  host: string;
  user: string;
  keyPath: string;
  namespace: string;
  battlegroupName: string;
  worldUniqueName: string;
  phase: string;
};

type RemoteBattlegroupStatus = {
  stop: boolean;
  phase: string;
  serverGroupPhase: string;
  directorPhase: string;
};

type RemoteServerStatus = {
  battlegroup: RemoteBattlegroupStatus;
  package: RemoteServerPackageStatus;
};

type RemoteServerPackageStatus = {
  installedBuildId?: string | null;
  battlegroupVersion?: string | null;
  liveBattlegroupVersion?: string | null;
  operatorVersion?: string | null;
};

type RemoteServerComponent = {
  name: string;
  logKey: string;
  category: "system" | "map";
  state: string;
  tone: BadgeTone;
  summary: string;
  details: string[];
};

type RemoteComponentLogResult = {
  component: string;
  output: string;
};

type RemoteComponentRestartResult = {
  component: string;
  output: string;
};

type ServerTunnelStatus = {
  tunnelId: string;
  service: TunnelService;
  localPort: number;
  remotePort: number;
  url: string;
};

type ServerTunnelStartRequest = {
  tunnelId: string;
  serverKind: RemoteServerKind;
  service: TunnelService;
  host: string;
  user: string;
  keyPath?: string;
  namespace: string;
};

type RemoteAttachForm = {
  host: string;
  keyPath: string;
};

type AppErrorBoundaryProps = {
  onError: (message: string) => void;
  children: ReactNode;
};

type AppErrorBoundaryState = {
  error: string | null;
};

const log = {
  debug: (scope: string, message: string): LogRow => logEntry("debug", scope, message),
  info: (scope: string, message: string): LogRow => logEntry("info", scope, message),
  warn: (scope: string, message: string): LogRow => logEntry("warn", scope, message),
  error: (scope: string, message: string): LogRow => logEntry("error", scope, message),
};

let nextLogRowId = 1;

export function App() {
  const [activePage, setActivePage] = useState<PageId>("servers");
  const [logRows, setLogRows] = useState<LogRow[]>([]);
  const [logLevelFilter, setLogLevelFilter] = useState<LogLevelFilter>("info");
  const [logPanelCollapsed, setLogPanelCollapsed] = useState(false);
  const [remoteAttachOpen, setRemoteAttachOpen] = useState(false);
  const [remoteAttachRunning, setRemoteAttachRunning] = useState(false);
  const [remoteAttachForm, setRemoteAttachForm] = useState<RemoteAttachForm>({ host: "", keyPath: "" });
  const [remoteServerToRemove, setRemoteServerToRemove] = useState<RemoteServerRecord | null>(null);
  const [remoteServers, setRemoteServers] = useState<RemoteServerRecord[]>([]);
  const [remoteServerStatuses, setRemoteServerStatuses] = useState<Record<string, RemoteServerStatus>>({});
  const [remoteServerComponents, setRemoteServerComponents] = useState<Record<string, RemoteServerComponent[]>>({});
  const [remoteComponentLogs, setRemoteComponentLogs] = useState<Record<string, string>>({});
  const [remoteComponentLogBusy, setRemoteComponentLogBusy] = useState<Record<string, boolean>>({});
  const [remoteComponentRestartBusy, setRemoteComponentRestartBusy] = useState<Record<string, boolean>>({});
  const [remoteServerStatusErrors, setRemoteServerStatusErrors] = useState<Record<string, string>>({});
  const [remoteServerBusy, setRemoteServerBusy] = useState<Record<string, string>>({});
  const [serverTunnels, setServerTunnels] = useState<Record<string, ServerTunnelStatus>>({});
  const [serverTunnelBusy, setServerTunnelBusy] = useState<Record<string, boolean>>({});
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<string | null>(null);
  const updateCheckInFlight = useRef(false);

  const appendLogRow = (row: LogRow) => {
    setLogRows((rows) => limitLogRows([...rows, row]));
  };

  const clearLogRows = () => {
    setLogRows([]);
  };

  const checkForAppUpdate = async () => {
    if (updateCheckInFlight.current) return;
    updateCheckInFlight.current = true;
    setUpdateStatus("checking");
    setUpdateProgress(null);
    appendLogRow(log.info("updates", "Checking for app updates."));
    try {
      const nextUpdate = await check({ timeout: 15_000 });
      setAvailableUpdate(nextUpdate);
      if (nextUpdate) {
        setUpdateStatus("available");
        appendLogRow(
          log.info("updates", `Update ${nextUpdate.version} is available; current version is ${nextUpdate.currentVersion}.`),
        );
        setUpdateDialogOpen(true);
      } else {
        setUpdateStatus("current");
        appendLogRow(log.info("updates", "The app is up to date."));
      }
    } catch (err) {
      setUpdateStatus("failed");
      appendLogRow(log.warn("updates", `Update check failed: ${errorMessage(err)}`));
    } finally {
      updateCheckInFlight.current = false;
    }
  };

  const installAppUpdate = async () => {
    if (!availableUpdate) return;
    let downloaded = 0;
    let total: number | null = null;
    setUpdateStatus("installing");
    setUpdateProgress("Preparing download...");
    appendLogRow(log.info("updates", `Installing update ${availableUpdate.version}.`));
    try {
      await availableUpdate.downloadAndInstall(
        (event: DownloadEvent) => {
          if (event.event === "Started") {
            total = event.data.contentLength ?? null;
            downloaded = 0;
            setUpdateProgress(total ? `Downloading 0 of ${formatBytes(total)}` : "Downloading update...");
          }
          if (event.event === "Progress") {
            downloaded += event.data.chunkLength;
            setUpdateProgress(
              total ? `Downloading ${formatBytes(downloaded)} of ${formatBytes(total)}` : `Downloading ${formatBytes(downloaded)}`,
            );
          }
          if (event.event === "Finished") {
            setUpdateProgress("Installing update...");
          }
        },
        { timeout: 120_000 },
      );
      setUpdateStatus("relaunching");
      setUpdateProgress("Relaunching...");
      appendLogRow(log.info("updates", "Update installed; relaunching the app."));
      await relaunch();
    } catch (err) {
      setUpdateStatus("failed");
      setUpdateProgress(null);
      appendLogRow(log.error("updates", errorMessage(err)));
    }
  };

  const addRemoteServer = async () => {
    const host = remoteAttachForm.host.trim();
    const keyPath = remoteAttachForm.keyPath.trim();
    if (!host || !keyPath) return;
    setRemoteAttachRunning(true);
    appendLogRow(log.info("remote.attach", "Detecting remote Dune battlegroups."));
    try {
      const detected = await invoke<RemoteServerRecord[]>("detect_remote_ubuntu_servers", {
        request: { host, keyPath, serverType: "ubuntu", user: "root" },
      });
      if (detected.length === 0) {
        throw new Error("No Dune battlegroups were detected on the remote server.");
      }
      const nextServers = mergeRemoteServers(remoteServers, detected);
      setRemoteServers(persistRemoteServers(nextServers));
      setRemoteAttachOpen(false);
      setRemoteAttachForm({ host: "", keyPath: "" });
      appendLogRow(log.info("remote.attach", `Added ${detected.length} remote battlegroup profile(s).`));
      for (const server of detected) {
        void refreshRemoteServerStatus(server);
      }
    } catch (err) {
      appendLogRow(log.error("remote.attach", errorMessage(err)));
    } finally {
      setRemoteAttachRunning(false);
    }
  };

  const removeRemoteServer = (server: RemoteServerRecord) => {
    stopTunnelsForServer(server.id);
    setRemoteServers((servers) => persistRemoteServers(servers.filter((candidate) => candidate.id !== server.id)));
    setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
    setRemoteServerComponents((components) => omitKey(components, server.id));
    setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${server.id}:`));
    appendLogRow(log.info("remote.attach", `Forgot remote server ${server.name}.`));
  };

  const stopTunnelsForServer = (serverKey: string) => {
    for (const tunnelId of Object.keys(serverTunnels).filter((id) => id.startsWith(`${serverKey}:tunnel:`))) {
      void stopServerTunnel(tunnelId);
    }
  };

  const detectRemoteServerDetails = async (server: RemoteServerRecord): Promise<RemoteServerRecord> => {
    const detected = await invoke<RemoteServerRecord[]>("detect_remote_ubuntu_servers", {
      request: {
        host: server.host,
        keyPath: server.keyPath,
        serverType: "ubuntu",
        user: server.user || remoteServerDefaultUser(server.type),
      },
    });
    if (detected.length === 0) {
      throw new Error("No Dune battlegroups were detected on the remote server.");
    }
    return detected.find((candidate) => candidate.battlegroupName === server.battlegroupName) ?? detected[0];
  };

  const refreshRemoteServerStatus = async (server: RemoteServerRecord) => {
    if (!server.host || !server.keyPath) return;
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: "Retrieving server information" }));
    setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
    setRemoteServerComponents((components) => omitKey(components, server.id));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
    try {
      const liveServer = await detectRemoteServerDetails(server);
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      const status = await invoke<RemoteServerStatus>("remote_server_status", {
        request: remoteServerActionRequest(liveServer),
      });
      const components = await invoke<RemoteServerComponent[]>("remote_server_components", {
        request: remoteServerActionRequest(liveServer),
      });
      setRemoteServerStatuses((statuses) => ({ ...statuses, [liveServer.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, liveServer.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === liveServer.id ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase } : candidate,
          ),
        ),
      );
      appendLogRow(
        log.info(
          "remote.status",
          `${liveServer.battlegroupName}: ${status.battlegroup.phase || "unknown"}, server group ${
            status.battlegroup.serverGroupPhase || "unknown"
          }, Director ${status.battlegroup.directorPhase || "unknown"}.`,
        ),
      );
    } catch (err) {
      const message = errorMessage(err);
      setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
      setRemoteServerComponents((components) => omitKey(components, server.id));
      setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      appendLogRow(log.warn("remote.status", message));
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, server.id));
    }
  };

  const runRemoteBattlegroupAction = async (server: RemoteServerRecord, action: "start" | "stop" | "update") => {
    const busyText =
      action === "start" ? "Starting battlegroup" : action === "stop" ? "Stopping battlegroup" : "Updating battlegroup";
    const verb = action === "start" ? "Starting" : action === "stop" ? "Stopping" : "Updating";
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: busyText }));
    appendLogRow(log.info("bg", `${verb} remote battlegroup.`));
    try {
      const liveServer = server.namespace && server.battlegroupName ? server : await detectRemoteServerDetails(server);
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      const command =
        action === "start"
          ? "start_remote_battlegroup"
          : action === "stop"
            ? "stop_remote_battlegroup"
            : "update_remote_battlegroup";
      const status = await invoke<RemoteServerStatus>(command, { request: remoteServerActionRequest(liveServer) });
      const components = await invoke<RemoteServerComponent[]>("remote_server_components", {
        request: remoteServerActionRequest(liveServer),
      });
      setRemoteServerStatuses((statuses) => ({ ...statuses, [liveServer.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, liveServer.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === liveServer.id ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase } : candidate,
          ),
        ),
      );
    } catch (err) {
      const message = errorMessage(err);
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      appendLogRow(log.error("bg", message));
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, server.id));
    }
  };

  const startServerTunnel = async (request: ServerTunnelStartRequest) => {
    setServerTunnelBusy((busy) => ({ ...busy, [request.tunnelId]: true }));
    appendLogRow(log.info("tunnel", `Starting ${tunnelServiceLabel(request.service)} tunnel.`));
    try {
      const status = await invoke<ServerTunnelStatus>("start_server_tunnel", { request });
      setServerTunnels((tunnels) => ({ ...tunnels, [status.tunnelId]: status }));
      appendLogRow(log.info("tunnel", `${tunnelServiceLabel(request.service)} tunnel is ready at ${status.url}`));
    } catch (err) {
      appendLogRow(log.error("tunnel", errorMessage(err)));
    } finally {
      setServerTunnelBusy((busy) => omitKey(busy, request.tunnelId));
    }
  };

  const openServerTunnel = async (tunnel: ServerTunnelStatus) => {
    try {
      const status = await invoke<ServerTunnelStatus | null>("server_tunnel_status", {
        request: { tunnelId: tunnel.tunnelId },
      });
      if (!status) {
        setServerTunnels((tunnels) => omitKey(tunnels, tunnel.tunnelId));
        appendLogRow(log.warn("tunnel", "The SSH tunnel is no longer running."));
        return;
      }
      setServerTunnels((tunnels) => ({ ...tunnels, [status.tunnelId]: status }));
      if (status.service === "database") {
        await copyTextToClipboard(status.url);
        appendLogRow(log.info("tunnel", `Copied Postgres connection URI ${status.url}`));
        return;
      }
      await openExternal(status.url);
    } catch (err) {
      appendLogRow(log.error("tunnel", errorMessage(err)));
    }
  };

  const stopServerTunnel = async (tunnelId: string) => {
    setServerTunnelBusy((busy) => ({ ...busy, [tunnelId]: true }));
    try {
      await invoke("stop_server_tunnel", { request: { tunnelId } });
      setServerTunnels((tunnels) => omitKey(tunnels, tunnelId));
      appendLogRow(log.info("tunnel", "SSH tunnel stopped."));
    } catch (err) {
      appendLogRow(log.error("tunnel", errorMessage(err)));
    } finally {
      setServerTunnelBusy((busy) => omitKey(busy, tunnelId));
    }
  };

  const refreshRemoteComponentLog = async (server: RemoteServerRecord, component: RemoteServerComponent) => {
    const key = componentLogStateKey(server.id, component);
    setRemoteComponentLogBusy((busy) => ({ ...busy, [key]: true }));
    appendLogRow(log.info("remote.logs", `Refreshing ${component.name} logs.`));
    try {
      const liveServer = server.namespace ? server : await detectRemoteServerDetails(server);
      const result = await invoke<RemoteComponentLogResult>("remote_component_log_tail", {
        request: {
          serverType: liveServer.type,
          host: liveServer.host,
          user: liveServer.user || remoteServerDefaultUser(liveServer.type),
          keyPath: liveServer.keyPath || undefined,
          namespace: liveServer.namespace,
          component: component.logKey,
          tail: 160,
        },
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || "No log output."),
      }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      appendLogRow(log.warn("remote.logs", message));
    } finally {
      setRemoteComponentLogBusy((busy) => omitKey(busy, key));
    }
  };

  const restartRemoteComponent = async (server: RemoteServerRecord, component: RemoteServerComponent) => {
    if (isCriticalRestartComponent(component)) {
      const confirmed = window.confirm(
        `Restart ${component.name}? This can temporarily interrupt persistence, messaging, or active players.`,
      );
      if (!confirmed) return;
    }
    const key = componentLogStateKey(server.id, component);
    setRemoteComponentRestartBusy((busy) => ({ ...busy, [key]: true }));
    appendLogRow(log.warn("remote.restart", `Restarting ${component.name}.`));
    try {
      const liveServer = server.namespace ? server : await detectRemoteServerDetails(server);
      const result = await invoke<RemoteComponentRestartResult>("restart_remote_component", {
        request: {
          serverType: liveServer.type,
          host: liveServer.host,
          user: liveServer.user || remoteServerDefaultUser(liveServer.type),
          keyPath: liveServer.keyPath || undefined,
          namespace: liveServer.namespace,
          component: component.logKey,
        },
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || `${component.name} restart requested.`),
      }));
      const components = await invoke<RemoteServerComponent[]>("remote_server_components", {
        request: remoteServerActionRequest(liveServer),
      });
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      appendLogRow(log.error("remote.restart", message));
    } finally {
      setRemoteComponentRestartBusy((busy) => omitKey(busy, key));
    }
  };

  useEffect(() => {
    setRemoteServers(readRemoteServers());
  }, []);

  useEffect(() => {
    for (const server of remoteServers) {
      if (!server.host || !server.keyPath || remoteServerBusy[server.id]) continue;
      void refreshRemoteServerStatus(server);
    }
  }, [remoteServers.map((server) => server.id).join("|")]);

  useEffect(() => {
    if (!startupUpdateChecksEnabled) {
      appendLogRow(log.debug("updates", "Automatic update checks are disabled for this local build."));
      return;
    }
    void checkForAppUpdate();
  }, []);

  useEffect(() => {
    const unlisten = listen<OperationLogPayload>("operation-log", (event) => {
      const payload = event.payload;
      appendLogRow(logEntry(payload.level, payload.scope, payload.message));
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    return () => {
      void invoke("stop_all_tunnels");
    };
  }, []);

  const renderedLogRows = filterLogRows(logRows, logLevelFilter).slice(-maxRenderedLogRows);

  return (
    <Theme accentColor="bronze" grayColor="sand" radius="medium" scaling="100%">
      <Flex direction="column" height="100vh" className="app-shell">
        <Header
          activePage={activePage}
          onNavigate={setActivePage}
          serverCount={remoteServers.length}
          updateStatus={updateStatus}
          update={availableUpdate}
          updateProgress={updateProgress}
          onCheckUpdate={checkForAppUpdate}
          onOpenUpdate={() => setUpdateDialogOpen(true)}
        />
        <Flex className="content-shell" gap="3" p="4" pt="0" minHeight="0">
          <Box className="main-pane">
            <AppErrorBoundary onError={(message) => appendLogRow(log.error("ui", message))}>
              <ServersPage
                remoteServers={remoteServers}
                remoteStatuses={remoteServerStatuses}
                remoteComponents={remoteServerComponents}
                remoteComponentLogs={remoteComponentLogs}
                remoteComponentLogBusy={remoteComponentLogBusy}
                remoteComponentRestartBusy={remoteComponentRestartBusy}
                remoteStatusErrors={remoteServerStatusErrors}
                remoteBusy={remoteServerBusy}
                tunnels={serverTunnels}
                tunnelBusy={serverTunnelBusy}
                onAddRemoteServer={() => setRemoteAttachOpen(true)}
                onRemoveRemoteServer={(server) => setRemoteServerToRemove(server)}
                onRefreshRemoteStatus={refreshRemoteServerStatus}
                onStartRemoteBattlegroup={(server) => runRemoteBattlegroupAction(server, "start")}
                onStopRemoteBattlegroup={(server) => runRemoteBattlegroupAction(server, "stop")}
                onUpdateRemoteBattlegroup={(server) => runRemoteBattlegroupAction(server, "update")}
                onStartTunnel={startServerTunnel}
                onStopTunnel={stopServerTunnel}
                onOpenTunnel={openServerTunnel}
                onRefreshRemoteComponentLog={refreshRemoteComponentLog}
                onRestartRemoteComponent={restartRemoteComponent}
              />
            </AppErrorBoundary>
          </Box>
          <LogWindow
            rows={renderedLogRows}
            level={logLevelFilter}
            collapsed={logPanelCollapsed}
            onLevelChange={setLogLevelFilter}
            onClear={clearLogRows}
            onToggleCollapsed={() => setLogPanelCollapsed((collapsed) => !collapsed)}
          />
        </Flex>
        <RemoteAttachDialog
          open={remoteAttachOpen}
          form={remoteAttachForm}
          running={remoteAttachRunning}
          onOpenChange={setRemoteAttachOpen}
          onChange={setRemoteAttachForm}
          onAttach={addRemoteServer}
        />
        <RemoveRemoteServerDialog
          server={remoteServerToRemove}
          onOpenChange={(open) => {
            if (!open) setRemoteServerToRemove(null);
          }}
          onRemove={(server) => {
            removeRemoteServer(server);
            setRemoteServerToRemove(null);
          }}
        />
        <UpdateDialog
          open={updateDialogOpen}
          update={availableUpdate}
          status={updateStatus}
          progress={updateProgress}
          onOpenChange={setUpdateDialogOpen}
          onInstall={installAppUpdate}
        />
      </Flex>
    </Theme>
  );
}

class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error: error.message };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    this.props.onError(`${error.message}\n${info.componentStack}`);
  }

  render() {
    if (this.state.error) {
      return (
        <Card size="3" variant="surface" className="pane page-pane">
          <Flex direction="column" gap="3">
            <Heading size="4">UI Error</Heading>
            <Text size="2" color="gray">
              The view failed to render. Details were written to the log window.
            </Text>
            <Text size="2" className="mono">
              {this.state.error}
            </Text>
          </Flex>
        </Card>
      );
    }

    return this.props.children;
  }
}

function Header({
  activePage,
  onNavigate,
  serverCount,
  updateStatus,
  update,
  updateProgress,
  onCheckUpdate,
  onOpenUpdate,
}: {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
  serverCount: number;
  updateStatus: UpdateStatus;
  update: Update | null;
  updateProgress: string | null;
  onCheckUpdate: () => void;
  onOpenUpdate: () => void;
}) {
  return (
    <Flex asChild align="center" justify="between" p="4">
      <header>
        <Flex align="center" gap="5">
          <Flex align="center" gap="3">
            <CubeIcon width="24" height="24" />
            <Heading size="4">Dune Dedicated Server Manager</Heading>
          </Flex>
          <TopNav activePage={activePage} onNavigate={onNavigate} serverCount={serverCount} />
        </Flex>
        <UpdateHeaderControl
          status={updateStatus}
          update={update}
          progress={updateProgress}
          onCheck={onCheckUpdate}
          onOpenUpdate={onOpenUpdate}
        />
      </header>
    </Flex>
  );
}

function TopNav({
  activePage,
  onNavigate,
  serverCount,
}: {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
  serverCount: number;
}) {
  return (
    <Box asChild>
      <nav aria-label="Primary navigation">
        <TabNav.Root size="2" color="bronze">
          {pages.map((page) => (
            <TabNav.Link
              key={page.id}
              href="#"
              active={page.id === activePage}
              onClick={(event) => {
                event.preventDefault();
                onNavigate(page.id);
              }}
            >
              {page.id === "servers" ? `${page.label} (${serverCount})` : page.label}
            </TabNav.Link>
          ))}
        </TabNav.Root>
      </nav>
    </Box>
  );
}

function UpdateHeaderControl({
  status,
  update,
  progress,
  onCheck,
  onOpenUpdate,
}: {
  status: UpdateStatus;
  update: Update | null;
  progress: string | null;
  onCheck: () => void;
  onOpenUpdate: () => void;
}) {
  const busy = status === "checking" || status === "installing" || status === "relaunching";
  const hasUpdate = Boolean(update);
  return (
    <Flex align="center" gap="2" className="header-update">
      <Badge color={updateTone(status)} variant="soft">
        {updateLabel(status, update, progress)}
      </Badge>
      <Button size="1" variant={hasUpdate ? "solid" : "surface"} disabled={busy} onClick={hasUpdate ? onOpenUpdate : onCheck}>
        {busy ? "Working..." : hasUpdate ? "Install" : "Check for updates"}
      </Button>
    </Flex>
  );
}

function ServersPage({
  remoteServers,
  remoteStatuses,
  remoteComponents,
  remoteComponentLogs,
  remoteComponentLogBusy,
  remoteComponentRestartBusy,
  remoteStatusErrors,
  remoteBusy,
  tunnels,
  tunnelBusy,
  onAddRemoteServer,
  onRemoveRemoteServer,
  onRefreshRemoteStatus,
  onStartRemoteBattlegroup,
  onStopRemoteBattlegroup,
  onUpdateRemoteBattlegroup,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshRemoteComponentLog,
  onRestartRemoteComponent,
}: {
  remoteServers: RemoteServerRecord[];
  remoteStatuses: Record<string, RemoteServerStatus>;
  remoteComponents: Record<string, RemoteServerComponent[]>;
  remoteComponentLogs: Record<string, string>;
  remoteComponentLogBusy: Record<string, boolean>;
  remoteComponentRestartBusy: Record<string, boolean>;
  remoteStatusErrors: Record<string, string>;
  remoteBusy: Record<string, string>;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onAddRemoteServer: () => void;
  onRemoveRemoteServer: (server: RemoteServerRecord) => void;
  onRefreshRemoteStatus: (server: RemoteServerRecord) => void;
  onStartRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onStopRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onUpdateRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshRemoteComponentLog: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
  onRestartRemoteComponent: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
}) {
  return (
    <Card size="3" variant="surface" className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="5">Servers</Heading>
            <Text as="p" size="2" color="gray" mb="0">
              Manage existing remote Dune servers over SSH and Kubernetes.
            </Text>
          </Box>
          <Button type="button" variant="surface" onClick={onAddRemoteServer}>
            Add remote server
          </Button>
        </Flex>
        <Box className="page-scroll">
          <Flex direction="column" gap="3">
            {remoteServers.length > 0 ? (
              remoteServers.map((server) => (
                <RemoteServerCard
                  key={server.id}
                  server={server}
                  status={remoteStatuses[server.id]}
                  components={remoteComponents[server.id] ?? []}
                  componentLogs={remoteComponentLogs}
                  componentLogBusy={remoteComponentLogBusy}
                  componentRestartBusy={remoteComponentRestartBusy}
                  statusError={remoteStatusErrors[server.id]}
                  busyLabel={remoteBusy[server.id]}
                  tunnels={tunnels}
                  tunnelBusy={tunnelBusy}
                  onRemove={() => onRemoveRemoteServer(server)}
                  onRefresh={() => onRefreshRemoteStatus(server)}
                  onStartBattlegroup={() => onStartRemoteBattlegroup(server)}
                  onStopBattlegroup={() => onStopRemoteBattlegroup(server)}
                  onUpdateBattlegroup={() => onUpdateRemoteBattlegroup(server)}
                  onStartTunnel={onStartTunnel}
                  onStopTunnel={onStopTunnel}
                  onOpenTunnel={onOpenTunnel}
                  onRefreshComponentLog={(component) => onRefreshRemoteComponentLog(server, component)}
                  onRestartComponent={(component) => onRestartRemoteComponent(server, component)}
                />
              ))
            ) : (
              <EmptyState title="No remote servers" body="Add a remote Ubuntu host that already has a Dune battlegroup." />
            )}
          </Flex>
        </Box>
      </Flex>
    </Card>
  );
}

function RemoteServerCard({
  server,
  onRemove,
  status,
  components,
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
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshComponentLog,
  onRestartComponent,
}: {
  server: RemoteServerRecord;
  onRemove: () => void;
  status?: RemoteServerStatus;
  components: RemoteServerComponent[];
  componentLogs: Record<string, string>;
  componentLogBusy: Record<string, boolean>;
  componentRestartBusy: Record<string, boolean>;
  statusError?: string;
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onRefresh: () => void;
  onStartBattlegroup: () => void;
  onStopBattlegroup: () => void;
  onUpdateBattlegroup: () => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshComponentLog: (component: RemoteServerComponent) => void;
  onRestartComponent: (component: RemoteServerComponent) => void;
}) {
  const liveStatus = statusError ? undefined : status;
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
            <Heading size="3">{server.name}</Heading>
            <Badge color="bronze" variant="soft">
              Ubuntu SSH
            </Badge>
          </Flex>
          <Text as="div" size="2" color="gray">
            {server.host} · {server.battlegroupName || "unknown battlegroup"}
          </Text>
        </Box>
        <Flex align="center" gap="2">
          <Button type="button" size="1" variant="surface" disabled={busy} onClick={onRefresh}>
            Refresh
          </Button>
          <Badge color={remoteStatusTone(statusError, liveStatus, battlegroupStarted, battlegroupStartRequested, battlegroupStopped, server)} variant="surface">
            {remoteStatusLabel(statusError, liveStatus, busyLabel, battlegroupStarted, battlegroupStartRequested, server)}
          </Badge>
          <Button type="button" size="1" color="red" variant="soft" onClick={onRemove}>
            Forget
          </Button>
        </Flex>
      </Flex>

      <Grid columns="4" gap="3" mt="3">
        <Metric label="Namespace" value={server.namespace || "pending"} />
        <Metric label="BattleGroup" value={server.battlegroupName || "pending"} />
        <Metric label="Type" value="Remote Ubuntu" />
        <Metric label="World" value={server.worldUniqueName || "unknown"} />
      </Grid>
      <ServerPackageCardStatus guestPackage={liveStatus?.package} />
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
      <Box className="server-state" mt="3">
        <Grid columns="3" gap="3">
          <Metric
            label="BattleGroup State"
            value={
              liveStatus
                ? `${liveStatus.battlegroup.phase || "unknown"}; stop=${liveStatus.battlegroup.stop ? "true" : "false"}`
                : statusError || "Checking"
            }
          />
          <Metric label="Director" value={liveStatus ? liveStatus.battlegroup.directorPhase || "unknown" : statusError || "Checking"} />
          <Metric label="Server Group" value={liveStatus ? liveStatus.battlegroup.serverGroupPhase || "unknown" : statusError || "Checking"} />
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
          keyPath={server.keyPath}
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
            <Button size="1" variant="surface" disabled={busy || !liveStatus || !battlegroupStopped} onClick={onStartBattlegroup}>
              Start BattleGroup
            </Button>
            <Button size="1" variant="surface" disabled={busy || !liveStatus || !battlegroupStartRequested} onClick={onStopBattlegroup}>
              Stop BattleGroup
            </Button>
            <Button size="2" color="amber" variant="solid" disabled={busy || !liveStatus} onClick={onUpdateBattlegroup}>
              Update Server
            </Button>
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

function ServerPackageCardStatus({ guestPackage }: { guestPackage?: RemoteServerPackageStatus }) {
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

function ServerTunnelControls({
  serverKey,
  namespace,
  host,
  serverKind,
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
  serverKind: RemoteServerKind;
  user: string;
  keyPath?: string;
  canStartDirectorTunnel: boolean;
  canStartFileBrowserTunnel: boolean;
  canStartDatabaseTunnel: boolean;
  canStartPgHeroTunnel: boolean;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
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
          const disabled = busy || (!active && (!serviceAvailable || !host.trim() || !namespace.trim()));
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
                        ? "Requires detected server namespace and host"
                        : "Tunnel stopped"}
                </Text>
              </Flex>
              <Flex align="center" gap="2" wrap="wrap" justify="end">
                {active ? (
                  <Button type="button" size="1" variant="surface" onClick={() => onOpenTunnel(active)}>
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
                      onStopTunnel(tunnelId);
                      return;
                    }
                    onStartTunnel({ tunnelId, serverKind, service, host, user, keyPath, namespace });
                  }}
                >
                  {busy ? (
                    <Flex align="center" gap="1">
                      <BusySpinner /> Working
                    </Flex>
                  ) : active ? (
                    "Stop Tunnel"
                  ) : (
                    "Start Tunnel"
                  )}
                </Button>
                {active ? (
                  <Link
                    size="1"
                    href="#"
                    className="mono tunnel-url"
                    onClick={(event) => {
                      event.preventDefault();
                      onOpenTunnel(active);
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

function ComponentHealthList({
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
  onRefreshLog: (component: RemoteServerComponent) => void;
  onRestart: (component: RemoteServerComponent) => void;
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

function ComponentHealthGroup({
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
  onRefreshLog: (component: RemoteServerComponent) => void;
  onRestart: (component: RemoteServerComponent) => void;
}) {
  if (components.length === 0) return null;
  return (
    <details className="component-group">
      <summary className="component-group-summary">
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
            <details key={`${component.logKey}-${component.name}`} className="component-row">
              <summary className="component-summary">
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
                        onRefreshLog(component);
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
                        onRestart(component);
                      }}
                    >
                      {restarting ? "Restarting" : "Restart"}
                    </Button>
                  </Flex>
                </Flex>
              </summary>
              <Box className="component-body">
                {component.details.length > 0 ? (
                  <ul className="component-details">
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
                      <Button type="button" size="1" variant="soft" onClick={() => void copyTextToClipboard(logText)}>
                        Copy logs
                      </Button>
                    </Flex>
                    <Box className="component-log" mt="2">
                      {logText.split(/\r?\n/).map((line, index) => (
                        <Text as="div" size="1" className="mono" key={`${component.logKey}-${index}`}>
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

function RemoteAttachDialog({
  open,
  form,
  running,
  onOpenChange,
  onChange,
  onAttach,
}: {
  open: boolean;
  form: RemoteAttachForm;
  running: boolean;
  onOpenChange: (open: boolean) => void;
  onChange: (form: RemoteAttachForm) => void;
  onAttach: () => void;
}) {
  const canAttach = form.host.trim().length > 0 && form.keyPath.trim().length > 0 && !running;
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="520px">
        <Dialog.Title>Add Remote Server</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Connect over SSH and detect existing Dune battlegroups. This does not provision or modify the server.
        </Dialog.Description>
        <Flex direction="column" gap="3" mt="4">
          <Field label="Host or IP">
            <TextField.Root
              placeholder="203.0.113.10"
              disabled={running}
              value={form.host}
              onChange={(event) => onChange({ ...form, host: event.target.value })}
            />
          </Field>
          <Field label="Private Key">
            <Grid columns="1fr auto" gap="2">
              <TextField.Root
                placeholder="Choose SSH private key"
                value={form.keyPath}
                disabled={running}
                onChange={(event) => onChange({ ...form, keyPath: event.target.value })}
              />
              <Button
                type="button"
                variant="surface"
                disabled={running}
                onClick={async () => {
                  const selected = await openFileDialog("Choose SSH private key");
                  if (selected) onChange({ ...form, keyPath: selected });
                }}
              >
                Choose
              </Button>
            </Grid>
          </Field>
        </Flex>
        <Flex gap="3" justify="end" mt="5">
          <Dialog.Close>
            <Button variant="soft" color="gray" disabled={running}>
              Cancel
            </Button>
          </Dialog.Close>
          <Button disabled={!canAttach} onClick={onAttach}>
            {running ? "Detecting..." : "Detect and Add"}
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function RemoveRemoteServerDialog({
  server,
  onOpenChange,
  onRemove,
}: {
  server: RemoteServerRecord | null;
  onOpenChange: (open: boolean) => void;
  onRemove: (server: RemoteServerRecord) => void;
}) {
  return (
    <AlertDialog.Root open={!!server} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Forget Remote Server</AlertDialog.Title>
        <AlertDialog.Description size="2" color="gray">
          This only removes the saved server entry from this app. The remote host and Dune battlegroup will not be changed.
        </AlertDialog.Description>
        {server ? (
          <Box className="info-card" mt="4">
            <Metric label="Host" value={server.host} />
            <Metric label="Battlegroup" value={server.battlegroupName || "unknown"} />
          </Box>
        ) : null}
        <Flex gap="3" justify="end" mt="5">
          <AlertDialog.Cancel>
            <Button variant="soft" color="gray">
              Cancel
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action>
            <Button color="red" onClick={() => server && onRemove(server)}>
              Forget Server
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}

function UpdateDialog({
  open,
  update,
  status,
  progress,
  onOpenChange,
  onInstall,
}: {
  open: boolean;
  update: Update | null;
  status: UpdateStatus;
  progress: string | null;
  onOpenChange: (open: boolean) => void;
  onInstall: () => void;
}) {
  const busy = status === "installing" || status === "relaunching";
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Install app update?</AlertDialog.Title>
        <AlertDialog.Description size="2">
          {update
            ? `Version ${update.version} is available. The app will download the signed installer, install it, and relaunch.`
            : "No update is currently selected."}
        </AlertDialog.Description>
        {update?.body ? <TextArea mt="3" value={update.body} readOnly rows={7} /> : null}
        {progress ? (
          <Text as="p" size="2" color="gray" mt="3" className="mono">
            {progress}
          </Text>
        ) : null}
        <Flex gap="3" mt="4" justify="end">
          <AlertDialog.Cancel disabled={busy}>
            <Button variant="soft" color="gray" disabled={busy}>
              Later
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action disabled={!update || busy}>
            <Button disabled={!update || busy} onClick={onInstall}>
              {busy ? "Installing..." : "Install update"}
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}

function LogWindow({
  rows,
  level,
  collapsed,
  onLevelChange,
  onClear,
  onToggleCollapsed,
}: {
  rows: LogRow[];
  level: LogLevelFilter;
  collapsed: boolean;
  onLevelChange: (level: LogLevelFilter) => void;
  onClear: () => void;
  onToggleCollapsed: () => void;
}) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);
  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    if (stickToBottomRef.current) {
      body.scrollTop = body.scrollHeight;
    }
  }, [rows]);
  return (
    <Card size="3" variant="surface" className={`pane log-pane${collapsed ? " is-collapsed" : ""}`}>
      <Flex direction="column" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3" mb={collapsed ? "0" : "3"}>
          <Box minWidth="0">
            <Text as="div" size="2" weight="medium">
              Logs
            </Text>
            <Text as="div" size="1" color="gray">
              {rows.length} entries
            </Text>
          </Box>
          <Flex align="center" gap="2">
            {collapsed ? null : (
              <>
                <Select.Root value={level} onValueChange={(value) => onLevelChange(value as LogLevelFilter)}>
                  <Select.Trigger aria-label="Minimum log level" />
                  <Select.Content>
                    <Select.Item value="debug">Debug</Select.Item>
                    <Select.Item value="info">Info</Select.Item>
                    <Select.Item value="warn">Warn</Select.Item>
                    <Select.Item value="error">Error</Select.Item>
                  </Select.Content>
                </Select.Root>
                <Button type="button" size="1" variant="surface" disabled={rows.length === 0} onClick={onClear}>
                  Clear
                </Button>
              </>
            )}
            <Button
              type="button"
              size="1"
              variant="surface"
              aria-label={collapsed ? "Expand logs" : "Collapse logs"}
              onClick={onToggleCollapsed}
            >
              {collapsed ? <ChevronUpIcon /> : <ChevronDownIcon />}
            </Button>
          </Flex>
        </Flex>
        {collapsed ? null : (
          <Box
            className="log-body"
            ref={bodyRef}
            onScroll={(event) => {
              const body = event.currentTarget;
              const distanceFromBottom = body.scrollHeight - body.scrollTop - body.clientHeight;
              stickToBottomRef.current = distanceFromBottom < 80;
            }}
          >
            <Flex direction="column" gap="0">
              {rows.map((row) => (
                <Grid key={row.id} columns="96px 44px 1fr" gap="2" align="center" className={`log-line log-${row.level}`}>
                  <Text color="gray" className="mono log-meta log-text">
                    {row.timestamp}
                  </Text>
                  <Text className="mono log-meta log-level log-text">{row.level}</Text>
                  <Text className="mono log-text">{row.message}</Text>
                </Grid>
              ))}
            </Flex>
          </Box>
        )}
      </Flex>
    </Card>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <Box className="metric">
      <Text as="div" size="1" color="gray">
        {label}
      </Text>
      <Text as="div" size="2" className="mono metric-value">
        {value}
      </Text>
    </Box>
  );
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <Box className="empty-state">
      <Heading size="4">{title}</Heading>
      <Text as="p" size="2" color="gray">
        {body}
      </Text>
    </Box>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Box>
      <Text as="label" size="2" weight="medium" mb="1" className="field-label">
        {label}
      </Text>
      {children}
    </Box>
  );
}

function BusySpinner() {
  return <Box className="inline-spinner" aria-hidden />;
}

function logEntry(level: LogLevel, scope: string, message: string): LogRow {
  return {
    id: nextLogRowId++,
    timestamp: new Date().toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
    level,
    scope,
    message: sanitizeLogMessage(message),
  };
}

function filterLogRows(rows: LogRow[], minimum: LogLevelFilter): LogRow[] {
  const rank: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };
  return rows.filter((row) => rank[row.level] >= rank[minimum]);
}

function limitLogRows(rows: LogRow[]): LogRow[] {
  if (rows.length <= maxStoredLogRows) return rows;
  return rows.slice(-maxStoredLogRows);
}

function updateLabel(status: UpdateStatus, availableUpdate: Update | null, progress: string | null): string {
  if (status === "checking") return "Checking";
  if (status === "installing") return progress ?? "Installing";
  if (status === "relaunching") return progress ?? "Relaunching";
  if (status === "failed") return "Check failed";
  if (availableUpdate) return `${availableUpdate.version} available`;
  if (status === "current") return "Up to date";
  return "Not checked";
}

function updateTone(status: UpdateStatus): "green" | "amber" | "red" {
  if (status === "failed") return "red";
  if (status === "current") return "green";
  return "amber";
}

function remoteStatusTone(
  statusError: string | undefined,
  liveStatus: RemoteServerStatus | undefined,
  battlegroupStarted: boolean,
  battlegroupStartRequested: boolean,
  battlegroupStopped: boolean,
  server: RemoteServerRecord,
): "green" | "amber" | "red" | "gray" {
  if (statusError) return "red";
  if (battlegroupStarted) return "green";
  if (battlegroupStartRequested) return "amber";
  if (battlegroupStopped) return "gray";
  if (server.phase === "Setup running") return "amber";
  return liveStatus ? "green" : "gray";
}

function remoteStatusLabel(
  statusError: string | undefined,
  liveStatus: RemoteServerStatus | undefined,
  busyLabel: string | undefined,
  battlegroupStarted: boolean,
  battlegroupStartRequested: boolean,
  server: RemoteServerRecord,
): string {
  if (statusError) return "Check failed";
  if (busyLabel) return "Retrieving";
  if (!liveStatus) return server.phase || "Unknown";
  if (battlegroupStarted) return "Started";
  return battlegroupStartRequested ? "Starting" : "Stopped";
}

function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return "Operation failed.";
}

function sanitizeLogMessage(message: string): string {
  return message.replace(
    /\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?::\d{1,5})?\b/g,
    "IP address",
  );
}

async function openFileDialog(title: string): Promise<string | null> {
  const selected = await open({ directory: false, multiple: false, title });
  return typeof selected === "string" ? selected : null;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${Math.round(bytes / 1024 / 1024)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function isBattlegroupStarted(status: RemoteBattlegroupStatus): boolean {
  return !status.stop && status.phase.toLowerCase() === "running";
}

function isDirectorReadyPhase(phase: string): boolean {
  const normalized = phase.toLowerCase();
  return normalized.includes("ready") || normalized.includes("running") || normalized === "true";
}

function tunnelServiceLabel(service: TunnelService): string {
  if (service === "fileBrowser") return "File Browser";
  if (service === "database") return "Postgres";
  if (service === "pgHero") return "PgHero";
  return "Director";
}

function serverTunnelKey(serverKey: string, service: TunnelService): string {
  return `${serverKey}:tunnel:${service}`;
}

function componentLogStateKey(serverKey: string, component: RemoteServerComponent): string {
  return `${serverKey}:${component.logKey}`;
}

function remoteServerDefaultUser(kind: RemoteServerKind): string {
  return kind === "ubuntu" ? "root" : "root";
}

function remoteServerActionRequest(server: RemoteServerRecord) {
  return {
    serverType: server.type,
    host: server.host,
    user: server.user || remoteServerDefaultUser(server.type),
    keyPath: server.keyPath || undefined,
    namespace: server.namespace,
    battlegroupName: server.battlegroupName,
  };
}

function isCriticalRestartComponent(component: RemoteServerComponent): boolean {
  const key = component.logKey.toLowerCase();
  const name = component.name.toLowerCase();
  return key.includes("database") || key.includes("messagequeue") || name.includes("database") || name.includes("message queue");
}

function readRemoteServers(): RemoteServerRecord[] {
  const text = window.localStorage.getItem(remoteServersStorageKey);
  if (!text) return [];
  try {
    const parsed = JSON.parse(text);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isRemoteServerRecord);
  } catch {
    window.localStorage.removeItem(remoteServersStorageKey);
    return [];
  }
}

function persistRemoteServers(servers: RemoteServerRecord[]): RemoteServerRecord[] {
  const unique = mergeRemoteServers([], servers);
  window.localStorage.setItem(remoteServersStorageKey, JSON.stringify(unique));
  return unique;
}

function mergeRemoteServers(current: RemoteServerRecord[], incoming: RemoteServerRecord[]): RemoteServerRecord[] {
  const byId = new Map(current.map((server) => [server.id, server]));
  for (const server of incoming) {
    byId.set(server.id, { ...byId.get(server.id), ...server });
  }
  return Array.from(byId.values()).sort((a, b) => a.name.localeCompare(b.name));
}

function upsertRemoteServer(servers: RemoteServerRecord[], server: RemoteServerRecord): RemoteServerRecord[] {
  return mergeRemoteServers(servers, [server]);
}

function isRemoteServerRecord(value: unknown): value is RemoteServerRecord {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<RemoteServerRecord>;
  return (
    record.type === "ubuntu" &&
    typeof record.id === "string" &&
    typeof record.name === "string" &&
    typeof record.host === "string" &&
    typeof record.keyPath === "string"
  );
}

function omitKey<T>(record: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _removed, ...rest } = record;
  return rest;
}

function omitPrefix<T>(record: Record<string, T>, prefix: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([key]) => !key.startsWith(prefix)));
}

async function copyTextToClipboard(text: string) {
  await navigator.clipboard.writeText(text);
}
