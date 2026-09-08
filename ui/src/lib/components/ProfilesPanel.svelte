<script lang="ts">
  import { listProfiles } from '../api';
  import type { DeviceProfile } from '../types';
  import ProfileCard from './ProfileCard.svelte';

  let profiles = $state<DeviceProfile[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);

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
</script>

<section class="space-y-4" data-testid="profiles-panel">
  <h2 class="h3">Device profiles</h2>

  {#if error}
    <p class="preset-tonal-error p-3 rounded text-sm" role="alert">{error}</p>
  {/if}

  {#if loaded && profiles.length === 0}
    <p class="opacity-60 text-sm">
      No profiles yet. A profile names a device and the rails it needs, so you can
      stage them all at once.
    </p>
  {/if}

  <div class="grid gap-3 md:grid-cols-2">
    {#each profiles as profile (profile.id)}
      <ProfileCard {profile} onchange={refresh} />
    {/each}
  </div>
</section>
