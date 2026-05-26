export type ServerSubPage =
  | "dashboard"
  | "update"
  | "pods"
  | "users"
  | "admin"
  | "tasks";

export type ActivePage =
  | { kind: "servers" }
  | { kind: "server"; serverId: string; sub: ServerSubPage };

export const SERVER_SUB_PAGES: readonly ServerSubPage[] = [
  "dashboard",
  "update",
  "pods",
  "users",
  "admin",
  "tasks",
] as const;

export const MANAGEMENT_SUB_PAGES: readonly ServerSubPage[] = [
  "users",
  "admin",
  "tasks",
] as const;

export function isManagementSubPage(sub: ServerSubPage): boolean {
  return MANAGEMENT_SUB_PAGES.includes(sub);
}

export type DetectionState = "idle" | "detecting" | "ready" | "failed";

export type BadgeTone = "green" | "amber" | "red" | "gray" | "bronze";

export type RemoteAttachForm = {
  host: string;
  user: string;
  keyPath: string;
  port: number;
};
