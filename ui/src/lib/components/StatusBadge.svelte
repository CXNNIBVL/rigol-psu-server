<script lang="ts">
  import { TONAL, type StatusTone } from '../format';

  let { status }: { status: StatusTone } = $props();
</script>

<!-- Colour reinforces; the label always states the condition in words. -->
<span class="badge {TONAL[status.tone]}" title={status.detail ?? status.label}>
  {status.label}
</span>
