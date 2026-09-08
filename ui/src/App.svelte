<script lang="ts">
  import { onMount } from 'svelte';
  import { Bench } from './lib/bench.svelte';
  import DeviceCard from './lib/components/DeviceCard.svelte';

  const bench = new Bench();

  // `onMount` rather than `$effect`: the connection must be opened exactly once
  // for the life of the component. An effect would re-run if anything it
  // touched became reactive, silently turning every state update into a
  // reconnect.
  onMount(() => {
    bench.start();
    return () => bench.stop();
  });
</script>

<main class="mx-auto max-w-7xl p-4 space-y-6">
  <header class="flex items-baseline justify-between gap-4">
    <h1 class="h2">PSU Bench</h1>
    {#if bench.connection !== 'open'}
      <!-- Losing the connection means losing knowledge, so it is stated
           plainly rather than tucked into a corner icon. -->
      <span class="badge preset-tonal-warning" role="status" data-testid="connection">
        {bench.connection === 'connecting' ? 'Connecting…' : 'Disconnected — readings are unknown'}
      </span>
    {/if}
  </header>

  {#if bench.snapshot}
    <div class="space-y-8">
      {#each bench.snapshot.devices as device (device.slot)}
        <DeviceCard {device} />
      {/each}
    </div>
  {:else}
    <p class="opacity-60">Waiting for the server…</p>
  {/if}
</main>
