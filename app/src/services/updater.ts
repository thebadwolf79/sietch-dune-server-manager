import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";

export type { DownloadEvent, Update } from "@tauri-apps/plugin-updater";

export async function checkForUpdate(timeoutMs = 15_000): Promise<Update | null> {
  return check({ timeout: timeoutMs });
}

export async function downloadAndInstallUpdate(
  update: Update,
  onEvent: (event: DownloadEvent) => void,
  timeoutMs = 120_000,
): Promise<void> {
  await update.downloadAndInstall(onEvent, { timeout: timeoutMs });
}
