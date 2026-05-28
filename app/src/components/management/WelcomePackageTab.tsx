import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ChevronDownIcon,
  ChevronUpIcon,
  PlusIcon,
  PaperPlaneIcon,
  TrashIcon,
} from "@radix-ui/react-icons";
import {
  Badge,
  Box,
  Button,
  Checkbox,
  Dialog,
  Flex,
  Separator,
  Table,
  Text,
  TextArea,
  TextField,
  Tooltip,
} from "@radix-ui/themes";

import { managementApi, managementService } from "../../services/management";
import type { PlayerDto, PublishResultDto, ScheduleConfig, WelcomeGrantDto } from "../../types/management";
import type { RemoteServerRecord } from "../../types/server";
import Combobox from "./Combobox";
import ItemCombobox from "./ItemCombobox";

type WelcomeAction =
  | { type: "grantItem"; itemName: string; quantity: number; durability: number }
  | { type: "refillWater"; waterAmount: number; delayAfterPreviousSecs: number };

export type WelcomePackageTabProps = {
  tunnelId: string;
  server: RemoteServerRecord;
  onAfterRestart?: () => Promise<void> | void;
};

export default function WelcomePackageTab({
  tunnelId,
  server,
  onAfterRestart,
}: WelcomePackageTabProps) {
  const [config, setConfig] = useState<ScheduleConfig | null>(null);
  const [grants, setGrants] = useState<WelcomeGrantDto[]>([]);
  const [enabled, setEnabled] = useState(false);
  const [messageEnabled, setMessageEnabled] = useState(false);
  const [requireEmptyBackpack, setRequireEmptyBackpack] = useState(false);
  const [pollSecs, setPollSecs] = useState(30);
  const [onlineGraceSecs, setOnlineGraceSecs] = useState(20);
  const [whisperSourcePlayer, setWhisperSourcePlayer] = useState("");
  const [welcomeMessage, setWelcomeMessage] = useState("");
  const [testRecipientPlayer, setTestRecipientPlayer] = useState("");
  const [testMessage, setTestMessage] = useState("");
  const [testOpen, setTestOpen] = useState(false);
  const [actions, setActions] = useState<WelcomeAction[]>([]);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [sendingWhisper, setSendingWhisper] = useState(false);
  const [whisperResult, setWhisperResult] = useState<PublishResultDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const c = await managementApi.getConfig(tunnelId);
      const g = await managementApi.welcomeGrants(tunnelId, 50);
      setConfig(c);
      setEnabled(c.welcomePackageEnabled);
      setMessageEnabled(c.welcomeMessageEnabled ?? false);
      setRequireEmptyBackpack(c.welcomePackageRequireEmptyBackpack ?? false);
      setPollSecs(c.welcomePackagePollSecs);
      setOnlineGraceSecs(c.welcomePackageOnlineGraceSecs);
      setWhisperSourcePlayer(c.welcomeWhisperSourcePlayer ?? "");
      setWelcomeMessage(c.welcomeMessage ?? "");
      setActions(parseActions(c.welcomePackageActionsJson || c.welcomePackageItemsJson || "[]"));
      setGrants(g);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [tunnelId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const actionsJson = useMemo(() => JSON.stringify(actions, null, 2), [actions]);

  const save = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      if (messageEnabled && !welcomeMessage.trim()) {
        throw new Error("Enabled welcome message needs message text.");
      }
      validateActions(actions);
      await managementApi.setConfig(tunnelId, {
        welcomeMessageEnabled: messageEnabled,
        welcomePackageEnabled: enabled,
        welcomePackageRequireEmptyBackpack: requireEmptyBackpack,
        welcomePackageVersion: "v1",
        welcomePackagePollSecs: pollSecs,
        welcomePackageOnlineGraceSecs: onlineGraceSecs,
        welcomePackageActionsJson: actionsJson,
        welcomeWhisperSourcePlayer: whisperSourcePlayer,
        welcomeMessage,
      });
      await managementService.restart({
        host: server.host,
        user: server.user,
        keyPath: server.keyPath,
        port: server.port,
      });
      await waitForConfig(tunnelId);
      await refresh();
      await onAfterRestart?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [
    actions,
    actionsJson,
    enabled,
    messageEnabled,
    onlineGraceSecs,
    pollSecs,
    requireEmptyBackpack,
    refresh,
    server.host,
    server.keyPath,
    server.port,
    server.user,
    tunnelId,
    whisperSourcePlayer,
    welcomeMessage,
    onAfterRestart,
  ]);

  const sendWhisper = useCallback(async () => {
    setSendingWhisper(true);
    setError(null);
    setWhisperResult(null);
    try {
      if (!testRecipientPlayer.trim()) throw new Error("Pick a recipient player.");
      if (!testMessage.trim()) throw new Error("Welcome message must not be empty.");
      const result = await managementApi.sendWelcomeWhisper(
        tunnelId,
        testRecipientPlayer,
        whisperSourcePlayer,
        testMessage,
      );
      setWhisperResult(result);
      if (result.ok) setTestOpen(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSendingWhisper(false);
    }
  }, [testMessage, testRecipientPlayer, tunnelId, whisperSourcePlayer]);

  const trigger = useCallback(async () => {
    setRunning(true);
    setError(null);
    try {
      await managementApi.triggerRun(tunnelId, "welcome-package");
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }, [refresh, tunnelId]);

  const restartRequired = config?.restartRequired ?? false;

  return (
    <Box mt="3">
      <Flex justify="between" align="start" gap="3" wrap="wrap">
        <Box>
          <Text size="3" weight="medium">Welcome automation</Text>
          <Flex gap="2" mt="2" align="center" wrap="wrap">
            <Badge color={messageEnabled ? "green" : "gray"}>
              message {messageEnabled ? "enabled" : "off"}
            </Badge>
            <Badge color={enabled ? "green" : "gray"}>
              package {enabled ? "enabled" : "off"}
            </Badge>
            <Text size="1" color="gray" className="mono">
              {pollSecs}s poll · {onlineGraceSecs}s grace
            </Text>
          </Flex>
        </Box>
        <Flex gap="2" align="center" wrap="wrap">
          <Button size="1" variant="surface" onClick={refresh} disabled={busy || running}>
            Refresh
          </Button>
          <Button size="1" variant="surface" onClick={trigger} disabled={busy || running}>
            {running ? "Running..." : "Run scan"}
          </Button>
          <Button size="1" onClick={save} disabled={busy || running}>
            {busy ? "Saving..." : "Save & restart service"}
          </Button>
        </Flex>
      </Flex>

      <Separator size="4" my="3" />

      <Box className="run-row-body">
        <Flex direction="column" gap="3">
          <Flex justify="between" align="center" gap="3" wrap="wrap">
            <Text size="2" weight="medium">Welcome message</Text>
            <Flex gap="2" align="center" wrap="wrap">
              {whisperResult ? (
                <Badge color={whisperResult.ok ? "green" : "red"}>
                  {whisperResult.ok ? "sent" : "failed"}
                </Badge>
              ) : null}
              <Button
                size="1"
                variant="surface"
                onClick={() => {
                  setTestMessage(welcomeMessage);
                  setTestOpen(true);
                }}
                disabled={busy}
              >
                <PaperPlaneIcon />
                Test
              </Button>
            </Flex>
          </Flex>
          <Flex align="center" gap="2">
            <Checkbox
              checked={messageEnabled}
              onCheckedChange={(checked) => setMessageEnabled(Boolean(checked))}
            />
            <Text size="2">Send to new players</Text>
          </Flex>
          <Flex gap="3" align="end" wrap="wrap">
            <Box style={{ flex: "1 1 280px", minWidth: 240 }}>
              <Text size="1" color="gray">Sender identity</Text>
              <PlayerCombobox
                tunnelId={tunnelId}
                value={whisperSourcePlayer}
                onChange={setWhisperSourcePlayer}
              />
            </Box>
          </Flex>
          <Box>
            <Text size="1" color="gray">Message</Text>
            <TextArea
              value={welcomeMessage}
              onChange={(e) => setWelcomeMessage(e.target.value)}
              rows={3}
              maxLength={1000}
            />
          </Box>
        </Flex>
      </Box>

      <Box mt="4">
        <Flex direction="column" gap="2" mb="2">
          <Text size="2" weight="medium">Welcome package</Text>
          <Flex align="center" gap="2">
            <Checkbox checked={enabled} onCheckedChange={(checked) => setEnabled(Boolean(checked))} />
            <Text size="2">Grant to new players</Text>
          </Flex>
          <Flex align="center" gap="2">
            <Checkbox
              checked={requireEmptyBackpack}
              onCheckedChange={(checked) => setRequireEmptyBackpack(Boolean(checked))}
            />
            <Text size="2">Wait for empty backpack</Text>
          </Flex>
        </Flex>

        <Box className="schedule-grid">
          <Text size="2">Poll seconds</Text>
          <TextField.Root
            type="number"
            value={String(pollSecs)}
            onChange={(e) => setPollSecs(Number(e.target.value) || 0)}
          />

          <Text size="2">Online grace seconds</Text>
          <TextField.Root
            type="number"
            value={String(onlineGraceSecs)}
            onChange={(e) => setOnlineGraceSecs(Number(e.target.value) || 0)}
          />
        </Box>

        <Flex justify="between" align="center" gap="3" wrap="wrap" mb="2">
          <Text size="2" weight="medium">Action chain</Text>
          <Flex gap="2" wrap="wrap">
            <Button
              size="1"
              variant="surface"
              onClick={() =>
                setActions((prev) => [
                  ...prev,
                  { type: "grantItem", itemName: "", quantity: 1, durability: 1.0 },
                ])
              }
            >
              <PlusIcon />
              Add item grant
            </Button>
            <Button
              size="1"
              variant="surface"
              onClick={() =>
                setActions((prev) => [
                  ...prev,
                  { type: "refillWater", waterAmount: 1_000_000, delayAfterPreviousSecs: 30 },
                ])
              }
            >
              <PlusIcon />
              Add water refill
            </Button>
          </Flex>
        </Flex>

        <Flex direction="column" gap="2">
          {actions.length === 0 ? (
            <Text size="2" color="gray">No actions configured.</Text>
          ) : (
            actions.map((action, index) => (
              <ActionRow
                key={`${index}:${action.type}`}
                tunnelId={tunnelId}
                index={index}
                actionCount={actions.length}
                action={action}
                onChange={(next) =>
                  setActions((prev) => prev.map((row, i) => (i === index ? next : row)))
                }
                onRemove={() => setActions((prev) => prev.filter((_, i) => i !== index))}
                onMove={(direction) => {
                  setActions((prev) => moveAction(prev, index, direction));
                }}
              />
            ))
          )}
        </Flex>
      </Box>

      {restartRequired ? (
        <Text size="1" color="amber" as="div" mt="3">
          Saved values differ from the running service; save/restart applies the active chain.
        </Text>
      ) : null}
      {error ? (
        <Text size="1" color="red" as="div" mt="3">
          {error}
        </Text>
      ) : null}

      <Dialog.Root open={testOpen} onOpenChange={setTestOpen}>
        <Dialog.Content maxWidth="520px">
          <Dialog.Title>Test welcome message</Dialog.Title>
          <Flex direction="column" gap="3" mt="3">
            <Box>
              <Text size="1" color="gray">Recipient</Text>
              <PlayerCombobox
                tunnelId={tunnelId}
                value={testRecipientPlayer}
                onChange={setTestRecipientPlayer}
              />
            </Box>
            <Box>
              <Text size="1" color="gray">Message</Text>
              <TextArea
                value={testMessage}
                onChange={(e) => setTestMessage(e.target.value)}
                rows={4}
                maxLength={1000}
              />
            </Box>
          </Flex>
          <Flex justify="end" gap="2" mt="4">
            <Dialog.Close>
              <Button size="1" variant="ghost" color="gray">
                Cancel
              </Button>
            </Dialog.Close>
            <Button
              size="1"
              onClick={sendWhisper}
              disabled={busy || sendingWhisper}
            >
              <PaperPlaneIcon />
              {sendingWhisper ? "Sending..." : "Send"}
            </Button>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>

      <Box mt="4">
        <Text size="2" weight="medium">Recent grants</Text>
        <Table.Root variant="surface" size="1" mt="2">
          <Table.Header>
            <Table.Row>
              <Table.ColumnHeaderCell>Status</Table.ColumnHeaderCell>
              <Table.ColumnHeaderCell>Player</Table.ColumnHeaderCell>
              <Table.ColumnHeaderCell>Updated</Table.ColumnHeaderCell>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {grants.length === 0 ? (
              <Table.Row>
                <Table.Cell colSpan={3}>
                  <Text size="1" color="gray">No grants recorded yet.</Text>
                </Table.Cell>
              </Table.Row>
            ) : (
              grants.map((grant) => (
                <Table.Row key={`${grant.playerId}:${grant.packageVersion}`}>
                  <Table.Cell>
                    <Badge color={grant.status === "granted" ? "green" : grant.status === "failed" ? "red" : "amber"}>
                      {grant.status}
                    </Badge>
                  </Table.Cell>
                  <Table.Cell>
                    <Text size="1" className="mono">{grant.playerId}</Text>
                    {grant.characterName ? (
                      <Text size="1" color="gray" as="div">{grant.characterName}</Text>
                    ) : null}
                  </Table.Cell>
                  <Table.Cell className="mono">{fmtDateTime(grant.updatedAt)}</Table.Cell>
                </Table.Row>
              ))
            )}
          </Table.Body>
        </Table.Root>
      </Box>
    </Box>
  );
}

function ActionRow({
  tunnelId,
  index,
  actionCount,
  action,
  onChange,
  onRemove,
  onMove,
}: {
  tunnelId: string;
  index: number;
  actionCount: number;
  action: WelcomeAction;
  onChange: (action: WelcomeAction) => void;
  onRemove: () => void;
  onMove: (direction: -1 | 1) => void;
}) {
  return (
    <Box className="run-row-body">
      <Flex gap="3" align="end" wrap="wrap">
        <Box style={{ flex: "0 0 70px" }}>
          <Text size="1" color="gray">Step</Text>
          <Text size="2" className="mono" as="div">{index + 1}</Text>
        </Box>
        {action.type === "grantItem" ? (
          <>
            <Box style={{ flex: "1 1 320px", minWidth: 260 }}>
              <Text size="1" color="gray">Grant item</Text>
              <ItemCombobox
                tunnelId={tunnelId}
                value={action.itemName}
                onChange={(itemName) => onChange({ ...action, itemName })}
              />
            </Box>
            <NumberField
              label="Qty"
              value={action.quantity}
              width={90}
              onChange={(quantity) => onChange({ ...action, quantity })}
            />
            <NumberField
              label="Durability"
              value={action.durability}
              width={110}
              step="0.1"
              onChange={(durability) => onChange({ ...action, durability })}
            />
          </>
        ) : (
          <>
            <Box style={{ flex: "1 1 320px", minWidth: 260 }}>
              <Text size="1" color="gray">Refill water</Text>
              <Text size="2" as="div">UpdateAllWaterFillables</Text>
            </Box>
            <NumberField
              label="Amount"
              value={action.waterAmount}
              width={150}
              onChange={(waterAmount) => onChange({ ...action, waterAmount })}
            />
            <NumberField
              label="Delay"
              value={action.delayAfterPreviousSecs}
              width={110}
              onChange={(delayAfterPreviousSecs) =>
                onChange({ ...action, delayAfterPreviousSecs })
              }
            />
          </>
        )}
        <Flex gap="1" align="center" style={{ marginLeft: "auto" }}>
          <Tooltip content="Move up">
            <Button
              size="1"
              variant="ghost"
              color="gray"
              onClick={() => onMove(-1)}
              disabled={index === 0}
              aria-label="Move action up"
            >
              <ChevronUpIcon />
            </Button>
          </Tooltip>
          <Tooltip content="Move down">
            <Button
              size="1"
              variant="ghost"
              color="gray"
              onClick={() => onMove(1)}
              disabled={index >= actionCount - 1}
              aria-label="Move action down"
            >
              <ChevronDownIcon />
            </Button>
          </Tooltip>
          <Tooltip content="Remove action">
            <Button
              size="1"
              variant="ghost"
              color="red"
              onClick={onRemove}
              aria-label="Remove action"
            >
              <TrashIcon />
            </Button>
          </Tooltip>
        </Flex>
      </Flex>
    </Box>
  );
}

function PlayerCombobox({
  tunnelId,
  value,
  onChange,
}: {
  tunnelId: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const loadOptions = useCallback(
    async (query: string) => managementApi.searchPlayers(tunnelId, query, 30),
    [tunnelId],
  );
  const resolveLabel = useCallback(
    async (id: string): Promise<string | null> => {
      if (!id) return null;
      try {
        const rows = await managementApi.searchPlayers(tunnelId, id, 5);
        const hit = rows.find((p) => p.flsId === id);
        return hit ? `${hit.name || "(unnamed)"} (${hit.online})  ·  ${hit.flsId}` : id;
      } catch {
        return id;
      }
    },
    [tunnelId],
  );
  return (
    <Combobox<PlayerDto>
      value={value}
      onChange={onChange}
      loadOptions={loadOptions}
      getOptionValue={(p) => p.flsId}
      resolveLabel={resolveLabel}
      renderOption={(p) => (
        <Flex justify="between" gap="2" align="center">
          <Box>
            <Text size="2">{p.name || "(unnamed)"}</Text>
            <Text size="1" color="gray" as="div" className="mono">{p.flsId}</Text>
          </Box>
          <Badge color={p.online?.toLowerCase() === "online" ? "green" : "gray"}>
            {p.online || "offline"}
          </Badge>
        </Flex>
      )}
      placeholder="Pick a player…"
      searchPlaceholder="Search players…"
    />
  );
}

function NumberField({
  label,
  value,
  width,
  step,
  onChange,
}: {
  label: string;
  value: number;
  width: number;
  step?: string;
  onChange: (value: number) => void;
}) {
  return (
    <Box style={{ flex: `0 0 ${width}px` }}>
      <Text size="1" color="gray">{label}</Text>
      <TextField.Root
        type="number"
        step={step}
        value={String(value)}
        onChange={(e) => onChange(Number(e.target.value) || 0)}
      />
    </Box>
  );
}

function parseActions(raw: string): WelcomeAction[] {
  try {
    const parsed = JSON.parse(raw || "[]");
    if (!Array.isArray(parsed)) return [];
    if (parsed.some((row) => row && typeof row === "object" && "type" in row)) {
      return parsed
        .map((row): WelcomeAction | null => {
          if (row?.type === "grantItem") {
            return {
              type: "grantItem",
              itemName: String(row.itemName ?? row.item_name ?? ""),
              quantity: Number(row.quantity ?? 1) || 1,
              durability: Number(row.durability ?? 1.0) || 1.0,
            };
          }
          if (row?.type === "refillWater") {
            return {
              type: "refillWater",
              waterAmount: Number(row.waterAmount ?? row.water_amount ?? 1_000_000) || 1_000_000,
              delayAfterPreviousSecs:
                Number(row.delayAfterPreviousSecs ?? row.delay_after_previous_secs ?? 30) || 0,
            };
          }
          return null;
        })
        .filter((row): row is WelcomeAction => !!row);
    }
    return parsed
      .map((row): WelcomeAction | null => ({
        type: "grantItem",
        itemName: String(row?.itemName ?? row?.item_name ?? ""),
        quantity: Number(row?.quantity ?? 1) || 1,
        durability: Number(row?.durability ?? 1.0) || 1.0,
      }))
      .filter((row): row is WelcomeAction => !!row && row.type === "grantItem" && row.itemName.trim().length > 0);
  } catch {
    return [];
  }
}

function validateActions(actions: WelcomeAction[]) {
  for (const action of actions) {
    if (action.type === "grantItem") {
      if (!action.itemName.trim()) throw new Error("Every item grant needs an item.");
      if (action.quantity <= 0) throw new Error(`Quantity for ${action.itemName} must be greater than 0.`);
      if (action.durability <= 0) throw new Error(`Durability for ${action.itemName} must be greater than 0.`);
    } else if (action.type === "refillWater" && action.waterAmount <= 0) {
      throw new Error("Water refill amount must be greater than 0.");
    } else if (action.type === "refillWater" && (action.delayAfterPreviousSecs < 0 || action.delayAfterPreviousSecs > 600)) {
      throw new Error("Water refill delay must be between 0 and 600 seconds.");
    }
  }
}

function moveAction(actions: WelcomeAction[], index: number, direction: -1 | 1): WelcomeAction[] {
  const target = index + direction;
  if (target < 0 || target >= actions.length) return actions;
  const next = [...actions];
  const [row] = next.splice(index, 1);
  next.splice(target, 0, row);
  return next;
}

async function waitForConfig(tunnelId: string) {
  const deadline = Date.now() + 15_000;
  let lastErr: unknown = null;
  while (Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 700));
    try {
      await managementApi.getConfig(tunnelId);
      return;
    } catch (err) {
      lastErr = err;
    }
  }
  throw new Error(`service did not come back up: ${lastErr}`);
}

function fmtDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.toISOString().slice(0, 10)} ${d.toISOString().slice(11, 19)}`;
}
