import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Box, Button, Flex, Popover, Text, TextField } from "@radix-ui/themes";

export type ComboboxProps<T> = {
  value: string;
  onChange: (value: string) => void;
  loadOptions: (query: string) => Promise<T[]>;
  getOptionValue: (option: T) => string;
  renderOption: (option: T) => ReactNode;
  /** Resolve the friendly label for the current value (e.g. fetch the item name). */
  resolveLabel?: (value: string) => Promise<string | null>;
  placeholder?: string;
  searchPlaceholder?: string;
  disabled?: boolean;
  /** Optional override of the trigger label (e.g. "All online" sentinel). */
  triggerLabelOverride?: (value: string) => string | null;
};

export default function Combobox<T>({
  value,
  onChange,
  loadOptions,
  getOptionValue,
  renderOption,
  resolveLabel,
  placeholder = "(none)",
  searchPlaceholder = "Search…",
  disabled,
  triggerLabelOverride,
}: ComboboxProps<T>) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [options, setOptions] = useState<T[]>([]);
  const [loading, setLoading] = useState(false);
  const [resolvedLabel, setResolvedLabel] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

  // Resolve the friendly label whenever the value changes (and isn't an override).
  useEffect(() => {
    const override = triggerLabelOverride?.(value);
    if (override !== undefined && override !== null) {
      setResolvedLabel(override);
      return;
    }
    if (!value || !resolveLabel) {
      setResolvedLabel(null);
      return;
    }
    let cancelled = false;
    resolveLabel(value)
      .then((label) => {
        if (!cancelled) setResolvedLabel(label);
      })
      .catch(() => {
        if (!cancelled) setResolvedLabel(null);
      });
    return () => {
      cancelled = true;
    };
  }, [value, resolveLabel, triggerLabelOverride]);

  // Debounced load on open + on query change.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    const handle = setTimeout(() => {
      loadOptions(query)
        .then((opts) => {
          if (!cancelled) {
            setOptions(opts);
            setLoading(false);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setOptions([]);
            setLoading(false);
          }
        });
    }, 200);
    return () => {
      cancelled = true;
      clearTimeout(handle);
    };
  }, [open, query, loadOptions]);

  const handleOpenChange = useCallback((next: boolean) => {
    setOpen(next);
    if (next) {
      // Reset search and focus the input when opening.
      setQuery("");
      setTimeout(() => searchRef.current?.focus(), 30);
    }
  }, []);

  const pick = useCallback(
    (option: T) => {
      onChange(getOptionValue(option));
      setOpen(false);
    },
    [getOptionValue, onChange],
  );

  const triggerLabel = resolvedLabel ?? value ?? "";

  return (
    <Popover.Root open={open} onOpenChange={handleOpenChange}>
      <Popover.Trigger>
        <Button
          variant="surface"
          color="gray"
          disabled={disabled}
          type="button"
          className="combobox-trigger"
        >
          <Flex align="center" justify="between" gap="2" width="100%">
            {triggerLabel ? (
              <Text size="2" className="combobox-trigger-label">
                {triggerLabel}
              </Text>
            ) : (
              <Text size="2" color="gray">
                {placeholder}
              </Text>
            )}
            <Text size="1" color="gray" aria-hidden>
              ▾
            </Text>
          </Flex>
        </Button>
      </Popover.Trigger>
      <Popover.Content size="1" minWidth="320px" maxWidth="480px">
        <Flex direction="column" gap="2">
          <TextField.Root
            ref={searchRef}
            size="1"
            placeholder={searchPlaceholder}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <Box className="combobox-list">
            {loading ? (
              <Text size="1" color="gray">
                Loading…
              </Text>
            ) : options.length === 0 ? (
              <Text size="1" color="gray">
                No matches.
              </Text>
            ) : (
              options.map((option) => (
                <button
                  key={getOptionValue(option)}
                  type="button"
                  className="combobox-option"
                  onClick={() => pick(option)}
                >
                  {renderOption(option)}
                </button>
              ))
            )}
          </Box>
          {value ? (
            <Flex justify="end">
              <Button
                size="1"
                variant="ghost"
                color="gray"
                onClick={() => {
                  onChange("");
                  setOpen(false);
                }}
              >
                Clear
              </Button>
            </Flex>
          ) : null}
        </Flex>
      </Popover.Content>
    </Popover.Root>
  );
}
