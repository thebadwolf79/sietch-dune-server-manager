import {
  Badge,
  Box,
  Button,
  Card,
  Flex,
  Grid,
  Heading,
  Separator,
  TabNav,
  Text,
  Theme,
} from "@radix-ui/themes";
import {
  ArchiveIcon,
  CubeIcon,
  DownloadIcon,
  GearIcon,
  LightningBoltIcon,
  MixerHorizontalIcon,
  RocketIcon,
} from "@radix-ui/react-icons";

const pages = [
  { id: "install", label: "Install", hasLog: true },
  { id: "servers", label: "Servers", hasLog: false },
  { id: "configuration", label: "Configuration", hasLog: false },
  { id: "telemetry", label: "Telemetry", hasLog: true },
];

const activePage = pages[0];

const installSteps = [
  {
    icon: DownloadIcon,
    title: "Toolchain",
    detail: "Install manager-owned SteamCMD and OpenSSH.",
    status: "Ready",
  },
  {
    icon: ArchiveIcon,
    title: "Server package",
    detail: "Download the dedicated server payload into the app workspace.",
    status: "Waiting",
  },
  {
    icon: MixerHorizontalIcon,
    title: "Host preparation",
    detail: "Check Hyper-V, networking, VM destination, memory, and disk shape.",
    status: "Waiting",
  },
  {
    icon: RocketIcon,
    title: "Guest bootstrap",
    detail: "Create the VM, start k3s, create the world, and apply defaults.",
    status: "Waiting",
  },
];

const logRows = [
  { time: "00:00:00", level: "info", message: "Install workspace initialized." },
  { time: "00:00:00", level: "info", message: "No operation is running." },
  { time: "00:00:00", level: "hint", message: "Choose an install action to begin." },
];

export function App() {
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
        <TopNav />
        <Separator size="4" />
        <Box className={activePage.hasLog ? "app-main has-log" : "app-main"}>
          <InstallControls />
          {activePage.hasLog ? <OperationLog /> : null}
        </Box>
      </Flex>
    </Theme>
  );
}

function Header() {
  return (
    <Flex asChild align="center" justify="between" p="4">
      <header>
        <Flex align="center" gap="3">
          <CubeIcon width="24" height="24" />
          <Box>
            <Text as="div" size="1" color="bronze" weight="medium">
              Dune Server
            </Text>
            <Heading size="4">Dedicated manager</Heading>
          </Box>
        </Flex>
        <Flex align="center" gap="2">
          <Badge color="bronze" variant="surface">
            Local
          </Badge>
          <Badge color="gray" variant="surface">
            Manager offline
          </Badge>
        </Flex>
      </header>
    </Flex>
  );
}

function TopNav() {
  return (
    <Box asChild px="4" py="2">
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

function InstallControls() {
  return (
    <Card size="3" variant="surface" className="pane">
      <Flex direction="column" gap="4" height="100%">
        <Flex align="start" justify="between" gap="3">
          <Box>
            <Text as="div" size="1" color="bronze" weight="medium">
              First run
            </Text>
            <Heading size="5">Install</Heading>
            <Text as="p" size="2" color="gray">
              Provision a new Dune Awakening dedicated server from tools to guest bootstrap.
            </Text>
          </Box>
          <Badge color="bronze" variant="soft">
            Draft
          </Badge>
        </Flex>

        <Flex direction="column" gap="3">
          {installSteps.map((step, index) => {
            const Icon = step.icon;
            return (
              <Box key={step.title}>
                {index > 0 ? <Separator size="4" mb="3" /> : null}
                <Flex align="start" gap="3" py="1">
                  <Box pt="1">
                    <Icon width="18" height="18" />
                  </Box>
                  <Box flexGrow="1">
                    <Flex align="center" justify="between" gap="3">
                      <Text size="2" weight="bold">
                        {step.title}
                      </Text>
                      <Badge color={step.status === "Ready" ? "green" : "gray"} variant="soft">
                        {step.status}
                      </Badge>
                    </Flex>
                    <Text as="p" size="2" color="gray">
                      {step.detail}
                    </Text>
                  </Box>
                </Flex>
              </Box>
            );
          })}
        </Flex>

        <Separator size="4" />

        <Grid columns="2" gap="2" mt="auto">
          <Button variant="solid">
            <LightningBoltIcon /> Start install
          </Button>
          <Button variant="surface" color="gray">
            <GearIcon /> Options
          </Button>
        </Grid>
      </Flex>
    </Card>
  );
}

function OperationLog() {
  return (
    <Card size="3" variant="surface" className="pane">
      <Flex direction="column" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Text as="div" size="1" color="bronze" weight="medium">
              Operation output
            </Text>
            <Heading size="4">Setup log</Heading>
          </Box>
          <Badge color="gray" variant="surface">
            Idle
          </Badge>
        </Flex>

        <Separator size="4" my="3" />

        <Box className="log-body">
          <Flex direction="column" gap="1">
            {logRows.map((row, index) => (
              <Grid
                key={`${row.time}-${index}`}
                columns="82px 64px 1fr"
                gap="3"
                align="center"
                className="log-line"
              >
                <Text size="2" color="gray" className="mono">
                  {row.time}
                </Text>
                <Text size="2" color={row.level === "hint" ? "gray" : "bronze"} className="mono">
                  {row.level}
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
