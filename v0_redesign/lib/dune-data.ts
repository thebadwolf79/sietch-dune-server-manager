export type PodStatus = "ready" | "problem" | "stopped"

export type StatusKind = "success" | "warning" | "destructive" | "muted"

export interface LogEntry {
  time: string
  level: "INFO" | "WARN" | "ERROR"
  message: string
}

export interface PodGroup {
  name: string
  slug: string
  status: PodStatus
  detail: string
}

export interface MapRow {
  name: string
  phase: string
  ready: boolean
  players: number
  maxPlayers: number
  fps: number
  age: string
}

// Mirrors SystemState from app/src/types/vm.ts (collapsed to the labels the UI needs)
export type VmStage = "off" | "saved" | "running"
export type LifecyclePhase =
  | "stopped"
  | "starting"
  | "healthy"
  | "degraded"
  | "stopping"

export type HealthSeverity = "ok" | "info" | "warning" | "critical"

export interface HostMetricChip {
  label: string
  value: string
  severity: HealthSeverity
}

export interface HealthFinding {
  id: string
  severity: HealthSeverity
  title: string
  detail: string
  recommendation: string
  fixLabel: string | null
}

export interface HostHealthReport {
  overallSeverity: HealthSeverity
  summary: string
  clusterChecked: boolean
  metrics: HostMetricChip[]
  findings: HealthFinding[]
}

export interface ServerUser {
  name: string
  flsId: string
  level: string
  partition: number
  online: boolean
  lastSeen: string
}

export interface PackageItem {
  id: string
  label: string
  schematic: string
  qty: number
}

export type RunStatus = "running" | "success" | "failed" | "skipped"
export type RunTrigger = "scheduled" | "manual" | "startup"

export interface TaskRun {
  id: string
  task: string
  status: RunStatus
  trigger?: RunTrigger
  dryRun?: boolean
  when: string
  duration: string
}

export interface PublishRow {
  cmd: string
  ok: boolean
  when: string
}

export const server = {
  name: "BadWolf",
  state: "STARTED" as const,
  host: "dune@192.168.200.10",
  battlegroup: "sh-431c7b16e03f3f97-jlbdmm",
  uptime: "4h 8m",
  namespace: "funcom-seabass-sh-431c7b16e03f3f97-jlbd…",
  database: "Ready",
  gateway: "Running",
  director: "Healthy",
  vm: "dune-awakening",
  vmStage: "running" as VmStage,
  lifecycle: "healthy" as LifecyclePhase,
  managementVersion: "0.3.16",
  managementInit: "openrc",
}

export type Verdict = "operational" | "degraded" | "down"

// Synthesized single-glance verdict derived from cluster phases + host health.
export const systemStatus = {
  verdict: "degraded" as Verdict,
  headline: "Degraded",
  detail: "Deep Desert map stopped · database pod restarted 2× · swap nearly exhausted.",
  activePlayers: 0,
  peakPlayers: 12,
  capacity: 140,
}

// Recent samples (oldest → newest) for at-a-glance sparkline trends.
export const playerTrend = [3, 5, 4, 6, 8, 12, 9, 7, 5, 4, 2, 1, 0, 0]
export const fpsTrend = [30, 30, 29, 30, 30, 28, 27, 30, 30, 29, 30, 30]
export const memTrend = [9.1, 9.6, 10.2, 10.8, 11.1, 11.4, 11.2, 11.4]

export const logs: LogEntry[] = [
  { time: "03:42:27 PM", level: "INFO", message: "BadWolf: Healthy, server group Running, DB Ready, Director Healthy, up 4h 8m." },
  { time: "03:43:02 PM", level: "INFO", message: "Checking management service on IP address…" },
  { time: "03:43:02 PM", level: "INFO", message: "Checking management service on IP address…" },
  { time: "03:43:02 PM", level: "INFO", message: "Management service on IP address: active v0.3.16 (openrc)." },
  { time: "03:43:02 PM", level: "INFO", message: "Management service on IP address: active v0.3.16 (openrc)." },
]

