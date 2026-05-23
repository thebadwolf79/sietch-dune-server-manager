import { useState } from "react";
import { ChevronDownIcon, ChevronRightIcon } from "@radix-ui/react-icons";
import { Flex, Text } from "@radix-ui/themes";

import type { RemoteServerComponent } from "../../types/server";
import { copyTextToClipboard } from "../../utils/clipboard";
import { phaseTone } from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import StatusPill from "../ui/StatusPill";

export type ServerPodRowProps = {
  component: RemoteServerComponent;
  logKey: string;
  logText?: string;
  logBusy: boolean;
  restartBusy: boolean;
  onRefreshLog: () => void;
  onRestart: () => void;
};

export default function ServerPodRow({
  component,
  logKey,
  logText,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: ServerPodRowProps) {
  const [open, setOpen] = useState(false);
  const tone = component.tone === "green"
    ? "ok"
    : component.tone === "amber"
      ? "warn"
      : component.tone === "red"
        ? "err"
        : phaseTone(component.state);
  return (
    <div className="pod-row" data-tone={tone} data-open={open}>
      <button
        type="button"
        className="pod-row-summary"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
      >
        <span className="pod-row-chevron" aria-hidden>
          {open ? <ChevronDownIcon /> : <ChevronRightIcon />}
        </span>
        <span className="pod-row-title">
          <span className="pod-row-name">{component.name}</span>
          <span className="pod-row-key">{component.logKey}</span>
        </span>
        <StatusPill label={component.state} tone={tone} />
        <span className="pod-row-summary-text">{component.summary}</span>
      </button>
      {open ? (
        <div className="pod-row-body">
          {component.details.length > 0 ? (
            <ul className="component-details">
              {component.details.map((detail) => (
                <li key={detail}>{detail}</li>
              ))}
            </ul>
          ) : (
            <Text size="1" style={{ color: "var(--color-text-muted)" }}>
              No additional details reported.
            </Text>
          )}
          <Flex gap="2" mt="2" wrap="wrap">
            <ActionButton
              onClick={onRefreshLog}
              busy={logBusy}
              pendingLabel="Loading logs"
            >
              {logText ? "Refresh logs" : "View logs"}
            </ActionButton>
            <ActionButton
              onClick={onRestart}
              busy={restartBusy}
              tone="danger"
              pendingLabel="Restarting"
            >
              Restart
            </ActionButton>
            {logText ? (
              <ActionButton onClick={() => void copyTextToClipboard(logText)}>
                Copy logs
              </ActionButton>
            ) : null}
          </Flex>
          {logText ? (
            <pre className="component-log" data-log-key={logKey}>
              {logText}
            </pre>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
