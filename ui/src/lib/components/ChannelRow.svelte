<script lang="ts">
  import type { ChannelView, DeviceView } from '../types';
  import {
    canEnergise,
    channelBadge,
    channelPresentation,
    isCurrentLimited,
    isWritable,
    reading,
    trim,
    UNKNOWN,
  } from '../format';
  import { ApiFailure, setOutput, setSetpoint } from '../api';
  import StatusBadge from './StatusBadge.svelte';
  import ActionIcon from './ActionIcon.svelte';
  import PowerButton from './PowerButton.svelte';
  import ProtectionControl from './ProtectionControl.svelte';

  let { device, channel }: { device: DeviceView; channel: ChannelView } = $props();

  const presentation = $derived(channelPresentation(channel));
  const badge = $derived(channelBadge(channel));

  // "The supply is not answering" is a fact about the supply, and the panel
  // already says it once. Repeating it under all three channels is noise that
  // buries the case where a single channel has something wrong with it.
  const detail = $derived(presentation === 'violation' ? badge?.detail : undefined);
  const writable = $derived(isWritable(device, channel));
  const energisable = $derived(canEnergise(device, channel));

  // Switching off is never blocked, whatever state the supply is in, so the
  // button is only unavailable in the direction that would put energy into
  // something.
  const cannotToggle = $derived(channel.output_on !== true && !energisable);

  // The setpoint fields start from what the instrument reports and are only
  // "dirty" once someone actually changes them — so a value arriving from the
  // 0.5 Hz setpoint poll does not fight with what is being typed.
  let editing = $state(false);
  let volts = $state<string | null>(null);
  let amps = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const shownVolts = $derived(volts ?? channel.setpoint?.volt.toFixed(3) ?? '');
  const shownAmps = $derived(amps ?? channel.setpoint?.curr_limit.toFixed(3) ?? '');
  const dirty = $derived(volts !== null || amps !== null);

  function open() {
    editing = true;
    error = null;
  }

  function revert() {
    editing = false;
    volts = null;
    amps = null;
    error = null;
  }

  async function applySetpoint() {
    if (!dirty) {
      revert();
      return;
    }
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

  // The response says the command was accepted; the snapshot says it happened,
  // and those are up to half a second apart. In between, the rail's real state
  // is still the old one — so the button goes on showing it and reports that an
  // answer is outstanding, rather than snapping back and flipping later.
  let requested = $state<boolean | null>(null);
  let settle: ReturnType<typeof setTimeout> | null = null;

  const pending = $derived(requested !== null && channel.output_on !== requested);

  $effect(() => {
    if (requested !== null && channel.output_on === requested) requested = null;
  });

  $effect(() => () => {
    if (settle) clearTimeout(settle);
  });

  async function toggle(on: boolean) {
    error = null;
    busy = true;
    try {
      await setOutput(device.slot, channel.id, on);
      requested = on;
      // If the supply never gets there — quarantined, or the write did not
      // take — stop claiming to wait and go back to reporting what is true.
      if (settle) clearTimeout(settle);
      settle = setTimeout(() => (requested = null), 5000);
    } catch (e) {
      requested = null;
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
    <span class="flex items-center gap-3">
      {#if device.state === 'offline'}
        <!-- Nothing can be done with an unreachable supply, and the badge
             already says why. The space is held so the column does not jump. -->
        <span class="size-9" aria-hidden="true"></span>
      {:else}
        <PowerButton
          on={channel.output_on}
          disabled={cannotToggle}
          {busy}
          {pending}
          label="{channel.id} output"
          onToggle={toggle}
        />
      {/if}
      {channel.id}
      {#if badge}
        <StatusBadge status={badge} />
      {/if}
      {#if channel.claimed_by}
        <!-- Who wrote these values. Verified against the instrument rather than
             remembered, so it disappears as soon as anyone changes them. -->
        <span
          class="text-xs font-normal opacity-60 truncate"
          title="Applied by {channel.claimed_by.profile_name}"
        >
          {channel.claimed_by.profile_name}
        </span>
      {/if}
    </span>
  </th>

  <td class="reading py-2 px-3 text-right tabular-nums" data-testid="volt">
    {reading(channel, 'volt')}
  </td>
  <td class="reading py-2 px-3 text-right tabular-nums" data-testid="curr">
    {reading(channel, 'curr')}
    {#if isCurrentLimited(channel)}
      <span
        class="badge preset-tonal-warning text-xs ml-1 align-middle"
        title="At its current limit — the supply is holding {channel.setpoint?.curr_limit.toFixed(3)} A and letting the voltage fall. The load wants more than this rail allows."
        data-testid="current-limited"
      >
        CC
      </span>
    {/if}
  </td>
  <td class="reading py-2 px-3 text-right tabular-nums" data-testid="power">
    {reading(channel, 'power')}
  </td>

  <td class="py-2 pl-3 text-right">
    {#if channel.setpoint === null}
      <span class="opacity-60">{UNKNOWN}</span>
    {:else if editing}
      <!-- Inputs exist only for the row being edited. Nine pairs of permanently
           open boxes out-shouted the readings, which are the point of the
           table. -->
      <div class="flex items-center justify-end gap-1">
        <input
          class="input w-24 text-right"
          type="number"
          step="0.001"
          min="0"
          max={channel.limits.max_volt}
          aria-label="{channel.id} voltage"
          value={shownVolts}
          oninput={(e) => (volts = e.currentTarget.value)}
          onkeydown={onKey}
          disabled={busy}
        />
        <span class="text-xs opacity-60">V</span>
        <input
          class="input w-24 text-right"
          type="number"
          step="0.001"
          min="0"
          max={channel.limits.max_curr}
          aria-label="{channel.id} current limit"
          value={shownAmps}
          oninput={(e) => (amps = e.currentTarget.value)}
          onkeydown={onKey}
          disabled={busy}
        />
        <span class="text-xs opacity-60">A</span>
        <ActionIcon
          action="confirm"
          label="Apply {channel.id} setpoint"
          onclick={applySetpoint}
          disabled={busy}
        />
        <ActionIcon action="dismiss" label="Discard {channel.id} edit" onclick={revert} />
      </div>
      <!--
        On a line of its own. Six controls and a sentence crammed into one row
        left nothing room to breathe, and the limit is a note about what you are
        typing rather than another control.
      -->
      <p class="text-xs opacity-60 mt-1">
        Enter applies. Limit {trim(channel.limits.max_volt, 'V')} / {trim(channel.limits.max_curr, 'A')}
      </p>
    {:else}
      <div class="flex items-center justify-end gap-2">
        <span class="reading tabular-nums">
          {channel.setpoint.volt.toFixed(3)} <span class="text-xs opacity-60">V</span>
          <span class="opacity-40">/</span>
          {channel.setpoint.curr_limit.toFixed(3)} <span class="text-xs opacity-60">A</span>
        </span>
        {#if writable}
          <ActionIcon
            action="edit"
            label="Edit {channel.id} setpoint"
            onclick={open}
          />
        {/if}
      </div>
      <ProtectionControl {device} {channel} {writable} />
    {/if}
  </td>

</tr>

{#if detail || error}
  <tr>
    <td colspan="5" class="pb-2 px-3">
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
