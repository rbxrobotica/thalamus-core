<script lang="ts">
  import { getClient } from '../../lib/sdk.svelte.ts';
  import DecisionBadge from '../../components/DecisionBadge.svelte';
  import JsonView from '../../components/JsonView.svelte';
  import ErrorAlert from '../../components/ErrorAlert.svelte';
  import type { PostCallRequest, PostCallResponse } from '@rbx/thalamus-sdk';

  let auditId = $state('');
  let content = $state('');
  let tokensUsed = $state('');
  let latencyMs = $state('');

  let result: PostCallResponse | null = $state(null);
  let error = $state('');
  let loading = $state(false);

  async function submit() {
    const client = getClient();
    if (!client) {
      error = 'Not connected. Configure server URL in Settings.';
      return;
    }
    if (!auditId || !content) {
      error = 'Audit ID and Content are required.';
      return;
    }
    error = '';
    result = null;
    loading = true;
    try {
      const req: PostCallRequest = {
        audit_id: auditId,
        content,
      };
      if (tokensUsed) req.tokens_used = Number(tokensUsed);
      if (latencyMs) req.latency_ms = Number(latencyMs);
      result = await client.postCall(req);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }
</script>

<h2 style="margin-bottom: var(--s-3);">Post-Call</h2>
<p style="color: var(--fg-2); margin-bottom: var(--s-3); font-size: var(--text-sm);">
  Submit execution feedback for a prior pre-call or full-call. Use the audit_id from a previous response.
</p>

<div class="card">
  <label for="auditId">Audit ID</label>
  <input id="auditId" type="text" bind:value={auditId} placeholder="UUID from pre-call or call response" />

  <label for="content">Content</label>
  <textarea id="content" bind:value={content} rows="4" placeholder="Backend response content"></textarea>

  <div class="row">
    <div>
      <label for="tokensUsed">Tokens Used (optional)</label>
      <input id="tokensUsed" type="number" bind:value={tokensUsed} />
    </div>
    <div>
      <label for="latencyMs">Latency (ms, optional)</label>
      <input id="latencyMs" type="number" bind:value={latencyMs} />
    </div>
  </div>

  <button class="primary" onclick={submit} disabled={loading}>
    {#if loading}
      <span class="spinner" style="margin-right: 8px;"></span> Submitting...
    {:else}
      Submit Feedback
    {/if}
  </button>
</div>

{#if error}
  <ErrorAlert message={error} />
{/if}

{#if result}
  <div class="card">
    <h3>Result</h3>
    <div class="row" style="margin-bottom: var(--s-3);">
      <div>
        <label>Status</label>
        <span class="badge badge-allow">{result.status}</span>
      </div>
      <div>
        <label>Risk Class</label>
        <span class="badge" style="background: rgba(0, 255, 255, 0.08); color: var(--cyan-muted);">{result.risk_class}</span>
      </div>
    </div>
    <div class="row" style="margin-bottom: var(--s-3);">
      <div>
        <label>Executable by Agent</label>
        <span>{result.executable_by_agent ? 'Yes' : 'No'}</span>
      </div>
      <div>
        <label>Schema Valid</label>
        <span>{result.schema_valid ? 'Yes' : 'No'}</span>
      </div>
    </div>
    <JsonView data={result} />
  </div>
{/if}
