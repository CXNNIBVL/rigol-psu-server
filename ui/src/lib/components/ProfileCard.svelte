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
  import { isCurrentLimited, reading, trim, UNKNOWN } from '../format';
  import ActionIcon from './ActionIcon.svelte';
  import PowerButton from './PowerButton.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';

  let {
    profile,
    devices,
    compact = false,
    onchange,
    onedit,
    ondelete,
    onexpand,
    oncollapse,
  }: {
    profile: DeviceProfile;
    devices: DeviceView[];
    /** Collapsed to a single row: an idle profile is mostly dashes expanded. */
    compact?: boolean;
    onchange?: () => void;
    onedit?: () => void;
    ondelete?: () => void;
    onexpand?: () => void;
    /** Only passed where collapsing is possible, so a live card never offers it. */
    oncollapse?: () => void;
  } = $props();

  let error = $state<string | null>(null);
  let forceable = $state(false);
  let busy = $state(false);

  const condition = $derived(profileState(profile, devices));
  const staged = $derived(isStaged(profile, devices));
  const owner = $derived(railsOwnedBy(profile, devices));

  // What each rail is actually doing, beside what it is meant to do. Without
  // this the card describes an intention and you have to leave for the bench to
  // find out whether the board is drawing what you expect — which is the
  // difference between a configuration screen and one you can work from.
  const rails = $derived(
    profile.bindings.map((binding) => ({
      binding,
      channel: devices
        .find((d) => d.slot === binding.slot)
        ?.channels.find((c) => c.id === binding.channel),
    })),
  );
  const detail = $derived(stateDetail(condition, owner));
  // Cycling de-energises something that is currently live, so it asks first.
  let confirmingCycle = $state(false);

  // Power is one concept with two directions, the same as it is on a channel
  // row — so it is one control here too, with the same symbol. Two buttons made
  // the card's busiest row busier and implied they were separate acts.
  const anyOn = $derived(
    profile.bindings.some(
      (b) =>
        b.output_on &&
        devices
          .find((d) => d.slot === b.slot)
          ?.channels.find((c) => c.id === b.channel)?.output_on === true,
    ),
  );
  // Nothing on to switch off and nothing staged to switch on: the control would
  // have nothing to do either way.
  const cannotToggle = $derived(!anyOn && !staged);

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
  class="card preset-outlined-surface-200-800 {compact ? 'p-3 space-y-2' : 'p-4 space-y-3'}"
  data-testid="profile-{profile.id}"
  data-profile-state={condition}
