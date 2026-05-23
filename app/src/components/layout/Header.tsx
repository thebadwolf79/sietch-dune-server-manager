import { Flex } from "@radix-ui/themes";

import type { Update } from "../../services/updater";
import type { PageId } from "../../types/ui";
import type { UpdateStatus } from "../../types/update";
import TopNav from "./TopNav";
import UpdateHeaderControl from "./UpdateHeaderControl";

export type HeaderProps = {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
  serverCount: number;
  updateStatus: UpdateStatus;
  update: Update | null;
  updateProgress: string | null;
  onCheckUpdate: () => void;
  onOpenUpdate: () => void;
};

export default function Header({
  activePage,
  onNavigate,
  serverCount,
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
              D
            </span>
            <Flex direction="column" gap="0">
              <span className="app-title">Dune Dedicated Server Manager</span>
              <span className="app-title-sub">Operator console</span>
            </Flex>
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
