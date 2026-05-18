import { Card, Flex, Box, Heading, Text, Button, TextArea, Grid } from "@radix-ui/themes";
import { DesktopIcon } from "@radix-ui/react-icons";
import { type GenerateSshKeyResult } from "../types";
import { SetupSection } from "./Common";

export function ToolsPage({
  generatedSshKey,
  sshKeyGenerationRunning,
  onGenerateUbuntuSshKey,
}: {
  generatedSshKey: GenerateSshKeyResult | null;
  sshKeyGenerationRunning: boolean;
  onGenerateUbuntuSshKey: () => void;
}) {
  return (
    <Card size="3" variant="surface" className="pane setup-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Box>
          <Heading size="5">Tools</Heading>
          <Text as="p" size="2" color="gray">
            Utilities for preparing hosts before server setup.
          </Text>
        </Box>
        <Box className="setup-scroll" style={{ flexGrow: 1, overflowY: "auto" }}>
          <SetupSection icon={DesktopIcon} title="Ubuntu SSH Key Pair">
            <Flex direction="column" gap="3">
              <Text size="2" color="gray">
                Generate an Ed25519 key pair for Ubuntu VPS setup. Upload the public key to your hosting provider,
                then use the private key path during Remote Ubuntu detection.
              </Text>
              <Button
                type="button"
                variant="surface"
                onClick={onGenerateUbuntuSshKey}
                disabled={sshKeyGenerationRunning}
              >
                {sshKeyGenerationRunning ? "Generating..." : "Generate SSH key pair"}
              </Button>
              {generatedSshKey ? (
                <Box className="generated-key-box" mt="2">
                  <Text as="div" size="2" weight="medium" mb="1">
                    Public key to upload to your host
                  </Text>
                  <TextArea readOnly value={generatedSshKey.publicKey} rows={4} />
                  <Grid columns="140px 1fr" gap="2" mt="3">
                    <Text size="2" color="gray">
                      Private key
                    </Text>
                    <Text size="2" className="mono metric-value">
                      {generatedSshKey.privateKeyPath}
                    </Text>
                    <Text size="2" color="gray">
                      Public key
                    </Text>
                    <Text size="2" className="mono metric-value">
                      {generatedSshKey.publicKeyPath}
                    </Text>
                  </Grid>
                </Box>
              ) : null}
            </Flex>
          </SetupSection>
        </Box>
      </Flex>
    </Card>
  );
}