>
  <header class="flex flex-wrap items-start justify-between gap-x-4 gap-y-1">
    <div>
      <h3 class={compact ? 'text-base font-semibold' : 'h4'}>{profile.name}</h3>
      {#if profile.description}
        <p class="text-sm opacity-70">{profile.description}</p>
      {/if}
    </div>
    <span
      class="badge {TONE[condition]} {compact ? 'text-xs px-2 py-0.5' : 'text-sm px-3 py-1'}"
      data-testid="profile-status"
    >
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
  {#if compact}
    <!-- One line: an idle profile expanded is a table of dashes. -->
    <button
      class="flex w-full items-center gap-3 text-left text-sm opacity-80 hover:opacity-100"
      type="button"
      onclick={() => onexpand?.()}
      data-testid="expand-profile"
    >
      <span class="opacity-60">
        {profile.bindings.map((b) => `${b.slot}·${b.channel}`).join(', ')}
      </span>
      <span class="grow"></span>
      <span class="text-xs opacity-50">Show rails</span>
    </button>
  {:else}
  <ol class="space-y-0 text-sm overflow-x-auto" data-testid="profile-sequence">
    <!--
      A grid, not a flex row with a spacer. The spacer pushed the measured
      values to the far edge, so the two numbers this row exists to let you
      compare — specified and actual — sat as far apart as the layout could put
      them.
    -->
    <li class="grid grid-cols-[1.25rem_7rem_minmax(6rem,11rem)_7.5rem_5.5rem_5.5rem_2.5rem] items-baseline gap-x-3 text-xs opacity-50">
      <span></span>
      <span>Rail</span>
      <span>Settings</span>
      <span>Specified</span>
      <span class="text-right">Voltage</span>
      <span class="text-right">Current</span>
      <span></span>
    </li>
    {#each rails as { binding, channel }, index (binding.id)}
      <li
        class="grid grid-cols-[1.25rem_7rem_minmax(6rem,11rem)_7.5rem_5.5rem_5.5rem_2.5rem] items-baseline gap-x-3 py-1"
        data-testid="rail-{binding.id}"
      >
        <span class="reading text-xs opacity-50 tabular-nums">{index + 1}</span>
        <span class="whitespace-nowrap">{binding.slot} · {binding.channel}</span>
        <span class="text-xs opacity-60 truncate">
          {binding.preset.name}
          {#if !binding.output_on}— staged only{/if}
        </span>
        <span class="reading tabular-nums">
          {trim(binding.preset.volt, 'V')}
          <span class="opacity-40">/</span>
          {trim(binding.preset.curr_limit, 'A')}
        </span>
        <span class="reading text-right tabular-nums" data-testid="rail-volt">
          {channel ? reading(channel, 'volt') : UNKNOWN}
        </span>
        <span class="reading text-right tabular-nums" data-testid="rail-curr">
          {channel ? reading(channel, 'curr') : UNKNOWN}
        </span>
        <span>
          {#if channel && isCurrentLimited(channel)}
            <span
              class="badge preset-tonal-warning text-xs"
              title="At its current limit — the voltage is falling because the load wants more than this rail allows."
              data-testid="rail-cc"
            >
              CC
            </span>
          {/if}
        </span>
      </li>
      {#if binding.delay_ms > 0 && index < rails.length - 1}
        <li class="flex items-center gap-3 pl-4 text-xs opacity-50" aria-hidden="true">
          <span class="border-l border-surface-300-700 h-4 ml-1.5"></span>
          <span>then wait {binding.delay_ms} ms</span>
        </li>
      {/if}
    {/each}
  </ol>
  {#if oncollapse}
    <!-- Expanding reflows the grid around this card, so it has to be undoable.
         Same words and same place as the control that opened it. -->
    <div class="flex justify-end">
      <button
        class="text-xs opacity-50 hover:opacity-100"
        type="button"
        onclick={() => oncollapse?.()}
        data-testid="collapse-profile"
      >
        Hide rails
      </button>
    </div>
  {/if}
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <button
      class="btn btn-sm preset-tonal"
      type="button"
      disabled={busy}
      onclick={() => run(() => applyProfile(profile.id))}
    >
      Apply setpoints
    </button>

    <!--
      Collapsed in a side column, a control that cannot act is just width. The
      power button reappears the moment the setpoints are applied, which is the
      only thing that makes it usable — so nothing is hidden that you could have
      pressed.
    -->
    {#if !compact || !cannotToggle}
      <PowerButton
        on={anyOn}
        disabled={cannotToggle}
        {busy}
        label="{profile.name} power"
        onToggle={(next) =>
          run(() => (next ? powerOnProfile(profile.id) : powerOffProfile(profile.id)))}
      />
    {/if}

    {#if !compact || condition === 'powered'}
      <ActionIcon
        action="cycle"
        label="Power cycle {profile.name}"
        testid="power-cycle"
        disabled={busy || condition !== 'powered'}
        title={condition === 'powered'
          ? 'Switch these rails off, let them settle, and bring them back up in order'
          : 'Only available while this profile has its rails powered'}
        onclick={() => (confirmingCycle = true)}
      />
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

  {#if cannotToggle && !compact}
    <!-- On its own line: inside the action row it made this card's controls a
         different width from every other card's, so the column of power buttons
         did not line up. Shown only when the control is genuinely blocked —
         keyed on `!staged` it also appeared for rails that are on at the wrong
         values, where switching off is exactly what you want and is available. -->
    <p class="text-xs opacity-60">Apply the setpoints before powering on</p>
  {/if}

  <ConfirmDialog
    open={confirmingCycle}
    title="Power cycle {profile.name}?"
    confirmLabel="Power cycle"
    cancelLabel="Leave it up"
    tone="warning"
    testid="confirm-cycle"
    onconfirm={() => {
      confirmingCycle = false;
      void run(() => powerCycleProfile(profile.id));
    }}
    oncancel={() => (confirmingCycle = false)}
  >
    Its rails go off, settle, and come back up in order. Whatever is connected
    loses power for about half a second.
  </ConfirmDialog>

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
