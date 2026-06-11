import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Box,
  DropdownMenu,
  Flex,
  Switch,
  Text,
  TextField,
} from "@radix-ui/themes";
import { Search } from "lucide-react";

import { managementApi } from "../../services/management";
import type { PlayerDto } from "../../types/management";
import { copyTextToClipboard } from "../../utils/clipboard";
import { formatDateTime } from "../../utils/formatting";

import type { AdminTabPrefill } from "./AdminTab";

// The service sends last-seen as a UTC wall-clock string with no offset
// ("YYYY-MM-DD HH:MM:SS"). Tag it as UTC so it localizes instead of being
// parsed as the viewer's local time, then render in their timezone.
function formatLastSeen(raw: string): string {
  const s = raw.trim();
  if (!s) return "—";
  return formatDateTime(`${s.replace(" ", "T")}Z`);
}

export type UsersTabProps = {
  tunnelId: string;
  /**
   * Whether the BattleGroup is up and the player query can succeed. When false
   * (BG stopped/offline) we stop the initial load, debounced search, and the
   * auto-refresh poll — otherwise each poll hangs on an unavailable endpoint
   * and stacks up, freezing the screen (#25).
   */
  serverReachable: boolean;
  onSwitchToAdmin: (prefill: AdminTabPrefill) => void;
};