export const maps: MapRow[] = [
  { name: "Hagga Basin #1", phase: "Running", ready: true, players: 0, maxPlayers: 40, fps: 30, age: "4h 8m" },
  { name: "Overmap #2", phase: "Running", ready: true, players: 0, maxPlayers: 60, fps: 30, age: "4h 8m" },
  { name: "Deep Desert", phase: "Stopped", ready: false, players: 0, maxPlayers: 40, fps: 0, age: "—" },
]

export const hostHealth: HostHealthReport = {
  overallSeverity: "warning",
  clusterChecked: true,
  summary: "1 warning, 1 info. Swap is undersized for the configured memory pressure.",
  metrics: [
    { label: "RAM", value: "11.4 / 15.6 GB", severity: "ok" },
    { label: "Swap", value: "1.9 / 2.0 GB", severity: "warning" },
    { label: "Swappiness", value: "60", severity: "info" },
    { label: "Disk /", value: "61% used · 38 GB free", severity: "ok" },
    { label: "DB restarts", value: "2", severity: "info" },
  ],
  findings: [
    {
      id: "swap-undersized",
      severity: "warning",
      title: "Swap space is nearly exhausted",
      detail: "Swap is 1.9 GB used of 2.0 GB while 4.2 GB of RAM is committed. Under load the kernel may OOM-kill the database pod.",
      recommendation: "Grow the swapfile to at least 8 GB and persist it in /etc/fstab.",
      fixLabel: "Resize swap to 8 GB",
    },
    {
      id: "swappiness-high",
      severity: "info",
      title: "Swappiness is higher than recommended",
      detail: "vm.swappiness=60 encourages the kernel to swap out game-server pages, adding latency spikes.",
      recommendation: "Lower vm.swappiness to 10 for a latency-sensitive game host.",
      fixLabel: "Set swappiness to 10",
    },
    {
      id: "fstab-swap-missing",
      severity: "info",
      title: "Swap is not persisted in /etc/fstab",
      detail: "The active swapfile is not referenced in /etc/fstab and will not survive a host reboot.",
      recommendation: "Add the swapfile entry to /etc/fstab.",
      fixLabel: "Persist swap in fstab",
    },
  ],
}

export const tunnels = [
  { name: "Director UI", status: "Tunnel stopped" },
  { name: "File Browser", status: "Tunnel stopped" },
  { name: "Postgres", status: "Tunnel stopped" },
  { name: "PgHero", status: "Tunnel stopped" },
]

export const systemPods: PodGroup[] = [
  { name: "Database", slug: "database", status: "problem", detail: "One or more pods are failing." },
  { name: "Database utilities", slug: "database-utilities", status: "ready", detail: "All pods are ready." },
  { name: "Message Queue", slug: "message-queue", status: "ready", detail: "All pods are ready." },
  { name: "Director", slug: "director", status: "ready", detail: "All pods are ready." },
  { name: "Gateway", slug: "gateway", status: "ready", detail: "All pods are ready." },
  { name: "Text Router", slug: "text-router", status: "ready", detail: "All pods are ready." },
  { name: "File Browser", slug: "file-browser", status: "ready", detail: "All pods are ready." },
  { name: "Server Group", slug: "server-group", status: "ready", detail: "Server Group reports Running." },
  { name: "Gateway Resource", slug: "gateway-resource", status: "ready", detail: "Gateway Resource reports Healthy." },
]

export const mapPods: PodGroup[] = [
  { name: "Deep Desert", slug: "map-deepdesert", status: "stopped", detail: "Deep Desert reports Stopped." },
  { name: "Overmap", slug: "map-overmap", status: "ready", detail: "Overmap reports Running." },
  { name: "Hagga Basin", slug: "map-survival-1", status: "ready", detail: "Hagga Basin reports Running." },
]

export const users: ServerUser[] = [
  { name: "Maren Shai", flsId: "431C7B16E03F3F97", level: "—", partition: 1, online: false, lastSeen: "06/10/2026 12:10:13" },
]

