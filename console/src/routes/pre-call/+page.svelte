<script lang="ts">
  import { getClient } from '../../lib/sdk.svelte.ts';
  import RequestForm from '../../components/RequestForm.svelte';
  import DecisionBadge from '../../components/DecisionBadge.svelte';
  import JsonView from '../../components/JsonView.svelte';
  import ErrorAlert from '../../components/ErrorAlert.svelte';
  import type { DecideRequest, PreCallResponse } from '@rbx/thalamus-sdk';

  let result: PreCallResponse | null = $state(null);
  let error = $state('');
  let loading = $state(false);

  async function handleRequest(req: DecideRequest) {
    const client = getClient();
    if (!client) {
      error = 'Not connected. Configure server URL in Settings.';
      return;
    }
    error = '';
    result = null;
    loading = true;
    try {
      result = await client.preCall(req);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }
</script>

<h2 style="margin-bottom: var(--s-3);">Pre-Call</h2>
<p style="color: var(--fg-2); margin-bottom: var(--s-3); font-size: var(--text-sm);">
  Policy gate with execution envelope. Returns trace/audit IDs and backend parameters.
</p>

<RequestForm onsubmit={handleRequest} {loading} />

{#if error}
  <ErrorAlert message={error} />
{/if}

{#if result}
  <div class="card">
    <h3>Result</h3>
    <div style="margin-bottom: var(--s-3);">
      <DecisionBadge decision={result.decision} />
      <span style="margin-left: var(--s-2); color: var(--fg-2); font-size: var(--text-sm);">
        Policy: {result.policy_id}
      </span>
    </div>
    <div class="row" style="margin-bottom: var(--s-3);">
      <div>
        <span class="field-label">Trace ID</span>
        <code style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--cyan-muted);">{result.trace_id}</code>
      </div>
      <div>
        <span class="field-label">Audit ID</span>
        <code style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--cyan-muted);">{result.audit_id}</code>
      </div>
    </div>
    {#if result.envelope}
      <div class="card" style="background: var(--bg-3);">
        <h3>Envelope</h3>
        <JsonView data={result.envelope} />
      </div>
    {/if}
    {#if result.review_reason}
      <p style="color: var(--warn); margin-top: var(--s-2);">Review: {result.review_reason}</p>
    {/if}
    <div style="margin-top: var(--s-3);">
      <h3>Full Response</h3>
      <JsonView data={result} />
    </div>
  </div>
{/if}
