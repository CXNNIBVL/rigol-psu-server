<script lang="ts">
  import {
    ApiFailure,
    createPreset,
    deletePreset,
    listPresets,
    listProfiles,
    updatePreset,
  } from '../api';
  import { trim } from '../format';
  import type { ChannelPreset, DeviceProfile } from '../types';

  let presets = $state<ChannelPreset[]>([]);
  let profiles = $state<DeviceProfile[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);
  let editing = $state<ChannelPreset | null>(null);
  let confirming = $state<ChannelPreset | null>(null);
  let busy = $state(false);
  let form = $state(blank());

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
      [presets, profiles] = await Promise.all([listPresets(), listProfiles()]);
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

  /**
   * Which profiles depend on each preset.
   *
   * Deleting a preset in use is refused by the server, and used to be
   * discoverable only by trying it. Showing the count turns a rejection into
   * information — and names the profiles, so you know where to look.
   */
  const usage = $derived(
    new Map(
      presets.map((preset) => [
        preset.id,
        profiles
          .filter((p) => p.bindings.some((b) => b.preset.id === preset.id))
          .map((p) => p.name),
      ]),
    ),
  );

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
      if (editing) await updatePreset(editing.id, form);
      else await createPreset(form);
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
      confirming = null;
      if (editing?.id === preset.id) cancel();
      await refresh();
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="space-y-4" data-testid="presets-panel">
  <div>
    <h2 class="h3">Rail settings</h2>
    <p class="text-sm opacity-70">
      Saved voltage and current settings for one channel. Profiles use them; on
      their own they are not applied to anything.
    </p>
  </div>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert" data-testid="preset-error">
      {error}
    </p>
  {/if}

  <!-- A table rather than cards: these are the smallest unit, least interesting
       alone and most useful compared down a column. -->
  <div class="card preset-outlined-surface-200-800 overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="text-xs opacity-60">
          <th scope="col" class="p-3 text-left font-normal">Name</th>
          <th scope="col" class="p-3 text-right font-normal">Voltage</th>
          <th scope="col" class="p-3 text-right font-normal">Current limit</th>
          <th scope="col" class="p-3 text-right font-normal">OVP</th>
          <th scope="col" class="p-3 text-right font-normal">OCP</th>
          <th scope="col" class="p-3 text-left font-normal">Used by</th>
          <th scope="col" class="p-3"><span class="sr-only">Actions</span></th>
        </tr>
      </thead>
      <tbody>
        {#each presets as preset (preset.id)}
          <tr class="border-t border-surface-200-800" data-testid="preset-{preset.id}">
            <th scope="row" class="p-3 text-left font-semibold">{preset.name}</th>
            <td class="reading p-3 text-right tabular-nums">{trim(preset.volt, 'V')}</td>
            <td class="reading p-3 text-right tabular-nums">{trim(preset.curr_limit, 'A')}</td>
            <td class="reading p-3 text-right tabular-nums opacity-70">
              {preset.ovp_enabled ? trim(preset.ovp_level, 'V') : 'off'}
            </td>
            <td class="reading p-3 text-right tabular-nums opacity-70">
              {preset.ocp_enabled ? trim(preset.ocp_level, 'A') : 'off'}
            </td>
            <td class="p-3 text-xs opacity-70">
              {(usage.get(preset.id) ?? []).join(', ') || 'nothing yet'}
            </td>
            <td class="p-3">
              <div class="flex justify-end gap-1">
                <button
                  class="btn btn-sm preset-tonal"
                  type="button"
                  onclick={() => edit(preset)}
                  disabled={busy}
                >
                  Edit
                </button>
                <!-- Quiet until sought; red belongs on the confirmation. -->
                <button
                  class="btn btn-sm hover:preset-tonal-error"
                  type="button"
                  onclick={() => (confirming = preset)}
                  disabled={busy || (usage.get(preset.id) ?? []).length > 0}
                  title={(usage.get(preset.id) ?? []).length > 0
                    ? 'Used by a profile'
                    : undefined}
                >
                  Delete
                </button>
              </div>
            </td>
          </tr>
        {/each}
        {#if loaded && presets.length === 0}
          <tr class="border-t border-surface-200-800">
            <td colspan="7" class="p-4 text-sm opacity-60">
              No saved settings yet. Add one below, or create them as you go from
              a profile's rails.
            </td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>

  {#if confirming}
    <div
      class="card preset-outlined-surface-200-800 p-4 space-y-3"
      role="alertdialog"
      aria-label="Confirm delete"
      data-testid="confirm-delete"
    >
      <p class="text-sm">Delete <strong>{confirming.name}</strong>?</p>
      <div class="flex gap-2">
        <button
          class="btn btn-sm preset-filled-error-500"
          type="button"
          onclick={() => confirming && remove(confirming)}
          data-testid="confirm-delete-yes"
        >
          Delete settings
        </button>
        <button class="btn btn-sm preset-tonal" type="button" onclick={() => (confirming = null)}>
          Keep them
        </button>
      </div>
    </div>
  {/if}

  <form class="card preset-outlined-surface-200-800 p-4 space-y-3 max-w-3xl" onsubmit={submit} novalidate>
    <h3 class="h4">{editing ? `Edit ${editing.name}` : 'New settings'}</h3>

    <div class="flex flex-wrap gap-3">
      <label class="label">
        <span class="label-text">Name</span>
        <input class="input w-56" type="text" bind:value={form.name} aria-label="Preset name" />
      </label>
      <label class="label">
        <span class="label-text">Voltage (V)</span>
        <input class="input w-32" type="number" step="0.001" min="0" bind:value={form.volt} aria-label="Voltage" />
      </label>
      <label class="label">
        <span class="label-text">Current limit (A)</span>
        <input class="input w-32" type="number" step="0.001" min="0" bind:value={form.curr_limit} aria-label="Current limit" />
      </label>
    </div>

    <div class="flex flex-wrap gap-3 items-end">
      <label class="label">
        <span class="label-text">OVP level (V)</span>
        <input class="input w-32" type="number" step="0.001" min="0" bind:value={form.ovp_level} aria-label="OVP level" />
      </label>
      <label class="flex items-center gap-2 text-sm pb-2">
        <input class="checkbox" type="checkbox" bind:checked={form.ovp_enabled} />
        <span>Enable OVP</span>
      </label>
      <label class="label">
        <span class="label-text">OCP level (A)</span>
        <input class="input w-32" type="number" step="0.001" min="0" bind:value={form.ocp_level} aria-label="OCP level" />
      </label>
      <label class="flex items-center gap-2 text-sm pb-2">
        <input class="checkbox" type="checkbox" bind:checked={form.ocp_enabled} />
        <span>Enable OCP</span>
      </label>
    </div>

    <hr class="hr" />

    <div class="flex gap-2">
      <button class="btn preset-filled" type="submit" disabled={busy || !form.name}>
        {editing ? 'Save settings' : 'Add settings'}
      </button>
      {#if editing}
        <button class="btn preset-tonal" type="button" onclick={cancel} disabled={busy}>
          Cancel
        </button>
      {/if}
    </div>
  </form>
</section>
