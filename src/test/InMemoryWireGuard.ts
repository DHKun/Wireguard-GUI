import type {
  ApplyMode,
  ApplyOutcome,
  AppSettings,
  EnvCheck,
  InterfaceAction,
  InterfaceStatus,
  KeyPair,
  PeerConf,
} from "../types";
import type { WireGuardPort } from "../wireguard";

const success: ApplyOutcome = {
  persisted: true,
  runtime_applied: true,
  warnings: [],
};

export class InMemoryWireGuard implements WireGuardPort {
  configurations = new Map<string, string>();
  interfaces: InterfaceStatus[] = [];
  configurationOutcome: ApplyOutcome = success;
  peerOutcome: ApplyOutcome = success;
  observeCalls = 0;
  observeHandler: () => Promise<InterfaceStatus[]> = async () => this.interfaces;
  appliedPeers: { name: string; peers: PeerConf[]; mode: ApplyMode }[] = [];
  appSettings: AppSettings = {
    autostart: false,
    silent_start: false,
    close_to_tray: false,
  };

  observe() {
    this.observeCalls += 1;
    return this.observeHandler();
  }

  async listConfigurations() {
    return [...this.configurations.keys()].sort();
  }

  async readConfiguration(name: string) {
    const value = this.configurations.get(name);
    if (value === undefined) throw new Error(`missing ${name}`);
    return value;
  }

  async applyConfiguration(name: string, content: string, _synchronize: boolean) {
    this.configurations.set(name, content);
    return this.configurationOutcome;
  }

  async exportConfiguration(_name: string, _destination: string) {}

  async applyInterface(_name: string, _action: InterfaceAction) {}

  async applyPeers(name: string, peers: PeerConf[], mode: ApplyMode) {
    this.appliedPeers.push({ name, peers, mode });
    return this.peerOutcome;
  }

  async generateKeypair(): Promise<KeyPair> {
    return { private_key: "private", public_key: "public" };
  }

  async generatePresharedKey() {
    return "preshared";
  }

  async checkEnvironment(): Promise<EnvCheck> {
    return {
      wg: true,
      wg_quick: true,
      pkexec: true,
      conf_dir_exists: true,
      home: "/home/test",
    };
  }

  async getAppSettings() {
    return { ...this.appSettings };
  }

  async updateAppSettings(settings: AppSettings) {
    this.appSettings = { ...settings };
    return { ...this.appSettings };
  }
}
