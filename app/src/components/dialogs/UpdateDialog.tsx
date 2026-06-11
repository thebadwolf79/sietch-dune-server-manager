import type { ComponentPropsWithoutRef } from "react";
import Markdown from "markdown-to-jsx";
import { AlertDialog, Box, Button, Flex, Link, Text } from "@radix-ui/themes";

import { openExternal } from "../../services/tauri";
import type { Update } from "../../services/updater";
import type { UpdateStatus } from "../../types/update";

const RELEASES_URL = "https://github.com/thebadwolf79/sietch-dune-server-manager/releases";

// Links inside the release notes must open in the system browser, not navigate
// the Tauri webview away from the app.
function NotesLink({ href, children }: ComponentPropsWithoutRef<"a">) {
  return (
    <Link
      size="2"
      href={href}
      onClick={(e) => {
        e.preventDefault();
        if (href) void openExternal(href);
      }}
    >
      {children}
    </Link>
  );
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
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Install app update?</AlertDialog.Title>
        <AlertDialog.Description size="2">
          {update
            ? `Version ${update.version} is available. The app will download the signed installer, install it, and relaunch.`
            : "No update is currently selected."}
        </AlertDialog.Description>
        {update?.body ? (
          <Box mt="3">
            <Text size="2" weight="medium">
              What&apos;s new
            </Text>
            {/* Render the release notes as markdown, bounded with scroll so a
                long changelog can never push the dialog past the viewport. */}
            <Box className="release-notes-md">
              <Markdown options={{ forceBlock: true, overrides: { a: NotesLink } }}>
                {update.body}
              </Markdown>
            </Box>
            <Flex mt="2">
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
