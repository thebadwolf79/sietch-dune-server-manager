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
  isLast?: boolean;
};

export default function ServerPodRow({
  component,
  logKey,
  logText,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
  isLast = false,
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
    <div
      style={{
        borderBottom: isLast ? "none" : "1px solid var(--color-border-hair)",
        backgroundColor: open ? "var(--color-bg-elevated)" : "transparent",
        transition: "background-color 160ms var(--ease-out)",
      }}
      data-tone={tone}
      data-open={open}
    >
      <button
        type="button"
        className="pod-row-summary"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        style={{
          display: "grid",
          gridTemplateColumns: "20px minmax(0, 1.6fr) auto minmax(0, 1fr)",
          gap: "12px",
          alignItems: "center",
          width: "100%",
          padding: "10px 14px",
          background: "transparent",
          border: 0,
          color: "inherit",
          textAlign: "left",
          cursor: "pointer",
        }}
      >
        <span className="pod-row-chevron" aria-hidden>
          {open ? <ChevronDownIcon /> : <ChevronRightIcon />}
        </span>
        <span className="pod-row-title">
          <span className="pod-row-name">{component.name}</span>
          <span className="pod-row-key">{component.logKey}</span>
        </span>
        <StatusPill label={component.state} tone={tone} />
        <span className="pod-row-summary-text" style={{ paddingLeft: "8px" }}>
          {component.summary}
        </span>
      </button>
      {open ? (
        <div
          className="pod-row-body"
          style={{
            padding: "12px 14px 14px 46px",
            borderTop: "1px solid var(--color-border-hair)",
            backgroundColor: "rgba(0,0,0,0.15)",
          }}
        >
          {component.details.length > 0 ? (
            <ul className="component-details" style={{ margin: "0 0 12px 0", paddingLeft: "16px" }}>
              {component.details.map((detail) => (
                <li key={detail} style={{ fontSize: "12px", color: "var(--color-text-secondary)", marginBottom: "3px" }}>
                  {detail}
                </li>
              ))}
            </ul>
          ) : (
            <Text size="1" style={{ color: "var(--color-text-muted)", display: "block", marginBottom: "12px" }}>
              No additional details reported.
            </Text>
          )}
          <Flex gap="2" wrap="wrap">
            <ActionButton
              onClick={onRefreshLog}
              busy={logBusy}
              pendingLabel="Loading logs"
              className="chamfer-sm"
            >
              {logText ? "Refresh logs" : "View logs"}
            </ActionButton>
            <ActionButton
              onClick={onRestart}
              busy={restartBusy}
              tone="danger"
              pendingLabel="Restarting"
              className="chamfer-sm"
            >
              Restart
            </ActionButton>
            {logText ? (
              <ActionButton onClick={() => void copyTextToClipboard(logText)} className="chamfer-sm">
                Copy logs
              </ActionButton>
            ) : null}
          </Flex>
          {logText ? (
            <pre
              className="component-log"
              data-log-key={logKey}
              style={{
                marginTop: "12px",
                maxHeight: "300px",
                overflow: "auto",
                padding: "10px",
                borderRadius: "var(--radius-2)",
                border: "1px solid var(--color-border-hair)",
                background: "var(--color-bg-base)",
                fontFamily: "var(--font-mono)",
                fontSize: "11.5px",
                color: "var(--color-text-secondary)",
              }}
            >
              {logText}
            </pre>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

