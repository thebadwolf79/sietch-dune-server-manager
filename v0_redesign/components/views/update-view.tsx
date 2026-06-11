"use client"

import { Panel, SectionLabel, ViewContainer } from "@/components/primitives"
import { packageVersions } from "@/lib/dune-data"

function VersionStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="px-4 py-3">
      <SectionLabel>{label}</SectionLabel>
      <p className="mt-1 font-mono text-sm text-foreground">{value}</p>
    </div>
  )
}

export function UpdateView() {
  return (
    <ViewContainer>
      <div className="space-y-6">
        <div>
          <SectionLabel className="mb-2">Package versions</SectionLabel>
          <Panel className="grid grid-cols-1 divide-border sm:grid-cols-2 lg:grid-cols-4 lg:divide-x">
            <VersionStat label="Installed build" value={packageVersions.installedBuild} />
            <VersionStat label="Downloaded" value={packageVersions.downloaded} />
            <VersionStat label="Running" value={packageVersions.running} />
            <VersionStat label="Operator" value={packageVersions.operator} />
          </Panel>
        </div>

        <div>
          <SectionLabel className="mb-2">Apply update</SectionLabel>
          <Panel className="p-4">
            <p className="text-sm leading-relaxed text-muted-foreground">
              The downloaded battlegroup version matches what is currently running. No update pending.
            </p>
          </Panel>
        </div>
      </div>
    </ViewContainer>
  )
}
