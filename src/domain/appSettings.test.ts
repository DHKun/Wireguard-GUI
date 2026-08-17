import { describe, expect, it } from "vitest";
import { AppPreferences } from "./appSettings";
import { InMemoryWireGuard } from "../test/InMemoryWireGuard";

describe("AppPreferences", () => {
  it("persists toggles through the port", async () => {
    const port = new InMemoryWireGuard();
    const preferences = new AppPreferences(port);

    const saved = await preferences.save(
      preferences.toggle(await preferences.load(), "close_to_tray", true),
    );

    expect(saved.close_to_tray).toBe(true);
    expect(port.appSettings.close_to_tray).toBe(true);
  });
});
