import { type ComponentType, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertDialog,
  Badge,
  Box,
  Button,
  Card,
  Checkbox,
  Flex,
  Grid,
  Heading,
  Link,
  Separator,
  Select,
  Switch,
  TabNav,
  Text,
  TextArea,
  TextField,
  Theme,
} from "@radix-ui/themes";
import {
  CubeIcon,
  GlobeIcon,
  LightningBoltIcon,
  MixIcon,
  RocketIcon,
  DesktopIcon,
} from "@radix-ui/react-icons";

const pages = [
  { id: "install", label: "Install", hasLog: true },
  { id: "servers", label: "Servers", hasLog: false },
  { id: "configuration", label: "Configuration", hasLog: false },
  { id: "telemetry", label: "Telemetry", hasLog: true },
];

const activePage = pages[0];

type NetworkMode = "static" | "dhcp";
type PlayerIpMode = "local" | "external";

type NetworkAdapterCandidate = {
  name: string;
  interfaceDescription: string;
  ipv4Address: string;
  prefixLength: number;
  gateway: string;
  suggestedIpv4Address: string;
  existingExternalSwitch: string;
};

type HostReadiness = {
  elevated: boolean;
  hypervAvailable: boolean;
  vmmsRunning: boolean;
  virtualizationFirmwareEnabled: boolean | null;
  totalPhysicalMemoryBytes: number;
  availablePhysicalMemoryBytes: number;
};

type DriveCandidate = {
  name: string;
  root: string;
  freeBytes: number;
};

type EnvironmentDetection = {
  readiness: HostReadiness;
  drives: DriveCandidate[];
  networkAdapters: NetworkAdapterCandidate[];
  externalIp: string | null;
};

type DetectionState = "detecting" | "ready" | "failed";
type LogLevel = "debug" | "info" | "warn" | "error";

type LogRow = {
  timestamp: string;
  level: LogLevel;
  scope: string;
  message: string;
};

type EnvironmentGate = {
  canContinue: boolean;
  reasons: string[];
};

type SetupLogPayload = {
  level: LogLevel;
  scope: string;
  message: string;
};

type SetupRunRequest = {
  vmDestination: string;
  vmName: string;
  diskGb: number;
  memoryGb: number;
  enableSwap: boolean;
  networkMode: NetworkMode;
  switchName: string;
  adapterName: string;
  staticIp: string;
  gateway: string;
  dns: string;
  playerIp: string;
  worldName: string;
  region: string;
  selfHostToken: string;
  survivalInstances: number;
  deepDesertPveInstances: number;
  deepDesertPvpInstances: number;
  deepDesertWarmServers: number;
};

type RollbackRequest = {
  vmName: string;
  vmDestination: string;
  switchName: string;
};

const log = {
  debug: (scope: string, message: string): LogRow => logEntry("debug", scope, message),
  info: (scope: string, message: string): LogRow => logEntry("info", scope, message),
  warn: (scope: string, message: string): LogRow => logEntry("warn", scope, message),
  error: (scope: string, message: string): LogRow => logEntry("error", scope, message),
};

type SetupForm = {
  vmDestination: string;
  vmName: string;
  diskGb: string;
  enableSwap: boolean;
  networkMode: NetworkMode;
  switchName: string;
  adapterName: string;
  staticIp: string;
  gateway: string;
  dns: string;
  playerIpMode: PlayerIpMode;
  playerIp: string;
  worldName: string;
  region: string;
  tokenSource: string;
  survivalInstances: string;
  includeSocial: boolean;
  deepDesertPveInstances: string;
  deepDesertPvpInstances: string;
  deepDesertWarmServers: string;
};

const defaultForm: SetupForm = {
  vmDestination: "",
  vmName: "dune-server",
  diskGb: "100",
  enableSwap: false,
  networkMode: "static",
  switchName: "",
  adapterName: "",
  staticIp: "",
  gateway: "",
  dns: "1.1.1.1",
  playerIpMode: "local",
  playerIp: "",
  worldName: "Arrakis",
  region: "Europe Test",
  tokenSource: "",
  survivalInstances: "1",
  includeSocial: true,
  deepDesertPveInstances: "1",
  deepDesertPvpInstances: "0",
  deepDesertWarmServers: "0",
};

const zeroToFour = ["0", "1", "2", "3", "4"];
const oneToFour = ["1", "2", "3", "4"];
const playerPortForwards = [
  { ports: "7777-7810", protocol: "UDP", purpose: "Game servers" },
  { ports: "31982", protocol: "TCP", purpose: "RMQ" },
];

