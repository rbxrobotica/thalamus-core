<script lang="ts">
  import { getConfig, setConfig } from '../lib/config.svelte.ts';

  let baseUrl = $state(getConfig().baseUrl);
  let authHeader = $state(getConfig().authHeader ?? '');
  let observabilityUrl = $state(getConfig().observabilityUrl ?? '');
  let observabilityToken = $state(getConfig().observabilityToken ?? '');
  let memoryUrl = $state(getConfig().memoryUrl ?? '');
  let maestroUrl = $state(getConfig().maestroUrl ?? '');

  function save() {
    setConfig({
      baseUrl: baseUrl.replace(/\/+$/, ''),
      authHeader: authHeader || undefined,
      observabilityUrl: observabilityUrl.replace(/\/+$/, '') || undefined,
      observabilityToken: observabilityToken || undefined,
      memoryUrl: memoryUrl.replace(/\/+$/, '') || undefined,
      maestroUrl: maestroUrl.replace(/\/+$/, '') || undefined,
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

<div class="card config-panel">
  <h3>System Dependencies</h3>
  <div class="row">
    <div>
      <label for="observabilityUrl">RBX Observability URL (optional)</label>
      <input id="observabilityUrl" type="text" bind:value={observabilityUrl} placeholder="http://localhost:8080" />
    </div>
    <div>
      <label for="observabilityToken">RBX Observability Token (optional)</label>
      <input id="observabilityToken" type="password" bind:value={observabilityToken} placeholder="Bearer token configured on server" />
    </div>
  </div>
  <div class="row">
    <div>
      <label for="memoryUrl">RBX Memory URL (optional)</label>
      <input id="memoryUrl" type="text" bind:value={memoryUrl} placeholder="http://localhost:8090" />
    </div>
    <div>
      <label for="maestroUrl">RBX Maestro URL (optional)</label>
      <input id="maestroUrl" type="text" bind:value={maestroUrl} placeholder="http://localhost:8070" />
    </div>
  </div>
  <button class="primary" onclick={save}>Save</button>
</div>

<style>
  .config-panel {
    margin-bottom: var(--s-3);
  }
</style>
