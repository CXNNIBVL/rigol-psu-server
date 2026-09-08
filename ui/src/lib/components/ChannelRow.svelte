<script lang="ts">
  import { SegmentedControl } from '@skeletonlabs/skeleton-svelte';
  import type { ChannelView, DeviceView } from '../types';
  import {
    canEnergise,
    channelBadge,
    channelPresentation,
    isWritable,
    reading,
    trim,
    UNKNOWN,
  } from '../format';
  import { ApiFailure, setOutput, setSetpoint } from '../api';
  import StatusBadge from './StatusBadge.svelte';

  let { device, channel }: { device: DeviceView; channel: ChannelView } = $props();

  const presentation = $derived(channelPresentation(channel));
  const badge = $derived(channelBadge(channel));

  // "The supply is not answering" is a fact about the supply, and the panel
  // already says it once. Repeating it under all three channels is noise that
  // buries the case where a single channel has something wrong with it.
  const detail = $derived(presentation === 'violation' ? badge?.detail : undefined);
  const writable = $derived(isWritable(device, channel));
  const energisable = $derived(canEnergise(device, channel));

  const outputValue = $derived(
    channel.output_on === null ? '' : channel.output_on ? 'on' : 'off',
  );

  // The setpoint fields start from what the instrument reports and are only
  // "dirty" once someone actually changes them — so a value arriving from the
  // 0.5 Hz setpoint poll does not fight with what is being typed.
  let volts = $state<string | null>(null);
  let amps = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const shownVolts = $derived(volts ?? channel.setpoint?.volt.toFixed(3) ?? '');
  const shownAmps = $derived(amps ?? channel.setpoint?.curr_limit.toFixed(3) ?? '');
  const dirty = $derived(volts !== null || amps !== null);

  function revert() {
    volts = null;
    amps = null;
    error = null;
  }

  async function applySetpoint() {
    if (!dirty) return;
    error = null;
    busy = true;
    try {
      // Either field alone is a valid edit: whichever was not touched keeps the
      // value the instrument already has.
      await setSetpoint(device.slot, channel.id, {
        volt: Number(shownVolts),
        curr_limit: Number(shownAmps),
      });
      revert();
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void applySetpoint();
    } else if (event.key === 'Escape') {
      revert();
    }
  }

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

  // A live row is the one worth looking at; an idle one recedes without
  // disappearing. Whole literal strings — Tailwind cannot see an assembled one.
  const ROW: Record<typeof presentation, string> = {
    live: 'bg-success-500/5',
    off: 'opacity-70',
    unknown: '',
    violation: '',
  };
</script>

<!--
  One `tbody` per channel. A table may hold several, and it gives the channel a
  single element to be identified by — the detail and error rows are siblings of
  the reading row, so hanging the identity on the reading row alone left them
  outside it.
-->
<tbody
  class="{ROW[presentation]}"
  data-testid="channel-{device.slot}-{channel.id}"
  data-state={presentation}
>
<tr class="border-t border-surface-200-800">
  <th scope="row" class="py-2 pr-3 text-left font-semibold whitespace-nowrap">
    <span class="inline-flex items-center gap-2">
      <!-- A live rail is marked on the row itself, so the accent survives at a
           glance down a column of channels. -->
      <span
        class="inline-block size-2 rounded-full {presentation === 'live'
          ? 'bg-success-500'
          : 'bg-surface-400-600/40'}"
        aria-hidden="true"
      ></span>
      {channel.id}
    </span>
  </th>

  <td class="reading py-2 px-3 text-right tabular-nums" data-testid="volt">
    {reading(channel, 'volt')}
  </td>
  <td class="reading py-2 px-3 text-right tabular-nums" data-testid="curr">
    {reading(channel, 'curr')}
  </td>
  <td class="reading py-2 px-3 text-right tabular-nums" data-testid="power">
    {reading(channel, 'power')}
  </td>

  <td class="py-2 px-3">
    <div class="flex items-center gap-1">
      <input
        class="input w-24 text-right"
        type="number"
        step="0.001"
        min="0"
        aria-label="{channel.id} voltage"
        value={shownVolts}
        oninput={(e) => (volts = e.currentTarget.value)}
        onkeydown={onKey}
        disabled={!writable || busy}
      />
      <span class="text-xs opacity-60">V</span>
      <input
        class="input w-24 text-right"
        type="number"
        step="0.001"
        min="0"
        aria-label="{channel.id} current limit"
        value={shownAmps}
        oninput={(e) => (amps = e.currentTarget.value)}
        onkeydown={onKey}
        disabled={!writable || busy}
      />
      <span class="text-xs opacity-60">A</span>
      {#if dirty}
        <!-- Appears only once something has changed, so the row is not carrying
             a permanently dead button. Enter does the same thing. -->
        <button
          class="btn btn-sm preset-filled"
          type="button"
          onclick={applySetpoint}
          disabled={busy}
        >
          Apply
        </button>
        <button class="btn btn-sm preset-tonal" type="button" onclick={revert}>
          Cancel
        </button>
      {/if}
    </div>
  </td>

  <td class="py-2 px-3 text-right text-xs opacity-60 whitespace-nowrap">
    {trim(channel.limits.max_volt, 'V')} / {trim(channel.limits.max_curr, 'A')}
  </td>

  <td class="py-2 pl-3">
    <div class="flex items-center justify-end gap-2">
      {#if badge}
        <StatusBadge status={badge} />
      {/if}
      <SegmentedControl
        value={outputValue}
        onValueChange={(details) => toggle(details.value === 'on')}
        disabled={busy || device.state === 'offline'}
        aria-label="{channel.id} output"
        data-output={outputValue || undefined}
      >
        <SegmentedControl.Control>
          <SegmentedControl.Indicator />
          <!-- Switching off is never blocked, whatever state the supply is in. -->
          <SegmentedControl.Item value="off">
            <SegmentedControl.ItemText>Off</SegmentedControl.ItemText>
            <SegmentedControl.ItemHiddenInput />
          </SegmentedControl.Item>
          <SegmentedControl.Item value="on" disabled={!energisable}>
            <SegmentedControl.ItemText>On</SegmentedControl.ItemText>
            <SegmentedControl.ItemHiddenInput />
          </SegmentedControl.Item>
        </SegmentedControl.Control>
      </SegmentedControl>
    </div>
  </td>
</tr>

{#if detail || error}
  <tr>
    <td colspan="7" class="pb-2 px-3">
      {#if detail}
        <p class="text-xs preset-tonal-error p-2 rounded" data-testid="channel-detail">
          {detail}
        </p>
      {/if}
      {#if error}
        <p
          class="text-sm preset-tonal-error p-2 rounded mt-1"
          role="alert"
          data-testid="channel-error"
        >
          {error}
        </p>
      {/if}
    </td>
  </tr>
{/if}
</tbody>
