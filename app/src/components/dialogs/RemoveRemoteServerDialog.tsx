import { AlertDialog, Box, Button, Flex } from "@radix-ui/themes";

import type { RemoteServerRecord } from "../../types/server";
import Metric from "../ui/Metric";

export type RemoveRemoteServerDialogProps = {
  server: RemoteServerRecord | null;
  onOpenChange: (open: boolean) => void;
  onRemove: (server: RemoteServerRecord) => void;
};

export default function RemoveRemoteServerDialog({
  server,
  onOpenChange,
  onRemove,
}: RemoveRemoteServerDialogProps) {
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
