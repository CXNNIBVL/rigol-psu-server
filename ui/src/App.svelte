<script lang="ts">
  import { onMount } from 'svelte';
  import { Navigation } from '@skeletonlabs/skeleton-svelte';
  import { Bench } from './lib/bench.svelte';
  import { summarise } from './lib/bench-summary';
  import BenchView from './lib/components/BenchView.svelte';
  import ProfilesPanel from './lib/components/ProfilesPanel.svelte';
  import PresetsPanel from './lib/components/PresetsPanel.svelte';
  import NavIcon from './lib/components/NavIcon.svelte';

  const bench = new Bench();

  type Tab = 'bench' | 'profiles' | 'presets';
  let tab = $state<Tab>('bench');

  const TABS: Array<{ id: Tab; label: string }> = [
    { id: 'bench', label: 'Bench' },
    { id: 'profiles', label: 'Profiles' },
    { id: 'presets', label: 'Settings' },
  ];

  const devices = $derived(bench.snapshot?.devices ?? []);

  // Carried on Bench, which is where the supply in question lives. Health used
  // to hold this, but its only unique content was two serial numbers — now a
  // disclosure on the supply itself.
  const needAttention = $derived(summarise(devices).needAttention);

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
    `h-screen` rather than leaning on the flex container: the rail's own
    stylesheet sets `height: 100%`, which resolves against the parent — and
    `min-h-screen` gives no definite height to resolve against, so the panel
    collapsed to the height of its items. `sticky` follows from that.
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
            data-testid="nav-{entry.id}"
          >
            <div class="relative">
              <NavIcon name={entry.id} />
              {#if entry.id === 'bench' && needAttention > 0}
                <!-- A count, not a dot: "two supplies need attention" is a
                     different situation from "one does". -->
                <span
                  class="badge preset-filled-warning-500 absolute -top-2 -right-3 text-xs px-1.5"
                  data-testid="nav-attention-count"
                >
                  {needAttention}
                </span>
              {/if}
            </div>
            <Navigation.TriggerText>{entry.label}</Navigation.TriggerText>
          </Navigation.Trigger>
        {/each}
      </Navigation.Menu>
    </Navigation.Content>

    <Navigation.Footer>
      <!-- Connection is chrome, not content: it belongs with the rest of the
           frame rather than in whichever view happens to be open. -->
      {#if bench.connection !== 'open'}
        <span
          class="badge preset-tonal-warning text-xs"
          role="status"
          data-testid="connection"
          title={bench.connection === 'connecting'
            ? 'Connecting to the server'
            : 'Disconnected — readings are unknown'}
        >
          {bench.connection === 'connecting' ? 'Connecting' : 'Offline'}
        </span>
      {/if}
    </Navigation.Footer>
  </Navigation>

  <main class="grow p-4 space-y-6 overflow-x-auto">
    {#if tab === 'profiles'}
      <ProfilesPanel {devices} />
    {:else if tab === 'presets'}
      <PresetsPanel />
    {:else if bench.snapshot}
      <BenchView {devices} connected={bench.connection === 'open'} />
    {:else}
      <p class="opacity-60">Waiting for the server…</p>
    {/if}
  </main>
</div>
