<script lang="ts">
  import type { DeviceView } from '../types';
  import { attentionText, summarise } from '../bench-summary';
  import SupplyPanel from './SupplyPanel.svelte';

  let { devices, connected }: { devices: DeviceView[]; connected: boolean } = $props();

  const summary = $derived(summarise(devices));
</script>

<section class="space-y-4">
  <!--
    The bench's headline is what the bench is doing, not the words "PSU Bench" —
    which the rail already says and which answers nothing. Three parallel facts,
    separated by rules rather than joined by a dot.
  -->
  <header class="flex flex-wrap items-center gap-x-6 gap-y-2">
    <h1 class="sr-only">Bench</h1>

    <p class="reading tabular-nums">
      <span class="text-2xl font-semibold">{summary.liveRails}</span>
      <span class="opacity-70">of {summary.totalRails} rails live</span>
    </p>

    <div class="vr h-8"></div>

    <p class="reading tabular-nums">
      <span class="text-2xl font-semibold">{summary.watts.toFixed(2)}</span>
      <span class="opacity-70">W total</span>
    </p>

    {#if summary.needAttention > 0}
      <div class="vr h-8"></div>
      <p class="badge preset-tonal-warning text-sm px-3 py-1" data-testid="bench-attention">
        {attentionText(summary.needAttention)}
      </p>
    {/if}

    {#if !connected}
      <div class="vr h-8"></div>
      <p class="badge preset-tonal-warning text-sm px-3 py-1">Readings are unknown</p>
    {/if}
  </header>

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
