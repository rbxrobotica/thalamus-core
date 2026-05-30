<script lang="ts">
  import { getClient } from '../../lib/sdk.svelte.ts';
  import RequestForm from '../../components/RequestForm.svelte';
  import DecisionBadge from '../../components/DecisionBadge.svelte';
  import JsonView from '../../components/JsonView.svelte';
  import ErrorAlert from '../../components/ErrorAlert.svelte';
  import type { DecideRequest, FullCallResponse } from '@rbx/thalamus-sdk';

  let result: FullCallResponse | null = $state(null);
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
      result = await client.call(req);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }
</script>

<h2 style="margin-bottom: var(--s-3);">Full Call</h2>
<p style="color: var(--fg-2); margin-bottom: var(--s-3); font-size: var(--text-sm);">
  Complete flow: policy decision → backend execution → post-call validation. Returns decision, post-call
  results, and backend content.
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
    </div>

    {#if result.post_call}
      <div class="card" style="background: var(--bg-3); margin-bottom: var(--s-3);">
        <h3>Post-Call</h3>
        <div class="row">
          <div>
            <span class="field-label">Status</span>
            <span class="badge badge-allow">{result.post_call.status}</span>
          </div>
          <div>
            <span class="field-label">Risk Class</span>
            <span class="badge" style="background: rgba(0, 255, 255, 0.08); color: var(--cyan-muted);">{result.post_call.risk_class}</span>
          </div>
        </div>
        <div class="row">
          <div>
            <span class="field-label">Executable by Agent</span>
            <span>{result.post_call.executable_by_agent ? 'Yes' : 'No'}</span>
          </div>
          <div>
            <span class="field-label">Schema Valid</span>
            <span>{result.post_call.schema_valid ? 'Yes' : 'No'}</span>
          </div>
        </div>
        {#if result.post_call.audit_id}
          <div>
            <span class="field-label">Audit ID</span>
            <code style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--cyan-muted);">{result.post_call.audit_id}</code>
          </div>
        {/if}
      </div>
    {/if}

    {#if result.backend_content}
      <div class="card" style="background: var(--bg-3); margin-bottom: var(--s-3);">
        <h3>Backend Content</h3>
        <pre class="result">{result.backend_content}</pre>
      </div>
    {/if}

    <div style="margin-top: var(--s-3);">
      <h3>Full Response</h3>
      <JsonView data={result} />
    </div>
  </div>
{/if}
