<script lang="ts">
  import { getConfig, setConfig } from '../lib/config.svelte.ts';

  let baseUrl = $state(getConfig().baseUrl);
  let authHeader = $state(getConfig().authHeader ?? '');

  function save() {
    setConfig({
      baseUrl: baseUrl.replace(/\/+$/, ''),
      authHeader: authHeader || undefined,
    });
  }
</script>

<div class="card config-panel">
  <h3>Connection Settings</h3>
  <div class="row">
    <div>
      <label for="baseUrl">Base URL</label>
      <input id="baseUrl" type="text" bind:value={baseUrl} placeholder="http://localhost:3000" />
    </div>
    <div>
      <label for="authHeader">Auth Token (optional)</label>
      <input id="authHeader" type="password" bind:value={authHeader} placeholder="Bearer ..." />
    </div>
  </div>
  <button class="primary" onclick={save}>Save</button>
</div>

<style>
  .config-panel {
    margin-bottom: var(--s-3);
  }
</style>