export const packageItems: PackageItem[] = [
  { id: "1", label: "Aren's Chestpiece", schematic: "Schematic_UniquePincushionChest", qty: 1 },
  { id: "2", label: "Aren's Gloves", schematic: "Schematic_UniquePincushionHands", qty: 1 },
  { id: "3", label: "Aren's Mask", schematic: "Schematic_UniquePincushionHead", qty: 1 },
  { id: "4", label: "Aren's Pants", schematic: "Schematic_UniquePincushionLegs", qty: 1 },
  { id: "5", label: "Hollower Stillsuit Boots", schematic: "Stillsuit_Unique_Armored_01_Boots_Schematic", qty: 1 },
  { id: "6", label: "Hollower Stillsuit Garment", schematic: "Stillsuit_Unique_Armored_01_Top_Schematic", qty: 1 },
  { id: "7", label: "Hollower Stillsuit Gloves", schematic: "Stillsuit_Unique_Armored_01_Gloves_Schematic", qty: 1 },
  { id: "8", label: "Hollower Stillsuit Mask", schematic: "Stillsuit_Unique_Armored_01_Mask_Schematic", qty: 1 },
  { id: "9", label: "Mohandis Sandbike Engine Mk1", schematic: "SandbikeEngine_Unique_Speed_1_Schematic", qty: 1 },
  { id: "10", label: "Old Sparky Mk1", schematic: "PowerPack_Unique_Regen_01_Schematic", qty: 1 },
  { id: "11", label: "Sim's Cutter", schematic: "Schematic_UniqueCutteray2", qty: 1 },
  { id: "12", label: "Way of the Fallen", schematic: "Schematic_UniqueMaulaPistol", qty: 1 },
  { id: "13", label: "Spice-infused Copper Dust", schematic: "T1UniqueComponent", qty: 1000 },
  { id: "14", label: "Plasteel Plate", schematic: "T3MaterialComponent", qty: 250 },
  { id: "15", label: "Hydration Pack", schematic: "Consumable_WaterPack_Schematic", qty: 5 },
]

export const taskRuns: TaskRun[] = [
  { id: "#79859", task: "backup", status: "success", when: "06/10/2026 12:00:00", duration: "5.7s" },
  { id: "#79073", task: "welcome-package", status: "failed", when: "06/10/2026 11:34:12", duration: "0.1s" },
  { id: "#79072", task: "welcome-package", status: "failed", when: "06/10/2026 11:34:09", duration: "0.1s" },
  { id: "#79071", task: "welcome-package", status: "failed", when: "06/10/2026 11:34:07", duration: "0.1s" },
  { id: "#79070", task: "welcome-package", status: "failed", when: "06/10/2026 11:34:04", duration: "1.0s" },
  { id: "#79069", task: "welcome-package", status: "failed", when: "06/10/2026 11:34:02", duration: "0.2s" },
  { id: "#79068", task: "welcome-package", status: "failed", when: "06/10/2026 11:34:00", duration: "0.1s" },
  { id: "#79067", task: "welcome-package", status: "failed", when: "06/10/2026 11:33:57", duration: "0.7s" },
  { id: "#79066", task: "welcome-package", status: "failed", when: "06/10/2026 11:33:54", duration: "1.0s" },
  { id: "#79065", task: "welcome-package", status: "failed", when: "06/10/2026 11:33:52", duration: "0.2s" },
]

