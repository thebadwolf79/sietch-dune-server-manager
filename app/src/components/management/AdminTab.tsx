import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertDialog,
  Badge,
  Box,
  Button,
  Checkbox,
  Flex,
  Grid,
  Select,
  Table,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";
import { AlertTriangle, Check, Send, Terminal, Users2, Search } from "lucide-react";

import { managementApi } from "../../services/management";
import type {
  Category,
  CommandSpec,
  FieldSpec,
  HistoryDto,
  PublishResultDto,
} from "../../types/management";
import { formatTime } from "../../utils/formatting";
import Combobox from "./Combobox";

export type AdminTabPrefill = {
  commandId: string;
  values: Record<string, unknown>;
} | null;

export type AdminTabProps = {
  tunnelId: string;
  prefill?: AdminTabPrefill;
  onPrefillConsumed?: () => void;
};

const CATEGORY_LABEL: Record<Category, string> = {
  items: "Inventory",
  player: "Player ops",
  progression: "Progression",
  movement: "Teleport & spawn",
  broadcast: "Broadcast",
  journey: "Story journey",
  exec: "Server scripts",
};

const CATEGORY_ORDER: Category[] = [
  "broadcast",
  "items",
  "player",
  "progression",
  "movement",
  "journey",
  "exec",
];

const CLIENT_DEFAULTS: Record<string, unknown> = {
  Quantity: 1,
  Durability: 1.0,
  WaterAmount: 1_000_000,
  Experience: 1000,
  Level: 1,
  SkillPoints: 0,
  BroadcastType: "Generic",
  BroadcastDuration: 30,
  Persistent: 1.0,
};

function applyDefaults(spec: CommandSpec): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const field of spec.fields) {
    if (CLIENT_DEFAULTS[field.key] !== undefined) {
      out[field.key] = CLIENT_DEFAULTS[field.key];
    } else if (field.default !== undefined && field.default !== null) {
      out[field.key] = field.default;
    }
  }
  return out;
}

function StepBadge({ n, label, done }: { n: number; label: string; done: boolean }) {
  return (
    <Flex align="center" gap="2">
      <span
        style={{
          display: "flex",
          width: "20px",
          height: "20px",
          alignItems: "center",
          justifyContent: "center",
          borderRadius: "50%",
          border: "1px solid",
          fontSize: "11px",
          fontWeight: "600",
          borderColor: done ? "var(--color-accent)" : "var(--color-border-default)",
          backgroundColor: done ? "rgba(217, 119, 87, 0.15)" : "transparent",
          color: done ? "var(--color-accent-strong)" : "var(--color-text-muted)",
        }}
      >
        {done ? <Check size={12} /> : n}
      </span>
      <Text
        size="1"
        weight="medium"
        style={{
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          color: done ? "var(--color-text-primary)" : "var(--color-text-muted)",
        }}
      >
        {label}
      </Text>
    </Flex>
  );
}

function CommandPreview({
  cmd,
  values,
}: {
  cmd: CommandSpec;
  values: Record<string, unknown>;
}) {
  const parts: string[] = [];

  // If player-id is present, let's show it first
  if (cmd.needsPlayer) {
    const pId = values.PlayerId;
    if (pId) {
      parts.push(`--target="${pId}"`);
    } else {
      parts.push(`--target=<pending>`);
    }
  }

  // Add other fields
  const fields = cmd.fields.filter((f) => f.key !== "PlayerId");
  for (const f of fields) {
    const val = values[f.key];
    if (val !== undefined && val !== null && val !== "") {
      parts.push(`--${f.key}=${typeof val === "string" ? `"${val}"` : val}`);
    }
  }

  return (
    <Box
      p="3"
      style={{
        borderRadius: "var(--radius-2)",
        border: "1px solid var(--color-border-hair)",
        backgroundColor: "rgba(0, 0, 0, 0.2)",
      }}
    >
      <Flex align="center" gap="2" mb="1.5" style={{ opacity: 0.7 }}>
        <Terminal size={12} />
        <Text
          size="1"
          weight="medium"
          style={{ textTransform: "uppercase", letterSpacing: "0.08em" }}
        >
          Command preview
        </Text>
      </Flex>
      <code
        className="mono"
        style={{
          fontSize: "11px",
          wordBreak: "break-all",
          whiteSpace: "pre-wrap",
          color: "var(--color-text-primary)",
        }}
      >
        <span style={{ color: "var(--color-accent-strong)" }}>publish</span> {cmd.id}{" "}
        {parts.map((p, i) => (
          <span
            key={i}
            style={{
              color: p.includes("<pending>") ? "var(--color-text-muted)" : "var(--color-text-secondary)",
            }}
          >
            {p}{" "}
          </span>
        ))}
      </code>
    </Box>
  );
}

