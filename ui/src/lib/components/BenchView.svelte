<script lang="ts">
  import type { DeviceView } from '../types';
  import { attentionText, countNeedingAttention } from '../bench-summary';
  import SupplyPanel from './SupplyPanel.svelte';

  let { devices, connected }: { devices: DeviceView[]; connected: boolean } = $props();

  const needAttention = $derived(countNeedingAttention(devices));
</script>

<section class="space-y-4">
  <h1 class="sr-only">Bench</h1>

  <!--
    A header that appears only when it has something to say. It previously
    carried a live-rail count and a running total; neither prompted any action,
    and both restated what the table underneath already shows.
  -->
  {#if needAttention > 0 || !connected}
    <header class="flex flex-wrap items-center gap-3">
      {#if needAttention > 0}
        <p class="badge preset-tonal-warning text-sm px-3 py-1" data-testid="bench-attention">
          {attentionText(needAttention)}
        </p>
      {/if}
      {#if !connected}
        <p class="badge preset-tonal-warning text-sm px-3 py-1">Readings are unknown</p>
      {/if}
    </header>
  {/if}

  {#if devices.length === 0}
    <p class="opacity-60 text-sm">
      No supplies are configured. Check the server's config file.
    </p>
  {/if}

  <div class="space-y-4">
    {#each devices as device (device.slot)}
      <SupplyPanel {device} />
    {/each}
  </div>
</section>
