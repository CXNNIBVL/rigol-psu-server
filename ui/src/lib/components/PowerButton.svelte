<script lang="ts">
  /**
   * One control carrying one bit: is this rail energised.
   *
   * It replaces a segmented control sitting beside a status dot, which said the
   * same thing twice — once as a reading and once as a choice.
   *
   * Lit green when energised, neutral when not. Deliberately *not* red for off:
   * red on a power supply means a fault or a trip, and it is already this
   * interface's destructive colour. A rail that is simply off is the safe
   * resting state, so a bench at rest would otherwise show six red buttons and
   * teach everyone to ignore the colour that matters.
   *
   * `aria-pressed` rather than a label that flips between "On" and "Off": a
   * toggle button that renames itself is ambiguous about whether it reports the
   * current state or the action a press performs. The pressed state reports;
   * the name stays constant.
   */
  let {
    on,
    disabled = false,
    busy = false,
    label,
    onToggle,
  }: {
    /** `null` when the rail's state is not known. */
    on: boolean | null;
    disabled?: boolean;
    busy?: boolean;
    label: string;
    onToggle: (next: boolean) => void;
  } = $props();

  const state = $derived(on === null ? 'unknown' : on ? 'on' : 'off');

  // Whole literal strings — Tailwind cannot see a class assembled at runtime.
  const LOOK = {
    on: 'preset-filled-success-500',
    // Outlined rather than filled: a tonal fill on this background was so close
    // to invisible that the control stopped reading as one. It still has to
    // look pressable while staying quieter than a live rail.
    off: 'preset-outlined-surface-200-800 opacity-70 hover:opacity-100 hover:preset-tonal',
    unknown: 'preset-outlined-warning-500 text-warning-500',
  } as const;

  const TITLE = {
    on: 'Energised — press to switch off',
    off: 'Off — press to energise',
    unknown: 'State unknown',
  } as const;
</script>

<button
  type="button"
  class="btn-icon btn-icon-lg {LOOK[state]}"
  aria-label={label}
  aria-pressed={on === null ? 'mixed' : on}
  title={TITLE[state]}
  disabled={disabled || busy || on === null}
  data-state={state}
  onclick={() => onToggle(!on)}
>
  <!-- IEC 5009: the power symbol every instrument on the bench already uses. -->
  <svg
    class="size-5"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    aria-hidden="true"
  >
    <path d="M12 3.5v8" />
    <path d="M7.2 6.6a7 7 0 1 0 9.6 0" />
  </svg>
</button>
