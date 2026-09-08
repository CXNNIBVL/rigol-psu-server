<script lang="ts">
  import type { DeviceView } from '../types';
  import { deviceStatus, TONAL } from '../format';
  import ChannelRow from './ChannelRow.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let { device }: { device: DeviceView } = $props();

  const status = $derived(deviceStatus(device));
  const live = $derived(device.channels.filter((c) => c.output_on === true).length);
  const draw = $derived(
    device.channels.reduce((total, c) => total + (c.measurement?.power ?? 0), 0),
  );
</script>

<!--
  A DP832 is one object with one face and three outputs, so its channels are
  rows of a single instrument rather than three cards that happen to sit near
  each other. That is what lets the column headings appear once instead of nine
  times, and what makes a column of currents comparable down the page.
-->
<section
  class="card preset-outlined-surface-200-800 p-4 space-y-3 relative"
  data-testid="device-{device.slot}"
>
  <header class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
    <div class="flex items-center gap-3">
      <h2 class="h3">{device.label}</h2>
      <StatusBadge {status} size="lg" />
      {#if device.front_panel_locked}
        <span
          class="badge preset-tonal-surface text-sm px-3 py-1"
          title="The front panel is locked while the server is driving this supply."
        >
          Panel locked
        </span>
      {/if}
    </div>

    <!-- One shape whatever the state: the same two facts in the same place,
         rather than three different strings depending on what is happening. -->
    <div class="flex items-center gap-4">
      {#if device.state !== 'offline'}
        <p class="reading text-sm tabular-nums">
          <span class="font-semibold">{live}</span>
          <span class="opacity-70">of {device.channels.length} rails live</span>
        </p>
        <div class="vr h-4"></div>
        <p class="reading text-sm tabular-nums">
          <span class="font-semibold">{draw.toFixed(2)}</span>
          <span class="opacity-70">W</span>
        </p>
        <div class="vr h-4"></div>
      {/if}

      <!--
        Identity, on demand. Slot, device node and the two serials are reference
        facts: needed rarely, and needed precisely when something is wrong. They
        had a whole section to themselves and duplicated everything around them.
      -->
      <details class="text-xs" data-testid="identity-{device.slot}">
        <summary class="cursor-pointer opacity-60 hover:opacity-100 select-none">
          {device.slot}
        </summary>
        <dl class="absolute z-10 mt-1 card preset-outlined-surface-200-800 p-3 space-y-1 bg-surface-100-900">
          <div class="flex gap-3">
            <dt class="opacity-60 w-32">Device node</dt>
            <dd class="reading">{device.device}</dd>
          </div>
          <div class="flex gap-3">
            <dt class="opacity-60 w-32">Configured serial</dt>
            <dd class="reading">{device.expected_serial}</dd>
          </div>
          <div class="flex gap-3">
            <dt class="opacity-60 w-32">Reported serial</dt>
            <!-- Unknown, not blank: we have not heard from the instrument. -->
            <dd class="reading">{device.reported_serial ?? 'Unknown'}</dd>
          </div>
        </dl>
      </details>
    </div>
  </header>

  {#if status.detail}
    <p class="{TONAL[status.tone]} p-3 rounded text-sm" role="status" data-testid="device-detail">
      {status.detail}
    </p>
  {/if}

  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="text-xs opacity-60">
          <th scope="col" class="pb-1 pr-3 text-left font-normal">Channel</th>
          <th scope="col" class="pb-1 px-3 text-right font-normal">Voltage</th>
          <th scope="col" class="pb-1 px-3 text-right font-normal">Current</th>
          <th scope="col" class="pb-1 px-3 text-right font-normal">Power</th>
          <th scope="col" class="pb-1 px-3 text-left font-normal">Setpoint</th>
          <th scope="col" class="pb-1 pl-3 text-right font-normal">Output</th>
        </tr>
      </thead>
      {#each device.channels as channel (channel.id)}
        <ChannelRow {device} {channel} />
      {/each}
    </table>
  </div>
</section>