export const publishes: PublishRow[] = [
  { cmd: "AddItemToInventory", ok: true, when: "12:09:34" },
  { cmd: "AddItemToInventory", ok: true, when: "11:56:31" },
  { cmd: "AddItemToInventory", ok: true, when: "21:39:00" },
  { cmd: "AddItemToInventory", ok: true, when: "21:31:20" },
  { cmd: "AddItemToInventory", ok: true, when: "21:18:55" },
  { cmd: "AddItemToInventory", ok: true, when: "21:11:04" },
  { cmd: "AddItemToInventory", ok: true, when: "21:10:46" },
  { cmd: "UpdateAllWaterFillables", ok: true, when: "20:52:59" },
  { cmd: "UpdateAllWaterFillables", ok: true, when: "20:52:45" },
  { cmd: "UpdateAllWaterFillables", ok: true, when: "20:52:38" },
  { cmd: "UpdateAllWaterFillables", ok: true, when: "20:52:32" },
  { cmd: "UpdateAllWaterFillables", ok: true, when: "20:52:19" },
  { cmd: "AddItemToInventory", ok: true, when: "20:50:29" },
  { cmd: "AddItemToInventory", ok: true, when: "20:49:12" },
  { cmd: "AddItemToInventory", ok: true, when: "20:47:51" },
]

export const schedule = {
  autoRestart: "disabled",
  dailyRestart: "05:00",
  timezone: "America/Phoenix",
  warningLead: "300s",
  warningFrequency: "300s",
  autoUpdate: "enabled",
  updateApplyLead: "300s",
  autoBackup: "enabled",
  backupCron: "0 */4 * * *",
}

export const packageVersions = {
  installedBuild: "23654991",
  downloaded: "1988751-0-shipping",
  running: "1988751-0-shipping",
  operator: "v1.5.0",
}

export type FieldKind = "string" | "int" | "float" | "bool" | "select" | "text"
export interface FieldSpec {
  key: string
  label: string
  kind: FieldKind
  required?: boolean
  helper?: string
  options?: { value: string; label: string }[]
}
export interface CommandSpec {
  id: string
  label: string
  group: string
  destructive?: boolean
  needsPlayer: boolean
  allowAllPlayers: boolean
  describe: string
  fields: FieldSpec[]
}

export const commandPlayers = [
  { flsId: "431C7B16E03F3F97", name: "Maren Shai", online: false },
  { flsId: "9F2A11C4D8E07B36", name: "Duncan Idaho", online: true },
  { flsId: "AC50E931F7B2148D", name: "Chani Kynes", online: true },
]

