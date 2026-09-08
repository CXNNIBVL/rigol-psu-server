<script lang="ts">
  import type { DeviceView } from '../types';
  import { deviceStatus, TONAL } from '../format';
  import ChannelCard from './ChannelCard.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let { device }: { device: DeviceView } = $props();
  const status = $derived(deviceStatus(device));
</script>

<section class="space-y-3" data-testid="device-{device.slot}">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <div class="flex items-center gap-3">
      <h2 class="h3">{device.label}</h2>
      <StatusBadge {status} />
      {#if device.front_panel_locked}
        <span class="badge preset-tonal-surface" title="The instrument's front panel is locked while the server is driving it.">
          Panel locked
        </span>
      {/if}
    </div>
    <p class="text-xs opacity-60">{device.slot} &middot; {device.device}</p>
  </header>

  {#if status.detail}
    <p class="{TONAL[status.tone]} p-3 rounded text-sm" role="status" data-testid="device-detail">
      {status.detail}
    </p>
  {/if}

  <div class="grid gap-3 md:grid-cols-3">
    {#each device.channels as channel (channel.id)}
      <ChannelCard {device} {channel} />
    {/each}
  </div>
</section>
