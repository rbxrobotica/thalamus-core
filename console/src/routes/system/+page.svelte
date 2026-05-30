<script lang="ts">
  import DecisionBadge from '../../components/DecisionBadge.svelte';
  import JsonView from '../../components/JsonView.svelte';
  import Unavailable from '../../components/Unavailable.svelte';
  import { getConfig } from '../../lib/config.svelte.ts';
  import { getClient } from '../../lib/sdk.svelte.ts';

  let cfg = $derived(getConfig());
  let connected = $derived(getClient() !== null);
  let hasObservabilityConfig = $derived(Boolean(cfg.observabilityUrl || cfg.observabilityToken));
  let hasMemoryConfig = $derived(Boolean(cfg.memoryUrl));
  let tracePath = $derived(
    hasObservabilityConfig ? 'HTTP exporter -> rbx-observability (ADR-0300)' : 'Langfuse default',
  );
  let topology = $derived({
    leanCore: {
      thalamus: cfg.baseUrl || null,
      connected,
    },
    observability: {
      tracePath,
      urlConfigured: Boolean(cfg.observabilityUrl),
      tokenConfigured: Boolean(cfg.observabilityToken),
      adr: 'ADR-0300',
    },
    serviceDiscovery: {
      owner: 'rbx-maestro',
      url: cfg.maestroUrl || null,
      adr: 'ADR-0400',
    },
    memory: {
      owner: 'rbx-memory',
      url: cfg.memoryUrl || null,
      configured: hasMemoryConfig,
      adr: 'ADR-0200',
    },
    anchor: 'ADR-0010',
  });
</script>

<h2 style="margin-bottom: var(--s-3);">System</h2>
<p style="color: var(--fg-2); margin-bottom: var(--s-3); font-size: var(--text-sm);">
  Read-only view of the lean Thalamus topology after ADR-0010.
</p>

{#if !connected}
  <Unavailable
    title="Thalamus not configured"
    description="Open Settings and configure the Thalamus server URL to connect the lean control plane."
  />
{/if}

<div class="system-grid">
  <section class="card system-card">
    <div class="system-card-header">
      <h3>Lean Core</h3>
      <DecisionBadge decision={connected ? 'Allow' : 'Deny'} />
    </div>
    <span class="field-label">Thalamus</span>
    <p>{cfg.baseUrl || 'Not configured'}</p>
    <p class="muted">Semantic routing and BackendPort remain in Thalamus.</p>
  </section>

  <section class="card system-card">
    <div class="system-card-header">
      <h3>Observability</h3>
      <span class="badge" class:badge-review={hasObservabilityConfig}>{hasObservabilityConfig ? 'ADR-0300' : 'Default'}</span>
    </div>
    <span class="field-label">Trace Path</span>
    <p>{tracePath}</p>
    <p class="muted">
      {cfg.observabilityUrl || 'No RBX observability URL configured.'}
      Token: {cfg.observabilityToken ? 'configured' : 'not configured'}.
    </p>
  </section>

  <section class="card system-card">
    <div class="system-card-header">
      <h3>Service Discovery</h3>
      <span class="badge badge-review">ADR-0400</span>
    </div>
    <span class="field-label">Backend Registry</span>
    <p>Backends are discovered via rbx-maestro.</p>
    <p class="muted">{cfg.maestroUrl || 'No maestro URL configured in console settings.'}</p>
  </section>

  <section class="card system-card">
    <div class="system-card-header">
      <h3>Memory</h3>
      <span class="badge" class:badge-allow={hasMemoryConfig}>{hasMemoryConfig ? 'Configured' : 'External'}</span>
    </div>
    <span class="field-label">Dependency</span>
    <p>rbx-memory is an external dependency.</p>
    <p class="muted">{cfg.memoryUrl || 'No memory URL configured in console settings.'}</p>
  </section>
</div>

<div class="card">
  <h3>Topology Snapshot</h3>
  <JsonView data={topology} />
</div>

<style>
  .system-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--s-3);
    margin-bottom: var(--s-3);
  }

  .system-card {
    margin-bottom: 0;
  }

  .system-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-2);
    margin-bottom: var(--s-2);
  }

  .system-card p {
    color: var(--fg-1);
    font-size: var(--text-sm);
    overflow-wrap: anywhere;
  }

  .system-card .muted {
    color: var(--fg-3);
    margin-top: var(--s-2);
  }

  @media (max-width: 760px) {
    .system-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
