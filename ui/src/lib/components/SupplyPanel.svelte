<script lang="ts">
  import type { DeviceView } from '../types';
  import { Collapsible } from '@skeletonlabs/skeleton-svelte';
  import { deviceStatus, TONAL } from '../format';
  import ChannelRow from './ChannelRow.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let { device }: { device: DeviceView } = $props();

  const status = $derived(deviceStatus(device));
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

  </header>

  <!--
    Metadata: the device node and the two serials. In flow rather than floating
    — as an absolutely positioned panel it was clipped by the table's own
    horizontal scroll container and could not be read at all.
  -->
  <Collapsible data-testid="identity-{device.slot}">
    <Collapsible.Trigger class="btn btn-sm preset-tonal self-start">
      <Collapsible.Indicator>
        <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M9 6l6 6-6 6" />
        </svg>
      </Collapsible.Indicator>
      Metadata
    </Collapsible.Trigger>
    <Collapsible.Content class="w-full">
      <dl class="text-xs grid gap-1 sm:grid-cols-3 rounded border border-surface-200-800 p-3">
        <div>
          <dt class="opacity-60">Slot</dt>
          <dd class="reading">{device.slot}</dd>
        </div>
        <div>
          <dt class="opacity-60">Device node</dt>
          <dd class="reading">{device.device}</dd>
        </div>
        <div>
          <dt class="opacity-60">Serial</dt>
          <dd class="reading">
            {device.reported_serial ?? 'Unknown'}
            {#if device.reported_serial && device.reported_serial !== device.expected_serial}
              <!-- Only worth stating when the two disagree; when they match,
                   printing both twice says nothing. -->
              <span class="text-warning-500">
                (configured as {device.expected_serial})
              </span>
            {/if}
          </dd>
        </div>
      </dl>
    </Collapsible.Content>
  </Collapsible>

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
