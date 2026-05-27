<script lang="ts">
  import { getClient } from '../../lib/sdk.svelte.ts';
  import RequestForm from '../../components/RequestForm.svelte';
  import DecisionBadge from '../../components/DecisionBadge.svelte';
  import JsonView from '../../components/JsonView.svelte';
  import ErrorAlert from '../../components/ErrorAlert.svelte';
  import type { DecideRequest, DecideResponse } from '@rbx/thalamus-sdk';

  let result: DecideResponse | null = $state(null);
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
      result = await client.decide(req);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }
</script>

<h2 style="margin-bottom: var(--s-3);">Decide</h2>
<p style="color: var(--fg-2); margin-bottom: var(--s-3); font-size: var(--text-sm);">
  Quick policy check without backend execution.
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
    {#if result.reason}
      <p style="color: var(--err); margin-bottom: var(--s-2);">Reason: {result.reason}</p>
    {/if}
    {#if result.review_reason}
      <p style="color: var(--warn); margin-bottom: var(--s-2);">Review: {result.review_reason}</p>
    {/if}
    <JsonView data={result} />
  </div>
{/if}
