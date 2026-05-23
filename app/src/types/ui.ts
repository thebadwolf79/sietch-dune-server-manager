export type ServerSubPage = "dashboard" | "update" | "pods";

export type ActivePage =
  | { kind: "servers" }
  | { kind: "server"; serverId: string; sub: ServerSubPage };

export const SERVER_SUB_PAGES: readonly ServerSubPage[] = ["dashboard", "update", "pods"] as const;

export type DetectionState = "idle" | "detecting" | "ready" | "failed";

export type BadgeTone = "green" | "amber" | "red" | "gray" | "bronze";

export type RemoteAttachForm = {
  host: string;
  user: string;
  keyPath: string;
};
