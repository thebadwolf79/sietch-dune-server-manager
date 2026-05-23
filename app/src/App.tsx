import { Box, Flex, Theme } from "@radix-ui/themes";

import AppErrorBoundary from "./components/AppErrorBoundary";
import Header from "./components/layout/Header";
import LogWindow from "./components/logs/LogWindow";
import RemoteAttachDialog from "./components/dialogs/RemoteAttachDialog";
import RemoveRemoteServerDialog from "./components/dialogs/RemoveRemoteServerDialog";
import UpdateDialog from "./components/dialogs/UpdateDialog";
import ServersPage from "./components/servers/ServersPage";
import { useAppUpdates } from "./hooks/useAppUpdates";
import { useComponentActions } from "./hooks/useComponentActions";
import { useOperationLogs } from "./hooks/useOperationLogs";
import { useRemoteServerStatus } from "./hooks/useRemoteServerStatus";
import { useRemoteServers } from "./hooks/useRemoteServers";
import { useServerTunnels } from "./hooks/useServerTunnels";
import { useActivePage } from "./hooks/useActivePage";
import { log } from "./utils/logging";

export function App() {
  const { activePage, setActivePage } = useActivePage();

  const {
    logLevelFilter,
    setLogLevelFilter,
    logPanelCollapsed,
    setLogPanelCollapsed,
    appendLogRow,
    clearLogRows,
    renderedLogRows,
  } = useOperationLogs();

  const remoteServersHook = useRemoteServers({ appendLogRow });

  const tunnels = useServerTunnels({ appendLogRow });

  const status = useRemoteServerStatus({
    appendLogRow,
    setRemoteServers: remoteServersHook.setRemoteServers,
  });

  const componentActions = useComponentActions({
    appendLogRow,
    detectRemoteServerDetails: status.detectRemoteServerDetails,
    setRemoteServerComponents: status.setRemoteServerComponents,
    setRemoteComponentLogs: status.setRemoteComponentLogs,
    setRemoteComponentLogBusy: status.setRemoteComponentLogBusy,
    setRemoteComponentRestartBusy: status.setRemoteComponentRestartBusy,
  });

  const updates = useAppUpdates({ appendLogRow });

  remoteServersHook.bindRefreshRemoteServerStatus(status.refreshRemoteServerStatus);
  remoteServersHook.bindRemoteServerBusy(status.remoteServerBusy);
  remoteServersHook.bindClearStatusForServer(status.clearStatusForServer);
  remoteServersHook.bindStopTunnelsForServer(tunnels.stopTunnelsForServer);

  return (
    <Theme accentColor="bronze" grayColor="sand" radius="medium" scaling="100%">
      <Flex direction="column" height="100vh" className="app-shell">
        <Header
          activePage={activePage}
          onNavigate={setActivePage}
          serverCount={remoteServersHook.remoteServers.length}
          updateStatus={updates.updateStatus}
          update={updates.availableUpdate}
          updateProgress={updates.updateProgress}
          onCheckUpdate={updates.checkForAppUpdate}
          onOpenUpdate={() => updates.setUpdateDialogOpen(true)}
        />
        <Flex className="content-shell" gap="3" p="4" pt="0" minHeight="0">
          <Box className="main-pane">
            <AppErrorBoundary onError={(message) => appendLogRow(log.error("ui", message))}>
              <ServersPage
                remoteServers={remoteServersHook.remoteServers}
                remoteStatuses={status.remoteServerStatuses}
                remoteComponents={status.remoteServerComponents}
                remoteComponentLogs={status.remoteComponentLogs}
                remoteComponentLogBusy={status.remoteComponentLogBusy}
                remoteComponentRestartBusy={status.remoteComponentRestartBusy}
                remoteStatusErrors={status.remoteServerStatusErrors}
                remoteBusy={status.remoteServerBusy}
                tunnels={tunnels.serverTunnels}
                tunnelBusy={tunnels.serverTunnelBusy}
                onAddRemoteServer={() => remoteServersHook.setRemoteAttachOpen(true)}
                onRemoveRemoteServer={(server) => remoteServersHook.setRemoteServerToRemove(server)}
                onRefreshRemoteStatus={status.refreshRemoteServerStatus}
                onStartRemoteBattlegroup={(server) => status.runRemoteBattlegroupAction(server, "start")}
                onStopRemoteBattlegroup={(server) => status.runRemoteBattlegroupAction(server, "stop")}
                onUpdateRemoteBattlegroup={(server) => status.runRemoteBattlegroupAction(server, "update")}
                onStartTunnel={tunnels.startServerTunnel}
                onStopTunnel={tunnels.stopServerTunnel}
                onOpenTunnel={tunnels.openServerTunnel}
                onRefreshRemoteComponentLog={componentActions.refreshRemoteComponentLog}
                onRestartRemoteComponent={componentActions.restartRemoteComponent}
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
          open={remoteServersHook.remoteAttachOpen}
          form={remoteServersHook.remoteAttachForm}
          running={remoteServersHook.remoteAttachRunning}
          onOpenChange={remoteServersHook.setRemoteAttachOpen}
          onChange={remoteServersHook.setRemoteAttachForm}
          onAttach={remoteServersHook.addRemoteServer}
        />
        <RemoveRemoteServerDialog
          server={remoteServersHook.remoteServerToRemove}
          onOpenChange={(open) => {
            if (!open) remoteServersHook.setRemoteServerToRemove(null);
          }}
          onRemove={(server) => {
            remoteServersHook.removeRemoteServer(server);
            remoteServersHook.setRemoteServerToRemove(null);
          }}
        />
        <UpdateDialog
          open={updates.updateDialogOpen}
          update={updates.availableUpdate}
          status={updates.updateStatus}
          progress={updates.updateProgress}
          onOpenChange={updates.setUpdateDialogOpen}
          onInstall={updates.installAppUpdate}
        />
      </Flex>
    </Theme>
  );
}
