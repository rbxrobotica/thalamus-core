<script lang="ts">
  import type { AuditEvent } from '@rbx/thalamus-sdk';

  let { events }: { events: AuditEvent[] } = $props();
</script>

<table class="events-table">
  <thead>
    <tr>
      <th>Kind</th>
      <th>Trace ID</th>
      <th>Timestamp</th>
      <th>Details</th>
    </tr>
  </thead>
  <tbody>
    {#each events as event}
      <tr>
        <td>{event.kind}</td>
        <td class="mono">{event.trace_id}</td>
        <td>{event.timestamp}</td>
        <td><pre>{JSON.stringify(event.details, null, 2)}</pre></td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
</style>
