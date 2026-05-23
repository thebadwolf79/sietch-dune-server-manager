import { Flex, Heading } from "@radix-ui/themes";
import { CubeIcon } from "@radix-ui/react-icons";

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
