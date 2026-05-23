export const pages = [{ id: "servers", label: "Servers" }] as const;
export type PageId = (typeof pages)[number]["id"];

export type DetectionState = "idle" | "detecting" | "ready" | "failed";

export type BadgeTone = "green" | "amber" | "red" | "gray" | "bronze";

export type RemoteAttachForm = {
  host: string;
  keyPath: string;
};