export const commandCatalog: { group: string; items: CommandSpec[] }[] = [
  {
    group: "Broadcast",
    items: [
      {
        id: "broadcast",
        label: "Broadcast message",
        group: "Broadcast",
        needsPlayer: false,
        allowAllPlayers: false,
        describe: "Send a server-wide message banner to every connected player.",
        fields: [
          { key: "message", label: "Message", kind: "text", required: true, helper: "Shown to all players." },
          { key: "durationS", label: "Duration (s)", kind: "int", helper: "How long the banner stays on screen." },
        ],
      },
    ],
  },
  {
    group: "Inventory",
    items: [
      {
        id: "grant-item",
        label: "Grant item",
        group: "Inventory",
        needsPlayer: true,
        allowAllPlayers: true,
        describe: "Add an item or schematic to the target player's inventory.",
        fields: [
          { key: "schematic", label: "Schematic ID", kind: "string", required: true },
          { key: "qty", label: "Quantity", kind: "int", required: true },
        ],
      },
      {
        id: "clean-inventory",
        label: "Clean inventory",
        group: "Inventory",
        destructive: true,
        needsPlayer: true,
        allowAllPlayers: false,
        describe: "Permanently remove every item from the target player's inventory.",
        fields: [],
      },
    ],
  },
  {
    group: "Currency",
    items: [
      {
        id: "grant-currency",
        label: "Grant currency",
        group: "Currency",
        needsPlayer: true,
        allowAllPlayers: true,
        describe: "Grant Solari or Intel to the target player.",
        fields: [
          {
            key: "type",
            label: "Currency",
            kind: "select",
            required: true,
            options: [
              { value: "solari", label: "Solari" },
              { value: "intel", label: "Intel" },
            ],
          },
          { key: "amount", label: "Amount", kind: "int", required: true },
        ],
      },
    ],
  },
  {
    group: "Player Ops",
    items: [
      {
        id: "kick",
        label: "Kick player",
        group: "Player Ops",
        needsPlayer: true,
        allowAllPlayers: false,
        describe: "Disconnect the target player from the server.",
        fields: [{ key: "reason", label: "Reason", kind: "string" }],
      },
      {
        id: "reset-progression",
        label: "Reset progression",
        group: "Player Ops",
        destructive: true,
        needsPlayer: true,
        allowAllPlayers: false,
        describe: "Wipe the target player's skill progression. This cannot be undone.",
        fields: [],
      },
      {
        id: "refill-water",
        label: "Refill water",
        group: "Player Ops",
        needsPlayer: true,
        allowAllPlayers: true,
        describe: "Refill the target player's hydration and water fillables.",
        fields: [],
      },
    ],
  },
  {
    group: "Progression",
    items: [
      {
        id: "award-xp",
        label: "Award XP",
        group: "Progression",
        needsPlayer: true,
        allowAllPlayers: true,
        describe: "Grant experience points to the target player.",
        fields: [{ key: "xp", label: "XP", kind: "int", required: true }],
      },
      {
        id: "set-skill",
        label: "Set skill module level",
        group: "Progression",
        needsPlayer: true,
        allowAllPlayers: false,
        describe: "Force a specific skill module to a given level.",
        fields: [
          { key: "module", label: "Module", kind: "string", required: true },
          { key: "level", label: "Level", kind: "int", required: true },
        ],
      },
    ],
  },
  {
    group: "Teleport & Spawn",
    items: [
      {
        id: "teleport-safe",
        label: "Teleport (safe)",
        group: "Teleport & Spawn",
        needsPlayer: true,
        allowAllPlayers: false,
        describe: "Teleport the player to the nearest safe location near the given coordinates.",
        fields: [
          { key: "x", label: "X", kind: "float", required: true },
          { key: "y", label: "Y", kind: "float", required: true },
          { key: "z", label: "Z", kind: "float", required: true },
        ],
      },
      {
        id: "spawn-vehicle",
        label: "Spawn vehicle",
        group: "Teleport & Spawn",
        needsPlayer: true,
        allowAllPlayers: false,
        describe: "Spawn a vehicle next to the target player.",
        fields: [{ key: "vehicle", label: "Vehicle ID", kind: "string", required: true }],
      },
    ],
  },
  {
    group: "Server Scripts",
    items: [
      {
        id: "cheat-raw",
        label: "Cheat script (raw)",
        group: "Server Scripts",
        destructive: true,
        needsPlayer: false,
        allowAllPlayers: false,
        describe: "Execute a raw cheat command against the server. Unvalidated — use with care.",
        fields: [{ key: "script", label: "Script", kind: "text", required: true }],
      },
      {
        id: "server-exec",
        label: "Server exec (raw)",
        group: "Server Scripts",
        destructive: true,
        needsPlayer: false,
        allowAllPlayers: false,
        describe: "Run a raw server exec command. Unvalidated — use with care.",
        fields: [{ key: "command", label: "Command", kind: "text", required: true }],
      },
    ],
  },
]

export const adminCommands = [
  { group: "Broadcast", items: [{ label: "Broadcast", destructive: false }] },
  { group: "Inventory", items: [{ label: "Grant item", destructive: false }] },
  {
    group: "Player Ops",
    items: [
      { label: "Kick player", destructive: false },
      { label: "Clean inventory", destructive: true },
      { label: "Reset progression", destructive: true },
      { label: "Refill water", destructive: false },
    ],
  },
  {
    group: "Progression",
    items: [
      { label: "Award XP", destructive: false },
      { label: "Set skill module level", destructive: false },
      { label: "Set unspent skill points", destructive: false },
    ],
  },
  {
    group: "Teleport & Spawn",
    items: [
      { label: "Teleport (safe)", destructive: false },
      { label: "Teleport (exact)", destructive: false },
      { label: "Spawn vehicle", destructive: false },
    ],
  },
  {
    group: "Server Scripts",
    items: [
      { label: "Cheat script (raw)", destructive: true },
      { label: "Server exec (raw)", destructive: true },
    ],
  },
]
