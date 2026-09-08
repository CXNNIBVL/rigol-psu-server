<script lang="ts">
  import { ApiFailure, deleteProfile, listProfiles } from '../api';
  import type { DeviceProfile, DeviceView } from '../types';
  import ProfileCard from './ProfileCard.svelte';
  import ProfileEditor from './ProfileEditor.svelte';

  let { devices }: { devices: DeviceView[] } = $props();

  let profiles = $state<DeviceProfile[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  // undefined = not editing; null = creating; a profile = editing that one.
  let editing = $state<DeviceProfile | null | undefined>(undefined);
  let confirming = $state<DeviceProfile | null>(null);

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
    {#if editing === undefined}
      <button
        class="btn preset-filled"
        type="button"
        onclick={() => (editing = null)}
        data-testid="new-profile"
      >
        New profile
      </button>
    {/if}
  </div>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert">{error}</p>
  {/if}

  {#if editing !== undefined}
    <!-- Keyed so switching between profiles reseeds the form rather than
         leaving stale values from the last one. -->
    {#key editing?.id ?? 'new'}
      <ProfileEditor {devices} profile={editing} ondone={done} />
    {/key}
  {/if}

  {#if loaded && profiles.length === 0 && editing === undefined}
    <p class="opacity-60 text-sm">
      No profiles yet. Create one to stage a device's rails together and bring
      them up in order.
    </p>
  {/if}

  <!-- A list rather than a grid: profiles vary in length, and forcing them into
       columns produced rows of wildly different heights. -->
  <div class="space-y-3">
    {#each profiles as profile (profile.id)}
      <ProfileCard
        {profile}
        {devices}
        onchange={refresh}
        onedit={() => (editing = profile)}
        ondelete={() => (confirming = profile)}
      />
    {/each}
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
