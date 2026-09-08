<script lang="ts">
  import { ApiFailure, createProfile, listPresets, updateProfile, type NewBinding } from '../api';
  import { channelChoices, exceedsLimits, findChannel } from '../bench-info';
  import type { ChannelPreset, DeviceProfile, DeviceView } from '../types';

  let {
    devices,
    profile = null,
    ondone,
  }: {
    devices: DeviceView[];
    profile?: DeviceProfile | null;
    ondone?: () => void;
  } = $props();

  let presets = $state<ChannelPreset[]>([]);

  // The form is seeded from `profile` once and then owned by the user; it must
  // not be yanked back to server state mid-edit. Callers key this component on
  // the profile id, so choosing a different profile remounts it with fresh
  // seed values rather than mutating a form someone is typing into.
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

  const choices = $derived(channelChoices(devices));

  $effect(() => {
    listPresets()
      .then((p) => (presets = p))
      .catch((e) => (error = String(e)));
  });

  function addBinding() {
    const first = choices[0];
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
   * Warnings the server would also raise, surfaced while the profile is being
   * built rather than at submission. The server remains the authority — this
   * only saves the user from constructing something certain to be refused.
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
      const payload = { name, description, bindings };
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
  class="card preset-outlined-surface-200-800 p-4 space-y-4"
  onsubmit={submit}
  novalidate
  data-testid="profile-editor"
>
  <h3 class="h4">{profile ? `Edit ${profile.name}` : 'New profile'}</h3>

  <div class="grid gap-2 md:grid-cols-2">
    <label class="label">
      <span class="label-text">Name</span>
      <input class="input" type="text" bind:value={name} aria-label="Profile name" />
    </label>
    <label class="label">
      <span class="label-text">Description</span>
      <input class="input" type="text" bind:value={description} aria-label="Profile description" />
    </label>
  </div>

  <div class="space-y-2">
    <div class="flex items-center justify-between">
      <h4 class="font-semibold">Rails</h4>
      <button
        class="btn btn-sm preset-tonal"
        type="button"
        onclick={addBinding}
        disabled={choices.length === 0 || presets.length === 0}
      >
        Add rail
      </button>
    </div>

    {#if choices.length === 0}
      <p class="text-sm opacity-60">
        No supplies are visible yet, so there is nothing to bind to.
      </p>
    {:else if presets.length === 0}
      <p class="text-sm opacity-60">Create a preset first — a rail needs settings to apply.</p>
    {/if}

    {#each bindings as binding, index (index)}
      <div class="space-y-1" data-testid="binding-{index}">
        <div class="flex flex-wrap gap-2 items-end">
          <label class="label grow">
            <span class="label-text text-xs">Channel</span>
            <select
              class="select"
              aria-label="Rail {index + 1} channel"
              value={`${binding.slot}/${binding.channel}`}
              onchange={(e) => {
                const [slot, channel] = e.currentTarget.value.split('/');
                bindings[index] = { ...binding, slot: slot!, channel: channel! };
              }}
            >
              {#each choices as choice (choice.key)}
                <option value={choice.key}>
                  {choice.label} · {choice.channel} (max
                  {choice.limits.max_volt.toFixed(1)} V / {choice.limits.max_curr.toFixed(1)} A)
                </option>
              {/each}
            </select>
          </label>

          <label class="label grow">
            <span class="label-text text-xs">Preset</span>
            <select class="select" aria-label="Rail {index + 1} preset" bind:value={bindings[index]!.preset_id}>
              {#each presets as preset (preset.id)}
                <option value={preset.id}>
                  {preset.name} ({preset.volt.toFixed(3)} V)
                </option>
              {/each}
            </select>
          </label>

          <label class="label w-24">
            <span class="label-text text-xs">Order</span>
            <input
              class="input"
              type="number"
              min="1"
              aria-label="Rail {index + 1} order"
              bind:value={bindings[index]!.sequence_order}
            />
          </label>

          <label class="label w-28">
            <span class="label-text text-xs">Dwell (ms)</span>
            <input
              class="input"
              type="number"
              min="0"
              aria-label="Rail {index + 1} dwell"
              bind:value={bindings[index]!.delay_ms}
            />
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

          <button
            class="btn btn-sm preset-tonal-error"
            type="button"
            onclick={() => removeBinding(index)}
          >
            Remove
          </button>
        </div>

        {#if warnings[index]}
          <p class="text-xs preset-tonal-warning p-2 rounded" data-testid="binding-warning-{index}">
            {warnings[index]}
          </p>
        {/if}
      </div>
    {/each}

    {#if bindings.length > 1}
      <p class="text-xs opacity-60">
        Rails come up in ascending order. The dwell is the pause <em>after</em> that rail
        is switched on, before the next one. Unticking <em>Energise</em> stages the rail's
        settings without bringing it up.
      </p>
    {/if}
  </div>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert" data-testid="editor-error">
      {error}
    </p>
  {/if}

  <div class="flex gap-2">
    <button
      class="btn preset-filled-primary-500"
      type="submit"
      disabled={busy || !name || bindings.length === 0 || hasWarnings}
    >
      {profile ? 'Save' : 'Create'}
    </button>
    <button class="btn preset-tonal" type="button" onclick={() => ondone?.()} disabled={busy}>
      Cancel
    </button>
  </div>
</form>
