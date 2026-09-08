<script lang="ts">
  import { onMount } from 'svelte';
  import { Navigation } from '@skeletonlabs/skeleton-svelte';
  import { Bench } from './lib/bench.svelte';
  import DeviceCard from './lib/components/DeviceCard.svelte';
  import ProfilesPanel from './lib/components/ProfilesPanel.svelte';
  import PresetsPanel from './lib/components/PresetsPanel.svelte';
  import HealthPanel from './lib/components/HealthPanel.svelte';
  import NavIcon from './lib/components/NavIcon.svelte';

  const bench = new Bench();

  type Tab = 'bench' | 'profiles' | 'presets' | 'health';
  let tab = $state<Tab>('bench');

  const TABS: Array<{ id: Tab; label: string }> = [
    { id: 'bench', label: 'Bench' },
    { id: 'profiles', label: 'Profiles' },
    { id: 'presets', label: 'Presets' },
    { id: 'health', label: 'Health' },
  ];

  const devices = $derived(bench.snapshot?.devices ?? []);

  // Surfaced on the rail so a supply going offline is noticeable from any
  // section, not only from the one that happens to show it.
  const unhealthy = $derived(
    devices.filter(
      (d) => d.state !== 'online' || d.channels.some((c) => c.violation !== null),
    ).length,
  );

  // `onMount` rather than `$effect`: the connection must be opened exactly once
  // for the life of the component. An effect would re-run if anything it
  // touched became reactive, silently turning every state update into a
  // reconnect.
  onMount(() => {
    bench.start();
    return () => bench.stop();
  });
</script>

<div class="flex min-h-screen">
  <!--
    `h-screen` rather than leaning on the flex container: the rail's own stylesheet
    sets `height: 100%`, which resolves against the parent — and `min-h-screen`
    gives no definite height to resolve against, so the panel collapsed to the
    height of its four items and left a gap down the rest of the page.

    `sticky` follows from that: once it is exactly one viewport tall, it should
    stay put while a long bench scrolls past it.
  -->
  <Navigation layout="rail" class="shrink-0 sticky top-0 h-screen">
    <Navigation.Header>
      <span class="font-bold text-lg" title="PSU Bench">PSU</span>
      <hr class="hr mt-2" />
    </Navigation.Header>

    <Navigation.Content>
      <Navigation.Menu>
        {#each TABS as entry (entry.id)}
          <Navigation.Trigger
            onclick={() => (tab = entry.id)}
            aria-current={tab === entry.id ? 'page' : undefined}
            data-selected={tab === entry.id ? '' : undefined}
            data-testid="nav-{entry.id}"
          >
            <div class="relative">
              <NavIcon name={entry.id} />
              {#if entry.id === 'health' && unhealthy > 0}
                <!-- Count, not just a dot: "two supplies need attention" is a
                     different situation from "one does". -->
                <span
                  class="badge preset-filled-warning-500 absolute -top-2 -right-3 text-xs px-1.5"
                  data-testid="nav-health-count"
                >
                  {unhealthy}
                </span>
              {/if}
            </div>
            <Navigation.TriggerText>{entry.label}</Navigation.TriggerText>
          </Navigation.Trigger>
        {/each}
      </Navigation.Menu>
    </Navigation.Content>
  </Navigation>

  <main class="grow p-4 space-y-6 overflow-x-auto">
    <header class="flex items-baseline justify-between gap-4 pb-3 border-b border-surface-300-700">
      <h1 class="h2">PSU Bench</h1>
      {#if bench.connection !== 'open'}
        <!-- Losing the connection means losing knowledge, so it is stated
             plainly rather than tucked into a corner icon. -->
        <span class="badge preset-tonal-warning text-sm px-3 py-1" role="status" data-testid="connection">
          {bench.connection === 'connecting' ? 'Connecting…' : 'Disconnected — readings are unknown'}
        </span>
      {/if}
    </header>

    {#if tab === 'profiles'}
      <ProfilesPanel {devices} />
    {:else if tab === 'presets'}
      <PresetsPanel />
    {:else if tab === 'health'}
      <HealthPanel {devices} />
    {:else if bench.snapshot}
      <div class="space-y-8">
        {#each bench.snapshot.devices as device, index (device.slot)}
          {#if index > 0}
            <hr class="hr" />
          {/if}
          <DeviceCard {device} />
        {/each}
      </div>
    {:else}
      <p class="opacity-60">Waiting for the server…</p>
    {/if}
  </main>
</div>
