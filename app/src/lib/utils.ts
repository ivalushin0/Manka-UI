import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { locale, t } from "./i18n.svelte";

export async function copyText(text: string): Promise<boolean> {
  try {
    await writeText(text);
    return true;
  } catch {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      return false;
    }
  }
}

export function fmtUptime(sec: number): string {
  if (!sec) return `0 ${t("units.sec")}`;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h) return `${h} ${t("units.hour")} ${m} ${t("units.min")}`;
  if (m) return `${m} ${t("units.min")} ${s} ${t("units.sec")}`;
  return `${s} ${t("units.sec")}`;
}

export function fmtTime(ms: number): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return d.toLocaleString(locale(), { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
}

export function percent(ok: number, total: number): number {
  return total ? Math.round((ok / total) * 100) : 0;
}

export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

export function fmtBytes(bytes: number): string {
  if (!bytes) return `0 ${t("units.b")}`;
  if (bytes < 1024) return `${bytes} ${t("units.b")}`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} ${t("units.kb")}`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} ${t("units.mb")}`;
}

export function sourceTitle(source: string): string {
  switch (source) {
    case "builtin":
    case "store":
    case "auto":
    case "user":
    case "catalog":
    case "external":
    case "zapretgui":
      return t(`source.${source}`);
    default:
      return source;
  }
}

/** Strip the heavy Markdown of CDPI store descriptions for card previews. */
export function cleanMarkdown(text: string): string {
  return text
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[*_`>#]/g, "")
    .replace(/\r?\n+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
