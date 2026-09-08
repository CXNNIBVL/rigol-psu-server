<script lang="ts">
  import type { DeviceProfile } from '../types';
  import {
    ApiFailure,
    applyProfile,
    powerOffProfile,
    powerOnProfile,
  } from '../api';

  let {
    profile,
    onchange,
  }: { profile: DeviceProfile; onchange?: () => void } = $props();

  let error = $state<string | null>(null);
  let forceable = $state(false);
  let staged = $state(false);
  let busy = $state(false);

  async function run(action: () => Promise<unknown>, stages: boolean) {
    error = null;
    forceable = false;
    busy = true;
    try {
      await action();
      if (stages) staged = true;
      onchange?.();
    } catch (e) {
      if (e instanceof ApiFailure) {
        error = e.detail.message;
        // Only offered when the server says this particular refusal is one
        // force can override. It never covers a clamp or a quarantine.
        forceable = e.detail.forceable === true;
      } else {
        error = String(e);
      }
    } finally {
      busy = false;
    }
  }

  const stage = (force = false) => run(() => applyProfile(profile.id, force), true);
  const powerOn = () => run(() => powerOnProfile(profile.id), false);
  const powerOff = () => run(() => powerOffProfile(profile.id), false);

  const rails = $derived(
    profile.bindings.map((b) => ({
      ...b,
      label: `${b.slot}/${b.channel}`,
    })),
  );
</script>

<article class="card preset-outlined-surface-200-800 p-4 space-y-3" data-testid="profile-{profile.id}">
  <header class="space-y-1">
    <h3 class="h4">{profile.name}</h3>
    {#if profile.description}
      <p class="text-sm opacity-70">{profile.description}</p>
    {/if}
  </header>

  <ol class="text-sm space-y-1">
    {#each rails as rail (rail.id)}
      <li class="flex justify-between gap-3">
        <span class="reading">
          {rail.sequence_order}. {rail.label}
        </span>
        <span class="opacity-70">
          {rail.preset.name} &middot; {rail.preset.volt.toFixed(3)} V /
          {rail.preset.curr_limit.toFixed(3)} A
          {#if rail.delay_ms > 0}&middot; wait {rail.delay_ms} ms{/if}
          {#if !rail.output_on}&middot; staged only{/if}
        </span>
      </li>
    {/each}
  </ol>

  <hr class="hr" />

  <div class="flex flex-wrap gap-2">
    <!--
      Three separate buttons, not one. Applying stages setpoints and cannot
      energise anything; powering on is the only action that puts energy into a
      device, so it stays a deliberate second press rather than a consequence of
      loading a profile.
    -->
    <button
      class="btn preset-tonal"
      type="button"
      disabled={busy}
      onclick={() => stage()}
    >
      Apply setpoints
    </button>
    <button
      class="btn preset-filled-primary-500"
      type="button"
      disabled={busy || !staged}
      title={staged ? 'Energise the rails in sequence' : 'Apply the setpoints first'}
      onclick={powerOn}
    >
      Power on
    </button>
    <button
      class="btn preset-tonal-error"
      type="button"
      disabled={busy}
      onclick={powerOff}
    >
      Power off
    </button>
  </div>

  {#if error}
    <div class="preset-tonal-error p-3 rounded space-y-2" role="alert" data-testid="profile-error">
      <p class="text-sm">{error}</p>
      {#if forceable}
        <button
          class="btn btn-sm preset-filled-warning-500"
          type="button"
          disabled={busy}
          onclick={() => stage(true)}
          data-testid="force-apply"
        >
          Apply anyway
        </button>
      {/if}
    </div>
  {/if}
</article>
