import { invoke } from "@tauri-apps/api/core";
import type {
  EnvCheck,
  InterfaceStatus,
  KeyPair,
  PeerConf,
  WgConf,
} from "./types";

export const api = {
  wgStatus: () => invoke<InterfaceStatus[]>("wg_status"),
  listConfigs: () => invoke<string[]>("list_configs"),
  readConfig: (name: string) => invoke<string>("read_config", { name }),
  readConfigParsed: (name: string) => invoke<WgConf>("read_config_parsed", { name }),
  writeConfig: (name: string, content: string) =>
    invoke<void>("write_config", { name, content }),
  writeConfigParsed: (name: string, conf: WgConf) =>
    invoke<void>("write_config_parsed", { name, conf }),
  exportConfig: (name: string, dest: string) =>
    invoke<void>("export_config", { name, dest }),
  interfaceUp: (name: string) => invoke<void>("interface_up", { name }),
  interfaceDown: (name: string) => invoke<void>("interface_down", { name }),
  interfaceRestart: (name: string) => invoke<void>("interface_restart", { name }),
  syncconf: (name: string) => invoke<void>("syncconf", { name }),
  setPeers: (name: string, peers: PeerConf[]) =>
    invoke<void>("set_peers", { name, peers }),
  generateKeypair: () => invoke<KeyPair>("generate_keypair"),
  generatePresharedKey: () => invoke<string>("generate_preshared_key"),
  derivePubkey: (privateKey: string) =>
    invoke<string>("derive_pubkey", { privateKey }),
  checkEnv: () => invoke<EnvCheck & { home?: string }>("check_env"),
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
