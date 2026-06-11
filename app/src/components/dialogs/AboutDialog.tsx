import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { InfoCircledIcon } from "@radix-ui/react-icons";
import { Button, Dialog, Flex, IconButton, Link, Text } from "@radix-ui/themes";

import { openExternal } from "../../services/tauri";

const REPO_URL = "https://github.com/thebadwolf79/sietch-dune-server-manager";
const ISSUES_URL = `${REPO_URL}/issues`;
const UPSTREAM_URL = "https://github.com/adainrivers/dune-dedicated-server-manager";

/**
 * Small info button (sits next to "Check for updates") that opens an About
 * modal showing the app version and links back to the project. Self-contained:
 * owns its open state so it can be dropped into the header without prop
 * threading.
 */
export default function AboutDialog() {
  const [open, setOpen] = useState(false);
  const [version, setVersion] = useState<string | null>(null);

  // The bundled dune-server-service ships with the same version as the app,
  // so this number identifies both. Fetched from the Tauri runtime rather than
  // package.json so it reflects the actually-installed build.
  useEffect(() => {
    let active = true;
    void getVersion()
      .then((v) => {
        if (active) setVersion(v);
      })
      .catch(() => {
        if (active) setVersion(null);
      });
    return () => {
      active = false;
    };
  }, []);

  const openLink = (url: string) => () => {
    void openExternal(url);
  };

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger>
        <IconButton size="1" variant="surface" aria-label="About this app" title="About">
          <InfoCircledIcon />
        </IconButton>
      </Dialog.Trigger>
      <Dialog.Content maxWidth="460px">
        <Dialog.Title>About Sietch</Dialog.Title>
        <Dialog.Description size="2" style={{ color: "var(--color-text-muted)" }}>
          Sietch — Dune Dedicated Server Manager · unofficial community fork
        </Dialog.Description>

        <Flex direction="column" gap="3" mt="4">
          <Flex justify="between" align="center">
            <Text size="2" color="gray">
              Version
            </Text>
            <Text size="2" className="mono">
              {version ?? "—"}
            </Text>
          </Flex>

          <Flex justify="between" align="center">
            <Text size="2" color="gray">
              Repository
            </Text>
            <Link size="2" href={REPO_URL} onClick={(e) => { e.preventDefault(); openLink(REPO_URL)(); }}>
              GitHub
            </Link>
          </Flex>

          <Flex justify="between" align="center">
            <Text size="2" color="gray">
              Found a bug?
            </Text>
            <Link size="2" href={ISSUES_URL} onClick={(e) => { e.preventDefault(); openLink(ISSUES_URL)(); }}>
              Report an issue
            </Link>
          </Flex>

          <Flex justify="between" align="center">
            <Text size="2" color="gray">
              Built on
            </Text>
            <Link size="2" href={UPSTREAM_URL} onClick={(e) => { e.preventDefault(); openLink(UPSTREAM_URL)(); }}>
              gaming.tools (upstream)
            </Link>
          </Flex>
        </Flex>

        <Flex gap="3" justify="end" mt="5">
          <Dialog.Close>
            <Button variant="soft" color="gray">
              Close
            </Button>
          </Dialog.Close>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
