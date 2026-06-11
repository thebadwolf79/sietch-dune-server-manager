import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  PlusIcon,
  PaperPlaneIcon,
  ReloadIcon,
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
import { formatDateTime } from "../../utils/formatting";
import Combobox from "./Combobox";
import ItemCombobox from "./ItemCombobox";

type WelcomeAction = { type: "grantItem"; itemName: string; quantity: number };

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
  const [whisperSourcePlayer, setWhisperSourcePlayer] = useState("");
  const [welcomeMessage, setWelcomeMessage] = useState("");
  const [testRecipientPlayer, setTestRecipientPlayer] = useState("");
  const [testMessage, setTestMessage] = useState("");
  const [testOpen, setTestOpen] = useState(false);
  const [actions, setActions] = useState<WelcomeAction[]>([]);
  const [contentsOpen, setContentsOpen] = useState(true);
  const [jsonMode, setJsonMode] = useState(false);
  const [jsonText, setJsonText] = useState("[]");
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [sendingWhisper, setSendingWhisper] = useState(false);
  const [whisperResult, setWhisperResult] = useState<PublishResultDto | null>(null);
  const [retryingKey, setRetryingKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const c = await managementApi.getConfig(tunnelId);
      const g = await managementApi.welcomeGrants(tunnelId, 50);
      setConfig(c);
      setEnabled(c.welcomePackageEnabled);
      setMessageEnabled(c.welcomeMessageEnabled ?? false);
      setWhisperSourcePlayer(c.welcomeWhisperSourcePlayer ?? "");
      setWelcomeMessage(c.welcomeMessage ?? "");
      const rawJson = c.welcomePackageActionsJson || c.welcomePackageItemsJson || "[]";
      setActions(parseActions(rawJson));
      // Pretty-print and keep the JSON-mode textarea in sync with what the
      // service is actually persisting, so toggling into JSON mode after a
      // reload shows the current config rather than a stale buffer.
      try {
        setJsonText(JSON.stringify(JSON.parse(rawJson), null, 2));
      } catch {
        setJsonText(rawJson);
      }
      setJsonError(null);
      setGrants(g);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [tunnelId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const retryGrant = useCallback(
    async (grant: WelcomeGrantDto) => {
      const key = `${grant.playerId}:${grant.packageVersion}:${grant.accountId}`;
      setRetryingKey(key);
      setError(null);
      try {
        await managementApi.retryWelcomeGrant(
          tunnelId,
          grant.playerId,
          grant.packageVersion,
          grant.accountId,
        );
        await refresh();
      } catch (err) {
        setError(String(err));
      } finally {
        setRetryingKey(null);
      }
    },
    [refresh, tunnelId],
  );

  const actionsJson = useMemo(() => JSON.stringify(actions, null, 2), [actions]);

  const save = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      if (messageEnabled && !welcomeMessage.trim()) {
        throw new Error("Enabled welcome message needs message text.");
      }
      // In JSON mode the textarea is the source of truth — validate by
      // parsing it through the same shape check the visual editor uses.
      let outgoingActionsJson = actionsJson;
      if (jsonMode) {
        const parsed = parseActions(jsonText);
        validateActions(parsed);
        outgoingActionsJson = JSON.stringify(parsed, null, 2);
      } else {
        validateActions(actions);
      }
      await managementApi.setConfig(tunnelId, {
        welcomeMessageEnabled: messageEnabled,
        welcomePackageEnabled: enabled,
        welcomePackageVersion: "v1",
        welcomePackageActionsJson: outgoingActionsJson,
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
    jsonMode,
    jsonText,
    messageEnabled,
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
    <Box mt="3" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      {/* Header Panel */}
      <Box
        className="bracket chamfer"
        style={{
          background: "var(--color-bg-panel)",
          border: "1px solid var(--color-border-hair)",
          padding: "16px 20px",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
        }}
      >
        <Flex justify="between" align="center" gap="3" wrap="wrap">
          <Box>
            <Text size="3" weight="bold" className="font-display">Welcome automation</Text>
            <Flex gap="2" mt="2" align="center" wrap="wrap">
              <Badge color={messageEnabled ? "green" : "gray"}>
                message {messageEnabled ? "enabled" : "off"}
              </Badge>
              <Badge color={enabled ? "green" : "gray"}>
                package {enabled ? "enabled" : "off"}
              </Badge>
            </Flex>
          </Box>
          <Flex gap="2" align="center" wrap="wrap">
            <button
              type="button"
              onClick={refresh}
              disabled={busy || running}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px 12px",
                fontSize: "12.5px",
                cursor: (busy || running) ? "not-allowed" : "pointer",
                border: "1px solid var(--color-border-hair)",
                background: "var(--color-bg-elevated)",
                borderRadius: "var(--radius-1)",
                color: "var(--color-text-primary)",
                transition: "all 140ms var(--ease-out)",
              }}
              className="chamfer-sm"
            >
              Refresh
            </button>
            <button
              type="button"
              onClick={trigger}
              disabled={busy || running}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px 12px",
                fontSize: "12.5px",
                cursor: (busy || running) ? "not-allowed" : "pointer",
                border: "1px solid var(--color-border-hair)",
                background: "var(--color-bg-elevated)",
                borderRadius: "var(--radius-1)",
                color: "var(--color-text-primary)",
                transition: "all 140ms var(--ease-out)",
              }}
              className="chamfer-sm"
            >
              {running ? "Running..." : "Run scan"}
            </button>
            <button
              type="button"
              onClick={save}
              disabled={busy || running}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px 12px",
                fontSize: "12.5px",
                cursor: (busy || running) ? "not-allowed" : "pointer",
                border: "1px solid var(--color-accent)",
                background: "var(--color-bg-panel)",
                borderRadius: "var(--radius-1)",
                color: "var(--color-accent-strong)",
                transition: "all 140ms var(--ease-out)",
              }}
              className="chamfer-sm"
            >
              {busy ? "Saving..." : "Save & restart service"}
            </button>
          </Flex>
        </Flex>
      </Box>

      {/* Main Settings Card */}
      <Box
        className="bracket chamfer"
        style={{
          background: "var(--color-bg-panel)",
          border: "1px solid var(--color-border-hair)",
          display: "flex",
          flexDirection: "column",
        }}
      >
        {/* Welcome Message Section */}
        <Box
          p="4"
          style={{
            borderBottom: "1px solid var(--color-border-hair)",
            display: "flex",
            flexDirection: "column",
            gap: "16px",
          }}
        >
          <Flex justify="between" align="center" gap="3" wrap="wrap">
            <Text size="2" weight="bold" className="font-display" style={{ color: "var(--color-text-primary)" }}>
              Welcome Message
            </Text>
            <Flex gap="2" align="center" wrap="wrap">
              {whisperResult ? (
                <Badge color={whisperResult.ok ? "green" : "red"}>
                  {whisperResult.ok ? "sent" : "failed"}
                </Badge>
              ) : null}
              <button
                type="button"
                onClick={() => {
                  setTestMessage(welcomeMessage);
                  setTestOpen(true);
                }}
                disabled={busy}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: "6px",
                  padding: "4px 10px",
                  fontSize: "12px",
                  cursor: "pointer",
                  border: "1px solid var(--color-border-hair)",
                  background: "var(--color-bg-elevated)",
                  borderRadius: "var(--radius-1)",
                  color: "var(--color-text-secondary)",
                }}
                className="chamfer-sm"
              >
                <PaperPlaneIcon />
                Test
              </button>
            </Flex>
          </Flex>

          <Flex align="center" gap="2">
            <Checkbox
              checked={messageEnabled}
              onCheckedChange={(checked) => setMessageEnabled(Boolean(checked))}
            />
            <Text size="2">Enabled</Text>
          </Flex>

          <Flex gap="3" align="end" wrap="wrap">
            <Box style={{ flex: "1 1 280px", minWidth: 240 }}>
              <Text size="1" color="gray" style={{ display: "block", marginBottom: "4px" }}>
                Sender identity
              </Text>
              <PlayerCombobox
                tunnelId={tunnelId}
                value={whisperSourcePlayer}
                onChange={setWhisperSourcePlayer}
              />
            </Box>
          </Flex>

          <Box>
            <Text size="1" color="gray" style={{ display: "block", marginBottom: "4px" }}>
              Message
            </Text>
            <TextArea
              value={welcomeMessage}
              onChange={(e) => setWelcomeMessage(e.target.value)}
              rows={3}
              maxLength={1000}
              placeholder="Welcome to BadWolf. Your starter kit has been delivered..."
            />
          </Box>
        </Box>

        {/* Welcome Package Section */}
        <Box p="4" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <Flex direction="column" gap="2">
            <Text size="2" weight="bold" className="font-display" style={{ color: "var(--color-text-primary)" }}>
              Welcome Package
            </Text>
            <Flex align="center" gap="2">
              <Checkbox checked={enabled} onCheckedChange={(checked) => setEnabled(Boolean(checked))} />
              <Text size="2">Enabled</Text>
            </Flex>
          </Flex>

          <Box mt="1">
            <Flex justify="between" align="center" gap="3" wrap="wrap" mb={contentsOpen ? "3" : "0"}>
              <button
                type="button"
                onClick={() => setContentsOpen((open) => !open)}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: "6px",
                  background: "transparent",
                  border: "none",
                  cursor: "pointer",
                  color: "var(--color-text-primary)",
                  padding: 0,
                }}
              >
                {contentsOpen ? <ChevronDownIcon /> : <ChevronRightIcon />}
                <Text size="2" weight="medium" style={{ marginRight: "4px" }}>Package contents</Text>
                <Badge color="gray">{actions.length} item{actions.length === 1 ? "" : "s"}</Badge>
              </button>
              {contentsOpen ? (
                <Flex gap="2" wrap="wrap" align="center">
                  <Text as="label" size="1" color="gray">
                    <Flex align="center" gap="1">
                      <Checkbox
                        checked={jsonMode}
                        onCheckedChange={(checked) => {
                          const next = checked === true;
                          if (next) {
                            setJsonText(JSON.stringify(actions, null, 2));
                            setJsonError(null);
                            setJsonMode(true);
                          } else {
                            try {
                              const parsed = parseActions(jsonText);
                              validateActions(parsed);
                              setActions(parsed);
                              setJsonError(null);
                              setJsonMode(false);
                            } catch (err) {
                              setJsonError(String(err));
                            }
                          }
                        }}
                      />
                      JSON mode
                    </Flex>
                  </Text>
                </Flex>
              ) : null}
            </Flex>

            {contentsOpen && jsonMode ? (
              <Flex direction="column" gap="2" mt="2">
                <TextArea
                  value={jsonText}
                  onChange={(e) => {
                    setJsonText(e.target.value);
                    if (jsonError) setJsonError(null);
                  }}
                  placeholder='[{"type":"grantItem","itemName":"PlantFiber","quantity":1}]'
                  rows={12}
                  style={{ fontFamily: "var(--font-mono)", fontSize: "11.5px" }}
                />
                {jsonError ? (
                  <Text size="1" color="red">{jsonError}</Text>
                ) : (
                  <Text size="1" color="gray">
                    Raw JSON of package contents. Saved after validation. Toggle JSON mode off to switch back to the visual editor.
                  </Text>
                )}
              </Flex>
            ) : null}

            {contentsOpen && !jsonMode ? (
              <Flex direction="column" gap="3" mt="2">
                {actions.length === 0 ? (
                  <Text size="2" color="gray">No items configured.</Text>
                ) : (
                  <Box
                    style={{
                      border: "1px solid var(--color-border-hair)",
                      borderRadius: "var(--radius-2)",
                      overflow: "hidden",
                    }}
                  >
                    <Table.Root variant="surface" size="1">
                      <Table.Header>
                        <Table.Row style={{ backgroundColor: "var(--color-bg-elevated)" }}>
                          <Table.ColumnHeaderCell style={{ color: "var(--color-text-muted)" }}>Item</Table.ColumnHeaderCell>
                          <Table.ColumnHeaderCell width="120px" style={{ color: "var(--color-text-muted)" }}>Qty</Table.ColumnHeaderCell>
                          <Table.ColumnHeaderCell width="44px"></Table.ColumnHeaderCell>
                        </Table.Row>
                      </Table.Header>
                      <Table.Body>
                        {actions.map((action, index) => (
                          <ActionRow
                            key={`${index}:${action.itemName}`}
                            tunnelId={tunnelId}
                            action={action}
                            onChange={(next) =>
                              setActions((prev) => prev.map((row, i) => (i === index ? next : row)))
                            }
                            onRemove={() => setActions((prev) => prev.filter((_, i) => i !== index))}
                          />
                        ))}
                      </Table.Body>
                    </Table.Root>
                  </Box>
                )}
                <Box>
                  <button
                    type="button"
                    onClick={() =>
                      setActions((prev) => [
                        ...prev,
                        { type: "grantItem", itemName: "", quantity: 1 },
                      ])
                    }
                    style={{
                      display: "inline-flex",
                      alignItems: "center",
                      gap: "6px",
                      padding: "6px 12px",
                      fontSize: "12px",
                      cursor: "pointer",
                      border: "1px solid var(--color-border-hair)",
                      background: "var(--color-bg-elevated)",
                      borderRadius: "var(--radius-1)",
                      color: "var(--color-text-secondary)",
                    }}
                    className="chamfer-sm"
                  >
                    <PlusIcon />
                    Add item
                  </button>
                </Box>
              </Flex>
            ) : null}
          </Box>
        </Box>
      </Box>

      {restartRequired ? (
        <Text size="1" color="amber" as="div" mt="1">
          Saved values differ from the running service; save/restart applies the current package.
        </Text>
      ) : null}
      {error ? (
        <Text size="1" color="red" as="div" mt="1">
          {error}
        </Text>
      ) : null}

      {/* Recent Grants Section */}
      <Box mt="4">
        <Text size="2" weight="bold" className="font-display" mb="2" style={{ display: "block", color: "var(--color-text-primary)" }}>
          Recent Grants
        </Text>
        <Box
          className="bracket chamfer"
          style={{
            background: "var(--color-bg-panel)",
            border: "1px solid var(--color-border-hair)",
            borderRadius: "var(--radius-3)",
            overflow: "hidden",
          }}
        >
          <Table.Root variant="surface" size="1">
            <Table.Header>
              <Table.Row style={{ backgroundColor: "var(--color-bg-elevated)" }}>
                <Table.ColumnHeaderCell style={{ color: "var(--color-text-muted)" }}>Status</Table.ColumnHeaderCell>
                <Table.ColumnHeaderCell style={{ color: "var(--color-text-muted)" }}>Player</Table.ColumnHeaderCell>
                <Table.ColumnHeaderCell style={{ color: "var(--color-text-muted)" }}>Updated</Table.ColumnHeaderCell>
                <Table.ColumnHeaderCell width="64px"></Table.ColumnHeaderCell>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {grants.length === 0 ? (
                <Table.Row>
                  <Table.Cell colSpan={4}>
                    <Text size="1" color="gray">No grants recorded yet.</Text>
                  </Table.Cell>
                </Table.Row>
              ) : (
                grants.map((grant) => {
                  const key = `${grant.playerId}:${grant.packageVersion}:${grant.accountId}`;
                  return (
                    <Table.Row key={key}>
                      <Table.Cell>
                        <Badge color={grant.status === "granted" ? "green" : grant.status === "failed" ? "red" : "amber"}>
                          {grant.status}
                        </Badge>
                        {grant.status === "failed" && grant.lastError ? (
                          <Text size="1" color="red" as="div" style={{ maxWidth: 320, marginTop: "4px" }}>
                            {grant.lastError}
                          </Text>
                        ) : null}
                      </Table.Cell>
                      <Table.Cell>
                        <Text size="1" className="mono">{grant.playerId}</Text>
                        {grant.characterName ? (
                          <Text size="1" color="gray" as="div">{grant.characterName}</Text>
                        ) : null}
                      </Table.Cell>
                      <Table.Cell className="mono">{formatDateTime(grant.updatedAt)}</Table.Cell>
                      <Table.Cell>
                        {grant.status === "failed" ? (
                          <Tooltip content="Clear the failed record so the next scan retries">
                            <button
                              type="button"
                              disabled={retryingKey === key}
                              onClick={() => void retryGrant(grant)}
                              style={{
                                display: "inline-flex",
                                alignItems: "center",
                                gap: "4px",
                                padding: "4px 8px",
                                fontSize: "11px",
                                cursor: "pointer",
                                border: "1px solid var(--color-border-hair)",
                                background: "var(--color-bg-elevated)",
                                borderRadius: "var(--radius-1)",
                                color: "var(--color-text-secondary)",
                              }}
                              className="chamfer-sm"
                              aria-label="Retry welcome package"
                            >
                              <ReloadIcon />
                              {retryingKey === key ? "Retrying..." : "Retry"}
                            </button>
                          </Tooltip>
                        ) : null}
                      </Table.Cell>
                    </Table.Row>
                  );
                })
              )}
            </Table.Body>
          </Table.Root>
        </Box>
      </Box>

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
              <button
                type="button"
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  padding: "6px 12px",
                  fontSize: "12px",
                  cursor: "pointer",
                  border: "1px solid var(--color-border-hair)",
                  background: "var(--color-bg-elevated)",
                  borderRadius: "var(--radius-1)",
                  color: "var(--color-text-secondary)",
                }}
                className="chamfer-sm"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              onClick={sendWhisper}
              disabled={busy || sendingWhisper}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                gap: "6px",
                padding: "6px 12px",
                fontSize: "12px",
                cursor: (busy || sendingWhisper) ? "not-allowed" : "pointer",
                border: "1px solid var(--color-accent)",
                background: "var(--color-bg-panel)",
                borderRadius: "var(--radius-1)",
                color: "var(--color-accent-strong)",
              }}
              className="chamfer-sm"
            >
              <PaperPlaneIcon />
              {sendingWhisper ? "Sending..." : "Send"}
            </button>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>
    </Box>
  );
}

