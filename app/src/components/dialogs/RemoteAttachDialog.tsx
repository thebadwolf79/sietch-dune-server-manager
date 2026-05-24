import { Button, Callout, Dialog, Flex, Grid, TextField } from "@radix-ui/themes";
import { ExclamationTriangleIcon } from "@radix-ui/react-icons";

import { openFileDialog, type PreflightCheck } from "../../services/tauri";
import type { RemoteAttachForm } from "../../types/ui";
import ActionButton from "../ui/ActionButton";
import Field from "../ui/Field";

export type RemoteAttachDialogProps = {
  open: boolean;
  form: RemoteAttachForm;
  running: boolean;
  errorMessage?: string | null;
  preflight?: PreflightCheck | null;
  onOpenChange: (open: boolean) => void;
  onChange: (form: RemoteAttachForm) => void;
  onAttach: () => void;
};

export default function RemoteAttachDialog({
  open,
  form,
  running,
  errorMessage,
  preflight,
  onOpenChange,
  onChange,
  onAttach,
}: RemoteAttachDialogProps) {
  const canAttach =
    form.host.trim().length > 0 &&
    form.user.trim().length > 0 &&
    form.keyPath.trim().length > 0 &&
    form.port > 0 &&
    !running;
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="540px">
        <Dialog.Title>Add Remote Server</Dialog.Title>
        <Dialog.Description size="2" style={{ color: "var(--color-text-muted)" }}>
          Connect over SSH and detect existing Dune battlegroups. Vendor wrapper commands
          always execute as <code>dune</code>; if you log in as root we drop into dune via
          sudo automatically.
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
          <Grid columns="3fr 1fr" gap="3">
            <Field label="SSH User">
              <TextField.Root
                placeholder="dune"
                disabled={running}
                value={form.user}
                onChange={(event) => onChange({ ...form, user: event.target.value })}
              />
            </Field>
            <Field label="SSH Port">
              <TextField.Root
                placeholder="22"
                disabled={running}
                type="number"
                min={1}
                max={65535}
                value={String(form.port)}
                onChange={(event) => {
                  const parsed = parseInt(event.target.value, 10);
                  onChange({ ...form, port: isNaN(parsed) ? 22 : parsed });
                }}
              />
            </Field>
          </Grid>
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
          {errorMessage ? (
            <Callout.Root color="red" variant="surface">
              <Callout.Icon>
                <ExclamationTriangleIcon />
              </Callout.Icon>
              <Callout.Text style={{ whiteSpace: "pre-wrap" }}>{errorMessage}</Callout.Text>
            </Callout.Root>
          ) : null}
          {preflight && !errorMessage ? (
            <Callout.Root color="green" variant="surface">
              <Callout.Text>
                Preflight passed: SSH ok, sudo to dune ok, dune passwordless sudo ok.
              </Callout.Text>
            </Callout.Root>
          ) : null}
        </Flex>
        <Flex gap="3" justify="end" mt="5">
          <Dialog.Close>
            <Button variant="soft" color="gray" disabled={running}>
              Cancel
            </Button>
          </Dialog.Close>
          <ActionButton
            onClick={onAttach}
            busy={running}
            disabled={!canAttach}
            tone="accent"
            pendingLabel="Checking"
          >
            Detect and Add
          </ActionButton>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
