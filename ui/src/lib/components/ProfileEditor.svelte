<script lang="ts">
  import {
    ApiFailure,
    createPreset,
    createProfile,
    listPresets,
    updateProfile,
    type NewBinding,
  } from '../api';
  import { channelChoices, exceedsLimits, findChannel } from '../bench-info';
  import { trim } from '../format';
  import ActionIcon from './ActionIcon.svelte';
  import ChannelPicker from './ChannelPicker.svelte';
  import type { ChannelPreset, DeviceProfile, DeviceView } from '../types';

  let {
    devices,
    profile = null,
    ondone,
    ondirty,
  }: {
    devices: DeviceView[];
    profile?: DeviceProfile | null;
    ondone?: () => void;
    ondirty?: (dirty: boolean) => void;
  } = $props();

  let presets = $state<ChannelPreset[]>([]);

  // The form is seeded from `profile` once and then owned by the user; callers
  // key this component on the profile id, so choosing a different profile
  // remounts it rather than yanking a form someone is typing into.
  // svelte-ignore state_referenced_locally
  let name = $state(profile?.name ?? '');
  // svelte-ignore state_referenced_locally
  let description = $state(profile?.description ?? '');
  // svelte-ignore state_referenced_locally
  let bindings = $state<NewBinding[]>(
    profile?.bindings.map((b) => ({
      slot: b.slot,
      channel: b.channel,
      preset_id: b.preset.id,
      sequence_order: b.sequence_order,
      delay_ms: b.delay_ms,
      output_on: b.output_on,
    })) ?? [],
  );
  let error = $state<string | null>(null);
  let busy = $state(false);

  // svelte-ignore state_referenced_locally
  const pristine = JSON.stringify({ name, description, bindings });
  const dirty = $derived(
    JSON.stringify({ name, description, bindings }) !== pristine,
  );
  // Reported upward rather than mirrored: the form owns this state, and a copy
  // in the panel would be one more thing to keep in step.
  $effect(() => ondirty?.(dirty));

  // Inline preset creation. Without this, needing a preset that does not exist
  // yet means leaving the editor and losing the profile in progress — a dead
  // end on the main path of the feature.
  let newPresetFor = $state<number | null>(null);
  let draft = $state({ name: '', volt: 0, curr_limit: 1 });

  const choices = $derived(channelChoices(devices));

  $effect(() => {
    listPresets()
      .then((p) => (presets = p))
      .catch((e) => (error = String(e)));
  });

  function addBinding() {
    // The first channel nothing else has claimed. Defaulting to the very first
    // one meant every rail after the first landed on a channel already in use
    // and warned about it before the user had done anything.
    const used = new Set(bindings.map((b) => `${b.slot}/${b.channel}`));
    const first = choices.find((c) => !used.has(c.key)) ?? choices[0];
    bindings = [
      ...bindings,
      {
        slot: first?.slot ?? '',
        channel: first?.channel ?? 'CH1',
        preset_id: presets[0]?.id ?? 0,
        sequence_order: bindings.length + 1,
        delay_ms: 0,
        // A rail named in a profile is normally one the profile powers.
        output_on: true,
      },
    ];
  }

  const removeBinding = (index: number) =>
    (bindings = bindings.filter((_, i) => i !== index));

  /**
   * Order is positional, not typed. A free number field allowed `1, 1, 1`,
   * storing ties whose resolution was then incidental; the array's order is the
   * sequence, and `sequence_order` is derived from it on save.
   */
  function move(index: number, by: -1 | 1) {
    const target = index + by;
    if (target < 0 || target >= bindings.length) return;
    const next = [...bindings];
    [next[index], next[target]] = [next[target]!, next[index]!];
    bindings = next;
  }

  function startPreset(index: number) {
    newPresetFor = index;
    draft = { name: '', volt: 0, curr_limit: 1 };
  }

  async function savePreset() {
    if (newPresetFor === null) return;
    error = null;
    busy = true;
    try {
      const created = await createPreset({
        name: draft.name,
        volt: Number(draft.volt),
        curr_limit: Number(draft.curr_limit),
        // Protection defaults follow the setpoint; they can be tuned in the
        // preset library, and a preset with no protection is worse than one
        // with a sensible starting point.
        ovp_level: Number(draft.volt) * 1.1,
        ovp_enabled: true,
        ocp_level: Number(draft.curr_limit) * 1.1,
        ocp_enabled: true,
      });
      presets = [...presets, created];
      bindings[newPresetFor]!.preset_id = created.id;
      newPresetFor = null;
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }

  /**
   * Warnings the server would also raise, surfaced while the profile is being
   * built rather than at submission. The server remains the authority.
   */
  const warnings = $derived(
    bindings.map((binding) => {
      const channel = findChannel(devices, binding.slot, binding.channel);
      if (!channel) return `${binding.slot} is not a configured supply`;

      const preset = presets.find((p) => p.id === binding.preset_id);
      if (!preset) return 'Choose a preset';

      const limit = exceedsLimits(preset, channel.limits);
      if (limit) return limit;

      const duplicate =
        bindings.filter((b) => b.slot === binding.slot && b.channel === binding.channel)
          .length > 1;
      return duplicate ? 'This channel is bound more than once' : null;
    }),
  );

  const hasWarnings = $derived(warnings.some((w) => w !== null));

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    busy = true;
    try {
      const payload = {
        name,
        description,
        bindings: bindings.map((b, index) => ({ ...b, sequence_order: index + 1 })),
      };
      if (profile) {
        await updateProfile(profile.id, payload);
      } else {
        await createProfile(payload);
      }
      ondone?.();
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<form
  class="space-y-4 w-[52rem] max-w-full"
  onsubmit={submit}
  novalidate
  data-testid="profile-editor"
>
  <div class="flex flex-wrap gap-4">
    <label class="label">
      <span class="label-text">Name</span>
      <!-- Field width is a signal about expected input; a name is not 600px of
           name. -->
      <input class="input w-72" type="text" bind:value={name} aria-label="Profile name" />
    </label>
    <label class="label grow">
      <span class="label-text">Description</span>
      <input
        class="input max-w-md"
        type="text"
        bind:value={description}
        aria-label="Profile description"
      />
    </label>
  </div>

  <hr class="hr" />

  <div class="space-y-2">
    <div class="flex items-center justify-between">
      <div>
        <h4 class="font-semibold">Power-up sequence</h4>
        <p class="text-xs opacity-60">
          Rails come up top to bottom. A dwell pauses before the next one starts.
        </p>
      </div>
      <button
        class="btn btn-sm preset-tonal"
        type="button"
        onclick={addBinding}
        disabled={choices.length === 0}
      >
        Add rail
      </button>
    </div>

    {#if choices.length === 0}
      <p class="text-sm opacity-60">
        No supplies are visible yet, so there is nothing to bind to.
      </p>
    {/if}

    {#each bindings as binding, index (index)}
      <div
        class="rounded border border-surface-200-800 p-3 space-y-2"
        data-testid="binding-{index}"
      >
        <!-- The step is titled rather than numbered in the margin: a bare digit
             beside a wrapping row of fields ended up orphaned on its own line. -->
        <div class="flex items-center justify-between gap-2">
          <span class="text-xs font-semibold opacity-70">Rail {index + 1}</span>
          <div class="flex gap-1">
            <button
              class="btn-icon btn-icon-lg opacity-70 hover:opacity-100"
              type="button"
              aria-label="Move rail {index + 1} earlier"
              disabled={index === 0}
              onclick={() => move(index, -1)}
            >
              ↑
            </button>
            <button
              class="btn-icon btn-icon-lg opacity-70 hover:opacity-100"
              type="button"
              aria-label="Move rail {index + 1} later"
              disabled={index === bindings.length - 1}
              onclick={() => move(index, 1)}
            >
              ↓
            </button>
            <ActionIcon
              action="delete"
              label="Remove rail {index + 1}"
              onclick={() => removeBinding(index)}
            />
          </div>
        </div>

        <div class="flex flex-wrap gap-4 items-start">
          <div class="label grow">
            <span class="label-text text-xs">Channel</span>
            <ChannelPicker
              {devices}
              slot={binding.slot}
              channel={binding.channel}
              taken={new Set(
                bindings
                  .filter((_, i) => i !== index)
                  .map((b) => `${b.slot}/${b.channel}`),
              )}
              label="Rail {index + 1} channel"
              onSelect={(slot, channel) => {
                bindings[index] = { ...binding, slot, channel: channel as typeof binding.channel };
              }}
            />
          </div>

          <label class="label min-w-56">
            <span class="label-text text-xs">Settings</span>
            <select
              class="select"
              aria-label="Rail {index + 1} preset"
              value={binding.preset_id}
              onchange={(e) => {
                if (e.currentTarget.value === '__new') {
                  startPreset(index);
                } else {
                  bindings[index]!.preset_id = Number(e.currentTarget.value);
                }
              }}
            >
              {#each presets as preset (preset.id)}
                <option value={preset.id}>
                  {preset.name} ({trim(preset.volt, 'V')})
                </option>
              {/each}
              <option value="__new">New settings…</option>
            </select>
          </label>

          <label class="flex items-center gap-2 text-sm pb-2">
            <input
              class="checkbox"
              type="checkbox"
              aria-label="Rail {index + 1} energise on power-on"
              bind:checked={bindings[index]!.output_on}
            />
            <span>Energise</span>
          </label>
        </div>

        {#if newPresetFor === index}
          <div
            class="preset-tonal p-3 rounded space-y-2"
            data-testid="inline-preset-{index}"
          >
            <p class="text-xs opacity-70">
              New settings, saved to the library and used for this rail.
            </p>
            <div class="flex flex-wrap gap-2 items-end">
              <label class="label">
                <span class="label-text text-xs">Name</span>
                <input class="input w-48" type="text" bind:value={draft.name} aria-label="New settings name" />
              </label>
              <label class="label w-28">
                <span class="label-text text-xs">Voltage (V)</span>
                <input class="input" type="number" step="0.001" min="0" bind:value={draft.volt} aria-label="New settings voltage" />
              </label>
              <label class="label w-32">
                <span class="label-text text-xs">Current limit (A)</span>
                <input class="input" type="number" step="0.001" min="0" bind:value={draft.curr_limit} aria-label="New settings current limit" />
              </label>
              <button
                class="btn btn-sm preset-filled"
                type="button"
                disabled={busy || !draft.name}
                onclick={savePreset}
              >
                Save settings
              </button>
              <button
                class="btn btn-sm preset-tonal"
                type="button"
                onclick={() => (newPresetFor = null)}
              >
                Cancel
              </button>
            </div>
          </div>
        {/if}

        {#if warnings[index]}
          <p class="text-xs preset-tonal-warning p-2 rounded" data-testid="binding-warning-{index}">
            {warnings[index]}
          </p>
        {/if}
      </div>

      {#if index < bindings.length - 1}
        <!--
          Between the two rails it separates, as the card draws it. Inside rail
          N's own block it read as "this rail waits", which is the opposite of
          what happens.
        -->
        <label class="flex items-center gap-2 pl-4 text-xs opacity-70">
          <span class="border-l border-surface-300-700 h-4"></span>
          <span>then wait</span>
          <input
            class="input w-20 text-right"
            type="number"
            min="0"
            aria-label="Rail {index + 1} dwell"
            bind:value={bindings[index]!.delay_ms}
          />
          <span>ms before rail {index + 2}</span>
        </label>
      {/if}
    {/each}
  </div>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert" data-testid="editor-error">
      {error}
    </p>
  {/if}

  <!-- Sticky: with two rails the action this form exists for was below the
       fold, reachable only by scrolling past everything. -->
  <div
    class="sticky bottom-0 -mb-1 flex gap-2 pt-3 pb-1 bg-surface-50-950 border-t border-surface-200-800"
  >
    <button
      class="btn preset-filled"
      type="submit"
      disabled={busy || !name || bindings.length === 0 || hasWarnings}
    >
      {profile ? 'Save profile' : 'Create profile'}
    </button>
    <button class="btn preset-tonal" type="button" onclick={() => ondone?.()} disabled={busy}>
      Cancel
    </button>
  </div>
</form>