function ActionRow({
  tunnelId,
  action,
  onChange,
  onRemove,
}: {
  tunnelId: string;
  action: WelcomeAction;
  onChange: (action: WelcomeAction) => void;
  onRemove: () => void;
}) {
  return (
    <Table.Row>
      <Table.Cell style={{ verticalAlign: "middle" }}>
        <ItemCombobox
          tunnelId={tunnelId}
          value={action.itemName}
          onChange={(itemName) => onChange({ ...action, itemName })}
        />
      </Table.Cell>
      <Table.Cell style={{ verticalAlign: "middle" }}>
        <TextField.Root
          type="number"
          min="1"
          value={String(action.quantity)}
          onChange={(e) => onChange({ ...action, quantity: Number(e.target.value) || 1 })}
        />
      </Table.Cell>
      <Table.Cell style={{ verticalAlign: "middle" }}>
        <Flex justify="center" align="center">
          <Tooltip content="Remove item">
            <button
              type="button"
              onClick={onRemove}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px",
                cursor: "pointer",
                border: "1px solid var(--color-border-hair)",
                background: "var(--color-bg-elevated)",
                color: "var(--color-err)",
                borderRadius: "var(--radius-1)",
              }}
              className="chamfer-sm"
              aria-label="Remove item"
            >
              <TrashIcon />
            </button>
          </Tooltip>
        </Flex>
      </Table.Cell>
    </Table.Row>
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
    }
  }
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
