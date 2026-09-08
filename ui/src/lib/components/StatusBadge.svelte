<script lang="ts">
  import type { StatusTone } from '../format';

  let { status }: { status: StatusTone } = $props();

  // Colour is a reinforcement, never the signal itself: the label always says
  // the state in words.
  const classes: Record<StatusTone['tone'], string> = {
    success: 'preset-tonal-success',
    warning: 'preset-tonal-warning',
    error: 'preset-tonal-error',
    surface: 'preset-tonal-surface',
  };
</script>

<span class="badge {classes[status.tone]}" title={status.detail ?? status.label}>
  {status.label}
</span>
