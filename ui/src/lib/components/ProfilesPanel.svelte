<script lang="ts">
  import { ApiFailure, deleteProfile, listProfiles } from '../api';
  import type { DeviceProfile, DeviceView } from '../types';
  import ProfileCard from './ProfileCard.svelte';
  import ProfileEditor from './ProfileEditor.svelte';
  import ActionIcon from './ActionIcon.svelte';
  import FormDialog from './FormDialog.svelte';
  import SearchBox from './SearchBox.svelte';
  import EmptyState from './EmptyState.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
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

<!--
  Two bands rather than two columns. What is powered is bounded by the hardware —
  nine rails, so a handful of profiles at most — and each one is a table of
  numbers that wants width. What is not powered is unbounded and one line each.
  Different shapes, so each band lays itself out on its own terms: both fill the
  window, and both size their tracks by content, so the number of cards across
  answers the window instead of a fixed split. A single card stays card-sized
  rather than stretching into gap.
-->
<section class="space-y-5" data-testid="profiles-panel">
  <div class="space-y-3">
    <div>
      <h2 class="h3">Device profiles</h2>
      <p class="text-sm opacity-70">
        A profile is a device and the rails it needs, in the order they come up.
      </p>
    </div>

    {#if !empty}
      <!-- Under the heading rather than in the far corner. It filters both
           columns, so it belongs to the section, not to either list. -->
      <SearchBox
        bind:value={query}
        label="Search profiles"
        count={shown.length}
        total={profiles.length}
      />
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

  <!--
    Two columns: what is running on the left, what is not on the right. Each
    scrolls on its own, so a long list of idle profiles cannot push a powered
    device off the page — the thing that must not happen on a bench.
  -->
  {#if !empty}
    <section class="space-y-3">
      <h3 class="text-sm font-semibold">
        Powered <span class="font-normal opacity-50">{live.length}</span>
      </h3>
      {#if live.length === 0}
        <p class="text-sm opacity-60">
          {query
            ? 'Nothing powered matches that.'
            : 'Nothing is powered. Apply a profile and switch it on to see it here.'}
        </p>
      {:else}
        <div
          class="grid items-start justify-start gap-4 grid-cols-[repeat(auto-fill,minmax(min(43rem,100%),48rem))] max-h-[45vh] overflow-y-auto pr-1"
          data-testid="live-profiles"
        >
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
    </section>

    <!-- The only rule on this page, because this is the only boundary on it
         that changes what you are allowed to assume about the hardware. -->
    <section class="space-y-3 border-t border-surface-200-800 pt-5">
      <h3 class="text-sm font-semibold">
        Not powered <span class="font-normal opacity-50">{idle.length}</span>
      </h3>
      <div
        class="grid items-start gap-3 grid-cols-[repeat(auto-fill,minmax(20rem,1fr))] max-h-[calc(100vh-30rem)] min-h-[14rem] overflow-y-auto pr-1"
        data-testid="idle-profiles"
      >
        <!-- The add action lives in the list it creates into, where what it
             makes will appear, rather than as an icon in a far corner. -->
        <button
          class="flex min-h-[6rem] w-full flex-col items-center justify-center gap-2 rounded border border-dashed border-surface-300-700 p-3 text-sm opacity-70 hover:opacity-100 hover:border-success-500"
          type="button"
          onclick={() => (editing = null)}
          data-testid="new-profile"
        >
          <svg class="size-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
            <path d="M12 5v14M5 12h14" />
          </svg>
          New profile
        </button>

        {#each idle as profile (profile.id)}
          <!-- Expanded, a card carries the rail table, which is wider than one
               track. Taking a second track is the room it needs, and it shows
               what the click did. -->
          <div
            class={expanded.has(profile.id) ? 'md:col-span-2' : ''}
          >
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
              oncollapse={() => {
                const next = new Set(expanded);
                next.delete(profile.id);
                expanded = next;
              }}
            />
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <ConfirmDialog
    open={confirming !== null}
    title="Delete {confirming?.name ?? ''}?"
    confirmLabel="Delete profile"
    cancelLabel="Keep it"
    testid="confirm-delete"
    onconfirm={() => confirming && remove(confirming)}
    oncancel={() => (confirming = null)}
  >
    <!-- A profile is minutes of work; a misclick should not be enough to lose
         it. Red appears here, at the point of commitment, and not on the row. -->
    Its rails stay exactly as they are — this removes the profile, not the power.
  </ConfirmDialog>
</section>
