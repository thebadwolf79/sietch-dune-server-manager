import { useEffect, useRef } from "react";
import type { CSSProperties } from "react";
import { parseAnsiLine } from "../domain/logs";

type LogOutputProps = {
  lines: string[];
  emptyText?: string;
};

export function LogOutput({ lines, emptyText = "No log lines returned." }: LogOutputProps) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);

  function scrollToBottom() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    viewport.scrollTop = viewport.scrollHeight;
  }

  function updateStickiness() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    stickToBottomRef.current = distanceFromBottom < 28;
  }

  useEffect(() => {
    scrollToBottom();
  }, []);

  useEffect(() => {
    if (stickToBottomRef.current) {
      requestAnimationFrame(scrollToBottom);
    }
  }, [lines]);

  if (lines.length === 0) {
    return <div className="log-output empty-log">{emptyText}</div>;
  }

  return (
    <div className="log-output" role="log" aria-live="polite" ref={viewportRef} onScroll={updateStickiness}>
      {lines.map((line, index) => (
        <div className="log-line" key={`${index}-${line.slice(0, 32)}`}>
          {parseAnsiLine(line).map((segment, segmentIndex) => (
            <span
              className={segment.className}
              style={segment.style as CSSProperties}
              key={`${segmentIndex}-${segment.text.slice(0, 16)}`}
            >
              {segment.text || " "}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}
