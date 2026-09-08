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
      await refresh();
    } catch (e) {
      error = e instanceof ApiFailure ? e.detail.message : String(e);
    }
  }
</script>

<section class="space-y-4" data-testid="profiles-panel">
  <div class="flex items-center justify-between gap-2">
    <h2 class="h3">Device profiles</h2>
    {#if editing === undefined}
      <button
        class="btn preset-filled-primary-500"
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
      No profiles yet. A profile names a device and the rails it needs, so you can
      stage them all at once and bring them up in order.
    </p>
  {/if}

  <div class="grid gap-3 md:grid-cols-2">
    {#each profiles as profile (profile.id)}
      <div class="space-y-2">
        <ProfileCard {profile} onchange={refresh} />
        <div class="flex gap-2">
          <button
            class="btn btn-sm preset-tonal"
            type="button"
            onclick={() => (editing = profile)}
            data-testid="edit-profile-{profile.id}"
          >
            Edit
          </button>
          <button
            class="btn btn-sm preset-tonal-error"
            type="button"
            onclick={() => remove(profile)}
          >
            Delete
          </button>
        </div>
      </div>
    {/each}
  </div>
</section>
