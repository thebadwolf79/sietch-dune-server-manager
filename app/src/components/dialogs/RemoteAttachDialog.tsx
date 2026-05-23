import { Button, Dialog, Flex, Grid, TextField } from "@radix-ui/themes";

import { openFileDialog } from "../../services/tauri";
import type { RemoteAttachForm } from "../../types/ui";
import Field from "../ui/Field";

export type RemoteAttachDialogProps = {
  open: boolean;
  form: RemoteAttachForm;
  running: boolean;
  onOpenChange: (open: boolean) => void;
  onChange: (form: RemoteAttachForm) => void;
  onAttach: () => void;
};

export default function RemoteAttachDialog({
  open,
  form,
  running,
  onOpenChange,
  onChange,
  onAttach,
}: RemoteAttachDialogProps) {
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
