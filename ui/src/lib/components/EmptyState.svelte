<script lang="ts">
  /**
   * An empty screen is an invitation to act, not a filing cabinet with nothing
   * in it. Both lists used to show a search box, a count of zero and a full set
   * of column headings over no rows, with prose pointing at an icon in the far
   * corner — chrome for content that does not exist yet.
   */
  import type { Snippet } from 'svelte';

  let {
    title,
    children,
    action,
    onAction,
    testid,
  }: {
    title: string;
    children: Snippet;
    action: string;
    onAction: () => void;
    testid?: string;
  } = $props();
</script>

<div
  class="card preset-outlined-surface-200-800 p-8 flex flex-col items-start gap-3 max-w-2xl"
  data-testid={testid}
>
  <h3 class="h4">{title}</h3>
  <p class="text-sm opacity-70">{@render children()}</p>
  <!-- The action is here, where the reader is looking, rather than described. -->
  <button class="btn preset-filled" type="button" onclick={onAction}>
    {action}
  </button>
</div>
