import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertDialog,
  Badge,
  Box,
  Button,
  Checkbox,
  Flex,
  Select,
  Table,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";

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
  currency: "Currency",
  journey: "Story journey",
  exec: "Server scripts",
};

const CATEGORY_ORDER: Category[] = [
  "broadcast",
  "items",
  "currency",
  "player",
  "progression",
  "movement",
  "journey",
  "exec",
];

// Frontend-synthetic grant commands: dedicated, locked forms in the "currency"
// category. Two flavours:
//  - Solari rides a real engine command — the in-game `solari` item (SolarisCoin),
//    so it publishes AddItemToInventory with ItemName/Durability locked (no picker).
//  - House Scrip has no engine command; it's a guarded offline DB write
//    (dbAction "grant_currency", currencyId locked to 1) the player must be offline for.
//    (Intel will join Progression later as its own DB write.)
function withSyntheticGrants(list: CommandSpec[]): CommandSpec[] {
  const addItem = list.find((c) => c.id === "AddItemToInventory");
  if (!addItem) return list;
  const solari: CommandSpec = {
    ...addItem,
    id: "GrantSolari",
    label: "Grant Solari",
    category: "currency",
    describe:
      "Grant Solari to a player. Publishes the engine AddItemToInventory command with the Solari item locked in.",
    // Hide ItemName (locked to solari) and Durability (meaningless for currency);
    // both are injected at publish so the engine AddItemToInventory call stays valid.
    fields: addItem.fields.filter((f) => f.key !== "ItemName" && f.key !== "Durability"),
    publishAs: "AddItemToInventory",
    lockedFields: { ItemName: "solari", Durability: 1.0 },
  };
  // Reuse the engine command's PlayerId field so House Scrip gets the same player
  // picker (its value is the FLS id). Add a plain Amount field.
  const playerField = addItem.fields.find((f) => f.key === "PlayerId");
  const houseScrip: CommandSpec = {
    id: "GrantHouseScrip",
    label: "Grant House Scrip",
    category: "currency",
    needsPlayer: true,
    allowAllPlayers: false,
    describe:
      "Add House Scrip to a player's balance. Direct database write — the player must be offline (the server overwrites currency edits on logout). The amount is added to any existing balance.",
    fields: [
      ...(playerField ? [playerField] : []),
      {
        key: "Amount",
        label: "Amount",
        kind: "int",
        required: true,
        default: 1000,
        helper: "House Scrip to add to the current balance",
      },
    ],
    dbAction: "grant_currency",
    lockedFields: { currencyId: 1 },
  };
  // Intel ("Tech Knowledge points") is a single integer in the character actor's
  // properties blob — its own DB write, placed under Progression beside Award XP.
  const intel: CommandSpec = {
    id: "AwardIntel",
    label: "Award Intel",
    category: "progression",
    needsPlayer: true,
    allowAllPlayers: false,
    describe:
      "Add Intel (Tech Knowledge points) to a player's character. Direct database write — the player must be offline (the server overwrites edits on logout). The amount is added to the current total.",
    fields: [
      ...(playerField ? [playerField] : []),
      {
        key: "Amount",
        label: "Amount",
        kind: "int",
        required: true,
        default: 10,
        helper: "Tech Knowledge points to add to the current total",
      },
    ],
    dbAction: "award_intel",
  };
  return [...list, solari, houseScrip, intel];
}

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

