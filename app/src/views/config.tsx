import type {
  BattleGroupDetail,
  DirectorMapSummary,
  FlsDraft,
  MapOverrideDraft,
  TransferDraft
} from "../types";
import { DirectorConfigPanel } from "./config/directorSettings";
import { LiveConfigPanel } from "./config/liveConfig";
import { MapOverridePanel } from "./config/mapOverride";

type ConfigViewProps = {
  battleGroupDetail: BattleGroupDetail | null;
  directorAvailable: boolean;
  directorFlsConfig: Record<string, unknown> | null;
  directorTransferConfig: Record<string, unknown> | null;
  directorMaps: DirectorMapSummary[];
  selectedDirectorMapSummary: DirectorMapSummary | null;
  flsDraft: FlsDraft;
  transferDraft: TransferDraft;
  mapOverrideDraft: MapOverrideDraft;
  busy: boolean;
  onFlsDraftChange: (draft: FlsDraft) => void;
  onTransferDraftChange: (draft: TransferDraft) => void;
  onMapOverrideDraftChange: (draft: MapOverrideDraft) => void;
  onSaveFlsConfig: () => void;
  onClearFlsConfig: () => void;
  onSaveTransferConfig: () => void;
  onClearTransferConfig: () => void;
  onSelectMap: (mapName: string) => void;
  onSaveMapOverride: () => void;
  onClearMapOverride: (mapName: string) => void;
};

export function ConfigView({
  battleGroupDetail,
  directorAvailable,
  directorFlsConfig,
  directorTransferConfig,
  directorMaps,
  selectedDirectorMapSummary,
  flsDraft,
  transferDraft,
  mapOverrideDraft,
  busy,
  onFlsDraftChange,
  onTransferDraftChange,
  onMapOverrideDraftChange,
  onSaveFlsConfig,
  onClearFlsConfig,
  onSaveTransferConfig,
  onClearTransferConfig,
  onSelectMap,
  onSaveMapOverride,
  onClearMapOverride
}: ConfigViewProps) {
  return (
    <>
      <LiveConfigPanel battleGroupDetail={battleGroupDetail} />
      {directorAvailable && (
        <DirectorConfigPanel
          directorFlsConfig={directorFlsConfig}
          directorTransferConfig={directorTransferConfig}
          flsDraft={flsDraft}
          transferDraft={transferDraft}
          busy={busy}
          onFlsDraftChange={onFlsDraftChange}
          onTransferDraftChange={onTransferDraftChange}
          onSaveFlsConfig={onSaveFlsConfig}
          onClearFlsConfig={onClearFlsConfig}
          onSaveTransferConfig={onSaveTransferConfig}
          onClearTransferConfig={onClearTransferConfig}
        />
      )}
      {directorAvailable && selectedDirectorMapSummary && (
        <MapOverridePanel
          directorMaps={directorMaps}
          selectedDirectorMapSummary={selectedDirectorMapSummary}
          mapOverrideDraft={mapOverrideDraft}
          busy={busy}
          onSelectMap={onSelectMap}
          onMapOverrideDraftChange={onMapOverrideDraftChange}
          onSaveMapOverride={onSaveMapOverride}
          onClearMapOverride={onClearMapOverride}
        />
      )}
    </>
  );
}