export default function AdminTab({ tunnelId, prefill, onPrefillConsumed }: AdminTabProps) {
  const [commands, setCommands] = useState<CommandSpec[]>([]);
  const [selected, setSelected] = useState<CommandSpec | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [history, setHistory] = useState<HistoryDto[]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<PublishResultDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [query, setQuery] = useState("");
  const appliedRef = useRef<{ selectedId: string; prefillFp: string | null } | null>(null);
  const [vehicleTemplates, setVehicleTemplates] = useState<string[]>([]);

  const refreshHistory = useCallback(async () => {
    try {
      const list = await managementApi.history(tunnelId, 30);
      setHistory(list);
    } catch (err) {
      setError(String(err));
    }
  }, [tunnelId]);

  useEffect(() => {
    managementApi
      .listCommands(tunnelId)
      .then(setCommands)
      .catch((err) => setError(String(err)));
    void refreshHistory();
  }, [tunnelId, refreshHistory]);

  useEffect(() => {
    if (!selected) {
      appliedRef.current = null;
      return;
    }
    const prefillFp =
      prefill && prefill.commandId === selected.id ? JSON.stringify(prefill) : null;
    const current = appliedRef.current;

    if (!current || current.selectedId !== selected.id) {
      if (prefillFp) {
        setValues({ ...applyDefaults(selected), ...(prefill?.values ?? {}) });
        onPrefillConsumed?.();
      } else {
        setValues(applyDefaults(selected));
      }
      setResult(null);
      appliedRef.current = { selectedId: selected.id, prefillFp };
      return;
    }

    if (prefillFp && prefillFp !== current.prefillFp) {
      setValues((prev) => ({ ...prev, ...(prefill?.values ?? {}) }));
      setResult(null);
      onPrefillConsumed?.();
      appliedRef.current = { selectedId: selected.id, prefillFp };
    }
  }, [selected, prefill, onPrefillConsumed]);

  useEffect(() => {
    if (!prefill || commands.length === 0) return;
    if (selected?.id === prefill.commandId) return;
    const target = commands.find((c) => c.id === prefill.commandId);
    if (!target) return;
    setSelected(target);
  }, [prefill, commands, selected?.id]);

  useEffect(() => {
    const cls =
      selected?.id === "SpawnVehicleAt" && typeof values.ClassName === "string"
        ? (values.ClassName as string).trim()
        : "";
    if (!cls) {
      setVehicleTemplates([]);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const matches = await managementApi.searchVehicles(tunnelId, cls, 10);
        const hit = matches.find((v) => v.id === cls || v.actor_class === cls);
        const templates = hit?.templates ?? [];
        if (cancelled) return;
        setVehicleTemplates(templates);
        if (templates.length > 0) {
          setValues((prev) => {
            const current = typeof prev.TemplateName === "string" ? prev.TemplateName : "";
            if (current && templates.includes(current)) return prev;
            return { ...prev, TemplateName: templates[0] };
          });
        }
      } catch {
        if (!cancelled) setVehicleTemplates([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selected?.id, values.ClassName, tunnelId]);

  useEffect(() => {
    setConfirm(false);
  }, [selected?.id]);

  const doPublish = useCallback(async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const out = await managementApi.publish(tunnelId, selected.id, values);
      setResult(out);
      await refreshHistory();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [selected, tunnelId, values, refreshHistory]);

  const publish = useCallback(() => {
    if (!selected) return;
    void doPublish();
  }, [selected, doPublish]);

  const queryLower = query.trim().toLowerCase();
  const filteredCommands = useMemo(() => {
    if (!queryLower) return commands;
    return commands.filter(
      (cmd) =>
        cmd.label.toLowerCase().includes(queryLower) ||
        (CATEGORY_LABEL[cmd.category] || cmd.category).toLowerCase().includes(queryLower) ||
        cmd.describe.toLowerCase().includes(queryLower),
    );
  }, [commands, queryLower]);

  const grouped = useMemo(() => groupByCategory(filteredCommands), [filteredCommands]);

  const fieldsToRender = useMemo(() => {
    if (!selected) return [];
    return visibleFields(selected, values).filter((f) => f.key !== "PlayerId");
  }, [selected, values]);

  const requiredFields = useMemo(() => {
    return fieldsToRender.filter((f) => f.required);
  }, [fieldsToRender]);

  const requiredFilled = useMemo(() => {
    return requiredFields.every((f) => {
      const val = values[f.key];
      return val !== undefined && val !== null && String(val).trim() !== "";
    });
  }, [requiredFields, values]);

  const targetReady = useMemo(() => {
    if (!selected) return false;
    return (
      !selected.needsPlayer ||
      (values.PlayerId !== undefined &&
        values.PlayerId !== null &&
        String(values.PlayerId).trim() !== "")
    );
  }, [selected, values.PlayerId]);

  const allPlayers = values.PlayerId === "*";

  const toggleAllPlayers = useCallback(() => {
    setValues((prev) => ({
      ...prev,
      PlayerId: prev.PlayerId === "*" ? "" : "*",
    }));
  }, []);

  const canPublish = useMemo(() => {
    return targetReady && requiredFilled && (!selected?.destructive || confirm);
  }, [targetReady, requiredFilled, selected?.destructive, confirm]);

  return (
    <Grid mt="3" gap="5" columns={{ initial: "1", lg: "280px 1fr" }} align="start">
      {/* Command Catalog Panel */}
      <Box
        className="bracket chamfer"
        style={{
          background: "var(--color-bg-panel)",
          border: "1px solid var(--color-border-hair)",
          padding: "16px",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
        }}
      >
        <div style={{ borderBottom: "1px solid var(--color-border-hair)", paddingBottom: "10px" }}>
          <Text
            size="2"
            weight="bold"
            style={{
              fontFamily: "var(--font-mono)",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
            }}
          >
            Command catalog
          </Text>
          <Box mt="2">
            <TextField.Root
              placeholder="Search commands…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              size="2"
            >
              <TextField.Slot>
                <Search size={14} style={{ opacity: 0.6 }} />
              </TextField.Slot>
            </TextField.Root>
          </Box>
        </div>

        <Box
          style={{
            maxHeight: "70vh",
            overflowY: "auto",
            display: "flex",
            flexDirection: "column",
            gap: "16px",
          }}
        >
          {CATEGORY_ORDER.map((cat) => {
            const specs = grouped[cat];
            if (!specs || specs.length === 0) return null;
            return (
              <Box key={cat}>
                <Text
                  size="1"
                  color="gray"
                  style={{
                    fontFamily: "var(--font-mono)",
                    textTransform: "uppercase",
                    letterSpacing: "0.08em",
                    fontSize: "10px",
                    display: "block",
                    marginBottom: "6px",
                  }}
                >
                  {CATEGORY_LABEL[cat] ?? cat}
                </Text>
                <Flex direction="column" gap="2">
                  {specs.map((spec) => (
                    <button
                      key={spec.id}
                      type="button"
                      onClick={() => setSelected(spec)}
                      style={{
                        display: "flex",
                        width: "100%",
                        alignItems: "center",
                        justifyContent: "space-between",
                        gap: "8px",
                        padding: "6px 10px",
                        borderRadius: "var(--radius-2)",
                        border: "1px solid",
                        fontSize: "12px",
                        textAlign: "left",
                        cursor: "pointer",
                        transition: "all 140ms var(--ease-out)",
                        borderColor:
                          selected?.id === spec.id
                            ? "var(--color-accent-strong)"
                            : spec.destructive
                              ? "rgba(214, 105, 94, 0.3)"
                              : "var(--color-border-hair)",
                        backgroundColor:
                          selected?.id === spec.id
                            ? "rgba(217, 119, 87, 0.15)"
                            : "var(--color-bg-panel)",
                        color:
                          selected?.id === spec.id
                            ? "var(--color-accent-strong)"
                            : spec.destructive
                              ? "var(--color-err)"
                              : "var(--color-text-primary)",
                      }}
                      className="chamfer-sm"
                    >
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {spec.label}
                      </span>
                      {spec.destructive && (
                        <AlertTriangle size={12} style={{ flexShrink: 0, opacity: 0.7 }} />
                      )}
                    </button>
                  ))}
                </Flex>
              </Box>
            );
          })}
          {filteredCommands.length === 0 && (
            <Text size="2" color="gray" style={{ textAlign: "center", padding: "16px 0" }}>
              No commands match.
            </Text>
          )}
        </Box>
      </Box>

      {/* Form + Recent Publishes stack */}
      <Flex direction="column" gap="5" style={{ minWidth: 0 }}>
        {selected ? (
          <Box
            className="bracket chamfer"
            style={{
              background: "var(--color-bg-panel)",
              border: "1px solid var(--color-border-hair)",
              padding: "20px",
              display: "flex",
              flexDirection: "column",
              gap: "16px",
            }}
          >
            {/* Header */}
            <div style={{ borderBottom: "1px solid var(--color-border-hair)", paddingBottom: "12px" }}>
              <Flex align="center" gap="2" wrap="wrap">
                <Text size="4" weight="bold">
                  {selected.label}
                </Text>
                <Badge color="gray" size="1">
                  {CATEGORY_LABEL[selected.category] ?? selected.category}
                </Badge>
                {selected.destructive && (
                  <Badge color="red" size="1">
                    destructive
                  </Badge>
                )}
              </Flex>
              <Text size="2" color="gray" as="div" mt="1">
                {selected.describe}
              </Text>

              {/* Step Rail */}
              <Flex gap="4" mt="3" align="center" wrap="wrap">
                <StepBadge n={1} label="Command" done={true} />
                {selected.needsPlayer && (
                  <StepBadge n={2} label="Target" done={targetReady} />
                )}
                <StepBadge n={selected.needsPlayer ? 3 : 2} label="Parameters" done={requiredFilled} />
              </Flex>
            </div>

            {/* Step 2: Target Player */}
            {selected.needsPlayer && (
              <Box style={{ borderBottom: "1px solid var(--color-border-hair)", paddingBottom: "16px" }}>
                <Flex justify="between" align="center" mb="2">
                  <Text
                    size="2"
                    weight="bold"
                    style={{
                      fontFamily: "var(--font-mono)",
                      textTransform: "uppercase",
                      letterSpacing: "0.05em",
                    }}
                  >
                    Target player
                  </Text>
                  {selected.allowAllPlayers && (
                    <button
                      type="button"
                      onClick={toggleAllPlayers}
                      style={{
                        display: "inline-flex",
                        alignItems: "center",
                        gap: "6px",
                        padding: "4px 8px",
                        fontSize: "11px",
                        fontFamily: "var(--font-sans)",
                        fontWeight: "500",
                        textTransform: "uppercase",
                        letterSpacing: "0.04em",
                        cursor: "pointer",
                        border: "1px solid",
                        borderRadius: "var(--radius-1)",
                        borderColor: allPlayers ? "var(--color-accent)" : "var(--color-border-default)",
                        backgroundColor: allPlayers ? "rgba(217, 119, 87, 0.15)" : "var(--color-bg-elevated)",
                        color: allPlayers ? "var(--color-accent-strong)" : "var(--color-text-secondary)",
                        transition: "all 140ms var(--ease-out)",
                      }}
                      className="chamfer-sm"
                    >
                      <Users2 size={12} />
                      All players
                    </button>
                  )}
                </Flex>

                {allPlayers ? (
                  <Box
                    p="2"
                    style={{
                      border: "1px dashed var(--color-accent)",
                      backgroundColor: "rgba(217, 119, 87, 0.05)",
                      borderRadius: "var(--radius-2)",
                      display: "flex",
                      alignItems: "center",
                      gap: "8px",
                    }}
                  >
                    <Check size={14} style={{ color: "var(--color-accent-strong)" }} />
                    <Text size="2" weight="medium" style={{ color: "var(--color-accent-strong)" }}>
                      Target: All online players (*)
                    </Text>
                  </Box>
                ) : (
                  <CommandCombobox
                    kind="players"
                    value={values.PlayerId}
                    onPick={(val) => setValues((prev) => ({ ...prev, PlayerId: val }))}
                    tunnelId={tunnelId}
                  />
                )}
              </Box>
            )}

            {/* Step 3: Parameters */}
            {fieldsToRender.length > 0 && (
              <Box style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <Text
                  size="2"
                  weight="bold"
                  style={{
                    fontFamily: "var(--font-mono)",
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                  }}
                >
                  Parameters
                </Text>
                <Grid columns={{ initial: "1", sm: "2" }} gap="3">
                  {fieldsToRender.map((field) => (
                    <Box
                      key={field.key}
                      style={{
                        gridColumn: field.kind === "text" ? "1 / -1" : undefined,
                      }}
                    >
                      <FieldInput
                        field={field}
                        value={values[field.key]}
                        onChange={(v) => setValues((prev) => ({ ...prev, [field.key]: v }))}
                        tunnelId={tunnelId}
                        vehicleTemplates={vehicleTemplates}
                      />
                    </Box>
                  ))}
                </Grid>
              </Box>
            )}

            {/* SpawnVehicleAt Player Position Button */}
            {selected.id === "SpawnVehicleAt" && (
              <UsePlayerPositionButton
                tunnelId={tunnelId}
                playerId={values.PlayerId as string | undefined}
                onLocation={(loc) =>
                  setValues((prev) => ({ ...prev, X: loc.x, Y: loc.y, Z: loc.z }))
                }
              />
            )}

            {/* Live Command Preview */}
            <CommandPreview cmd={selected} values={values} />

            {/* Destructive Confirm + Publish */}
            <Flex
              direction="column"
              gap="3"
              style={{ borderTop: "1px solid var(--color-border-hair)", paddingTop: "16px" }}
            >
              {selected.destructive && (
                <label
                  style={{
                    display: "flex",
                    alignItems: "flex-start",
                    gap: "8px",
                    cursor: "pointer",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={confirm}
                    onChange={(e) => setConfirm(e.target.checked)}
                    style={{
                      marginTop: "3px",
                      width: "16px",
                      height: "16px",
                      borderRadius: "var(--radius-1)",
                      accentColor: "var(--color-err)",
                    }}
                  />
                  <Flex align="center" gap="1" style={{ color: "var(--color-err)" }}>
                    <AlertTriangle size={14} style={{ flexShrink: 0 }} />
                    <Text size="2" weight="medium">
                      I understand this action is irreversible and cannot be undone.
                    </Text>
                  </Flex>
                </label>
              )}

              <Flex justify="between" align="center" gap="3">
                <Text size="1" color="gray">
                  {canPublish ? "Ready to publish." : "Complete required steps to publish."}
                </Text>
                <button
                  type="button"
                  onClick={publish}
                  disabled={!canPublish || busy}
                  className="action-btn"
                  data-tone={selected.destructive ? "danger" : "accent"}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "8px",
                    padding: "8px 16px",
                  }}
                >
                  {busy ? (
                    <>
                      <span className="inline-spinner" />
                      Publishing…
                    </>
                  ) : (
                    <>
                      <Send size={14} />
                      Publish command
                    </>
                  )}
                </button>
              </Flex>
            </Flex>

            {/* Publish Results & Outputs */}
            {result && !result.ok && result.error && (
              <Text size="2" color="red" mt="2" style={{ display: "block" }}>
                {result.error}
              </Text>
            )}
            {result?.output && (
              <Box
                mt="2"
                className="mono"
                style={{
                  fontSize: 11,
                  padding: 8,
                  background: "rgba(0, 0, 0, 0.3)",
                  border: "1px solid var(--color-border-hair)",
                  borderRadius: "var(--radius-2)",
                  whiteSpace: "pre-wrap",
                  maxHeight: "240px",
                  overflowY: "auto",
                }}
              >
                {result.output}
              </Box>
            )}
            {error && (
              <Text size="2" color="red" mt="2" style={{ display: "block" }}>
                {error}
              </Text>
            )}
          </Box>
        ) : (
          <Box
            className="bracket chamfer"
            style={{
              background: "var(--color-bg-panel)",
              border: "1px solid var(--color-border-hair)",
              borderRadius: "var(--radius-3)",
              padding: "48px 16px",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              textAlign: "center",
              gap: "8px",
            }}
          >
            <Terminal size={32} style={{ opacity: 0.4 }} />
            <Text size="3" weight="bold">
              No command selected
            </Text>
            <Text size="2" color="gray" style={{ maxWidth: "360px" }}>
              Select a command from the catalog on the left to configure and publish it. The publish
              command string assembles live.
            </Text>
          </Box>
        )}

        {/* Recent Publishes Table */}
        <Box>
          <Text
            size="2"
            weight="bold"
            style={{
              display: "block",
              marginBottom: "8px",
              fontFamily: "var(--font-mono)",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
            }}
          >
            Recent publishes
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
            <Table.Root variant="ghost" size="1">
              <Table.Header style={{ backgroundColor: "var(--color-bg-elevated)" }}>
                <Table.Row style={{ borderBottom: "1px solid var(--color-border-hair)" }}>
                  <Table.ColumnHeaderCell
                    style={{
                      padding: "8px 12px",
                      fontFamily: "var(--font-mono)",
                      textTransform: "uppercase",
                      letterSpacing: "0.04em",
                      fontSize: "10px",
                    }}
                  >
                    Cmd
                  </Table.ColumnHeaderCell>
                  <Table.ColumnHeaderCell
                    style={{
                      padding: "8px 12px",
                      fontFamily: "var(--font-mono)",
                      textTransform: "uppercase",
                      letterSpacing: "0.04em",
                      fontSize: "10px",
                    }}
                  >
                    OK
                  </Table.ColumnHeaderCell>
                  <Table.ColumnHeaderCell
                    style={{
                      padding: "8px 12px",
                      fontFamily: "var(--font-mono)",
                      textTransform: "uppercase",
                      letterSpacing: "0.04em",
                      fontSize: "10px",
                      textAlign: "right",
                    }}
                  >
                    When
                  </Table.ColumnHeaderCell>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {history.map((h) => (
                  <Table.Row key={h.id} style={{ borderBottom: "1px solid var(--color-border-hair)" }}>
                    <Table.Cell
                      className="mono"
                      style={{ padding: "8px 12px", fontSize: 11, verticalAlign: "middle" }}
                    >
                      {h.command}
                    </Table.Cell>
                    <Table.Cell style={{ padding: "8px 12px", verticalAlign: "middle" }}>
                      <Badge color={h.ok ? "green" : "red"}>{h.ok ? "ok" : "fail"}</Badge>
                    </Table.Cell>
                    <Table.Cell
                      className="mono"
                      style={{
                        padding: "8px 12px",
                        fontSize: 11,
                        textAlign: "right",
                        color: "var(--color-text-muted)",
                        verticalAlign: "middle",
                      }}
                    >
                      {formatTime(h.createdAt)}
                    </Table.Cell>
                  </Table.Row>
                ))}
                {history.length === 0 && (
                  <Table.Row>
                    <Table.Cell
                      colSpan={3}
                      style={{ textAlign: "center", padding: "16px 0", color: "var(--color-text-muted)" }}
                    >
                      No commands published yet.
                    </Table.Cell>
                  </Table.Row>
                )}
              </Table.Body>
            </Table.Root>
          </Box>
        </Box>
      </Flex>
    </Grid>
  );
}

function groupByCategory(specs: CommandSpec[]): Record<string, CommandSpec[]> {
  const out: Record<string, CommandSpec[]> = {};
  for (const spec of specs) {
    if (!out[spec.category]) out[spec.category] = [];
    out[spec.category].push(spec);
  }
  return out;
}

function compareText(a: string | undefined | null, b: string | undefined | null): number {
  return (a || "").localeCompare(b || "", undefined, { sensitivity: "base", numeric: true });
}

function comparePlayers(a: any, b: any): number {
  const aOnline = String(a.online || "").toLowerCase() === "online";
  const bOnline = String(b.online || "").toLowerCase() === "online";
  if (aOnline !== bOnline) return aOnline ? -1 : 1;
  return compareText(a.name || a.flsId, b.name || b.flsId);
}

function sortCommandOptions(kind: ComboboxKind, options: any[]): any[] {
  const rows = [...options];
  if (kind === "players") return rows.sort(comparePlayers);
  if (kind === "vehicles") return rows.sort((a, b) => compareText(a.id, b.id));
  return rows.sort((a, b) => compareText(a.name || a.id, b.name || b.id));
}

function UsePlayerPositionButton({
  tunnelId,
  playerId,
  onLocation,
}: {
  tunnelId: string;
  playerId: string | undefined;
  onLocation: (loc: { x: number; y: number; z: number }) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const enabled = !!playerId && !busy;

  const click = useCallback(async () => {
    if (!playerId) return;
    setBusy(true);
    setError(null);
    try {
      const loc = await managementApi.playerLocation(tunnelId, playerId);
      onLocation(loc);
    } catch (err) {
      // Backend wraps proxy errors as `GET /path -> STATUS: {"error":"…"}`.
      // Pull out the inner `error` field for a readable message; fall back to raw.
      const raw = String(err);
      let nice = raw;
      const bodyStart = raw.indexOf("{");
      if (bodyStart >= 0) {
        try {
          const obj = JSON.parse(raw.slice(bodyStart));
          if (obj && typeof obj.error === "string") nice = obj.error;
        } catch {
          // leave nice as raw
        }
      }
      setError(nice);
    } finally {
      setBusy(false);
    }
  }, [tunnelId, playerId, onLocation]);

  return (
    <Box mt="2">
      <Button size="1" variant="soft" disabled={!enabled} onClick={click}>
        {busy ? "Fetching…" : "Use player's current position"}
      </Button>
      {!playerId ? (
        <Text size="1" color="gray" ml="2">
          (pick a player first)
        </Text>
      ) : null}
      {error ? (
        <Text size="1" color="red" as="div" mt="1">
          {error}
        </Text>
      ) : null}
    </Box>
  );
}

type ComboboxKind = "items" | "vehicles" | "players" | "skill-modules";

function comboboxKindFor(fieldKey: string): ComboboxKind | null {
  switch (fieldKey) {
    case "ItemName":
      return "items";
    case "ClassName":
      return "vehicles";
    case "PlayerId":
      return "players";
    case "Module":
      return "skill-modules";
    default:
      return null;
  }
}

function FieldInput({
  field,
  value,
  onChange,
  tunnelId,
  vehicleTemplates,
}: {
  field: FieldSpec;
  value: unknown;
  onChange: (v: unknown) => void;
  tunnelId: string;
  vehicleTemplates: string[];
}) {
  const comboKind = comboboxKindFor(field.key);
  const templateMode = field.key === "TemplateName" && vehicleTemplates.length > 0;
  return (
    <Box>
      <Flex justify="between" align="baseline" gap="2">
        <Text size="2" weight="medium">
          {field.label}
          {field.required ? " *" : ""}
        </Text>
        {field.helper ? (
          <Text size="1" color="gray">
            {field.helper}
          </Text>
        ) : null}
      </Flex>
      <Box mt="1">
        {templateMode ? (
          <TemplateCombobox
            value={typeof value === "string" ? value : value == null ? "" : String(value)}
            onPick={onChange}
            templates={vehicleTemplates}
          />
        ) : comboKind ? (
          <CommandCombobox kind={comboKind} value={value} onPick={onChange} tunnelId={tunnelId} />
        ) : (
          renderInput(field, value, onChange)
        )}
      </Box>
    </Box>
  );
}

function TemplateCombobox({
  value,
  onPick,
  templates,
}: {
  value: string;
  onPick: (v: unknown) => void;
  templates: string[];
}) {
  const loadOptions = useCallback(
    async (query: string) => {
      const q = query.trim().toLowerCase();
      const filtered = q
        ? templates.filter((t) => t.toLowerCase().includes(q))
        : templates;
      return [...filtered].sort(compareText).map((name) => ({ name }));
    },
    [templates],
  );
  return (
    <Combobox
      value={value}
      onChange={onPick}
      loadOptions={loadOptions}
      getOptionValue={(o: { name: string }) => o.name}
      resolveLabel={async (id) => id}
      renderOption={(o: { name: string }) => (
        <Text size="2" className="mono">{o.name}</Text>
      )}
      placeholder="Pick a template…"
      searchPlaceholder="Filter templates…"
    />
  );
}

/// Filters the spec's field list down to what's relevant for the current
/// values. Today only ServiceBroadcast has conditional fields — Generic
/// hides the shutdown-specific knobs, ServerShutdown hides Generic-only
/// fields, and a `ShouldCancel=true` hides everything except the cancel
/// toggle itself.
function visibleFields(
  spec: CommandSpec,
  values: Record<string, unknown>,
): FieldSpec[] {
  if (spec.id !== "ServiceBroadcast") return [...spec.fields];
  const broadcastType = (values.BroadcastType as string) || "Generic";
  const shouldCancel = values.ShouldCancel === true;
  const GENERIC_ONLY = new Set(["Title", "Body"]);
  const SHUTDOWN_ONLY = new Set([
    "ShutdownType",
    "ShutdownDuration",
    "BroadcastFrequency",
    "ShouldCancel",
  ]);
  return spec.fields.filter((field) => {
    if (field.key === "BroadcastType") return true;
    if (broadcastType === "Generic") {
      if (SHUTDOWN_ONLY.has(field.key)) return false;
      return true;
    }
    // ServerShutdown branch
    if (GENERIC_ONLY.has(field.key)) return false;
    if (shouldCancel && field.key !== "ShouldCancel") return false;
    return true;
  });
}

function renderInput(field: FieldSpec, value: unknown, onChange: (v: unknown) => void) {
  const strValue = value === undefined || value === null ? "" : String(value);
  if (field.kind === "select" && field.options) {
    return (
      <Select.Root value={strValue || field.options[0].value} onValueChange={onChange}>
        <Select.Trigger />
        <Select.Content>
          {field.options.map((opt) => (
            <Select.Item key={opt.value} value={opt.value}>
              {opt.label}
            </Select.Item>
          ))}
        </Select.Content>
      </Select.Root>
    );
  }
  if (field.kind === "text") {
    return <TextArea value={strValue} onChange={(e) => onChange(e.target.value)} rows={3} />;
  }
  if (field.kind === "bool") {
    const checked = value === true || strValue === "true" || strValue === "1";
    return (
      <Checkbox checked={checked} onCheckedChange={(c) => onChange(Boolean(c))} />
    );
  }
  return (
    <TextField.Root
      value={strValue}
      onChange={(e) => {
        const raw = e.target.value;
        if (field.kind === "int" || field.kind === "float") {
          onChange(raw === "" ? "" : Number(raw));
        } else {
          onChange(raw);
        }
      }}
    />
  );
}

function CommandCombobox({
  kind,
  value,
  onPick,
  tunnelId,
}: {
  kind: ComboboxKind;
  value: unknown;
  onPick: (v: unknown) => void;
  tunnelId: string;
}) {
  const strVal = typeof value === "string" ? value : value == null ? "" : String(value);

  const loadOptions = useCallback(
    async (query: string) => {
      try {
        if (kind === "items") {
          return sortCommandOptions(kind, await managementApi.searchItems(tunnelId, query, 30));
        }
        if (kind === "vehicles") {
          return sortCommandOptions(kind, await managementApi.searchVehicles(tunnelId, query, 30));
        }
        if (kind === "skill-modules") {
          return sortCommandOptions(kind, await managementApi.searchSkillModules(tunnelId, query, 50));
        }
        return sortCommandOptions(kind, await managementApi.searchPlayers(tunnelId, query, 30));
      } catch {
        return [] as never[];
      }
    },
    [kind, tunnelId],
  );

  const resolveLabel = useCallback(
    async (id: string): Promise<string | null> => {
      if (!id) return null;
      try {
        if (kind === "items") {
          const r = await managementApi.searchItems(tunnelId, id, 5);
          const hit = r.find((it) => it.id === id);
          return hit ? `${hit.name}  ·  ${hit.id}` : id;
        }
        if (kind === "players") {
          const r = await managementApi.searchPlayers(tunnelId, id, 5);
          const hit = r.find((p) => p.flsId === id);
          return hit ? `${hit.name} (${hit.online})  ·  ${hit.flsId}` : id;
        }
        if (kind === "skill-modules") {
          const r = await managementApi.searchSkillModules(tunnelId, id, 5);
          const hit = r.find((m) => m.id === id);
          return hit ? `${hit.name}  ·  ${hit.id}` : id;
        }
        const r = await managementApi.searchVehicles(tunnelId, id, 5);
        const hit = r.find((v) => v.id === id || v.actor_class === id);
        if (!hit) return id;
        const templates = Array.isArray(hit.templates) && hit.templates.length > 0
          ? `  ·  templates: ${hit.templates.join(", ")}`
          : "";
        return `${hit.id}${templates}`;
      } catch {
        return id;
      }
    },
    [kind, tunnelId],
  );

  if (kind === "items") {
    return (
      <Combobox
        value={strVal}
        onChange={onPick}
        loadOptions={loadOptions}
        getOptionValue={(it: any) => it.id}
        resolveLabel={resolveLabel}
        renderOption={(it: any) => (
          <Flex justify="between" gap="2">
            <Text size="2">{it.name}</Text>
            <Text size="1" color="gray" className="mono">{it.id}</Text>
          </Flex>
        )}
        placeholder="Pick an item…"
        searchPlaceholder="Search items…"
      />
    );
  }
  if (kind === "vehicles") {
    return (
      <Combobox
        value={strVal}
        onChange={onPick}
        loadOptions={loadOptions}
        // Server expects the DT_VehicleTemplates row key (e.g. "Sandbike"),
        // not the full BP actor class path.
        getOptionValue={(v: any) => v.id}
        resolveLabel={resolveLabel}
        renderOption={(v: any) => (
          <Flex direction="column">
            <Text size="2">{v.id}</Text>
            <Text size="1" color="gray">
              templates: {Array.isArray(v.templates) && v.templates.length > 0 ? v.templates.join(", ") : "—"}
            </Text>
          </Flex>
        )}
        placeholder="Pick a vehicle…"
        searchPlaceholder="Search vehicles…"
      />
    );
  }
  if (kind === "skill-modules") {
    return (
      <Combobox
        value={strVal}
        onChange={onPick}
        loadOptions={loadOptions}
        getOptionValue={(m: any) => m.id}
        resolveLabel={resolveLabel}
        renderOption={(m: any) => (
          <Flex justify="between" gap="2">
            <Box>
              <Text size="2">{m.name}</Text>
              <Text size="1" color="gray" as="div">
                {m.category} · max {m.maxLevel}
              </Text>
            </Box>
            <Text size="1" color="gray" className="mono">{m.id}</Text>
          </Flex>
        )}
        placeholder="Pick a skill module…"
        searchPlaceholder="Search skill modules…"
      />
    );
  }
  return (
    <Combobox
      value={strVal}
      onChange={onPick}
      loadOptions={loadOptions}
      getOptionValue={(p: any) => p.flsId}
      resolveLabel={resolveLabel}
      renderOption={(p: any) => (
        <Flex justify="between" gap="2" align="center">
          <Box>
            <Text size="2">{p.name || "(unnamed)"}</Text>
            <Text size="1" color="gray" as="div" className="mono">{p.flsId}</Text>
          </Box>
          <Badge color={p.online === "online" ? "green" : "gray"}>{p.online || "offline"}</Badge>
        </Flex>
      )}
      placeholder="Pick a player…"
      searchPlaceholder="Search players…"
    />
  );
}
