<script lang="ts">
  import {
    ApiFailure,
    createPreset,
    deletePreset,
    listPresets,
    updatePreset,
  } from '../api';
  import type { ChannelPreset } from '../types';

  let presets = $state<ChannelPreset[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  // null = the create form; a preset = editing that one.
  let editing = $state<ChannelPreset | null>(null);
  let form = $state(blank());
  let busy = $state(false);

  function blank() {
    return {
      name: '',
      volt: 0,
      curr_limit: 1,
      ovp_level: 0,
      ovp_enabled: false,
      ocp_level: 0,
      ocp_enabled: false,
    };
  }

  async function refresh() {
    try {
      presets = await listPresets();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loaded = true;
    }
  }

  $effect(() => {
    void refresh();
  });

  function edit(preset: ChannelPreset) {
    editing = preset;
    const { id: _id, ...rest } = preset;
    form = { ...rest };
  }

  function cancel() {
    editing = null;
    form = blank();
    error = null;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    busy = true;
    try {
      if (editing) {
        await updatePreset(editing.id, form);
      } else {
        await createPreset(form);
      }
      cancel();
      await refresh();
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function remove(preset: ChannelPreset) {
    error = null;
    busy = true;
    try {
      await deletePreset(preset.id);
      if (editing?.id === preset.id) cancel();
      await refresh();
    } catch (e) {
      // Most often "a profile depends on this", which is a refusal by design
      // rather than a failure, so it is shown as plain explanatory text.
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="space-y-4" data-testid="presets-panel">
  <h2 class="h3">Preset library</h2>
  <p class="text-sm opacity-70">
    A preset is one channel's settings. Profiles compose them; presets on their own
    are never applied to anything.
  </p>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert" data-testid="preset-error">
      {error}
    </p>
  {/if}

  <div class="grid gap-4 lg:grid-cols-[2fr_3fr]">
    <form class="card preset-outlined-surface-200-800 p-4 space-y-3" onsubmit={submit} novalidate>
      <h3 class="h4">{editing ? `Edit ${editing.name}` : 'New preset'}</h3>

      <label class="label">
        <span class="label-text">Name</span>
        <input class="input" type="text" bind:value={form.name} required aria-label="Preset name" />
      </label>

      <div class="grid grid-cols-2 gap-2">
        <label class="label">
          <span class="label-text">Voltage (V)</span>
          <input class="input" type="number" step="0.001" min="0" bind:value={form.volt} aria-label="Voltage" />
        </label>
        <label class="label">
          <span class="label-text">Current limit (A)</span>
          <input class="input" type="number" step="0.001" min="0" bind:value={form.curr_limit} aria-label="Current limit" />
        </label>
      </div>

      <div class="grid grid-cols-2 gap-2">
        <label class="label">
          <span class="label-text">OVP level (V)</span>
          <input class="input" type="number" step="0.001" min="0" bind:value={form.ovp_level} aria-label="OVP level" />
        </label>
        <label class="label">
          <span class="label-text">OCP level (A)</span>
          <input class="input" type="number" step="0.001" min="0" bind:value={form.ocp_level} aria-label="OCP level" />
        </label>
      </div>

      <div class="flex flex-wrap gap-4 text-sm">
        <label class="flex items-center gap-2">
          <input class="checkbox" type="checkbox" bind:checked={form.ovp_enabled} />
          <span>Enable OVP</span>
        </label>
        <label class="flex items-center gap-2">
          <input class="checkbox" type="checkbox" bind:checked={form.ocp_enabled} />
          <span>Enable OCP</span>
        </label>
      </div>

      <p class="text-xs opacity-60">
        Whether a rail comes up on power-on is decided per profile, not here — the
        same preset can auto-start on one device and stay staged on another.
      </p>

      <div class="flex gap-2">
        <button class="btn preset-filled-primary-500" type="submit" disabled={busy || !form.name}>
          {editing ? 'Save' : 'Create'}
        </button>
        {#if editing}
          <button class="btn preset-tonal" type="button" onclick={cancel} disabled={busy}>
            Cancel
          </button>
        {/if}
      </div>
    </form>

    <div class="space-y-2">
      {#if loaded && presets.length === 0}
        <p class="opacity-60 text-sm">No presets yet.</p>
      {/if}
      {#each presets as preset (preset.id)}
        <article
          class="card preset-outlined-surface-200-800 p-3 flex flex-wrap items-center justify-between gap-2"
          data-testid="preset-{preset.id}"
        >
          <div>
            <p class="font-semibold">{preset.name}</p>
            <p class="reading text-sm opacity-70">
              {preset.volt.toFixed(3)} V / {preset.curr_limit.toFixed(3)} A
              {#if preset.ovp_enabled}&middot; OVP {preset.ovp_level.toFixed(3)} V{/if}
              {#if preset.ocp_enabled}&middot; OCP {preset.ocp_level.toFixed(3)} A{/if}
            </p>
          </div>
          <div class="flex gap-2">
            <button class="btn btn-sm preset-tonal" type="button" onclick={() => edit(preset)} disabled={busy}>
              Edit
            </button>
            <button
              class="btn btn-sm preset-tonal-error"
              type="button"
              onclick={() => remove(preset)}
              disabled={busy}
            >
              Delete
            </button>
          </div>
        </article>
      {/each}
    </div>
  </div>
</section>
