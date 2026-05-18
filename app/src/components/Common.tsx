import { type ReactNode, type ComponentType } from "react";
import { Box, Flex, Grid, Heading, Text, Badge, Button, Card, Table, Spinner } from "@radix-ui/themes";
import { type RemoteServerPackageStatus, type ServerPackageStatus, type RemoteServerComponent } from "../types";
import { playerPortForwards } from "../utils/helpers";

export function BusySpinner() {
  return <Spinner size="1" />;
}

export function SetupSection({
  className,
  disabled,
  icon: Icon,
  title,
  children,
}: {
  className?: string;
  disabled?: boolean;
  icon: ComponentType<{ width?: number | string; height?: number | string }>;
  title: string;
  children: ReactNode;
}) {
  return (
    <Box className={["setup-section", className, disabled ? "is-flow-disabled" : ""].filter(Boolean).join(" ")} aria-disabled={disabled}>
      <Flex align="center" gap="2" mb="3">
        <Icon width="17" height="17" />
        <Heading size="3">{title}</Heading>
      </Flex>
      <Flex direction="column" gap="3">
        {children}
      </Flex>
    </Box>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Box>
      <Text as="label" size="2" weight="medium" mb="1" className="field-label">
        {label}
      </Text>
      {children}
    </Box>
  );
}

export function FormRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Grid columns="130px minmax(0, 1fr)" gap="3" align="start" className="form-row">
      <Text size="2" weight="medium" mt="2">
        {label}
      </Text>
      <Box>{children}</Box>
    </Grid>
  );
}

export function LayoutRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Grid columns="minmax(180px, 1fr) 210px" gap="3" align="center" className="layout-row">
      <Text size="2" weight="medium">
        {label}
      </Text>
      <Box>{children}</Box>
    </Grid>
  );
}

export function InfoRow({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "green" | "amber" | "red";
}) {
  return (
    <Grid columns="160px 1fr auto" gap="3" align="center" className="info-row">
      <Text as="div" size="2" color="gray">
        {label}
      </Text>
      <Text as="div" size="2" className="mono metric-value">
        {value}
      </Text>
      <Badge color={tone} variant="soft">
        {tone === "green" ? "OK" : tone === "red" ? "Issue" : "Check"}
      </Badge>
    </Grid>
  );
}

export function InfoActionRow({
  label,
  value,
  tone,
  actionLabel,
  disabled,
  onAction,
}: {
  label: string;
  value: string;
  tone: "green" | "amber" | "red";
  actionLabel: string;
  disabled: boolean;
  onAction: () => void;
}) {
  return (
    <Grid columns="160px 1fr auto auto" gap="3" align="center" className="info-row">
      <Text as="div" size="2" color="gray">
        {label}
      </Text>
      <Text as="div" size="2" className="mono metric-value">
        {value}
      </Text>
      <Badge color={tone} variant="soft">
        {tone === "green" ? "OK" : tone === "red" ? "Issue" : "Check"}
      </Badge>
      <Button size="1" variant="soft" color="bronze" disabled={disabled} onClick={onAction}>
        {actionLabel}
      </Button>
    </Grid>
  );
}

export function Metric({ label, value }: { label: string; value: string }) {
  return (
    <Box>
      <Text as="div" size="1" color="gray">
        {label}
      </Text>
      <Text as="div" size="2" className="mono metric-value">
        {value}
      </Text>
    </Box>
  );
}

export function InlineRequirement({ ok, text }: { ok: boolean; text: string }) {
  return (
    <Flex align="center" gap="2" mt="2">
      <Badge color={ok ? "green" : "amber"} variant="soft">
        {ok ? "Enough" : "Needs attention"}
      </Badge>
      <Text size="2" color="gray">
        {text}
      </Text>
    </Flex>
  );
}

export function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <Flex direction="column" align="center" justify="center" style={{ flexGrow: 1, padding: "var(--space-6)" }} className="empty-state">
      <Heading size="3" mb="2">
        {title}
      </Heading>
      <Text as="p" size="2" color="gray" align="center" style={{ maxWidth: 360 }}>
        {body}
      </Text>
    </Flex>
  );
}

export function ServerPackageCardStatus({
  guestPackage,
  packageStatus,
}: {
  guestPackage?: RemoteServerPackageStatus;
  packageStatus: ServerPackageStatus | null;
}) {
  if (!guestPackage && !packageStatus) return null;
  const installed = guestPackage?.installedBuildId || null;
  const latest = packageStatus?.latestBuildId || packageStatus?.installedBuildId || null;
  const downloadedImage = guestPackage?.battlegroupVersion || null;
  const liveImage = guestPackage?.liveBattlegroupVersion || null;
  const updateRequired = Boolean(installed && latest && installed !== latest);
  const tone = !installed ? "amber" : updateRequired ? "amber" : "green";
  const label = !installed ? "Build unknown" : updateRequired ? "Update required" : "Current";
  return (
    <Flex align="center" gap="2" mt="3" wrap="wrap">
      <Metric label="Server Package" value={installed || "unknown"} />
      <Badge color={tone} variant="surface">
        {label}
      </Badge>
      {latest ? (
        <Text size="1" color="gray" className="mono">
          latest {latest}
        </Text>
      ) : null}
      {downloadedImage ? (
        <Text size="1" color="gray" className="mono">
          images {downloadedImage}
        </Text>
      ) : null}
      {liveImage && liveImage !== downloadedImage ? (
        <Text size="1" color="gray" className="mono">
          live {liveImage}
        </Text>
      ) : null}
    </Flex>
  );
}

export function serverPackageUpdateRequired(
  guestPackage: RemoteServerPackageStatus | undefined,
  packageStatus: ServerPackageStatus | null,
): boolean {
  const installed = guestPackage?.installedBuildId?.trim();
  const latest = (packageStatus?.latestBuildId || packageStatus?.installedBuildId || "").trim();
  return Boolean(installed && latest && installed !== latest);
}

export function PortForwardingNotice() {
  return (
    <Card size="2" variant="surface" className="info-card">
      <Text as="div" size="2" weight="medium" mb="2">
        Port Forwarding Required
      </Text>
      <Text as="div" size="2" color="gray" mb="3">
        To allow external players to connect to your BattleGroup world, ensure these ports are forwarded to your server's IP address:
      </Text>
      <Table.Root variant="surface">
        <Table.Header>
          <Table.Row>
            <Table.ColumnHeaderCell>Ports</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>Protocol</Table.ColumnHeaderCell>
            <Table.ColumnHeaderCell>Purpose</Table.ColumnHeaderCell>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {playerPortForwards.map((fw) => (
            <Table.Row key={fw.ports}>
              <Table.RowHeaderCell>{fw.ports}</Table.RowHeaderCell>
              <Table.Cell>{fw.protocol}</Table.Cell>
              <Table.Cell>{fw.purpose}</Table.Cell>
            </Table.Row>
          ))}
        </Table.Body>
      </Table.Root>
    </Card>
  );
}
