<script lang="ts">
  import type { ChannelView, DeviceView } from '../types';
  import {
    canEnergise,
    channelBadge,
    channelStatus,
    CHANNEL_CARD,
    channelPresentation,
    formatAmps,
    formatVolts,
    formatWatts,
    isWritable,
    TONAL,
    UNKNOWN,
  } from '../format';
  import { SegmentedControl } from '@skeletonlabs/skeleton-svelte';
  import { ApiFailure, setOutput, setSetpoint } from '../api';
  import StatusBadge from './StatusBadge.svelte';

  let { device, channel }: { device: DeviceView; channel: ChannelView } = $props();

  let volts = $state('');
  let amps = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  const status = $derived(channelStatus(channel));
  // A live rail gets a green border and full opacity; an off one is dimmed.
  // Neither needs a badge to say so.
  const presentation = $derived(channelPresentation(channel));
  const badge = $derived(channelBadge(channel));
  const writable = $derived(isWritable(device, channel));
  const energisable = $derived(canEnergise(device, channel));

  // Readings are null when the supply is not answering. They are rendered as
  // "Unknown", never as zero: the rail may still be live.
  const volt = $derived(formatVolts(channel.measurement?.volt ?? null));
  const curr = $derived(formatAmps(channel.measurement?.curr ?? null));
  const power = $derived(formatWatts(channel.measurement?.power ?? null));

  async function apply(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    busy = true;
    try {
      await setSetpoint(device.slot, channel.id, {
        volt: Number(volts),
        curr_limit: Number(amps),
      });
      volts = '';
      amps = '';
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }

  // Empty string means no selection: when the supply is not answering we do not
  // know which way this rail is set, and the control must not claim otherwise
  // by defaulting to "Off".
  const outputValue = $derived(
    channel.output_on === null ? '' : channel.output_on ? 'on' : 'off',
  );

  async function toggle(on: boolean) {
    error = null;
    busy = true;
    try {
      await setOutput(device.slot, channel.id, on);
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<article
  class="card p-4 space-y-3 {CHANNEL_CARD[presentation]}"
  data-testid="channel-{device.slot}-{channel.id}"
  data-state={presentation}
>
  <header class="flex items-center justify-between gap-2 min-h-7">
    <h3 class="h4">{channel.id}</h3>
    {#if badge}
      <StatusBadge status={badge} />
    {/if}
  </header>

  <dl class="reading grid grid-cols-3 gap-2 text-center">
    <div>
      <dt class="text-xs opacity-60">Voltage</dt>
      <dd class="text-lg" data-testid="volt">{volt}</dd>
    </div>
    <div>
      <dt class="text-xs opacity-60">Current</dt>
      <dd class="text-lg" data-testid="curr">{curr}</dd>
    </div>
    <div>
      <dt class="text-xs opacity-60">Power</dt>
      <dd class="text-lg" data-testid="power">{power}</dd>
    </div>
  </dl>

  <p class="text-xs opacity-60">
    Limits {channel.limits.max_volt.toFixed(3)} V / {channel.limits.max_curr.toFixed(3)} A
    &middot; setpoint
    {channel.setpoint ? `${channel.setpoint.volt.toFixed(3)} V` : UNKNOWN}
  </p>

  {#if status.detail}
    <p class="text-xs {TONAL[status.tone]} p-2 rounded" data-testid="channel-detail">
      {status.detail}
    </p>
  {/if}

  <!--
    `novalidate` is deliberate. The `max` attributes below still drive the
    spinners and assistive technology, but native constraint validation would
    silently refuse to submit an out-of-range value and show a generic browser
    tooltip instead. The server's clamp message names the requested value and
    the limit, and it is the same message however the value got there — so the
    server stays the single authority and the user always gets the useful text.
  -->
  <form class="flex gap-2" onsubmit={apply} novalidate>
    <input
      class="input"
      type="number"
      step="0.001"
      min="0"
      max={channel.limits.max_volt}
      placeholder="V"
      aria-label="{channel.id} voltage"
      bind:value={volts}
      disabled={!writable || busy}
    />
    <input
      class="input"
      type="number"
      step="0.001"
      min="0"
      max={channel.limits.max_curr}
      placeholder="A"
      aria-label="{channel.id} current limit"
      bind:value={amps}
      disabled={!writable || busy}
    />
    <button class="btn preset-filled" type="submit" disabled={!writable || busy || !volts || !amps}>
      Set
    </button>
  </form>

  <SegmentedControl
    value={outputValue}
    onValueChange={(details) => toggle(details.value === 'on')}
    disabled={busy || device.state === 'offline'}
    aria-label="{channel.id} output"
    class="w-full"
  >
    <SegmentedControl.Control class="w-full">
      <SegmentedControl.Indicator />
      <!-- Switching off is never blocked, whatever state the supply is in, so
           only the "on" segment carries a disabled condition. -->
      <SegmentedControl.Item value="off" class="grow">
        <SegmentedControl.ItemText>Off</SegmentedControl.ItemText>
        <SegmentedControl.ItemHiddenInput />
      </SegmentedControl.Item>
      <SegmentedControl.Item value="on" disabled={!energisable} class="grow">
        <SegmentedControl.ItemText>On</SegmentedControl.ItemText>
        <SegmentedControl.ItemHiddenInput />
      </SegmentedControl.Item>
    </SegmentedControl.Control>
  </SegmentedControl>

  {#if error}
    <p class="text-sm preset-tonal-error p-2 rounded" role="alert" data-testid="channel-error">
      {error}
    </p>
  {/if}
</article>
