import { describe, expect, it } from "vitest";
import { StatusMonitor, type MonitorSnapshot } from "./statusMonitor";
import type { InterfaceStatus } from "../types";
import { InMemoryWireGuard } from "../test/InMemoryWireGuard";

function status(rx: number, tx: number): InterfaceStatus {
  return {
    name: "wg0",
    running: true,
    public_key: "public",
    private_key: null,
    listen_port: 51820,
    fwmark: null,
    addresses: ["10.0.0.1/24"],
    mtu: 1420,
    rx_bytes: rx,
    tx_bytes: tx,
    peers: [],
  };
}

describe("StatusMonitor", () => {
  it("derives transfer rates from consecutive observations", async () => {
    const port = new InMemoryWireGuard();
    let now = 1_000;
    const monitor = new StatusMonitor(port, () => now);
    let latest: MonitorSnapshot | undefined;
    monitor.subscribe((snapshot) => (latest = snapshot));
    port.interfaces = [status(100, 200)];
    await monitor.refresh(true);

    now = 6_000;
    port.interfaces = [status(600, 1_200)];
    await monitor.refresh(true);

    expect(latest?.rates.wg0).toEqual({ rx: 100, tx: 200 });
    expect(latest?.lastUpdated).toBe(6_000);
  });

  it("cools down automatic retries after authorization cancellation", async () => {
    const port = new InMemoryWireGuard();
    let now = 1_000;
    const monitor = new StatusMonitor(port, () => now);
    port.observeHandler = async () => {
      throw new Error("pkexec request dismissed");
    };

    await monitor.refresh(true);
    await monitor.refresh(false);
    expect(port.observeCalls).toBe(1);

    now += 60_001;
    await monitor.refresh(false);
    expect(port.observeCalls).toBe(2);
  });

  it("keeps one observation in flight", async () => {
    const port = new InMemoryWireGuard();
    let resolve: ((value: InterfaceStatus[]) => void) | undefined;
    port.observeHandler = () =>
      new Promise<InterfaceStatus[]>((done) => {
        resolve = done;
      });
    const monitor = new StatusMonitor(port);

    const first = monitor.refresh(true);
    const second = monitor.refresh(true);
    expect(port.observeCalls).toBe(1);
    resolve?.([]);
    await Promise.all([first, second]);
  });
});
