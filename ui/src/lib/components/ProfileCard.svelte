<script lang="ts">
  import type { DeviceProfile, DeviceView } from '../types';
  import {
    ApiFailure,
    applyProfile,
    powerCycleProfile,
    powerOffProfile,
    powerOnProfile,
  } from '../api';
  import {
    isStaged,
    profileState,
    railsOwnedBy,
    stateDetail,
    stateLabel,
  } from '../profile-state';
  import { trim } from '../format';
  import ActionIcon from './ActionIcon.svelte';

  let {
    profile,
    devices,
    onchange,
    onedit,
    ondelete,
  }: {
    profile: DeviceProfile;
    devices: DeviceView[];
    onchange?: () => void;
    onedit?: () => void;
    ondelete?: () => void;
  } = $props();

  let error = $state<string | null>(null);
  let forceable = $state(false);
  let busy = $state(false);

  const condition = $derived(profileState(profile, devices));
  const staged = $derived(isStaged(profile, devices));
  const owner = $derived(railsOwnedBy(profile, devices));
  const detail = $derived(stateDetail(condition, owner));
  // Cycling de-energises something that is currently live, so it asks first.
  let confirmingCycle = $state(false);

  const TONE: Record<string, string> = {
    powered: 'preset-tonal-success',
    'partly-powered': 'preset-tonal-warning',
    staged: 'preset-tonal',
    matching: 'preset-tonal-surface',
    'live-altered': 'preset-tonal-warning',
    idle: 'preset-tonal-surface',
    'not-visible': 'preset-tonal-warning',
  };

  async function run(action: () => Promise<unknown>) {
    error = null;
    forceable = false;
    busy = true;
    try {
      await action();
      onchange?.();
    } catch (e) {
      if (e instanceof ApiFailure) {
        error = e.detail.message;
        forceable = e.detail.forceable === true;
      } else {
        error = String(e);
      }
    } finally {
      busy = false;
    }
  }
</script>

<article
  class="card preset-outlined-surface-200-800 p-4 space-y-3"
  data-testid="profile-{profile.id}"
  data-profile-state={condition}
>
  <header class="flex flex-wrap items-start justify-between gap-x-4 gap-y-1">
    <div>
      <h3 class="h4">{profile.name}</h3>
      {#if profile.description}
        <p class="text-sm opacity-70">{profile.description}</p>
      {/if}
    </div>
    <span class="badge {TONE[condition]} text-sm px-3 py-1" data-testid="profile-status">
      {stateLabel(condition)}
    </span>
  </header>

  {#if detail}
    <p class="text-xs preset-tonal-warning p-2 rounded" data-testid="profile-detail">
      {detail}
    </p>
  {/if}



  <!--
    A power-up sequence, drawn as one. Steps are numbered because this genuinely
    is ordered in time, and the dwell sits *between* the steps it separates —
    attached to the rail above it, it read as "this rail waits", which is the
    opposite of what happens.
  -->
  <ol class="space-y-0" data-testid="profile-sequence">
    {#each profile.bindings as binding, index (binding.id)}
      <li class="flex items-baseline gap-3 py-1">
        <span class="reading text-xs opacity-50 w-4 tabular-nums">{index + 1}</span>
        <span class="w-28 shrink-0">{binding.slot} · {binding.channel}</span>
        <span class="reading tabular-nums">
          {trim(binding.preset.volt, 'V')}
          <span class="opacity-40">/</span>
          {trim(binding.preset.curr_limit, 'A')}
        </span>
        <span class="text-xs opacity-60">{binding.preset.name}</span>
        {#if !binding.output_on}
          <span class="text-xs opacity-60">staged only</span>
        {/if}
      </li>
      {#if binding.delay_ms > 0 && index < profile.bindings.length - 1}
        <li class="flex items-center gap-3 pl-4 text-xs opacity-50" aria-hidden="true">
          <span class="border-l border-surface-300-700 h-4 ml-1.5"></span>
          <span>then wait {binding.delay_ms} ms</span>
        </li>
      {/if}
    {/each}
  </ol>

  <div class="flex flex-wrap items-center gap-2">
    <button
      class="btn btn-sm preset-tonal"
      type="button"
      disabled={busy}
      onclick={() => run(() => applyProfile(profile.id))}
    >
      Apply setpoints
    </button>
    <button
      class="btn btn-sm preset-filled-success-500"
      type="button"
      disabled={busy || !staged}
      onclick={() => run(() => powerOnProfile(profile.id))}
    >
      Power on
    </button>
    <button
      class="btn btn-sm preset-tonal-error"
      type="button"
      disabled={busy}
      onclick={() => run(() => powerOffProfile(profile.id))}
    >
      Power off
    </button>
    <button
      class="btn btn-sm preset-tonal"
      type="button"
      disabled={busy || condition !== 'powered'}
      title={condition === 'powered'
        ? 'Switch these rails off, let them settle, and bring them back up in order'
        : 'Only available while this profile has its rails powered'}
      onclick={() => (confirmingCycle = true)}
      data-testid="power-cycle"
    >
      Power cycle
    </button>

    {#if !staged}
      <!-- A disabled control with no visible reason reads as a broken one. -->
      <span class="text-xs opacity-60">Apply the setpoints before powering on</span>
    {/if}

    <span class="grow"></span>
    <!-- Inside the card, because they act on it. -->
    <ActionIcon
      action="edit"
      label="Edit {profile.name}"
      testid="edit-profile-{profile.id}"
      onclick={() => onedit?.()}
    />
    <ActionIcon
      action="delete"
      label="Delete {profile.name}"
      testid="delete-profile-{profile.id}"
      onclick={() => ondelete?.()}
    />
  </div>

  {#if confirmingCycle}
    <div
      class="preset-tonal-warning p-3 rounded space-y-2"
      role="alertdialog"
      aria-label="Confirm power cycle"
      data-testid="confirm-cycle"
    >
      <p class="text-sm">
        Switch off <strong>{profile.name}</strong>, wait, then bring its rails back
        up in order? Whatever is connected loses power for about half a second.
      </p>
      <div class="flex gap-2">
        <button
          class="btn btn-sm preset-filled-warning-500"
          type="button"
          onclick={() => {
            confirmingCycle = false;
            void run(() => powerCycleProfile(profile.id));
          }}
          data-testid="confirm-cycle-yes"
        >
          Power cycle
        </button>
        <button
          class="btn btn-sm preset-tonal"
          type="button"
          onclick={() => (confirmingCycle = false)}
        >
          Leave it up
        </button>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="preset-tonal-error p-3 rounded space-y-2" role="alert" data-testid="profile-error">
      <p class="text-sm">{error}</p>
      {#if forceable}
        <button
          class="btn btn-sm preset-tonal-warning"
          type="button"
          disabled={busy}
          onclick={() => run(() => applyProfile(profile.id, true))}
          data-testid="force-apply"
        >
          Apply anyway
        </button>
      {/if}
    </div>
  {/if}
</article>
