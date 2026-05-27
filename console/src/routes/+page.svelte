<script lang="ts">
  import { getClient } from '../lib/sdk.svelte.ts';

  let connected = $derived(getClient() !== null);
</script>

<div class="card">
  <h2>Thalamus Console</h2>
  <p>
    Admin interface for the Thalamus semantic control layer. Inspect policy decisions, run pre/post-call
    validations, and review audit trails.
  </p>

  {#if !connected}
    <div class="error-alert" style="margin-top: var(--s-3);">
      Not connected. Open <strong>Settings</strong> and configure the Thalamus server URL.
    </div>
  {:else}
    <div style="margin-top: var(--s-3);">
      <p style="color: var(--ok);">Connected.</p>
    </div>
  {/if}
</div>

<div class="card">
  <h3>Endpoints</h3>
  <ul class="endpoint-list">
    <li><a href="/decide">Decide</a> — Quick policy check without backend execution</li>
    <li><a href="/pre-call">Pre-Call</a> — Policy gate with execution envelope</li>
    <li><a href="/call">Full Call</a> — Complete decision + backend + post-call flow</li>
    <li><a href="/post-call">Post-Call</a> — Submit execution feedback</li>
    <li><a href="/audit">Audit</a> — Retrieve audit trail by ID</li>
  </ul>
</div>

<style>
  .endpoint-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }

  .endpoint-list li {
    font-size: var(--text-sm);
  }

  .endpoint-list a {
    font-weight: 600;
    color: var(--cyan-brand);
    margin-right: var(--s-2);
  }
</style>
