import type { RemoteServerRecord } from "../types/server";

const remoteServersStorageKey = "dune-manager.remote-servers";

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
