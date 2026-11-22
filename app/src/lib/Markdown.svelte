<script lang="ts">
  import { api } from "./api";

  let { text = "" }: { text?: string } = $props();

  function esc(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function inline(s: string): string {
    let out = esc(s);
    out = out.replace(/`([^`]+)`/g, "<code>$1</code>");
    out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    out = out.replace(/\*([^*]+)\*/g, "<em>$1</em>");
    out = out.replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<a data-href="$2">$1</a>');
    out = out.replace(
      /(^|[\s(])(https?:\/\/[^\s<)]+)/g,
      '$1<a data-href="$2">$2</a>',
    );
    return out;
  }

  type Block =
    | { kind: "h"; level: number; text: string }
    | { kind: "p"; text: string }
    | { kind: "quote"; text: string }
    | { kind: "hr" }
    | { kind: "list"; ordered: boolean; items: string[] }
    | { kind: "code"; lang: string; text: string }
    | { kind: "table"; head: string[]; rows: string[][] };

  function splitRow(line: string): string[] {
    return line
      .replace(/^\s*\|/, "")
      .replace(/\|\s*$/, "")
      .split("|")
      .map((c) => c.trim());
  }

  function parse(md: string): Block[] {
    const lines = md.replace(/\r\n?/g, "\n").split("\n");
    const out: Block[] = [];
    let i = 0;
    const isTableSep = (s: string) => /^\s*\|?[\s:|-]+\|[\s:|-]*$/.test(s) && s.includes("-");

    while (i < lines.length) {
      const line = lines[i];
      if (!line.trim()) {
        i++;
        continue;
      }
      if (/^\s*```/.test(line)) {
        const lang = line.replace(/^\s*```/, "").trim();
        const buf: string[] = [];
        i++;
        while (i < lines.length && !/^\s*```/.test(lines[i])) buf.push(lines[i++]);
        i++;
        out.push({ kind: "code", lang, text: buf.join("\n") });
        continue;
      }
      if (/^\s*(---+|\*\*\*+)\s*$/.test(line)) {
        out.push({ kind: "hr" });
        i++;
        continue;
      }
      const h = line.match(/^(#{1,6})\s+(.*)$/);
      if (h) {
        out.push({ kind: "h", level: h[1].length, text: h[2] });
        i++;
        continue;
      }
      if (/^\s*>\s?/.test(line)) {
        const buf: string[] = [];
        while (i < lines.length && /^\s*>\s?/.test(lines[i])) {
          buf.push(lines[i].replace(/^\s*>\s?/, ""));
          i++;
        }
        out.push({ kind: "quote", text: buf.join(" ") });
        continue;
      }
      if (/^\s*[-*+]\s+/.test(line)) {
        const items: string[] = [];
        while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i])) {
          items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
          i++;
        }
        out.push({ kind: "list", ordered: false, items });
        continue;
      }
      if (/^\s*\d+\.\s+/.test(line)) {
        const items: string[] = [];
        while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
          items.push(lines[i].replace(/^\s*\d+\.\s+/, ""));
          i++;
        }
        out.push({ kind: "list", ordered: true, items });
        continue;
      }
      if (line.includes("|") && i + 1 < lines.length && isTableSep(lines[i + 1])) {
        const head = splitRow(line);
        i += 2;
        const rows: string[][] = [];
        while (i < lines.length && lines[i].includes("|") && lines[i].trim()) {
          rows.push(splitRow(lines[i]));
          i++;
        }
        out.push({ kind: "table", head, rows });
        continue;
      }
      const buf: string[] = [];
      while (
        i < lines.length &&
        lines[i].trim() &&
        !/^\s*(```|#{1,6}\s|[-*+]\s+|\d+\.\s+|>\s?)/.test(lines[i])
      ) {
        buf.push(lines[i].trim());
        i++;
      }
      out.push({ kind: "p", text: buf.join(" ") });
    }
    return out;
  }

  const blocks = $derived(parse(text));

  function follow(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest?.("a[data-href]") as HTMLElement | null;
    if (!a) return;
    e.preventDefault();
    const href = a.dataset.href;
    if (href) void api.openUrl(href);
  }
</script>

<div class="md" onclick={follow} role="presentation">
  {#each blocks as b}
    {#if b.kind === "h"}
      <div class="md-h" class:h1={b.level === 1} class:h2={b.level === 2} class:h3={b.level >= 3}>
        {@html inline(b.text)}
      </div>
    {:else if b.kind === "p"}
      <p>{@html inline(b.text)}</p>
    {:else if b.kind === "quote"}
      <blockquote>{@html inline(b.text)}</blockquote>
    {:else if b.kind === "hr"}
      <hr />
    {:else if b.kind === "list"}
      {#if b.ordered}
        <ol>
          {#each b.items as item}<li>{@html inline(item)}</li>{/each}
        </ol>
      {:else}
        <ul>
          {#each b.items as item}<li>{@html inline(item)}</li>{/each}
        </ul>
      {/if}
    {:else if b.kind === "code"}
      <pre class="md-code"><code>{b.text}</code></pre>
    {:else if b.kind === "table"}
      <div class="md-table">
        <table>
          <thead>
            <tr>{#each b.head as c}<th>{@html inline(c)}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each b.rows as row}
              <tr>{#each row as c}<td>{@html inline(c)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/each}
</div>

<style>
  .md {
    font-size: 13px;
    line-height: 1.65;
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .md :global(p) {
    margin: 0 0 10px;
  }

  .md :global(p:last-child) {
    margin-bottom: 0;
  }

  .md-h {
    color: var(--text);
    font-weight: 650;
    margin: 16px 0 8px;
    line-height: 1.35;
  }
  .md-h:first-child {
    margin-top: 0;
  }
  .md-h.h1 {
    font-size: 19px;
  }
  .md-h.h2 {
    font-size: 16px;
  }
  .md-h.h3 {
    font-size: 13.5px;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    color: var(--muted);
  }

  .md :global(ul),
  .md :global(ol) {
    margin: 4px 0 10px;
    padding-left: 20px;
  }

  .md :global(li) {
    margin: 3px 0;
  }

  .md :global(a) {
    color: var(--accent-2);
    text-decoration: none;
    border-bottom: 1px solid rgba(34, 211, 238, 0.35);
    cursor: pointer;
  }

  .md :global(a:hover) {
    border-bottom-color: var(--accent-2);
  }

  .md :global(code) {
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    background: #0b1220;
    border: 1px solid var(--line-soft);
    border-radius: 6px;
    padding: 1px 5px;
    color: #c7d5f4;
  }

  .md :global(blockquote) {
    margin: 8px 0;
    padding: 8px 12px;
    border-left: 3px solid rgba(124, 92, 255, 0.55);
    background: rgba(124, 92, 255, 0.07);
    border-radius: 0 8px 8px 0;
    color: var(--muted);
  }

  .md :global(hr) {
    border: 0;
    height: 1px;
    background: var(--line-soft);
    margin: 14px 0;
  }

  .md-code {
    margin: 8px 0 12px;
    padding: 11px 13px;
    background: #070b13;
    border: 1px solid var(--line-soft);
    border-radius: 10px;
    overflow: auto;
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    line-height: 1.55;
    color: #b7c3da;
    white-space: pre-wrap;
  }

  .md-table {
    overflow: auto;
    margin: 8px 0 12px;
  }

  .md :global(table) {
    border-collapse: collapse;
    font-size: 12px;
    min-width: 100%;
  }

  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--line-soft);
    padding: 6px 9px;
    text-align: left;
    vertical-align: top;
  }

  .md :global(th) {
    background: rgba(124, 92, 255, 0.1);
    color: var(--text);
    font-weight: 600;
  }
</style>
