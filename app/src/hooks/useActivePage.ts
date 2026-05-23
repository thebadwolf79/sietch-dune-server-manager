import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { RemoteServerRecord } from "../types/server";
import type { ActivePage, ServerSubPage } from "../types/ui";
import { readActivePage, writeActivePage } from "../services/storage";

export type UseActivePageOptions = {
  remoteServers: RemoteServerRecord[];
};

/**
 * Tracks the active top-level page and persists the per-server sub-tab
 * across launches. Falls back to the Servers list when the persisted
 * server is no longer attached.
 */
export function useActivePage({ remoteServers }: UseActivePageOptions) {
  const attachedIds = useMemo(() => remoteServers.map((s) => s.id), [remoteServers]);
  const idsKey = attachedIds.join("|");
  const initializedRef = useRef(false);
  const [activePage, setActivePageState] = useState<ActivePage>(() => readActivePage(attachedIds));

  // Re-validate whenever the attached server list changes.
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      return;
    }
    setActivePageState((current) => {
      if (current.kind === "server" && !attachedIds.includes(current.serverId)) {
        const fallback: ActivePage = { kind: "servers" };
        writeActivePage(fallback);
        return fallback;
      }
      return current;
    });
    // idsKey covers attached id set changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idsKey]);

  const setActivePage = useCallback((next: ActivePage) => {
    writeActivePage(next);
    setActivePageState(next);
  }, []);

  const openServer = useCallback(
    (serverId: string, sub: ServerSubPage = "dashboard") => {
      setActivePage({ kind: "server", serverId, sub });
    },
    [setActivePage],
  );

  const openServersList = useCallback(() => {
    setActivePage({ kind: "servers" });
  }, [setActivePage]);

  const setSub = useCallback(
    (sub: ServerSubPage) => {
      setActivePageState((current) => {
        if (current.kind !== "server") return current;
        const next: ActivePage = { ...current, sub };
        writeActivePage(next);
        return next;
      });
    },
    [],
  );

  return { activePage, setActivePage, openServer, openServersList, setSub };
}
