// 展示工具函数

export function fmtBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "-";
  if (n < 1024) return `${n} B`;
  const units = ["KiB", "MiB", "GiB", "TiB", "PiB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function fmtRate(bytesPerSec: number): string {
  return `${fmtBytes(bytesPerSec)}/s`;
}

export function handshakeAge(unixSec: number, now = Date.now() / 1000): string | null {
  if (!unixSec) return null;
  const age = now - unixSec;
  if (age < 0) return "刚刚";
  const m = Math.floor(age / 60);
  if (m < 1) return "刚刚";
  if (m < 60) return `${m} 分钟前`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} 小时前`;
  const d = Math.floor(h / 24);
  return `${d} 天前`;
}

export function handshakeAt(unixSec: number): string {
  if (!unixSec) return "从未";
  return new Date(unixSec * 1000).toLocaleString();
}

/** 密钥脱敏：保留前 8 字符，其余打码；可切换显示 */
export function maskKey(key: string, reveal: boolean): string {
  if (!key) return "-";
  if (reveal) return key;
  if (key.length <= 12) return "••••••••";
  return `${key.slice(0, 8)}•••`;
}

/** 短公钥：前 8 + 后 6 */
export function shortKey(key: string): string {
  if (!key) return "-";
  if (key.length <= 15) return key;
  return `${key.slice(0, 8)}…${key.slice(-6)}`;
}

/** AllowedIPs / Address 列表显示 */
export function joinList(items: string[]): string {
  return items.length ? items.join(", ") : "—";
}

export function fmtTime(unixSec: number): string {
  return new Date(unixSec * 1000).toLocaleTimeString();
}
