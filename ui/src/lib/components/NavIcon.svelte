<script lang="ts">
  /**
   * Inline SVG rather than an icon package: four glyphs do not justify a
   * dependency, and the rail needs them to sit alongside a text label anyway —
   * an icon-only rail would make the sections guessable rather than readable.
   */
  let { name }: { name: 'bench' | 'profiles' | 'presets' | 'health' } = $props();
</script>

<svg
  class="size-5"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'bench'}
    <!-- A supply front panel: display over three channel outputs. -->
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M6 8h6" />
    <circle cx="8" cy="15" r="1.2" />
    <circle cx="12" cy="15" r="1.2" />
    <circle cx="16" cy="15" r="1.2" />
  {:else if name === 'profiles'}
    <!-- Stacked rails coming up in sequence. -->
    <path d="M4 18h4l2-9 2 13 2-8h6" />
  {:else if name === 'presets'}
    <path d="M4 6h16M4 12h16M4 18h10" />
  {:else}
    <!-- Health: a pulse. -->
    <path d="M3 12h4l2-5 3 10 2-5h7" />
  {/if}
</svg>
