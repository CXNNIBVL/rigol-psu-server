<script lang="ts">
  /**
   * A modal for creating or editing one thing.
   *
   * Built on the native `<dialog>` element rather than Skeleton's headless
   * Dialog anatomy: the anatomy ships no styles at all, while the base package
   * styles `<dialog>` directly — and the platform then gives focus trapping,
   * Escape-to-close and an inert background for free rather than as things to
   * reimplement.
   */
  import type { Snippet } from 'svelte';

  let {
    open,
    title,
    onclose,
    children,
    footer,
    testid,
    dirty = false,
  }: {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    /**
     * Actions, kept out of the scrolling region rather than stuck over it.
     * Handed the same guarded close that Escape and the close control use, so
     * a Cancel button cannot quietly skip it.
     */
    footer?: Snippet<[() => void]>;
    testid?: string;
    /** Whether closing would throw away work. */
    dirty?: boolean;
  } = $props();

  let element = $state<HTMLDialogElement | null>(null);
  let confirming = $state(false);

  $effect(() => {
    const dialog = element;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
    if (!open) confirming = false;
  });

  /**
   * Escape and the close control used to discard everything silently. A profile
   * can be several rails of work, so it asks — but only when there is something
   * to lose, since a confirmation on an untouched form is just an obstacle.
   */
  function attemptClose() {
    if (dirty) {
      confirming = true;
      return;
    }
    onclose();
  }
</script>

<dialog
  bind:this={element}
  class="dialog card preset-outlined-surface-200-800 p-4 space-y-3 backdrop:bg-surface-950/60"
  style="--dialog-max-width: min(56rem, 92vw)"
  data-testid={testid}
  oncancel={(event) => {
    // Escape: intercept so the guard gets a say.
    event.preventDefault();
    attemptClose();
  }}
  onclose={onclose}
>
  <header class="flex items-center justify-between gap-4">
    <h3 class="h4">{title}</h3>
    <button
      class="btn-icon btn-icon-lg opacity-70 hover:opacity-100"
      type="button"
      aria-label="Close"
      onclick={attemptClose}
    >
      <svg class="size-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true">
        <path d="M6 6l12 12M18 6L6 18" />
      </svg>
    </button>
  </header>

  {#if confirming}
    <div
      class="preset-tonal-warning p-3 rounded space-y-2"
      role="alertdialog"
      aria-label="Unsaved changes"
      data-testid="confirm-discard"
    >
      <p class="text-sm">Close without saving? The changes here will be lost.</p>
      <div class="flex gap-2">
        <button
          class="btn btn-sm preset-filled-warning-500"
          type="button"
          onclick={() => {
            confirming = false;
            onclose();
          }}
          data-testid="confirm-discard-yes"
        >
          Discard changes
        </button>
        <button
          class="btn btn-sm preset-tonal"
          type="button"
          onclick={() => (confirming = false)}
        >
          Keep editing
        </button>
      </div>
    </div>
  {/if}

  <div class="max-h-[65vh] overflow-y-auto pr-2">
    {@render children()}
  </div>

  {#if footer}
    <!--
      Outside the scroll region, not stuck to the bottom of it. As a sticky
      element inside, its background band ran under the scrollbar gutter and cut
      across whatever it happened to overlap.
    -->
    <footer class="flex gap-2 pt-3 border-t border-surface-200-800">
      {@render footer(attemptClose)}
    </footer>
  {/if}
</dialog>
