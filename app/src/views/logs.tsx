import { Download, RefreshCw, Terminal } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { LogOutput } from "../components/logOutput";
import { EmptyState } from "../components/primitives";
import { visibleLogLines } from "../domain/logs";
import type { KubeItem, ManagerLogResponse } from "../types";

type LogsPanelProps = {
  pods: KubeItem[];
  busy: boolean;
  onLoadLogs: (pod: string, container: string, tail: number) => Promise<ManagerLogResponse | null>;
};

export function LogsPanel({ pods, busy, onLoadLogs }: LogsPanelProps) {
  const [selectedPod, setSelectedPod] = useState("");
  const [selectedContainer, setSelectedContainer] = useState("");
  const [tail, setTail] = useState("300");
  const [filter, setFilter] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [logResponse, setLogResponse] = useState<ManagerLogResponse | null>(null);
  const loadingRef = useRef(false);
  const selectedPodItem = pods.find((pod) => pod.metadata?.name === selectedPod) ?? pods[0] ?? null;
  const containers = useMemo(() => {
    const values = selectedPodItem?.status?.containers;
    return Array.isArray(values) ? values.map(String) : [];
  }, [selectedPodItem]);

  async function load() {
    const podName = selectedPodItem?.metadata?.name;
    if (!podName || loadingRef.current) return;
    loadingRef.current = true;
    try {
      const response = await onLoadLogs(podName, selectedContainer, Number(tail) || 300);
      if (response) setLogResponse(response);
    } finally {
      loadingRef.current = false;
    }
  }

  function exportLogs() {
    if (!logResponse) return;
    const name = [logResponse.pod, logResponse.container, "logs.txt"].filter(Boolean).join("-");
    const blob = new Blob([visibleLogLines(logResponse.lines, filter).join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  useEffect(() => {
    if (!selectedPod && pods[0]?.metadata?.name) {
      setSelectedPod(pods[0].metadata.name);
    }
  }, [pods, selectedPod]);

  useEffect(() => {
    if (containers.length > 0 && selectedContainer && !containers.includes(selectedContainer)) {
      setSelectedContainer("");
    }
  }, [containers, selectedContainer]);

  useEffect(() => {
    if (!autoRefresh || !selectedPodItem?.metadata?.name) return;
    void load();
    const interval = window.setInterval(() => void load(), 4000);
    return () => window.clearInterval(interval);
  }, [autoRefresh, selectedPodItem?.metadata?.name, selectedContainer, tail]);

  return (
    <section className="panel logs-panel">
      <div className="panel-title">
        <h2>Logs</h2>
        <Terminal size={19} />
      </div>

      {pods.length === 0 ? (
        <EmptyState text="No pods are available for log collection." />
      ) : (
        <>
          <section className="log-controls">
            <label>
              Pod
              <select value={selectedPodItem?.metadata?.name ?? ""} onChange={(event) => setSelectedPod(event.target.value)}>
                {pods.map((pod) => (
                  <option key={pod.metadata?.name} value={pod.metadata?.name}>
                    {pod.metadata?.name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Container
              <select value={selectedContainer} onChange={(event) => setSelectedContainer(event.target.value)}>
                <option value="">Default</option>
                {containers.map((container) => (
                  <option key={container} value={container}>
                    {container}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Tail lines
              <input type="number" min="1" max="2000" value={tail} onChange={(event) => setTail(event.target.value)} />
            </label>
            <label>
              Filter
              <input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="warning, AMQP, pod id" />
            </label>
            <label className="inline-toggle">
              <input type="checkbox" checked={autoRefresh} onChange={(event) => setAutoRefresh(event.target.checked)} />
              Stream
            </label>
            <button onClick={load} disabled={busy || !selectedPodItem}>
              <RefreshCw size={16} />
              Refresh
            </button>
            <button onClick={exportLogs} disabled={!logResponse}>
              <Download size={16} />
              Export
            </button>
          </section>

          {logResponse ? (
            <LogOutput lines={visibleLogLines(logResponse.lines, filter)} />
          ) : (
            <EmptyState text="Select a pod and refresh to load recent logs." />
          )}
        </>
      )}
    </section>
  );
}
