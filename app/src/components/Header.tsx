import { Component, type ErrorInfo } from "react";
import { Flex, Heading, Badge, Button, Box, TabNav, Card, Text } from "@radix-ui/themes";
import { CubeIcon } from "@radix-ui/react-icons";
import { type Update } from "@tauri-apps/plugin-updater";
import {
  type PageId,
  type UpdateStatus,
  type ServerPackageStatus,
  type ServerPackageCheckStatus,
  type AppErrorBoundaryProps,
  type AppErrorBoundaryState,
  pages
} from "../types";
import {
  serverPackageTone,
  serverPackageLabel,
  updateTone,
  updateLabel
} from "../utils/helpers";

export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  override state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error: error.message };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    this.props.onError(`${error.message}${info.componentStack ? `; ${info.componentStack.split("\n")[1]?.trim() ?? ""}` : ""}`);
  }

  override render() {
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

export function Header({
  activePage,
  onNavigate,
  serverCount,
  updateStatus,
  update,
  updateProgress,
  serverPackageStatus,
  serverPackageCheckStatus,
  onCheckUpdate,
  onOpenUpdate,
  onCheckServerPackage,
  onUpdateServerPackage,
}: {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
  serverCount: number;
  updateStatus: UpdateStatus;
  update: Update | null;
  updateProgress: string | null;
  serverPackageStatus: ServerPackageStatus | null;
  serverPackageCheckStatus: ServerPackageCheckStatus;
  onCheckUpdate: () => void;
  onOpenUpdate: () => void;
  onCheckServerPackage: () => void;
  onUpdateServerPackage: () => void;
}) {
  return (
    <Flex asChild align="center" justify="between" p="4">
      <header>
        <Flex align="center" gap="5">
          <Flex align="center" gap="3">
            <CubeIcon width="24" height="24" />
            <Heading size="4">Dune Dedicated Server Manager</Heading>
          </Flex>
          <TopNav
            activePage={activePage}
            onNavigate={onNavigate}
            serverCount={serverCount}
          />
        </Flex>
        <Flex align="center" gap="2" wrap="wrap" justify="end">
          <ServerPackageHeaderControl
            status={serverPackageCheckStatus}
            packageStatus={serverPackageStatus}
            onCheck={onCheckServerPackage}
            onUpdate={onUpdateServerPackage}
          />
          <UpdateHeaderControl
            status={updateStatus}
            update={update}
            progress={updateProgress}
            onCheck={onCheckUpdate}
            onOpenUpdate={onOpenUpdate}
          />
        </Flex>
      </header>
    </Flex>
  );
}

export function ServerPackageHeaderControl({
  status,
  packageStatus,
  onCheck,
  onUpdate,
}: {
  status: ServerPackageCheckStatus;
  packageStatus: ServerPackageStatus | null;
  onCheck: () => void;
  onUpdate: () => void;
}) {
  const busy = status === "checking" || status === "updating";
  const canUpdate = status === "available" || status === "missing" || (packageStatus ? !packageStatus.complete : false);
  return (
    <Flex align="center" gap="2" className="header-update">
      <Badge color={serverPackageTone(status)} variant="soft">
        {serverPackageLabel(status, packageStatus)}
      </Badge>
      <Button size="1" variant="surface" disabled={busy} onClick={canUpdate ? onUpdate : onCheck}>
        {busy ? "Working..." : canUpdate ? "Update package" : "Check package"}
      </Button>
    </Flex>
  );
}

export function UpdateHeaderControl({
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
  const actionLabel = hasUpdate ? "Install" : "Check for updates";

  return (
    <Flex align="center" gap="2" className="header-update">
      <Badge color={updateTone(status)} variant="soft">
        {updateLabel(status, update, progress)}
      </Badge>
      <Button
        size="1"
        variant={hasUpdate ? "solid" : "surface"}
        disabled={busy}
        onClick={hasUpdate ? onOpenUpdate : onCheck}
      >
        {busy ? "Working..." : actionLabel}
      </Button>
    </Flex>
  );
}

export function TopNav({
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
