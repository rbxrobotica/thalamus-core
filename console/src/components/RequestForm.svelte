<script lang="ts">
  import type { DecideRequest } from '@rbx/thalamus-sdk';

  let {
    onsubmit,
    loading = false,
  }: {
    onsubmit: (req: DecideRequest) => void;
    loading?: boolean;
  } = $props();

  let tenant = $state('rbx');
  let product = $state('robson');
  let user = $state('agent-1');
  let workflow = $state('trade-analysis');
  let intent = $state('market-summary');
  let prompt = $state('Analyze the current market conditions for BTC/EUR and provide a summary.');
  let backendId = $state('gpt-4o');
  let backendType = $state('Model');
  let maxTokens = $state('4096');
  let maxLatency = $state('30000');

  function submit() {
    const req: DecideRequest = {
      tenant,
      product,
      user,
      workflow,
      intent,
      prompt,
    };
    if (backendId) {
      req.requested_backend = { id: backendId, backend_type: backendType };
    }
    if (maxTokens || maxLatency) {
      req.budget_hint = {
        max_tokens: maxTokens ? Number(maxTokens) : undefined,
        max_latency_ms: maxLatency ? Number(maxLatency) : undefined,
      };
    }
    onsubmit(req);
  }
</script>

<div class="card">
  <div class="row">
    <div>
      <label for="tenant">Tenant</label>
      <input id="tenant" bind:value={tenant} />
    </div>
    <div>
      <label for="product">Product</label>
      <input id="product" bind:value={product} />
    </div>
  </div>
  <div class="row">
    <div>
      <label for="user">User</label>
      <input id="user" bind:value={user} />
    </div>
    <div>
      <label for="workflow">Workflow</label>
      <input id="workflow" bind:value={workflow} />
    </div>
  </div>
  <div class="row">
    <div>
      <label for="intent">Intent</label>
      <input id="intent" bind:value={intent} />
    </div>
  </div>

  <label for="prompt">Prompt</label>
  <textarea id="prompt" bind:value={prompt} rows="3"></textarea>

  <h3 style="margin-top: var(--s-3);">Backend</h3>
  <div class="row">
    <div>
      <label for="backendId">Backend ID</label>
      <input id="backendId" bind:value={backendId} />
    </div>
    <div>
      <label for="backendType">Type</label>
      <select id="backendType" bind:value={backendType}>
        <option>Model</option>
        <option>Tool</option>
        <option>McpServer</option>
        <option>A2AAgent</option>
      </select>
    </div>
  </div>

  <h3 style="margin-top: var(--s-3);">Budget</h3>
  <div class="row">
    <div>
      <label for="maxTokens">Max Tokens</label>
      <input id="maxTokens" type="number" bind:value={maxTokens} />
    </div>
    <div>
      <label for="maxLatency">Max Latency (ms)</label>
      <input id="maxLatency" type="number" bind:value={maxLatency} />
    </div>
  </div>

  <button class="primary" onclick={submit} disabled={loading}>
    {#if loading}
      <span class="spinner" style="margin-right: 8px;"></span> Sending...
    {:else}
      Send Request
    {/if}
  </button>
</div>
