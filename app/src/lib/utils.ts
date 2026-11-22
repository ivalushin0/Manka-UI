import { writeText } from "@tauri-apps/plugin-clipboard-manager";

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
  if (!sec) return "0 с";
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h) return `${h} ч ${m} мин`;
  if (m) return `${m} мин ${s} с`;
  return `${s} с`;
}

export function fmtTime(ms: number): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return d.toLocaleString("ru-RU", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
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
  if (!bytes) return "0 Б";
  if (bytes < 1024) return `${bytes} Б`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} КБ`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} МБ`;
}

export function sourceTitle(source: string): string {
  switch (source) {
    case "builtin":
      return "Встроенная";
    case "store":
      return "Магазин";
    case "auto":
      return "Автоподбор";
    case "user":
      return "Своя";
    case "catalog":
      return "Каталог";
    case "external":
      return "Внешняя";
    case "zapretgui":
      return "Zapret GUI";
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
