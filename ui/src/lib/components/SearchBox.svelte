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
  <!-- States the filter's effect rather than leaving you to count rows. -->
  <span class="text-xs opacity-60" aria-live="polite">
    {value ? `${count} of ${total}` : `${total}`}
  </span>
</div>
