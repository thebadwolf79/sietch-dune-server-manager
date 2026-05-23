export type StatusTone = "ok" | "warn" | "err" | "gray";

export type StatusPillProps = {
  label: string;
  tone: StatusTone;
  pulse?: boolean;
};

/**
 * Status pill with optional pulsing dot. Used as the hero indicator on
 * server cards. Tone follows the same vocabulary as the rest of the app
 * (`ok` / `warn` / `err` / `gray`).
 */
export default function StatusPill({ label, tone, pulse = false }: StatusPillProps) {
  return (
    <span className="status-pill" data-tone={tone}>
      <span className="status-dot" data-pulse={pulse ? "true" : "false"} aria-hidden />
      {label}
    </span>
  );
}
