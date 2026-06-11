import { TabNav, Tooltip } from "@radix-ui/themes";
import { PlusIcon } from "@radix-ui/react-icons";

import type { RemoteServerRecord, RemoteServerStatus } from "../../types/server";
import type { ActivePage } from "../../types/ui";
import { resolveServerStatus } from "../../utils/remote-server";

export type TopNavProps = {
  activePage: ActivePage;
  servers: RemoteServerRecord[];
  statuses: Record<string, RemoteServerStatus>;
  statusErrors: Record<string, string>;
  busyMap: Record<string, string>;
  onOpenServersList: () => void;
  onOpenServer: (serverId: string) => void;
  onAddServer: () => void;
};

export default function TopNav({
  activePage,
  servers,
  statuses,
  statusErrors,
  busyMap,
  onOpenServersList,
  onOpenServer,
  onAddServer,
}: TopNavProps) {
  const serversActive = activePage.kind === "servers";
  const activeServerId = activePage.kind === "server" ? activePage.serverId : null;
  return (
    <nav aria-label="Primary navigation" className="top-nav">
      <TabNav.Root size="2" className="server-tab-strip">
        <TabNav.Link
          href="#"
          active={serversActive}
          aria-current={serversActive ? "page" : undefined}
          onClick={(event) => {
            event.preventDefault();
            onOpenServersList();
          }}
        >
          Servers ({servers.length})
        </TabNav.Link>
        {servers.map((server) => {
          const status = statuses[server.id];
          const resolved = resolveServerStatus(
            statusErrors[server.id],
            status,
            !!busyMap[server.id],
            server,
          );
          const isActive = activeServerId === server.id;
          return (
            <TabNav.Link
              key={server.id}
              href="#"
              active={isActive}
              aria-current={isActive ? "page" : undefined}
              onClick={(event) => {
                event.preventDefault();
                onOpenServer(server.id);
              }}
            >
              <span className="server-tab-dot" data-tone={resolved.tone} aria-hidden />
              <span className="server-tab-label">{server.name}</span>
            </TabNav.Link>
          );
        })}
        <Tooltip content="Add remote server">
          <button
            type="button"
            className="server-tab-add"
            aria-label="Add remote server"
            onClick={onAddServer}
          >
            <PlusIcon />
          </button>
        </Tooltip>
      </TabNav.Root>
    </nav>
  );
}
