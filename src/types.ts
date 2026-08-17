// 与 Rust Tauri seam 上的 serde 数据结构一一对应。

export type ToastTone = "ok" | "warn" | "err";

export interface PeerStatus {
  public_key: string;
  preshared_key: string | null;
  endpoint: string | null;
  allowed_ips: string[];
  latest_handshake: number; // unix 秒，0 = 从未握手
  transfer_rx: number;
  transfer_tx: number;
  persistent_keepalive: number | null;
}

export interface InterfaceStatus {
  name: string;
  running: boolean;
  public_key: string;
  private_key: string | null;
  listen_port: number;
  fwmark: string | null;
  addresses: string[];
  mtu: number | null;
  rx_bytes: number;
  tx_bytes: number;
  peers: PeerStatus[];
}

export interface InterfaceConf {
  address: string[];
  listen_port?: number;
  private_key?: string;
  dns: string[];
  mtu?: number;
  table?: string;
  pre_up: string[];
  post_up: string[];
  pre_down: string[];
  post_down: string[];
  extras: [string, string][];
}

export interface PeerConf {
  public_key: string;
  preshared_key?: string;
  allowed_ips: string[];
  endpoint?: string;
  persistent_keepalive?: number;
  extras: [string, string][];
}

export interface WgConf {
  interface: InterfaceConf;
  peers: PeerConf[];
}

export interface KeyPair {
  private_key: string;
  public_key: string;
}

export interface EnvCheck {
  wg: boolean;
  wg_quick: boolean;
  pkexec: boolean;
  conf_dir_exists: boolean;
  home: string;
}

export type ApplyMode = "runtime_only" | "persist_and_sync";

export interface ApplyOutcome {
  persisted: boolean;
  runtime_applied: boolean;
  warnings: string[];
}

export type InterfaceAction = "up" | "down" | "restart" | "sync";

export interface TransferRates {
  rx: number;
  tx: number;
}
