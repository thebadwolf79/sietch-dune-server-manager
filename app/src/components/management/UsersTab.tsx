import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Box,
  Button,
  DropdownMenu,
  Flex,
  Switch,
  Table,
  Text,
  TextField,
} from "@radix-ui/themes";

import { managementApi } from "../../services/management";
import type { PlayerDto } from "../../types/management";
import { copyTextToClipboard } from "../../utils/clipboard";

import type { AdminTabPrefill } from "./AdminTab";

export type UsersTabProps = {
  tunnelId: string;
  onSwitchToAdmin: (prefill: AdminTabPrefill) => void;
};

export default function UsersTab({ tunnelId, onSwitchToAdmin }: UsersTabProps) {
  const [users, setUsers] = useState<PlayerDto[]>([]);
  const [query, setQuery] = useState("");
  const [onlineOnly, setOnlineOnly] = useState(false);
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
    void reload("");
  }, [reload]);

  useEffect(() => {
    const handle = setTimeout(() => {
      void reload(query.trim());
    }, 300);
    return () => clearTimeout(handle);
  }, [query, reload]);

  const visible = useMemo(
    () => (onlineOnly ? users.filter((u) => u.online.toLowerCase() === "online") : users),
    [users, onlineOnly],
  );

  return (
    <Box mt="3">
      <Flex gap="3" align="center" wrap="wrap" mb="3">
        <TextField.Root
          placeholder="Search name or FLS id…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{ flex: "1 1 280px", minWidth: 0 }}
        />
        <Flex align="center" gap="2">
          <Switch checked={onlineOnly} onCheckedChange={setOnlineOnly} />
          <Text size="2">Online only</Text>
        </Flex>
        <Button size="1" variant="ghost" onClick={() => void reload(query.trim())} disabled={busy}>
          {busy ? "Loading…" : "Refresh"}
        </Button>
        <Text size="1" color="gray" style={{ marginLeft: "auto" }}>
          {visible.length} of {users.length}
        </Text>
      </Flex>

      {error ? (
        <Text size="1" color="red">
          {error}
        </Text>
      ) : null}

      <Table.Root variant="surface" size="1">
        <Table.Header>
          <Table.Row>
            <Table.ColumnHeaderCell>Name</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>FLS ID</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>Status</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>Last seen (UTC)</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell></Table.ColumnHeaderCell>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {visible.map((user) => (
            <Table.Row key={user.flsId}>
              <Table.Cell>{user.name || <Text color="gray">—</Text>}</Table.Cell>
              <Table.Cell className="mono" style={{ fontSize: 11 }}>
                {user.flsId}
              </Table.Cell>
              <Table.Cell>
                <Badge color={user.online.toLowerCase() === "online" ? "green" : "gray"}>
                  {user.online || "offline"}
                </Badge>
              </Table.Cell>
              <Table.Cell className="mono" style={{ fontSize: 11, color: "var(--gray-10)" }}>
                {user.lastSeen || "—"}
              </Table.Cell>
              <Table.Cell>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    <Button size="1" variant="ghost">
                      Actions
                    </Button>
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
              </Table.Cell>
            </Table.Row>
          ))}
          {visible.length === 0 && !busy ? (
            <Table.Row>
              <Table.Cell colSpan={5}>
                <Text color="gray">
                  No users{onlineOnly ? " online" : ""}.
                </Text>
              </Table.Cell>
            </Table.Row>
          ) : null}
        </Table.Body>
      </Table.Root>
    </Box>
  );
}
