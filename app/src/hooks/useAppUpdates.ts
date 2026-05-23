import { useEffect, useRef, useState } from "react";

import {
  checkForUpdate,
  downloadAndInstallUpdate,
  type DownloadEvent,
  type Update,
} from "../services/updater";
import { relaunch } from "../services/tauri";
import type { LogRow } from "../types/log";
import type { UpdateStatus } from "../types/update";
import { errorMessage } from "../utils/errors";
import { formatBytes } from "../utils/formatting";
import { log } from "../utils/logging";

const startupUpdateChecksEnabled = import.meta.env.VITE_ENABLE_STARTUP_UPDATE_CHECK === "true";

type UseAppUpdatesArgs = {
  appendLogRow: (row: LogRow) => void;
};

export function useAppUpdates({ appendLogRow }: UseAppUpdatesArgs) {
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<string | null>(null);
  const updateCheckInFlight = useRef(false);

  const checkForAppUpdate = async () => {
    if (updateCheckInFlight.current) return;
    updateCheckInFlight.current = true;
    setUpdateStatus("checking");
    setUpdateProgress(null);
    appendLogRow(log.info("updates", "Checking for app updates."));
    try {
      const nextUpdate = await checkForUpdate(15_000);
      setAvailableUpdate(nextUpdate);
      if (nextUpdate) {
        setUpdateStatus("available");
        appendLogRow(
          log.info(
            "updates",
            `Update ${nextUpdate.version} is available; current version is ${nextUpdate.currentVersion}.`,
          ),
        );
        setUpdateDialogOpen(true);
      } else {
        setUpdateStatus("current");
        appendLogRow(log.info("updates", "The app is up to date."));
      }
    } catch (err) {
      setUpdateStatus("failed");
      appendLogRow(log.warn("updates", `Update check failed: ${errorMessage(err)}`));
    } finally {
      updateCheckInFlight.current = false;
    }
  };

  const installAppUpdate = async () => {
    if (!availableUpdate) return;
    let downloaded = 0;
    let total: number | null = null;
    setUpdateStatus("installing");
    setUpdateProgress("Preparing download...");
    appendLogRow(log.info("updates", `Installing update ${availableUpdate.version}.`));
    try {
      await downloadAndInstallUpdate(
        availableUpdate,
        (event: DownloadEvent) => {
          if (event.event === "Started") {
            total = event.data.contentLength ?? null;
            downloaded = 0;
            setUpdateProgress(total ? `Downloading 0 of ${formatBytes(total)}` : "Downloading update...");
          }
          if (event.event === "Progress") {
            downloaded += event.data.chunkLength;
            setUpdateProgress(
              total
                ? `Downloading ${formatBytes(downloaded)} of ${formatBytes(total)}`
                : `Downloading ${formatBytes(downloaded)}`,
            );
          }
          if (event.event === "Finished") {
            setUpdateProgress("Installing update...");
          }
        },
        120_000,
      );
      setUpdateStatus("relaunching");
      setUpdateProgress("Relaunching...");
      appendLogRow(log.info("updates", "Update installed; relaunching the app."));
      await relaunch();
    } catch (err) {
      setUpdateStatus("failed");
      setUpdateProgress(null);
      appendLogRow(log.error("updates", errorMessage(err)));
    }
  };

  useEffect(() => {
    if (!startupUpdateChecksEnabled) {
      appendLogRow(log.debug("updates", "Automatic update checks are disabled for this local build."));
      return;
    }
    void checkForAppUpdate();
  }, []);

  return {
    availableUpdate,
    updateStatus,
    updateDialogOpen,
    setUpdateDialogOpen,
    updateProgress,
    checkForAppUpdate,
    installAppUpdate,
  };
}
