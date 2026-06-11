import { Badge, Button, Flex } from "@radix-ui/themes";

import type { Update } from "../../services/updater";
import type { UpdateStatus } from "../../types/update";
import { updateLabel, updateTone } from "../../utils/formatting";
import AboutDialog from "../dialogs/AboutDialog";

export type UpdateHeaderControlProps = {
  status: UpdateStatus;
  update: Update | null;
  progress: string | null;
  onCheck: () => void;
  onOpenUpdate: () => void;
};

export default function UpdateHeaderControl({
  status,
  update,
  progress,
  onCheck,
  onOpenUpdate,
}: UpdateHeaderControlProps) {
  const busy = status === "checking" || status === "installing" || status === "relaunching";
  const hasUpdate = Boolean(update);
  // Only surface a status badge when it carries actionable meaning. An idle or
  // failed/unreachable check (e.g. no published release yet) shows nothing —
  // the "Check for updates" button alone is a cleaner, non-alarming default.
  const showBadge = busy || hasUpdate || status === "current";
  return (
    <Flex align="center" gap="2" className="header-update">
      {showBadge ? (
        <Badge color={updateTone(status)} variant="soft">
          {updateLabel(status, update, progress)}
        </Badge>
      ) : null}
      <Button size="1" variant={hasUpdate ? "solid" : "surface"} disabled={busy} onClick={hasUpdate ? onOpenUpdate : onCheck}>
        {busy ? "Working..." : hasUpdate ? "Install" : "Check for updates"}
      </Button>
      <AboutDialog />
    </Flex>
  );
}
