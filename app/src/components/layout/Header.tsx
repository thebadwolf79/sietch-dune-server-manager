import { Flex } from "@radix-ui/themes";

import type { Update } from "../../services/updater";
import type { RemoteServerRecord, RemoteServerStatus } from "../../types/server";
import type { ActivePage } from "../../types/ui";
import type { UpdateStatus } from "../../types/update";
import TopNav from "./TopNav";
import UpdateHeaderControl from "./UpdateHeaderControl";

export type HeaderProps = {
  activePage: ActivePage;
  servers: RemoteServerRecord[];
  statuses: Record<string, RemoteServerStatus>;
  statusErrors: Record<string, string>;
  busyMap: Record<string, string>;
  onOpenServersList: () => void;
  onOpenServer: (serverId: string) => void;
  onAddServer: () => void;
  updateStatus: UpdateStatus;
  update: Update | null;
  updateProgress: string | null;
  onCheckUpdate: () => void;
  onOpenUpdate: () => void;
};

export default function Header({
  activePage,
  servers,
  statuses,
  statusErrors,
  busyMap,
  onOpenServersList,
  onOpenServer,
  onAddServer,
  updateStatus,
  update,
  updateProgress,
  onCheckUpdate,
  onOpenUpdate,
}: HeaderProps) {
  return (
    <Flex asChild align="center" justify="between" px="4" py="3" className="app-header">
      <header>
        <Flex align="center" gap="4">
          <Flex align="center" gap="3">
            <span className="app-glyph" aria-hidden>
              <img src="/app-icon.png" alt="Sietch Logo" style={{ width: "100%", height: "100%", borderRadius: "inherit", objectFit: "cover" }} />
            </span>
            <Flex direction="column" gap="0">
              <span className="app-title">Sietch</span>
              <span className="app-title-sub">Dune Server Manager · Operator console</span>
            </Flex>
          </Flex>
          <TopNav
            activePage={activePage}
            servers={servers}
            statuses={statuses}
            statusErrors={statusErrors}
            busyMap={busyMap}
            onOpenServersList={onOpenServersList}
            onOpenServer={onOpenServer}
            onAddServer={onAddServer}
          />
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
