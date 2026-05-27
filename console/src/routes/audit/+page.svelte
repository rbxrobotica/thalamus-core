<script lang="ts">
  import { getClient } from '../../lib/sdk.svelte.ts';
  import EventTable from '../../components/EventTable.svelte';
  import JsonView from '../../components/JsonView.svelte';
  import ErrorAlert from '../../components/ErrorAlert.svelte';
  import type { AuditResponse } from '@rbx/thalamus-sdk';

  let auditId = $state('');
  let result: AuditResponse | null = $state(null);
  let error = $state('');
  let loading = $state(false);

  async function lookup() {
    const client = getClient();
    if (!client) {
      error = 'Not connected. Configure server URL in Settings.';
      return;
    }
    if (!auditId) {
      error = 'Audit ID is required.';
      return;
    }
    error = '';
    result = null;
    loading = true;
    try {
      result = await client.getAudit(auditId);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }
</script>

<h2 style="margin-bottom: var(--s-3);">Audit</h2>
<p style="color: var(--fg-2); margin-bottom: var(--s-3); font-size: var(--text-sm);">
  Retrieve the complete audit trail for a request by its audit ID.
</p>

<div class="card">
  <label for="auditId">Audit ID</label>
  <div style="display: flex; gap: var(--s-2); align-items: start;">
    <input id="auditId" type="text" bind:value={auditId} placeholder="Enter audit ID (UUID)" style="margin-bottom: 0;" />
    <button class="primary" onclick={lookup} disabled={loading}>
      {#if loading}
        <span class="spinner" style="margin-right: 8px;"></span>
      {:else}
        Lookup
      {/if}
    </button>
  </div>
</div>

{#if error}
  <ErrorAlert message={error} />
{/if}

{#if result}
  <div class="card">
    <h3>Audit: {result.audit_id}</h3>
    {#if result.events.length > 0}
      <EventTable events={result.events} />
    {:else}
      <p style="color: var(--fg-3);">No events recorded.</p>
    {/if}
    <div style="margin-top: var(--s-3);">
      <h3>Full Response</h3>
      <JsonView data={result} />
    </div>
  </div>
{/if}
