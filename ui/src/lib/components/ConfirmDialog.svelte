<script lang="ts">
  /**
   * A modal that asks before something you cannot take back.
   *
   * Native `<dialog>` for the same reasons as {@link FormDialog}: the platform
   * gives focus trapping, Escape-to-close and an inert background, and Skeleton
   * styles the element directly. Inline, these questions were a coloured block
   * that appeared somewhere on the page — inside a card for a power cycle, at
   * the foot of the section for a delete — which left it possible to ask and be
   * scrolled away from the asking.
   */
  import type { Snippet } from 'svelte';

  let {
    open,
    title,
    confirmLabel,
    cancelLabel,
    tone = 'error',
    onconfirm,
    oncancel,
    children,
    testid,
  }: {
    open: boolean;
    title: string;
    /** Names the act, not the answer: "Power cycle", never "Yes". */
    confirmLabel: string;
    /** Names the alternative, so neither button is the absence of a choice. */
    cancelLabel: string;
    tone?: 'error' | 'warning';
    onconfirm: () => void;
    oncancel: () => void;
    children: Snippet;
    testid?: string;
  } = $props();

  let element = $state<HTMLDialogElement | null>(null);
  let cancelButton = $state<HTMLButtonElement | null>(null);

  // Whole literal strings — Tailwind cannot see an assembled one.
  const CONFIRM = {
    error: 'btn preset-filled-error-500',
    warning: 'btn preset-filled-warning-500',
  };

  $effect(() => {
    const dialog = element;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      // Focus lands on the way out, not on the irreversible thing. Return is
      // the common answer to a question you did not mean to ask.
      cancelButton?.focus();
    }
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog
  bind:this={element}
  class="dialog card preset-outlined-surface-200-800 p-4 space-y-3 backdrop:bg-surface-950/60"
  style="--dialog-max-width: min(32rem, 92vw)"
  data-testid={testid}
  oncancel={(event) => {
    // Escape answers "no", which is the safe direction for every question this
    // dialog asks.
    event.preventDefault();
    oncancel();
  }}
>
  <h3 class="h4">{title}</h3>

  <div class="text-sm">
    {@render children()}
  </div>

  <footer class="flex gap-2 pt-3 border-t border-surface-200-800">
    <button
      class={CONFIRM[tone]}
      type="button"
      onclick={onconfirm}
      data-testid={testid ? `${testid}-yes` : undefined}
    >
      {confirmLabel}
    </button>
    <button
      bind:this={cancelButton}
      class="btn preset-tonal"
      type="button"
      onclick={oncancel}
    >
      {cancelLabel}
    </button>
  </footer>
</dialog>
