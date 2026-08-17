import type { AppSettings } from "../types";
import type { WireGuardPort } from "../wireguard";

export const defaultAppSettings: AppSettings = {
  autostart: false,
  silent_start: false,
  close_to_tray: false,
};

/** Owns desktop-shell preferences and their persistence through the WireGuard port. */
export class AppPreferences {
  constructor(private readonly port: WireGuardPort) {}

  load() {
    return this.port.getAppSettings();
  }

  save(settings: AppSettings) {
    return this.port.updateAppSettings(settings);
  }

  toggle<K extends keyof AppSettings>(settings: AppSettings, key: K, value: AppSettings[K]) {
    return { ...settings, [key]: value };
  }
}
