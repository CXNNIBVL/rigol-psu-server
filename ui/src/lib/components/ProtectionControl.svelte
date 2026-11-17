<script lang="ts">
  import type { ChannelView, DeviceView, Protection } from '../types';
  import { trim } from '../format';
  import { ApiFailure, setProtection } from '../api';
  import ActionIcon from './ActionIcon.svelte';

  let {
    device,
    channel,
    writable,
  }: { device: DeviceView; channel: ChannelView; writable: boolean } = $props();

  // Same pattern as the setpoint: a field is only overridden once it is
  // touched, so the poll does not overwrite what is being typed.
  let editing = $state(false);
  let draft = $state<Partial<Protection>>({});
  let error = $state<string | null>(null);
  let busy = $state(false);

  const shown = $derived<Protection | null>(
    channel.protection ? { ...channel.protection, ...draft } : null,
  );

  function revert() {
    editing = false;
    draft = {};
    error = null;
  }

  async function apply() {
    if (!shown || Object.keys(draft).length === 0) {
      revert();
      return;
    }
    error = null;
    busy = true;
    try {
      await setProtection(device.slot, channel.id, shown);
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
      void apply();
    } else if (event.key === 'Escape') {
      revert();
    }
  }
</script>

{#if shown}
  {#if editing}
    <div class="flex items-center justify-end gap-1 mt-1" data-testid="protection-edit">
      <label class="flex items-center gap-1 text-xs">
        <input
          class="checkbox"
          type="checkbox"
          aria-label="{channel.id} OVP enabled"
          checked={shown.ovp_enabled}
          onchange={(e) => (draft.ovp_enabled = e.currentTarget.checked)}
          disabled={busy}
        />
        OVP
      </label>
      <input
        class="input w-24 text-right"
        type="number"
        step="0.001"
        min="0"
        aria-label="{channel.id} OVP level"
        value={shown.ovp_level.toFixed(3)}
        oninput={(e) => (draft.ovp_level = Number(e.currentTarget.value))}
        onkeydown={onKey}
        disabled={busy}
      />
      <span class="text-xs opacity-60">V</span>
      <label class="flex items-center gap-1 text-xs ml-2">
        <input
          class="checkbox"
          type="checkbox"
          aria-label="{channel.id} OCP enabled"
          checked={shown.ocp_enabled}
          onchange={(e) => (draft.ocp_enabled = e.currentTarget.checked)}
          disabled={busy}
        />
        OCP
      </label>
      <input
        class="input w-24 text-right"
        type="number"
        step="0.001"
        min="0"
        aria-label="{channel.id} OCP level"
        value={shown.ocp_level.toFixed(3)}
        oninput={(e) => (draft.ocp_level = Number(e.currentTarget.value))}
        onkeydown={onKey}
        disabled={busy}
      />
      <span class="text-xs opacity-60">A</span>
      <ActionIcon
        action="confirm"
        label="Apply {channel.id} protection"
        onclick={apply}
        disabled={busy}
      />
      <ActionIcon action="dismiss" label="Discard {channel.id} protection edit" onclick={revert} />
    </div>
  {:else}
    <div class="flex items-center justify-end gap-2 text-xs" data-testid="protection">
      <span class="reading tabular-nums">
        <!-- A disabled trip point has no level worth reading, only the fact
             that nothing will trip. -->
        <span class={shown.ovp_enabled ? '' : 'opacity-50'}>
          OVP {shown.ovp_enabled ? trim(shown.ovp_level, 'V') : 'off'}
        </span>
        <span class="opacity-40">·</span>
        <span class={shown.ocp_enabled ? '' : 'opacity-50'}>
          OCP {shown.ocp_enabled ? trim(shown.ocp_level, 'A') : 'off'}
        </span>
      </span>
      {#if writable}
        <ActionIcon
          action="edit"
          label="Edit {channel.id} protection"
          onclick={() => {
            editing = true;
            error = null;
          }}
        />
      {/if}
    </div>
  {/if}
  {#if error}
    <p class="text-sm preset-tonal-error p-2 rounded mt-1 text-left" role="alert" data-testid="protection-error">
      {error}
    </p>
  {/if}
{/if}
