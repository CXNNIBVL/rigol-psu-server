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
  import ActionIcon from './ActionIcon.svelte';
  import FormDialog from './FormDialog.svelte';
  import SearchBox from './SearchBox.svelte';
  import EmptyState from './EmptyState.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';

  let presets = $state<ChannelPreset[]>([]);
  let profiles = $state<DeviceProfile[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);
  // undefined = dialog closed; null = creating; a preset = editing that one.
  let editing = $state<ChannelPreset | null | undefined>(undefined);
  let query = $state('');
  let confirming = $state<ChannelPreset | null>(null);
  let busy = $state(false);
  let form = $state(blank());
  let pristine = $state(JSON.stringify(blank()));
  const dirty = $derived(JSON.stringify(form) !== pristine);

  // The server refuses a trip point below the value it protects, because it
  // would fire the moment the rail comes up. Saying so while you type beats
  // saying it after you submit.
  const ovpTooLow = $derived(form.ovp_enabled && form.ovp_level < form.volt);
  const ocpTooLow = $derived(form.ocp_enabled && form.ocp_level < form.curr_limit);

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

  const empty = $derived(loaded && presets.length === 0);

  const shown = $derived(
    presets.filter((preset) =>
      preset.name.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );

  function edit(preset: ChannelPreset) {
    editing = preset;
    const { id: _id, ...rest } = preset;
    form = { ...rest };
    pristine = JSON.stringify(form);
  }

  function create() {
    editing = null;
    form = blank();
    pristine = JSON.stringify(form);
    error = null;
  }

  function cancel() {
    editing = undefined;
    form = blank();
    pristine = JSON.stringify(form);
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
  <div class="space-y-3">
    <div>
      <h2 class="h3">Rail settings</h2>
      <p class="text-sm opacity-70">
        Saved voltage and current settings for one channel. Profiles use them; on
        their own they are not applied to anything.
      </p>
    </div>

    {#if !empty}
      <!-- Under the heading, with the list it filters, rather than in the far
           corner of the page. -->
      <SearchBox
        bind:value={query}
        label="Search settings"
        count={shown.length}
        total={presets.length}
      />
    {/if}
  </div>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert" data-testid="preset-error">
      {error}
    </p>
  {/if}

  {#if empty}
    <EmptyState
      title="No rail settings yet"
      action="New settings"
      onAction={create}
      testid="empty-presets"
    >
      A setting is one channel's voltage and current limit, saved under a name so
      profiles can use it. You can also create them as you go, from a profile's
      rails.
    </EmptyState>
  {:else}
  <!-- A table rather than cards: these are the smallest unit, least interesting
       alone and most useful compared down a column. -->
  <div class="card preset-outlined-surface-200-800 overflow-x-auto max-h-[calc(100vh-22rem)] overflow-y-auto">
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
        <tr class="border-t border-surface-200-800">
          <td colspan="7" class="p-0">
            <!-- At the top of the list it creates into, where what it makes
                 will appear, rather than as an icon in a far corner. -->
            <button
              class="flex w-full items-center gap-2 p-3 text-sm opacity-70 hover:opacity-100"
              type="button"
              onclick={create}
              data-testid="new-preset"
            >
              <svg class="size-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true">
                <path d="M12 5v14M5 12h14" />
              </svg>
              New settings
            </button>
          </td>
        </tr>
        {#each shown as preset (preset.id)}
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
                <ActionIcon
                  action="edit"
                  label="Edit {preset.name}"
                  onclick={() => edit(preset)}
                  disabled={busy}
                />
                <ActionIcon
                  action="delete"
                  label="Delete {preset.name}"
                  onclick={() => (confirming = preset)}
                  disabled={busy || (usage.get(preset.id) ?? []).length > 0}
                  title={(usage.get(preset.id) ?? []).length > 0
                    ? `Used by ${(usage.get(preset.id) ?? []).join(', ')}`
                    : undefined}
                />
              </div>
            </td>
          </tr>
        {/each}
        {#if loaded && shown.length === 0}
          <tr class="border-t border-surface-200-800">
            <td colspan="7" class="p-4 text-sm opacity-60">
              Nothing matches "{query}".
            </td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>
  {/if}

  <ConfirmDialog
    open={confirming !== null}
    title="Delete {confirming?.name ?? ''}?"
    confirmLabel="Delete settings"
    cancelLabel="Keep them"
    testid="confirm-delete"
    onconfirm={() => confirming && remove(confirming)}
    oncancel={() => (confirming = null)}
  >
    Any profile using it keeps the values it was given; this removes the saved
    name, not the setpoints already on a rail.
  </ConfirmDialog>

  <FormDialog
    open={editing !== undefined}
    title={editing ? `Edit ${editing.name}` : 'New settings'}
    onclose={cancel}
    testid="preset-dialog"
    {dirty}
  >
    {#if editing !== undefined}
      <form id="preset-form" class="space-y-4 w-[32rem] max-w-full" onsubmit={submit} novalidate>
        <label class="label">
          <span class="label-text">Name</span>
          <input class="input w-72" type="text" bind:value={form.name} aria-label="Preset name" />
        </label>

        <!-- Paired, because voltage and its current limit describe one output.
             A grid rather than a flex row: Skeleton's `label` is width:100%, so
             two of them side by side always wrapped. -->
        <div class="grid grid-cols-2 gap-4">
          <label class="label">
            <span class="label-text">Voltage (V)</span>
            <input class="input" type="number" step="0.001" min="0" bind:value={form.volt} aria-label="Voltage" />
          </label>
          <label class="label">
            <span class="label-text">Current limit (A)</span>
            <input class="input" type="number" step="0.001" min="0" bind:value={form.curr_limit} aria-label="Current limit" />
          </label>
        </div>

        <hr class="hr" />

        <!--
          Each protection is one setting: a threshold and whether it is armed.
          Split across two rows they read as two unrelated controls, and the
          level sat there looking active while switched off.
        -->
        <div class="space-y-2">
          <div class="flex flex-wrap items-center gap-3">
            <label class="flex items-center gap-2 text-sm w-56">
              <input class="checkbox" type="checkbox" bind:checked={form.ovp_enabled} />
              <span>Over-voltage protection</span>
            </label>
            <input
              class="input w-32 {form.ovp_enabled ? '' : 'opacity-50'}"
              type="number"
              step="0.001"
              min="0"
              aria-label="OVP level"
              disabled={!form.ovp_enabled}
              bind:value={form.ovp_level}
            />
            <span class="text-xs opacity-60">V</span>
            {#if ovpTooLow}
              <span class="text-xs preset-tonal-warning px-2 py-1 rounded" data-testid="ovp-too-low">
                Must be at least {form.volt} V, or it trips as the rail comes up
              </span>
            {/if}
          </div>

          <div class="flex flex-wrap items-center gap-3">
            <label class="flex items-center gap-2 text-sm w-56">
              <input class="checkbox" type="checkbox" bind:checked={form.ocp_enabled} />
              <span>Over-current protection</span>
            </label>
            <input
              class="input w-32 {form.ocp_enabled ? '' : 'opacity-50'}"
              type="number"
              step="0.001"
              min="0"
              aria-label="OCP level"
              disabled={!form.ocp_enabled}
              bind:value={form.ocp_level}
            />
            <span class="text-xs opacity-60">A</span>
            {#if ocpTooLow}
              <span class="text-xs preset-tonal-warning px-2 py-1 rounded">
                Must be at least {form.curr_limit} A
              </span>
            {/if}
          </div>
        </div>

        {#if editing && (usage.get(editing.id) ?? []).length > 0}
          <!-- Editing a preset changes every profile using it, which is worth
               knowing at the moment you change it. -->
          <p class="text-xs opacity-70" data-testid="preset-usage">
            Used by {(usage.get(editing.id) ?? []).join(', ')} — saving changes what
            those profiles apply.
          </p>
        {/if}

      </form>
    {/if}

    {#snippet footer(requestClose: () => void)}
      <button
        class="btn preset-filled"
        type="submit"
        form="preset-form"
        disabled={busy || !form.name || ovpTooLow || ocpTooLow}
      >
        {editing ? 'Save settings' : 'Add settings'}
      </button>
      <button class="btn preset-tonal" type="button" onclick={requestClose} disabled={busy}>
        Cancel
      </button>
    {/snippet}
  </FormDialog>

</section>
