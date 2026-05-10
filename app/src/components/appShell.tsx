import { Activity, Database, HardDrive, RefreshCw, Server, ShieldCheck, Terminal, RadioTower } from "lucide-react";
import type { NavItem, ViewKey } from "../types";
import { StatusLamp } from "./primitives";

type SidebarProps = {
  navItems: NavItem[];
  activeView: ViewKey;
  onSelect: (view: ViewKey) => void;
};

export function AppSidebar({ navItems, activeView, onSelect }: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <Server size={26} />
        <div>
          <strong>Dune Dedicated</strong>
          <span>Server Manager</span>
        </div>
      </div>
      <nav>
        {navItems.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.key}
              className={`${activeView === item.key ? "active" : ""} ${item.disabled ? "disabled" : ""}`}
              disabled={item.disabled}
              onClick={() => onSelect(item.key)}
            >
              <Icon size={16} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}

type AppHeaderProps = {
  title: string;
  subtitle: string;
  busy: boolean;
  onRefresh: () => void;
};

export function AppHeader({ title, subtitle, busy, onRefresh }: AppHeaderProps) {
  return (
    <header className="topbar">
      <div>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
      <button className="primary" onClick={onRefresh} disabled={busy}>
        <RefreshCw size={17} />
        Refresh
      </button>
    </header>
  );
}

type StatusStripProps = {
  admin: boolean;
  vmState?: string;
  sshConnected: boolean;
  kubectlReady: boolean;
  battleGroupPhase?: string;
  managerReadiness: string;
};

export function StatusStrip({
  admin,
  vmState,
  sshConnected,
  kubectlReady,
  battleGroupPhase,
  managerReadiness
}: StatusStripProps) {
  return (
    <section className="status-strip">
      <div>
        <ShieldCheck size={18} />
        <span>Admin</span>
        <StatusLamp label="Admin" value={admin} />
      </div>
      <div>
        <HardDrive size={18} />
        <span>VM</span>
        <StatusLamp label="VM" value={vmState} />
      </div>
      <div>
        <Terminal size={18} />
        <span>SSH</span>
        <StatusLamp label="SSH" value={sshConnected} />
      </div>
      <div>
        <Database size={18} />
        <span>k3s</span>
        <StatusLamp label="k3s" value={kubectlReady} />
      </div>
      <div>
        <Activity size={18} />
        <span>BattleGroup</span>
        <StatusLamp label="BattleGroup" value={battleGroupPhase} />
      </div>
      <div>
        <RadioTower size={18} />
        <span>Manager API</span>
        <StatusLamp label="Manager API" value={managerReadiness} />
      </div>
    </section>
  );
}
