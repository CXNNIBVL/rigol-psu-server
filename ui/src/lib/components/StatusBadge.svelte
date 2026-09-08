<script lang="ts">
  import { TONAL, type StatusTone } from '../format';

  let { status, size = 'sm' }: { status: StatusTone; size?: 'sm' | 'lg' } = $props();

  // Literal strings: Tailwind never sees a class assembled at runtime.
  const SIZES = {
    sm: 'text-xs',
    lg: 'text-sm px-3 py-1 font-semibold',
  } as const;
</script>

<!-- Colour reinforces; the label always states the condition in words. -->
<span class="badge {TONAL[status.tone]} {SIZES[size]}" title={status.detail ?? status.label}>
  {status.label}
</span>
