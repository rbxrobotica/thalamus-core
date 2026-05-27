<script lang="ts">
  import '../app.css';
  import Nav from '../components/Nav.svelte';
  import ConfigPanel from '../components/ConfigPanel.svelte';
  import { getConfig } from '../lib/config.svelte.ts';

  let { children } = $props();
  let showConfig = $state(false);
  let cfg = $derived(getConfig());

  function toggleConfig() {
    showConfig = !showConfig;
  }
</script>

<div class="layout">
  <header>
    <h1>Thalamus Console</h1>
    <div class="header-actions">
      <span class="conn-status" class:connected={cfg.baseUrl !== ''}>
        {cfg.baseUrl ? cfg.baseUrl : 'Not connected'}
      </span>
      <button class="primary" onclick={toggleConfig}>
        {showConfig ? 'Close' : 'Settings'}
      </button>
    </div>
  </header>

  {#if showConfig}
    <ConfigPanel />
  {/if}

  <div class="body">
    <Nav />
    <main>
      {@render children()}
    </main>
  </div>
</div>

<style>
  .layout {
    max-width: 1200px;
    margin: 0 auto;
    padding: var(--s-3) var(--s-4);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--s-3);
    padding-bottom: var(--s-3);
    border-bottom: 1px solid var(--bg-4);
  }

  header h1 {
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--cyan-brand);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--s-3);
  }

  .conn-status {
    font-size: var(--text-sm);
    color: var(--fg-3);
  }

  .conn-status.connected {
    color: var(--ok);
  }

  button.primary {
    padding: 6px 14px;
    font-size: var(--text-sm);
  }

  .body {
    display: flex;
    gap: var(--s-4);
  }

  main {
    flex: 1;
    min-width: 0;
  }
</style>
