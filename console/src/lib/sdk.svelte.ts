import { ThalamusClient } from '@rbx/thalamus-sdk';
import { getConfig } from './config.svelte.ts';

let client = $derived.by(() => {
  const cfg = getConfig();
  if (!cfg.baseUrl) return null;
  return new ThalamusClient({
    baseUrl: cfg.baseUrl,
    authHeader: cfg.authHeader,
  });
});

export function getClient(): ThalamusClient | null {
  return client;
}
