<script lang="ts">
  import { ApiFailure, deleteProfile, listProfiles } from '../api';
  import type { DeviceProfile, DeviceView } from '../types';
  import ProfileCard from './ProfileCard.svelte';
  import ProfileEditor from './ProfileEditor.svelte';
  import ActionIcon from './ActionIcon.svelte';
  import FormDialog from './FormDialog.svelte';
  import SearchBox from './SearchBox.svelte';
  import EmptyState from './EmptyState.svelte';
  import { profileState } from '../profile-state';

  let { devices }: { devices: DeviceView[] } = $props();

  let profiles = $state<DeviceProfile[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  // undefined = not editing; null = creating; a profile = editing that one.
  let editing = $state<DeviceProfile | null | undefined>(undefined);
  let confirming = $state<DeviceProfile | null>(null);
  let query = $state('');
  const empty = $derived(loaded && profiles.length === 0);

  // Idle profiles the user has opened by hand; live ones are always open.
  let expanded = $state(new Set<number>());

  // Reported by the editor: the form owns this state, and the dialog's footer
  // needs both — whether closing would lose work, and whether saving is legal.
  let editorDirty = $state(false);
  let editorCanSave = $state(false);

  // Matches the profile's own words and the rails it names, so "psu2" or "3v3"
  // finds the devices that use them — which is how you look for a profile when
  // you remember the hardware but not the name.
  /**
   * Live profiles first, and always fully visible.
   *
   * Alphabetical order scattered the powered ones between inert ones, so with
   * enough profiles a device that is actually energised scrolls off the page —
   * which is the one thing that must not happen on a bench.
   */
  const isLive = (p: DeviceProfile) =>
    ['powered', 'partly-powered', 'live-altered'].includes(profileState(p, devices));

  const shown = $derived(
    profiles.filter((profile) => {
      const needle = query.trim().toLowerCase();
      if (!needle) return true;
      const haystack = [
        profile.name,
        profile.description,
        ...profile.bindings.flatMap((b) => [b.slot, b.channel, b.preset.name]),
      ]
        .join(' ')
        .toLowerCase();
      return haystack.includes(needle);
    }),
  );

  async function refresh() {
    try {
      profiles = await listProfiles();
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

  async function done() {
    editing = undefined;
    await refresh();
  }

  const live = $derived(shown.filter(isLive));
  const idle = $derived(shown.filter((p) => !isLive(p)));

  async function remove(profile: DeviceProfile) {
    try {
      await deleteProfile(profile.id);
      confirming = null;
      await refresh();
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    }
  }
</script>

<section class="space-y-4" data-testid="profiles-panel">
  <div class="flex items-center justify-between gap-2">
    <div>
      <h2 class="h3">Device profiles</h2>
      <p class="text-sm opacity-70">
        A profile is a device and the rails it needs, in the order they come up.
      </p>
    </div>
    {#if !empty}
      <div class="flex items-center gap-3">
        <SearchBox
          bind:value={query}
          label="Search profiles"
          count={shown.length}
          total={profiles.length}
        />
        <ActionIcon
          action="new"
          label="New profile"
          testid="new-profile"
          onclick={() => (editing = null)}
        />
      </div>
    {/if}
  </div>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert">{error}</p>
  {/if}

  <FormDialog
    open={editing !== undefined}
    title={editing ? `Edit ${editing.name}` : 'New profile'}
    onclose={() => (editing = undefined)}
    testid="profile-dialog"
    dirty={editorDirty}
  >
    <!--
      Mounted only while the dialog is open, so every open starts from a fresh
      form. This was keyed on `editing?.id ?? 'new'`, which is the same string
      whether the dialog is closed or being opened for a new profile — so
      creating one and then pressing New reused the component, and its state,
      complete with the profile just saved.
    -->
    {#if editing !== undefined}
      <ProfileEditor
        {devices}
        profile={editing}
        ondone={done}
        ondirty={(d) => (editorDirty = d)}
        oncansave={(ok) => (editorCanSave = ok)}
      />
    {/if}

    {#snippet footer(requestClose: () => void)}
      <button
        class="btn preset-filled"
        type="submit"
        form="profile-form"
        disabled={!editorCanSave}
      >
        {editing ? 'Save profile' : 'Create profile'}
      </button>
      <button class="btn preset-tonal" type="button" onclick={requestClose}>
        Cancel
      </button>
    {/snippet}
  </FormDialog>

  {#if empty}
    <EmptyState
      title="No profiles yet"
      action="New profile"
      onAction={() => (editing = null)}
      testid="empty-profiles"
    >
      A profile names a device and the rails it needs, so you can stage them
      together and bring them up in order — and power-cycle them later without
      touching each channel.
    </EmptyState>
  {:else if loaded && shown.length === 0}
    <p class="opacity-60 text-sm">
      No profile matches "{query}".
    </p>
  {/if}

  <!-- A list rather than a grid: profiles vary in length, and forcing them into
       columns produced rows of wildly different heights. Scrolls within itself
       so the search box stays reachable however many there are. -->
  <div class="space-y-4">
    <!-- Outside the scrolling region on purpose: a powered device must not be
         something you have to scroll to find. Only the idle list scrolls. -->
    {#if live.length > 0}
      <div class="space-y-3" data-testid="live-profiles">
        <h3 class="text-xs opacity-60">
          Live &mdash; {live.length} of {shown.length}
        </h3>
        {#each live as profile (profile.id)}
          <ProfileCard
            {profile}
            {devices}
            onchange={refresh}
            onedit={() => (editing = profile)}
            ondelete={() => (confirming = profile)}
          />
        {/each}
      </div>
    {/if}

    {#if idle.length > 0}
      <div
        class="space-y-2 max-h-[calc(100vh-22rem)] overflow-y-auto pr-1"
        data-testid="idle-profiles"
      >
        {#if live.length > 0}
          <h3 class="text-xs opacity-60">Idle</h3>
        {/if}
        {#each idle as profile (profile.id)}
          <ProfileCard
            {profile}
            {devices}
            compact={!expanded.has(profile.id)}
            onchange={refresh}
            onedit={() => (editing = profile)}
            ondelete={() => (confirming = profile)}
            onexpand={() => {
              const next = new Set(expanded);
              next.add(profile.id);
              expanded = next;
            }}
          />
        {/each}
      </div>
    {/if}
  </div>

  {#if confirming}
    <!-- A profile is minutes of work; a misclick should not be enough to lose
         it. Red appears here, at the point of commitment, and not on the row. -->
    <div
      class="card preset-outlined-surface-200-800 p-4 space-y-3"
      role="alertdialog"
      aria-label="Confirm delete"
      data-testid="confirm-delete"
    >
      <p class="text-sm">
        Delete <strong>{confirming.name}</strong>? Its rails stay exactly as they
        are — this removes the profile, not the power.
      </p>
      <div class="flex gap-2">
        <button
          class="btn btn-sm preset-filled-error-500"
          type="button"
          onclick={() => confirming && remove(confirming)}
          data-testid="confirm-delete-yes"
        >
          Delete profile
        </button>
        <button class="btn btn-sm preset-tonal" type="button" onclick={() => (confirming = null)}>
          Keep it
        </button>
      </div>
    </div>
  {/if}
</section>
