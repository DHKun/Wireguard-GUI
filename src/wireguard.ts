import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyMode,
  ApplyOutcome,
  AppSettings,
  EnvCheck,
  InterfaceAction,
  InterfaceStatus,
  KeyPair,
  PeerConf,
} from "./types";

/** Owned Tauri seam. React workflows depend on this interface, never on invoke strings. */
export interface WireGuardPort {
  observe(): Promise<InterfaceStatus[]>;
  listConfigurations(): Promise<string[]>;
  readConfiguration(name: string): Promise<string>;
  applyConfiguration(name: string, content: string, synchronize: boolean): Promise<ApplyOutcome>;
  exportConfiguration(name: string, destination: string): Promise<void>;
  applyInterface(name: string, action: InterfaceAction): Promise<void>;
  applyPeers(name: string, peers: PeerConf[], mode: ApplyMode): Promise<ApplyOutcome>;
  generateKeypair(): Promise<KeyPair>;
  generatePresharedKey(): Promise<string>;
  checkEnvironment(): Promise<EnvCheck>;
  getAppSettings(): Promise<AppSettings>;
  updateAppSettings(settings: AppSettings): Promise<AppSettings>;
}

class TauriWireGuardAdapter implements WireGuardPort {
  observe() {
    return invoke<InterfaceStatus[]>("wg_status");
  }

  listConfigurations() {
    return invoke<string[]>("list_configs");
  }

  readConfiguration(name: string) {
    return invoke<string>("read_config", { name });
  }

  applyConfiguration(name: string, content: string, synchronize: boolean) {
    return invoke<ApplyOutcome>("apply_config", { name, content, synchronize });
  }

  exportConfiguration(name: string, destination: string) {
    return invoke<void>("export_config", { name, dest: destination });
  }

  applyInterface(name: string, action: InterfaceAction) {
    return invoke<void>("interface_action", { name, action });
  }

  applyPeers(name: string, peers: PeerConf[], mode: ApplyMode) {
    return invoke<ApplyOutcome>("apply_peers", { name, peers, mode });
  }

  generateKeypair() {
    return invoke<KeyPair>("generate_keypair");
  }

  generatePresharedKey() {
    return invoke<string>("generate_preshared_key");
  }

  checkEnvironment() {
    return invoke<EnvCheck>("check_env");
  }

  getAppSettings() {
    return invoke<AppSettings>("get_app_settings");
  }

  updateAppSettings(settings: AppSettings) {
    return invoke<AppSettings>("update_app_settings", { next: settings });
  }
}

export const wireguard: WireGuardPort = new TauriWireGuardAdapter();

export function errText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
