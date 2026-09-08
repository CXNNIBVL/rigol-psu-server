<script lang="ts">
  let {
    value = $bindable(''),
    label,
    count,
    total,
  }: { value: string; label: string; count: number; total: number } = $props();
</script>

<div class="flex items-center gap-3">
  <input
    class="input w-72"
    type="search"
    placeholder={label}
    aria-label={label}
    bind:value
  />
  <!-- States the filter's effect rather than leaving you to count rows. Silent
       when nothing is filtered: a bare total beside an empty box is a number
       with no question attached to it, and the lists say how many they hold. -->
  {#if value}
    <span class="text-xs opacity-60" aria-live="polite">
      {count} of {total}
    </span>
  {/if}
</div>
