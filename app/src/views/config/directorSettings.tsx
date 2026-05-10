import { Map } from "lucide-react";
import type { FlsDraft, TransferDraft } from "../../types";

export type DirectorConfigPanelProps = {
  directorFlsConfig: Record<string, unknown> | null;
  directorTransferConfig: Record<string, unknown> | null;
  flsDraft: FlsDraft;
  transferDraft: TransferDraft;
  busy: boolean;
  onFlsDraftChange: (draft: FlsDraft) => void;
  onTransferDraftChange: (draft: TransferDraft) => void;
  onSaveFlsConfig: () => void;
  onClearFlsConfig: () => void;
  onSaveTransferConfig: () => void;
  onClearTransferConfig: () => void;
};

export function DirectorConfigPanel({
  directorFlsConfig,
  directorTransferConfig,
  flsDraft,
  transferDraft,
  busy,
  onFlsDraftChange,
  onTransferDraftChange,
  onSaveFlsConfig,
  onClearFlsConfig,
  onSaveTransferConfig,
  onClearTransferConfig
}: DirectorConfigPanelProps) {
  return (
    <section className="panel">
      <div className="panel-title">
        <h2>Director Config</h2>
        <Map size={19} />
      </div>
      <section className="native-config-grid">
        <div className="native-config-box">
          <div className="mini-title">
            <strong>FLS Report Settings</strong>
            <span>{directorFlsConfig?.webOverrideConfig ? "Override active" : "Base config"}</span>
          </div>
          <label>
            Heartbeat update seconds
            <input
              type="number"
              min="1"
              value={flsDraft.heartbeatSeconds}
              onChange={(event) => onFlsDraftChange({ ...flsDraft, heartbeatSeconds: event.target.value })}
            />
          </label>
          <label>
            Settings update seconds
            <input
              type="number"
              min="1"
              value={flsDraft.settingsSeconds}
              onChange={(event) => onFlsDraftChange({ ...flsDraft, settingsSeconds: event.target.value })}
            />
          </label>
          <div className="button-row">
            <button onClick={onSaveFlsConfig} disabled={busy || !flsDraft.heartbeatSeconds || !flsDraft.settingsSeconds}>
              Update
            </button>
            <button onClick={onClearFlsConfig} disabled={busy}>
              Clear Override
            </button>
          </div>
        </div>

        <div className="native-config-box">
          <div className="mini-title">
            <strong>Character Transfer</strong>
            <span>{directorTransferConfig?.webOverrideConfig ? "Override active" : "Base config"}</span>
          </div>
          <div className="form-grid">
            <label>
              Incoming
              <select
                value={transferDraft.incoming}
                onChange={(event) => onTransferDraftChange({ ...transferDraft, incoming: event.target.value })}
              >
                <option value="0">Default</option>
                <option value="10">Deny all incoming</option>
                <option value="20">Accept private</option>
                <option value="30">Accept official</option>
                <option value="40">Accept all</option>
              </select>
            </label>
            <label>
              Export timeout
              <input
                type="number"
                min="1"
                value={transferDraft.exportTimeout}
                onChange={(event) => onTransferDraftChange({ ...transferDraft, exportTimeout: event.target.value })}
              />
            </label>
            <label>
              Import timeout
              <input
                type="number"
                min="1"
                value={transferDraft.importTimeout}
                onChange={(event) => onTransferDraftChange({ ...transferDraft, importTimeout: event.target.value })}
              />
            </label>
            <label>
              Validate timeout
              <input
                type="number"
                min="1"
                value={transferDraft.validateTimeout}
                onChange={(event) => onTransferDraftChange({ ...transferDraft, validateTimeout: event.target.value })}
              />
            </label>
          </div>
          <div className="toggle-grid">
            <label><input type="checkbox" checked={transferDraft.deleteOrigin} onChange={(event) => onTransferDraftChange({ ...transferDraft, deleteOrigin: event.target.checked })} /> Delete origin</label>
            <label><input type="checkbox" checked={transferDraft.outgoing} onChange={(event) => onTransferDraftChange({ ...transferDraft, outgoing: event.target.checked })} /> Outgoing</label>
            <label><input type="checkbox" checked={transferDraft.freeFrom} onChange={(event) => onTransferDraftChange({ ...transferDraft, freeFrom: event.target.checked })} /> Free from</label>
            <label><input type="checkbox" checked={transferDraft.freeTo} onChange={(event) => onTransferDraftChange({ ...transferDraft, freeTo: event.target.checked })} /> Free to</label>
            <label><input type="checkbox" checked={transferDraft.worldClosed} onChange={(event) => onTransferDraftChange({ ...transferDraft, worldClosed: event.target.checked })} /> World closed</label>
            <label><input type="checkbox" checked={transferDraft.worldClosingSoon} onChange={(event) => onTransferDraftChange({ ...transferDraft, worldClosingSoon: event.target.checked })} /> Closing soon</label>
          </div>
          <div className="button-row">
            <button
              onClick={onSaveTransferConfig}
              disabled={busy || !transferDraft.exportTimeout || !transferDraft.importTimeout || !transferDraft.validateTimeout}
            >
              Update
            </button>
            <button onClick={onClearTransferConfig} disabled={busy}>
              Clear Override
            </button>
          </div>
        </div>
      </section>
    </section>
  );
}
