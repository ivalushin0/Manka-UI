<script lang="ts">
  let { name, size = 18 }: { name: string; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "home"}
    <path d="M3 10.8 12 3l9 7.8" />
    <path d="M5 9.8V21h14V9.8" />
    <path d="M9.5 21v-6h5v6" />
  {:else if name === "layers"}
    <path d="m12 3 9 4.5-9 4.5-9-4.5L12 3Z" />
    <path d="m3 12 9 4.5 9-4.5" />
    <path d="m3 16.5 9 4.5 9-4.5" />
  {:else if name === "wand"}
    <path d="m4 20 11-11" />
    <path d="m14 4 1 2.2L17 7l-2 1L14 10l-1-2L11 7l2-1 1-2Z" />
    <path d="m19 12 .7 1.6L21 14l-1.3.6L19 16l-.7-1.4L17 14l1.3-.4L19 12Z" />
  {:else if name === "package"}
    <path d="m12 3 8 4.2v9.6L12 21l-8-4.2V7.2L12 3Z" />
    <path d="m4.3 7.4 7.7 4.1 7.7-4.1" />
    <path d="M12 11.5V21" />
  {:else if name === "send"}
    <path d="M21 3 3 10.5l6.5 2.9L21 3Z" />
    <path d="M21 3 14.5 21l-2.9-6.5" />
    <path d="M9.5 13.4 21 3" />
  {:else if name === "sliders"}
    <path d="M4 6h9" />
    <path d="M17 6h3" />
    <path d="M4 12h3" />
    <path d="M11 12h9" />
    <path d="M4 18h11" />
    <path d="M19 18h1" />
    <circle cx="15" cy="6" r="2" />
    <circle cx="9" cy="12" r="2" />
    <circle cx="17" cy="18" r="2" />
  {:else if name === "terminal"}
    <rect x="3" y="4" width="18" height="16" rx="3" />
    <path d="m7 9 3 3-3 3" />
    <path d="M13 15h4" />
  {:else if name === "power"}
    <path d="M12 3v9" />
    <path d="M6.3 6.3a8 8 0 1 0 11.4 0" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 0 0-14-4.5L4 9" />
    <path d="M4 4v5h5" />
    <path d="M4 13a8 8 0 0 0 14 4.5L20 15" />
    <path d="M20 20v-5h-5" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.5-3.5" />
  {:else if name === "copy"}
    <rect x="9" y="9" width="12" height="12" rx="2.5" />
    <path d="M15 5.5A2.5 2.5 0 0 0 12.5 3H6a3 3 0 0 0-3 3v6.5A2.5 2.5 0 0 0 5.5 15" />
  {:else if name === "external"}
    <path d="M14 4h6v6" />
    <path d="M20 4 11 13" />
    <path d="M19 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5" />
  {:else if name === "check"}
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  {:else if name === "x"}
    <path d="m6 6 12 12" />
    <path d="m18 6-12 12" />
  {:else if name === "play"}
    <path d="m8 5 11 7-11 7V5Z" />
  {:else if name === "stop"}
    <rect x="6" y="6" width="12" height="12" rx="2" />
  {:else if name === "trash"}
    <path d="M4 7h16" />
    <path d="M9 7V4h6v3" />
    <path d="m6 7 1 13h10l1-13" />
    <path d="M10 11v5M14 11v5" />
  {:else if name === "bolt"}
    <path d="M13 2 4 14h6l-1 8 9-12h-6l1-8Z" />
  {:else if name === "download"}
    <path d="M12 3v12" />
    <path d="m7 10 5 5 5-5" />
    <path d="M4 20h16" />
  {:else if name === "shield"}
    <path d="M12 3 5 6v6c0 4.2 2.9 7.4 7 9 4.1-1.6 7-4.8 7-9V6l-7-3Z" />
    <path d="m9 12 2 2 4-4" />
  {:else if name === "clock"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v5l3 2" />
  {:else if name === "chevron"}
    <path d="m9 6 6 6-6 6" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5" />
    <path d="M12 8h.01" />
  {:else if name === "file"}
    <path d="M7 3h7l4 4v14H7V3Z" />
    <path d="M14 3v4h4" />
  {:else}
    <circle cx="12" cy="12" r="8" />
  {/if}
</svg>
