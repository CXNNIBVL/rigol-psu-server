<script lang="ts">
  import type { DeviceView } from '../types';
  import { deviceStatus, TONAL } from '../format';
  import StatusBadge from './StatusBadge.svelte';

  let { devices }: { devices: DeviceView[] } = $props();

  const rows = $derived(
    devices.map((device) => ({
      device,
      status: deviceStatus(device),
      // A channel left energised outside its limits at startup. Surfaced here
      // as well as on the dashboard, because it is a condition someone has to
      // go and resolve rather than something to notice in passing.
      violations: device.channels.filter((c) => c.violation !== null),
    })),
  );

  const unhealthy = $derived(
    rows.filter((r) => r.device.state !== 'online' || r.violations.length > 0),
  );
</script>

<section class="space-y-4" data-testid="health-panel">
  <h2 class="h3">Supply health</h2>

  {#if unhealthy.length === 0}
    <p class="preset-tonal-success p-3 rounded text-sm" data-testid="all-well">
      All {devices.length} supplies are online, and no channel is outside its limits.
    </p>
  {/if}

  <div class="space-y-3">
    {#each rows as row (row.device.slot)}
      <article
        class="card preset-outlined-surface-200-800 p-4 space-y-2"
        data-testid="health-{row.device.slot}"
      >
        <header class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-3">
            <h3 class="h4">{row.device.label}</h3>
            <StatusBadge status={row.status} size="lg" />
            {#if row.device.front_panel_locked}
              <span class="badge preset-tonal-surface text-sm px-3 py-1">Panel locked</span>
            {/if}
          </div>
          <p class="text-xs opacity-60">{row.device.slot} · {row.device.device}</p>
        </header>

        <dl class="grid gap-x-6 gap-y-1 text-sm sm:grid-cols-2">
          <div class="flex gap-2">
            <dt class="opacity-60">Configured serial</dt>
            <dd class="reading">{row.device.expected_serial}</dd>
          </div>
          <div class="flex gap-2">
            <dt class="opacity-60">Reported serial</dt>
            <!-- Unknown, not blank: we have not heard from the instrument. -->
            <dd class="reading">{row.device.reported_serial ?? 'Unknown'}</dd>
          </div>
        </dl>

        {#if row.status.detail}
          <p class="{TONAL[row.status.tone]} p-3 rounded text-sm" data-testid="health-detail-{row.device.slot}">
            {row.status.detail}
          </p>
        {/if}

        {#each row.violations as channel (channel.id)}
          <p class="preset-tonal-error p-3 rounded text-sm" data-testid="health-violation-{row.device.slot}-{channel.id}">
            <strong>{channel.id}</strong> was found live outside its configured limits and
            has been left energised on purpose — a restart must not end a running
            experiment. Writes to it are refused until someone resolves it.
          </p>
        {/each}
      </article>
    {/each}
  </div>
</section>
