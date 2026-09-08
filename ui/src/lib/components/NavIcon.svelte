<script lang="ts">
  /**
   * Inline SVG rather than an icon package: four glyphs do not justify a
   * dependency, and the rail shows them alongside a text label anyway — an
   * icon-only rail would make the sections guessable rather than readable.
   *
   * The four are deliberately different *shapes*, not four variations on a
   * line: a panel, a chip, sliders, a pulse. Two waveform glyphs sitting two
   * rows apart read as the same icon at a glance, which is worse than no icon.
   */
  let { name }: { name: 'bench' | 'profiles' | 'presets' | 'health' } = $props();
</script>

<svg
  class="size-6"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'bench'}
    <!-- A supply front panel: display above three output terminals. -->
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M6 8h6" />
    <circle cx="8" cy="15" r="1.2" />
    <circle cx="12" cy="15" r="1.2" />
    <circle cx="16" cy="15" r="1.2" />
  {:else if name === 'profiles'}
    <!-- A chip with pins: the device a profile describes and powers. -->
    <rect x="7" y="7" width="10" height="10" rx="1.5" />
    <path d="M10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4" />
  {:else if name === 'presets'}
    <!-- Sliders: a bundle of settings, rather than a generic list. -->
    <path d="M4 8h8M17 8h3M4 16h3M12 16h8" />
    <circle cx="14.5" cy="8" r="2.2" />
    <circle cx="9.5" cy="16" r="2.2" />
  {:else}
    <!-- Health: a pulse. -->
    <path d="M3 12h4l2-5 3 10 2-5h7" />
  {/if}
</svg>
