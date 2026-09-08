<script lang="ts">
  /**
   * The three actions that recur across every list: add, edit, remove.
   *
   * Icon-only, so an accessible name is required rather than optional — there
   * is no visible text to fall back on, and a bare glyph is not a label. The
   * name is also the tooltip, so hovering tells you the same thing a screen
   * reader is told.
   *
   * Colour arrives on hover only. At rest these are quiet: a row of three
   * coloured buttons per item would make the actions louder than the content
   * they act on, and a red bin sitting permanently lit is the loudest thing on
   * any list.
   */
  import type { Snippet } from 'svelte';

  let {
    action,
    label,
    onclick,
    disabled = false,
    title,
    testid,
  }: {
    action: 'new' | 'edit' | 'delete';
    label: string;
    onclick?: () => void;
    disabled?: boolean;
    title?: string;
    testid?: string;
  } = $props();

  // Whole literal strings — Tailwind cannot see a class assembled at runtime.
  const TONE = {
    new: 'hover:text-success-500 focus-visible:text-success-500',
    edit: 'hover:text-warning-500 focus-visible:text-warning-500',
    delete: 'hover:text-error-500 focus-visible:text-error-500',
  } as const;
</script>

<button
  type="button"
  class="btn-icon btn-icon-sm opacity-70 hover:opacity-100 transition-colors {TONE[action]}"
  aria-label={label}
  title={title ?? label}
  {disabled}
  {onclick}
  data-testid={testid}
  data-action={action}
>
  <svg
    class="size-4"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    {#if action === 'new'}
      <path d="M12 5v14M5 12h14" />
    {:else if action === 'edit'}
      <!-- Pencil -->
      <path d="M4 20h4L19 9a2.1 2.1 0 0 0-3-3L5 17v3z" />
      <path d="M14.5 6.5l3 3" />
    {:else}
      <!-- Bin -->
      <path d="M4 7h16M10 7V5h4v2M6 7l1 13h10l1-13" />
      <path d="M10 11v6M14 11v6" />
    {/if}
  </svg>
</button>
