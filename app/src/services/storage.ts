import type { RemoteServerRecord } from "../types/server";
import type { ActivePage, ServerSubPage } from "../types/ui";
import { SERVER_SUB_PAGES } from "../types/ui";

const remoteServersStorageKey = "dune-manager.remote-servers";
const activePageStorageKey = "dune-manager.active-page";
const logSidebarStorageKey = "dune-manager.log-sidebar";

export function isRemoteServerRecord(value: unknown): value is RemoteServerRecord {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<RemoteServerRecord>;
  return (
    record.type === "ubuntu" &&
    typeof record.id === "string" &&
    typeof record.name === "string" &&
    typeof record.host === "string" &&
    typeof record.keyPath === "string"
  );
}

export function readRemoteServers(): RemoteServerRecord[] {
  const text = window.localStorage.getItem(remoteServersStorageKey);
  if (!text) return [];
  try {
    const parsed = JSON.parse(text);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isRemoteServerRecord);
  } catch {
    window.localStorage.removeItem(remoteServersStorageKey);
    return [];
  }
}

export function mergeRemoteServers(
  current: RemoteServerRecord[],
  incoming: RemoteServerRecord[],
): RemoteServerRecord[] {
  const byId = new Map(current.map((server) => [server.id, server]));
  for (const server of incoming) {
    byId.set(server.id, { ...byId.get(server.id), ...server });
  }
  return Array.from(byId.values()).sort((a, b) => a.name.localeCompare(b.name));
}

export function persistRemoteServers(servers: RemoteServerRecord[]): RemoteServerRecord[] {
  const unique = mergeRemoteServers([], servers);
  window.localStorage.setItem(remoteServersStorageKey, JSON.stringify(unique));
  return unique;
}

export function upsertRemoteServer(
  servers: RemoteServerRecord[],
  server: RemoteServerRecord,
): RemoteServerRecord[] {
  return mergeRemoteServers(servers, [server]);
}

type PersistedActivePage = { activeServerId?: string; activeSub?: ServerSubPage };

function isServerSubPage(value: unknown): value is ServerSubPage {
  return typeof value === "string" && (SERVER_SUB_PAGES as readonly string[]).includes(value);
}

export function readActivePage(attachedServerIds: string[]): ActivePage {
  const text = window.localStorage.getItem(activePageStorageKey);
  if (!text) return { kind: "servers" };
  try {
    const parsed = JSON.parse(text) as PersistedActivePage;
    const id = parsed?.activeServerId;
    if (!id || !attachedServerIds.includes(id)) return { kind: "servers" };
    const sub = isServerSubPage(parsed?.activeSub) ? parsed.activeSub : "dashboard";
    return { kind: "server", serverId: id, sub };
  } catch {
    window.localStorage.removeItem(activePageStorageKey);
    return { kind: "servers" };
  }
}

export function writeActivePage(page: ActivePage): void {
  if (page.kind === "servers") {
    window.localStorage.removeItem(activePageStorageKey);
    return;
  }
  const payload: PersistedActivePage = { activeServerId: page.serverId, activeSub: page.sub };
  window.localStorage.setItem(activePageStorageKey, JSON.stringify(payload));
}

type PersistedLogSidebar = { collapsed?: boolean; scopeToActiveServer?: boolean };

export function readLogSidebar(): PersistedLogSidebar {
  const text = window.localStorage.getItem(logSidebarStorageKey);
  if (!text) return {};
  try {
    const parsed = JSON.parse(text) as PersistedLogSidebar;
    return {
      collapsed: typeof parsed.collapsed === "boolean" ? parsed.collapsed : undefined,
      scopeToActiveServer:
        typeof parsed.scopeToActiveServer === "boolean" ? parsed.scopeToActiveServer : undefined,
    };
  } catch {
    window.localStorage.removeItem(logSidebarStorageKey);
    return {};
  }
}

export function writeLogSidebar(state: PersistedLogSidebar): void {
  window.localStorage.setItem(logSidebarStorageKey, JSON.stringify(state));
}
