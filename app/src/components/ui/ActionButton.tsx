import { useEffect, useRef, useState } from "react";

import BusySpinner from "./BusySpinner";

export type ActionButtonTone = "default" | "accent" | "ok" | "danger";

export type ActionButtonProps = {
  children: React.ReactNode;
  onClick: () => void | Promise<void>;
  busy?: boolean;
  disabled?: boolean;
  tone?: ActionButtonTone;
  pendingLabel?: string;
  title?: string;
  className?: string;
};

type Reaction = "idle" | "success" | "error";

/**
 * Button that visually reacts to every action.
 *
 * - While `busy` is true, the label is replaced by an inline spinner +
 *   `pendingLabel`, and the button is locked.
 * - When `busy` flips from true to false, the button briefly flashes
 *   "success". If the onClick handler rejected, the parent should set
 *   the `reactionKey` (via remount) and the button shake-flashes "error".
 *
 * The simplest contract — caller passes `busy`. We watch transitions to
 * decide success/error using the most recent `onClick` outcome.
 */
export default function ActionButton({
  children,
  onClick,
  busy = false,
  disabled = false,
  tone = "default",
  pendingLabel,
  title,
  className,
}: ActionButtonProps) {
  const [reaction, setReaction] = useState<Reaction>("idle");
  const prevBusyRef = useRef(busy);
  const lastResultRef = useRef<"ok" | "error" | null>(null);

  useEffect(() => {
    if (prevBusyRef.current && !busy) {
      // Busy → idle transition. Play whichever reaction the last click
      // reported. Default to success.
      const next: Reaction = lastResultRef.current === "error" ? "error" : "success";
      setReaction(next);
      const timer = window.setTimeout(() => setReaction("idle"), next === "error" ? 520 : 720);
      return () => window.clearTimeout(timer);
    }
    prevBusyRef.current = busy;
    return undefined;
  }, [busy]);

  async function handleClick() {
    if (busy || disabled) return;
    lastResultRef.current = null;
    try {
      const result = onClick();
      if (result && typeof (result as Promise<void>).then === "function") {
        await result;
      }
      lastResultRef.current = "ok";
    } catch (err) {
      lastResultRef.current = "error";
      // Re-throw so callers can still observe failures upstream.
      throw err;
    }
  }

  const state: "idle" | "pending" | "success" | "error" = busy
    ? "pending"
    : reaction === "success"
      ? "success"
      : reaction === "error"
        ? "error"
        : "idle";

  return (
    <button
      type="button"
      className={`action-btn ${className ?? ""}`}
      data-tone={tone}
      data-state={state}
      disabled={disabled || busy}
      onClick={handleClick}
      title={title}
    >
      {busy ? (
        <>
          <BusySpinner />
          <span>{pendingLabel ?? "Working"}</span>
        </>
      ) : (
        children
      )}
    </button>
  );
}
