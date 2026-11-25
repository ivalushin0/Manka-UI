<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    open = $bindable(false),
    summary,
    actions,
    children,
    tone = "",
  }: {
    open?: boolean;
    summary: Snippet;
    actions?: Snippet;
    children?: Snippet;
    tone?: string;
  } = $props();
</script>

<div class="disclosure {tone}" class:open>
  <div class="head">
    <button class="toggle" type="button" aria-expanded={open} onclick={() => (open = !open)}>
      <span class="chev"><Icon name="chevron" size={14} /></span>
      <span class="summary">{@render summary()}</span>
    </button>
    {#if actions}
      <div class="actions" role="presentation" onclick={(e) => e.stopPropagation()}>{@render actions()}</div>
    {/if}
  </div>
  {#if open}
    <div class="body">{@render children?.()}</div>
  {/if}
</div>

<style>
  .disclosure {
    border: 1px solid var(--line-soft);
    border-radius: 12px;
    background: rgba(15, 21, 34, 0.55);
    overflow: hidden;
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .disclosure.open,
  .disclosure:hover {
    border-color: #2a3a59;
    background: rgba(17, 25, 41, 0.8);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 11px 13px;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .summary {
    flex: 1;
    min-width: 0;
  }

  .chev {
    color: var(--dim);
    display: grid;
    place-items: center;
    transition: transform 0.16s ease;
    flex: none;
  }

  .disclosure.open .chev {
    transform: rotate(90deg);
    color: var(--accent-2);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 11px;
    flex: none;
  }

  .body {
    border-top: 1px solid var(--line-soft);
    padding: 13px;
    animation: reveal 0.16s ease;
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
  }
</style>
