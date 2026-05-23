import { Badge, Button, Flex } from "@radix-ui/themes";

import type { Update } from "../../services/updater";
import type { UpdateStatus } from "../../types/update";
import { updateLabel, updateTone } from "../../utils/formatting";

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
