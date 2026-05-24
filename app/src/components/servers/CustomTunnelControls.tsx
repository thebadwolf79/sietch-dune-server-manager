import { useEffect, useState } from "react";

import { Box, Button, Flex, Grid, Link, Select, Text, TextField } from "@radix-ui/themes";
import { TrashIcon } from "@radix-ui/react-icons";

import type { RemoteServerKind } from "../../types/server";
import type {
  CustomTunnelDef,
  CustomTunnelProtocol,
  CustomTunnelStartRequest,
  ServerTunnelStatus,
} from "../../types/tunnel";
import { readCustomTunnels, writeCustomTunnels } from "../../services/storage";
import BusySpinner from "../ui/BusySpinner";

type Props = {
  serverKey: string;
  host: string;
  serverKind: RemoteServerKind;
  user: string;
  keyPath?: string;
  port?: number;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onStartCustomTunnel: (request: CustomTunnelStartRequest, name: string) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
};

const BLANK_FORM = {
  name: "",
  protocol: "http" as CustomTunnelProtocol,
  remotePort: "",
  localPort: "",
};

export default function CustomTunnelControls({
  serverKey,
  host,
  serverKind,
  user,
  keyPath,
  port,
  tunnels,
  tunnelBusy,
  onStartCustomTunnel,
  onStopTunnel,
  onOpenTunnel,
}: Props) {
  const [defs, setDefs] = useState<CustomTunnelDef[]>(() => readCustomTunnels(serverKey));
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState(BLANK_FORM);

  useEffect(() => {
    writeCustomTunnels(serverKey, defs);
  }, [serverKey, defs]);

  const addDef = () => {
    const remotePort = parseInt(form.remotePort, 10);
    const localPort = parseInt(form.localPort, 10) || 0;
    if (!form.name.trim() || !remotePort) return;
    const def: CustomTunnelDef = {
      id: crypto.randomUUID(),
      name: form.name.trim(),
      protocol: form.protocol,
      remotePort,
      localPort,
    };
    setDefs((prev) => [...prev, def]);
    setForm(BLANK_FORM);
    setShowForm(false);
  };

  const removeDef = (id: string) => setDefs((prev) => prev.filter((d) => d.id !== id));

  return (
    <Box mt="2">
      <Flex direction="column" gap="2">
        {defs.map((def) => {
          const tunnelId = `${serverKey}:tunnel:custom:${def.id}`;
          const active = tunnels[tunnelId];
          const busy = !!tunnelBusy[tunnelId];
          const openLabel = def.protocol === "postgresql" ? "Copy URI" : "Open";
          const disabled = busy || (!active && !host.trim());
          return (
            <Flex key={def.id} align="center" justify="between" gap="3" wrap="wrap" className="tunnel-row">
              <Flex direction="column" gap="1" minWidth="0">
                <Text size="2" weight="medium">{def.name}</Text>
                <Text size="1" color="gray">
                  {active
                    ? `Forwarding remote port ${active.remotePort} to local port ${active.localPort}`
                    : !host.trim()
                      ? "Requires server host"
                      : "Tunnel stopped"}
                </Text>
              </Flex>
              <Flex align="center" gap="2" wrap="wrap" justify="end">
                {active ? (
                  <Button type="button" size="1" variant="surface" onClick={() => onOpenTunnel(active)}>
                    {openLabel}
                  </Button>
                ) : null}
                <Button
                  type="button"
                  size="1"
                  variant={active ? "soft" : "surface"}
                  color={active ? "red" : undefined}
                  disabled={disabled}
                  onClick={() => {
                    if (active) {
                      onStopTunnel(tunnelId);
                      return;
                    }
                    onStartCustomTunnel(
                      { tunnelId, serverKind, host, user, keyPath, port, protocol: def.protocol, remotePort: def.remotePort, localPort: def.localPort },
                      def.name,
                    );
                  }}
                >
                  {busy ? (
                    <Flex align="center" gap="1"><BusySpinner /> Working</Flex>
                  ) : active ? (
                    "Stop Tunnel"
                  ) : (
                    "Start Tunnel"
                  )}
                </Button>
                {!active ? (
                  <Button type="button" size="1" variant="ghost" color="red" disabled={busy} onClick={() => removeDef(def.id)}>
                    <TrashIcon />
                  </Button>
                ) : null}
                {active ? (
                  <Link
                    size="1"
                    href="#"
                    className="mono tunnel-url"
                    onClick={(event) => {
                      event.preventDefault();
                      onOpenTunnel(active);
                    }}
                  >
                    {active.url}
                  </Link>
                ) : null}
              </Flex>
            </Flex>
          );
        })}

        {showForm ? (
          <Flex direction="column" gap="2" mt="1">
            <Grid columns="2fr 1fr" gap="2">
              <TextField.Root
                placeholder="Name"
                size="1"
                value={form.name}
                onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              />
              <Select.Root
                value={form.protocol}
                onValueChange={(v) => setForm((f) => ({ ...f, protocol: v as CustomTunnelProtocol }))}
              >
                <Select.Trigger />
                <Select.Content>
                  <Select.Item value="http">http</Select.Item>
                  <Select.Item value="https">https</Select.Item>
                  <Select.Item value="postgresql">postgresql</Select.Item>
                </Select.Content>
              </Select.Root>
            </Grid>
            <Grid columns="1fr 1fr" gap="2">
              <TextField.Root
                placeholder="Remote port"
                size="1"
                type="number"
                min={1}
                max={65535}
                value={form.remotePort}
                onChange={(e) => setForm((f) => ({ ...f, remotePort: e.target.value }))}
              />
              <TextField.Root
                placeholder="Local port (0 = auto)"
                size="1"
                type="number"
                min={0}
                max={65535}
                value={form.localPort}
                onChange={(e) => setForm((f) => ({ ...f, localPort: e.target.value }))}
              />
            </Grid>
            <Flex gap="2">
              <Button
                type="button"
                size="1"
                variant="surface"
                disabled={!form.name.trim() || !form.remotePort}
                onClick={addDef}
              >
                Add
              </Button>
              <Button
                type="button"
                size="1"
                variant="ghost"
                color="gray"
                onClick={() => { setShowForm(false); setForm(BLANK_FORM); }}
              >
                Cancel
              </Button>
            </Flex>
          </Flex>
        ) : (
          <Button
            type="button"
            size="1"
            variant="ghost"
            style={{ alignSelf: "flex-start" }}
            onClick={() => setShowForm(true)}
          >
            + Add Custom Tunnel
          </Button>
        )}
      </Flex>
    </Box>
  );
}
