import { useCallback } from "react";
import { Flex, Text } from "@radix-ui/themes";

import { managementApi } from "../../services/management";
import Combobox from "./Combobox";

export type ItemComboboxProps = {
  tunnelId: string;
  value: string;
  onChange: (value: string) => void;
};

export default function ItemCombobox({ tunnelId, value, onChange }: ItemComboboxProps) {
  const loadOptions = useCallback(
    async (query: string) => {
      try {
        const rows = await managementApi.searchItems(tunnelId, query, 30);
        return [...rows].sort((a, b) =>
          (a.name || a.id).localeCompare(b.name || b.id, undefined, {
            sensitivity: "base",
            numeric: true,
          }),
        );
      } catch {
        return [];
      }
    },
    [tunnelId],
  );

  const resolveLabel = useCallback(
    async (id: string): Promise<string | null> => {
      if (!id) return null;
      try {
        const rows = await managementApi.searchItems(tunnelId, id, 5);
        const hit = rows.find((it) => it.id === id);
        return hit ? `${hit.name}  ·  ${hit.id}` : id;
      } catch {
        return id;
      }
    },
    [tunnelId],
  );

  return (
    <Combobox
      value={value}
      onChange={onChange}
      loadOptions={loadOptions}
      getOptionValue={(it) => it.id}
      resolveLabel={resolveLabel}
      renderOption={(it) => (
        <Flex justify="between" gap="2">
          <Text size="2">{it.name}</Text>
          <Text size="1" color="gray" className="mono">{it.id}</Text>
        </Flex>
      )}
      placeholder="Pick an item..."
      searchPlaceholder="Search items..."
    />
  );
}
