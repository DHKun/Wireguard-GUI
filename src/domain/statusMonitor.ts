import type { EnvCheck, InterfaceStatus, TransferRates } from "../types";
import { errText, type WireGuardPort } from "../wireguard";

export interface MonitorSnapshot {
  interfaces: InterfaceStatus[];
  rates: Record<string, TransferRates>;
  loading: boolean;
  stale: boolean;
  error: string | null;
  lastUpdated: number | null;
}

export interface EnvironmentStatus {
  ok: boolean;
  detail: string;
  environment?: EnvCheck;
}

type Listener = (snapshot: MonitorSnapshot) => void;

const AUTHORIZATION_ERROR = /pkexec|polkit|授权|dismissed|cancel(?:led|ed)/i;

/** Owns observation timing, single-flight, transfer rates, and authorization cooldown. */
export class StatusMonitor {
  private listeners = new Set<Listener>();
  private previous = new Map<string, { time: number; rx: number; tx: number }>();
  private inFlight = false;
  private cooldownUntil = 0;
  private snapshot: MonitorSnapshot = {
    interfaces: [],
    rates: {},
    loading: true,
    stale: false,
    error: null,
    lastUpdated: null,
  };

  constructor(
    private readonly port: WireGuardPort,
    private readonly clock: () => number = Date.now,
  ) {}

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    listener(this.snapshot);
    return () => this.listeners.delete(listener);
  }

  start(intervalMs: number | null = 5000): () => void {
    void this.refresh(true);
    if (intervalMs === null) return () => undefined;
    const timer = window.setInterval(() => void this.refresh(false), intervalMs);
    return () => window.clearInterval(timer);
  }

  async refresh(manual: boolean): Promise<void> {
    const now = this.clock();
    if (this.inFlight || (!manual && now < this.cooldownUntil)) return;
    this.inFlight = true;
    this.update({ ...this.snapshot, loading: true });
    try {
      const interfaces = await this.port.observe();
      const observedAt = this.clock();
      const rates: Record<string, TransferRates> = {};
      const nextPrevious = new Map<string, { time: number; rx: number; tx: number }>();
      for (const interfaceStatus of interfaces) {
        const previous = this.previous.get(interfaceStatus.name);
        const elapsed = previous ? (observedAt - previous.time) / 1000 : 0;
        rates[interfaceStatus.name] =
          previous && elapsed > 0
            ? {
                rx: Math.max(0, (interfaceStatus.rx_bytes - previous.rx) / elapsed),
                tx: Math.max(0, (interfaceStatus.tx_bytes - previous.tx) / elapsed),
              }
            : { rx: 0, tx: 0 };
        nextPrevious.set(interfaceStatus.name, {
          time: observedAt,
          rx: interfaceStatus.rx_bytes,
          tx: interfaceStatus.tx_bytes,
        });
      }
      this.previous = nextPrevious;
      this.update({
        interfaces,
        rates,
        loading: false,
        stale: false,
        error: null,
        lastUpdated: observedAt,
      });
    } catch (error) {
      const message = errText(error);
      if (AUTHORIZATION_ERROR.test(message)) this.cooldownUntil = this.clock() + 60_000;
      this.update({ ...this.snapshot, loading: false, stale: true, error: message });
    } finally {
      this.inFlight = false;
    }
  }

  async checkEnvironment(): Promise<EnvironmentStatus> {
    try {
      const environment = await this.port.checkEnvironment();
      const missing: string[] = [];
      if (!environment.wg) missing.push("wg");
      if (!environment.wg_quick) missing.push("wg-quick");
      if (!environment.pkexec) missing.push("pkexec");
      if (!environment.conf_dir_exists) missing.push("/etc/wireguard");
      return {
        ok: missing.length === 0,
        detail: missing.length ? `缺少：${missing.join(", ")}` : "",
        environment,
      };
    } catch {
      return { ok: false, detail: "环境检查失败" };
    }
  }

  private update(snapshot: MonitorSnapshot) {
    this.snapshot = snapshot;
    for (const listener of this.listeners) listener(snapshot);
  }
}