export default function AdminTab({ tunnelId, prefill, onPrefillConsumed }: AdminTabProps) {
  const [commands, setCommands] = useState<CommandSpec[]>([]);
  const [selected, setSelected] = useState<CommandSpec | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [history, setHistory] = useState<HistoryDto[]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<PublishResultDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const appliedRef = useRef<{ selectedId: string; prefillFp: string | null } | null>(null);
  // Templates available for the currently-picked vehicle (SpawnVehicleAt).
  // Populated whenever values.ClassName changes so TemplateName renders as a
  // proper combobox of valid options instead of a free-text field.
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
      .then((list) => setCommands(withSyntheticGrants(list)))
      .catch((err) => setError(String(err)));
    void refreshHistory();
  }, [tunnelId, refreshHistory]);

  useEffect(() => {
    // Reset values + apply prefill exactly once per (selected, prefill) pair.
    // The earlier two-effect version raced; the single-effect version still
    // clobbered prefill on the next render after onPrefillConsumed cleared it
    // because the [prefill] dep change re-ran the defaults reset. Track what
    // we've already applied so post-consumption re-renders are a no-op.
    if (!selected) {
      appliedRef.current = null;
      return;
    }
    const prefillFp =
      prefill && prefill.commandId === selected.id ? JSON.stringify(prefill) : null;
    const current = appliedRef.current;

    if (!current || current.selectedId !== selected.id) {
      // Brand new command pick (sidebar click or first prefill into a new command).
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

    // Same command. Only act if a NEW prefill arrived for it.
    if (prefillFp && prefillFp !== current.prefillFp) {
      setValues((prev) => ({ ...prev, ...(prefill?.values ?? {}) }));
      setResult(null);
      onPrefillConsumed?.();
      appliedRef.current = { selectedId: selected.id, prefillFp };
    }
    // Otherwise the prefill was cleared after we consumed it — leave values alone.
  }, [selected, prefill, onPrefillConsumed]);

  useEffect(() => {
    // If a prefill arrives for a command different from what's currently
    // selected, switch the sidebar to that command. The effect above will
    // then notice prefill.commandId === selected.id and apply the values.
    if (!prefill || commands.length === 0) return;
    if (selected?.id === prefill.commandId) return;
    const target = commands.find((c) => c.id === prefill.commandId);
    if (!target) return;
    setSelected(target);
  }, [prefill, commands, selected?.id]);

  useEffect(() => {
    // For SpawnVehicleAt, look up the templates of the picked vehicle so the
    // TemplateName field can render its real options.
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
        // If the current TemplateName isn't valid for this vehicle, auto-pick
        // the first available one. Keeps the form submittable without the user
        // having to know that TreadWheel doesn't carry a T0.
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

  const grouped = useMemo(() => groupByCategory(commands), [commands]);

  const doPublish = useCallback(async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      let out: PublishResultDto;
      if (selected.dbAction === "grant_currency") {
        // DB-grant path: route to the dedicated currency endpoint, not MQ publish.
        const flsId = typeof values.PlayerId === "string" ? values.PlayerId.trim() : "";
        const currencyId = Number((selected.lockedFields?.currencyId as number | undefined) ?? 1);
        const amount = Number(values.Amount);
        out = await managementApi.grantCurrency(tunnelId, flsId, currencyId, amount);
      } else if (selected.dbAction === "award_intel") {
        // DB-grant path: single-leaf jsonb_set on the character actor.
        const flsId = typeof values.PlayerId === "string" ? values.PlayerId.trim() : "";
        const amount = Number(values.Amount);
        out = await managementApi.awardIntel(tunnelId, flsId, amount);
      } else {
        const publishId = selected.publishAs ?? selected.id;
        const payload = { ...values, ...(selected.lockedFields ?? {}) };
        out = await managementApi.publish(tunnelId, publishId, payload);
      }
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
    if (selected.destructive) {
      setConfirmOpen(true);
    } else {
      void doPublish();
    }
  }, [selected, doPublish]);

  return (
    <Flex mt="3" gap="3" align="stretch" wrap="wrap">
      <Box style={{ flex: "0 0 240px", minWidth: 0 }}>
        <Text size="2" weight="medium">
          Commands
        </Text>
        {CATEGORY_ORDER.map((cat) => {
          const specs = grouped[cat];
          if (!specs || specs.length === 0) return null;
          return (
            <Box key={cat} mt="2">
              <Text size="1" color="gray" style={{ textTransform: "uppercase", letterSpacing: 0.5 }}>
                {CATEGORY_LABEL[cat] ?? cat}
              </Text>
              <Flex direction="column" gap="1" mt="1">
                {specs.map((spec) => (
                  <Button
                    key={spec.id}
                    size="1"
                    variant={selected?.id === spec.id ? "solid" : "surface"}
                    color={spec.destructive ? "red" : undefined}
                    onClick={() => setSelected(spec)}
                    style={{ justifyContent: "flex-start" }}
                  >
                    {spec.label}
                  </Button>
                ))}
              </Flex>
            </Box>
          );
        })}
      </Box>
      <Box style={{ flex: "1 1 400px", minWidth: 0 }}>
        {selected ? (
          <Box>
            <Flex justify="between" align="baseline" wrap="wrap" gap="2">
              <Text size="3" weight="medium">
                {selected.label}
              </Text>
              {selected.destructive ? <Badge color="red">destructive</Badge> : null}
            </Flex>
            <Text size="1" color="gray">
              {selected.describe}
            </Text>
            <Flex direction="column" gap="3" mt="3">
              {visibleFields(selected, values).map((field) => (
                <FieldInput
                  key={field.key}
                  field={field}
                  value={values[field.key]}
                  onChange={(v) => setValues((prev) => ({ ...prev, [field.key]: v }))}
                  tunnelId={tunnelId}
                  vehicleTemplates={vehicleTemplates}
                />
              ))}
            </Flex>
            {selected.id === "SpawnVehicleAt" ? (
              <UsePlayerPositionButton
                tunnelId={tunnelId}
                playerId={values.PlayerId as string | undefined}
                onLocation={(loc) =>
                  setValues((prev) => ({ ...prev, X: loc.x, Y: loc.y, Z: loc.z }))
                }
              />
            ) : null}
            <Flex mt="3" gap="2" align="center">
              <Button onClick={publish} disabled={busy} color={selected.destructive ? "red" : undefined}>
                {busy ? "Publishing…" : selected.destructive ? "Publish (destructive)" : "Publish"}
              </Button>
              {result ? (
                <Badge color={result.ok ? "green" : "red"}>{result.ok ? "ok" : "failed"}</Badge>
              ) : null}
            </Flex>
            {result && !result.ok && result.error ? (
              <Text size="1" color="red" mt="2">
                {result.error}
              </Text>
            ) : null}
            {result?.output ? (
              <Box
                mt="2"
                className="mono"
                style={{ fontSize: 11, padding: 6, background: "var(--color-panel-translucent)", whiteSpace: "pre-wrap" }}
              >
                {result.output}
              </Box>
            ) : null}
            {error ? (
              <Text size="1" color="red" mt="2">
                {error}
              </Text>
            ) : null}
          </Box>
        ) : (
          <Text color="gray">Select a command on the left.</Text>
        )}
      </Box>
      <Box style={{ flex: "1 1 320px", minWidth: 0 }}>
        <Text size="2" weight="medium">
          Recent publishes
        </Text>
        <Table.Root variant="surface" size="1" mt="1">
          <Table.Header>
            <Table.Row>
              <Table.ColumnHeaderCell>Cmd</Table.ColumnHeaderCell>
              <Table.ColumnHeaderCell>OK</Table.ColumnHeaderCell>
              <Table.ColumnHeaderCell>When</Table.ColumnHeaderCell>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {history.map((h) => (
              <Table.Row key={h.id}>
                <Table.Cell className="mono" style={{ fontSize: 11 }}>
                  {h.command}
                </Table.Cell>
                <Table.Cell>
                  <Badge color={h.ok ? "green" : "red"}>{h.ok ? "ok" : "fail"}</Badge>
                </Table.Cell>
                <Table.Cell className="mono" style={{ fontSize: 11 }}>
                  {formatTime(h.createdAt)}
                </Table.Cell>
              </Table.Row>
            ))}
          </Table.Body>
        </Table.Root>
      </Box>

      <AlertDialog.Root open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialog.Content maxWidth="460px">
          <AlertDialog.Title>Run {selected?.label}?</AlertDialog.Title>
          <AlertDialog.Description size="2">
            This command is destructive and cannot be undone. {selected?.describe}
          </AlertDialog.Description>
          <Flex gap="2" mt="4" justify="end">
            <AlertDialog.Cancel>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </AlertDialog.Cancel>
            <Button
              color="red"
              onClick={() => {
                setConfirmOpen(false);
                void doPublish();
              }}
            >
              Run it
            </Button>
          </Flex>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </Flex>
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
