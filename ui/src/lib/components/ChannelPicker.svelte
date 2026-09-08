<script lang="ts">
  /**
   * Picking a rail is a two-dimensional choice — which supply, which channel —
   * and a flat dropdown threw that away. Nine rows each repeated the supply name
   * and the channel's limits, and the list grew a row per channel for every
   * supply added.
   *
   * Laid out as the rack is instead: a row per supply, its three channels as
   * chips. The supply name appears once, the limits appear once for whatever is
   * selected, and six supplies are six short rows rather than eighteen lines of
   * scrolling.
   */
  import type { DeviceView } from '../types';
  import { trim } from '../format';

  let {
    devices,
    slot,
    channel,
    /** `slot/channel` keys already bound by other rails of this profile. */
    taken = new Set<string>(),
    label,
    onSelect,
  }: {
    devices: DeviceView[];
    slot: string;
    channel: string;
    taken?: Set<string>;
    label: string;
    onSelect: (slot: string, channel: string) => void;
  } = $props();

  const selected = $derived(
    devices
      .find((d) => d.slot === slot)
      ?.channels.find((c) => c.id === channel),
  );
</script>

<div role="radiogroup" aria-label={label} class="space-y-1">
  {#each devices as device (device.slot)}
    <div class="flex items-center gap-3">
      <span
        class="w-44 shrink-0 truncate text-sm {device.state === 'offline'
          ? 'opacity-50'
          : 'opacity-80'}"
        title={device.label}
      >
        {device.label}
      </span>
      <div class="flex gap-1">
        {#each device.channels as option (option.id)}
          {@const isSelected = device.slot === slot && option.id === channel}
          {@const isTaken = taken.has(`${device.slot}/${option.id}`) && !isSelected}
          <button
            type="button"
            role="radio"
            aria-checked={isSelected}
            aria-label="{device.label} {option.id}"
            class="btn btn-sm {isSelected
              ? 'preset-filled-success-500'
              : 'preset-outlined-surface-200-800'} {isTaken ? 'opacity-40' : ''}"
            disabled={isTaken}
            title={isTaken
              ? 'Already used by another rail in this profile'
              : `max ${trim(option.limits.max_volt, 'V')} / ${trim(option.limits.max_curr, 'A')}`}
            onclick={() => onSelect(device.slot, option.id)}
          >
            {option.id}
          </button>
        {/each}
      </div>
      {#if device.state === 'offline'}
        <!-- Selectable anyway: a profile can name a supply that happens to be
             unplugged right now, and applying it is what refuses. -->
        <span class="text-xs opacity-50">not answering</span>
      {/if}
    </div>
  {/each}

  {#if selected}
    <p class="text-xs opacity-60 pt-1">
      Limit {trim(selected.limits.max_volt, 'V')} / {trim(selected.limits.max_curr, 'A')}
    </p>
  {/if}
</div>
