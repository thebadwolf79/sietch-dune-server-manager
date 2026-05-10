import type { LucideIcon } from "lucide-react";

export type ViewKey =
  | "overview"
  | "host"
  | "manager"
  | "players"
  | "battlegroups"
  | "workloads"
  | "director"
  | "config"
  | "logs";

export type NavItem = {
  key: ViewKey;
  label: string;
  icon: LucideIcon;
  disabled?: boolean;
};