export function App() {
  const [form, setForm] = useState<SetupForm>(defaultForm);
  const [started, setStarted] = useState(false);
  const [setupRunning, setSetupRunning] = useState(false);
  const [setupRows, setSetupRows] = useState<LogRow[]>([]);
  const [rollbackOpen, setRollbackOpen] = useState(false);
  const [rollbackRunning, setRollbackRunning] = useState(false);
  const [failedRollbackRequest, setFailedRollbackRequest] = useState<RollbackRequest | null>(null);
  const [hostReadiness, setHostReadiness] = useState<HostReadiness | null>(null);
  const [driveCandidates, setDriveCandidates] = useState<DriveCandidate[]>([]);
  const [networkAdapters, setNetworkAdapters] = useState<NetworkAdapterCandidate[]>([]);
  const [externalIp, setExternalIp] = useState<string | null>(null);
  const [networkDetection, setNetworkDetection] = useState<DetectionState>("detecting");
  const [vmDestinationHasVm, setVmDestinationHasVm] = useState(false);
  const calculatedMemory = useMemo(() => calculateRequiredMemory(form), [form]);
  const environmentGate = useMemo(
    () => setupEnvironmentGate(networkDetection, hostReadiness, networkAdapters),
    [hostReadiness, networkAdapters, networkDetection],
  );
  const environmentRows = useMemo(
    () =>
      environmentLogRows(
        networkDetection,
        hostReadiness,
        networkAdapters,
        driveCandidates,
        externalIp,
        environmentGate,
      ),
    [driveCandidates, environmentGate, externalIp, hostReadiness, networkAdapters, networkDetection],
  );
  const layoutPreview = useMemo(() => setupLayoutPreview(form), [form]);
  const update = <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => {
    setForm((current) => normalizeSetupForm({ ...current, [key]: value }));
  };

  useEffect(() => {
    let cancelled = false;
    invoke<string>("default_vm_location")
      .then((location) => {
        if (cancelled) return;
        setForm((current) => (current.vmDestination ? current : { ...current, vmDestination: location }));
      })
      .catch(() => {
        // Keep the field user-editable if the native default path cannot be resolved.
      });
    invoke<EnvironmentDetection>("detect_environment")
      .then((environment) => {
        if (cancelled) return;
        setHostReadiness(environment.readiness);
        setDriveCandidates(environment.drives);
        setNetworkAdapters(environment.networkAdapters);
        setExternalIp(environment.externalIp);
        setNetworkDetection("ready");
        const first = environment.networkAdapters[0];
        if (first) {
          setForm((current) => {
            if (current.adapterName || current.staticIp || current.playerIp || current.gateway) {
              return current;
            }
            return {
              ...current,
              adapterName: first.name,
              switchName: first.existingExternalSwitch || first.name,
              staticIp: first.suggestedIpv4Address,
              playerIp: current.playerIpMode === "external" && environment.externalIp
                ? environment.externalIp
                : first.suggestedIpv4Address,
              gateway: first.gateway,
            };
          });
        }
      })
      .catch(() => {
        if (!cancelled) setNetworkDetection("failed");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen<SetupLogPayload>("setup-log", (event) => {
      if (cancelled) return;
      setSetupRows((rows) => [
        ...rows,
        logEntry(event.payload.level, event.payload.scope, event.payload.message),
      ]);
    }).then((handler) => {
      unlisten = handler;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const path = form.vmDestination.trim();
    if (!/^[A-Za-z]:[\\/]/.test(path)) {
      setVmDestinationHasVm(false);
      return () => {
        cancelled = true;
      };
    }
    const timer = window.setTimeout(() => {
      invoke<boolean>("vm_destination_has_vm", { path })
        .then((hasVm) => {
          if (!cancelled) setVmDestinationHasVm(hasVm);
        })
        .catch(() => {
          if (!cancelled) setVmDestinationHasVm(false);
        });
    }, 150);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [form.vmDestination]);

  const logRows = useMemo(
    () =>
      started
        ? [...environmentRows, ...setupRows]
        : [
            ...environmentRows,
            log.info("setup", "Choose the full setup shape, then run it once."),
          ],
    [environmentRows, setupRows, started],
  );

  const startSetup = async () => {
    const request = setupRunRequest(form, calculatedMemory.gb);
    setStarted(true);
    setSetupRunning(true);
    setSetupRows([]);
    setFailedRollbackRequest(null);
    try {
      await invoke("start_full_setup", {
        request,
      });
    } catch (err) {
      console.error(err);
      setFailedRollbackRequest(rollbackRequestFromSetup(request));
      setRollbackOpen(true);
    } finally {
      setSetupRunning(false);
    }
  };

  const rollback = async () => {
    if (!failedRollbackRequest) return;
    setRollbackRunning(true);
    try {
      await invoke("rollback_setup", { request: failedRollbackRequest });
      setRollbackOpen(false);
      setFailedRollbackRequest(null);
    } finally {
      setRollbackRunning(false);
    }
  };

  return (
    <Theme
      appearance="dark"
      accentColor="bronze"
      grayColor="sand"
      panelBackground="solid"
      radius="medium"
      scaling="95%"
    >
      <Flex direction="column" className="app-root">
        <Header />
        <Separator size="4" />
        <Box className={activePage.hasLog ? "app-main has-log" : "app-main"}>
          <InstallControls
            form={form}
            calculatedMemory={calculatedMemory}
            layoutPreview={layoutPreview}
            hostReadiness={hostReadiness}
            driveCandidates={driveCandidates}
            networkAdapters={networkAdapters}
            networkDetection={networkDetection}
            externalIp={externalIp}
            environmentGate={environmentGate}
            setupRunning={setupRunning}
            vmDestinationHasVm={vmDestinationHasVm}
            update={update}
            onStart={startSetup}
          />
          {activePage.hasLog ? (
            <LogWindow rows={logRows} />
          ) : null}
        </Box>
        <RollbackDialog
          open={rollbackOpen}
          rollbackRunning={rollbackRunning}
          onOpenChange={setRollbackOpen}
          onRollback={rollback}
        />
      </Flex>
    </Theme>
  );
}

function Header() {
  return (
    <Flex asChild align="center" justify="between" p="4">
      <header>
        <Flex align="center" gap="5">
          <Flex align="center" gap="3">
            <CubeIcon width="24" height="24" />
            <Heading size="4">Dune Dedicated Server Manager</Heading>
          </Flex>
          <TopNav />
        </Flex>
      </header>
    </Flex>
  );
}

function TopNav() {
  return (
    <Box asChild>
      <nav aria-label="Primary navigation">
        <TabNav.Root size="2" color="bronze">
          {pages.map((page) => (
            <TabNav.Link key={page.id} href="#" active={page.id === activePage.id}>
              {page.label}
            </TabNav.Link>
          ))}
        </TabNav.Root>
      </nav>
    </Box>
  );
}

function InstallControls({
  form,
  calculatedMemory,
  layoutPreview,
  hostReadiness,
  driveCandidates,
  networkAdapters,
  networkDetection,
  externalIp,
  environmentGate,
  setupRunning,
  vmDestinationHasVm,
  update,
  onStart,
}: {
  form: SetupForm;
  calculatedMemory: CalculatedMemory;
  layoutPreview: SetupLayoutPreview;
  hostReadiness: HostReadiness | null;
  driveCandidates: DriveCandidate[];
  networkAdapters: NetworkAdapterCandidate[];
  networkDetection: DetectionState;
  externalIp: string | null;
  environmentGate: EnvironmentGate;
  setupRunning: boolean;
  vmDestinationHasVm: boolean;
  update: <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => void;
  onStart: () => void;
}) {
  const deepDesertEnabled = layoutPreview.deepDesertTotal > 0;
  const warmOptions = zeroTo(layoutPreview.deepDesertTotal);
  const requirements = setupRequirementStatus(
    calculatedMemory,
    form.diskGb,
    form.vmDestination,
    hostReadiness,
    driveCandidates,
  );
  const hasServiceToken = form.tokenSource.trim().length > 0;
  const setupIssues = setupBlockingIssues(
    environmentGate,
    requirements,
    hasServiceToken,
    vmDestinationHasVm,
  );
  const canStart = setupIssues.length === 0;

  return (
    <Card size="3" variant="surface" className="pane setup-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Flex align="start" justify="between" gap="4">
          <Box>
            <Heading size="5">Server Setup</Heading>
            <Text as="p" size="2" color="gray">
              Please configure your server settings below. You'll be able to change them later.
            </Text>
          </Box>
        </Flex>

        <Box className="setup-scroll">
          <Flex direction="column" gap="5" className={setupRunning ? "setup-controls is-disabled" : "setup-controls"}>
            <SetupSection icon={GlobeIcon} title="World" className="setup-order-world">
              <Grid columns="2" gap="3">
                <Field label="World name">
                  <TextField.Root value={form.worldName} onChange={(event) => update("worldName", event.target.value)} />
                </Field>
                <Field label="Region">
                  <Select.Root value={form.region} onValueChange={(value) => update("region", value)}>
                    <Select.Trigger />
                    <Select.Content>
                      <Select.Item value="Europe Test">Europe Test</Select.Item>
                      <Select.Item value="North America Test">North America Test</Select.Item>
                    </Select.Content>
                  </Select.Root>
                </Field>
              </Grid>
              <Field label="Self-Host Service Token">
                <TextArea
                  placeholder="Paste your Self-Host Service Token"
                  value={form.tokenSource}
                  onChange={(event) => update("tokenSource", event.target.value)}
                />
                <Text as="p" size="2" color="gray">
                  Get the token from{" "}
                  <Link href="https://account-pts.duneawakening.com/account" target="_blank" rel="noreferrer">
                    account-pts.duneawakening.com/account
                  </Link>
                  .
                </Text>
              </Field>
            </SetupSection>

            <SetupSection icon={RocketIcon} title="World Layout" className="setup-order-layout">
              <Flex direction="column" gap="2">
                <LayoutRow label="Hagga Basin">
                  <Select.Root
                    value={form.survivalInstances}
                    onValueChange={(value) => update("survivalInstances", value)}
                  >
                    <Select.Trigger />
                    <Select.Content>
                      {oneToFour.map((value) => (
                        <Select.Item key={value} value={value}>
                          {value} {value === "1" ? "instance" : "instances"}
                        </Select.Item>
                      ))}
                    </Select.Content>
                  </Select.Root>
                </LayoutRow>
                <LayoutRow label="Social Hubs">
                  <Flex align="center" gap="3">
                    <Checkbox
                      checked={deepDesertEnabled || form.includeSocial}
                      disabled={deepDesertEnabled}
                      onCheckedChange={(value) => update("includeSocial", value === true)}
                    />
                    <Text size="2" color="gray">
                      {deepDesertEnabled ? "Required by Deep Desert" : "Optional"}
                    </Text>
                  </Flex>
                </LayoutRow>
                <LayoutRow label="Deep Desert PvE">
                  <Select.Root
                    value={form.deepDesertPveInstances}
                    onValueChange={(value) => update("deepDesertPveInstances", value)}
                  >
                    <Select.Trigger />
                    <Select.Content>
                      {zeroToFour.map((value) => (
                        <Select.Item key={value} value={value}>
                          {value} {value === "1" ? "instance" : "instances"}
                        </Select.Item>
                      ))}
                    </Select.Content>
                  </Select.Root>
                </LayoutRow>
                <LayoutRow label="Deep Desert PvP">
                  <Select.Root
                    value={form.deepDesertPvpInstances}
                    onValueChange={(value) => update("deepDesertPvpInstances", value)}
                  >
                    <Select.Trigger />
                    <Select.Content>
                      {zeroToFour.map((value) => (
                        <Select.Item key={value} value={value}>
                          {value} {value === "1" ? "instance" : "instances"}
                        </Select.Item>
                      ))}
                    </Select.Content>
                  </Select.Root>
                </LayoutRow>
                <LayoutRow label="Warm Deep Desert Instances">
                  <Select.Root
                    value={form.deepDesertWarmServers}
                    onValueChange={(value) => update("deepDesertWarmServers", value)}
                  >
                    <Select.Trigger />
                    <Select.Content>
                      {warmOptions.map((value) => (
                        <Select.Item key={value} value={value}>
                          {value === "0" ? "0, on demand" : `${value} warm`}
                        </Select.Item>
                      ))}
                    </Select.Content>
                  </Select.Root>
                </LayoutRow>
              </Flex>
            </SetupSection>

            <SetupSection icon={DesktopIcon} title="Host and VM" className="setup-order-vm">
              <Flex direction="column" gap="2">
                <FormRow label="VM Name">
                  <TextField.Root value={form.vmName} onChange={(event) => update("vmName", event.target.value)} />
                </FormRow>
                <FormRow label="VM Location">
                  <Grid columns="1fr auto" gap="2">
                    <TextField.Root
                      placeholder="Resolving default VM location..."
                      value={form.vmDestination}
                      onChange={(event) => update("vmDestination", event.target.value)}
                    />
                    <Button
                      type="button"
                      variant="surface"
                      onClick={async () => {
                        const selected = await open({
                          directory: true,
                          defaultPath: form.vmDestination || undefined,
                          multiple: false,
                          title: "Choose VM files destination",
                        });
                        if (typeof selected === "string") {
                          update("vmDestination", selected);
                        }
                      }}
                    >
                      Choose
                    </Button>
                  </Grid>
                  <InlineRequirement
                    ok={requirements.diskOk && !vmDestinationHasVm}
                    text={
                      vmDestinationHasVm
                        ? "Destination already contains VM files. Choose another folder."
                        : `${requirements.diskRequired}; ${requirements.diskAvailable}`
                    }
                  />
                </FormRow>
                <FormRow label="Disk Size">
                  <TextField.Root value={form.diskGb} onChange={(event) => update("diskGb", event.target.value)}>
                    <TextField.Slot side="right">GB</TextField.Slot>
                  </TextField.Root>
                </FormRow>
              </Flex>

              <Box className="memory-calculation">
                <Flex align="start" justify="between" gap="4">
                  <Box>
                    <Text as="div" size="2" weight="medium">
                      Calculated VM memory
                    </Text>
                    <Text as="div" size="2" color="gray">
                      Derived from the selected world layout.
                    </Text>
                  </Box>
                  <Text size="7" weight="bold" color="bronze">
                    {calculatedMemory.gb} GB
                  </Text>
                </Flex>
                <InlineRequirement
                  ok={requirements.memoryOk}
                  text={`${requirements.memoryRequired}; ${requirements.memoryAvailable}`}
                />
                <Separator size="4" my="3" />
                <Flex direction="column" gap="1">
                  {calculatedMemory.lines.map((line) => (
                    <Text key={line} size="2" color="gray">
                      {line}
                    </Text>
                  ))}
                </Flex>
              </Box>

              <Flex align="center" justify="between" gap="3">
                <Box>
                  <Text as="div" size="2" weight="medium">
                    Enable experimental swap
                  </Text>
                  <Text as="div" size="2" color="gray">
                    Helps large layouts fit on constrained hosts.
                  </Text>
                </Box>
                <Switch checked={form.enableSwap} onCheckedChange={(value) => update("enableSwap", value)} />
              </Flex>
            </SetupSection>

            <SetupSection icon={MixIcon} title="Network" className="setup-order-network">
              <Field label="Network mode">
                <Select.Root
                  value={form.networkMode}
                  onValueChange={(value) => update("networkMode", value as NetworkMode)}
                >
                  <Select.Trigger />
                  <Select.Content>
                    <Select.Item value="static">Static internal IP</Select.Item>
                    <Select.Item value="dhcp">DHCP</Select.Item>
                  </Select.Content>
                </Select.Root>
              </Field>
              <Field label="Host network adapter">
                <Select.Root
                  value={form.adapterName || undefined}
                  onValueChange={(value) => {
                    const adapter = networkAdapters.find((candidate) => candidate.name === value);
                    if (!adapter) return;
                    update("adapterName", value);
                    update("switchName", adapter.existingExternalSwitch || adapter.name);
                    update("staticIp", adapter.suggestedIpv4Address);
                    update(
                      "playerIp",
                      form.playerIpMode === "external" && externalIp ? externalIp : adapter.suggestedIpv4Address,
                    );
                    update("gateway", adapter.gateway);
                  }}
                >
                  <Select.Trigger placeholder={networkStatusLabel(networkDetection)} />
                  <Select.Content>
                    {networkAdapters.map((adapter) => (
                      <Select.Item key={adapter.name} value={adapter.name}>
                        {adapter.name} - {adapter.ipv4Address}/{adapter.prefixLength}
                      </Select.Item>
                    ))}
                  </Select.Content>
                </Select.Root>
              </Field>
              <Field label="Hyper-V switch">
                <TextField.Root
                  placeholder="Detected from adapter"
                  value={form.switchName}
                  onChange={(event) => update("switchName", event.target.value)}
                />
              </Field>
              <Grid columns="3" gap="3">
                <Field label="VM IP">
                  <TextField.Root
                    placeholder="Detected suggestion"
                    value={form.staticIp}
                    onChange={(event) => update("staticIp", event.target.value)}
                  />
                </Field>
                <Field label="Gateway">
                  <TextField.Root
                    placeholder="Detected gateway"
                    value={form.gateway}
                    onChange={(event) => update("gateway", event.target.value)}
                  />
                </Field>
                <Field label="DNS">
                  <TextField.Root value={form.dns} onChange={(event) => update("dns", event.target.value)} />
                </Field>
              </Grid>
              <Field label="Player-facing IP">
                <Grid columns="160px 1fr" gap="3">
                  <Select.Root
                    value={form.playerIpMode}
                    onValueChange={(value) => {
                      const mode = value as PlayerIpMode;
                      update("playerIpMode", mode);
                      update("playerIp", mode === "external" ? externalIp || "" : form.staticIp);
                    }}
                  >
                    <Select.Trigger />
                    <Select.Content>
                      <Select.Item value="local">Local IP</Select.Item>
                      <Select.Item value="external">External IP</Select.Item>
                    </Select.Content>
                  </Select.Root>
                  <TextField.Root
                    placeholder={form.playerIpMode === "external" ? "Detected external IP" : "Same as VM IP for LAN"}
                    value={form.playerIp}
                    onChange={(event) => update("playerIp", event.target.value)}
                  />
                </Grid>
              </Field>
              {form.playerIpMode === "external" ? <PortForwardingNotice /> : null}
            </SetupSection>

          </Flex>
        </Box>

        <Separator size="4" />

        <Flex align="center" justify="between" gap="3">
          <Box className="setup-readiness">
            {setupRunning ? null : canStart ? (
              <Text size="2" color="gray">
                Ready to create one full setup plan.
              </Text>
            ) : (
              <ul className="setup-issues">
                {setupIssues.map((issue) => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            )}
          </Box>
          <Button size="3" onClick={onStart} disabled={!canStart || setupRunning}>
            <LightningBoltIcon /> {setupRunning ? "Setup running..." : "Start full setup"}
          </Button>
        </Flex>
      </Flex>
    </Card>
  );
}

type CalculatedMemory = {
  gb: number;
  lines: string[];
};

type SetupLayoutPreview = {
  survivalDimensions: string;
  deepDesertTotal: number;
  deepDesertPvp: number;
};

type SetupRequirements = {
  canContinue: boolean;
  memoryOk: boolean;
  diskOk: boolean;
  memoryRequired: string;
  memoryAvailable: string;
  diskRequired: string;
  diskAvailable: string;
};

function setupLayoutPreview(form: SetupForm): SetupLayoutPreview {
  const survivalInstances = Math.max(1, parsePositiveInt(form.survivalInstances));
  const deepDesertPve = parsePositiveInt(form.deepDesertPveInstances);
  const deepDesertPvp = parsePositiveInt(form.deepDesertPvpInstances);
  const deepDesertTotal = deepDesertPve + deepDesertPvp;
  const survivalDimensions = Array.from({ length: survivalInstances }, (_, index) => index).join(", ");

  return {
    survivalDimensions,
    deepDesertTotal,
    deepDesertPvp,
  };
}

function setupRunRequest(form: SetupForm, memoryGb: number): SetupRunRequest {
  return {
    vmDestination: form.vmDestination,
    vmName: form.vmName,
    diskGb: parsePositiveInt(form.diskGb),
    memoryGb,
    enableSwap: form.enableSwap,
    networkMode: form.networkMode,
    switchName: form.switchName,
    adapterName: form.adapterName,
    staticIp: form.staticIp,
    gateway: form.gateway,
    dns: form.dns,
    playerIp: form.playerIp,
    worldName: form.worldName,
    region: form.region,
    selfHostToken: form.tokenSource,
    survivalInstances: Math.max(1, parsePositiveInt(form.survivalInstances)),
    deepDesertPveInstances: parsePositiveInt(form.deepDesertPveInstances),
    deepDesertPvpInstances: parsePositiveInt(form.deepDesertPvpInstances),
    deepDesertWarmServers: parsePositiveInt(form.deepDesertWarmServers),
  };
}

function rollbackRequestFromSetup(request: SetupRunRequest): RollbackRequest {
  return {
    vmName: request.vmName,
    vmDestination: request.vmDestination,
    switchName: request.switchName,
  };
}

function calculateRequiredMemory(form: SetupForm): CalculatedMemory {
  const survivalInstances = Math.max(1, parsePositiveInt(form.survivalInstances));
  const deepDesertInstances =
    parsePositiveInt(form.deepDesertPveInstances) + parsePositiveInt(form.deepDesertPvpInstances);
  const survivalGb = survivalInstances * 20;
  const socialGb = form.includeSocial || deepDesertInstances > 0 ? 10 : 0;
  const deepDesertGb = deepDesertInstances * 10;
  const gb = survivalGb + socialGb + deepDesertGb;
  const lines = [
    `${survivalInstances} Hagga Basin ${survivalInstances === 1 ? "instance" : "instances"} x 20 GB = ${survivalGb} GB`,
  ];

  if (form.includeSocial || deepDesertInstances > 0) {
    lines.push("Social Hubs = 10 GB");
  }
  if (deepDesertInstances > 0) {
    lines.push(
      `${deepDesertInstances} Deep Desert ${
        deepDesertInstances === 1 ? "instance" : "instances"
      } x 10 GB = ${deepDesertGb} GB`,
    );
  }

  return { gb, lines };
}

function normalizeSetupForm(form: SetupForm): SetupForm {
  const deepDesertInstances =
    parsePositiveInt(form.deepDesertPveInstances) + parsePositiveInt(form.deepDesertPvpInstances);
  const warmServers = Math.min(parsePositiveInt(form.deepDesertWarmServers), deepDesertInstances);
  const normalized = {
    ...form,
    includeSocial: deepDesertInstances > 0 ? true : form.includeSocial,
    deepDesertWarmServers: String(warmServers),
  };
  if (normalized.playerIpMode === "local" && normalized.staticIp && normalized.playerIp !== normalized.staticIp) {
    return { ...normalized, playerIp: normalized.staticIp };
  }
  return normalized;
}

function parsePositiveInt(value: string): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

function zeroTo(max: number): string[] {
  return Array.from({ length: Math.max(0, max) + 1 }, (_, index) => String(index));
}

function environmentLogRows(
  status: DetectionState,
  readiness: HostReadiness | null,
  adapters: NetworkAdapterCandidate[],
  drives: DriveCandidate[],
  externalIp: string | null,
  gate: EnvironmentGate,
): LogRow[] {
  if (status === "detecting") {
    return [
      log.debug("env", "Checking administrator privileges..."),
      log.debug("env", "Checking virtualization firmware, Hyper-V, and vmms service..."),
      log.debug("env", "Waiting to detect host networking after host checks complete..."),
    ];
  }
  if (status === "failed") {
    return [log.error("env", "Environment detection failed. Network fields can still be filled manually.")];
  }
  const rows: LogRow[] = [];
  if (readiness) {
    rows.push(
      readiness.elevated
        ? log.info("env", "Administrator privileges detected.")
        : log.warn("env", "This app is not elevated; restart it as administrator to continue setup."),
    );
    rows.push(
      readiness.virtualizationFirmwareEnabled === false
        ? log.warn("env", "Virtualization firmware is disabled or unavailable.")
        : log.info("env", "Hyper-V virtualization support is operational."),
    );
    rows.push(
      readiness.hypervAvailable && readiness.vmmsRunning
        ? log.info("env", "Hyper-V available; vmms service running.")
        : log.warn(
            "env",
            `Hyper-V ${readiness.hypervAvailable ? "available" : "missing"}; vmms service ${
              readiness.vmmsRunning ? "running" : "not running"
            }.`,
          ),
    );
    rows.push(
      log.info(
        "env",
        `Physical memory: ${formatGiB(readiness.availablePhysicalMemoryBytes)} available of ${formatGiB(readiness.totalPhysicalMemoryBytes)} total.`,
      ),
    );
  }
  if (drives.length > 0) {
    rows.push(
      log.debug(
        "env",
        `Detected drives: ${drives
          .map((drive) => `${drive.root} ${formatGiB(drive.freeBytes)} free`)
          .join(", ")}.`,
      ),
    );
  }
  rows.push(
    externalIp
      ? log.info("env", `Detected external IP ${externalIp}.`)
      : log.warn("env", "External IP was not detected; it can be entered manually."),
  );
  if (adapters.length === 0) {
    rows.push(log.warn("env", "No active physical adapters with IPv4 gateway were detected."));
    return rows;
  }

  rows.push(
    ...adapters.map((adapter) =>
      log.info(
        "env",
        `Detected ${adapter.name}: ${adapter.ipv4Address}/${adapter.prefixLength}, gateway ${adapter.gateway}, suggested VM IP ${adapter.suggestedIpv4Address || "unavailable"}.`,
      ),
    ),
  );
  if (!gate.canContinue) {
    rows.push(...gate.reasons.map((reason) => log.error("env", reason)));
  }
  return rows;
}

function setupEnvironmentGate(
  status: DetectionState,
  readiness: HostReadiness | null,
  adapters: NetworkAdapterCandidate[],
): EnvironmentGate {
  const reasons: string[] = [];
  if (status !== "ready") {
    reasons.push("Environment detection has not completed.");
  }
  if (!readiness) {
    reasons.push("Host readiness was not detected.");
  } else {
    if (!readiness.elevated) {
      reasons.push("Restart the app as administrator to continue setup.");
    }
    if (readiness.virtualizationFirmwareEnabled === false) {
      reasons.push("Hyper-V virtualization support is not operational.");
    }
    if (!readiness.hypervAvailable) {
      reasons.push("Hyper-V PowerShell support is missing.");
    }
    if (!readiness.vmmsRunning) {
      reasons.push("Hyper-V vmms service is not running.");
    }
  }
  if (adapters.length === 0) {
    reasons.push("A physical network adapter with IPv4 and gateway is required.");
  }
  return {
    canContinue: reasons.length === 0,
    reasons,
  };
}

function setupRequirementStatus(
  calculatedMemory: CalculatedMemory,
  diskGb: string,
  vmDestination: string,
  readiness: HostReadiness | null,
  drives: DriveCandidate[],
): SetupRequirements {
  const requiredMemoryBytes = calculatedMemory.gb * 1024 * 1024 * 1024;
  const requiredDiskGb = Math.max(0, parsePositiveInt(diskGb));
  const requiredDiskBytes = requiredDiskGb * 1024 * 1024 * 1024;
  const memoryAvailable = readiness?.availablePhysicalMemoryBytes ?? 0;
  const memoryOk = memoryAvailable >= requiredMemoryBytes;
  const destinationDrive = findDriveForPath(vmDestination, drives);
  const diskOk = destinationDrive ? destinationDrive.freeBytes >= requiredDiskBytes : false;

  return {
    canContinue: memoryOk && diskOk,
    memoryOk,
    diskOk,
    memoryRequired: `${calculatedMemory.gb} GB required`,
    memoryAvailable: readiness ? `${formatGiB(memoryAvailable)} available` : "Detecting",
    diskRequired: `${requiredDiskGb} GB required`,
    diskAvailable: destinationDrive
      ? `${destinationDrive.root} has ${formatGiB(destinationDrive.freeBytes)} free`
      : "Choose a VM destination folder",
  };
}

function findDriveForPath(path: string, drives: DriveCandidate[]): DriveCandidate | null {
  const normalizedPath = path.trim().replace(/\//g, "\\").toUpperCase();
  if (!/^[A-Z]:\\/.test(normalizedPath)) {
    return null;
  }

  return (
    drives.find((drive) => {
      const root = drive.root.trim().replace(/\//g, "\\").toUpperCase();
      return normalizedPath.startsWith(root);
    }) ?? null
  );
}

function setupBlockingIssues(
  gate: EnvironmentGate,
  requirements: SetupRequirements,
  hasServiceToken: boolean,
  vmDestinationHasVm: boolean,
): string[] {
  const issues = [...gate.reasons];
  if (!requirements.memoryOk) {
    issues.push(`Memory: ${requirements.memoryRequired}; ${requirements.memoryAvailable}.`);
  }
  if (!requirements.diskOk) {
    issues.push(`VM Location: ${requirements.diskRequired}; ${requirements.diskAvailable}.`);
  }
  if (vmDestinationHasVm) {
    issues.push("VM Location already contains VM files. Choose another folder.");
  }
  if (!hasServiceToken) {
    issues.push("Self-Host Service Token is required.");
  }
  return issues;
}

function LayoutRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Grid columns="minmax(180px, 1fr) 210px" gap="3" align="center" className="layout-row">
      <Text size="2" weight="medium">
        {label}
      </Text>
      <Box>{children}</Box>
    </Grid>
  );
}

function FormRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Grid columns="130px minmax(0, 1fr)" gap="3" align="start" className="form-row">
      <Text size="2" weight="medium" mt="2">
        {label}
      </Text>
      <Box>{children}</Box>
    </Grid>
  );
}

function InlineRequirement({ ok, text }: { ok: boolean; text: string }) {
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

function PortForwardingNotice() {
  return (
    <Box className="port-forwarding">
      <Text as="div" size="2" weight="medium">
        Port forwarding required
      </Text>
      <Text as="p" size="2" color="gray">
        Forward these ports from your router to the VM IP when players connect through the external IP.
      </Text>
      <Flex direction="column" gap="2">
        {playerPortForwards.map((entry) => (
          <Grid key={`${entry.ports}-${entry.protocol}`} columns="120px 70px 1fr" gap="3">
            <Text size="2" className="mono">
              {entry.ports}
            </Text>
            <Badge color={entry.protocol === "UDP" ? "bronze" : "gray"} variant="surface">
              {entry.protocol}
            </Badge>
            <Text size="2" color="gray">
              {entry.purpose}
            </Text>
          </Grid>
        ))}
      </Flex>
    </Box>
  );
}

function logEntry(level: LogLevel, scope: string, message: string): LogRow {
  return {
    timestamp: new Date().toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
    level,
    scope,
    message,
  };
}

function networkStatusLabel(status: DetectionState): string {
  if (status === "detecting") return "Detecting adapters...";
  if (status === "failed") return "Detection failed";
  return "Choose adapter";
}

function formatGiB(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "unknown";
  return `${Math.round(bytes / 1024 / 1024 / 1024)} GB`;
}

function SetupSection({
  className,
  icon: Icon,
  title,
  children,
}: {
  className?: string;
  icon: ComponentType<{ width?: number | string; height?: number | string }>;
  title: string;
  children: ReactNode;
}) {
  return (
    <Box className={["setup-section", className].filter(Boolean).join(" ")}>
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

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Box>
      <Text as="label" size="2" weight="medium" mb="1" className="field-label">
        {label}
      </Text>
      {children}
    </Box>
  );
}

function LogWindow({ rows }: { rows: LogRow[] }) {
  const bodyRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const distanceFromBottom = body.scrollHeight - body.scrollTop - body.clientHeight;
    if (distanceFromBottom < 80) {
      body.scrollTop = body.scrollHeight;
    }
  }, [rows]);

  return (
    <Card size="3" variant="surface" className="pane">
      <Flex direction="column" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="4">Setup log</Heading>
          </Box>
        </Flex>

        <Separator size="4" my="3" />

        <Box className="log-body" ref={bodyRef}>
          <Flex direction="column" gap="0">
            {rows.map((row, index) => (
              <Grid
                key={`${row.timestamp}-${row.scope}-${row.level}-${index}`}
                columns="96px 58px 62px 1fr"
                gap="3"
                align="center"
                className={`log-line log-${row.level}`}
              >
                <Text size="2" color="gray" className="mono log-meta">
                  {row.timestamp}
                </Text>
                <Text size="2" className="mono log-meta log-level">
                  {row.level}
                </Text>
                <Text size="2" color="gray" className="mono log-meta">
                  {row.scope}
                </Text>
                <Text size="2" className="mono">
                  {row.message}
                </Text>
              </Grid>
            ))}
          </Flex>
        </Box>
      </Flex>
    </Card>
  );
}

function RollbackDialog({
  open,
  rollbackRunning,
  onOpenChange,
  onRollback,
}: {
  open: boolean;
  rollbackRunning: boolean;
  onOpenChange: (open: boolean) => void;
  onRollback: () => void;
}) {
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="460px">
        <AlertDialog.Title>Rollback setup artifacts?</AlertDialog.Title>
        <AlertDialog.Description size="2">
          Setup failed after creating or touching host resources. Rollback removes the selected VM,
          removes VM files when they look like manager-created VM files, and removes the Hyper-V
          switch only if no other VMs use it.
        </AlertDialog.Description>
        <Flex gap="3" mt="4" justify="end">
          <AlertDialog.Cancel disabled={rollbackRunning}>Keep artifacts</AlertDialog.Cancel>
          <AlertDialog.Action disabled={rollbackRunning} onClick={onRollback}>
            {rollbackRunning ? "Rolling back..." : "Rollback"}
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}
