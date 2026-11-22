<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    title,
    subtitle = "",
    wide = false,
    onclose,
    children,
  }: {
    title: string;
    subtitle?: string;
    wide?: boolean;
    onclose: () => void;
    children?: import("svelte").Snippet;
  } = $props();

  function onkey(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<div
  class="overlay"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div class="modal" class:wide role="dialog" aria-modal="true" aria-label={title}>
    <header>
      <div class="grow">
        <h3>{title}</h3>
        {#if subtitle}<p class="small muted">{subtitle}</p>{/if}
      </div>
      <button class="icon-btn close" aria-label="Закрыть" onclick={onclose}>
        <Icon name="x" size={17} />
      </button>
    </header>
    <div class="modal-body">{@render children?.()}</div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(4, 7, 14, 0.72);
    backdrop-filter: blur(5px);
    display: grid;
    place-items: center;
    padding: 32px;
    animation: fade 0.14s ease;
  }

  .modal {
    width: min(680px, 100%);
    max-height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: linear-gradient(180deg, #182133, #111827);
    border: 1px solid #2b3a58;
    border-radius: 18px;
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.55);
    animation: pop 0.16s ease;
  }

  .modal.wide {
    width: min(920px, 100%);
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 20px 22px 16px;
    border-bottom: 1px solid #1e2a41;
  }

  h3 {
    margin: 0;
    font-size: 17px;
    font-weight: 650;
    line-height: 1.35;
  }

  header p {
    margin-top: 4px;
  }

  .modal-body {
    padding: 20px 22px 24px;
    overflow: auto;
    min-height: 0;
  }

  .close {
    color: var(--muted);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes pop {
    from {
      transform: translateY(8px) scale(0.985);
      opacity: 0;
    }
  }
</style>
