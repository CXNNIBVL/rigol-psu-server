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
   * While a change is in flight it keeps showing the state that is still true.
   * It deliberately does not preview the state you asked for: a rail that is
   * still live must never look off, which is the direction of this mistake that
   * gets someone hurt. The ring says only that an answer is on its way.
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
    pending = false,
    label,
    onToggle,
  }: {
    /** `null` when the rail's state is not known. */
    on: boolean | null;
    disabled?: boolean;
    busy?: boolean;
    /**
     * The command has gone to the instrument and no snapshot has confirmed it
     * yet. Distinct from `busy`, which only covers the request: the truth
     * arrives with the next 2 Hz snapshot, well after the response.
     */
    pending?: boolean;
    label: string;
    onToggle: (next: boolean) => void;
  } = $props();

  const state = $derived(on === null ? 'unknown' : on ? 'on' : 'off');

  // One marker for the whole journey. `busy` is the request and `pending` is
  // the wait for the snapshot that confirms it; from the operator's side those
  // are one question — has this landed — and over a VPN either half can be the
  // slow one.
  const inFlight = $derived(busy || pending);

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

  // The bench is at the end of a wire. Saying so beats a control that goes dead
  // and looks exactly like one that is unavailable.
  const title = $derived(inFlight ? 'Waiting for the supply to confirm' : TITLE[state]);
</script>

<!--
  `disabled` is for what cannot be done; in flight is something being done.
  Using one attribute for both dimmed the button exactly like an unavailable
  one, which is the ambiguity this state exists to remove. A command in flight
  is announced with aria-disabled and guarded in the handler instead, which also
  keeps the focus a real disable would have thrown away.
-->
<button
  type="button"
  class="btn-icon btn-icon-lg relative {LOOK[state]}"
  aria-label={label}
  aria-pressed={on === null ? 'mixed' : on}
  title={title}
  disabled={disabled || on === null}
  aria-disabled={inFlight ? 'true' : undefined}
  data-state={state}
  data-pending={inFlight ? 'true' : undefined}
  onclick={() => !inFlight && onToggle(!on)}
>
  {#if inFlight}
    <span class="pending-ring" data-testid="power-pending" aria-hidden="true"></span>
    <span class="sr-only">Waiting for the supply to confirm</span>
  {/if}
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
