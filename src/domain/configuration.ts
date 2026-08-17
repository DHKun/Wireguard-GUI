import type { ApplyOutcome } from "../types";
import type { WireGuardPort } from "../wireguard";

export interface ConfigurationDocument {
  name: string;
  text: string;
  source: "stored" | "imported";
  dirty: boolean;
}

export interface ConfigurationApplyResult {
  document: ConfigurationDocument;
  outcome: ApplyOutcome;
  message: string;
}

export function isConfigurationName(name: string): boolean {
  return (
    name.length > 0 &&
    name.length <= 64 &&
    name.endsWith(".conf") &&
    !name.startsWith(".") &&
    /^[A-Za-z0-9_.-]+$/.test(name)
  );
}

export function describeApplyOutcome(success: string, outcome: ApplyOutcome): string {
  if (outcome.warnings.length > 0) return `⚠ ${outcome.warnings.join("；")}`;
  return success;
}

function suggestedName(fileName: string): string {
  const base = fileName.split(/[\\/]/).pop() || "imported.conf";
  if (isConfigurationName(base)) return base;
  const cleaned = base
    .replace(/[^A-Za-z0-9_.-]+/g, "-")
    .replace(/^\.+/, "")
    .replace(/\.conf$/i, "");
  const suggestion = `${cleaned || "imported"}.conf`;
  return isConfigurationName(suggestion) ? suggestion : "imported.conf";
}

/** Owns the Interface Configuration document lifecycle and Apply Outcome semantics. */
export class ConfigurationLifecycle {
  constructor(private readonly port: WireGuardPort) {}

  list() {
    return this.port.listConfigurations();
  }

  async open(name: string): Promise<ConfigurationDocument> {
    return {
      name,
      text: await this.port.readConfiguration(name),
      source: "stored",
      dirty: false,
    };
  }

  imported(fileName: string, text: string): ConfigurationDocument {
    return {
      name: suggestedName(fileName),
      text,
      source: "imported",
      dirty: true,
    };
  }

  edit(document: ConfigurationDocument, text: string): ConfigurationDocument {
    return { ...document, text, dirty: true };
  }

  rename(document: ConfigurationDocument, name: string): ConfigurationDocument {
    return { ...document, name, dirty: true };
  }

  async apply(
    document: ConfigurationDocument,
    synchronize: boolean,
  ): Promise<ConfigurationApplyResult> {
    if (!isConfigurationName(document.name)) {
      throw new Error("配置名必须以 .conf 结尾，且只能包含字母、数字、点、下划线和连字符");
    }
    const outcome = await this.port.applyConfiguration(
      document.name,
      document.text,
      synchronize,
    );
    const saved = { ...document, source: "stored" as const, dirty: false };
    return {
      document: saved,
      outcome,
      message: describeApplyOutcome(
        synchronize ? `已保存并热同步 ${document.name}` : `已保存 ${document.name}`,
        outcome,
      ),
    };
  }

  export(document: ConfigurationDocument, destination: string) {
    return this.port.exportConfiguration(document.name, destination);
  }
}