export default function UsersTab({
  tunnelId,
  serverReachable,
  onSwitchToAdmin,
}: UsersTabProps) {
  const [users, setUsers] = useState<PlayerDto[]>([]);
  const [query, setQuery] = useState("");
  const [onlineOnly, setOnlineOnly] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(
    async (q: string) => {
      setBusy(true);
      setError(null);
      try {
        const rows = await managementApi.searchPlayers(tunnelId, q, 200);
        setUsers(rows);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [tunnelId],
  );

  useEffect(() => {
    if (!serverReachable) return;
    void reload("");
  }, [reload, serverReachable]);

  useEffect(() => {
    if (!serverReachable) return;
    const handle = setTimeout(() => {
      void reload(query.trim());
    }, 300);
    return () => clearTimeout(handle);
  }, [query, reload, serverReachable]);

  // Poll for live player-status changes. Without this the list only refreshed
  // on mount / manual click, so logins and logouts went unseen until the app
  // was reopened (#13). Toggleable per #14; on by default. Gated on
  // serverReachable so a stopped BattleGroup doesn't get polled (#25).
  useEffect(() => {
    if (!autoRefresh || !serverReachable) return;
    const handle = setInterval(() => {
      void reload(query.trim());
    }, 5000);
    return () => clearInterval(handle);
  }, [autoRefresh, query, reload, serverReachable]);

  const visible = useMemo(
    () => (onlineOnly ? users.filter((u) => u.online.toLowerCase() === "online") : users),
    [users, onlineOnly],
  );

  return (
    <Box mt="3" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      {/* Filters Toolbar */}
      <Flex gap="3" align="center" wrap="wrap">
        <Box style={{ flex: "1 1 280px", minWidth: 0 }}>
          <TextField.Root
            placeholder="Search name or FLS id…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            disabled={!serverReachable}
            size="2"
          >
            <TextField.Slot>
              <Search size={14} style={{ opacity: 0.6 }} />
            </TextField.Slot>
          </TextField.Root>
        </Box>
        <Flex align="center" gap="2">
          <Switch checked={onlineOnly} onCheckedChange={setOnlineOnly} />
          <Text size="2">Online only</Text>
        </Flex>
        <Flex align="center" gap="2">
          <Switch checked={autoRefresh} onCheckedChange={setAutoRefresh} />
          <Text size="2">Auto-refresh</Text>
        </Flex>
        <button
          type="button"
          onClick={() => void reload(query.trim())}
          disabled={busy || !serverReachable}
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            padding: "4px 10px",
            fontSize: "12px",
            cursor: busy || !serverReachable ? "not-allowed" : "pointer",
            border: "1px solid var(--color-border-hair)",
            background: "var(--color-bg-elevated)",
            borderRadius: "var(--radius-1)",
            color: "var(--color-text-primary)",
            transition: "all 140ms var(--ease-out)",
          }}
          className="chamfer-sm"
        >
          {busy ? "Loading…" : "Refresh"}
        </button>
        <Text
          size="1"
          color="gray"
          style={{
            marginLeft: "auto",
            flexShrink: 0,
            textAlign: "right",
            fontVariantNumeric: "tabular-nums",
          }}
        >
          {visible.length} of {users.length}
        </Text>
      </Flex>

      {error && (
        <Text size="1" color="red">
          {error}
        </Text>
      )}

      {/* Players List Panel */}
      {!serverReachable ? (
        <Box
          className="bracket chamfer"
          style={{
            background: "var(--color-bg-panel)",
            border: "1px solid var(--color-border-hair)",
            borderRadius: "var(--radius-3)",
            padding: "24px 16px",
            textAlign: "center",
          }}
        >
          <Text size="2" color="gray">
            The BattleGroup is offline — player data isn&apos;t available. Auto-refresh is
            paused and resumes automatically when the server is back up.
          </Text>
        </Box>
      ) : (
      <Box
        className="bracket chamfer"
        style={{
          background: "var(--color-bg-panel)",
          border: "1px solid var(--color-border-hair)",
          borderRadius: "var(--radius-3)",
          overflow: "hidden",
        }}
      >
        {/* Table Header */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(12, minmax(0, 1fr))",
            gap: "8px",
            borderBottom: "1px solid var(--color-border-hair)",
            backgroundColor: "var(--color-bg-elevated)",
            padding: "10px 16px",
          }}
          className="hidden-sm-header"
        >
          <Text size="1" weight="bold" style={{ gridColumn: "span 3", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)" }}>Name</Text>
          <Text size="1" weight="bold" style={{ gridColumn: "span 3", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)" }}>FLS ID</Text>
          <Text size="1" weight="bold" style={{ gridColumn: "span 1", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)" }}>Level</Text>
          <Text size="1" weight="bold" style={{ gridColumn: "span 1", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)" }}>Part.</Text>
          <Text size="1" weight="bold" style={{ gridColumn: "span 1", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)" }}>Status</Text>
          <Text size="1" weight="bold" style={{ gridColumn: "span 2", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)" }}>Last seen</Text>
          <Text size="1" weight="bold" style={{ gridColumn: "span 1", fontFamily: "var(--font-mono)", textTransform: "uppercase", letterSpacing: "0.04em", color: "var(--color-text-muted)", textAlign: "right" }}></Text>
        </div>

        {/* Rows */}
        <Box style={{ display: "flex", flexDirection: "column" }}>
          {visible.map((user) => {
            const isOnline = user.online.toLowerCase() === "online";
            return (
              <div
                key={user.flsId}
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(12, minmax(0, 1fr))",
                  gap: "8px",
                  alignItems: "center",
                  borderBottom: "1px solid var(--color-border-hair)",
                  padding: "10px 16px",
                  transition: "background-color 140ms var(--ease-out)",
                }}
                className="user-row-grid"
              >
                <span style={{ gridColumn: "span 3", fontSize: "13px", fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {user.name || <Text color="gray">—</Text>}
                </span>
                <span className="mono" style={{ gridColumn: "span 3", fontSize: "11px", color: "var(--color-text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {user.flsId}
                </span>
                <span className="mono" style={{ gridColumn: "span 1", fontSize: "11px", color: "var(--color-text-secondary)" }}>
                  {user.level ?? <Text color="gray">—</Text>}
                </span>
                <span className="mono" style={{ gridColumn: "span 1", fontSize: "11px", color: "var(--color-text-secondary)" }}>
                  {user.partitionId ?? <Text color="gray">—</Text>}
                </span>
                <span style={{ gridColumn: "span 1" }}>
                  <Badge color={isOnline ? "green" : "gray"}>
                    {user.online || "offline"}
                  </Badge>
                </span>
                <span className="mono" style={{ gridColumn: "span 2", fontSize: "11px", color: "var(--color-text-muted)" }}>
                  {isOnline ? "—" : formatLastSeen(user.lastSeen)}
                </span>
                <span style={{ gridColumn: "span 1", textAlign: "right" }}>
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      <button
                        type="button"
                        style={{
                          background: "transparent",
                          border: 0,
                          fontSize: "12px",
                          fontWeight: 600,
                          color: "var(--color-accent-strong)",
                          cursor: "pointer",
                          padding: "4px 8px",
                        }}
                        className="hover-accent-link"
                      >
                        Actions
                      </button>
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content>
                      <DropdownMenu.Item onSelect={() => void copyTextToClipboard(user.flsId)}>
                        Copy FLS ID
                      </DropdownMenu.Item>
                      <DropdownMenu.Item
                        onSelect={() =>
                          onSwitchToAdmin({
                            commandId: "AddItemToInventory",
                            values: { PlayerId: user.flsId },
                          })
                        }
                      >
                        Grant item…
                      </DropdownMenu.Item>
                      <DropdownMenu.Item
                        onSelect={() =>
                          onSwitchToAdmin({
                            commandId: "AwardXP",
                            values: { PlayerId: user.flsId },
                          })
                        }
                      >
                        Award XP…
                      </DropdownMenu.Item>
                      <DropdownMenu.Item
                        onSelect={() =>
                          onSwitchToAdmin({
                            commandId: "TeleportTo",
                            values: { PlayerId: user.flsId },
                          })
                        }
                      >
                        Teleport…
                      </DropdownMenu.Item>
                      <DropdownMenu.Separator />
                      <DropdownMenu.Item
                        color="red"
                        onSelect={() =>
                          onSwitchToAdmin({
                            commandId: "KickPlayer",
                            values: { PlayerId: user.flsId },
                          })
                        }
                      >
                        Kick player…
                      </DropdownMenu.Item>
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </span>
              </div>
            );
          })}
          {visible.length === 0 && !busy && (
            <div style={{ textAlign: "center", padding: "24px 0" }}>
              <Text color="gray" size="2">
                No users{onlineOnly ? " online" : ""} match your filters.
              </Text>
            </div>
          )}
        </Box>
      </Box>
      )}
    </Box>
  );
}
