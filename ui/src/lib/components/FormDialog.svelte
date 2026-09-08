<script lang="ts">
  /**
   * A modal for creating or editing one thing.
   *
   * Built on the native `<dialog>` element rather than Skeleton's headless
   * Dialog anatomy: the anatomy ships no styles at all, while the base package
   * styles `<dialog>` directly — and the platform then gives focus trapping,
   * Escape-to-close and an inert background for free rather than as things to
   * reimplement.
   *
   * Both editors used to push the list down the page while open, so you lost
   * sight of what you were editing against.
   */
  import type { Snippet } from 'svelte';

  let {
    open,
    title,
    onclose,
    children,
    testid,
  }: {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    testid?: string;
  } = $props();

  let element = $state<HTMLDialogElement | null>(null);

  $effect(() => {
    const dialog = element;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog
  bind:this={element}
  class="dialog card preset-outlined-surface-200-800 p-4 space-y-3 backdrop:bg-surface-950/60"
  style="--dialog-max-width: min(56rem, 92vw)"
  data-testid={testid}
  onclose={onclose}
>
  <header class="flex items-center justify-between gap-4">
    <h3 class="h4">{title}</h3>
    <button
      class="btn-icon btn-icon-sm opacity-70 hover:opacity-100"
      type="button"
      aria-label="Close"
      onclick={onclose}
    >
      <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true">
        <path d="M6 6l12 12M18 6L6 18" />
      </svg>
    </button>
  </header>

  <div class="max-h-[70vh] overflow-y-auto pr-1">
    {@render children()}
  </div>
</dialog>
