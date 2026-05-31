import { AlertDialog, Box, Button, Flex, Link, Text, TextArea } from "@radix-ui/themes";

import { openExternal } from "../../services/tauri";
import type { Update } from "../../services/updater";
import type { UpdateStatus } from "../../types/update";

const RELEASES_URL = "https://github.com/adainrivers/dune-dedicated-server-manager/releases";
const MAX_NOTES = 6;

// Pull the bullet lines out of the release-notes markdown so the dialog shows a
// short, tidy list of changes instead of the raw "## Fixed / - foo" markdown.
function summarizeNotes(body: string): string[] {
  return body
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^[-*]\s+/.test(line))
    .map((line) => line.replace(/^[-*]\s+/, "").replace(/`/g, "").trim())
    .filter(Boolean);
}

export type UpdateDialogProps = {
  open: boolean;
  update: Update | null;
  status: UpdateStatus;
  progress: string | null;
  onOpenChange: (open: boolean) => void;
  onInstall: () => void;
};

export default function UpdateDialog({
  open,
  update,
  status,
  progress,
  onOpenChange,
  onInstall,
}: UpdateDialogProps) {
  const busy = status === "installing" || status === "relaunching";
  const notes = update?.body ? summarizeNotes(update.body) : [];
  const shownNotes = notes.slice(0, MAX_NOTES);
  const extraNotes = notes.length - shownNotes.length;
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Install app update?</AlertDialog.Title>
        <AlertDialog.Description size="2">
          {update
            ? `Version ${update.version} is available. The app will download the signed installer, install it, and relaunch.`
            : "No update is currently selected."}
        </AlertDialog.Description>
        {notes.length > 0 ? (
          <Box mt="3">
            <Text size="2" weight="medium">
              What&apos;s new
            </Text>
            <ul
              style={{
                margin: "var(--space-1) 0 0",
                paddingLeft: "1.1rem",
                maxHeight: 180,
                overflowY: "auto",
              }}
            >
              {shownNotes.map((item, i) => (
                <li key={i}>
                  <Text size="2">{item}</Text>
                </li>
              ))}
            </ul>
            <Flex mt="2" gap="3" align="center" wrap="wrap">
              {extraNotes > 0 ? (
                <Text size="1" color="gray">
                  +{extraNotes} more
                </Text>
              ) : null}
              <Link
                size="1"
                href={RELEASES_URL}
                onClick={(e) => {
                  e.preventDefault();
                  void openExternal(RELEASES_URL);
                }}
              >
                Full release notes
              </Link>
            </Flex>
          </Box>
        ) : update?.body ? (
          <TextArea mt="3" value={update.body} readOnly rows={7} />
        ) : null}
        {progress ? (
          <Text as="p" size="2" color="gray" mt="3" className="mono">
            {progress}
          </Text>
        ) : null}
        <Flex gap="3" mt="4" justify="end">
          <AlertDialog.Cancel disabled={busy}>
            <Button variant="soft" color="gray" disabled={busy}>
              Later
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action disabled={!update || busy}>
            <Button disabled={!update || busy} onClick={onInstall}>
              {busy ? "Installing..." : "Install update"}
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}
